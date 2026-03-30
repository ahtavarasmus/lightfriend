#!/bin/bash
# Watchdog: monitors service health and restarts stuck services.
#
# Two checks:
# 1. STOPPED state detection: if export.sh crashes mid-export, services
#    stay STOPPED forever. Restart after 2 minutes.
# 2. Backend health: if the backend process is running but not responding
#    to health checks (Tokio runtime frozen), restart it.

STOPPED_COUNT=0
STOPPED_THRESHOLD=4  # 4 checks * 30s = 2 minutes

UNHEALTHY_COUNT=0
UNHEALTHY_THRESHOLD=3  # 3 consecutive failures * 30s = 90 seconds before restart

PORT="${PORT:-3100}"

while true; do
    sleep 30

    STATUS=$(supervisorctl status 2>/dev/null || echo "")

    # ── Check 1: Services stuck in STOPPED state ──
    HAS_STOPPED=false
    for proc in tuwunel mautrix-whatsapp mautrix-signal mautrix-telegram lightfriend; do
        if echo "$STATUS" | grep -q "${proc}.*STOPPED"; then
            HAS_STOPPED=true
            break
        fi
    done

    if [ "$HAS_STOPPED" = "true" ]; then
        STOPPED_COUNT=$((STOPPED_COUNT + 1))
        if [ $STOPPED_COUNT -ge $STOPPED_THRESHOLD ]; then
            echo "WATCHDOG: Services stuck in STOPPED state for >2 minutes. Restarting..."

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

    # ── Check 2: Backend health (detect frozen Tokio runtime) ──
    # Only check if the backend process is RUNNING (not during startup/export)
    if echo "$STATUS" | grep -q "lightfriend.*RUNNING"; then
        # Try health check with strict 2-second timeout
        if curl -sf --max-time 2 --connect-timeout 1 "http://localhost:${PORT}/api/health" > /dev/null 2>&1; then
            UNHEALTHY_COUNT=0
        else
            UNHEALTHY_COUNT=$((UNHEALTHY_COUNT + 1))
            echo "WATCHDOG: Backend health check failed ($UNHEALTHY_COUNT/$UNHEALTHY_THRESHOLD)"

            if [ $UNHEALTHY_COUNT -ge $UNHEALTHY_THRESHOLD ]; then
                echo "WATCHDOG: Backend unresponsive for >90s - restarting lightfriend"
                echo "WATCHDOG: supervisorctl status before restart:"
                supervisorctl status 2>&1
                supervisorctl restart lightfriend 2>/dev/null || true
                echo "WATCHDOG: Backend restarted at $(date -u +%Y-%m-%dT%H:%M:%SZ)"
                UNHEALTHY_COUNT=0
                # Give it time to start up before checking again
                sleep 30
            fi
        fi
    else
        UNHEALTHY_COUNT=0
    fi
done
