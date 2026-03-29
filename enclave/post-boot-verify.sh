#!/bin/bash
# Post-boot verify runner. Launched by supervisord after all services start.
# Finds and executes the startup script created by entrypoint.sh.
echo "=== post-boot-verify.sh started at $(date -u) ==="

echo "Sleeping 30s for services to initialize..."
sleep 30

echo "Checking /data/seed/ for startup scripts:"
ls -la /data/seed/start-*.sh 2>&1 || echo "  none found"
echo ""

for script in /data/seed/start-and-verify.sh /data/seed/start-and-signal.sh; do
    echo "Checking $script: exists=$([ -f "$script" ] && echo yes || echo no) executable=$([ -x "$script" ] && echo yes || echo no)"
    if [ -x "$script" ]; then
        echo "Executing: $script"
        exec "$script"
    fi
done

echo "No startup script found. Running verify directly as fallback..."
# Fallback: wait for backend, run verify, upload result
BACKEND_PORT="${PORT:-3100}"
BACKEND_READY=false
for i in $(seq 1 60); do
    if curl -sf --max-time 3 "http://localhost:${BACKEND_PORT}/api/health" > /dev/null 2>&1; then
        echo "Backend healthy after $((i * 5))s"
        BACKEND_READY=true
        break
    fi
    [ $((i % 12)) -eq 0 ] && echo "  Waiting for backend ($((i * 5))s)..."
    sleep 5
done
if [ "$BACKEND_READY" = "false" ]; then
    echo "WARNING: Backend not ready after 300s"
    echo "=== supervisorctl status ==="
    supervisorctl status 2>&1
    echo "=== lightfriend stderr (last 30 lines) ==="
    tail -30 /var/log/supervisor/lightfriend-err.log 2>/dev/null
    echo "=== lightfriend stdout (last 30 lines) ==="
    tail -30 /var/log/supervisor/lightfriend.log 2>/dev/null
    echo "=== postgresql stderr (last 10 lines) ==="
    tail -10 /var/log/supervisor/postgresql-err.log 2>/dev/null
fi

# Ensure cloudflared is running
if [ -n "${CLOUDFLARE_TUNNEL_TOKEN:-}" ]; then
    supervisorctl start cloudflared 2>/dev/null || true
    for i in $(seq 1 15); do
        supervisorctl status cloudflared 2>/dev/null | grep -q RUNNING && break
        sleep 2
    done
fi

# Run verify, retry up to 3 times if it fails (backend may still be starting)
VERIFY_RC=1
for attempt in 1 2 3; do
    echo "Running verify.sh (attempt $attempt/3)..."
    /app/verify.sh
    VERIFY_RC=$?
    echo "verify.sh exited with rc=$VERIFY_RC"
    [ $VERIFY_RC -eq 0 ] && break
    echo "Verify failed. Backend logs:"
    tail -20 /var/log/supervisor/lightfriend-err.log 2>/dev/null
    tail -10 /var/log/supervisor/lightfriend.log 2>/dev/null
    echo "Waiting 30s before retry..."
    sleep 30
done
if [ -f /data/seed/verify-result.json ]; then
    # If verify failed, inject backend error logs into the JSON for debugging
    if [ $VERIFY_RC -ne 0 ]; then
        BACKEND_ERR=$(tail -20 /var/log/supervisor/lightfriend-err.log 2>/dev/null | tr '\n' '|' | sed 's/"/\\"/g' | cut -c1-500)
        python3 -c "
import json
with open('/data/seed/verify-result.json') as f:
    data = json.load(f)
data['backend_error'] = '''${BACKEND_ERR}'''
with open('/data/seed/verify-result.json', 'w') as f:
    json.dump(data, f)
" 2>/dev/null || true
    fi
    echo "Uploading verify result..."
    cat /data/seed/verify-result.json
    curl -v --max-time 30 -T /data/seed/verify-result.json \
        "http://127.0.0.1:9081/upload/verify-result.json" 2>&1
fi
echo "=== post-boot-verify.sh finished at $(date -u) ==="
