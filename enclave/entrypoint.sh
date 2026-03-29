#!/bin/bash
set -euo pipefail

# ── Boot trace: capture ALL output to a log file ────────────────────────────
# In normal Nitro mode the console is invisible. This log gets sent to the
# host via VSOCK on every exit so we always have diagnostics.
BOOT_TRACE="/data/seed/boot-trace.log"
mkdir -p /data/seed
exec > >(tee "$BOOT_TRACE") 2>&1

send_boot_trace() {
    local exit_code="${1:-0}"
    echo ""
    echo "=== BOOT TRACE END (exit=$exit_code, $(date -u +%Y-%m-%dT%H:%M:%SZ)) ==="
    # Small delay to let tee flush its buffer to the file
    sleep 0.5
    if [ -e /dev/vsock ] && [ -f "$BOOT_TRACE" ]; then
        # Port 9007: host-side boot trace receiver
        socat -T10 -u FILE:"$BOOT_TRACE" VSOCK-CONNECT:3:9007 2>/dev/null || true
    fi
}
trap 'send_boot_trace $?' EXIT

echo "=== Lightfriend Enclave Entrypoint ($(date -u +%Y-%m-%dT%H:%M:%SZ)) ==="
echo "VSOCK device: $([ -e /dev/vsock ] && echo 'present' || echo 'MISSING')"
echo "Kernel: $(uname -r)"
echo "Memory: $(free -m 2>/dev/null | awk '/^Mem:/{print $2"MB"}' || echo 'unknown')"

# ── Bring up loopback interface ──────────────────────────────────────────────
# Nitro enclaves have no network interfaces up by default. Without lo,
# 127.0.0.1 is unreachable and all localhost services (socat, postgres,
# tuwunel, bridges, backend) fail.
if ip link set lo up 2>/dev/null; then
    echo "Loopback interface: up"
else
    echo "WARNING: failed to bring up loopback (ip link set lo up)"
fi

# Set up DNS resolver - Tuwunel needs /etc/resolv.conf
# Docker leaves a dangling symlink; remove it and create a real file.
rm -f /etc/resolv.conf 2>/dev/null || true
echo "nameserver 127.0.0.1" > /etc/resolv.conf 2>/dev/null && echo "/etc/resolv.conf: created" || echo "WARNING: could not create /etc/resolv.conf"


# ── 0a. Fetch environment from host via VSOCK ────────────────────────────────
ENV_LOADED=false
if [ -e /dev/vsock ]; then
    echo ""
    echo "[STEP 0a] Fetching environment from host via VSOCK port 9000..."
    for attempt in $(seq 1 10); do
        echo "  attempt $attempt: connecting to VSOCK-CONNECT:3:9000..."
        SOCAT_ERR=$(mktemp)
        if socat -T5 - VSOCK-CONNECT:3:9000 > /tmp/host_env 2>"$SOCAT_ERR"; then
            SOCAT_RC=0
        else
            SOCAT_RC=$?
        fi
        ENV_SIZE=$(stat -c%s /tmp/host_env 2>/dev/null || echo "0")
        SOCAT_ERR_MSG=$(cat "$SOCAT_ERR" 2>/dev/null || true)
        rm -f "$SOCAT_ERR"
        echo "  attempt $attempt: socat exit=$SOCAT_RC, received=${ENV_SIZE} bytes"
        [ -n "$SOCAT_ERR_MSG" ] && echo "  attempt $attempt: socat stderr: $SOCAT_ERR_MSG"

        if [ "$SOCAT_RC" -eq 0 ] && [ "$ENV_SIZE" -gt 0 ]; then
            # Show key names (not values) for debugging
            echo "  env keys received: $(grep -c '=' /tmp/host_env) variables"
            echo "  env key names: $(grep '=' /tmp/host_env | cut -d= -f1 | tr '\n' ' ')"
            # Load env safely: source is unsafe because values may contain
            # special chars ($, ^, newlines). Use export per-line instead.
            ENV_COUNT=0
            while IFS= read -r line || [[ -n "$line" ]]; do
                [[ -z "$line" || "$line" == \#* ]] && continue
                if [[ "$line" =~ ^[A-Za-z_][A-Za-z_0-9]*= ]]; then
                    export "${line?}"
                    ENV_COUNT=$((ENV_COUNT + 1))
                fi
            done < /tmp/host_env
            echo "  Exported $ENV_COUNT variables"
            echo "  Environment loaded from host (attempt $attempt)"
            # Never persist BACKUP_ENCRYPTION_KEY to disk. The trustless backup
            # key must be derived again on each boot and only exist in memory.
            if [ -n "${BACKUP_ENCRYPTION_KEY:-}" ] && [ "${ALLOW_INSECURE_BACKUP_KEY_FALLBACK:-false}" = "true" ]; then
                export INSECURE_BACKUP_ENCRYPTION_KEY_FALLBACK="${BACKUP_ENCRYPTION_KEY}"
            fi
            unset BACKUP_ENCRYPTION_KEY

            # Persist non-secret env for supervisord-managed processes.
            mkdir -p /etc/lightfriend
            grep -v '^BACKUP_ENCRYPTION_KEY=' /tmp/host_env > /etc/lightfriend/env || true
            chmod 600 /etc/lightfriend/env
            rm -f /tmp/host_env
            ENV_LOADED=true
            break
        fi
        echo "  Waiting for host config server (attempt $attempt/10)..."
        sleep 2
    done

    if [ "$ENV_LOADED" = "false" ]; then
        echo "FATAL: Failed to load environment from host after 10 attempts"
        echo "  Last socat exit=$SOCAT_RC, last received bytes=$ENV_SIZE"
        echo "  Check: is vsock-config-server running on host? Does /opt/lightfriend/host-env exist?"
        exit 1
    fi
fi

# ── 0b. Set internal defaults ────────────────────────────────────────────────
export PG_DATABASE_URL="${PG_DATABASE_URL:-postgres://lightfriend:lightfriend@localhost:5432/lightfriend_db?sslmode=disable}"
export MATRIX_HOMESERVER="${MATRIX_HOMESERVER:-http://localhost:8008}"
export MATRIX_HOMESERVER_PERSISTENT_STORE_PATH="${MATRIX_HOMESERVER_PERSISTENT_STORE_PATH:-/app/matrix_store}"
export WHATSAPP_BRIDGE_BOT="${WHATSAPP_BRIDGE_BOT:-@whatsappbot:localhost}"
export SIGNAL_BRIDGE_BOT="${SIGNAL_BRIDGE_BOT:-@signalbot:localhost}"
export TELEGRAM_BRIDGE_BOT="${TELEGRAM_BRIDGE_BOT:-@telegrambot:localhost}"
export PORT="${PORT:-3100}"
export SKIP_BACKEND="${SKIP_BACKEND:-false}"
export RESTORE_MODE="${RESTORE_MODE:-none}"

# ── 0c. Start VSOCK outbound proxy bridge ────────────────────────────────────
echo ""
echo "[STEP 0c] Starting VSOCK outbound proxy bridge..."

if [ -e /dev/vsock ]; then
    socat TCP-LISTEN:3128,reuseaddr,fork VSOCK-CONNECT:3:8001 &
    SOCAT_PID=$!
    sleep 0.5

    if ! kill -0 $SOCAT_PID 2>/dev/null; then
        echo "  ERROR: VSOCK bridge failed to start (PID $SOCAT_PID died)"
        exit 1
    fi
    echo "  VSOCK bridge running (PID $SOCAT_PID, localhost:3128 -> host:8001)"

    export HTTP_PROXY="http://127.0.0.1:3128"
    export HTTPS_PROXY="http://127.0.0.1:3128"
    export http_proxy="http://127.0.0.1:3128"
    export https_proxy="http://127.0.0.1:3128"
    export NO_PROXY="localhost,127.0.0.1,::1"
    export no_proxy="localhost,127.0.0.1,::1"

    # Quick connectivity test through the proxy
    echo "  Testing outbound connectivity via proxy..."
    if curl -sf --max-time 10 -x http://127.0.0.1:3128 https://api.cloudflare.com/cdn-cgi/trace > /tmp/proxy-test 2>&1; then
        echo "  Outbound proxy OK: $(head -1 /tmp/proxy-test)"
    else
        echo "  WARNING: outbound proxy test failed (curl exit=$?)"
        echo "  This may cause cloudflared/API calls to fail later"
        cat /tmp/proxy-test 2>/dev/null || true
    fi
    rm -f /tmp/proxy-test
else
    echo "  No VSOCK device - running in direct network mode"
fi

# In local Docker mode there is no host-env bootstrap step, so preserve the
# dev-only fallback key in memory before the runtime derivation step.
if [ -n "${BACKUP_ENCRYPTION_KEY:-}" ] && [ "${ALLOW_INSECURE_BACKUP_KEY_FALLBACK:-false}" = "true" ] && [ -z "${INSECURE_BACKUP_ENCRYPTION_KEY_FALLBACK:-}" ]; then
    export INSECURE_BACKUP_ENCRYPTION_KEY_FALLBACK="${BACKUP_ENCRYPTION_KEY}"
fi

# ── 0d. Derive backup encryption key (runtime only, never persisted) ────────
echo ""
echo "[STEP 0d] Deriving runtime backup encryption key..."
echo "  NSM device: $([ -e /dev/nsm ] && echo 'present' || echo 'MISSING')"
echo "  MARLIN_KMS_CONTRACT_ADDRESS: ${MARLIN_KMS_CONTRACT_ADDRESS:-(not set)}"
echo "  MARLIN_ROOT_SERVER_ENDPOINT: ${MARLIN_ROOT_SERVER_ENDPOINT:-(not set)}"
echo "  ALLOW_INSECURE_BACKUP_KEY_FALLBACK: ${ALLOW_INSECURE_BACKUP_KEY_FALLBACK:-(not set)}"
KMS_STDERR="/tmp/derive-backup-key.stderr"
if ! BACKUP_ENCRYPTION_KEY="$(/usr/local/bin/derive-backup-key.sh 2>"$KMS_STDERR")"; then
    echo "  FATAL: derive-backup-key.sh failed"
    echo "  stderr:"
    cat "$KMS_STDERR" 2>/dev/null
    # Dump Marlin KMS sidecar logs if they exist
    for logf in /tmp/marlin-kms/*.log; do
        [ -f "$logf" ] && echo "  --- $(basename "$logf") ---" && tail -20 "$logf"
    done
    rm -f "$KMS_STDERR"
    exit 1
fi
# Log KMS startup info (was previously polluting the key via 2>&1)
cat "$KMS_STDERR" 2>/dev/null
rm -f "$KMS_STDERR"
if [ -z "${BACKUP_ENCRYPTION_KEY}" ]; then
    echo "  FATAL: Derived BACKUP_ENCRYPTION_KEY is empty"
    exit 1
fi
export BACKUP_ENCRYPTION_KEY
unset INSECURE_BACKUP_ENCRYPTION_KEY_FALLBACK

BACKUP_KEY_FINGERPRINT="$(printf '%s' "${BACKUP_ENCRYPTION_KEY}" | sha256sum | awk '{print $1}' | cut -c1-16)"
echo "  Backup key ready (fingerprint: ${BACKUP_KEY_FINGERPRINT})"

# ── 0e. Fetch backup from host (explicit restore only) ──────────────────────
if [ "${RESTORE_MODE}" != "none" ]; then
    echo "Restore requested (mode: ${RESTORE_MODE}) - fetching backup from host..."
    mkdir -p /data/seed
    RECEIVED="/data/seed/backup-received.tar.gz.enc"

    # Start temporary VSOCK bridge for HTTP seed server (port 9080)
    # Supervisord's vsock-bridge-9080 isn't running yet at this point
    if [ -e /dev/vsock ]; then
        socat TCP-LISTEN:9080,reuseaddr,fork VSOCK-CONNECT:3:9080 &
        RESTORE_BRIDGE_PID=$!
        echo "  Started temporary VSOCK bridge (PID ${RESTORE_BRIDGE_PID})"
        sleep 1
    fi

    # Download backup via HTTP seed server (port 9080 VSOCK bridge)
    BACKUP_URL="http://127.0.0.1:9080/restore-backup.tar.gz.enc"
    echo "  Downloading backup via HTTP (port 9080)..."
    HTTP_OK=false
    for attempt in $(seq 1 10); do
        if curl -sf --max-time 300 -o "$RECEIVED" "$BACKUP_URL" 2>/dev/null && [ -s "$RECEIVED" ]; then
            HTTP_OK=true
            break
        fi
        echo "  HTTP attempt ${attempt}/10 - waiting for seed server..."
        sleep 3
    done

    if [ -s "$RECEIVED" ] && ! grep -q "NO_BACKUP" "$RECEIVED" 2>/dev/null; then
        RECEIVED_SIZE=$(stat -c%s "$RECEIVED" 2>/dev/null || stat -f%z "$RECEIVED" 2>/dev/null || echo "unknown")
        echo "Backup received from host (${RECEIVED_SIZE} bytes)"
        case "${RESTORE_MODE}" in
            pg_only)
                mv "$RECEIVED" "/data/seed/lightfriend-pg-backup-received.tar.gz.enc"
                ;;
            *)
                mv "$RECEIVED" "/data/seed/lightfriend-full-backup-received.tar.gz.enc"
                ;;
        esac
    else
        rm -f "$RECEIVED"
        echo "FATAL: Restore requested but no backup available from host"
        exit 1
    fi
else
    echo "No restore requested - starting with current state"
fi

# Kill temporary restore bridge (step 0f starts its own)
if [ -n "${RESTORE_BRIDGE_PID:-}" ]; then kill "$RESTORE_BRIDGE_PID" 2>/dev/null && wait "$RESTORE_BRIDGE_PID" 2>/dev/null; fi || true

# ── 0f. Fetch one-time SQL seed from host (first bootstrap only) ─────────────
# Host runs python3 http.server on port 9080 serving /opt/lightfriend/seed/.
# We bridge to it via VSOCK and fetch with curl (HTTP framing handles large files
# reliably, unlike raw VSOCK dumps which drop 17MB payloads).
# Temporary bridge for seed fetch. Killed before exec supervisord so
# the vsock-bridge-9080 supervisord program can take over.
if [ -e /dev/vsock ]; then
    socat TCP-LISTEN:9080,reuseaddr,fork VSOCK-CONNECT:3:9080 &
    SEED_BRIDGE_PID=$!
    sleep 0.3
fi

echo ""
echo "[STEP 0f] Checking for SQL seed from host..."
if [ "${RESTORE_MODE}" = "none" ]; then
    mkdir -p /data/seed
    SEED_DUMP="/data/seed/lightfriend_db.sql"
    if [ -f "$SEED_DUMP" ]; then
        EXISTING_SIZE=$(stat -c%s "$SEED_DUMP" 2>/dev/null || echo "unknown")
        echo "  Seed dump already exists (${EXISTING_SIZE} bytes) - skipping fetch"
    else
        echo "  No existing seed dump. Fetching via HTTP..."

        RECEIVED_SEED="/data/seed/lightfriend_db.seed.tmp"
        SEED_FETCHED=false
        for seed_attempt in $(seq 1 5); do
            echo "  seed attempt $seed_attempt: curl http://127.0.0.1:9080/lightfriend_db.sql..."
            CURL_OUT=$(curl -s --max-time 120 \
                -o "$RECEIVED_SEED" -w '%{http_code}' \
                http://127.0.0.1:9080/lightfriend_db.sql 2>&1)
            CURL_RC=$?
            SEED_BYTES=$(stat -c%s "$RECEIVED_SEED" 2>/dev/null || echo "0")
            echo "  seed attempt $seed_attempt: curl exit=$CURL_RC, http=$CURL_OUT, received ${SEED_BYTES} bytes"

            if [ "$CURL_RC" -eq 0 ] && [ "$SEED_BYTES" -gt 100 ]; then
                mv "$RECEIVED_SEED" "$SEED_DUMP"
                echo "  SQL seed received from host (${SEED_BYTES} bytes)"
                SEED_FETCHED=true
                break
            elif echo "$CURL_OUT" | grep -q "404"; then
                echo "  seed attempt $seed_attempt: no seed file on host (HTTP 404)"
                rm -f "$RECEIVED_SEED"
                break
            fi
            echo "  seed attempt $seed_attempt: failed, retrying in 3s..."
            rm -f "$RECEIVED_SEED"
            sleep 3
        done

        # Keep bridge alive - export-watcher also uses port 9080 to poll for export requests

        if [ "$SEED_FETCHED" = "false" ]; then
            echo "  No SQL seed available from host"
            echo "  Check: is seed-http-server running? Is /opt/lightfriend/seed/lightfriend_db.sql staged?"
        fi
    fi
else
    echo "  Restore mode=${RESTORE_MODE} - seed fetch skipped"
fi

# ── 1. Initialize PostgreSQL if needed ──────────────────────────────────────
echo ""
echo "[STEP 1] PostgreSQL initialization..."

PG_DATA="/var/lib/postgresql/data"
PG_BIN="/usr/lib/postgresql/15/bin"

if [ ! -f "$PG_DATA/PG_VERSION" ]; then
    echo "  No PG_VERSION found - initializing fresh PostgreSQL data directory..."
    mkdir -p "$PG_DATA"
    chown -R postgres:postgres "$PG_DATA"
    su postgres -c "$PG_BIN/initdb -D $PG_DATA"

    # Allow local connections without password (all in same container)
    echo "host all all 127.0.0.1/32 trust" >> "$PG_DATA/pg_hba.conf"
    echo "local all all trust" >> "$PG_DATA/pg_hba.conf"

    # Listen on localhost only
    sed -i "s/#listen_addresses = 'localhost'/listen_addresses = 'localhost'/" "$PG_DATA/postgresql.conf"
fi

mkdir -p /run/postgresql /var/log/supervisor
chown -R postgres:postgres "$PG_DATA" /run/postgresql /var/log/supervisor

# ── 2. Start PostgreSQL temporarily to create databases ─────────────────────
echo ""
echo "[STEP 2] Starting PostgreSQL for initialization..."
su postgres -c "$PG_BIN/pg_ctl -D $PG_DATA -l /var/log/supervisor/postgresql-init.log start -w"

# Create bridge databases if they don't exist
for db_info in "lightfriend lightfriend lightfriend_db" \
               "whatsapp_user whatsapp_password whatsapp_db" \
               "signal_user signal_password signal_db" \
               "telegram_user telegram_password telegram_db"; do
    db_user=$(echo "$db_info" | awk '{print $1}')
    db_pass=$(echo "$db_info" | awk '{print $2}')
    db_name=$(echo "$db_info" | awk '{print $3}')

    su postgres -c "psql -tc \"SELECT 1 FROM pg_roles WHERE rolname='$db_user'\" | grep -q 1" || \
        su postgres -c "psql -c \"CREATE USER $db_user WITH PASSWORD '$db_pass'\""

    su postgres -c "psql -tc \"SELECT 1 FROM pg_database WHERE datname='$db_name'\" | grep -q 1" || \
        su postgres -c "psql -c \"CREATE DATABASE $db_name OWNER $db_user\""
done

echo "Bridge databases ready."

# ── 2a. Full encrypted backup restore ────────────────────────────────────
FULL_RESTORE_DONE=false
# shellcheck disable=SC2012
FULL_BACKUP=$(ls /data/seed/lightfriend-full-backup-*.tar.gz.enc 2>/dev/null | head -1 || true)

if [ -n "${FULL_BACKUP}" ]; then
    echo "=== Full encrypted backup detected: $(basename "${FULL_BACKUP}") ==="
    RESTORE_STATUS="/data/seed/restore-status.json"
    RESTORE_TIMESTAMP=$(date -u +%Y%m%dT%H%M%SZ)

    write_restore_status() {
        local status="$1"
        shift
        if [ "$status" = "SUCCESS" ]; then
            cat > "${RESTORE_STATUS}" <<REOF
{"status": "SUCCESS", "timestamp": "${RESTORE_TIMESTAMP}", "components_restored": [$1]}
REOF
        else
            local error="$1"
            local step="$2"
            cat > "${RESTORE_STATUS}" <<REOF
{"status": "FAILED", "timestamp": "${RESTORE_TIMESTAMP}", "error": "${error}", "step": "${step}"}
REOF
        fi
    }

    restore_abort() {
        local msg="$1"
        local step="$2"
        echo "RESTORE FAILED: ${msg} (step: ${step})"
        write_restore_status "FAILED" "${msg}" "${step}"
        # Write failure to verify-result.json (post-boot-verify will upload it via HTTP)
        echo "{\"status\": \"RESTORE_FAILED\", \"restore_type\": \"full_restore\", \"error\": \"${msg}\", \"step\": \"${step}\", \"user_count\": 0}" > /data/seed/verify-result.json
        echo "Old enclave is still running. Fix the issue and retry."
        su postgres -c "$PG_BIN/pg_ctl -D $PG_DATA stop -w" 2>/dev/null || true
        exit 1
    }

    # Require encryption key
    if [ -z "${BACKUP_ENCRYPTION_KEY:-}" ]; then
        restore_abort "BACKUP_ENCRYPTION_KEY not set but encrypted backup is present" "decrypt"
    fi

    # Decrypt
    echo "Decrypting backup..."
    DECRYPT_DIR="/tmp/backup-restore"
    mkdir -p "${DECRYPT_DIR}"
    openssl enc -d -aes-256-cbc -pbkdf2 -iter 600000 \
        -pass env:BACKUP_ENCRYPTION_KEY \
        -in "${FULL_BACKUP}" \
        -out "${DECRYPT_DIR}/backup.tar.gz" 2>/dev/null \
        || restore_abort "Decryption failed - wrong key or corrupt file" "decrypt"

    # Extract
    echo "Extracting backup..."
    cd "${DECRYPT_DIR}"
    tar xzf backup.tar.gz \
        || restore_abort "Archive extraction failed - corrupt tar.gz" "extract"

    # Find the backup directory (lightfriend-full-backup-*)
    # shellcheck disable=SC2012
    BACKUP_DIR=$(ls -d lightfriend-full-backup-* 2>/dev/null | head -1)
    if [ -z "${BACKUP_DIR}" ]; then
        restore_abort "No lightfriend-full-backup-* directory found in archive" "extract"
    fi

    # Verify checksums
    echo "Verifying checksums..."
    cd "${DECRYPT_DIR}/${BACKUP_DIR}"
    if [ ! -f checksums.sha256 ]; then
        restore_abort "checksums.sha256 missing from backup" "verify-checksums"
    fi
    # Verify all files except checksums.sha256 itself
    grep -v "checksums.sha256" checksums.sha256 | sha256sum -c --quiet 2>/dev/null \
        || restore_abort "Checksum verification failed - backup corrupted during transfer" "verify-checksums"
    echo "Checksums verified."

    # Save manifest for post-restore verification
    if [ -f manifest.json ]; then
        cp manifest.json /tmp/backup-manifest.json
    fi

    # Save expected bridge registration hashes from the backup so verification
    # can confirm startup didn't regenerate or overwrite bridge tokens.
    BRIDGE_HASHES_FILE="/tmp/bridge-registration-hashes"
    : > "${BRIDGE_HASHES_FILE}"
    for bridge in whatsapp signal telegram; do
        backup_reg="bridges/${bridge}/${bridge}-registration.yaml"
        if [ -f "${backup_reg}" ]; then
            reg_hash=$(sha256sum "${backup_reg}" | awk '{print $1}')
            echo "${bridge}:${reg_hash}" >> "${BRIDGE_HASHES_FILE}"
        fi
    done

    RESTORED_COMPONENTS=""

    # Restore all 4 PostgreSQL databases
    echo "Restoring PostgreSQL databases..."
    for db in lightfriend_db whatsapp_db signal_db telegram_db; do
        dump_file="postgres/${db}.sql"
        if [ ! -f "${dump_file}" ]; then
            restore_abort "Missing dump: ${dump_file}" "restore-${db}"
        fi
        echo "  Restoring ${db}..."
        # Drop and recreate the database
        su postgres -c "psql -c \"DROP DATABASE IF EXISTS ${db}\"" 2>/dev/null || true
        su postgres -c "psql -c \"CREATE DATABASE ${db}\"" 2>/dev/null \
            || restore_abort "Failed to create database ${db}" "restore-${db}"
        su postgres -c "psql --set ON_ERROR_STOP=on -d ${db} < ${dump_file}" \
            || restore_abort "psql restore of ${db} failed" "restore-${db}"
        # Grant ownership
        for db_info in "lightfriend lightfriend_db" "whatsapp_user whatsapp_db" "signal_user signal_db" "telegram_user telegram_db"; do
            owner=$(echo "$db_info" | awk '{print $1}')
            owned_db=$(echo "$db_info" | awk '{print $2}')
            if [ "${owned_db}" = "${db}" ]; then
                su postgres -c "psql -c \"ALTER DATABASE ${db} OWNER TO ${owner}\"" 2>/dev/null || true
                su postgres -c "psql -d ${db} -c \"GRANT ALL ON ALL TABLES IN SCHEMA public TO ${owner}\"" 2>/dev/null || true
                su postgres -c "psql -d ${db} -c \"GRANT ALL ON ALL SEQUENCES IN SCHEMA public TO ${owner}\"" 2>/dev/null || true
                su postgres -c "psql -d ${db} -c \"DO \\\$\\\$ DECLARE r RECORD; BEGIN FOR r IN SELECT tablename FROM pg_tables WHERE schemaname='public' LOOP EXECUTE 'ALTER TABLE public.' || quote_ident(r.tablename) || ' OWNER TO ${owner}'; END LOOP; END \\\$\\\$\"" 2>/dev/null || true
            fi
        done
        echo "  ${db} restored."
        RESTORED_COMPONENTS="${RESTORED_COMPONENTS}\"${db}\", "
    done

    # Restore filesystem stores
    echo "Restoring filesystem stores..."

    # Tuwunel (RocksDB)
    if [ -f tuwunel/tuwunel_data.tar ]; then
        echo "  Restoring /var/lib/tuwunel..."
        rm -rf /var/lib/tuwunel/*
        tar xf tuwunel/tuwunel_data.tar -C / \
            || restore_abort "Failed to restore tuwunel data" "restore-tuwunel"
        RESTORED_COMPONENTS="${RESTORED_COMPONENTS}\"tuwunel\", "
    else
        restore_abort "Missing tuwunel/tuwunel_data.tar" "restore-tuwunel"
    fi

    # Matrix store (per-user SQLite)
    if [ -f matrix_store/matrix_store.tar ]; then
        echo "  Restoring /app/matrix_store..."
        rm -rf /app/matrix_store/*
        tar xf matrix_store/matrix_store.tar -C /app \
            || restore_abort "Failed to restore matrix_store" "restore-matrix_store"
        RESTORED_COMPONENTS="${RESTORED_COMPONENTS}\"matrix_store\", "
    else
        restore_abort "Missing matrix_store/matrix_store.tar" "restore-matrix_store"
    fi

    # Bridges (registrations + device state)
    if [ -f bridges/bridge_data.tar ]; then
        echo "  Restoring /data/bridges..."
        rm -rf /data/bridges/*
        tar xf bridges/bridge_data.tar -C /data \
            || restore_abort "Failed to restore bridge data" "restore-bridges"
        RESTORED_COMPONENTS="${RESTORED_COMPONENTS}\"bridges\", "
    else
        restore_abort "Missing bridges/bridge_data.tar" "restore-bridges"
    fi

    # Uploads (user media)
    if [ -f uploads/uploads.tar ]; then
        echo "  Restoring /app/uploads..."
        rm -rf /app/uploads/*
        tar xf uploads/uploads.tar -C /app \
            || restore_abort "Failed to restore uploads" "restore-uploads"
        RESTORED_COMPONENTS="${RESTORED_COMPONENTS}\"uploads\", "
    else
        restore_abort "Missing uploads/uploads.tar" "restore-uploads"
    fi

    # Core data (app state)
    if [ -f core_data/core_data.tar ]; then
        echo "  Restoring /app/data..."
        rm -rf /app/data/*
        tar xf core_data/core_data.tar -C /app \
            || restore_abort "Failed to restore core data" "restore-core_data"
        RESTORED_COMPONENTS="${RESTORED_COMPONENTS}\"core_data\", "
    else
        restore_abort "Missing core_data/core_data.tar" "restore-core_data"
    fi

    # Trim trailing comma-space
    RESTORED_COMPONENTS="${RESTORED_COMPONENTS%, }"

    # Cleanup decrypted data from /tmp
    rm -rf "${DECRYPT_DIR}"

    # Write success status
    write_restore_status "SUCCESS" "${RESTORED_COMPONENTS}"

    # Remove backup from seed to prevent re-import on restart
    rm -f "${FULL_BACKUP}"

    FULL_RESTORE_DONE=true
    echo "=== Full restore complete ==="
fi

# ── 2a-pg. PG-only backup restore (disaster recovery) ────────────────────
# If no full backup exists but a PG-only daily backup does, restore databases
# only. Bridge/Matrix state starts fresh (users re-link).
if [ "${FULL_RESTORE_DONE}" = "false" ]; then
# shellcheck disable=SC2012
PG_BACKUP=$(ls /data/seed/lightfriend-pg-backup-*.tar.gz.enc 2>/dev/null | head -1 || true)

if [ -n "${PG_BACKUP}" ]; then
    echo "=== PG-only backup detected: $(basename "${PG_BACKUP}") ==="
    echo "  Databases will be restored. Bridge/Matrix state starts fresh."
    RESTORE_STATUS="/data/seed/restore-status.json"
    RESTORE_TIMESTAMP=$(date -u +%Y%m%dT%H%M%SZ)

    pg_restore_abort() {
        local msg="$1"
        local step="$2"
        echo "PG RESTORE FAILED: ${msg} (step: ${step})"
        echo "{\"status\": \"FAILED\", \"timestamp\": \"${RESTORE_TIMESTAMP}\", \"error\": \"${msg}\", \"step\": \"${step}\"}" > "${RESTORE_STATUS}"
        if [ -e /dev/vsock ]; then
            echo "{\"status\": \"RESTORE_FAILED\", \"restore_type\": \"pg_only\", \"error\": \"${msg}\", \"step\": \"${step}\", \"user_count\": 0}" > /data/seed/verify-result.json
            socat -u FILE:/data/seed/verify-result.json VSOCK-CONNECT:3:9004 2>/dev/null || true
        fi
        su postgres -c "$PG_BIN/pg_ctl -D $PG_DATA stop -w" 2>/dev/null || true
        exit 1
    }

    if [ -z "${BACKUP_ENCRYPTION_KEY:-}" ]; then
        pg_restore_abort "BACKUP_ENCRYPTION_KEY not set" "decrypt"
    fi

    # Decrypt
    echo "Decrypting PG backup..."
    DECRYPT_DIR="/tmp/pg-restore"
    mkdir -p "${DECRYPT_DIR}"
    openssl enc -d -aes-256-cbc -pbkdf2 -iter 600000 \
        -pass env:BACKUP_ENCRYPTION_KEY \
        -in "${PG_BACKUP}" \
        -out "${DECRYPT_DIR}/backup.tar.gz" 2>/dev/null \
        || pg_restore_abort "Decryption failed" "decrypt"

    # Extract
    echo "Extracting..."
    cd "${DECRYPT_DIR}"
    tar xzf backup.tar.gz \
        || pg_restore_abort "Archive extraction failed" "extract"

    # shellcheck disable=SC2012
    BACKUP_DIR=$(ls -d lightfriend-pg-backup-* 2>/dev/null | head -1)
    if [ -z "${BACKUP_DIR}" ]; then
        pg_restore_abort "No lightfriend-pg-backup-* directory in archive" "extract"
    fi

    # Verify checksums
    cd "${DECRYPT_DIR}/${BACKUP_DIR}"
    if [ ! -f checksums.sha256 ]; then
        pg_restore_abort "checksums.sha256 missing" "verify-checksums"
    fi
    grep -v "checksums.sha256" checksums.sha256 | sha256sum -c --quiet 2>/dev/null \
        || pg_restore_abort "Checksum verification failed" "verify-checksums"
    echo "Checksums verified."

    # Save manifest for post-restore verification
    if [ -f manifest.json ]; then
        cp manifest.json /tmp/backup-manifest.json
    fi

    # PG-only restores intentionally do not preserve bridge state. Remove any
    # stale hash file so verification doesn't expect full-restore semantics.
    rm -f /tmp/bridge-registration-hashes

    # Restore all 4 PostgreSQL databases
    echo "Restoring PostgreSQL databases..."
    RESTORED_COMPONENTS=""
    for db in lightfriend_db whatsapp_db signal_db telegram_db; do
        dump_file="postgres/${db}.sql"
        if [ ! -f "${dump_file}" ]; then
            pg_restore_abort "Missing dump: ${dump_file}" "restore-${db}"
        fi
        echo "  Restoring ${db}..."
        su postgres -c "psql -c \"DROP DATABASE IF EXISTS ${db}\"" 2>/dev/null || true
        su postgres -c "psql -c \"CREATE DATABASE ${db}\"" 2>/dev/null \
            || pg_restore_abort "Failed to create database ${db}" "restore-${db}"
        su postgres -c "psql --set ON_ERROR_STOP=on -d ${db} < ${dump_file}" \
            || pg_restore_abort "psql restore of ${db} failed" "restore-${db}"
        for db_info in "lightfriend lightfriend_db" "whatsapp_user whatsapp_db" "signal_user signal_db" "telegram_user telegram_db"; do
            owner=$(echo "$db_info" | awk '{print $1}')
            owned_db=$(echo "$db_info" | awk '{print $2}')
            if [ "${owned_db}" = "${db}" ]; then
                su postgres -c "psql -c \"ALTER DATABASE ${db} OWNER TO ${owner}\"" 2>/dev/null || true
                su postgres -c "psql -d ${db} -c \"GRANT ALL ON ALL TABLES IN SCHEMA public TO ${owner}\"" 2>/dev/null || true
                su postgres -c "psql -d ${db} -c \"GRANT ALL ON ALL SEQUENCES IN SCHEMA public TO ${owner}\"" 2>/dev/null || true
                su postgres -c "psql -d ${db} -c \"DO \\\$\\\$ DECLARE r RECORD; BEGIN FOR r IN SELECT tablename FROM pg_tables WHERE schemaname='public' LOOP EXECUTE 'ALTER TABLE public.' || quote_ident(r.tablename) || ' OWNER TO ${owner}'; END LOOP; END \\\$\\\$\"" 2>/dev/null || true
            fi
        done
        echo "  ${db} restored."
        RESTORED_COMPONENTS="${RESTORED_COMPONENTS}\"${db}\", "
    done

    # Clean stale connection data (no bridge/Matrix state to match)
    echo "Cleaning stale connection data (PG-only restore, bridges start fresh)..."
    su postgres -c "psql -d lightfriend_db" <<'CLEANUP'
        TRUNCATE bridges;
        TRUNCATE bridge_disconnection_events;
        TRUNCATE contact_profiles CASCADE;
        UPDATE user_secrets
        SET matrix_username = NULL,
            matrix_device_id = NULL,
            encrypted_matrix_access_token = NULL,
            encrypted_matrix_password = NULL,
            encrypted_matrix_secret_storage_recovery_key = NULL;
        UPDATE users SET matrix_e2ee_enabled = false;
CLEANUP
    echo "Connection data cleaned."

    RESTORED_COMPONENTS="${RESTORED_COMPONENTS%, }"
    rm -rf "${DECRYPT_DIR}"
    echo "{\"status\": \"SUCCESS\", \"timestamp\": \"${RESTORE_TIMESTAMP}\", \"type\": \"pg_only\", \"components_restored\": [${RESTORED_COMPONENTS}]}" > "${RESTORE_STATUS}"
    rm -f "${PG_BACKUP}"
    FULL_RESTORE_DONE=true
    echo "=== PG-only restore complete ==="
fi
fi

# ── 2b. Restore lightfriend_db from seed dump if present ─────────────────
# Skip when full restore was done (all data already restored above)
echo ""
echo "[STEP 2b] Seed dump restore check..."
if [ "${FULL_RESTORE_DONE}" = "false" ]; then
SEED_DUMP="/data/seed/lightfriend_db.sql"
if [ -f "$SEED_DUMP" ]; then
    DUMP_SIZE=$(stat -c%s "$SEED_DUMP" 2>/dev/null || echo "unknown")
    echo "  Restoring lightfriend_db from seed dump (${DUMP_SIZE} bytes)..."
    if su postgres -c "psql -d lightfriend_db < $SEED_DUMP" 2>&1; then
        echo "  psql restore succeeded"
    else
        echo "  WARNING: psql restore had errors (exit=$?)"
    fi

    # Clean stale Matrix/bridge connection data (homeserver + bridges start fresh)
    echo "Cleaning stale connection data..."
    su postgres -c "psql -d lightfriend_db" <<'CLEANUP'
        -- Bridge connection state: all stale, users must re-link
        TRUNCATE bridges;
        TRUNCATE bridge_disconnection_events;

        -- Contact profiles: drop entirely, users recreate manually
        TRUNCATE contact_profiles CASCADE;

        -- User Matrix data: wipe everything (old Synapse -> new Tuwunel, all new)
        UPDATE user_secrets
        SET matrix_username = NULL,
            matrix_device_id = NULL,
            encrypted_matrix_access_token = NULL,
            encrypted_matrix_password = NULL,
            encrypted_matrix_secret_storage_recovery_key = NULL;

        -- Reset E2EE flag
        UPDATE users SET matrix_e2ee_enabled = false;
CLEANUP
    echo "Connection data cleaned."

    echo "Seed dump restored. Removing to prevent re-import on restart."
    rm -f "$SEED_DUMP"
fi
fi

# ── 3. Generate configs from templates ─────────────────────────────────────
# Note: Database migrations run automatically at backend startup via embed_migrations!
echo ""
echo "[STEP 3] Generating configuration files from templates..."

substitute_vars() {
    local template_file="$1"
    local output_file="$2"
    local content
    content=$(cat "$template_file")

    # All variables that may appear in templates
    local vars=(
        "MATRIX_HOMESERVER_SHARED_SECRET"
        "MATRIX_REGISTRATION_TOKEN"
        "DOUBLE_PUPPET_SECRET"
        "TELEGRAM_API_ID"
        "TELEGRAM_API_HASH"
        "TELEGRAM_PROXY_TYPE"
        "TELEGRAM_PROXY_ADDRESS"
        "TELEGRAM_PROXY_PORT"
        "TELEGRAM_PROXY_USERNAME"
        "TELEGRAM_PROXY_PASSWORD"
        "TELEGRAM_PUBLIC_EXTERNAL_URL"
        "WHATSAPP_AS_TOKEN"
        "WHATSAPP_HS_TOKEN"
        "WHATSAPP_SENDER_LOCALPART"
        "SIGNAL_AS_TOKEN"
        "SIGNAL_HS_TOKEN"
        "SIGNAL_SENDER_LOCALPART"
        "TELEGRAM_AS_TOKEN"
        "TELEGRAM_HS_TOKEN"
        "TELEGRAM_SENDER_LOCALPART"
    )

    for var in "${vars[@]}"; do
        local value="${!var-}"
        value=$(printf '%s\n' "$value" | sed 's/[&/\]/\\&/g')
        # shellcheck disable=SC2001
        content=$(echo "$content" | sed "s/\${${var}}/${value}/g")
        # Also handle ${VAR:-default} patterns - strip the default syntax
        # shellcheck disable=SC2001
        content=$(echo "$content" | sed "s/\${${var}:-[^}]*}/${value}/g")
    done

    echo "$content" > "$output_file"
}

ensure_telegram_config_compat() {
    local config_file="/data/bridges/telegram/config.yaml"
    [ -f "$config_file" ] || return 0

    python3 - <<'PY'
from pathlib import Path
import os
import re

path = Path("/data/bridges/telegram/config.yaml")
text = path.read_text()
external = os.environ.get("TELEGRAM_PUBLIC_EXTERNAL_URL") or "http://localhost:3000/public"

public_block = (
    "    public:\n"
    "        enabled: true\n"
    "        prefix: /public\n"
    f"        external: {external}\n"
)

public_pattern = r"(?ms)^    public:\n(?:        .*?\n)+?(?=    [A-Za-z_][A-Za-z0-9_]*:|\Z)"
if re.search(public_pattern, text):
    text = re.sub(public_pattern, public_block, text, count=1)
else:
    db_opts_pattern = r"(?ms)^(    database_opts:\n(?:        .*?\n)+)"
    text, count = re.subn(db_opts_pattern, r"\1" + public_block, text, count=1)
    if count == 0:
        appservice_pattern = r"(?ms)^(appservice:\n(?:    .*?\n)+?)^(    id:)"
        text = re.sub(appservice_pattern, r"\1" + public_block + r"\2", text, count=1)

text = re.sub(
    r"(?m)^    sync_with_custom_puppets:\s+.*$",
    "    sync_with_custom_puppets: false",
    text,
    count=1,
)

shared_secret = os.environ.get("MATRIX_HOMESERVER_SHARED_SECRET", "")
if shared_secret:
    login_secret_block = (
        "    login_shared_secret_map:\n"
        f"        localhost: {shared_secret}\n"
    )
    login_secret_pattern = r"(?ms)^    login_shared_secret_map:\n(?:        .*?\n)+?(?=    [A-Za-z_][A-Za-z0-9_]*:|\Z)"
    if re.search(login_secret_pattern, text):
        text = re.sub(login_secret_pattern, login_secret_block, text, count=1)

path.write_text(text)
PY

    echo "  Ensured Telegram public login config in ${config_file}"
}

# Generate Tuwunel config
substitute_vars /etc/enclave-configs/tuwunel.toml.template /etc/tuwunel/tuwunel.toml
echo "  Generated /etc/tuwunel/tuwunel.toml"

# Generate bridge configs (only if not already present - mautrix -g merges the
# template into a full config on first run, and we must not overwrite that)
for bridge in whatsapp signal telegram; do
    config_file="/data/bridges/${bridge}/config.yaml"
    if [ ! -f "$config_file" ]; then
        substitute_vars "/etc/enclave-configs/${bridge}.yaml.template" "$config_file"
        echo "  Generated ${config_file}"
    else
        echo "  ${config_file} exists, skipping template generation"
    fi
done

ensure_telegram_config_compat

# Generate doublepuppet registration
substitute_vars /etc/enclave-configs/doublepuppet.yaml.template /data/bridges/doublepuppet.yaml
echo "  Generated /data/bridges/doublepuppet.yaml"

# ── 4. Generate bridge registration files if missing ───────────────────────

echo "Checking bridge registration files..."

generate_registration() {
    local bridge="$1"
    local executable="$2"
    local reg_file="/data/bridges/${bridge}/${bridge}-registration.yaml"
    local config_file="/data/bridges/${bridge}/config.yaml"

    if [ ! -f "$reg_file" ]; then
        echo "  Generating ${bridge} registration..."
        $executable -g -c "$config_file" -r "$reg_file" 2>/dev/null || true
    else
        echo "  ${bridge} registration exists."
    fi
}

generate_registration "whatsapp" "/usr/local/bin/mautrix-whatsapp"
generate_registration "signal" "/usr/local/bin/mautrix-signal"

# Telegram uses Python module
TELEGRAM_REG="/data/bridges/telegram/telegram-registration.yaml"
TELEGRAM_CFG="/data/bridges/telegram/config.yaml"
if [ ! -f "$TELEGRAM_REG" ]; then
    echo "  Generating telegram registration..."
    /opt/mautrix-telegram-venv/bin/python -m mautrix_telegram -g -c "$TELEGRAM_CFG" -r "$TELEGRAM_REG" 2>/dev/null || true
else
    echo "  telegram registration exists."
fi

# ── 5. Extract bridge tokens and regenerate Tuwunel config ─────────────────

echo "Extracting bridge tokens for Tuwunel..."

extract_yaml_value() {
    local file="$1"
    local key="$2"
    grep "^${key}:" "$file" 2>/dev/null | sed "s/^${key}: *//" | tr -d '"' | tr -d "'"
}

for bridge in whatsapp signal telegram; do
    reg_file="/data/bridges/${bridge}/${bridge}-registration.yaml"
    if [ -f "$reg_file" ]; then
        bridge_upper=$(echo "$bridge" | tr '[:lower:]' '[:upper:]')
        as_token=$(extract_yaml_value "$reg_file" "as_token")
        hs_token=$(extract_yaml_value "$reg_file" "hs_token")
        sender_localpart=$(extract_yaml_value "$reg_file" "sender_localpart")

        if [ -n "$as_token" ] && [ -n "$hs_token" ]; then
            export "${bridge_upper}_AS_TOKEN=${as_token}"
            export "${bridge_upper}_HS_TOKEN=${hs_token}"
            export "${bridge_upper}_SENDER_LOCALPART=${sender_localpart}"
            echo "  Extracted tokens for ${bridge}"
        else
            echo "  WARNING: Could not extract tokens from ${reg_file}"
        fi
    fi
done

# Inject tokens into bridge configs (mautrix -g doesn't patch configs automatically)
# Uses awk for reliable replacement - sed can mangle tokens with special characters
for bridge in whatsapp signal telegram; do
    config_file="/data/bridges/${bridge}/config.yaml"
    reg_file="/data/bridges/${bridge}/${bridge}-registration.yaml"
    if [ -f "$config_file" ] && [ -f "$reg_file" ]; then
        as_token=$(extract_yaml_value "$reg_file" "as_token")
        hs_token=$(extract_yaml_value "$reg_file" "hs_token")
        if [ -n "$as_token" ] && [ -n "$hs_token" ]; then
            # Replace any as_token/hs_token line under the appservice section
            # Matches the placeholder text OR empty strings from a previous failed injection
            awk -v tok="$as_token" '
                /^    as_token:/ && !/localhost:/ { print "    as_token: \"" tok "\""; next }
                { print }
            ' "$config_file" > "${config_file}.tmp" && mv "${config_file}.tmp" "$config_file"
            awk -v tok="$hs_token" '
                /^    hs_token:/ { print "    hs_token: \"" tok "\""; next }
                { print }
            ' "$config_file" > "${config_file}.tmp" && mv "${config_file}.tmp" "$config_file"
            echo "  Injected tokens into ${bridge} config"
        fi
    fi
done

# Regenerate Tuwunel config with actual bridge tokens
substitute_vars /etc/enclave-configs/tuwunel.toml.template /etc/tuwunel/tuwunel.toml
echo "  Regenerated Tuwunel config with bridge tokens."

# ── 6. Stop temporary PostgreSQL (supervisord will start it properly) ──────

echo "Stopping temporary PostgreSQL..."
su postgres -c "$PG_BIN/pg_ctl -D $PG_DATA stop -w"

# ── 7. Handle SKIP_BACKEND mode ───────────────────────────────────────────

if [ "${SKIP_BACKEND}" = "true" ]; then
    echo "SKIP_BACKEND=true: disabling Lightfriend backend in supervisord"
    sed -i '/\[program:lightfriend\]/,/^$/s/autostart=false/autostart=false/' /etc/supervisor/conf.d/lightfriend.conf
fi

# ── 8. Start supervisord ──────────────────────────────────────────────────

echo ""
echo "=== Starting all services via supervisord ==="

# ── Set up cloudflared edge routing (network config only) ────────────────────
# Cloudflared uses SRV DNS records to find edge IPs, then raw TCP to connect.
# It does NOT use HTTPS_PROXY. In the enclave with no direct internet, we need:
# 1. DNS-over-TLS (DoT) to 1.1.1.1:853 for SRV lookups (cloudflared's fallback)
# 2. TCP to edge IPs on port 7844 for the actual tunnel
# Both routed through VSOCK to the host.
#
# NOTE: The socat bridges for 7844 and 853 are now managed by supervisord
# (vsock-bridge-7844 and vsock-bridge-dot programs). They were previously started
# as background processes here, but those DIED when exec supervisord replaced the
# shell. This was the root cause of cloudflared "context canceled" errors.
if [ -e /dev/vsock ] && [ -n "${CLOUDFLARE_TUNNEL_TOKEN:-}" ]; then
    echo ""
    echo "[STEP CF] Setting up cloudflared edge routing..."

    # Add 1.1.1.1 as a local address so the DoT bridge (supervisord) can bind to it
    ip addr add 1.1.1.1/32 dev lo 2>/dev/null || true
    echo "  Added 1.1.1.1/32 to lo for DoT bridge"

    # Override DNS so cloudflared's resolved edge hostnames point to local bridges
    # These bridges are managed by supervisord (vsock-bridge-7844 and vsock-bridge-dot)
    echo "127.0.0.1 region1.v2.argotunnel.com region2.v2.argotunnel.com" >> /etc/hosts
    echo "  /etc/hosts: edge hostnames -> 127.0.0.1"
    echo "  Bridge chain: cloudflared -> TCP:7844 -> VSOCK -> host -> Cloudflare edge"
    echo "  DoT chain: cloudflared -> TCP:1.1.1.1:853 -> VSOCK -> host -> 1.1.1.1:853"
    echo "  NOTE: socat bridges now managed by supervisord (priority 2, start before cloudflared)"
fi

# Use a startup script that starts services in order
cat > /data/seed/start-services.sh << 'STARTUP'
#!/bin/bash
# Wait for PostgreSQL to be ready, then start dependent services
exec > /data/seed/startup-services.log 2>&1

# Debug beacon: curl to unique URLs through proxy. Shows up in squid log.
beacon() { curl -sf --max-time 5 -x http://127.0.0.1:3128 "http://httpbin.org/anything/enclave-$1" -o /dev/null 2>/dev/null & }

echo "=== Service startup $(date -u +%Y-%m-%dT%H:%M:%SZ) ==="
beacon "startup-begin"

sleep 2

echo "Waiting for PostgreSQL..."
for i in $(seq 1 30); do
    if pg_isready -h localhost -U postgres > /dev/null 2>&1; then
        echo "PostgreSQL is ready."
        break
    fi
    [ "$i" -eq 30 ] && echo "ERROR: PostgreSQL not ready" && exit 1
    sleep 1
done
beacon "pg-ready"

echo "Starting Tuwunel..."
supervisorctl start tuwunel
for i in $(seq 1 30); do
    if curl -sf http://localhost:8008/_matrix/client/versions > /dev/null 2>&1; then
        echo "Tuwunel is ready."
        break
    fi
    [ "$i" -eq 30 ] && echo "WARNING: Tuwunel not responding after 30s"
    sleep 1
done
beacon "tuwunel-ready"

echo "Registering bridge bots..."
cd /app && bash register-bridge-bots.sh http://localhost:8008 2>&1 || true

echo "Starting bridges..."
supervisorctl start mautrix-whatsapp
supervisorctl start mautrix-signal
supervisorctl start mautrix-telegram
beacon "bridges-started"

if [ "${SKIP_BACKEND}" != "true" ]; then
    sleep 2
    echo "Starting Lightfriend backend..."
    supervisorctl start lightfriend
    beacon "backend-started"
fi

if [ -n "${CLOUDFLARE_TUNNEL_TOKEN:-}" ] && [ "${DEFER_CLOUDFLARED:-}" != "true" ]; then
    echo "Starting Cloudflare tunnel..."
    echo "  Token length: ${#CLOUDFLARE_TUNNEL_TOKEN}"
    echo "  /etc/hosts edge entries:"
    grep argotunnel /etc/hosts 2>/dev/null || echo "    MISSING - cloudflared will try to connect to real IPs!"
    echo "  VSOCK bridge 7844 (supervisor):"
    supervisorctl status vsock-bridge-7844 2>&1 || echo "    NOT FOUND"
    echo "  Port 7844 listener:"
    ss -tlnp 2>/dev/null | grep 7844 || echo "    NOT LISTENING - bridge may not have started yet"
    echo "  VSOCK bridge DoT (supervisor):"
    supervisorctl status vsock-bridge-dot 2>&1 || echo "    NOT FOUND"
    echo "  Port 853 listener:"
    ss -tlnp 2>/dev/null | grep ':853' || echo "    NOT LISTENING"
    echo "  1.1.1.1 on lo:"
    ip addr show lo 2>/dev/null | grep '1.1.1.1' || echo "    NOT CONFIGURED"
    echo "  DNS test: $(getent hosts region1.v2.argotunnel.com 2>&1 | head -1)"
    supervisorctl start cloudflared
    beacon "cf-started"
    sleep 10
    echo "  Cloudflared status: $(supervisorctl status cloudflared 2>&1)"
    echo "  Cloudflared stderr (last 30 lines):"
    tail -30 /var/log/supervisor/cloudflared-err.log 2>/dev/null || echo "    empty"
    echo "  Cloudflared stdout (last 30 lines):"
    tail -30 /var/log/supervisor/cloudflared.log 2>/dev/null || echo "    empty"
    echo "  Cloudflared diag log:"
    cat /var/log/supervisor/cloudflared-diag.log 2>/dev/null || echo "    empty"
    echo "  Port 7844 connections after start:"
    ss -tnp 2>/dev/null | grep 7844 || echo "    none"
    echo "  Cloudflared monitor start:"
    supervisorctl start cloudflared-monitor 2>&1 || echo "    already running"
    beacon "cf-checked"
fi

echo "=== Service status ==="
supervisorctl status 2>&1
echo "=== All services started $(date -u +%Y-%m-%dT%H:%M:%SZ) ==="
beacon "all-done"
STARTUP
chmod +x /data/seed/start-services.sh

# If a full restore was done, create a post-startup verification wrapper
if [ "${FULL_RESTORE_DONE}" = "true" ]; then
    cat > /data/seed/start-and-verify.sh <<'VERIFYEOF'
#!/bin/bash
set -x  # Trace all commands for debugging
echo "=== start-and-verify.sh started at $(date -u) ==="

# Wait for backend to be healthy before running verify
echo "Waiting for backend health..."
BACKEND_PORT="${PORT:-3100}"
BACKEND_READY=false
for i in $(seq 1 60); do
    if curl -sf --max-time 3 "http://localhost:${BACKEND_PORT}/api/health" > /dev/null 2>&1; then
        echo "Backend healthy after $((i * 5))s"
        BACKEND_READY=true
        break
    fi
    [ $((i % 6)) -eq 0 ] && echo "  Still waiting for backend ($((i * 5))s)..."
    sleep 5
done
if [ "$BACKEND_READY" = "false" ]; then
    echo "WARNING: Backend not ready after 300s, running verify anyway"
    supervisorctl status 2>&1
fi

# Start cloudflared BEFORE final verify so the check includes tunnel status
if [ -n "${CLOUDFLARE_TUNNEL_TOKEN:-}" ]; then
    echo "Ensuring cloudflared is running..."
    supervisorctl start cloudflared 2>&1 || true
    for i in $(seq 1 15); do
        if supervisorctl status cloudflared 2>/dev/null | grep -q "RUNNING"; then
            echo "Cloudflare tunnel confirmed running"
            break
        fi
        sleep 2
    done
fi

# Run verification
echo "Running verify.sh..."
/app/verify.sh
VERIFY_RC=$?
echo "verify.sh exited with rc=$VERIFY_RC"

# Send verify result to host via HTTP (port 9081 VSOCK bridge, managed by supervisord)
if [ -f /data/seed/verify-result.json ]; then
    echo "Verify result contents:"
    cat /data/seed/verify-result.json
    echo ""
    echo "Uploading verify result to host via HTTP port 9081..."
    UPLOAD_RESP=$(curl -v --max-time 30 -T /data/seed/verify-result.json \
        "http://127.0.0.1:9081/upload/verify-result.json" 2>&1)
    echo "Upload response: $UPLOAD_RESP"
else
    echo "FATAL: /data/seed/verify-result.json does not exist!"
    ls -la /data/seed/ 2>&1
fi
echo "=== start-and-verify.sh finished at $(date -u) ==="
VERIFYEOF
    chmod +x /data/seed/start-and-verify.sh
    STARTUP_SCRIPT="/data/seed/start-and-verify.sh"
else
    # Fresh start (no restore) - run full verification before signaling
    cat > /data/seed/start-and-signal.sh <<'SIGNALEOF'
#!/bin/bash
exec >> /data/seed/startup-signal.log 2>&1
echo "=== Signal script $(date -u +%Y-%m-%dT%H:%M:%SZ) ==="

/data/seed/start-services.sh

# Wait for backend to be ready before running verification
echo "Waiting for backend to start..."
BACKEND_READY=false
for i in $(seq 1 60); do
    if curl -sf http://localhost:${PORT:-3100}/api/health > /dev/null 2>&1; then
        echo "Backend ready after $((i * 5))s"
        BACKEND_READY=true
        break
    fi
    if [ "$((i % 12))" -eq 0 ]; then
        echo "  Still waiting ($((i * 5))s)... supervisorctl:"
        supervisorctl status 2>&1
    fi
    sleep 5
done

if [ "$BACKEND_READY" = "false" ]; then
    echo "WARNING: Backend not ready after 300s"
    echo "Supervisorctl status:"
    supervisorctl status 2>&1
    echo "Backend log (last 30 lines):"
    tail -30 /var/log/supervisor/lightfriend-err.log 2>/dev/null
    tail -30 /var/log/supervisor/lightfriend.log 2>/dev/null
fi

echo "Running verification..."
sleep 3
/app/verify.sh || echo "WARNING: Verification reported failures"

# Send verify result to host via HTTP (port 9081 VSOCK bridge, managed by supervisord)
if [ -f /data/seed/verify-result.json ]; then
    curl -sf --max-time 30 -T /data/seed/verify-result.json \
        "http://127.0.0.1:9081/upload/verify-result.json" 2>/dev/null || true
fi

# Send startup logs to host boot trace receiver
echo "=== Sending startup logs to host ==="
cat /data/seed/startup-services.log /data/seed/startup-signal.log 2>/dev/null \
    | socat -T10 -u STDIN VSOCK-CONNECT:3:9007 2>/dev/null || true
SIGNALEOF
    chmod +x /data/seed/start-and-signal.sh
    STARTUP_SCRIPT="/data/seed/start-and-signal.sh"
fi

# Enable PostgreSQL autostart and run the startup orchestrator in background
sed -i 's/\[program:postgresql\]/[program:postgresql]/' /etc/supervisor/conf.d/lightfriend.conf

echo ""
echo "[STEP FINAL] Launching supervisord with startup script: ${STARTUP_SCRIPT}"
echo "  Entrypoint setup complete at $(date -u +%Y-%m-%dT%H:%M:%SZ)"

# Send boot trace now - exec will replace this shell so the EXIT trap won't fire
send_boot_trace 0

# Kill entrypoint bridges so supervisord can bind the same ports
if [ -n "${SEED_BRIDGE_PID:-}" ]; then kill "$SEED_BRIDGE_PID" 2>/dev/null; fi || true
sleep 0.2

# Startup script runs via supervisord's post-boot-verify program (see supervisord.conf).
# It detects /data/seed/start-and-verify.sh or /data/seed/start-and-signal.sh created above.

# Reset CWD to / before exec - restore process may have cd'd to /tmp dirs that get cleaned up,
# leaving child processes with invalid CWD (PostgreSQL "could not locate my own executable path")
cd /
exec /usr/bin/supervisord -c /etc/supervisor/conf.d/lightfriend.conf
