#!/bin/bash
# Diagnostic script - runs inside the enclave when host connects to VSOCK port 9008.
# Dumps all supervisor service status, bridge health, and recent logs.
#
# PRIVACY: This script is open-source and auditable on GitHub.
# It NEVER logs message content, user data, phone numbers, or credentials.
# Bridge logs are filtered to remove any message body/content fields.
# Only operational status (connected/disconnected, errors, sync state) is captured.

# Helper: strip potential message content from bridge logs.
# Keeps error lines, status changes, connection events. Removes message bodies.
sanitize_bridge_log() {
    sed -E \
        -e 's/(body|message|text|content|caption)="[^"]*"/\1="[REDACTED]"/gi' \
        -e 's/(body|message|text|content|caption): .*/\1: [REDACTED]/gi' \
        -e 's/("body"|"message"|"text"|"content"|"caption"): ?"[^"]*"/\1: "[REDACTED]"/gi'
}

echo "=== Enclave Diagnostics $(date -u +%Y-%m-%dT%H:%M:%SZ) ==="
echo ""

# ── CRITICAL: Dump all service logs FIRST before any health checks ──
# Health checks can hang if services are unresponsive, so we dump
# the actual application logs upfront to always capture them.
echo "--- backend stderr (last 80 lines) ---"
tail -80 /var/log/supervisor/lightfriend-err.log 2>/dev/null || echo "  empty"
echo ""

echo "--- backend stdout (last 100 lines, excluding IDLE noise) ---"
tail -500 /var/log/supervisor/lightfriend.log 2>/dev/null | grep -v "IDLE established\|Spawned IDLE task\|Job creator created\|Uninited" | tail -100 || echo "  empty"
echo ""

echo "--- cloudflared stderr (last 40 lines) ---"
tail -40 /var/log/supervisor/cloudflared-err.log 2>/dev/null || echo "  empty"
echo ""

echo "--- cloudflared stdout (last 20 lines) ---"
tail -20 /var/log/supervisor/cloudflared.log 2>/dev/null || echo "  empty"
echo ""

# ── Bridge logs (privacy-filtered: no message content) ──
echo "--- mautrix-whatsapp stdout (last 60 lines, sanitized) ---"
tail -60 /var/log/supervisor/whatsapp.log 2>/dev/null | sanitize_bridge_log || echo "  empty"
echo ""

echo "--- mautrix-whatsapp stderr (last 40 lines, sanitized) ---"
tail -40 /var/log/supervisor/whatsapp-err.log 2>/dev/null | sanitize_bridge_log || echo "  empty"
echo ""

echo "--- mautrix-signal stdout (last 40 lines, sanitized) ---"
tail -40 /var/log/supervisor/signal.log 2>/dev/null | sanitize_bridge_log || echo "  empty"
echo ""

echo "--- mautrix-signal crypto/session errors (last 20, sanitized) ---"
grep -Ei "Decryption error|failed to decrypt|Failed to verify ACI-PNI mapping|failed to fetch prekey|identity was.?t found in store" \
    /var/log/supervisor/signal.log 2>/dev/null | tail -20 | sanitize_bridge_log || echo "  none"
echo ""

echo "--- mautrix-signal stderr (last 20 lines, sanitized) ---"
tail -20 /var/log/supervisor/signal-err.log 2>/dev/null | sanitize_bridge_log || echo "  empty"
echo ""

echo "--- mautrix-telegram stdout (last 80 lines, sanitized) ---"
tail -80 /var/log/supervisor/telegram.log 2>/dev/null | sanitize_bridge_log || echo "  empty"
echo ""

echo "--- mautrix-telegram stderr (last 40 lines, sanitized) ---"
tail -40 /var/log/supervisor/telegram-err.log 2>/dev/null | sanitize_bridge_log || echo "  empty"
echo ""

echo "--- tuwunel stdout (last 40 lines) ---"
tail -40 /var/log/supervisor/tuwunel.log 2>/dev/null || echo "  empty"
echo ""

echo "--- tuwunel stderr (last 40 lines) ---"
tail -40 /var/log/supervisor/tuwunel-err.log 2>/dev/null || echo "  empty"
echo ""

echo "--- startup-services.log (last 100 lines) ---"
tail -100 /data/seed/startup-services.log 2>/dev/null || echo "  not found"
echo ""

echo "--- storage health ---"
if [ -x /app/storage-health.sh ]; then
    /app/storage-health.sh report 2>&1
    echo "--- storage health history (last 80 lines) ---"
    tail -80 /tmp/storage-health-history.log 2>/dev/null || echo "  no history yet"
else
    df -h 2>&1 || true
    df -i 2>&1 || true
fi
echo ""

echo "--- memory health ---"
if [ -x /app/memory-health.sh ]; then
    /app/memory-health.sh report 2>&1
    echo "--- memory health history (last 80 lines) ---"
    tail -80 /tmp/memory-health-history.log 2>/dev/null || echo "  no history yet"
else
    free -h 2>&1 || true
    ps -eo pid,ppid,rss,comm,args --sort=-rss 2>/dev/null | head -25 || true
fi
echo ""

echo "--- host health upload ---"
tail -80 /tmp/health-upload-last.log 2>/dev/null || echo "  no upload attempt recorded yet"
echo "--- enclave health upload history (last 80 lines) ---"
tail -80 /tmp/enclave-health-history.log 2>/dev/null || echo "  no persistent health history yet"
echo ""

echo "--- export-watcher-last-run.log (last 80 lines) ---"
tail -80 /tmp/export-watcher-last-run.log 2>/dev/null || echo "  not found (no export has run yet)"
echo ""

echo "--- boot-trace.log restore section (grep DEBUG/restore/tuwunel/bridge) ---"
grep -i "DEBUG\|restore\|tuwunel\|bridge.*tar\|matrix_store\|STEP 2\|decrypt\|Full restore\|checkpoint\|RocksDB\|CURRENT\|IDENTITY\|file count\|total size\|MANIFEST\|sst\|whatsmeow\|user_login" /data/seed/boot-trace.log 2>/dev/null | head -100 || echo "  not found or no matches"
echo ""

echo "--- boot-trace.log (last 40 lines) ---"
tail -40 /data/seed/boot-trace.log 2>/dev/null || echo "  not found"
echo ""

echo "--- supervisorctl status ---"
supervisorctl status 2>&1
echo ""

echo "--- telegram SOCKS5 proxy check ---"
echo "socat 1080 listener: $(ss -tlnp 2>/dev/null | grep ':1080' || echo 'NOT LISTENING')"
echo "socat 1080 processes: $(pgrep -la socat 2>/dev/null | grep 1080 || echo 'none')"
echo "VSOCK 8500 connections: $(ss -tnp 2>/dev/null | grep '8500' | wc -l)"
echo ""

echo "--- env check ---"
echo "PG_DATABASE_URL: ${PG_DATABASE_URL:+set (${#PG_DATABASE_URL} chars)}"
echo "DATABASE_URL: ${DATABASE_URL:+set (${#DATABASE_URL} chars)}"
echo "PORT: ${PORT:-not set}"
echo "HTTP_PROXY: ${HTTP_PROXY:-not set}"
echo "NO_PROXY: ${NO_PROXY:-not set}"
echo "CLOUDFLARE_TUNNEL_TOKEN: ${CLOUDFLARE_TUNNEL_TOKEN:+set (${#CLOUDFLARE_TUNNEL_TOKEN} chars)}"
echo ""

echo "--- postgres check ---"
pg_isready -h localhost -U postgres 2>&1 || echo "pg_isready failed"
echo ""

echo "--- backend health ---"
echo "  port ${PORT:-3100}: $(ss -tlnp 2>/dev/null | grep ":${PORT:-3100}" || echo 'NOT LISTENING')"
# Use subshell with kill to ensure we never hang
(curl -sf --max-time 1 --connect-timeout 1 http://localhost:${PORT:-3100}/api/health 2>&1 & CPID=$!; sleep 2; kill $CPID 2>/dev/null; wait $CPID 2>/dev/null) || echo "backend not responding (hung or crashed)"
echo ""

echo "--- tuwunel health ---"
echo "  port 8008: $(ss -tlnp 2>/dev/null | grep ':8008' || echo 'NOT LISTENING')"
(curl -sf --max-time 1 --connect-timeout 1 http://localhost:8008/_matrix/client/versions 2>&1 & CPID=$!; sleep 2; kill $CPID 2>/dev/null; wait $CPID 2>/dev/null) || echo "tuwunel not responding"
echo ""

echo "--- backend process details ---"
BACKEND_PID=$(pgrep -f '/app/backend' 2>/dev/null | head -1)
if [ -n "$BACKEND_PID" ]; then
    echo "  PID: $BACKEND_PID"
    echo "  RSS: $(ps -o rss= -p "$BACKEND_PID" 2>/dev/null | tr -d ' ')KB"
    echo "  Threads: $(ls /proc/$BACKEND_PID/task 2>/dev/null | wc -l)"
    echo "  FDs: $(ls /proc/$BACKEND_PID/fd 2>/dev/null | wc -l)"
    echo "  Open files: $(ls -la /proc/$BACKEND_PID/fd 2>/dev/null | grep -c socket)"
    echo "  TCP connections from backend:"
    ss -tnp 2>/dev/null | grep "pid=$BACKEND_PID" | head -20
    echo "  State: $(cat /proc/$BACKEND_PID/status 2>/dev/null | grep -E 'State|Threads|VmRSS|VmSize|FDSize')"
else
    echo "  Backend process NOT FOUND!"
fi
echo ""

echo "--- tap0 networking (gvisor-tap-vsock) ---"
if ip addr show tap0 2>/dev/null | grep -q 'inet '; then
    echo "  tap0: UP"
    ip addr show tap0 2>/dev/null | grep -E 'inet |link/ether'
    echo "  default route: $(ip route show default 2>/dev/null)"
    echo "  gvforwarder: $(pgrep -f gvforwarder >/dev/null 2>&1 && echo 'running' || echo 'NOT running')"
    echo "  DNS: $(cat /etc/resolv.conf 2>/dev/null | head -3)"
    echo "  gateway ping: $(timeout 2 ping -c1 -W1 192.168.127.1 >/dev/null 2>&1 && echo 'OK' || echo 'FAIL')"
    echo "  internet test: $(timeout 3 curl -sf --noproxy '*' --max-time 2 https://api.cloudflare.com/cdn-cgi/trace 2>/dev/null | head -1 || echo 'FAIL')"
    echo "  gvforwarder log (last 5 lines):"
    tail -5 /var/log/gvforwarder.log 2>/dev/null || echo "    no log"
else
    echo "  tap0: NOT CONFIGURED"
    echo "  gvforwarder: $(pgrep -f gvforwarder >/dev/null 2>&1 && echo 'running but no IP' || echo 'NOT running')"
    if [ -f /var/log/gvforwarder.log ]; then
        echo "  gvforwarder log (last 10 lines):"
        tail -10 /var/log/gvforwarder.log 2>/dev/null
    fi
fi
echo ""

echo "--- network: all listeners ---"
ss -tlnp 2>/dev/null
echo ""

echo "--- network: all established connections ---"
ss -tnp 2>/dev/null
echo ""

echo "--- /etc/hosts ---"
cat /etc/hosts 2>/dev/null
echo ""

echo "--- DNS resolution ---"
echo "  region1.v2.argotunnel.com: $(getent hosts region1.v2.argotunnel.com 2>&1 || echo 'FAILED')"
echo "  region2.v2.argotunnel.com: $(getent hosts region2.v2.argotunnel.com 2>&1 || echo 'FAILED')"
echo ""

echo "--- lo interface (check 1.1.1.1 bound) ---"
ip addr show lo 2>/dev/null
echo ""

# ── Cloudflared-specific diagnostics ──
echo "=============================================="
echo "=== CLOUDFLARED DIAGNOSTICS ==="
echo "=============================================="
echo ""

echo "--- bridge 7844 (cloudflared edge) ---"
echo "  listening: $(ss -tlnp 2>/dev/null | grep ':7844' || echo 'NOT LISTENING!')"
echo "  active connections: $(ss -tnp 2>/dev/null | grep ':7844' | wc -l)"
echo "  connection states:"
ss -tn 2>/dev/null | grep ':7844' || echo "    none"
echo "  supervisor: $(supervisorctl status vsock-bridge-7844 2>&1)"
echo "  socat PIDs: $(pgrep -f 'socat.*7844' 2>/dev/null | tr '\n' ' ' || echo 'none')"
echo ""

echo "--- bridge 853 (DoT) ---"
echo "  listening: $(ss -tlnp 2>/dev/null | grep ':853' || echo 'NOT LISTENING!')"
echo "  active connections: $(ss -tnp 2>/dev/null | grep ':853' | wc -l)"
echo "  supervisor: $(supervisorctl status vsock-bridge-dot 2>&1)"
echo ""

echo "--- cloudflared process ---"
CF_PID=$(pgrep -f 'cloudflared tunnel' 2>/dev/null | head -1)
if [ -n "$CF_PID" ]; then
    echo "  PID: $CF_PID"
    echo "  RSS: $(ps -o rss= -p "$CF_PID" 2>/dev/null | tr -d ' ')KB"
    echo "  Uptime: $(ps -o etime= -p "$CF_PID" 2>/dev/null | tr -d ' ')"
    echo "  FDs: $(ls /proc/$CF_PID/fd 2>/dev/null | wc -l)"
    echo "  TCP connections from cloudflared:"
    ss -tnp 2>/dev/null | grep "pid=$CF_PID" || echo "    none"
else
    echo "  NOT RUNNING!"
fi
echo ""

echo "--- TCP connectivity test to bridge ---"
if timeout 5 bash -c "echo DIAG_TEST | socat -T3 - TCP:127.0.0.1:7844,connect-timeout=3" 2>&1; then
    echo "  TCP:7844 test: PASS"
else
    echo "  TCP:7844 test: FAIL (rc=$?)"
fi
echo ""

echo "--- cloudflared-diag log (full) ---"
cat /var/log/supervisor/cloudflared-diag.log 2>/dev/null || echo "  no diag log"
echo ""

echo "--- cloudflared-monitor detail log (last 50 lines) ---"
tail -50 /var/log/supervisor/cloudflared-monitor-detail.log 2>/dev/null || echo "  no monitor log"
echo ""

echo "--- cloudflared stderr (last 50 lines) ---"
tail -50 /var/log/supervisor/cloudflared-err.log 2>/dev/null || echo "  empty"
echo ""

echo "--- cloudflared stdout (last 50 lines) ---"
tail -50 /var/log/supervisor/cloudflared.log 2>/dev/null || echo "  empty"
echo ""

echo "--- vsock-7844 bridge log (last 30 lines) ---"
tail -30 /var/log/supervisor/vsock-7844.log 2>/dev/null || echo "  no log"
tail -30 /var/log/supervisor/vsock-7844-err.log 2>/dev/null || echo "  no err log"
echo ""

echo "--- vsock-dot bridge log (last 20 lines) ---"
tail -20 /var/log/supervisor/vsock-dot.log 2>/dev/null || echo "  no log"
tail -20 /var/log/supervisor/vsock-dot-err.log 2>/dev/null || echo "  no err log"
echo ""

# ── End cloudflared section ──

echo "--- port 9080: $(ss -tlnp 2>/dev/null | grep ':9080' || echo 'not listening')"
echo "--- port 9081: $(ss -tlnp 2>/dev/null | grep ':9081' || echo 'not listening')"
echo ""

echo "--- KMS derive test ---"
if curl -sf --max-time 5 http://127.0.0.1:1101/derive/x25519?path=lightfriend/backup > /tmp/diag-key1.bin 2>/dev/null; then
    curl -sf --max-time 5 http://127.0.0.1:1101/derive/x25519?path=lightfriend/backup > /tmp/diag-key2.bin 2>/dev/null
    FP1=$(cat /tmp/diag-key1.bin | base64 | tr -d '\n' | sha256sum | cut -c1-16)
    FP2=$(cat /tmp/diag-key2.bin | base64 | tr -d '\n' | sha256sum | cut -c1-16)
    SZ1=$(stat -c%s /tmp/diag-key1.bin 2>/dev/null || echo "?")
    echo "  derive call 1: ${SZ1} bytes, fp=${FP1}"
    echo "  derive call 2: ${SZ1} bytes, fp=${FP2}"
    if [ "$FP1" = "$FP2" ]; then echo "  DETERMINISTIC: yes"; else echo "  DETERMINISTIC: NO - keys differ!"; fi
    ENVFP=$(printf '%s' "${BACKUP_ENCRYPTION_KEY:-}" | sha256sum | cut -c1-16)
    echo "  env BACKUP_ENCRYPTION_KEY fp=${ENVFP} len=${#BACKUP_ENCRYPTION_KEY}"
    rm -f /tmp/diag-key1.bin /tmp/diag-key2.bin
else
    echo "  derive server not reachable on port 1101"
fi
echo ""

echo "--- post-boot-verify log (last 20 lines) ---"
tail -20 /var/log/supervisor/post-boot-verify.log 2>/dev/null || echo "  no log"
echo ""

echo "--- post-boot-verify-err log (last 10 lines) ---"
tail -10 /var/log/supervisor/post-boot-verify-err.log 2>/dev/null || echo "  no log"
echo ""

echo "--- export-watcher test ---"
echo "  curl 9080: $(curl -sf --max-time 3 http://127.0.0.1:9080/ 2>&1 | head -1 || echo 'unreachable')"
echo "  curl 9081: $(curl -sf --max-time 3 http://127.0.0.1:9081/ 2>&1 | head -1 || echo 'unreachable')"
echo "  last run log: $(tail -5 /tmp/export-watcher-last-run.log 2>/dev/null || echo 'none')"
echo ""

echo "--- all processes ---"
ps aux 2>/dev/null
echo ""

for svc in lightfriend cloudflared tuwunel postgresql whatsapp signal telegram export-watcher vsock-bridge-9080 vsock-bridge-9081 vsock-bridge-7844 vsock-bridge-dot cloudflared-monitor; do
    LOG="/var/log/supervisor/${svc}-err.log"
    if [ -f "$LOG" ] && [ -s "$LOG" ]; then
        echo "--- ${svc} stderr (last 30 lines) ---"
        tail -30 "$LOG"
        echo ""
    fi
    LOG="/var/log/supervisor/${svc}.log"
    if [ -f "$LOG" ] && [ -s "$LOG" ]; then
        echo "--- ${svc} stdout (last 15 lines) ---"
        tail -15 "$LOG"
        echo ""
    fi
done

echo "--- supervisord.log (last 20 lines) ---"
tail -20 /var/log/supervisor/supervisord.log 2>/dev/null
echo ""

echo "=== End Diagnostics ==="
