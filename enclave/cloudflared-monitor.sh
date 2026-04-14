#!/bin/bash
# Continuous monitoring of cloudflared edge connections through VSOCK bridge.
# Runs every 15 seconds, logs connection states, bridge health, and any anomalies.
# This is the primary diagnostic tool for the VSOCK/cloudflared connectivity issue.

LOG="/var/log/supervisor/cloudflared-monitor-detail.log"
ITERATION=0

rotate_log_if_needed() {
    if [ -f "$LOG" ] && [ "$(stat -c%s "$LOG" 2>/dev/null || echo 0)" -gt 1048576 ]; then
        tail -c 524288 "$LOG" > "${LOG}.tmp" 2>/dev/null && mv "${LOG}.tmp" "$LOG"
    fi
}

log() { echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] [iter=$ITERATION] $*" >> "$LOG"; }

echo "" >> "$LOG"
log "=== cloudflared-monitor started (PID $$) ==="

while true; do
    ITERATION=$((ITERATION + 1))
    rotate_log_if_needed

    # ── Bridge 7844 status ──
    BRIDGE_7844_LISTEN=$(ss -tlnp 2>/dev/null | grep ':7844' | head -1)
    BRIDGE_7844_CONNS=$(ss -tnp 2>/dev/null | grep -c ':7844')
    if [ -n "$BRIDGE_7844_LISTEN" ]; then
        log "bridge-7844: LISTENING, active_conns=$BRIDGE_7844_CONNS"
    else
        log "bridge-7844: NOT LISTENING! Connections will fail!"
        log "  supervisorctl: $(supervisorctl status vsock-bridge-7844 2>&1)"
        # Try to restart it
        supervisorctl restart vsock-bridge-7844 >> "$LOG" 2>&1 || true
    fi

    # ── Bridge 853 (DoT) status ──
    BRIDGE_DOT_LISTEN=$(ss -tlnp 2>/dev/null | grep ':853' | head -1)
    if [ -n "$BRIDGE_DOT_LISTEN" ]; then
        log "bridge-dot: LISTENING"
    else
        log "bridge-dot: NOT LISTENING!"
        supervisorctl restart vsock-bridge-dot >> "$LOG" 2>&1 || true
    fi

    # ── TCP connection states for port 7844 ──
    TCP_STATES=$(ss -tn 2>/dev/null | grep ':7844' | awk '{print $1}' | sort | uniq -c | tr '\n' ' ')
    if [ -n "$TCP_STATES" ]; then
        log "tcp-7844-states: $TCP_STATES"
    else
        log "tcp-7844-states: no connections"
    fi

    # ── Cloudflared process status ──
    CF_PID=$(pgrep -f 'cloudflared tunnel' 2>/dev/null | head -1)
    if [ -n "$CF_PID" ]; then
        CF_RSS=$(ps -o rss= -p "$CF_PID" 2>/dev/null | tr -d ' ')
        CF_UPTIME=$(ps -o etime= -p "$CF_PID" 2>/dev/null | tr -d ' ')
        log "cloudflared: pid=$CF_PID rss=${CF_RSS}KB uptime=$CF_UPTIME"
    else
        log "cloudflared: NOT RUNNING!"
        log "  supervisorctl: $(supervisorctl status cloudflared 2>&1)"
    fi

    # ── socat child processes (forked connections) ──
    SOCAT_7844_CHILDREN=$(pgrep -f 'socat.*7844' 2>/dev/null | wc -l)
    SOCAT_DOT_CHILDREN=$(pgrep -f 'socat.*853\|socat.*8530' 2>/dev/null | wc -l)
    log "socat-children: 7844=$SOCAT_7844_CHILDREN dot=$SOCAT_DOT_CHILDREN"

    # ── Every 4th iteration (60s), do a deeper check ──
    if [ $((ITERATION % 4)) -eq 0 ]; then
        log "--- deep check (every 60s) ---"

        # Full TCP state dump for 7844
        log "  full tcp-7844 dump:"
        ss -tnp 2>/dev/null | grep ':7844' >> "$LOG" 2>/dev/null || log "    none"

        # Check if cloudflared stderr has recent errors
        CF_ERRORS=$(tail -5 /var/log/supervisor/cloudflared-err.log 2>/dev/null | grep -i 'error\|fail\|cancel\|EOF\|reset\|closed' | tail -3)
        if [ -n "$CF_ERRORS" ]; then
            log "  cloudflared-recent-errors:"
            echo "$CF_ERRORS" >> "$LOG"
        fi

        # Check cloudflared stdout for connection events
        CF_EVENTS=$(tail -10 /var/log/supervisor/cloudflared.log 2>/dev/null | grep -i 'register\|connect\|disconnect\|error\|retry\|edge\|quic\|h2' | tail -5)
        if [ -n "$CF_EVENTS" ]; then
            log "  cloudflared-recent-events:"
            echo "$CF_EVENTS" >> "$LOG"
        fi

        # Test TCP connectivity through bridge
        if timeout 5 bash -c "echo TEST | socat -T3 - TCP:127.0.0.1:7844,connect-timeout=3" > /dev/null 2>&1; then
            log "  tcp-7844-test: PASS"
        else
            log "  tcp-7844-test: FAIL (rc=$?)"
        fi

        # VSOCK device check
        if [ -e /dev/vsock ]; then
            log "  /dev/vsock: present"
        else
            log "  /dev/vsock: MISSING!"
        fi

        # All socat processes with details
        log "  all-socat-processes:"
        pgrep -a socat >> "$LOG" || log "    none"

        log "--- end deep check ---"
    fi

    # ── Every 20th iteration (5min), dump cloudflared log tail ──
    if [ $((ITERATION % 20)) -eq 0 ]; then
        log "--- cloudflared log dump (every 5min) ---"
        log "  === cloudflared stdout (last 30 lines) ==="
        tail -30 /var/log/supervisor/cloudflared.log 2>/dev/null >> "$LOG" || true
        log "  === cloudflared stderr (last 30 lines) ==="
        tail -30 /var/log/supervisor/cloudflared-err.log 2>/dev/null >> "$LOG" || true
        log "--- end log dump ---"
    fi

    sleep 15
done
