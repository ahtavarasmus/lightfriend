#!/bin/bash
set -euo pipefail

# Full encrypted export of all data stores from running enclave.
# Produces a single .tar.gz.enc file in /data/seed/.
# All plaintext stays in /tmp/ (enclave ephemeral space).
# The export NEVER produces a partial or unverified backup.

TIMESTAMP=$(date -u +%Y%m%dT%H%M%SZ)
BACKUP_NAME="lightfriend-full-backup-${TIMESTAMP}"
STAGING="/tmp/backup-staging/${BACKUP_NAME}"
ARCHIVE="/tmp/${BACKUP_NAME}.tar.gz"
ENCRYPTED="/data/seed/${BACKUP_NAME}.tar.gz.enc"
STATUS_FILE="/data/seed/export-status.json"

# Cleanup function - restart services and remove staging
cleanup() {
    echo "Restarting services..."
    supervisorctl start tuwunel 2>/dev/null || true
    sleep 2
    supervisorctl start mautrix-whatsapp 2>/dev/null || true
    supervisorctl start mautrix-signal 2>/dev/null || true
    supervisorctl start mautrix-telegram 2>/dev/null || true
    sleep 1
    supervisorctl start lightfriend 2>/dev/null || true
    rm -rf /tmp/backup-staging /tmp/verify.tar.gz "${ARCHIVE}" 2>/dev/null || true
    echo "Services restarted."
}
trap cleanup EXIT

write_status() {
    local status="$1"
    shift
    if [ "$status" = "SUCCESS" ]; then
        cat > "${STATUS_FILE}" <<EOF
{"status": "SUCCESS", "timestamp": "${TIMESTAMP}", "file": "${BACKUP_NAME}.tar.gz.enc", "size_bytes": $1, "user_count": $2}
EOF
    else
        local error="$1"
        local step="$2"
        cat > "${STATUS_FILE}" <<EOF
{"status": "FAILED", "timestamp": "${TIMESTAMP}", "error": "${error}", "step": "${step}"}
EOF
    fi
}

abort() {
    local msg="$1"
    local step="$2"
    echo "EXPORT FAILED: ${msg} (step: ${step})"
    write_status "FAILED" "${msg}" "${step}"
    rm -f "${ENCRYPTED}" 2>/dev/null || true
    exit 1
}

echo "=== Lightfriend Full Data Export ==="
echo "Timestamp: ${TIMESTAMP}"

# ── Preflight ────────────────────────────────────────────────────────────────

if [ -z "${BACKUP_ENCRYPTION_KEY:-}" ]; then
    abort "BACKUP_ENCRYPTION_KEY is not set" "preflight"
fi

# ── Stop services (PG stays up - MVCC snapshots) ────────────────────────────

echo "Stopping services for consistent snapshot..."
supervisorctl stop lightfriend 2>/dev/null || true
sleep 1
supervisorctl stop mautrix-whatsapp 2>/dev/null || true
supervisorctl stop mautrix-signal 2>/dev/null || true
supervisorctl stop mautrix-telegram 2>/dev/null || true
sleep 1
supervisorctl stop tuwunel 2>/dev/null || true
sleep 2
echo "Services stopped."

# ── Phase A: Dump everything ────────────────────────────────────────────────

echo "Phase A: Dumping all data stores..."
mkdir -p "${STAGING}/postgres"

# Dump all 4 PostgreSQL databases
for db in lightfriend_db whatsapp_db signal_db telegram_db; do
    echo "  Dumping ${db}..."
    pg_dump -h localhost -U postgres --no-owner --no-acl "${db}" \
        > "${STAGING}/postgres/${db}.sql" 2>/dev/null \
        || abort "pg_dump of ${db} failed" "dump-${db}"
done

# Tar filesystem stores
echo "  Archiving /var/lib/tuwunel (RocksDB)..."
mkdir -p "${STAGING}/tuwunel"
tar cf "${STAGING}/tuwunel/tuwunel_data.tar" -C / var/lib/tuwunel 2>/dev/null \
    || abort "tar of /var/lib/tuwunel failed" "dump-tuwunel"

echo "  Archiving /app/matrix_store (per-user SQLite)..."
mkdir -p "${STAGING}/matrix_store"
tar cf "${STAGING}/matrix_store/matrix_store.tar" -C /app matrix_store 2>/dev/null \
    || abort "tar of /app/matrix_store failed" "dump-matrix_store"

echo "  Archiving /data/bridges (registrations + device state)..."
mkdir -p "${STAGING}/bridges"
tar cf "${STAGING}/bridges/bridge_data.tar" -C /data bridges 2>/dev/null \
    || abort "tar of /data/bridges failed" "dump-bridges"

echo "  Archiving /app/uploads (user media)..."
mkdir -p "${STAGING}/uploads"
tar cf "${STAGING}/uploads/uploads.tar" -C /app uploads 2>/dev/null \
    || abort "tar of /app/uploads failed" "dump-uploads"

echo "  Archiving /app/data (core state)..."
mkdir -p "${STAGING}/core_data"
tar cf "${STAGING}/core_data/core_data.tar" -C /app data 2>/dev/null \
    || abort "tar of /app/data failed" "dump-core_data"

echo "Phase A complete."

# ── Phase B: Verify every component ─────────────────────────────────────────

echo "Phase B: Verifying all dumps..."

# Verify PG dumps - must be non-empty and start with valid SQL
for db in lightfriend_db whatsapp_db signal_db telegram_db; do
    dump_file="${STAGING}/postgres/${db}.sql"
    dump_size=$(stat -c%s "${dump_file}" 2>/dev/null || stat -f%z "${dump_file}" 2>/dev/null || echo "0")
    if [ "${dump_size}" -eq 0 ]; then
        abort "pg_dump of ${db} returned empty file" "verify-dumps"
    fi
    # Check starts with valid SQL (-- comment, SET, or CREATE)
    first_line=$(head -1 "${dump_file}")
    if ! echo "${first_line}" | grep -qE '^(--|SET |CREATE )'; then
        abort "pg_dump of ${db} does not start with valid SQL: ${first_line}" "verify-dumps"
    fi
    echo "  ${db}.sql: ${dump_size} bytes, valid SQL header"
done

# Verify tar archives
for tar_path in \
    "${STAGING}/tuwunel/tuwunel_data.tar" \
    "${STAGING}/matrix_store/matrix_store.tar" \
    "${STAGING}/bridges/bridge_data.tar" \
    "${STAGING}/uploads/uploads.tar" \
    "${STAGING}/core_data/core_data.tar"; do
    tar_name=$(basename "${tar_path}")
    tar tf "${tar_path}" > /dev/null 2>&1 \
        || abort "tar integrity check failed for ${tar_name}" "verify-tars"
    tar_size=$(stat -c%s "${tar_path}" 2>/dev/null || stat -f%z "${tar_path}" 2>/dev/null || echo "0")
    echo "  ${tar_name}: ${tar_size} bytes, integrity OK"
done

echo "Phase B complete."

# ── Phase C: Assemble archive ───────────────────────────────────────────────

echo "Phase C: Assembling archive..."

# Generate checksums
cd "${STAGING}"
find . -type f -print0 | sort -z | xargs -0 sha256sum > checksums.sha256
echo "  Generated checksums.sha256"

# Get user count for manifest
USER_COUNT=$(psql -h localhost -U postgres -d lightfriend_db -t -A -c "SELECT count(*) FROM users" 2>/dev/null || echo "0")
echo "  User count: ${USER_COUNT}"

# Write manifest
cat > manifest.json <<EOF
{
    "version": "1.0",
    "format": "lightfriend-full-backup",
    "timestamp": "${TIMESTAMP}",
    "user_count": ${USER_COUNT},
    "components": [
        "postgres/lightfriend_db.sql",
        "postgres/whatsapp_db.sql",
        "postgres/signal_db.sql",
        "postgres/telegram_db.sql",
        "tuwunel/tuwunel_data.tar",
        "matrix_store/matrix_store.tar",
        "bridges/bridge_data.tar",
        "uploads/uploads.tar",
        "core_data/core_data.tar"
    ]
}
EOF
echo "  Generated manifest.json"

# Update checksums to include manifest
sha256sum manifest.json >> checksums.sha256

# Create tar.gz
cd /tmp/backup-staging
tar czf "${ARCHIVE}" "${BACKUP_NAME}" \
    || abort "tar.gz creation failed" "assemble-archive"

# Verify tar.gz round-trip
tar tzf "${ARCHIVE}" > /dev/null 2>&1 \
    || abort "tar.gz round-trip verification failed" "verify-archive"

ARCHIVE_SIZE=$(stat -c%s "${ARCHIVE}" 2>/dev/null || stat -f%z "${ARCHIVE}" 2>/dev/null || echo "0")
echo "  Archive: ${ARCHIVE_SIZE} bytes, integrity OK"
echo "Phase C complete."

# ── Phase D: Encrypt and output ─────────────────────────────────────────────

echo "Phase D: Encrypting..."

# Compute SHA-256 of original archive
ORIGINAL_SHA=$(sha256sum "${ARCHIVE}" | awk '{print $1}')

# Encrypt
openssl enc -aes-256-cbc -pbkdf2 -iter 600000 -salt \
    -pass env:BACKUP_ENCRYPTION_KEY \
    -in "${ARCHIVE}" \
    -out "${ENCRYPTED}" \
    || abort "Encryption failed" "encrypt"

# Verify: decrypt and compare
openssl enc -d -aes-256-cbc -pbkdf2 -iter 600000 \
    -pass env:BACKUP_ENCRYPTION_KEY \
    -in "${ENCRYPTED}" \
    -out /tmp/verify.tar.gz \
    || abort "Decrypt verification failed - encrypted file may be corrupt" "verify-encrypt"

VERIFY_SHA=$(sha256sum /tmp/verify.tar.gz | awk '{print $1}')
if [ "${ORIGINAL_SHA}" != "${VERIFY_SHA}" ]; then
    rm -f "${ENCRYPTED}"
    abort "SHA-256 mismatch after decrypt: original=${ORIGINAL_SHA} verify=${VERIFY_SHA}" "verify-encrypt"
fi

rm -f /tmp/verify.tar.gz

ENCRYPTED_SIZE=$(stat -c%s "${ENCRYPTED}" 2>/dev/null || stat -f%z "${ENCRYPTED}" 2>/dev/null || echo "0")
echo "  Encrypted: ${ENCRYPTED_SIZE} bytes, decrypt-verify OK"
echo "Phase D complete."

# ── Phase E: Transfer to host via VSOCK ──────────────────────────────────────
if [ -e /dev/vsock ]; then
    echo "Phase E: Transferring backup to host via VSOCK..."
    socat -u FILE:"${ENCRYPTED}" VSOCK-CONNECT:3:9001 \
        || abort "VSOCK transfer to host failed" "transfer"
    echo "Phase E complete."
fi

# ── Write success status ────────────────────────────────────────────────────

write_status "SUCCESS" "${ENCRYPTED_SIZE}" "${USER_COUNT}"

echo ""
echo "=== Export Complete ==="
echo "File: ${ENCRYPTED}"
echo "Size: ${ENCRYPTED_SIZE} bytes"
echo "Users: ${USER_COUNT}"
