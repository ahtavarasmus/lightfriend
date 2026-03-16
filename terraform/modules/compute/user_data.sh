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
ConnectPort 0
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
    socat -u VSOCK-LISTEN:9001,reuseaddr CREATE:"${BACKUP_DIR}/backup-${TIMESTAMP}.tar.gz.enc" 2>/dev/null
    echo "Received backup: backup-${TIMESTAMP}.tar.gz.enc"
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

docker pull "$IMAGE"
nitro-cli build-enclave --docker-uri "$IMAGE" --output-file "$EIF_PATH" 2>&1 | tee /tmp/eif-build.log
grep -E "PCR[0-9]" /tmp/eif-build.log || true

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
aws s3 cp "$BACKUP" "s3://${BUCKET}/backups/$(basename $BACKUP)"
echo "Uploaded: s3://${BUCKET}/backups/$(basename $BACKUP)"
SCRIPT
chmod +x /opt/lightfriend/upload-backup.sh

cat > /opt/lightfriend/download-backup.sh <<'SCRIPT'
#!/bin/bash
set -e
BUCKET=$(grep S3_BACKUP_BUCKET /opt/lightfriend/.env | cut -d= -f2)
[ -z "$BUCKET" ] && { echo "S3_BACKUP_BUCKET not set in .env"; exit 1; }
LATEST=$(aws s3 ls "s3://${BUCKET}/backups/" | sort | tail -1 | awk '{print $4}')
[ -z "$LATEST" ] && { echo "No backups in S3"; exit 1; }
mkdir -p /opt/lightfriend/seed
aws s3 cp "s3://${BUCKET}/backups/${LATEST}" "/opt/lightfriend/seed/${LATEST}"
echo "Downloaded: ${LATEST}"
SCRIPT
chmod +x /opt/lightfriend/download-backup.sh

chown -R ec2-user:ec2-user /opt/lightfriend

echo "Lightfriend enclave host setup complete!"
