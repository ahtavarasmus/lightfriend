#!/bin/bash
# Diagnostic script - runs inside the enclave when host connects to VSOCK port 9008.
# Dumps all supervisor service status and recent logs.

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

echo "--- backend health ---"
curl -sf --max-time 3 http://localhost:${PORT:-3100}/api/health 2>&1 || echo "backend not responding"
echo ""

echo "--- tuwunel health ---"
curl -sf --max-time 3 http://localhost:8008/_matrix/client/versions 2>&1 || echo "tuwunel not responding"
echo ""

echo "--- network ---"
echo "lo: $(ip addr show lo 2>/dev/null | grep 'inet ' | head -3)"
echo "port ${PORT:-3100}: $(ss -tlnp 2>/dev/null | grep ':${PORT:-3100}' || echo 'not listening')"
echo "port 8008: $(ss -tlnp 2>/dev/null | grep ':8008' || echo 'not listening')"
echo "port 7844: $(ss -tlnp 2>/dev/null | grep ':7844' || echo 'not listening')"
echo ""

echo "port 9080: $(ss -tlnp 2>/dev/null | grep ':9080' || echo 'not listening')"
echo "port 9081: $(ss -tlnp 2>/dev/null | grep ':9081' || echo 'not listening')"
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

echo "--- cloudflared-diag log ---"
cat /var/log/supervisor/cloudflared-diag.log 2>/dev/null || echo "  no diag log"
echo ""

echo "--- export-watcher test ---"
echo "  curl 9080: $(curl -sf --max-time 3 http://127.0.0.1:9080/ 2>&1 | head -1 || echo 'unreachable')"
echo "  curl 9081: $(curl -sf --max-time 3 http://127.0.0.1:9081/ 2>&1 | head -1 || echo 'unreachable')"
echo "  last run log: $(tail -5 /tmp/export-watcher-last-run.log 2>/dev/null || echo 'none')"
echo ""

for svc in lightfriend cloudflared tuwunel postgresql whatsapp signal telegram export-watcher vsock-bridge-9080 vsock-bridge-9081; do
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
