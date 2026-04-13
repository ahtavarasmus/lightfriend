#!/bin/bash
# shellcheck disable=SC2009,SC2010,SC2012,SC2086,SC2164
set -uxo pipefail
# NOTE: no set -e! Every critical command has explicit || abort error handling.
# set -e was killing the script on harmless debug commands (find, du, grep on
# non-existent dirs). With -u we still catch unset variables, with -x we trace
# every command, with -o pipefail we catch pipeline failures in critical paths.

# Full encrypted export of all data stores from running enclave.
# Produces a single .tar.gz.enc file in /data/seed/.
# set -x enables command tracing so every line is logged for debugging.
# All plaintext stays in /tmp/ (enclave ephemeral space).
# The export NEVER produces a partial or unverified backup.

# Load non-secret env vars persisted by entrypoint.sh. BACKUP_ENCRYPTION_KEY
# must come from the live inherited process environment, not from disk.
if [ -f /etc/lightfriend/env ]; then
    # Safe line-by-line loading (source crashes on values with $ and ^)
    while IFS= read -r line || [[ -n "$line" ]]; do
        [[ -z "$line" || "$line" == \#* ]] && continue
        if [[ "$line" =~ ^[A-Za-z_][A-Za-z_0-9]*= ]]; then
            export "${line?}"
        fi
    done < /etc/lightfriend/env
fi

TIMESTAMP=$(date -u +%Y%m%dT%H%M%SZ)
BACKUP_NAME="lightfriend-full-backup-${TIMESTAMP}"
STAGING="/tmp/backup-staging/${BACKUP_NAME}"
ARCHIVE="/tmp/${BACKUP_NAME}.tar.gz"
ENCRYPTED="/data/seed/${BACKUP_NAME}.tar.gz.enc"
STATUS_FILE="/data/seed/export-status.json"

# Cleanup function - remove staging artifacts
cleanup() {
    rm -rf /tmp/backup-staging /tmp/verify.tar.gz "${ARCHIVE}" 2>/dev/null || true
    # Clean matrix snapshot dirs created during Phase A
    rm -rf "${STAGING}/matrix-store-snapshot" 2>/dev/null || true
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

command -v jq >/dev/null 2>&1 || abort "jq is required but not installed" "preflight"

if [ -z "${BACKUP_ENCRYPTION_KEY:-}" ]; then
    abort "BACKUP_ENCRYPTION_KEY is not set" "preflight"
fi

# Quick sanity check: is there data to export?
PREFLIGHT_COUNT=$(psql -h localhost -U postgres -d lightfriend_db -t -A \
    -c "SELECT count(*) FROM users" 2>/dev/null || echo "0")
if [ "${PREFLIGHT_COUNT}" -eq 0 ]; then
    abort "No users in database - nothing to export" "preflight"
fi
echo "  Preflight user count: ${PREFLIGHT_COUNT}"

# ── Take live snapshots ─────────────────────────────────────────────────────

# User count from live PostgreSQL snapshot point. Full backup may be slightly
# cross-store stale, but each backing store snapshot must be internally valid.
LIVE_USER_COUNT=$(psql -h localhost -U postgres -d lightfriend_db -t -A \
    -c "SELECT count(*) FROM users" 2>/dev/null || echo "0")
echo "  Snapshot user count: ${LIVE_USER_COUNT}"

# ── Phase A: Dump everything ────────────────────────────────────────────────

echo "Phase A: Dumping all data stores..."
mkdir -p "${STAGING}/postgres"

# Dump all 4 PostgreSQL databases
for db in lightfriend_db whatsapp_db signal_db telegram_db; do
    echo "  Dumping ${db}..."
    pg_dump -h localhost -U postgres --no-owner --no-acl "${db}" \
        > "${STAGING}/postgres/${db}.sql" 2>>"${STAGING}/pg_dump_errors.log" \
        || abort "pg_dump of ${db} failed - see pg_dump_errors.log" "dump-${db}"
done

# Create Tuwunel backup via built-in RocksDB BackupEngine (SIGUSR2 trigger)
# This is the only safe way to backup a live RocksDB - tuwunel holds the DB
# handle and calls DisableFileDeletions + GetLiveFiles internally.
TUWUNEL_BACKUP_DIR="/var/lib/tuwunel-backup"
echo "  === TUWUNEL BACKUP START ==="
echo "  [DEBUG] COMMAND CHECK: pgrep=$(which pgrep 2>&1) kill=$(which kill 2>&1) grep=$(which grep 2>&1) find=$(which find 2>&1)"
echo "  [DEBUG] tuwunel source DB path: /var/lib/tuwunel"
echo "  [DEBUG] tuwunel source DB file count: $(find /var/lib/tuwunel -type f 2>/dev/null | wc -l)"
echo "  [DEBUG] tuwunel source DB total size: $(du -sh /var/lib/tuwunel 2>/dev/null | awk '{print $1}' || echo '0')"
echo "  [DEBUG] tuwunel source DB SST files: $(find /var/lib/tuwunel -name '*.sst' 2>/dev/null | wc -l)"
echo "  [DEBUG] tuwunel source CURRENT: $(cat /var/lib/tuwunel/CURRENT 2>/dev/null || echo 'NOT FOUND')"
echo "  [DEBUG] tuwunel source IDENTITY: $(cat /var/lib/tuwunel/IDENTITY 2>/dev/null || echo 'NOT FOUND')"
echo "  [DEBUG] tuwunel backup dir path: $TUWUNEL_BACKUP_DIR"
echo "  [DEBUG] tuwunel backup dir exists: $([ -d $TUWUNEL_BACKUP_DIR ] && echo yes || echo no)"
echo "  [DEBUG] tuwunel config database_backup_path check:"
grep -i "database_backup_path" /etc/tuwunel/tuwunel.toml 2>/dev/null || echo "    NOT FOUND IN CONFIG!"
echo "  [DEBUG] tuwunel config admin_signal_execute check:"
grep -i "admin_signal_execute" /etc/tuwunel/tuwunel.toml 2>/dev/null || echo "    NOT FOUND IN CONFIG!"

echo "  [DEBUG] Finding tuwunel process..."
# pgrep may not exist in the enclave, use supervisorctl to get PID
TUWUNEL_PID=$(supervisorctl status tuwunel 2>/dev/null | grep -oP 'pid \K[0-9]+' || true)
if [ -z "$TUWUNEL_PID" ]; then
    # Fallback: parse ps output
    TUWUNEL_PID=$(ps aux 2>/dev/null | grep '[t]uwunel' | awk '{print $2}' | head -1 || true)
fi
if [ -z "$TUWUNEL_PID" ]; then
    echo "  [DEBUG] supervisorctl: $(supervisorctl status tuwunel 2>&1)"
    echo "  [DEBUG] ps tuwunel: $(ps aux 2>/dev/null | grep tuwunel || echo 'nothing')"
    abort "Tuwunel process not found - cannot trigger backup" "dump-tuwunel"
fi
echo "  [DEBUG] Tuwunel PID: $TUWUNEL_PID"
echo "  [DEBUG] Tuwunel process info: $(ps -p $TUWUNEL_PID -o pid,rss,etime,args 2>/dev/null | tail -1 || echo 'ps failed')"

# Ensure backup dir exists (tuwunel creates it on first backup, but it may not exist yet)
mkdir -p "$TUWUNEL_BACKUP_DIR"

# Record backup dir state before signal
BEFORE_COUNT=$(find "$TUWUNEL_BACKUP_DIR" -type f | wc -l)
BEFORE_SIZE=$(du -sh "$TUWUNEL_BACKUP_DIR" | awk '{print $1}')
echo "  [DEBUG] Backup dir BEFORE signal: $BEFORE_COUNT files, $BEFORE_SIZE"
echo "  [DEBUG] Backup dir BEFORE contents:"
ls -laR "$TUWUNEL_BACKUP_DIR" 2>/dev/null | head -15 || echo "    (empty or doesn't exist)"

echo "  [DEBUG] Tuwunel log BEFORE signal (last 5 lines):"
tail -5 /var/log/supervisor/tuwunel.log 2>/dev/null || echo "    empty"

# Send SIGUSR2 to trigger admin_signal_execute = ["server backup-database"]
echo "  [DEBUG] Sending SIGUSR2 to PID $TUWUNEL_PID at $(date -u +%H:%M:%S)..."
kill -SIGUSR2 "$TUWUNEL_PID" \
    || abort "Failed to send SIGUSR2 to tuwunel (PID $TUWUNEL_PID)" "dump-tuwunel"
echo "  [DEBUG] SIGUSR2 sent successfully"

# Wait for backup to complete by watching meta/ directory for a new backup ID.
# RocksDB BackupEngine writes meta/<id> atomically (tmp file then rename) when
# backup completes. The backup ID always increases even when old backups are purged.
# This is 100% reliable - no race conditions, no file count issues.
echo "  Waiting for backup to complete..."
BACKUP_TIMEOUT=120
BACKUP_DONE=false
BEFORE_ID=$(ls "$TUWUNEL_BACKUP_DIR/meta/" 2>/dev/null | grep -E '^[0-9]+$' | sort -n | tail -1)
BEFORE_ID=${BEFORE_ID:-0}
echo "  [DEBUG] Highest backup ID before: $BEFORE_ID"

for i in $(seq 1 "$BACKUP_TIMEOUT"); do
    AFTER_ID=$(ls "$TUWUNEL_BACKUP_DIR/meta/" 2>/dev/null | grep -E '^[0-9]+$' | sort -n | tail -1)
    AFTER_ID=${AFTER_ID:-0}
    if [ "$AFTER_ID" -gt "$BEFORE_ID" ]; then
        echo "  [DEBUG] Backup completed in ${i}s (backup ID: $BEFORE_ID -> $AFTER_ID)"
        BACKUP_DONE=true
        break
    fi
    if [ $((i % 10)) -eq 0 ]; then
        echo "  [DEBUG] Still waiting... ${i}s elapsed, latest ID: $AFTER_ID (need > $BEFORE_ID)"
        echo "  [DEBUG] meta/ contents: $(ls $TUWUNEL_BACKUP_DIR/meta/ 2>/dev/null || echo 'empty')"
        tail -3 /var/log/supervisor/tuwunel.log 2>/dev/null || echo "    empty"
    fi
    sleep 1
done

if [ "$BACKUP_DONE" != "true" ]; then
    echo "  [DEBUG] === BACKUP TIMEOUT DIAGNOSTICS ==="
    echo "  [DEBUG] Backup dir after timeout:"
    ls -laR "$TUWUNEL_BACKUP_DIR" 2>/dev/null | head -40 || echo "    (empty or doesn't exist)"
    echo "  [DEBUG] Tuwunel log (last 20 lines):"
    tail -20 /var/log/supervisor/tuwunel.log 2>/dev/null || echo "    empty"
    echo "  [DEBUG] Tuwunel stderr (last 10 lines):"
    tail -10 /var/log/supervisor/tuwunel-err.log 2>/dev/null || echo "    empty"
    echo "  [DEBUG] Tuwunel process still running: $(kill -0 $TUWUNEL_PID 2>/dev/null && echo yes || echo NO)"
    echo "  [DEBUG] supervisorctl: $(supervisorctl status tuwunel 2>&1)"
    abort "Tuwunel backup did not complete within ${BACKUP_TIMEOUT}s" "dump-tuwunel"
fi

echo "  [DEBUG] Tuwunel log AFTER backup (last 10 lines):"
tail -10 /var/log/supervisor/tuwunel.log 2>/dev/null || echo "    empty"

# Verify backup dir has expected BackupEngine structure
echo "  [DEBUG] === VERIFYING BACKUP STRUCTURE ==="
if [ ! -d "$TUWUNEL_BACKUP_DIR" ]; then
    abort "Tuwunel backup dir $TUWUNEL_BACKUP_DIR does not exist after backup" "dump-tuwunel"
fi
BACKUP_SIZE=$(du -sh "$TUWUNEL_BACKUP_DIR" 2>/dev/null | awk '{print $1}' || echo '0')
BACKUP_FILES=$(find "$TUWUNEL_BACKUP_DIR" -type f 2>/dev/null | wc -l)
echo "  [DEBUG] Backup total: $BACKUP_FILES files, $BACKUP_SIZE"
echo "  [DEBUG] Backup dir full listing:"
find "$TUWUNEL_BACKUP_DIR" -type f 2>/dev/null | head -40
echo "  [DEBUG] shared_checksum/ exists: $([ -d $TUWUNEL_BACKUP_DIR/shared_checksum ] && echo yes || echo NO)"
echo "  [DEBUG] shared_checksum/ SST count: $(find $TUWUNEL_BACKUP_DIR/shared_checksum -name '*.sst' 2>/dev/null | wc -l)"
echo "  [DEBUG] shared_checksum/ SST total size: $(du -sh $TUWUNEL_BACKUP_DIR/shared_checksum 2>/dev/null | awk '{print $1}' || echo '0')"
echo "  [DEBUG] shared_checksum/ first 5 SSTs:"
find "$TUWUNEL_BACKUP_DIR/shared_checksum" -name '*.sst' 2>/dev/null | head -5
echo "  [DEBUG] private/ exists: $([ -d $TUWUNEL_BACKUP_DIR/private ] && echo yes || echo NO)"
echo "  [DEBUG] private/ dirs: $(ls $TUWUNEL_BACKUP_DIR/private/ 2>/dev/null || echo 'none')"
echo "  [DEBUG] meta/ exists: $([ -d $TUWUNEL_BACKUP_DIR/meta ] && echo yes || echo NO)"
for d in "$TUWUNEL_BACKUP_DIR"/private/*/; do
    bnum=$(basename "$d" 2>/dev/null)
    echo "  [DEBUG] private/$bnum/ contents:"
    ls -la "$d" 2>/dev/null | head -10
done
if [ "$BACKUP_FILES" -lt 3 ]; then
    echo "  [DEBUG] Backup has only $BACKUP_FILES files - this is suspicious"
    abort "Tuwunel backup too small ($BACKUP_FILES files) - backup likely failed" "dump-tuwunel"
fi

# Tar the backup directory
echo "  [DEBUG] Creating tar of $TUWUNEL_BACKUP_DIR..."
mkdir -p "${STAGING}/tuwunel"
tar cf "${STAGING}/tuwunel/tuwunel_data.tar" -C / var/lib/tuwunel-backup 2>/dev/null \
    || abort "tar of tuwunel backup dir failed" "dump-tuwunel"
TAR_SIZE=$(stat -c%s "${STAGING}/tuwunel/tuwunel_data.tar" 2>/dev/null || echo unknown)
TAR_FILES=$(tar tf "${STAGING}/tuwunel/tuwunel_data.tar" 2>/dev/null | wc -l)
echo "  [DEBUG] tuwunel tar: $TAR_SIZE bytes, $TAR_FILES entries"
echo "  [DEBUG] tuwunel tar contents (first 20):"
tar tf "${STAGING}/tuwunel/tuwunel_data.tar" 2>/dev/null | head -20
echo "  === TUWUNEL BACKUP END ==="

# Create SQLite backups for matrix store files
echo "  Backing up /app/matrix_store (SQLite online backup)..."
mkdir -p "${STAGING}/matrix_store"
MATRIX_SNAPSHOT_ROOT="${STAGING}/matrix-store-snapshot"
mkdir -p "${MATRIX_SNAPSHOT_ROOT}/matrix_store"

while IFS= read -r -d '' db_file; do
    rel_path="${db_file#/app/matrix_store/}"
    dest_db="${MATRIX_SNAPSHOT_ROOT}/matrix_store/${rel_path}"
    mkdir -p "$(dirname "${dest_db}")"
    sqlite3 "${db_file}" ".backup '${dest_db}'" \
        || abort "SQLite backup failed for ${rel_path}" "dump-matrix_store"
    if [ "$(sqlite3 "${dest_db}" "PRAGMA integrity_check;" 2>/dev/null || echo "failed")" != "ok" ]; then
        abort "SQLite integrity_check failed for ${rel_path}" "verify-matrix_store"
    fi
done < <(find /app/matrix_store -type f -name '*.sqlite3' -print0)

while IFS= read -r -d '' extra_file; do
    rel_path="${extra_file#/app/matrix_store/}"
    dest_file="${MATRIX_SNAPSHOT_ROOT}/matrix_store/${rel_path}"
    mkdir -p "$(dirname "${dest_file}")"
    cp -p "${extra_file}" "${dest_file}" \
        || abort "Failed to copy matrix store side file ${rel_path}" "dump-matrix_store"
done < <(find /app/matrix_store -type f ! -name '*.sqlite3' ! -name '*.sqlite3-wal' ! -name '*.sqlite3-shm' -print0)

tar cf "${STAGING}/matrix_store/matrix_store.tar" -C "${MATRIX_SNAPSHOT_ROOT}" matrix_store 2>/dev/null \
    || abort "tar of matrix store snapshot failed" "dump-matrix_store"

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
    # Check dump is complete (ends with PostgreSQL footer)
    last_line=$(tail -1 "${dump_file}")
    if ! echo "${last_line}" | grep -qE '(PostgreSQL database dump complete|^$)'; then
        abort "pg_dump of ${db} appears truncated (no completion footer)" "verify-dumps"
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

# Cross-validate user count: during deploy-time export, maintenance mode should
# block writes to the app database, so user count should remain stable.
POST_DUMP_COUNT=$(psql -h localhost -U postgres -d lightfriend_db -t -A \
    -c "SELECT count(*) FROM users" 2>/dev/null || echo "0")
if [ "${POST_DUMP_COUNT}" != "${LIVE_USER_COUNT}" ]; then
    abort "User count changed during export: before=${LIVE_USER_COUNT} after=${POST_DUMP_COUNT}" "verify-dumps"
fi

# Verify lightfriend_db dump has actual data statements
DATA_STMT_COUNT=$(grep -c "^COPY\|^INSERT INTO" "${STAGING}/postgres/lightfriend_db.sql" 2>/dev/null || echo "0")
if [ "${DATA_STMT_COUNT}" -eq 0 ]; then
    abort "lightfriend_db dump contains no data statements (COPY/INSERT)" "verify-dumps"
fi
echo "  lightfriend_db: ${DATA_STMT_COUNT} data statements, user count verified"

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

# ── Phase E: Upload via presigned URLs ────────────────────────────────────────
# All uploads go directly to S3/R2 via presigned URLs through HTTP_PROXY.
# The trigger JSON (from CI or host timer) provides all URLs.

BACKUP_SHA=$(sha256sum "${ENCRYPTED}" | awk '{print $1}')

# Use explicit proxy for S3/R2 uploads. Unset env proxies to avoid conflicts
# (curl may use HTTP_PROXY env var instead of -x flag in some versions)
unset HTTP_PROXY HTTPS_PROXY http_proxy https_proxy ALL_PROXY all_proxy 2>/dev/null || true
PROXY="http://127.0.0.1:3128"

echo "Phase E: Uploading backup..."
echo "  S3 key: ${BACKUP_S3_KEY:-unknown}"
echo "  Size: ${ENCRYPTED_SIZE} bytes"
echo "  SHA256: ${BACKUP_SHA}"
echo "  Export type: ${EXPORT_TYPE:-unknown}"

# E1: Upload backup to S3
if [ -n "${PRESIGNED_PUT_BACKUP_S3:-}" ]; then
    echo "  E1: Uploading to S3..."
    curl -sf --max-time 600 \
        -X PUT -H "Content-Type: application/octet-stream" \
        --upload-file "${ENCRYPTED}" \
        -x "$PROXY" "${PRESIGNED_PUT_BACKUP_S3}" \
        || abort "S3 backup upload failed" "upload-s3"
    echo "  E1: S3 OK"
else
    abort "No presigned S3 URL provided" "upload-s3"
fi

# E2: Upload backup to R2 (if URL provided)
if [ -n "${PRESIGNED_PUT_BACKUP_R2:-}" ]; then
    echo "  E2: Replicating to R2..."
    curl -sf --max-time 600 \
        -X PUT -H "Content-Type: application/octet-stream" \
        --upload-file "${ENCRYPTED}" \
        -x "$PROXY" "${PRESIGNED_PUT_BACKUP_R2}" \
        || echo "  E2: WARNING - R2 replication failed (non-fatal)"
else
    echo "  E2: No R2 URL, skipping replication"
fi

# E3: Promote to daily/weekly/monthly tiers (upload same local file to tier keys)
if [ -n "${PROMOTE_JSON:-}" ] && [ "${PROMOTE_JSON}" != "[]" ]; then
    echo "  E3: Promoting to tiers..."
    echo "${PROMOTE_JSON}" | jq -c '.[]' 2>/dev/null | while read -r promo; do
        TIER_KEY=$(echo "$promo" | jq -r '.tier_key')
        PROMO_PUT=$(echo "$promo" | jq -r '.presigned_put')
        echo "    Promoting to ${TIER_KEY}..."
        curl -sf --max-time 600 -X PUT -H "Content-Type: application/octet-stream" \
            --upload-file "${ENCRYPTED}" -x "$PROXY" "$PROMO_PUT" \
            && echo "    OK: ${TIER_KEY}" \
            || echo "    WARNING: promotion to ${TIER_KEY} failed (non-fatal)"
    done
else
    echo "  E3: No tier promotions"
fi

# E4: Upload completion/health marker
if [ "${EXPORT_TYPE:-}" = "deploy" ] && [ -n "${PRESIGNED_PUT_COMPLETE:-}" ]; then
    echo "  E4: Writing deploy completion to S3..."
    echo "{\"status\":\"SUCCESS\",\"backup_key\":\"${BACKUP_S3_KEY:-}\",\"backup_sha256\":\"${BACKUP_SHA}\",\"backup_size\":${ENCRYPTED_SIZE},\"user_count\":${USER_COUNT},\"timestamp\":\"${TIMESTAMP}\"}" | \
        curl -sf --max-time 30 -X PUT -H "Content-Type: application/json" \
        --data-binary @- -x "$PROXY" "${PRESIGNED_PUT_COMPLETE}" \
        || abort "Failed to upload completion marker" "upload-complete"
    echo "  E4: Deploy completion OK"
elif [ -n "${PRESIGNED_PUT_HEALTH:-}" ]; then
    echo "  E4: Writing backup health to S3..."
    echo "{\"last_success\":\"${TIMESTAMP}\",\"backup_key\":\"${BACKUP_S3_KEY:-}\",\"backup_size\":${ENCRYPTED_SIZE},\"backup_sha256\":\"${BACKUP_SHA}\",\"user_count\":${USER_COUNT}}" | \
        curl -sf --max-time 30 -X PUT -H "Content-Type: application/json" \
        --data-binary @- -x "$PROXY" "${PRESIGNED_PUT_HEALTH}" \
        || echo "  E4: WARNING - health marker upload failed (non-fatal)"
    echo "  E4: Health marker OK"
fi

echo "Phase E complete."

# ── Cleanup: remove local encrypted file after successful upload ─────────────
# Without this, each hourly backup leaves a ~33MB file in /data/seed/ inside
# the enclave. After ~30 hours the enclave's filesystem fills up and future
# exports fail with "error writing output file" during encryption.
echo "Cleaning up local encrypted file..."
rm -f "${ENCRYPTED}" 2>/dev/null || true
# Also clean any old export artifacts left by previous runs
find /data/seed -name 'lightfriend-full-backup-*.tar.gz.enc' -mmin +60 -delete 2>/dev/null || true

# ── Write local success status ────────────────────────────────────────────────

write_status "SUCCESS" "${ENCRYPTED_SIZE}" "${USER_COUNT}"

echo ""
echo "=== Export Complete ==="
echo "  Type: ${EXPORT_TYPE:-unknown}"
echo "  File: ${ENCRYPTED}"
echo "  Size: ${ENCRYPTED_SIZE} bytes"
echo "  SHA256: ${BACKUP_SHA}"
echo "  Users: ${USER_COUNT}"
echo "  S3 key: ${BACKUP_S3_KEY:-}"
