#!/bin/bash
# Post-boot verify runner. Launched by supervisord after all services start.
# Finds and executes the startup script created by entrypoint.sh.
echo "=== post-boot-verify.sh started at $(date -u) ==="

echo "Contents of /tmp/ before sleep:"
ls -la /tmp/ 2>&1
echo ""

echo "Sleeping 30s for services to initialize..."
sleep 30

echo "Contents of /tmp/ after sleep:"
ls -la /tmp/ 2>&1
echo ""

for script in /tmp/start-and-verify.sh /tmp/start-and-signal.sh; do
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
[ "$BACKEND_READY" = "false" ] && echo "WARNING: Backend not ready after 300s"

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
    echo "Verify failed, waiting 30s before retry..."
    sleep 30
done
if [ -f /data/seed/verify-result.json ]; then
    echo "Uploading verify result..."
    cat /data/seed/verify-result.json
    curl -v --max-time 30 -T /data/seed/verify-result.json \
        "http://127.0.0.1:9081/upload/verify-result.json" 2>&1
fi
echo "=== post-boot-verify.sh finished at $(date -u) ==="
