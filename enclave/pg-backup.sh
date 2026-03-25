#!/bin/bash
set -euo pipefail

# PostgreSQL-only backup for scheduled runs.
# Zero downtime - uses pg_dump's MVCC snapshot (no service stops).
# Produces a single .tar.gz.enc file in /data/seed/.
#
# For full backups (all data stores), use export.sh (requires stopping services).

# Load non-secret env vars persisted by entrypoint.sh. BACKUP_ENCRYPTION_KEY
# must come from the live inherited process environment, not from disk.
if [ -f /etc/lightfriend/env ]; then
    set -a
    # shellcheck source=/dev/null
    source /etc/lightfriend/env
    set +a
fi

TIMESTAMP=$(date -u +%Y%m%dT%H%M%SZ)
BACKUP_NAME="lightfriend-pg-backup-${TIMESTAMP}"
STAGING="/tmp/pg-backup-staging/${BACKUP_NAME}"
ARCHIVE="/tmp/${BACKUP_NAME}.tar.gz"
ENCRYPTED="/data/seed/${BACKUP_NAME}.tar.gz.enc"
STATUS_FILE="/data/seed/pg-backup-status.json"

cleanup() {
    rm -rf /tmp/pg-backup-staging /tmp/pg-verify.tar.gz "${ARCHIVE}" 2>/dev/null || true
}
trap cleanup EXIT

write_status() {
    local status="$1"
    shift
    if [ "$status" = "SUCCESS" ]; then
        cat > "${STATUS_FILE}" <<EOF
{"status": "SUCCESS", "type": "pg_only", "timestamp": "${TIMESTAMP}", "file": "${BACKUP_NAME}.tar.gz.enc", "size_bytes": $1, "user_count": $2}
EOF
    else
        local error="$1"
        local step="$2"
        cat > "${STATUS_FILE}" <<EOF
{"status": "FAILED", "type": "pg_only", "timestamp": "${TIMESTAMP}", "error": "${error}", "step": "${step}"}
EOF
    fi
}

abort() {
    local msg="$1"
    local step="$2"
    echo "PG BACKUP FAILED: ${msg} (step: ${step})"
    write_status "FAILED" "${msg}" "${step}"
    rm -f "${ENCRYPTED}" 2>/dev/null || true
    exit 1
}

echo "=== Lightfriend PG-Only Backup (zero downtime) ==="
echo "Timestamp: ${TIMESTAMP}"

# ── Preflight ────────────────────────────────────────────────────────────────

if [ -z "${BACKUP_ENCRYPTION_KEY:-}" ]; then
    abort "BACKUP_ENCRYPTION_KEY is not set" "preflight"
fi

USER_COUNT=$(psql -h localhost -U postgres -d lightfriend_db -t -A \
    -c "SELECT count(*) FROM users" 2>/dev/null || echo "0")
if [ "${USER_COUNT}" -eq 0 ]; then
    abort "No users in database - nothing to backup" "preflight"
fi
echo "  User count: ${USER_COUNT}"

# ── Dump all 4 PostgreSQL databases (MVCC - no downtime) ─────────────────

echo "Dumping PostgreSQL databases (live, MVCC snapshot)..."
mkdir -p "${STAGING}/postgres"

for db in lightfriend_db whatsapp_db signal_db telegram_db; do
    echo "  Dumping ${db}..."
    pg_dump -h localhost -U postgres --no-owner --no-acl "${db}" \
        > "${STAGING}/postgres/${db}.sql" 2>>"${STAGING}/pg_dump_errors.log" \
        || abort "pg_dump of ${db} failed" "dump-${db}"
done

# ── Verify dumps ─────────────────────────────────────────────────────────

echo "Verifying dumps..."

for db in lightfriend_db whatsapp_db signal_db telegram_db; do
    dump_file="${STAGING}/postgres/${db}.sql"
    dump_size=$(stat -c%s "${dump_file}" 2>/dev/null || stat -f%z "${dump_file}" 2>/dev/null || echo "0")
    if [ "${dump_size}" -eq 0 ]; then
        abort "pg_dump of ${db} returned empty file" "verify"
    fi
    first_line=$(head -1 "${dump_file}")
    if ! echo "${first_line}" | grep -qE '^(--|SET |CREATE )'; then
        abort "pg_dump of ${db} does not start with valid SQL: ${first_line}" "verify"
    fi
    last_line=$(tail -1 "${dump_file}")
    if ! echo "${last_line}" | grep -qE '(PostgreSQL database dump complete|^$)'; then
        abort "pg_dump of ${db} appears truncated (no completion footer)" "verify"
    fi
    echo "  ${db}.sql: ${dump_size} bytes, valid"
done

DATA_STMT_COUNT=$(grep -c "^COPY\|^INSERT INTO" "${STAGING}/postgres/lightfriend_db.sql" 2>/dev/null || echo "0")
if [ "${DATA_STMT_COUNT}" -eq 0 ]; then
    abort "lightfriend_db dump contains no data statements" "verify"
fi

# ── Assemble archive ─────────────────────────────────────────────────────

echo "Assembling archive..."

cd "${STAGING}"
find . -type f -print0 | sort -z | xargs -0 sha256sum > checksums.sha256

cat > manifest.json <<EOF
{
    "version": "1.0",
    "format": "lightfriend-pg-backup",
    "timestamp": "${TIMESTAMP}",
    "user_count": ${USER_COUNT},
    "components": [
        "postgres/lightfriend_db.sql",
        "postgres/whatsapp_db.sql",
        "postgres/signal_db.sql",
        "postgres/telegram_db.sql"
    ]
}
EOF
sha256sum manifest.json >> checksums.sha256

cd /tmp/pg-backup-staging
tar czf "${ARCHIVE}" "${BACKUP_NAME}" \
    || abort "tar.gz creation failed" "assemble"

tar tzf "${ARCHIVE}" > /dev/null 2>&1 \
    || abort "tar.gz round-trip verification failed" "assemble"

ARCHIVE_SIZE=$(stat -c%s "${ARCHIVE}" 2>/dev/null || stat -f%z "${ARCHIVE}" 2>/dev/null || echo "0")
echo "  Archive: ${ARCHIVE_SIZE} bytes"

# ── Encrypt ──────────────────────────────────────────────────────────────

echo "Encrypting..."

ORIGINAL_SHA=$(sha256sum "${ARCHIVE}" | awk '{print $1}')

openssl enc -aes-256-cbc -pbkdf2 -iter 600000 -salt \
    -pass env:BACKUP_ENCRYPTION_KEY \
    -in "${ARCHIVE}" \
    -out "${ENCRYPTED}" \
    || abort "Encryption failed" "encrypt"

openssl enc -d -aes-256-cbc -pbkdf2 -iter 600000 \
    -pass env:BACKUP_ENCRYPTION_KEY \
    -in "${ENCRYPTED}" \
    -out /tmp/pg-verify.tar.gz \
    || abort "Decrypt verification failed" "verify-encrypt"

VERIFY_SHA=$(sha256sum /tmp/pg-verify.tar.gz | awk '{print $1}')
if [ "${ORIGINAL_SHA}" != "${VERIFY_SHA}" ]; then
    rm -f "${ENCRYPTED}"
    abort "SHA-256 mismatch after decrypt" "verify-encrypt"
fi
rm -f /tmp/pg-verify.tar.gz

ENCRYPTED_SIZE=$(stat -c%s "${ENCRYPTED}" 2>/dev/null || stat -f%z "${ENCRYPTED}" 2>/dev/null || echo "0")

# ── Transfer to host via VSOCK ───────────────────────────────────────────
if [ -e /dev/vsock ]; then
    echo "Transferring to host via VSOCK..."
    socat -u FILE:"${ENCRYPTED}" VSOCK-CONNECT:3:9001 \
        || abort "VSOCK transfer to host failed" "transfer"
fi

write_status "SUCCESS" "${ENCRYPTED_SIZE}" "${USER_COUNT}"

echo ""
echo "=== PG Backup Complete ==="
echo "File: ${ENCRYPTED}"
echo "Size: ${ENCRYPTED_SIZE} bytes"
echo "Users: ${USER_COUNT}"
echo "Downtime: none"
