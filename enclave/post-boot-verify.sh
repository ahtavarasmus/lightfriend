#!/bin/bash
# Post-boot verify runner. Launched by supervisord after all services start.
# Finds and executes the startup script created by entrypoint.sh.
echo "=== post-boot-verify.sh started at $(date -u) ==="
echo "Sleeping 30s for services to initialize..."
sleep 30

for script in /tmp/start-and-verify.sh /tmp/start-and-signal.sh; do
    if [ -x "$script" ]; then
        echo "Found startup script: $script"
        exec "$script"
    fi
done

echo "WARNING: No startup script found in /tmp/"
ls -la /tmp/start-*.sh 2>&1
