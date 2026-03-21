#!/bin/bash
# Don't use set -e to prevent one failure from stopping the entire script
# We'll check critical services individually

# Lightfriend Enclave Host Setup Script
# Environment: ${environment}
# Domain: ${subdomain}.${domain}

echo "Starting Lightfriend enclave host setup..."
echo "Environment: ${environment}"
echo "Hostname: ${subdomain}.${domain}"

# Update system
echo "Updating system packages..."
dnf update -y

# Install Nitro Enclaves CLI (optional - don't fail if this doesn't work)
echo "Installing Nitro Enclaves CLI..."
if dnf install -y aws-nitro-enclaves-cli aws-nitro-enclaves-cli-devel; then
    echo "Nitro Enclaves CLI installed successfully"

    # Configure Nitro Enclaves allocator
    # Reserve ~8GB RAM and 4 vCPUs for enclave (50% of c6a.2xlarge resources)
    # CPU count must be multiple of 2 (threads per core)
    # c6a.2xlarge has 8 vCPUs/16GB RAM
    mkdir -p /etc/nitro_enclaves
    cat > /etc/nitro_enclaves/allocator.yaml <<EOF
---
memory_mib: 8192
cpu_count: 4
EOF

    # Start Nitro Enclaves allocator
    systemctl enable nitro-enclaves-allocator || echo "Failed to enable nitro-enclaves-allocator"
    systemctl start nitro-enclaves-allocator || echo "Failed to start nitro-enclaves-allocator"

    # Add ec2-user to ne group for enclave management
    usermod -aG ne ec2-user || echo "Failed to add ec2-user to ne group"
else
    echo "WARNING: Nitro Enclaves CLI installation failed - continuing without enclave support"
fi

# Install Docker for EIF builds
echo "Installing Docker..."
dnf install -y docker
systemctl enable docker
systemctl start docker
usermod -aG docker ec2-user

# Set up /opt/lightfriend application directory
echo "Setting up /opt/lightfriend..."
mkdir -p /opt/lightfriend/{seed,backups}

chown -R ec2-user:ec2-user /opt/lightfriend

# ── Install HTTP forward proxy for enclave outbound traffic ──────────────────

echo "Installing tinyproxy and socat for enclave networking..."
dnf install -y tinyproxy socat jq || echo "WARNING: Failed to install tinyproxy/socat/jq"

# Configure tinyproxy - localhost only, permissive for enclave traffic
cat > /etc/tinyproxy/tinyproxy.conf <<'PROXYEOF'
User tinyproxy
Group tinyproxy
Port 3128
Listen 127.0.0.1
Timeout 600
MaxClients 100
Allow 127.0.0.1/8
ConnectPort 443
PROXYEOF

systemctl enable tinyproxy
systemctl start tinyproxy || echo "WARNING: tinyproxy failed to start"

# ── VSOCK services ──────────────────────────────────────────────────────────

# VSOCK bridge: enclave's VSOCK port 8001 -> tinyproxy on localhost:3128
cat > /etc/systemd/system/vsock-proxy-bridge.service <<'VSOCKEOF'
[Unit]
Description=VSOCK to tinyproxy bridge for Nitro Enclave
After=tinyproxy.service
Requires=tinyproxy.service

[Service]
ExecStart=/usr/bin/socat VSOCK-LISTEN:8001,reuseaddr,fork TCP:127.0.0.1:3128
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
VSOCKEOF

# VSOCK config server (port 9000) - serves .env to enclave
cat > /opt/lightfriend/config-server.sh <<'SCRIPT'
#!/bin/bash
while true; do
    socat VSOCK-LISTEN:9000,reuseaddr SYSTEM:"cat /opt/lightfriend/.env" 2>/dev/null
done
SCRIPT
chmod +x /opt/lightfriend/config-server.sh

cat > /etc/systemd/system/vsock-config-server.service <<'CFGEOF'
[Unit]
Description=VSOCK config server for Nitro Enclave (port 9000)

[Service]
ExecStart=/opt/lightfriend/config-server.sh
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
CFGEOF

# VSOCK backup receiver (port 9001) - receives backup from enclave
cat > /opt/lightfriend/backup-receiver.sh <<'SCRIPT'
#!/bin/bash
BACKUP_DIR="/opt/lightfriend/backups"
mkdir -p "$BACKUP_DIR"
while true; do
    TIMESTAMP=$(date -u +%Y%m%dT%H%M%SZ)
    socat -u VSOCK-LISTEN:9001,reuseaddr CREATE:"$${BACKUP_DIR}/backup-$${TIMESTAMP}.tar.gz.enc" 2>/dev/null
    echo "Received backup: backup-$${TIMESTAMP}.tar.gz.enc"
done
SCRIPT
chmod +x /opt/lightfriend/backup-receiver.sh

cat > /etc/systemd/system/vsock-backup-receiver.service <<'BKPEOF'
[Unit]
Description=VSOCK backup receiver for Nitro Enclave (port 9001)

[Service]
ExecStart=/opt/lightfriend/backup-receiver.sh
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
BKPEOF

# VSOCK seed server (port 9002) - sends backup to enclave for restore
cat > /opt/lightfriend/seed-server.sh <<'SCRIPT'
#!/bin/bash
# Serves the latest backup file to the enclave via VSOCK port 9002
while true; do
    LATEST=$(ls -t /opt/lightfriend/seed/*.tar.gz.enc 2>/dev/null | head -1)
    if [ -n "$LATEST" ]; then
        socat -u VSOCK-LISTEN:9002,reuseaddr FILE:"$LATEST" 2>/dev/null
    else
        socat VSOCK-LISTEN:9002,reuseaddr SYSTEM:"echo NO_BACKUP" 2>/dev/null
    fi
done
SCRIPT
chmod +x /opt/lightfriend/seed-server.sh

cat > /etc/systemd/system/vsock-seed-server.service <<'SEEDEOF'
[Unit]
Description=VSOCK seed server for Nitro Enclave (port 9002)

[Service]
ExecStart=/opt/lightfriend/seed-server.sh
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
SEEDEOF

# Enable and start all VSOCK services
systemctl daemon-reload
for svc in vsock-proxy-bridge vsock-config-server vsock-backup-receiver vsock-seed-server; do
    systemctl enable "$svc"
    systemctl start "$svc" || echo "WARNING: $svc failed to start"
done

echo "VSOCK services configured: proxy:8001, config:9000, backup:9001, seed:9002"

# ── Enclave launch script ───────────────────────────────────────────────────

cat > /opt/lightfriend/launch-enclave.sh <<'SCRIPT'
#!/bin/bash
set -e
IMAGE="ahtavarasmus/lightfriend-enclave:latest"
EIF_PATH="/opt/lightfriend/lightfriend.eif"

[ -f /opt/lightfriend/.env ] || { echo "ERROR: /opt/lightfriend/.env not found"; exit 1; }

# Skip pull+build if EIF already exists (pre-warmed)
if [ ! -f "$EIF_PATH" ]; then
    docker pull "$IMAGE"
    nitro-cli build-enclave --docker-uri "$IMAGE" --output-file "$EIF_PATH" 2>&1 | tee /tmp/eif-build.log
    grep -E "PCR[0-9]" /tmp/eif-build.log || true
else
    echo "Using pre-built EIF: $EIF_PATH"
fi

EXISTING=$(nitro-cli describe-enclaves | jq -r '.[0].EnclaveID // empty' 2>/dev/null)
[ -n "$EXISTING" ] && nitro-cli terminate-enclave --enclave-id "$EXISTING" && sleep 2

nitro-cli run-enclave --eif-path "$EIF_PATH" --memory 8192 --cpu-count 4 --enclave-cid 16
nitro-cli describe-enclaves
SCRIPT
chmod +x /opt/lightfriend/launch-enclave.sh

# ── S3 backup upload/download scripts ───────────────────────────────────────

cat > /opt/lightfriend/upload-backup.sh <<'SCRIPT'
#!/bin/bash
set -e
BACKUP=$(ls -t /opt/lightfriend/backups/*.tar.gz.enc 2>/dev/null | head -1)
[ -z "$BACKUP" ] && { echo "No backup found"; exit 1; }
BUCKET=$(grep S3_BACKUP_BUCKET /opt/lightfriend/.env | cut -d= -f2)
[ -z "$BUCKET" ] && { echo "S3_BACKUP_BUCKET not set in .env"; exit 1; }

LOCAL_SIZE=$(stat -c%s "$BACKUP" 2>/dev/null || stat -f%z "$BACKUP" 2>/dev/null)
S3_KEY="backups/$(basename $BACKUP)"

# Upload with checksum verification (S3 validates Content-MD5 on receipt)
aws s3 cp "$BACKUP" "s3://$${BUCKET}/$${S3_KEY}" --sse AES256

# Verify: S3 object exists and size matches local file
S3_SIZE=$(aws s3api head-object --bucket "$${BUCKET}" --key "$${S3_KEY}" --query ContentLength --output text 2>/dev/null || echo "0")
if [ "$${LOCAL_SIZE}" != "$${S3_SIZE}" ]; then
    echo "FATAL: S3 size mismatch - local=$${LOCAL_SIZE} s3=$${S3_SIZE}"
    exit 1
fi

echo "Uploaded and verified: s3://$${BUCKET}/$${S3_KEY} ($${S3_SIZE} bytes)"
SCRIPT
chmod +x /opt/lightfriend/upload-backup.sh

cat > /opt/lightfriend/download-backup.sh <<'SCRIPT'
#!/bin/bash
set -e
BUCKET=$(grep S3_BACKUP_BUCKET /opt/lightfriend/.env | cut -d= -f2)
[ -z "$BUCKET" ] && { echo "S3_BACKUP_BUCKET not set in .env"; exit 1; }
LATEST=$(aws s3 ls "s3://$${BUCKET}/backups/" | sort | tail -1 | awk '{print $4}')
[ -z "$LATEST" ] && { echo "No backups in S3"; exit 1; }
mkdir -p /opt/lightfriend/seed

# Get expected size from S3 before download
S3_SIZE=$(aws s3api head-object --bucket "$${BUCKET}" --key "backups/$${LATEST}" --query ContentLength --output text)
echo "Expected size: $${S3_SIZE} bytes"

aws s3 cp "s3://$${BUCKET}/backups/$${LATEST}" "/opt/lightfriend/seed/$${LATEST}"

# Verify downloaded file matches S3 size
LOCAL_SIZE=$(stat -c%s "/opt/lightfriend/seed/$${LATEST}" 2>/dev/null || stat -f%z "/opt/lightfriend/seed/$${LATEST}" 2>/dev/null)
if [ "$${LOCAL_SIZE}" != "$${S3_SIZE}" ]; then
    echo "FATAL: Download size mismatch - expected=$${S3_SIZE} got=$${LOCAL_SIZE}"
    rm -f "/opt/lightfriend/seed/$${LATEST}"
    exit 1
fi
echo "Downloaded and verified: $${LATEST} ($${LOCAL_SIZE} bytes)"
SCRIPT
chmod +x /opt/lightfriend/download-backup.sh

# ── Pre-deploy script (run on OLD instance before blue-green) ────────────

cat > /opt/lightfriend/trigger-export.sh <<'SCRIPT'
#!/bin/bash
set -e
echo "Triggering export in enclave via VSOCK port 9003..."
echo "This will block until export completes (5-10 minutes)..."
# Connect to enclave CID 16, port 9003. The enclave runs export.sh,
# sends backup via port 9001, then closes this connection.
timeout 900 socat -T600 - VSOCK-CONNECT:16:9003 || { echo "Export trigger failed or timed out"; exit 1; }
echo "Export complete."
ls -la /opt/lightfriend/backups/ | tail -3
SCRIPT
chmod +x /opt/lightfriend/trigger-export.sh

cat > /opt/lightfriend/upload-env.sh <<'SCRIPT'
#!/bin/bash
set -e
[ -f /opt/lightfriend/.env ] || { echo "No .env file found"; exit 1; }
BUCKET=$(grep S3_BACKUP_BUCKET /opt/lightfriend/.env | cut -d= -f2)
[ -z "$BUCKET" ] && { echo "S3_BACKUP_BUCKET not set in .env"; exit 1; }
aws s3 cp /opt/lightfriend/.env "s3://$BUCKET/config/.env" --sse AES256
echo "Uploaded .env to s3://$BUCKET/config/.env"
SCRIPT
chmod +x /opt/lightfriend/upload-env.sh

cat > /opt/lightfriend/trigger-maintenance.sh <<'SCRIPT'
#!/bin/bash
set -euo pipefail
ACTION="$${1:-status}"
RESULT=$(echo "$ACTION" | timeout 30 socat - VSOCK-CONNECT:16:9005 2>&1) || {
    echo "FATAL: Maintenance trigger via VSOCK failed"
    exit 1
}
echo "$RESULT"
SCRIPT
chmod +x /opt/lightfriend/trigger-maintenance.sh

cat > /opt/lightfriend/pre-deploy.sh <<'SCRIPT'
#!/bin/bash
set -e
LOCKFILE="/tmp/lightfriend-backup.lock"
exec 200>"$LOCKFILE"
flock -n 200 || { echo "FATAL: Another backup/export is already running"; exit 1; }
echo "=== Pre-deploy: maintenance mode + export + upload ==="
echo "Enabling maintenance mode..."
/opt/lightfriend/trigger-maintenance.sh enable
/opt/lightfriend/trigger-export.sh
/opt/lightfriend/upload-backup.sh
/opt/lightfriend/upload-env.sh
echo "=== Pre-deploy complete ==="
SCRIPT
chmod +x /opt/lightfriend/pre-deploy.sh

# ── PG-only backup trigger script ────────────────────────────────────────

cat > /opt/lightfriend/trigger-pg-backup.sh <<'SCRIPT'
#!/bin/bash
set -e
echo "Triggering PG-only backup in enclave via VSOCK port 9006..."
echo "This runs against live PostgreSQL (zero downtime)..."
timeout 300 socat -T300 - VSOCK-CONNECT:16:9006 || { echo "PG backup trigger failed or timed out"; exit 1; }
echo "PG backup complete."
ls -la /opt/lightfriend/backups/ | tail -3
SCRIPT
chmod +x /opt/lightfriend/trigger-pg-backup.sh

# ── Scheduled daily backup ───────────────────────────────────────────────
# PG-only backup: zero downtime (MVCC snapshot against live PostgreSQL).
# Full exports (all data stores, requires stopping services) only happen
# during deploys, where maintenance mode is already active.
# Worst-case data loss: 24 hours of PostgreSQL data. Bridge/Matrix state
# is reconstructable (users re-link).

cat > /opt/lightfriend/scheduled-backup.sh <<'SCRIPT'
#!/bin/bash
set -euo pipefail
LOCKFILE="/tmp/lightfriend-backup.lock"
exec 200>"$LOCKFILE"
flock -n 200 || { echo "Skipping: another backup/deploy is running"; exit 0; }
LOG="/var/log/lightfriend-backup.log"
TIMESTAMP=$(date -u +%Y-%m-%dT%H:%M:%SZ)
echo "=== Scheduled PG backup: $${TIMESTAMP} ===" >> "$LOG"

BUCKET=$(grep S3_BACKUP_BUCKET /opt/lightfriend/.env | cut -d= -f2)

# Step 1: Trigger PG-only backup inside enclave (zero downtime)
/opt/lightfriend/trigger-pg-backup.sh >> "$LOG" 2>&1 || {
    echo "FAILED: PG backup trigger failed at $${TIMESTAMP}" >> "$LOG"
    # Write failure marker to S3 so deploys can detect stale backups
    [ -n "$${BUCKET:-}" ] && echo "{\"last_failure\": \"$${TIMESTAMP}\"}" | \
        aws s3 cp - "s3://$${BUCKET}/backups/backup-health.json" 2>/dev/null || true
    exit 1
}

# Step 2: Upload to S3 (upload-backup.sh verifies size match after upload)
/opt/lightfriend/upload-backup.sh >> "$LOG" 2>&1 || {
    echo "FAILED: S3 upload/verify failed at $${TIMESTAMP}" >> "$LOG"
    [ -n "$${BUCKET:-}" ] && echo "{\"last_failure\": \"$${TIMESTAMP}\"}" | \
        aws s3 cp - "s3://$${BUCKET}/backups/backup-health.json" 2>/dev/null || true
    exit 1
}

# Step 3: Clean up old local backups (keep last 3)
cd /opt/lightfriend/backups
ls -t *.tar.gz.enc 2>/dev/null | tail -n +4 | xargs rm -f 2>/dev/null || true

# Step 4: Write success marker to S3
[ -n "$${BUCKET:-}" ] && echo "{\"last_success\": \"$${TIMESTAMP}\", \"type\": \"pg_only\"}" | \
    aws s3 cp - "s3://$${BUCKET}/backups/backup-health.json" 2>/dev/null || true

echo "=== PG backup verified and complete: $${TIMESTAMP} ===" >> "$LOG"
SCRIPT
chmod +x /opt/lightfriend/scheduled-backup.sh

echo "0 3 * * * root /opt/lightfriend/scheduled-backup.sh" > /etc/cron.d/lightfriend-backup
chmod 644 /etc/cron.d/lightfriend-backup

chown -R ec2-user:ec2-user /opt/lightfriend

echo "Lightfriend enclave host setup complete!"

# ── Auto-bootstrap (two-phase for blue-green deploys) ─────────────────────
# Phase 1: Pre-warm (docker pull + EIF build) while old instance still serves
# Phase 2: Wait for CI proceed signal, then download backup + launch enclave

BUCKET=$(aws ssm get-parameter --name /lightfriend/s3-bucket --query Parameter.Value --output text 2>/dev/null || echo "")

if [ -n "$BUCKET" ] && aws s3 ls "s3://$BUCKET/config/.env" 2>/dev/null; then
    INSTANCE_ID=$(curl -s http://169.254.169.254/latest/meta-data/instance-id)
    VERIFY="/opt/lightfriend/verify-result.json"

    # ── Phase 1: Pre-warm ─────────────────────────────────────────────────
    echo "=== Phase 1: Pre-warming (docker pull + EIF build) ==="

    # Download .env (needed for launch-enclave.sh .env check)
    aws s3 cp "s3://$BUCKET/config/.env" /opt/lightfriend/.env
    chmod 600 /opt/lightfriend/.env

    # Pull image and build EIF (the slow part - 10-15 min)
    IMAGE="ahtavarasmus/lightfriend-enclave:latest"
    EIF_PATH="/opt/lightfriend/lightfriend.eif"
    if ! docker pull "$IMAGE" 2>&1 | tee /tmp/pull.log; then
        echo "{\"status\": \"PULL_FAILED\", \"instance_id\": \"$INSTANCE_ID\"}" | \
            aws s3 cp - "s3://$BUCKET/deploy/pre-warm-$INSTANCE_ID.json"
        exit 1
    fi
    if ! nitro-cli build-enclave --docker-uri "$IMAGE" --output-file "$EIF_PATH" 2>&1 | tee /tmp/eif-build.log; then
        LAUNCH_ERR=$(tail -5 /tmp/eif-build.log | tr '\n' ' ' | head -c 500)
        echo "{\"status\": \"BUILD_FAILED\", \"instance_id\": \"$INSTANCE_ID\", \"error\": \"$LAUNCH_ERR\"}" | \
            aws s3 cp - "s3://$BUCKET/deploy/pre-warm-$INSTANCE_ID.json"
        exit 1
    fi

    # Signal pre-warm complete
    echo "{\"status\": \"PRE_WARM_COMPLETE\", \"instance_id\": \"$INSTANCE_ID\", \"timestamp\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\"}" | \
        aws s3 cp - "s3://$BUCKET/deploy/pre-warm-$INSTANCE_ID.json"
    echo "=== Phase 1 complete: EIF built, waiting for proceed signal ==="

    # ── Phase 2: Wait for proceed, then restore + launch ──────────────────
    echo "Polling s3://$BUCKET/deploy/proceed-$INSTANCE_ID.json..."
    PROCEED=""
    for i in $(seq 1 180); do
        PROCEED=$(aws s3 cp "s3://$BUCKET/deploy/proceed-$INSTANCE_ID.json" - 2>/dev/null || echo "")
        if [ -n "$PROCEED" ]; then
            echo "Proceed signal received"
            break
        fi
        sleep 10
    done

    if [ -z "$PROCEED" ]; then
        echo "{\"status\": \"PROCEED_TIMEOUT\", \"instance_id\": \"$INSTANCE_ID\"}" | \
            aws s3 cp - "s3://$BUCKET/deploy/verify-$INSTANCE_ID.json"
        echo "FATAL: No proceed signal after 30 minutes"
        exit 1
    fi

    # Re-download .env (CI may have uploaded a fresh one from old instance)
    aws s3 cp "s3://$BUCKET/config/.env" /opt/lightfriend/.env
    chmod 600 /opt/lightfriend/.env

    # Download backup - MUST succeed if this is a migration (not first deploy)
    echo "Downloading backup from S3..."
    BACKUP_LIST=$(aws s3 ls "s3://$BUCKET/backups/" 2>/dev/null || echo "")
    if echo "$BACKUP_LIST" | grep -q ".tar.gz.enc"; then
        /opt/lightfriend/download-backup.sh || {
            echo "FATAL: Backup exists in S3 but download failed"
            echo "{\"status\": \"DOWNLOAD_FAILED\", \"instance_id\": \"$INSTANCE_ID\"}" | \
                aws s3 cp - "s3://$BUCKET/deploy/verify-$INSTANCE_ID.json"
            exit 1
        }
    else
        # Check if an old instance exists - if so, backups MUST exist
        OLD_INSTANCE=$(aws ssm get-parameter --name /lightfriend/instance-id --query Parameter.Value --output text 2>/dev/null || echo "")
        if [ -n "$OLD_INSTANCE" ] && [ "$OLD_INSTANCE" != "$INSTANCE_ID" ]; then
            echo "FATAL: Old instance $OLD_INSTANCE exists but no backups found in S3"
            echo "{\"status\": \"NO_BACKUP_FOR_MIGRATION\", \"instance_id\": \"$INSTANCE_ID\", \"old_instance\": \"$OLD_INSTANCE\"}" | \
                aws s3 cp - "s3://$BUCKET/deploy/verify-$INSTANCE_ID.json"
            exit 1
        fi
        echo "No backups in S3 (first deploy)"
    fi

    # Launch enclave (EIF already built - just nitro-cli run-enclave)
    echo "Launching enclave..."
    if ! /opt/lightfriend/launch-enclave.sh 2>&1 | tee /tmp/launch.log; then
        LAUNCH_ERR=$(tail -5 /tmp/launch.log | tr '\n' ' ' | head -c 500)
        echo "{\"status\": \"LAUNCH_FAILED\", \"instance_id\": \"$INSTANCE_ID\", \"error\": \"$LAUNCH_ERR\"}" | \
            aws s3 cp - "s3://$BUCKET/deploy/verify-$INSTANCE_ID.json"
        exit 1
    fi

    # Wait for enclave verify result via VSOCK port 9004
    # 15 min: restore can take a while (PG restore + service startup + verification)
    echo "Waiting for enclave verify result (up to 15 min)..."
    timeout 900 socat -u VSOCK-LISTEN:9004,reuseaddr CREATE:"$VERIFY" 2>/dev/null || echo "Verify signal timeout"

    if [ -s "$VERIFY" ]; then
        aws s3 cp "$VERIFY" "s3://$BUCKET/deploy/verify-$INSTANCE_ID.json"
        echo "Verify result uploaded"
        cat "$VERIFY"
    else
        ENCLAVE_STATUS=$(nitro-cli describe-enclaves 2>/dev/null | jq -r '.[0].State // "unknown"')
        echo "{\"status\": \"TIMEOUT\", \"instance_id\": \"$INSTANCE_ID\", \"enclave_state\": \"$ENCLAVE_STATUS\"}" | \
            aws s3 cp - "s3://$BUCKET/deploy/verify-$INSTANCE_ID.json"
        echo "WARNING: No verify result received (enclave state: $ENCLAVE_STATUS)"
    fi

    echo "=== Auto-bootstrap complete ==="
else
    echo "No .env in S3 - skipping auto-bootstrap (manual first-time setup required)"
fi
