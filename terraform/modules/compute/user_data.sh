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

# ── Host log/artifact maintenance ────────────────────────────────────────────

cat > /opt/lightfriend/host-log-maintenance.sh <<'SCRIPT'
#!/bin/bash
set -uo pipefail

LOG_DIR="/opt/lightfriend/logs"
RESTORE_FAILED_DIR="/opt/lightfriend/restore/failed"
mkdir -p "$LOG_DIR" "$RESTORE_FAILED_DIR"

cap_file() {
    local file="$1"
    local max_bytes="$2"
    local keep_bytes="$3"
    [ -f "$file" ] || return 0
    local size
    size=$(stat -c%s "$file" 2>/dev/null || echo 0)
    if [ "$size" -gt "$max_bytes" ]; then
        tail -c "$keep_bytes" "$file" > "$file.tmp" 2>/dev/null && cat "$file.tmp" > "$file"
        rm -f "$file.tmp" 2>/dev/null || true
    fi
}

for f in \
    "$LOG_DIR/gvproxy.log" \
    "$LOG_DIR/gvproxy-err.log" \
    "$LOG_DIR/scheduled-backup.log" \
    "$LOG_DIR/cloudflared-edge-stdout.log" \
    "$LOG_DIR/cloudflared-edge-stderr.log" \
    "$LOG_DIR/telegram-proxy-bridge.log" \
    "$LOG_DIR/config-server.log" \
    "$LOG_DIR/dot-bridge.log"; do
    cap_file "$f" 5242880 1048576
done

cap_file /tmp/restore-enclave-debug.log 5242880 1048576
cap_file /tmp/launch.log 5242880 1048576
cap_file /tmp/eif-download.log 2097152 524288

# Boot traces are useful during failed deploys, but each launch creates a new file.
find "$LOG_DIR" -maxdepth 1 -type f -name 'boot-trace-*.log' -mtime +7 -delete 2>/dev/null || true
if [ -d "$LOG_DIR" ]; then
    ls -1t "$LOG_DIR"/boot-trace-*.log 2>/dev/null | tail -n +11 | xargs -r rm -f
fi

# Failed restore artifacts are full encrypted backups. Keep only recent evidence.
find "$RESTORE_FAILED_DIR" -type f -mtime +3 -delete 2>/dev/null || true
ls -1t "$RESTORE_FAILED_DIR"/* 2>/dev/null | tail -n +4 | xargs -r rm -f
SCRIPT
chmod +x /opt/lightfriend/host-log-maintenance.sh

cat > /etc/systemd/system/lightfriend-log-maintenance.service <<'LOGMAINTSVCEOF'
[Unit]
Description=Lightfriend host log and artifact maintenance

[Service]
Type=oneshot
ExecStart=/opt/lightfriend/host-log-maintenance.sh
LOGMAINTSVCEOF

cat > /etc/systemd/system/lightfriend-log-maintenance.timer <<'LOGMAINTTIMEREOF'
[Unit]
Description=Run Lightfriend host log maintenance every 5 minutes

[Timer]
OnBootSec=2min
OnUnitActiveSec=5min
Persistent=true

[Install]
WantedBy=timers.target
LOGMAINTTIMEREOF

# ── Install HTTP forward proxy for enclave outbound traffic ──────────────────

echo "Installing squid, socat, jq, and boto3 for enclave networking + presigned URLs..."
dnf install -y squid socat jq python3-pip || echo "WARNING: Failed to install packages"
pip3 install boto3 || echo "WARNING: Failed to install boto3"
# Verify boto3 is usable (scheduled backups depend on it for presigned URLs)
python3 -c "import boto3; print('boto3 OK:', boto3.__version__)" || echo "ERROR: boto3 not working - hourly backups will fail"

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

# VSOCK bridge: enclave's VSOCK port 8500 -> Telegram residential SOCKS5 proxy
# Reads proxy address from .env so the host routes outbound TCP to the real
# residential proxy server. Inside the enclave, a local socat bridges
# TCP:1080 -> VSOCK:8500 so the bridge config points at 127.0.0.1:1080.
cat > /opt/lightfriend/telegram-proxy-bridge.sh <<'SCRIPT'
#!/bin/bash
set -eu

LOG="/opt/lightfriend/logs/telegram-proxy-bridge.log"
mkdir -p /opt/lightfriend/logs

log() { echo "$(date -u +%Y-%m-%dT%H:%M:%SZ) telegram-proxy-bridge: $*" | tee -a "$LOG"; }

while true; do
    if [ ! -f /opt/lightfriend/.env ]; then
        sleep 5
        continue
    fi

    ADDR=$(grep '^TELEGRAM_PROXY_ADDRESS=' /opt/lightfriend/.env 2>/dev/null | tail -1 | cut -d= -f2- | tr -d '\r')
    PORT=$(grep '^TELEGRAM_PROXY_PORT=' /opt/lightfriend/.env 2>/dev/null | tail -1 | cut -d= -f2- | tr -d '\r')

    if [ -z "$${ADDR}" ] || [ -z "$${PORT}" ]; then
        log "TELEGRAM_PROXY_ADDRESS or TELEGRAM_PROXY_PORT not set, sleeping..."
        sleep 30
        continue
    fi

    log "Starting VSOCK:8500 -> TCP:$${ADDR}:$${PORT}"
    /usr/bin/socat VSOCK-LISTEN:8500,reuseaddr,fork TCP:"$${ADDR}":"$${PORT}" 2>>"$LOG" || true
    log "socat exited, restarting in 2s..."
    sleep 2
done
SCRIPT
chmod +x /opt/lightfriend/telegram-proxy-bridge.sh

cat > /etc/systemd/system/vsock-telegram-proxy-bridge.service <<'TGPROXYEOF'
[Unit]
Description=VSOCK bridge to Telegram SOCKS5 residential proxy for Nitro Enclave
After=network-online.target
Wants=network-online.target

[Service]
ExecStart=/opt/lightfriend/telegram-proxy-bridge.sh
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
TGPROXYEOF

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
"""Simple HTTP server that accepts PUT uploads from the enclave."""
import os
from http.server import HTTPServer, BaseHTTPRequestHandler

BACKUP_DIR = "/opt/lightfriend/backups"
LOG_DIR = "/opt/lightfriend/logs"
LOG_ALLOWLIST = {
    "enclave-health-latest.txt": 256 * 1024,
    "enclave-health-history.log": 2 * 1024 * 1024,
}
UPLOAD_ALLOWLIST = {
    "verify-result.json": 256 * 1024,
}
os.makedirs(BACKUP_DIR, exist_ok=True)
os.makedirs(LOG_DIR, exist_ok=True)

class UploadHandler(BaseHTTPRequestHandler):
    def do_PUT(self):
        if self.path.startswith("/upload-log/"):
            target_dir = LOG_DIR
            filename = os.path.basename(self.path[len("/upload-log/"):])
            kind = "log"
            allowed = LOG_ALLOWLIST
        elif self.path.startswith("/upload/"):
            target_dir = BACKUP_DIR
            filename = os.path.basename(self.path[len("/upload/"):])
            kind = "backup"
            allowed = UPLOAD_ALLOWLIST
        else:
            self.send_response(404)
            self.end_headers()
            return
        if not filename or filename not in allowed:
            self.send_response(400)
            self.end_headers()
            return
        dest = os.path.join(target_dir, filename)
        length = int(self.headers.get("Content-Length", 0))
        if length < 1 or length > allowed[filename]:
            self.send_response(413)
            self.end_headers()
            return
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
        print(f"Received {kind} {filename}: {size} bytes")

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
# Enclave's cloudflared connects to Cloudflare edge via this bridge.
# Flow: enclave cloudflared -> VSOCK:7844 -> this bridge -> TCP to Cloudflare edge
# Uses keepalive to detect dead connections, nodelay for HTTP/2 frame latency.
cat > /opt/lightfriend/cloudflared-edge-bridge.sh <<'SCRIPT'
#!/bin/bash
LOG="/opt/lightfriend/logs/cloudflared-edge.log"
mkdir -p /opt/lightfriend/logs

rotate_log_if_needed() {
    if [ -f "$LOG" ] && [ "$(stat -c%s "$LOG" 2>/dev/null || echo 0)" -gt 5242880 ]; then
        tail -c 1048576 "$LOG" > "$${LOG}.tmp" 2>/dev/null && mv "$${LOG}.tmp" "$LOG"
    fi
}

log() { echo "$(date -u +%Y-%m-%dT%H:%M:%SZ): $*" >> "$LOG"; }

rotate_log_if_needed
log "=== Starting cloudflared edge bridge on VSOCK:7844 ==="
log "PID: $$"
log "Resolving region1.v2.argotunnel.com..."
EDGE_IPS=$(getent hosts region1.v2.argotunnel.com 2>&1 || echo "DNS FAILED")
log "  region1 IPs: $EDGE_IPS"
EDGE_IPS2=$(getent hosts region2.v2.argotunnel.com 2>&1 || echo "DNS FAILED")
log "  region2 IPs: $EDGE_IPS2"

# socat options:
#   -d -d: connection lifecycle without unbounded byte-transfer logs
#   VSOCK-LISTEN: accept connections from enclave
#   reuseaddr: allow rebind after restart
#   fork: handle multiple connections (cloudflared opens 4)
#   TCP: connect to Cloudflare edge
#   nodelay: disable Nagle for HTTP/2 frame latency
#   keepalive: enable TCP keepalive to detect dead connections
#   keepidle=10: send first keepalive after 10s idle
#   keepintvl=5: retry every 5s
#   keepcnt=3: give up after 3 failed probes (25s total to detect dead conn)
log "Starting socat bridge..."
exec socat -d -d \
    VSOCK-LISTEN:7844,reuseaddr,fork \
    TCP:region1.v2.argotunnel.com:7844,nodelay,keepalive,keepidle=10,keepintvl=5,keepcnt=3 \
    2>>"$LOG"
SCRIPT
chmod +x /opt/lightfriend/cloudflared-edge-bridge.sh

cat > /etc/systemd/system/vsock-cloudflared-edge.service <<'CFEDGEEOF'
[Unit]
Description=VSOCK bridge for cloudflared edge connections (port 7844)

[Service]
ExecStart=/opt/lightfriend/cloudflared-edge-bridge.sh
Restart=always
RestartSec=3
StandardOutput=append:/opt/lightfriend/logs/cloudflared-edge-stdout.log
StandardError=append:/opt/lightfriend/logs/cloudflared-edge-stderr.log

[Install]
WantedBy=multi-user.target
CFEDGEEOF

# VSOCK bridge for DNS-over-TLS (port 8530 -> 1.1.1.1:853)
# Cloudflared uses DoT for SRV record lookups to discover edge servers
cat > /opt/lightfriend/dot-bridge.sh <<'SCRIPT'
#!/bin/bash
LOG="/opt/lightfriend/logs/dot-bridge.log"
mkdir -p /opt/lightfriend/logs
echo "$(date -u +%Y-%m-%dT%H:%M:%SZ): Starting DoT bridge VSOCK:8530 -> 1.1.1.1:853 (PID $$)" >> "$LOG"
exec socat -d -d VSOCK-LISTEN:8530,reuseaddr,fork TCP:1.1.1.1:853,keepalive 2>>"$LOG"
SCRIPT
chmod +x /opt/lightfriend/dot-bridge.sh

cat > /etc/systemd/system/vsock-dot-bridge.service <<'DOTEOF'
[Unit]
Description=VSOCK bridge for DNS-over-TLS to 1.1.1.1:853

[Service]
ExecStart=/opt/lightfriend/dot-bridge.sh
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
DOTEOF

# ── gvproxy for enclave tap networking (IMAP/SMTP outbound) ──────────────────
# gvisor-tap-vsock provides a real network interface to the enclave over VSOCK.
# gvproxy runs on the host, gvforwarder runs inside the enclave.
# This enables protocols that can't use HTTP proxy (IMAP, SMTP, Telegram MTProto).
echo "Installing gvproxy for enclave tap networking..."
curl -sL -o /usr/local/bin/gvproxy \
    https://github.com/containers/gvisor-tap-vsock/releases/download/v0.8.8/gvproxy-linux-amd64
chmod +x /usr/local/bin/gvproxy

cat > /etc/systemd/system/gvproxy.service <<'GVPROXYEOF'
[Unit]
Description=gvisor-tap-vsock proxy for enclave networking (VSOCK port 1024)
After=network.target

[Service]
ExecStartPre=/bin/sh -c '[ -S /tmp/network.sock ] && rm -f /tmp/network.sock || true'
ExecStart=/usr/local/bin/gvproxy -listen vsock://:1024 -listen unix:///tmp/network.sock
Restart=always
RestartSec=3
StandardOutput=append:/opt/lightfriend/logs/gvproxy.log
StandardError=append:/opt/lightfriend/logs/gvproxy-err.log

[Install]
WantedBy=multi-user.target
GVPROXYEOF

# Enable and start all VSOCK services + gvproxy
systemctl daemon-reload
systemctl enable lightfriend-log-maintenance.timer
systemctl start lightfriend-log-maintenance.timer
/opt/lightfriend/host-log-maintenance.sh || true
for svc in vsock-proxy-bridge vsock-config-server vsock-marlin-kms-bridge vsock-telegram-proxy-bridge vsock-boot-trace seed-http-server vsock-seed-http vsock-cloudflared-edge vsock-dot-bridge backup-upload-server vsock-backup-upload gvproxy; do
    systemctl enable "$svc"
    systemctl start "$svc" || echo "WARNING: $svc failed to start"
done

echo "VSOCK services configured: proxy:8001, config:9000, boot-trace:9007, tg-proxy:8500, seed-http:9080, cf-edge:7844, dot:8530, marlin-kms:9010, gvproxy:1024"

# ── Enclave launch script ───────────────────────────────────────────────────

cat > /opt/lightfriend/launch-enclave.sh <<'SCRIPT'
#!/bin/bash
set -e
EIF_PATH="/opt/lightfriend/lightfriend.eif"
VERIFY="/opt/lightfriend/verify-result.json"
VSOCK_SVCS="vsock-proxy-bridge vsock-config-server vsock-marlin-kms-bridge vsock-telegram-proxy-bridge vsock-boot-trace vsock-seed-http vsock-cloudflared-edge vsock-dot-bridge vsock-backup-upload gvproxy"

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
for svc in squid vsock-proxy-bridge vsock-config-server vsock-boot-trace vsock-marlin-kms-bridge vsock-cloudflared-edge vsock-dot-bridge vsock-seed-http vsock-backup-upload gvproxy; do
    STATUS=$(systemctl is-active "$svc" 2>/dev/null || echo "not-found")
    printf "  %-30s %s\n" "$svc" "$STATUS"
done

echo ""
echo "--- 2b. Cloudflared Edge Bridge ---"
echo "  Service: $(systemctl is-active vsock-cloudflared-edge 2>&1)"
echo "  VSOCK listeners on 7844:"
ss -tlnp 2>/dev/null | grep 7844 || echo "    NONE - bridge not listening!"
echo "  Active connections through bridge:"
ss -tnp 2>/dev/null | grep 7844 || echo "    none"
echo "  socat processes:"
ps aux 2>/dev/null | grep '[s]ocat.*7844' || echo "    none"
if [ -f /opt/lightfriend/logs/cloudflared-edge.log ]; then
    echo "  Bridge log (last 20 lines):"
    tail -20 /opt/lightfriend/logs/cloudflared-edge.log
fi
echo ""
echo "--- 2c. DoT Bridge ---"
echo "  Service: $(systemctl is-active vsock-dot-bridge 2>&1)"
if [ -f /opt/lightfriend/logs/dot-bridge.log ]; then
    echo "  DoT log (last 10 lines):"
    tail -10 /opt/lightfriend/logs/dot-bridge.log
fi

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
HTTP_CODE=$(curl -sf --max-time 10 -o /dev/null -w '%%{http_code}' https://lightfriend.ai 2>/dev/null || echo "timeout")
echo "  https://lightfriend.ai -> HTTP $HTTP_CODE"

echo ""
echo "--- 9. Enclave Internal Diagnostics (VSOCK 9008) ---"
timeout 15 socat -T10 - VSOCK-CONNECT:16:9008 2>/dev/null || echo "  Could not connect to enclave diagnostic port"

echo ""
echo "--- 10. Persistent Enclave Health Snapshots ---"
for f in /opt/lightfriend/logs/enclave-health-latest.txt /opt/lightfriend/logs/enclave-health-history.log /opt/lightfriend/logs/health-s3-sync.log; do
    if [ -f "$f" ]; then
        SIZE=$(stat -c%s "$f" 2>/dev/null || echo "?")
        echo "  $f ($SIZE bytes)"
        tail -40 "$f"
    else
        echo "  $f missing"
    fi
done

echo ""
echo "========================================"
SCRIPT
chmod +x /opt/lightfriend/diagnose.sh

# ── S3 backup download scripts ───────────────────────────────────────

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

# ── Scheduled hourly full snapshot backup ────────────────────────────────
# Host generates presigned PUT URLs and writes trigger JSON to seed/.
# Enclave's export-watcher picks it up, runs export.sh, uploads directly.
# Promote rules included in trigger - enclave re-uploads to tier keys.

cat > /opt/lightfriend/scheduled-backup.sh <<'SCRIPT'
#!/bin/bash
# Hourly backup: generate presigned PUT URLs, write trigger to seed/.
# Enclave's export-watcher picks it up, runs export.sh, uploads directly.
set -euo pipefail

LOCKFILE="/tmp/lightfriend-backup.lock"
exec 200>"$LOCKFILE"
flock -n 200 || { echo "Skipping: another backup/deploy is running"; exit 0; }

TIMESTAMP=$(date -u +%Y%m%dT%H%M%SZ)
BUCKET=$(grep S3_BACKUP_BUCKET /opt/lightfriend/.env | cut -d= -f2)
REGION=$(grep AWS_REGION /opt/lightfriend/.env | cut -d= -f2)
BACKUP_S3_KEY="backups/hourly/lightfriend-full-backup-$TIMESTAMP.tar.gz.enc"

echo "=== Hourly backup: $TIMESTAMP ==="

# Single python script generates all presigned URLs and writes trigger JSON.
# Promote rules: at midnight copy to daily, on Sunday to weekly, on 1st to monthly.
python3 - "$BUCKET" "$REGION" "$BACKUP_S3_KEY" "$TIMESTAMP" <<'PYEOF'
import boto3, json, sys, os
from datetime import datetime, timezone

bucket, region, backup_key, timestamp = sys.argv[1:5]
s3 = boto3.client('s3', region_name=region)

def put_url(client, bkt, key, content_type='application/octet-stream'):
    return client.generate_presigned_url('put_object',
        Params={'Bucket': bkt, 'Key': key, 'ContentType': content_type},
        ExpiresIn=3600)

trigger = {
    'action': 'export',
    'type': 'hourly',
    'backup_s3_key': backup_key,
    'presigned_put_backup_s3': put_url(s3, bucket, backup_key),
    'presigned_put_health': put_url(s3, bucket, 'backups/backup-health.json', 'application/json'),
    'timestamp': timestamp,
}

# Load R2 creds from .env file
env_vars = {}
try:
    with open('/opt/lightfriend/.env') as f:
        for line in f:
            if '=' in line and not line.startswith('#'):
                k, v = line.strip().split('=', 1)
                env_vars[k] = v
except Exception:
    pass

r2_bucket = env_vars.get('R2_BACKUP_BUCKET', '')
r2_endpoint = env_vars.get('R2_ENDPOINT_URL', '')
r2_access = env_vars.get('R2_ACCESS_KEY_ID', '')
r2_secret = env_vars.get('R2_SECRET_ACCESS_KEY', '')
if r2_bucket and r2_endpoint and r2_access:
    try:
        r2 = boto3.client('s3', endpoint_url=r2_endpoint,
            aws_access_key_id=r2_access, aws_secret_access_key=r2_secret,
            region_name='auto')
        trigger['presigned_put_backup_r2'] = put_url(r2, r2_bucket, backup_key)
    except Exception as e:
        print(f"WARNING: R2 URL generation failed: {e}", flush=True)

# Promote rules: at midnight -> daily, Sunday midnight -> weekly, 1st midnight -> monthly
# Enclave uploads the same local file to tier keys (no download needed)
now = datetime.now(timezone.utc)
basename = backup_key.rsplit('/', 1)[-1]
promote = []
if now.hour == 0:
    promote.append({'tier_key': f'backups/daily/{basename}', 'presigned_put': put_url(s3, bucket, f'backups/daily/{basename}')})
if now.hour == 0 and now.isoweekday() == 7:
    promote.append({'tier_key': f'backups/weekly/{basename}', 'presigned_put': put_url(s3, bucket, f'backups/weekly/{basename}')})
if now.hour == 0 and now.day == 1:
    promote.append({'tier_key': f'backups/monthly/{basename}', 'presigned_put': put_url(s3, bucket, f'backups/monthly/{basename}')})
trigger['promote'] = promote

with open('/opt/lightfriend/seed/export-request.json', 'w') as f:
    json.dump(trigger, f)
print(f"Trigger written: {backup_key}")
print(f"Promote tiers: {len(promote)}")
PYEOF

echo "=== Host done, enclave takes over ==="
SCRIPT
chmod +x /opt/lightfriend/scheduled-backup.sh

# Use systemd timer instead of cron (cron not installed on Amazon Linux 2023)
cat > /etc/systemd/system/scheduled-backup.service <<'BACKUPSVCEOF'
[Unit]
Description=Lightfriend scheduled backup
After=network-online.target

[Service]
Type=oneshot
ExecStart=/opt/lightfriend/scheduled-backup.sh
StandardOutput=append:/opt/lightfriend/logs/scheduled-backup.log
StandardError=append:/opt/lightfriend/logs/scheduled-backup.log
BACKUPSVCEOF

cat > /etc/systemd/system/scheduled-backup.timer <<'BACKUPTIMEREOF'
[Unit]
Description=Run Lightfriend backup every hour

[Timer]
OnCalendar=hourly
Persistent=true
RandomizedDelaySec=300

[Install]
WantedBy=timers.target
BACKUPTIMEREOF

# ── Persistent enclave health snapshot S3 archival ─────────────────────────

cat > /opt/lightfriend/health-s3-sync.sh <<'SCRIPT'
#!/bin/bash
set -euo pipefail

LOG="/opt/lightfriend/logs/health-s3-sync.log"
LATEST="/opt/lightfriend/logs/enclave-health-latest.txt"
HISTORY="/opt/lightfriend/logs/enclave-health-history.log"
STAMP_FILE="/opt/lightfriend/logs/.health-s3-last-archive-hour"
mkdir -p /opt/lightfriend/logs

exec >> "$LOG" 2>&1
echo "=== health-s3-sync $(date -u +%Y-%m-%dT%H:%M:%SZ) ==="

BUCKET=$(grep '^S3_BACKUP_BUCKET=' /opt/lightfriend/.env 2>/dev/null | tail -1 | cut -d= -f2- || true)
if [ -z "$BUCKET" ]; then
    BUCKET=$(aws ssm get-parameter --name /lightfriend/s3-bucket --query Parameter.Value --output text 2>/dev/null || true)
fi
if [ -z "$BUCKET" ] || [ "$BUCKET" = "None" ]; then
    echo "S3 backup bucket not found"
    exit 0
fi

TOKEN=$(curl -sf --max-time 2 -X PUT "http://169.254.169.254/latest/api/token" -H "X-aws-ec2-metadata-token-ttl-seconds: 21600" || true)
if [ -n "$TOKEN" ]; then
    INSTANCE_ID=$(curl -sf --max-time 2 -H "X-aws-ec2-metadata-token: $TOKEN" http://169.254.169.254/latest/meta-data/instance-id || hostname)
else
    INSTANCE_ID=$(hostname)
fi

PREFIX="diagnostics/enclave-health/$${INSTANCE_ID}"

if [ -s "$LATEST" ]; then
    aws s3 cp "$LATEST" "s3://$${BUCKET}/$${PREFIX}/latest.txt"
    echo "Uploaded latest snapshot"
else
    echo "No latest snapshot yet"
fi

if [ -s "$HISTORY" ]; then
    aws s3 cp "$HISTORY" "s3://$${BUCKET}/$${PREFIX}/history-latest.log"
    echo "Uploaded rolling history"

    HOUR=$(date -u +%Y%m%dT%H)
    LAST_HOUR=$(cat "$STAMP_FILE" 2>/dev/null || true)
    if [ "$HOUR" != "$LAST_HOUR" ]; then
        TS=$(date -u +%Y%m%dT%H%M%SZ)
        aws s3 cp "$HISTORY" "s3://$${BUCKET}/$${PREFIX}/history/enclave-health-$${TS}.log"
        echo "$HOUR" > "$STAMP_FILE"
        echo "Archived hourly history snapshot"
    fi
else
    echo "No history snapshot yet"
fi

if [ -f "$LOG" ] && [ "$(stat -c%s "$LOG" 2>/dev/null || echo 0)" -gt 1048576 ]; then
    tail -c 524288 "$LOG" > "$${LOG}.tmp" && mv "$${LOG}.tmp" "$LOG"
fi
SCRIPT
chmod +x /opt/lightfriend/health-s3-sync.sh

cat > /etc/systemd/system/health-s3-sync.service <<'HEALTHS3SVCEOF'
[Unit]
Description=Archive Lightfriend enclave health snapshots to S3
After=network-online.target backup-upload-server.service

[Service]
Type=oneshot
ExecStart=/opt/lightfriend/health-s3-sync.sh
HEALTHS3SVCEOF

cat > /etc/systemd/system/health-s3-sync.timer <<'HEALTHS3TIMEREOF'
[Unit]
Description=Archive Lightfriend enclave health snapshots to S3 every 5 minutes

[Timer]
OnBootSec=5min
OnUnitActiveSec=5min
Persistent=true
RandomizedDelaySec=30

[Install]
WantedBy=timers.target
HEALTHS3TIMEREOF

systemctl daemon-reload
systemctl enable scheduled-backup.timer
systemctl start scheduled-backup.timer
echo "Hourly backup timer enabled: $(systemctl is-active scheduled-backup.timer)"
systemctl enable health-s3-sync.timer
systemctl start health-s3-sync.timer
echo "Health S3 sync timer enabled: $(systemctl is-active health-s3-sync.timer)"

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

    # Download Tesla private key (for vehicle command signing proxy)
    if aws s3 ls "s3://$BUCKET/config/tesla_private_key.pem" 2>/dev/null; then
        aws s3 cp "s3://$BUCKET/config/tesla_private_key.pem" /opt/lightfriend/seed/tesla_private_key.pem
        chmod 600 /opt/lightfriend/seed/tesla_private_key.pem
        echo "Tesla private key downloaded to seed directory"
    fi

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
    echo "=== Phase 1 complete: EIF downloaded and verified ==="

    # ── Phase 2: Wait for export-complete.json from old enclave, then restore ──
    # The old enclave uploads export-complete.json directly to S3 when export finishes.
    # We poll for it here - no CI middleman needed.
    COMPLETE_KEY="deploy/export-complete.json"
    # Poll for export-complete.json from old enclave (5s interval, 30 min max)
    echo "Polling s3://$BUCKET/$COMPLETE_KEY (5s interval)..."
    BACKUP_KEY=""
    DEPLOY_ID="unknown"
    RESTORE_TYPE=""
    for i in $(seq 1 360); do
        EXPORT_COMPLETE=$(aws s3 cp "s3://$BUCKET/$COMPLETE_KEY" - 2>/dev/null || echo "")
        if [ -n "$EXPORT_COMPLETE" ]; then
            STATUS=$(echo "$EXPORT_COMPLETE" | jq -r '.status // "UNKNOWN"')
            if [ "$STATUS" = "SUCCESS" ]; then
                BACKUP_KEY=$(echo "$EXPORT_COMPLETE" | jq -r '.backup_key // empty')
                DEPLOY_ID=$(echo "$EXPORT_COMPLETE" | jq -r '.deploy_id // "unknown"')
                RESTORE_TYPE=$(echo "$EXPORT_COMPLETE" | jq -r '.restore_type // "full"')
                echo "Signal received after $((i * 5))s: type=$RESTORE_TYPE key=$BACKUP_KEY"
                break
            elif [ "$STATUS" = "FAILED" ]; then
                echo "{\"status\": \"EXPORT_FAILED\", \"instance_id\": \"$INSTANCE_ID\"}" | \
                    aws s3 cp - "s3://$BUCKET/deploy/verify-$INSTANCE_ID.json"
                echo "FATAL: Export failed"
                exit 1
            fi
        fi
        [ $((i % 12)) -eq 0 ] && echo "  Waiting... $((i * 5))s"
        sleep 5
    done

    # Validate we got what we need
    if [ -z "$RESTORE_TYPE" ]; then
        echo "{\"status\": \"EXPORT_TIMEOUT\", \"instance_id\": \"$INSTANCE_ID\"}" | \
            aws s3 cp - "s3://$BUCKET/deploy/verify-$INSTANCE_ID.json"
        echo "FATAL: No export-complete.json after 30 minutes"
        exit 1
    fi
    if [ "$RESTORE_TYPE" = "full" ] && [ -z "$BACKUP_KEY" ]; then
        echo "{\"status\": \"MISSING_BACKUP_KEY\", \"instance_id\": \"$INSTANCE_ID\"}" | \
            aws s3 cp - "s3://$BUCKET/deploy/verify-$INSTANCE_ID.json"
        echo "FATAL: export-complete.json has restore_type=full but no backup_key"
        exit 1
    fi

    # Re-download .env
    aws s3 cp "s3://$BUCKET/config/.env" /opt/lightfriend/.env
    chmod 600 /opt/lightfriend/.env

    # Re-download Tesla private key
    if aws s3 ls "s3://$BUCKET/config/tesla_private_key.pem" 2>/dev/null; then
        aws s3 cp "s3://$BUCKET/config/tesla_private_key.pem" /opt/lightfriend/seed/tesla_private_key.pem
        chmod 600 /opt/lightfriend/seed/tesla_private_key.pem
    fi

    if [ "$RESTORE_TYPE" = "seed" ]; then
        # Seed mode: seed SQL should already be at /opt/lightfriend/seed/lightfriend_db.sql
        # (delivered by disaster recovery workflow via SSM, or placed manually)
        # If not there, try S3
        if [ ! -f /opt/lightfriend/seed/lightfriend_db.sql ]; then
            aws s3 cp "s3://$BUCKET/seed/lightfriend_db.sql" /opt/lightfriend/seed/lightfriend_db.sql 2>/dev/null || true
        fi
        echo "=== Seed mode: launching enclave (entrypoint.sh will find seed SQL) ==="
        /opt/lightfriend/launch-enclave.sh 2>&1 | tee /tmp/launch.log || {
            echo "{\"status\": \"LAUNCH_FAILED\", \"instance_id\": \"$INSTANCE_ID\"}" | \
                aws s3 cp - "s3://$BUCKET/deploy/verify-$INSTANCE_ID.json"
            exit 1
        }
        VERIFY_SRC="/opt/lightfriend/backups/verify-result.json"
        for i in $(seq 1 180); do
            if [ -s "$VERIFY_SRC" ]; then
                cp "$VERIFY_SRC" "$VERIFY"
                rm -f "$VERIFY_SRC"
                break
            fi
            sleep 5
        done
    elif [ "$RESTORE_TYPE" = "fresh" ]; then
        # Fresh start: launch enclave with empty database
        echo "=== Fresh start: launching enclave with empty database ==="
        /opt/lightfriend/launch-enclave.sh 2>&1 | tee /tmp/launch.log || {
            echo "{\"status\": \"LAUNCH_FAILED\", \"instance_id\": \"$INSTANCE_ID\"}" | \
                aws s3 cp - "s3://$BUCKET/deploy/verify-$INSTANCE_ID.json"
            exit 1
        }
        VERIFY_SRC="/opt/lightfriend/backups/verify-result.json"
        for i in $(seq 1 180); do
            if [ -s "$VERIFY_SRC" ]; then
                cp "$VERIFY_SRC" "$VERIFY"
                rm -f "$VERIFY_SRC"
                break
            fi
            sleep 5
        done
    else
        # Normal backup restore
        echo "Restoring from: $BACKUP_KEY"
        set -o pipefail
        if ! /opt/lightfriend/restore-enclave.sh "$BACKUP_KEY" "$DEPLOY_ID" "full" 2>&1 | tee /tmp/launch.log; then
            RESTORE_LOG=$(cat /tmp/restore-enclave-debug.log 2>/dev/null | tail -50 | tr '\n' '|' | sed 's/"/\\"/g' | head -c 1500)
            echo "{\"status\": \"RESTORE_FAILED\", \"instance_id\": \"$INSTANCE_ID\", \"log\": \"$RESTORE_LOG\"}" | \
                aws s3 cp - "s3://$BUCKET/deploy/verify-$INSTANCE_ID.json"
            exit 1
        fi
        set +o pipefail
    fi

    if [ -s "$VERIFY" ]; then
        aws s3 cp "$VERIFY" "s3://$BUCKET/deploy/verify-$INSTANCE_ID.json"
        echo "Verify result uploaded"
        cat "$VERIFY"
    else
        ENCLAVE_STATUS=$(nitro-cli describe-enclaves 2>/dev/null | jq -r '.[0].State // "unknown"')
        echo "{\"status\": \"TIMEOUT\", \"instance_id\": \"$INSTANCE_ID\", \"enclave_state\": \"$ENCLAVE_STATUS\"}" | \
            aws s3 cp - "s3://$BUCKET/deploy/verify-$INSTANCE_ID.json"
        echo "FATAL: No verify result"
        exit 1
    fi

    echo "=== Auto-bootstrap complete ==="
else
    echo "No .env in S3 - skipping auto-bootstrap (manual first-time setup required)"
fi
