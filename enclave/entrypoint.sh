#!/bin/bash
set -euo pipefail

echo "=== Lightfriend Enclave Entrypoint ==="

# ── 0a. Fetch environment from host via VSOCK ────────────────────────────────
ENV_LOADED=false
if [ -e /dev/vsock ]; then
    echo "Fetching environment from host via VSOCK..."
    for attempt in $(seq 1 10); do
        if socat -T5 - VSOCK-CONNECT:3:9000 > /tmp/host_env 2>/dev/null && [ -s /tmp/host_env ]; then
            set -a
            source /tmp/host_env
            set +a
            echo "Environment loaded from host (attempt $attempt)"
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
export PORT="${PORT:-3000}"
export SKIP_BACKEND="${SKIP_BACKEND:-false}"
export RESTORE_MODE="${RESTORE_MODE:-none}"

# ── 0c. Start VSOCK outbound proxy bridge ────────────────────────────────────

if [ -e /dev/vsock ]; then
    echo "VSOCK device detected - starting outbound proxy bridge..."
    socat TCP-LISTEN:3128,reuseaddr,fork VSOCK-CONNECT:3:8001 &
    SOCAT_PID=$!
    sleep 0.5

    if ! kill -0 $SOCAT_PID 2>/dev/null; then
        echo "ERROR: VSOCK bridge failed to start"
        exit 1
    fi
    echo "VSOCK bridge running (PID $SOCAT_PID)"

    export HTTP_PROXY="http://127.0.0.1:3128"
    export HTTPS_PROXY="http://127.0.0.1:3128"
    export NO_PROXY="localhost,127.0.0.1"
else
    echo "No VSOCK device - running in direct network mode"
fi

# In local Docker mode there is no host-env bootstrap step, so preserve the
# dev-only fallback key in memory before the runtime derivation step.
if [ -n "${BACKUP_ENCRYPTION_KEY:-}" ] && [ "${ALLOW_INSECURE_BACKUP_KEY_FALLBACK:-false}" = "true" ] && [ -z "${INSECURE_BACKUP_ENCRYPTION_KEY_FALLBACK:-}" ]; then
    export INSECURE_BACKUP_ENCRYPTION_KEY_FALLBACK="${BACKUP_ENCRYPTION_KEY}"
fi

# ── 0d. Derive backup encryption key (runtime only, never persisted) ────────
echo "Deriving runtime backup encryption key..."
if ! BACKUP_ENCRYPTION_KEY="$(/usr/local/bin/derive-backup-key.sh)"; then
    echo "FATAL: Failed to derive BACKUP_ENCRYPTION_KEY"
    exit 1
fi
if [ -z "${BACKUP_ENCRYPTION_KEY}" ]; then
    echo "FATAL: Derived BACKUP_ENCRYPTION_KEY is empty"
    exit 1
fi
export BACKUP_ENCRYPTION_KEY
unset INSECURE_BACKUP_ENCRYPTION_KEY_FALLBACK

BACKUP_KEY_FINGERPRINT="$(printf '%s' "${BACKUP_ENCRYPTION_KEY}" | sha256sum | awk '{print $1}' | cut -c1-16)"
echo "Runtime backup key ready (fingerprint: ${BACKUP_KEY_FINGERPRINT})"

# ── 0e. Fetch backup from host via VSOCK (explicit restore only) ────────────
if [ -e /dev/vsock ] && [ "${RESTORE_MODE}" != "none" ]; then
    echo "Restore requested (mode: ${RESTORE_MODE}) - fetching backup from host..."
    mkdir -p /data/seed
    RECEIVED="/data/seed/backup-received.tar.gz.enc"
    socat -T300 -u VSOCK-CONNECT:3:9002 CREATE:"$RECEIVED" 2>/dev/null || true
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
elif [ "${RESTORE_MODE}" != "none" ]; then
    echo "FATAL: Restore requested but no VSOCK device is available"
    exit 1
else
    echo "No restore requested - starting with current state"
fi

# ── 0f. Fetch one-time SQL seed from host via VSOCK (first bootstrap only) ──
if [ -e /dev/vsock ] && [ "${RESTORE_MODE}" = "none" ]; then
    mkdir -p /data/seed
    SEED_DUMP="/data/seed/lightfriend_db.sql"
    if [ ! -f "$SEED_DUMP" ]; then
        echo "Checking host for one-time SQL seed..."
        RECEIVED_SEED="/data/seed/lightfriend_db.seed.tmp"
        socat -T30 -u VSOCK-CONNECT:3:9003 CREATE:"$RECEIVED_SEED" 2>/dev/null || true
        if [ -s "$RECEIVED_SEED" ] && ! grep -q "NO_SEED" "$RECEIVED_SEED" 2>/dev/null; then
            mv "$RECEIVED_SEED" "$SEED_DUMP"
            SEED_SIZE=$(stat -c%s "$SEED_DUMP" 2>/dev/null || stat -f%z "$SEED_DUMP" 2>/dev/null || echo "unknown")
            echo "SQL seed received from host (${SEED_SIZE} bytes)"
        else
            rm -f "$RECEIVED_SEED"
            echo "No SQL seed available from host"
        fi
    fi
fi

# ── 1. Initialize PostgreSQL if needed ──────────────────────────────────────

PG_DATA="/var/lib/postgresql/data"
PG_BIN="/usr/lib/postgresql/15/bin"

if [ ! -f "$PG_DATA/PG_VERSION" ]; then
    echo "Initializing PostgreSQL data directory..."
    chown -R postgres:postgres "$PG_DATA"
    su postgres -c "$PG_BIN/initdb -D $PG_DATA"

    # Allow local connections without password (all in same container)
    echo "host all all 127.0.0.1/32 trust" >> "$PG_DATA/pg_hba.conf"
    echo "local all all trust" >> "$PG_DATA/pg_hba.conf"

    # Listen on localhost only
    sed -i "s/#listen_addresses = 'localhost'/listen_addresses = 'localhost'/" "$PG_DATA/postgresql.conf"
fi

chown -R postgres:postgres "$PG_DATA" /run/postgresql

# ── 2. Start PostgreSQL temporarily to create databases ─────────────────────

echo "Starting PostgreSQL for initialization..."
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
FULL_BACKUP=$(ls /data/seed/lightfriend-full-backup-*.tar.gz.enc 2>/dev/null | head -1 || true)

if [ -n "${FULL_BACKUP}" ]; then
    echo "=== Full encrypted backup detected: $(basename ${FULL_BACKUP}) ==="
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
        # Send failure to host via VSOCK so CI gets the actual error, not just TIMEOUT
        if [ -e /dev/vsock ]; then
            echo "{\"status\": \"RESTORE_FAILED\", \"restore_type\": \"full_restore\", \"error\": \"${msg}\", \"step\": \"${step}\", \"user_count\": 0}" > /data/seed/verify-result.json
            socat -u FILE:/data/seed/verify-result.json VSOCK-CONNECT:3:9004 2>/dev/null || true
        fi
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
PG_BACKUP=$(ls /data/seed/lightfriend-pg-backup-*.tar.gz.enc 2>/dev/null | head -1 || true)

if [ -n "${PG_BACKUP}" ]; then
    echo "=== PG-only backup detected: $(basename ${PG_BACKUP}) ==="
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
if [ "${FULL_RESTORE_DONE}" = "false" ]; then
SEED_DUMP="/data/seed/lightfriend_db.sql"
if [ -f "$SEED_DUMP" ]; then
    echo "Restoring lightfriend_db from seed dump..."
    su postgres -c "psql -d lightfriend_db < $SEED_DUMP"

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

echo "Generating configuration files from templates..."

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
        content=$(echo "$content" | sed "s/\${${var}}/${value}/g")
        # Also handle ${VAR:-default} patterns - strip the default syntax
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

# Use a startup script that starts services in order
cat > /tmp/start-services.sh << 'STARTUP'
#!/bin/bash
# Wait for PostgreSQL to be ready, then start dependent services

sleep 2

# Wait for PG
for i in $(seq 1 30); do
    if pg_isready -h localhost -U postgres > /dev/null 2>&1; then
        echo "PostgreSQL is ready."
        break
    fi
    if [ "$i" -eq 30 ]; then
        echo "ERROR: PostgreSQL not ready after 30 seconds"
        exit 1
    fi
    sleep 1
done

# Start Tuwunel
supervisorctl start tuwunel

# Wait for Tuwunel
for i in $(seq 1 30); do
    if curl -sf http://localhost:8008/_matrix/client/versions > /dev/null 2>&1; then
        echo "Tuwunel is ready."
        break
    fi
    if [ "$i" -eq 30 ]; then
        echo "WARNING: Tuwunel not responding after 30 seconds, starting bridges anyway"
    fi
    sleep 1
done

# Register bridge bots (first run only, idempotent)
cd /app && bash register-bridge-bots.sh http://localhost:8008 2>/dev/null || true

# Start bridges
supervisorctl start mautrix-whatsapp
supervisorctl start mautrix-signal
supervisorctl start mautrix-telegram

# Start Lightfriend backend (unless SKIP_BACKEND)
if [ "${SKIP_BACKEND}" != "true" ]; then
    sleep 2
    supervisorctl start lightfriend
fi

if [ -n "${CLOUDFLARE_TUNNEL_TOKEN:-}" ] && [ "${DEFER_CLOUDFLARED:-}" != "true" ]; then
    supervisorctl start cloudflared
    echo "Cloudflare tunnel started"
fi

echo "=== All services started ==="
STARTUP
chmod +x /tmp/start-services.sh

# If a full restore was done, create a post-startup verification wrapper
if [ "${FULL_RESTORE_DONE}" = "true" ]; then
    cat > /tmp/start-and-verify.sh <<'VERIFYEOF'
#!/bin/bash
# Start services but DO NOT start cloudflared yet (defer until verified)
DEFER_CLOUDFLARED=true
export DEFER_CLOUDFLARED
/tmp/start-services.sh

echo "Running post-restore verification (pre-cloudflared)..."
sleep 5

# Start cloudflared BEFORE final verify so the check includes tunnel status
if [ -n "${CLOUDFLARE_TUNNEL_TOKEN:-}" ]; then
    supervisorctl start cloudflared
    echo "Cloudflare tunnel starting..."
    # Wait for cloudflared to be RUNNING (up to 30s)
    for i in $(seq 1 15); do
        if supervisorctl status cloudflared 2>/dev/null | grep -q "RUNNING"; then
            echo "Cloudflare tunnel confirmed running"
            break
        fi
        sleep 2
    done
fi

# Now run verification (includes cloudflared check)
/app/verify.sh || echo "WARNING: Post-restore verification reported failures. Check /data/seed/verify-result.json"

# Send verify result to host via VSOCK port 9004
if [ -e /dev/vsock ] && [ -f /data/seed/verify-result.json ]; then
    echo "Sending verify result to host via VSOCK..."
    socat -u FILE:/data/seed/verify-result.json VSOCK-CONNECT:3:9004 2>/dev/null \
        || echo "WARNING: Failed to send verify result via VSOCK"
fi
VERIFYEOF
    chmod +x /tmp/start-and-verify.sh
    STARTUP_SCRIPT="/tmp/start-and-verify.sh"
else
    # Fresh start (no restore) - run full verification before signaling
    cat > /tmp/start-and-signal.sh <<'SIGNALEOF'
#!/bin/bash
/tmp/start-services.sh

# Wait for backend to be ready before running verification
echo "Waiting for backend to start..."
for i in $(seq 1 60); do
    if curl -sf http://localhost:3000/api/health > /dev/null 2>&1; then
        echo "Backend ready"
        break
    fi
    sleep 5
done

echo "Running verification..."
sleep 3
/app/verify.sh || echo "WARNING: Verification reported failures"

# Send verify result to host via VSOCK port 9004
if [ -e /dev/vsock ] && [ -f /data/seed/verify-result.json ]; then
    socat -u FILE:/data/seed/verify-result.json VSOCK-CONNECT:3:9004 2>/dev/null || true
fi
SIGNALEOF
    chmod +x /tmp/start-and-signal.sh
    STARTUP_SCRIPT="/tmp/start-and-signal.sh"
fi

# Enable PostgreSQL autostart and run the startup orchestrator in background
sed -i 's/\[program:postgresql\]/[program:postgresql]/' /etc/supervisor/conf.d/lightfriend.conf

# Start supervisord (postgresql starts via autostart=true, rest via startup script)
${STARTUP_SCRIPT} &
exec /usr/bin/supervisord -c /etc/supervisor/conf.d/lightfriend.conf
