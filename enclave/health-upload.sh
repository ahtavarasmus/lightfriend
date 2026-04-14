#!/bin/bash
# Persist compact enclave resource snapshots on the parent host.
#
# The enclave's /tmp and supervisor logs are ephemeral across enclave restarts.
# Upload only resource counters over the existing backup VSOCK bridge so the
# host keeps enough history to diagnose memory or disk growth after a restart.

set -uo pipefail

LATEST_FILE="${HEALTH_UPLOAD_LATEST_FILE:-/tmp/enclave-health-latest.txt}"
HISTORY_FILE="${HEALTH_UPLOAD_HISTORY_FILE:-/tmp/enclave-health-history.log}"
MAX_HISTORY_BYTES="${HEALTH_UPLOAD_MAX_HISTORY_BYTES:-1048576}"
UPLOAD_BASE="${HEALTH_UPLOAD_BASE:-http://127.0.0.1:9081/upload-log}"

rotate_history_if_needed() {
    if [ -f "$HISTORY_FILE" ] && [ "$(stat -c%s "$HISTORY_FILE" 2>/dev/null || echo 0)" -gt "$MAX_HISTORY_BYTES" ]; then
        tail -c $((MAX_HISTORY_BYTES / 2)) "$HISTORY_FILE" > "${HISTORY_FILE}.tmp" 2>/dev/null && mv "${HISTORY_FILE}.tmp" "$HISTORY_FILE"
    fi
}

write_snapshot() {
    {
        echo "=== Enclave Resource Snapshot $(date -u +%Y-%m-%dT%H:%M:%SZ) ==="
        echo "--- supervisor status ---"
        supervisorctl status 2>/dev/null || true
        echo ""
        echo "--- storage ---"
        if [ -x /app/storage-health.sh ]; then
            /app/storage-health.sh report 2>&1
        else
            df -h 2>&1 || true
            df -i 2>&1 || true
        fi
        echo ""
        echo "--- memory ---"
        if [ -x /app/memory-health.sh ]; then
            /app/memory-health.sh report 2>&1
        else
            free -h 2>&1 || true
            ps -eo pid,ppid,rss,comm,args --sort=-rss 2>/dev/null | head -25 || true
        fi
    } > "$LATEST_FILE"

    {
        cat "$LATEST_FILE"
        echo ""
    } >> "$HISTORY_FILE"
    rotate_history_if_needed
}

upload_file() {
    local src="$1"
    local name="$2"
    [ -s "$src" ] || return 0
    curl -sf --max-time 10 --connect-timeout 2 -T "$src" "${UPLOAD_BASE}/${name}" >/dev/null
}

write_snapshot
upload_file "$LATEST_FILE" "enclave-health-latest.txt"
upload_file "$HISTORY_FILE" "enclave-health-history.log"
