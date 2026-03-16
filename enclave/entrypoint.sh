#!/bin/bash
set -e

echo "=== Lightfriend Enclave Entrypoint ==="

# ── 0a. Fetch environment from host via VSOCK ────────────────────────────────
if [ -e /dev/vsock ]; then
    echo "Fetching environment from host via VSOCK..."
    for attempt in $(seq 1 10); do
        if socat -T5 - VSOCK-CONNECT:3:9000 > /tmp/host_env 2>/dev/null && [ -s /tmp/host_env ]; then
            set -a
            source /tmp/host_env
            set +a
            echo "Environment loaded from host (attempt $attempt)"
            rm -f /tmp/host_env
            break
        fi
        echo "  Waiting for host config server (attempt $attempt/10)..."
        sleep 2
    done
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

# ── 0d. Fetch backup from host via VSOCK (if available) ─────────────────────
if [ -e /dev/vsock ]; then
    echo "Checking host for backup to restore..."
    mkdir -p /data/seed
    RECEIVED="/data/seed/backup-received.tar.gz.enc"
    socat -T10 -u VSOCK-CONNECT:3:9002 CREATE:"$RECEIVED" 2>/dev/null || true
    if [ -s "$RECEIVED" ] && ! grep -q "NO_BACKUP" "$RECEIVED" 2>/dev/null; then
        RECEIVED_SIZE=$(stat -c%s "$RECEIVED" 2>/dev/null || stat -f%z "$RECEIVED" 2>/dev/null || echo "unknown")
        echo "Backup received from host (${RECEIVED_SIZE} bytes)"
        # Rename to match expected pattern for full restore
        mv "$RECEIVED" "/data/seed/lightfriend-full-backup-received.tar.gz.enc"
    else
        rm -f "$RECEIVED"
        echo "No backup available from host - starting fresh"
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
        su postgres -c "psql -d ${db} < ${dump_file}" 2>/dev/null \
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
        local value="${!var}"
        value=$(printf '%s\n' "$value" | sed 's/[&/\]/\\&/g')
        content=$(echo "$content" | sed "s/\${${var}}/${value}/g")
        # Also handle ${VAR:-default} patterns - strip the default syntax
        content=$(echo "$content" | sed "s/\${${var}:-[^}]*}/${value}/g")
    done

    echo "$content" > "$output_file"
}

# Generate Tuwunel config
substitute_vars /etc/enclave-configs/tuwunel.toml.template /etc/tuwunel/tuwunel.toml
echo "  Generated /etc/tuwunel/tuwunel.toml"

# Generate bridge configs
for bridge in whatsapp signal telegram; do
    substitute_vars "/etc/enclave-configs/${bridge}.yaml.template" "/data/bridges/${bridge}/config.yaml"
    echo "  Generated /data/bridges/${bridge}/config.yaml"
done

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
    /opt/mautrix-telegram/bin/python -m mautrix_telegram -g -c "$TELEGRAM_CFG" -r "$TELEGRAM_REG" 2>/dev/null || true
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

if [ -n "${CLOUDFLARE_TUNNEL_TOKEN:-}" ]; then
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
/tmp/start-services.sh
echo "Running post-restore verification..."
sleep 5
/app/verify.sh || echo "WARNING: Post-restore verification reported failures. Check /data/seed/verify-result.json"
VERIFYEOF
    chmod +x /tmp/start-and-verify.sh
    STARTUP_SCRIPT="/tmp/start-and-verify.sh"
else
    STARTUP_SCRIPT="/tmp/start-services.sh"
fi

# Enable PostgreSQL autostart and run the startup orchestrator in background
sed -i 's/\[program:postgresql\]/[program:postgresql]/' /etc/supervisor/conf.d/lightfriend.conf

# Start supervisord (postgresql starts via autostart=true, rest via startup script)
${STARTUP_SCRIPT} &
exec /usr/bin/supervisord -c /etc/supervisor/conf.d/lightfriend.conf
