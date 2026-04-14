#!/bin/bash
# Watchdog: monitors service health and restarts stuck services.
#
# Two checks:
# 1. STOPPED state detection: if export.sh crashes mid-export, services
#    stay STOPPED forever. Restart after 2 minutes.
# 2. Backend health: if the backend process is running but not responding
#    to health checks (Tokio runtime frozen), restart it.
# 3. Storage health: if temp/log/data space is low, clean expendable files
#    before Postgres starts failing writes.
# 4. Memory health: track per-service RSS growth and warn if a process is on
#    pace to exhaust available memory within the configured horizon.

STOPPED_COUNT=0
STOPPED_THRESHOLD=4  # 4 checks * 30s = 2 minutes

UNHEALTHY_COUNT=0
UNHEALTHY_THRESHOLD=3  # 3 consecutive failures * 30s = 90 seconds before restart

PORT="${PORT:-3100}"
STORAGE_WARN_COUNT=0
STORAGE_WARN_THRESHOLD=2
MEMORY_WARN_COUNT=0
MEMORY_WARN_THRESHOLD=2

storage_report() {
    if [ -x /app/storage-health.sh ]; then
        /app/storage-health.sh report 2>&1
    else
        df -h 2>&1 || true
        df -i 2>&1 || true
    fi
}

memory_report() {
    if [ -x /app/memory-health.sh ]; then
        /app/memory-health.sh report 2>&1
    else
        free -h 2>&1 || true
        ps -eo pid,ppid,rss,comm,args --sort=-rss 2>/dev/null | head -25 || true
    fi
}

while true; do
    sleep 30

    STATUS=$(supervisorctl status 2>/dev/null || echo "")

    # ── Check 0: Storage pressure ──
    if [ -x /app/storage-health.sh ] && ! /app/storage-health.sh check >/tmp/storage-health-check.log 2>&1; then
        STORAGE_WARN_COUNT=$((STORAGE_WARN_COUNT + 1))
        echo "WATCHDOG: Storage health check failed ($STORAGE_WARN_COUNT/$STORAGE_WARN_THRESHOLD)"
        cat /tmp/storage-health-check.log 2>/dev/null || true
        if [ "$STORAGE_WARN_COUNT" -ge "$STORAGE_WARN_THRESHOLD" ]; then
            echo "WATCHDOG: Running storage cleanup"
            /app/storage-health.sh cleanup 2>&1 || true
            STORAGE_WARN_COUNT=0
        fi
    else
        STORAGE_WARN_COUNT=0
    fi

    # ── Check 0b: Memory pressure / growth trend ──
    if [ -x /app/memory-health.sh ]; then
        if ! /app/memory-health.sh check >/tmp/memory-health-check.log 2>&1; then
            MEMORY_WARN_COUNT=$((MEMORY_WARN_COUNT + 1))
            echo "WATCHDOG: Memory growth check failed ($MEMORY_WARN_COUNT/$MEMORY_WARN_THRESHOLD)"
            cat /tmp/memory-health-check.log 2>/dev/null || true
            if [ "$MEMORY_WARN_COUNT" -ge "$MEMORY_WARN_THRESHOLD" ]; then
                echo "WATCHDOG: Memory growth risk persisted. Full memory report:"
                memory_report
                MEMORY_WARN_COUNT=0
            fi
        else
            MEMORY_WARN_COUNT=0
        fi
    fi

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
                echo "WATCHDOG: storage report before restart:"
                storage_report
                echo "WATCHDOG: memory report before restart:"
                memory_report
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
