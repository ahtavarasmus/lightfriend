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

echo "FATAL: No startup script found. Running verify directly as fallback..."
# Fallback: wait for backend, run verify, upload result
BACKEND_PORT="${PORT:-3100}"
for i in $(seq 1 60); do
    curl -sf --max-time 3 "http://localhost:${BACKEND_PORT}/api/health" > /dev/null 2>&1 && break
    [ $((i % 12)) -eq 0 ] && echo "  Waiting for backend ($((i * 5))s)..."
    sleep 5
done
/app/verify.sh || echo "verify.sh failed with rc=$?"
if [ -f /data/seed/verify-result.json ]; then
    echo "Uploading verify result..."
    cat /data/seed/verify-result.json
    curl -v --max-time 30 -T /data/seed/verify-result.json \
        "http://127.0.0.1:9081/upload/verify-result.json" 2>&1
fi
echo "=== post-boot-verify.sh finished at $(date -u) ==="
