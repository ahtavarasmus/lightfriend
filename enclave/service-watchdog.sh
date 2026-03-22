#!/bin/bash
# Watchdog: detects services stuck in STOPPED state and restarts them.
#
# export.sh stops services with `supervisorctl stop` for consistent snapshots.
# If export.sh crashes without its cleanup trap firing (SIGKILL, OOM),
# services stay STOPPED forever because supervisord only auto-restarts
# EXITED processes, not STOPPED ones.
#
# This watchdog checks every 30 seconds. If services have been STOPPED
# for more than 2 minutes (4 consecutive checks), it restarts them.
# The 2-minute grace period avoids interfering with active exports.

STOPPED_COUNT=0
THRESHOLD=4  # 4 checks * 30s = 2 minutes

while true; do
    sleep 30

    # Check if any core service is STOPPED (not RUNNING, not STARTING, not EXITED)
    STATUS=$(supervisorctl status 2>/dev/null || echo "")
    HAS_STOPPED=false

    for proc in tuwunel mautrix-whatsapp mautrix-signal mautrix-telegram lightfriend; do
        if echo "$STATUS" | grep -q "${proc}.*STOPPED"; then
            HAS_STOPPED=true
            break
        fi
    done

    if [ "$HAS_STOPPED" = "true" ]; then
        STOPPED_COUNT=$((STOPPED_COUNT + 1))
        if [ $STOPPED_COUNT -ge $THRESHOLD ]; then
            echo "WATCHDOG: Services stuck in STOPPED state for >2 minutes. Restarting..."

            # Check if export.sh is actually running (don't interfere with active export)
            if pgrep -f "export.sh" > /dev/null 2>&1; then
                echo "WATCHDOG: export.sh is still running - not restarting"
                STOPPED_COUNT=0
                continue
            fi

            supervisorctl start tuwunel 2>/dev/null || true
            sleep 2
            supervisorctl start mautrix-whatsapp 2>/dev/null || true
            supervisorctl start mautrix-signal 2>/dev/null || true
            supervisorctl start mautrix-telegram 2>/dev/null || true
            sleep 1
            supervisorctl start lightfriend 2>/dev/null || true

            echo "WATCHDOG: Services restarted"
            STOPPED_COUNT=0
        fi
    else
        STOPPED_COUNT=0
    fi
done
