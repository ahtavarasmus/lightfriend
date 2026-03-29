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
mkdir -p /opt/lightfriend/{backups,restore,seed}

chown -R ec2-user:ec2-user /opt/lightfriend

# ── Install HTTP forward proxy for enclave outbound traffic ──────────────────

echo "Installing squid and socat for enclave networking..."
dnf install -y squid socat jq || echo "WARNING: Failed to install squid/socat/jq"

# Configure squid - localhost only, no caching, permissive HTTPS CONNECT
mkdir -p /var/spool/squid /var/log/squid
chown -R squid:squid /var/spool/squid /var/log/squid || true

cat > /etc/squid/squid.conf <<'PROXYEOF'
http_port 127.0.0.1:3128
acl localhost src 127.0.0.1/32
http_access allow localhost
http_access deny all
cache deny all
access_log stdio:/var/log/squid/access.log
cache_log /var/log/squid/cache.log
pid_filename /run/squid.pid
coredump_dir /var/spool/squid
PROXYEOF

squid -k parse || echo "WARNING: squid config validation failed"
squid -z || echo "WARNING: squid cache init failed"
systemctl enable squid
systemctl start squid || echo "WARNING: squid failed to start"

# ── VSOCK services ──────────────────────────────────────────────────────────

# VSOCK bridge: enclave's VSOCK port 8001 -> squid on localhost:3128
cat > /etc/systemd/system/vsock-proxy-bridge.service <<'VSOCKEOF'
[Unit]
Description=VSOCK to squid bridge for Nitro Enclave
After=squid.service
Requires=squid.service

[Service]
ExecStart=/usr/bin/socat VSOCK-LISTEN:8001,reuseaddr,fork TCP:127.0.0.1:3128
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
VSOCKEOF

# VSOCK bridge: enclave's VSOCK port 9010 -> Marlin KMS root server TCP endpoint
cat > /opt/lightfriend/marlin-kms-bridge.sh <<'SCRIPT'
#!/bin/bash
set -eu

while true; do
    if [ ! -f /opt/lightfriend/.env ]; then
        sleep 5
        continue
    fi

    ENDPOINT=$(grep '^MARLIN_ROOT_SERVER_ENDPOINT=' /opt/lightfriend/.env 2>/dev/null | tail -1 | cut -d= -f2- | tr -d '\r')
    if [ -z "$${ENDPOINT}" ]; then
        sleep 30
        continue
    fi

    HOST="$${ENDPOINT%:*}"
    PORT="$${ENDPOINT##*:}"
    /usr/bin/socat VSOCK-LISTEN:9010,reuseaddr,fork TCP:"$${HOST}":"$${PORT}" 2>/dev/null || true
    sleep 2
done
SCRIPT
chmod +x /opt/lightfriend/marlin-kms-bridge.sh

cat > /etc/systemd/system/vsock-marlin-kms-bridge.service <<'KMSBRIDGEEOF'
[Unit]
Description=VSOCK bridge to Marlin KMS root server for Nitro Enclave
After=network-online.target
Wants=network-online.target

[Service]
ExecStart=/opt/lightfriend/marlin-kms-bridge.sh
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
KMSBRIDGEEOF

# VSOCK config server (port 9000) - serves .env + any one-shot restore vars
cat > /opt/lightfriend/config-server.sh <<'SCRIPT'
#!/bin/bash
set -eu

LOG="/opt/lightfriend/logs/config-server.log"
mkdir -p /opt/lightfriend/logs

log() { echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) config-server: $*" | tee -a "$LOG"; }

render_host_env() {
    TMP=$(mktemp /opt/lightfriend/host-env.XXXXXX)
    cat /opt/lightfriend/.env > "$TMP"
    if [ -f /opt/lightfriend/runtime.env ]; then
        printf '\n' >> "$TMP"
        cat /opt/lightfriend/runtime.env >> "$TMP"
    fi
    if [ -f /opt/lightfriend/restore.env ]; then
        printf '\n' >> "$TMP"
        cat /opt/lightfriend/restore.env >> "$TMP"
    fi
    chmod 600 "$TMP"
    mv "$TMP" /opt/lightfriend/host-env
}

render_host_env
ENV_SIZE=$(stat -c%s /opt/lightfriend/host-env 2>/dev/null || echo "0")
log "host-env rendered ($${ENV_SIZE} bytes), starting listener on port 9000"

# No -u flag: bidirectional so OPEN reads file and sends to VSOCK client
socat VSOCK-LISTEN:9000,reuseaddr,fork OPEN:/opt/lightfriend/host-env 2>&1 | while read -r line; do
    log "socat: $line"
done

log "socat exited unexpectedly, restarting via systemd"
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

# VSOCK boot trace receiver (port 9007) - receives enclave boot log
cat > /opt/lightfriend/boot-trace-receiver.sh <<'SCRIPT'
#!/bin/bash
LOG_DIR="/opt/lightfriend/logs"
mkdir -p "$LOG_DIR"
while true; do
    TIMESTAMP=$(date -u +%Y%m%dT%H%M%SZ)
    DEST="$LOG_DIR/boot-trace-$${TIMESTAMP}.log"
    socat -T60 -u VSOCK-LISTEN:9007,reuseaddr CREATE:"$DEST" 2>/dev/null || true
    if [ -s "$DEST" ]; then
        echo "$(date -u): Boot trace received ($(stat -c%s "$DEST") bytes) -> $DEST"
        # Keep a symlink to latest
        ln -sf "$DEST" "$LOG_DIR/boot-trace-latest.log"
    else
        rm -f "$DEST"
    fi
done
SCRIPT
chmod +x /opt/lightfriend/boot-trace-receiver.sh

cat > /etc/systemd/system/vsock-boot-trace.service <<'BTEOF'
[Unit]
Description=VSOCK boot trace receiver for Nitro Enclave (port 9007)

[Service]
ExecStart=/opt/lightfriend/boot-trace-receiver.sh
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
BTEOF

# Seed HTTP server (port 9080) - serves seed SQL via HTTP for enclave download
cat > /etc/systemd/system/seed-http-server.service <<'SEEDHTTPEOF'
[Unit]
Description=HTTP seed file server for enclave (port 9080)
After=network.target

[Service]
ExecStart=/usr/bin/python3 -m http.server 9080 --directory /opt/lightfriend/seed/
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
SEEDHTTPEOF

# VSOCK bridge for seed HTTP (port 9080) - enclave fetches seed via HTTP
cat > /etc/systemd/system/vsock-seed-http.service <<'SEEDHTTPVSOCKEOF'
[Unit]
Description=VSOCK bridge to seed HTTP server for enclave (port 9080)
After=seed-http-server.service

[Service]
ExecStart=/usr/bin/socat VSOCK-LISTEN:9080,reuseaddr,fork TCP:127.0.0.1:9080
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
SEEDHTTPVSOCKEOF

# Backup upload receiver (port 9081) - accepts HTTP PUT from enclave
cat > /opt/lightfriend/backup-upload-server.py <<'PYEOF'
#!/usr/bin/env python3
"""Simple HTTP server that accepts PUT uploads to /upload/<filename>."""
import os
from http.server import HTTPServer, BaseHTTPRequestHandler

BACKUP_DIR = "/opt/lightfriend/backups"
os.makedirs(BACKUP_DIR, exist_ok=True)

class UploadHandler(BaseHTTPRequestHandler):
    def do_PUT(self):
        if not self.path.startswith("/upload/"):
            self.send_response(404)
            self.end_headers()
            return
        filename = os.path.basename(self.path[8:])
        if not filename:
            self.send_response(400)
            self.end_headers()
            return
        dest = os.path.join(BACKUP_DIR, filename)
        length = int(self.headers.get("Content-Length", 0))
        with open(dest, "wb") as f:
            remaining = length
            while remaining > 0:
                chunk = self.rfile.read(min(remaining, 65536))
                if not chunk:
                    break
                f.write(chunk)
                remaining -= len(chunk)
        size = os.path.getsize(dest)
        self.send_response(200)
        self.end_headers()
        self.wfile.write(f"OK {size} bytes\n".encode())
        print(f"Received {filename}: {size} bytes")

    def log_message(self, format, *args):
        pass  # silence per-request logs

HTTPServer(("0.0.0.0", 9081), UploadHandler).serve_forever()
PYEOF
chmod +x /opt/lightfriend/backup-upload-server.py

cat > /etc/systemd/system/backup-upload-server.service <<'UPLOADEOF'
[Unit]
Description=HTTP backup upload receiver for enclave (port 9081)
After=network.target

[Service]
ExecStart=/usr/bin/python3 /opt/lightfriend/backup-upload-server.py
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
UPLOADEOF

# VSOCK bridge for backup uploads (port 9081)
cat > /etc/systemd/system/vsock-backup-upload.service <<'UPLOADVSOCKEOF'
[Unit]
Description=VSOCK bridge to backup upload server for enclave (port 9081)
After=backup-upload-server.service

[Service]
ExecStart=/usr/bin/socat VSOCK-LISTEN:9081,reuseaddr,fork TCP:127.0.0.1:9081
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
UPLOADVSOCKEOF

# VSOCK bridge for cloudflared edge connections (port 7844)
# Enclave's cloudflared connects to Cloudflare edge via this bridge
cat > /opt/lightfriend/cloudflared-edge-bridge.sh <<'SCRIPT'
#!/bin/bash
LOG="/opt/lightfriend/logs/cloudflared-edge.log"
mkdir -p /opt/lightfriend/logs
echo "$(date -u): Starting cloudflared edge bridge on VSOCK:7844" >> "$LOG"
# nodelay on both sides: disables Nagle's algorithm for HTTP/2 multiplexed frames
socat -d -d VSOCK-LISTEN:7844,reuseaddr,fork TCP:region1.v2.argotunnel.com:7844,nodelay 2>>"$LOG"
SCRIPT
chmod +x /opt/lightfriend/cloudflared-edge-bridge.sh

cat > /etc/systemd/system/vsock-cloudflared-edge.service <<'CFEDGEEOF'
[Unit]
Description=VSOCK bridge for cloudflared edge connections (port 7844)

[Service]
ExecStart=/opt/lightfriend/cloudflared-edge-bridge.sh
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
CFEDGEEOF

# VSOCK bridge for DNS-over-TLS (port 8530 -> 1.1.1.1:853)
# Cloudflared uses DoT for SRV record lookups
cat > /etc/systemd/system/vsock-dot-bridge.service <<'DOTEOF'
[Unit]
Description=VSOCK bridge for DNS-over-TLS to 1.1.1.1:853

[Service]
ExecStart=/usr/bin/socat VSOCK-LISTEN:8530,reuseaddr,fork TCP:1.1.1.1:853
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
DOTEOF

# Enable and start all VSOCK services
systemctl daemon-reload
for svc in vsock-proxy-bridge vsock-config-server vsock-marlin-kms-bridge vsock-boot-trace seed-http-server vsock-seed-http vsock-cloudflared-edge vsock-dot-bridge backup-upload-server vsock-backup-upload; do
    systemctl enable "$svc"
    systemctl start "$svc" || echo "WARNING: $svc failed to start"
done

echo "VSOCK services configured: proxy:8001, config:9000, backup:9001, restore:9002, seed:9003, boot-trace:9007, seed-http:9080, cf-edge:7844, dot:8530, marlin-kms:9010"

# S3 deploy signal poller - polls S3 for export-request.json and copies locally.
# Needed because SSM send-command from the deploy runner stays Pending.
cat > /opt/lightfriend/s3-signal-poller.sh <<'SCRIPT'
#!/bin/bash
# Bridges the deploy pipeline (S3) with the enclave (local HTTP).
# Pipeline writes to S3, this poller copies locally, and when done
# uploads results back to S3. No SSM needed.
BUCKET=$(aws ssm get-parameter --name /lightfriend/s3-bucket --query Parameter.Value --output text 2>/dev/null || echo "")
[ -z "$BUCKET" ] && echo "No S3 bucket configured" && exit 0
echo "s3-signal-poller: bucket=$BUCKET"

LAST_TRIGGER=""
POLL_COUNT=0
while true; do
    POLL_COUNT=$((POLL_COUNT + 1))
    # Heartbeat every 60 iterations (~5 min)
    [ $((POLL_COUNT % 60)) -eq 0 ] && echo "$(date -u): heartbeat poll=$POLL_COUNT last_trigger=$${LAST_TRIGGER:-none}"

    # Check for export trigger in S3 (timeout prevents hanging on DNS/credential issues)
    S3_ERR=$(timeout 10 aws s3 cp "s3://$BUCKET/deploy/export-request.json" /tmp/export-request-check.json 2>&1)
    S3_RC=$?
    if [ $S3_RC -eq 0 ] && [ -s /tmp/export-request-check.json ]; then
        TRIGGER=$(cat /tmp/export-request-check.json)
        if [ "$TRIGGER" != "$LAST_TRIGGER" ]; then
            echo "$(date -u): New export request: $TRIGGER"
            LAST_TRIGGER="$TRIGGER"
            # Copy to seed dir for enclave's watcher
            cp /tmp/export-request-check.json /opt/lightfriend/seed/export-request.json
            rm -f /opt/lightfriend/backups/export-complete.json
            # Remove from S3 so we don't re-trigger
            aws s3 rm "s3://$BUCKET/deploy/export-request.json" 2>/dev/null || true
        fi
        rm -f /tmp/export-request-check.json
    elif [ $S3_RC -ne 0 ] && ! echo "$S3_ERR" | grep -q "404\|NoSuchKey\|Not Found"; then
        # Log unexpected S3 errors (not 404)
        echo "$(date -u): S3 poll error (rc=$S3_RC): $S3_ERR"
    fi

    # Check for completed export and upload to S3
    if [ -f /opt/lightfriend/backups/export-complete.json ] && [ -s /opt/lightfriend/backups/export-complete.json ]; then
        STATUS=$(cat /opt/lightfriend/backups/export-complete.json | python3 -c "import json,sys; print(json.load(sys.stdin).get('status',''))" 2>/dev/null || echo "")
        if [ "$STATUS" = "SUCCESS" ] || [ "$STATUS" = "FAILED" ]; then
            echo "$(date -u): Export $STATUS, uploading results to S3..."
            # Upload the backup .enc file to S3 if successful
            if [ "$STATUS" = "SUCCESS" ]; then
                BACKUP_FILE=$(ls -t /opt/lightfriend/backups/*.enc 2>/dev/null | head -1)
                if [ -n "$BACKUP_FILE" ]; then
                    BACKUP_KEY="backups/deploy/$(basename $BACKUP_FILE)"
                    timeout 120 aws s3 cp "$BACKUP_FILE" "s3://$BUCKET/$BACKUP_KEY" 2>/dev/null
                    BACKUP_SHA=$(sha256sum "$BACKUP_FILE" | awk '{print $1}')
                    BACKUP_SIZE=$(stat -c%s "$BACKUP_FILE")
                    # Add S3 key to completion JSON
                    python3 -c "
import json
with open('/opt/lightfriend/backups/export-complete.json') as f:
    data = json.load(f)
data['backup_key'] = '$BACKUP_KEY'
data['backup_sha256'] = '$BACKUP_SHA'
data['backup_size'] = $BACKUP_SIZE
print(json.dumps(data))
" > /tmp/export-complete-s3.json
                    timeout 30 aws s3 cp /tmp/export-complete-s3.json "s3://$BUCKET/deploy/export-complete.json" 2>/dev/null
                    echo "$(date -u): Backup uploaded: s3://$BUCKET/$BACKUP_KEY ($BACKUP_SIZE bytes)"
                fi
            else
                # Upload failure status as-is
                aws s3 cp /opt/lightfriend/backups/export-complete.json "s3://$BUCKET/deploy/export-complete.json" 2>/dev/null
            fi
            # Clean up local files
            rm -f /opt/lightfriend/backups/export-complete.json
            rm -f /opt/lightfriend/seed/export-request.json
        fi
    fi

    sleep 5
done
SCRIPT
chmod +x /opt/lightfriend/s3-signal-poller.sh

cat > /etc/systemd/system/s3-signal-poller.service <<'POLLEREOF'
[Unit]
Description=S3 deploy signal poller
After=network-online.target

[Service]
ExecStart=/opt/lightfriend/s3-signal-poller.sh
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
POLLEREOF

systemctl daemon-reload
systemctl enable s3-signal-poller
systemctl start s3-signal-poller || echo "WARNING: s3-signal-poller failed to start"

# ── Enclave launch script ───────────────────────────────────────────────────

cat > /opt/lightfriend/launch-enclave.sh <<'SCRIPT'
#!/bin/bash
set -e
EIF_PATH="/opt/lightfriend/lightfriend.eif"
VERIFY="/opt/lightfriend/verify-result.json"
VSOCK_SVCS="vsock-proxy-bridge vsock-config-server vsock-marlin-kms-bridge vsock-boot-trace vsock-seed-http vsock-cloudflared-edge vsock-dot-bridge vsock-backup-upload"

echo "[launch] EIF: $(ls -la $EIF_PATH 2>&1)"
echo "[launch] .env: $(ls -la /opt/lightfriend/.env 2>&1)"
echo "[launch] Hugepages: $(cat /proc/meminfo | grep Hugetlb)"
echo "[launch] Allocator status: $(systemctl is-active nitro-enclaves-allocator 2>&1)"
echo "[launch] Existing enclaves: $(nitro-cli describe-enclaves 2>&1)"

[ -f /opt/lightfriend/.env ] || { echo "FATAL: /opt/lightfriend/.env not found"; exit 1; }
[ -f "$EIF_PATH" ] || { echo "FATAL: EIF not found at $EIF_PATH"; exit 1; }

EXISTING=$(nitro-cli describe-enclaves | jq -r '.[0].EnclaveID // empty' 2>/dev/null)
[ -n "$EXISTING" ] && echo "[launch] Terminating $EXISTING" && nitro-cli terminate-enclave --enclave-id "$EXISTING" && sleep 2

echo "[launch] Stopping VSOCK services..."
for svc in $VSOCK_SVCS; do
    systemctl stop "$svc" 2>/dev/null && echo "[launch] Stopped $svc" || echo "[launch] $svc not running"
done
pkill -f 'socat.*VSOCK' 2>/dev/null || true
echo "[launch] Sleeping 5s..."
sleep 5

echo "[launch] Checking for remaining VSOCK listeners..."
ss -tlnp 2>/dev/null | grep VSOCK || echo "[launch] No VSOCK listeners"

rm -f "$VERIFY"

echo "[launch] Running: nitro-cli run-enclave --eif-path $EIF_PATH --memory 8192 --cpu-count 4 --enclave-cid 16"
nitro-cli run-enclave --eif-path "$EIF_PATH" --memory 8192 --cpu-count 4 --enclave-cid 16
echo "[launch] nitro-cli exit code: $?"

# Restart VSOCK services after launch
echo "Restarting VSOCK services..."
for svc in $VSOCK_SVCS; do systemctl start "$svc" 2>/dev/null || echo "WARNING: $svc failed"; done

nitro-cli describe-enclaves
SCRIPT
chmod +x /opt/lightfriend/launch-enclave.sh

# ── Host diagnostic script (run after launch to see everything) ──────────────

cat > /opt/lightfriend/diagnose.sh <<'SCRIPT'
#!/bin/bash
echo "========================================"
echo "  Lightfriend Host Diagnostics"
echo "  $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "========================================"

echo ""
echo "--- 1. Enclave State ---"
nitro-cli describe-enclaves 2>&1 | head -20

echo ""
echo "--- 2. VSOCK Services ---"
for svc in squid vsock-proxy-bridge vsock-config-server vsock-boot-trace vsock-marlin-kms-bridge; do
    STATUS=$(systemctl is-active "$svc" 2>/dev/null || echo "not-found")
    printf "  %-30s %s\n" "$svc" "$STATUS"
done

echo ""
echo "--- 3. Key Files ---"
for f in /opt/lightfriend/.env /opt/lightfriend/host-env /opt/lightfriend/lightfriend.eif /opt/lightfriend/seed/lightfriend_db.sql; do
    if [ -f "$f" ]; then
        SIZE=$(stat -c%s "$f" 2>/dev/null || echo "?")
        printf "  %-55s %s bytes\n" "$f" "$SIZE"
    else
        printf "  %-55s MISSING\n" "$f"
    fi
done

echo ""
echo "--- 4. Verify Result ---"
if [ -f /opt/lightfriend/verify-result.json ]; then
    cat /opt/lightfriend/verify-result.json
else
    echo "  No verify-result.json yet"
fi

echo ""
echo "--- 5. Boot Trace (last 80 lines) ---"
LATEST="/opt/lightfriend/logs/boot-trace-latest.log"
if [ -f "$LATEST" ]; then
    TRACE_SIZE=$(stat -c%s "$LATEST" 2>/dev/null || echo "?")
    echo "  File: $(readlink -f "$LATEST") ($TRACE_SIZE bytes)"
    echo "  ---"
    tail -80 "$LATEST"
else
    echo "  No boot trace received yet"
    echo "  Check: is vsock-boot-trace service running?"
fi

echo ""
echo "--- 6. Nitro Enclave Logs (last 30 lines) ---"
NITRO_LOG=$(ls -t /var/log/nitro_enclaves/nitro_enclaves.log 2>/dev/null | head -1)
if [ -n "$NITRO_LOG" ]; then
    tail -30 "$NITRO_LOG"
else
    echo "  No nitro enclave logs found"
fi

echo ""
echo "--- 7. Squid Proxy Test ---"
if curl -sf --max-time 5 -x http://127.0.0.1:3128 https://api.cloudflare.com/cdn-cgi/trace > /tmp/diag-proxy-test 2>&1; then
    echo "  OK: $(head -1 /tmp/diag-proxy-test)"
else
    echo "  FAILED: proxy not reachable or Cloudflare unreachable"
fi
rm -f /tmp/diag-proxy-test

echo ""
echo "--- 8. Public Endpoint ---"
HTTP_CODE=$(curl -sf --max-time 10 -o /dev/null -w '%%{http_code}' https://enclave.lightfriend.ai 2>/dev/null || echo "timeout")
echo "  https://enclave.lightfriend.ai -> HTTP $HTTP_CODE"

echo ""
echo "========================================"
SCRIPT
chmod +x /opt/lightfriend/diagnose.sh

# ── S3 backup upload/download scripts ───────────────────────────────────────

cat > /opt/lightfriend/upload-backup.sh <<'SCRIPT'
#!/bin/bash
set -euo pipefail
BACKUP=$(ls -t /opt/lightfriend/backups/*.tar.gz.enc 2>/dev/null | head -1)
[ -z "$BACKUP" ] && { echo "No backup found"; exit 1; }
BUCKET=$(grep S3_BACKUP_BUCKET /opt/lightfriend/.env | cut -d= -f2)
[ -z "$BUCKET" ] && { echo "S3_BACKUP_BUCKET not set in .env"; exit 1; }
BACKUP_TIER="$${BACKUP_TIER:-hourly}"

LOCAL_SIZE=$(stat -c%s "$BACKUP" 2>/dev/null || stat -f%z "$BACKUP" 2>/dev/null)
LOCAL_SHA=$(sha256sum "$BACKUP" | awk '{print $1}')
S3_KEY="backups/$${BACKUP_TIER}/$(basename $BACKUP)"
RESULT_FILE="/opt/lightfriend/last-upload.json"

# Upload with checksum verification (S3 validates Content-MD5 on receipt)
aws s3 cp "$BACKUP" "s3://$${BUCKET}/$${S3_KEY}" --sse AES256

# Verify: S3 object exists and size matches local file
S3_SIZE=$(aws s3api head-object --bucket "$${BUCKET}" --key "$${S3_KEY}" --query ContentLength --output text 2>/dev/null || echo "0")
if [ "$${LOCAL_SIZE}" != "$${S3_SIZE}" ]; then
    echo "FATAL: S3 size mismatch - local=$${LOCAL_SIZE} s3=$${S3_SIZE}"
    exit 1
fi

cat > "$RESULT_FILE" <<EOF
{"backup_key":"$${S3_KEY}","size_bytes":$${LOCAL_SIZE},"sha256":"$${LOCAL_SHA}","local_path":"$${BACKUP}"}
EOF

echo "UPLOAD_RESULT $(cat "$RESULT_FILE")"
echo "Uploaded and verified: s3://$${BUCKET}/$${S3_KEY} ($${S3_SIZE} bytes)"
SCRIPT
chmod +x /opt/lightfriend/upload-backup.sh

cat > /opt/lightfriend/download-eif.sh <<'SCRIPT'
#!/bin/bash
set -euo pipefail
EIF_KEY="$${1:-}"
EXPECTED_SHA="$${2:-}"
BUCKET=$(grep S3_BACKUP_BUCKET /opt/lightfriend/.env | cut -d= -f2)
[ -n "$BUCKET" ] || { echo "S3_BACKUP_BUCKET not set in .env"; exit 1; }
[ -n "$EIF_KEY" ] || { echo "Usage: download-eif.sh <s3-key> <sha256>"; exit 1; }
[ -n "$EXPECTED_SHA" ] || { echo "Usage: download-eif.sh <s3-key> <sha256>"; exit 1; }

DEST="/opt/lightfriend/lightfriend.eif"
TMP_DEST=$(mktemp /opt/lightfriend/lightfriend.eif.XXXXXX)
trap 'rm -f "$TMP_DEST"' EXIT

aws s3 cp "s3://$BUCKET/$EIF_KEY" "$TMP_DEST"
ACTUAL_SHA=$(sha256sum "$TMP_DEST" | awk '{print $1}')
if [ "$ACTUAL_SHA" != "$EXPECTED_SHA" ]; then
    echo "FATAL: EIF sha256 mismatch - expected=$EXPECTED_SHA actual=$ACTUAL_SHA"
    exit 1
fi

mv "$TMP_DEST" "$DEST"
chmod 600 "$DEST"
echo "$DEST"
SCRIPT
chmod +x /opt/lightfriend/download-eif.sh

cat > /opt/lightfriend/download-backup.sh <<'SCRIPT'
#!/bin/bash
set -euo pipefail
BACKUP_KEY="$${1:-}"
BUCKET=$(grep S3_BACKUP_BUCKET /opt/lightfriend/.env | cut -d= -f2)
[ -z "$BUCKET" ] && { echo "S3_BACKUP_BUCKET not set in .env"; exit 1; }
[ -z "$BACKUP_KEY" ] && { echo "Usage: download-backup.sh <s3-key>"; exit 1; }
DEST="/opt/lightfriend/restore/$(basename "$${BACKUP_KEY}")"
mkdir -p /opt/lightfriend/restore

# Get expected size from S3 before download
S3_SIZE=$(aws s3api head-object --bucket "$${BUCKET}" --key "$${BACKUP_KEY}" --query ContentLength --output text)
echo "Expected size: $${S3_SIZE} bytes"

aws s3 cp "s3://$${BUCKET}/$${BACKUP_KEY}" "$DEST"

# Verify downloaded file matches S3 size
LOCAL_SIZE=$(stat -c%s "$DEST" 2>/dev/null || stat -f%z "$DEST" 2>/dev/null)
if [ "$${LOCAL_SIZE}" != "$${S3_SIZE}" ]; then
    echo "FATAL: Download size mismatch - expected=$${S3_SIZE} got=$${LOCAL_SIZE}"
    rm -f "$DEST"
    exit 1
fi
echo "Downloaded and verified: $${BACKUP_KEY} ($${LOCAL_SIZE} bytes)"
echo "$DEST"
SCRIPT
chmod +x /opt/lightfriend/download-backup.sh

cat > /opt/lightfriend/clear-restore-state.sh <<'SCRIPT'
#!/bin/bash
set -euo pipefail
rm -f /opt/lightfriend/restore.env /opt/lightfriend/restore/current-path
SCRIPT
chmod +x /opt/lightfriend/clear-restore-state.sh

cat > /opt/lightfriend/restore-enclave.sh <<'SCRIPT'
#!/bin/bash
# Every step logged to RLOG for diagnostics
RLOG="/tmp/restore-enclave-debug.log"
exec > >(tee -a "$RLOG") 2>&1
echo "=== restore-enclave.sh started at $(date -u) ==="
echo "Args: $*"
echo "PWD: $(pwd)"
echo "Uptime: $(uptime)"

set -euo pipefail
BACKUP_KEY="$${1:-}"
DEPLOY_ID="$${2:-manual}"
RESTORE_TYPE="$${3:-full}"
VERIFY="/opt/lightfriend/verify-result.json"
ARTIFACT=""
SUCCESS=false

echo "[CHECK] BACKUP_KEY=$BACKUP_KEY"
[ -n "$BACKUP_KEY" ] || { echo "FATAL: empty BACKUP_KEY"; exit 1; }

echo "[CHECK] .env exists: $(ls -la /opt/lightfriend/.env 2>&1)"
[ -f /opt/lightfriend/.env ] || { echo "FATAL: /opt/lightfriend/.env not found"; exit 1; }

echo "[CHECK] S3_BACKUP_BUCKET=$(grep S3_BACKUP_BUCKET /opt/lightfriend/.env | cut -d= -f2)"
echo "[CHECK] EIF exists: $(ls -la /opt/lightfriend/lightfriend.eif 2>&1)"
echo "[CHECK] Hugepages: $(cat /proc/meminfo | grep Hugetlb)"
echo "[CHECK] Allocator: $(systemctl is-active nitro-enclaves-allocator 2>&1)"
echo "[CHECK] Enclave support: $(nitro-cli describe-enclaves 2>&1 | head -3)"
echo "[CHECK] Free memory: $(free -h | head -2)"
echo "[CHECK] Running services: $(systemctl list-units --type=service --state=running 2>&1 | grep -c 'running') services"

cleanup() {
    echo "[CLEANUP] Running cleanup (SUCCESS=$SUCCESS, ARTIFACT=$ARTIFACT)"
    /opt/lightfriend/clear-restore-state.sh
    if [ -n "$ARTIFACT" ] && [ -f "$ARTIFACT" ]; then
        if [ "$SUCCESS" = "true" ]; then
            rm -f "$ARTIFACT"
        else
            FAILED_DIR="/opt/lightfriend/restore/failed"
            mkdir -p "$FAILED_DIR"
            mv "$ARTIFACT" "$FAILED_DIR/$${DEPLOY_ID}-$(basename "$ARTIFACT")"
        fi
    fi
    echo "[CLEANUP] Done"
}
trap cleanup EXIT

echo "[STEP 1] Downloading backup..."
ARTIFACT=$(/opt/lightfriend/download-backup.sh "$BACKUP_KEY" | tail -1)
echo "[STEP 1] ARTIFACT=$ARTIFACT"
echo "[STEP 1] File check: $(ls -la "$ARTIFACT" 2>&1)"
[ -f "$ARTIFACT" ] || { echo "FATAL: Restore artifact not downloaded"; exit 1; }

cat > /opt/lightfriend/restore.env <<EOF
RESTORE_MODE=$${RESTORE_TYPE}
RESTORE_BACKUP_KEY=$${BACKUP_KEY}
RESTORE_DEPLOY_ID=$${DEPLOY_ID}
EOF
printf '%s\n' "$ARTIFACT" > /opt/lightfriend/restore/current-path

echo "[STEP 2] Copying backup to seed dir..."
SEED_BACKUP="/opt/lightfriend/seed/restore-backup.tar.gz.enc"
cp "$ARTIFACT" "$SEED_BACKUP"
echo "[STEP 2] Seed backup: $(ls -la "$SEED_BACKUP" 2>&1)"

rm -f "$VERIFY"
VERIFY_SRC="/opt/lightfriend/backups/verify-result.json"
rm -f "$VERIFY_SRC"

echo "[STEP 3] Launching enclave..."
/opt/lightfriend/launch-enclave.sh
echo "[STEP 3] Launch exit code: $?"
echo "[STEP 3] Enclave state: $(nitro-cli describe-enclaves 2>&1 | jq -r '.[0].State // "none"')"

echo "[STEP 4] Polling for verify result..."
for i in $(seq 1 180); do
    if [ -s "$VERIFY_SRC" ]; then
        cp "$VERIFY_SRC" "$VERIFY"
        rm -f "$VERIFY_SRC"
        echo "[STEP 4] Verify result received after $((i * 5))s"
        break
    fi
    sleep 5
done

echo "[STEP 5] Verify file check: $(ls -la "$VERIFY" 2>&1)"
[ -s "$VERIFY" ] || { echo "FATAL: Verify result empty after polling"; exit 1; }
SUCCESS=true
echo "=== restore-enclave.sh completed successfully ==="
SCRIPT
chmod +x /opt/lightfriend/restore-enclave.sh

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

cat > /opt/lightfriend/pre-deploy.sh <<'SCRIPT'
#!/bin/bash
set -e
LOCKFILE="/tmp/lightfriend-backup.lock"
exec 200>"$LOCKFILE"
flock -n 200 || { echo "FATAL: Another backup/export is already running"; exit 1; }
echo "=== Pre-deploy: export + upload (old instance stays live) ==="
/opt/lightfriend/trigger-export.sh
BACKUP_TIER=hourly /opt/lightfriend/upload-backup.sh
/opt/lightfriend/upload-env.sh
echo "=== Pre-deploy complete ==="
SCRIPT
chmod +x /opt/lightfriend/pre-deploy.sh

# ── Scheduled hourly full snapshot backup ────────────────────────────────
# Runs the same full export path used during deploys, but without enabling
# maintenance mode. Each backing store snapshot is taken live inside export.sh,
# then the encrypted archive is uploaded and verified against S3 by size/SHA.
# Worst-case data loss target: 1 hour.

cat > /opt/lightfriend/promote-backup.sh <<'SCRIPT'
#!/bin/bash
set -euo pipefail
SOURCE_KEY="$${1:-}"
BUCKET=$(grep S3_BACKUP_BUCKET /opt/lightfriend/.env | cut -d= -f2)
[ -n "$SOURCE_KEY" ] || { echo "Usage: promote-backup.sh <source-key>"; exit 1; }
[ -n "$BUCKET" ] || { echo "S3_BACKUP_BUCKET not set in .env"; exit 1; }

SOURCE_SIZE=$(aws s3api head-object --bucket "$BUCKET" --key "$SOURCE_KEY" --query ContentLength --output text)
BASENAME=$(basename "$SOURCE_KEY")

copy_and_verify() {
    DEST_KEY="$1"
    aws s3 cp "s3://$BUCKET/$SOURCE_KEY" "s3://$BUCKET/$DEST_KEY" --sse AES256
    DEST_SIZE=$(aws s3api head-object --bucket "$BUCKET" --key "$DEST_KEY" --query ContentLength --output text)
    if [ "$SOURCE_SIZE" != "$DEST_SIZE" ]; then
        echo "FATAL: Promotion size mismatch for $DEST_KEY"
        exit 1
    fi
    echo "Promoted and verified: $DEST_KEY"
}

UTC_HOUR=$(date -u +%H)
UTC_DOM=$(date -u +%d)
UTC_DOW=$(date -u +%u)

if [ "$UTC_HOUR" = "00" ]; then
    copy_and_verify "backups/daily/$BASENAME"
fi

if [ "$UTC_HOUR" = "00" ] && [ "$UTC_DOW" = "7" ]; then
    copy_and_verify "backups/weekly/$BASENAME"
fi

if [ "$UTC_HOUR" = "00" ] && [ "$UTC_DOM" = "01" ]; then
    copy_and_verify "backups/monthly/$BASENAME"
fi
SCRIPT
chmod +x /opt/lightfriend/promote-backup.sh

cat > /opt/lightfriend/scheduled-backup.sh <<'SCRIPT'
#!/bin/bash
set -euo pipefail
LOCKFILE="/tmp/lightfriend-backup.lock"
exec 200>"$LOCKFILE"
flock -n 200 || { echo "Skipping: another backup/deploy is running"; exit 0; }
LOG="/var/log/lightfriend-backup.log"
TIMESTAMP=$(date -u +%Y-%m-%dT%H:%M:%SZ)
echo "=== Scheduled full snapshot backup: $${TIMESTAMP} ===" >> "$LOG"

BUCKET=$(grep S3_BACKUP_BUCKET /opt/lightfriend/.env | cut -d= -f2)

# Step 1: Trigger full export inside enclave
/opt/lightfriend/trigger-export.sh >> "$LOG" 2>&1 || {
    echo "FAILED: full export trigger failed at $${TIMESTAMP}" >> "$LOG"
    # Write failure marker to S3 so deploys can detect stale backups
    [ -n "$${BUCKET:-}" ] && echo "{\"last_failure\": \"$${TIMESTAMP}\"}" | \
        aws s3 cp - "s3://$${BUCKET}/backups/backup-health.json" 2>/dev/null || true
    exit 1
}

# Step 2: Upload to S3 hourly tier (upload-backup.sh verifies size/SHA after upload)
BACKUP_TIER=hourly /opt/lightfriend/upload-backup.sh >> "$LOG" 2>&1 || {
    echo "FAILED: S3 upload/verify failed at $${TIMESTAMP}" >> "$LOG"
    [ -n "$${BUCKET:-}" ] && echo "{\"last_failure\": \"$${TIMESTAMP}\"}" | \
        aws s3 cp - "s3://$${BUCKET}/backups/backup-health.json" 2>/dev/null || true
    exit 1
}

RESULT_JSON=$(cat /opt/lightfriend/last-upload.json 2>/dev/null || echo "")
BACKUP_KEY=$(echo "$RESULT_JSON" | jq -r '.backup_key // empty')
BACKUP_SHA=$(echo "$RESULT_JSON" | jq -r '.sha256 // empty')
if [ -z "$BACKUP_KEY" ] || [ -z "$BACKUP_SHA" ]; then
    echo "FAILED: upload result missing backup_key or sha256 at $${TIMESTAMP}" >> "$LOG"
    [ -n "$${BUCKET:-}" ] && echo "{\"last_failure\": \"$${TIMESTAMP}\"}" | \
        aws s3 cp - "s3://$${BUCKET}/backups/backup-health.json" 2>/dev/null || true
    exit 1
fi

# Step 3: Independently re-download and validate the exact encrypted artifact
TMP_BACKUP=$(mktemp /tmp/lightfriend-scheduled-backup.XXXXXX.enc)
trap 'rm -f "$TMP_BACKUP"' EXIT
aws s3 cp "s3://$${BUCKET}/$${BACKUP_KEY}" "$TMP_BACKUP" >> "$LOG" 2>&1 || {
    echo "FAILED: could not re-download uploaded backup at $${TIMESTAMP}" >> "$LOG"
    [ -n "$${BUCKET:-}" ] && echo "{\"last_failure\": \"$${TIMESTAMP}\"}" | \
        aws s3 cp - "s3://$${BUCKET}/backups/backup-health.json" 2>/dev/null || true
    exit 1
}
ACTUAL_SHA=$(sha256sum "$TMP_BACKUP" | awk '{print $1}')
if [ "$ACTUAL_SHA" != "$BACKUP_SHA" ]; then
    echo "FAILED: uploaded backup sha mismatch at $${TIMESTAMP}" >> "$LOG"
    [ -n "$${BUCKET:-}" ] && echo "{\"last_failure\": \"$${TIMESTAMP}\"}" | \
        aws s3 cp - "s3://$${BUCKET}/backups/backup-health.json" 2>/dev/null || true
    exit 1
fi

# Step 4: Promote selected hourly snapshots into daily/weekly/monthly tiers
/opt/lightfriend/promote-backup.sh "$BACKUP_KEY" >> "$LOG" 2>&1 || {
    echo "FAILED: backup promotion failed at $${TIMESTAMP}" >> "$LOG"
    [ -n "$${BUCKET:-}" ] && echo "{\"last_failure\": \"$${TIMESTAMP}\"}" | \
        aws s3 cp - "s3://$${BUCKET}/backups/backup-health.json" 2>/dev/null || true
    exit 1
}

# Step 5: Clean up old local backups (keep last 3)
cd /opt/lightfriend/backups
ls -t *.tar.gz.enc 2>/dev/null | tail -n +4 | xargs rm -f 2>/dev/null || true

# Step 6: Write success marker to S3
[ -n "$${BUCKET:-}" ] && echo "{\"last_success\": \"$${TIMESTAMP}\", \"type\": \"full_snapshot\", \"tier\": \"hourly\", \"backup_key\": \"$${BACKUP_KEY}\"}" | \
    aws s3 cp - "s3://$${BUCKET}/backups/backup-health.json" 2>/dev/null || true

echo "=== Full snapshot backup verified and complete: $${TIMESTAMP} ===" >> "$LOG"
SCRIPT
chmod +x /opt/lightfriend/scheduled-backup.sh

echo "0 * * * * root /opt/lightfriend/scheduled-backup.sh" > /etc/cron.d/lightfriend-backup
chmod 644 /etc/cron.d/lightfriend-backup

chown -R ec2-user:ec2-user /opt/lightfriend

echo "Lightfriend enclave host setup complete!"

# ── Auto-bootstrap (two-phase for blue-green deploys) ─────────────────────
# Phase 1: Pre-warm by downloading the CI-built EIF while old instance still serves
# Phase 2: Wait for an instance-scoped restore manifest, then restore + verify

BUCKET=$(aws ssm get-parameter --name /lightfriend/s3-bucket --query Parameter.Value --output text 2>/dev/null || echo "")

if [ -n "$BUCKET" ] && aws s3 ls "s3://$BUCKET/config/.env" 2>/dev/null; then
    IMDS_TOKEN=$(curl -sf -X PUT http://169.254.169.254/latest/api/token -H "X-aws-ec2-metadata-token-ttl-seconds: 300")
    INSTANCE_ID=$(curl -sf -H "X-aws-ec2-metadata-token: $IMDS_TOKEN" http://169.254.169.254/latest/meta-data/instance-id)
    VERIFY="/opt/lightfriend/verify-result.json"

    # ── Phase 1: Pre-warm ─────────────────────────────────────────────────
    echo "=== Phase 1: Pre-warming (download CI-built EIF) ==="

    # Download .env (needed for launch-enclave.sh .env check)
    aws s3 cp "s3://$BUCKET/config/.env" /opt/lightfriend/.env
    chmod 600 /opt/lightfriend/.env

    EIF_MANIFEST_KEY="deploy/eif-$INSTANCE_ID.json"
    EIF_MANIFEST=""
    echo "Polling s3://$BUCKET/$EIF_MANIFEST_KEY for CI-built EIF metadata..."
    for i in $(seq 1 180); do
        EIF_MANIFEST=$(aws s3 cp "s3://$BUCKET/$EIF_MANIFEST_KEY" - 2>/dev/null || echo "")
        if [ -n "$EIF_MANIFEST" ]; then
            echo "EIF manifest received"
            break
        fi
        sleep 10
    done

    if [ -z "$EIF_MANIFEST" ]; then
        echo "{\"status\": \"EIF_MANIFEST_TIMEOUT\", \"instance_id\": \"$INSTANCE_ID\"}" | \
            aws s3 cp - "s3://$BUCKET/deploy/pre-warm-$INSTANCE_ID.json"
        exit 1
    fi

    EIF_KEY=$(echo "$EIF_MANIFEST" | jq -r '.eif_key // empty')
    EIF_SHA256=$(echo "$EIF_MANIFEST" | jq -r '.eif_sha256 // empty')
    PCR0=$(echo "$EIF_MANIFEST" | jq -r '.pcr0 // empty')
    PCR1=$(echo "$EIF_MANIFEST" | jq -r '.pcr1 // empty')
    PCR2=$(echo "$EIF_MANIFEST" | jq -r '.pcr2 // empty')
    COMMIT_SHA=$(echo "$EIF_MANIFEST" | jq -r '.commit_sha // empty')
    IMAGE_REF=$(echo "$EIF_MANIFEST" | jq -r '.image_ref // empty')
    WORKFLOW_RUN_ID=$(echo "$EIF_MANIFEST" | jq -r '.workflow_run_id // empty')
    PUBLIC_METADATA_URL=$(echo "$EIF_MANIFEST" | jq -r '.public_metadata_url // empty')

    if [ -z "$EIF_KEY" ] || [ -z "$EIF_SHA256" ] || [ -z "$PCR0" ]; then
        echo "{\"status\": \"INVALID_EIF_MANIFEST\", \"instance_id\": \"$INSTANCE_ID\"}" | \
            aws s3 cp - "s3://$BUCKET/deploy/pre-warm-$INSTANCE_ID.json"
        exit 1
    fi

    if ! /opt/lightfriend/download-eif.sh "$EIF_KEY" "$EIF_SHA256" 2>&1 | tee /tmp/eif-download.log; then
        DOWNLOAD_ERR=$(tail -5 /tmp/eif-download.log | tr '\n' ' ' | head -c 500)
        echo "{\"status\": \"EIF_DOWNLOAD_FAILED\", \"instance_id\": \"$INSTANCE_ID\", \"error\": \"$DOWNLOAD_ERR\"}" | \
            aws s3 cp - "s3://$BUCKET/deploy/pre-warm-$INSTANCE_ID.json"
        exit 1
    fi

    cat > /opt/lightfriend/runtime.env <<EOF
CURRENT_COMMIT_SHA=$COMMIT_SHA
CURRENT_WORKFLOW_RUN_ID=$WORKFLOW_RUN_ID
CURRENT_IMAGE_REF=$IMAGE_REF
CURRENT_BUILD_METADATA_URL=$PUBLIC_METADATA_URL
CURRENT_EIF_SHA256=$EIF_SHA256
CURRENT_PCR0=$PCR0
CURRENT_PCR1=$PCR1
CURRENT_PCR2=$PCR2
EOF
    chmod 600 /opt/lightfriend/runtime.env

    # Signal pre-warm complete
    echo "{\"status\": \"PRE_WARM_COMPLETE\", \"instance_id\": \"$INSTANCE_ID\", \"eif_key\": \"$EIF_KEY\", \"eif_sha256\": \"$EIF_SHA256\", \"pcr0\": \"$PCR0\", \"pcr1\": \"$PCR1\", \"pcr2\": \"$PCR2\", \"timestamp\": \"$(date -u +%Y-%m-%dT%H:%M:%SZ)\"}" | \
        aws s3 cp - "s3://$BUCKET/deploy/pre-warm-$INSTANCE_ID.json"
    echo "=== Phase 1 complete: EIF downloaded and verified, waiting for restore manifest ==="

    # ── Phase 2: Wait for restore manifest, then restore + launch ─────────
    RESTORE_MANIFEST_KEY="deploy/restore-$INSTANCE_ID.json"
    echo "Polling s3://$BUCKET/$RESTORE_MANIFEST_KEY..."
    RESTORE_MANIFEST=""
    for i in $(seq 1 180); do
        RESTORE_MANIFEST=$(aws s3 cp "s3://$BUCKET/$RESTORE_MANIFEST_KEY" - 2>/dev/null || echo "")
        if [ -n "$RESTORE_MANIFEST" ]; then
            echo "Restore manifest received"
            break
        fi
        sleep 10
    done

    if [ -z "$RESTORE_MANIFEST" ]; then
        echo "{\"status\": \"RESTORE_MANIFEST_TIMEOUT\", \"instance_id\": \"$INSTANCE_ID\"}" | \
            aws s3 cp - "s3://$BUCKET/deploy/verify-$INSTANCE_ID.json"
        echo "FATAL: No restore manifest after 30 minutes"
        exit 1
    fi

    BACKUP_KEY=$(echo "$RESTORE_MANIFEST" | jq -r '.backup_key // empty')
    RESTORE_TYPE=$(echo "$RESTORE_MANIFEST" | jq -r '.restore_type // "full"')
    DEPLOY_ID=$(echo "$RESTORE_MANIFEST" | jq -r '.deploy_id // "unknown"')

    if [ -z "$BACKUP_KEY" ]; then
        echo "{\"status\": \"INVALID_RESTORE_MANIFEST\", \"instance_id\": \"$INSTANCE_ID\"}" | \
            aws s3 cp - "s3://$BUCKET/deploy/verify-$INSTANCE_ID.json"
        echo "FATAL: Restore manifest missing backup_key"
        exit 1
    fi

    # Re-download .env (CI may have uploaded a fresh one from old instance)
    aws s3 cp "s3://$BUCKET/config/.env" /opt/lightfriend/.env
    chmod 600 /opt/lightfriend/.env

    # Restore enclave from the exact backup artifact in the manifest
    echo "Restoring from $BACKUP_KEY..."
    set -o pipefail
    if ! /opt/lightfriend/restore-enclave.sh "$BACKUP_KEY" "$DEPLOY_ID" "$RESTORE_TYPE" 2>&1 | tee /tmp/launch.log; then
        RESTORE_LOG=$(cat /tmp/restore-enclave-debug.log 2>/dev/null | tail -50 | tr '\n' '|' | sed 's/"/\\"/g' | head -c 1500)
        echo "{\"status\": \"RESTORE_FAILED\", \"instance_id\": \"$INSTANCE_ID\", \"log\": \"$RESTORE_LOG\"}" | \
            aws s3 cp - "s3://$BUCKET/deploy/verify-$INSTANCE_ID.json"
        exit 1
    fi
    set +o pipefail

    if [ -s "$VERIFY" ]; then
        aws s3 cp "$VERIFY" "s3://$BUCKET/deploy/verify-$INSTANCE_ID.json"
        echo "Verify result uploaded"
        cat "$VERIFY"
    else
        ENCLAVE_STATUS=$(nitro-cli describe-enclaves 2>/dev/null | jq -r '.[0].State // "unknown"')
        RESTORE_LOG=$(cat /tmp/restore-enclave-debug.log 2>/dev/null | tail -50 | tr '\n' '|' | sed 's/"/\\"/g' | head -c 1500)
        echo "{\"status\": \"TIMEOUT\", \"instance_id\": \"$INSTANCE_ID\", \"enclave_state\": \"$ENCLAVE_STATUS\", \"log\": \"$RESTORE_LOG\"}" | \
            aws s3 cp - "s3://$BUCKET/deploy/verify-$INSTANCE_ID.json"
        echo "WARNING: No verify result received (enclave state: $ENCLAVE_STATUS)"
    fi

    echo "=== Auto-bootstrap complete ==="
else
    echo "No .env in S3 - skipping auto-bootstrap (manual first-time setup required)"
fi
