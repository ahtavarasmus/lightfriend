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
curl -sf --max-time 3 http://localhost:3000/api/health 2>&1 || echo "backend not responding"
echo ""

echo "--- tuwunel health ---"
curl -sf --max-time 3 http://localhost:8008/_matrix/client/versions 2>&1 || echo "tuwunel not responding"
echo ""

echo "--- network ---"
echo "lo: $(ip addr show lo 2>/dev/null | grep 'inet ' | head -3)"
echo "port 3000: $(ss -tlnp 2>/dev/null | grep ':3000' || echo 'not listening')"
echo "port 8008: $(ss -tlnp 2>/dev/null | grep ':8008' || echo 'not listening')"
echo "port 7844: $(ss -tlnp 2>/dev/null | grep ':7844' || echo 'not listening')"
echo ""

for svc in lightfriend cloudflared tuwunel postgresql whatsapp signal telegram; do
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
