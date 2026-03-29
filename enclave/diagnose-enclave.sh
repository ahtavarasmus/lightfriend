#!/bin/bash
# Diagnostic script - runs inside the enclave when host connects to VSOCK port 9008.
# Dumps all supervisor service status, cloudflared bridge health, and recent logs.

echo "=== Enclave Diagnostics $(date -u +%Y-%m-%dT%H:%M:%SZ) ==="
echo ""

echo "--- supervisorctl status ---"
supervisorctl status 2>&1
echo ""

echo "--- env check ---"
echo "PG_DATABASE_URL: ${PG_DATABASE_URL:+set (${#PG_DATABASE_URL} chars)}"
echo "DATABASE_URL: ${DATABASE_URL:+set (${#DATABASE_URL} chars)}"
echo "PORT: ${PORT:-not set}"
echo "HTTP_PROXY: ${HTTP_PROXY:-not set}"
echo "CLOUDFLARE_TUNNEL_TOKEN: ${CLOUDFLARE_TUNNEL_TOKEN:+set (${#CLOUDFLARE_TUNNEL_TOKEN} chars)}"
echo ""

echo "--- postgres check ---"
pg_isready -h localhost -U postgres 2>&1 || echo "pg_isready failed"
echo ""

echo "--- backend health (2s timeout) ---"
timeout 2 curl -sf --max-time 1 --connect-timeout 1 http://localhost:${PORT:-3100}/api/health 2>&1 || echo "backend not responding (hung or crashed)"
echo ""

echo "--- tuwunel health (2s timeout) ---"
timeout 2 curl -sf --max-time 1 --connect-timeout 1 http://localhost:8008/_matrix/client/versions 2>&1 || echo "tuwunel not responding"
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
