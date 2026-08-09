#!/bin/bash
set -uo pipefail

LOG_DIR="/opt/lightfriend/logs"
RESTORE_FAILED_DIR="/opt/lightfriend/restore/failed"
mkdir -p "$LOG_DIR" "$RESTORE_FAILED_DIR"

cap_file() {
    local file="$1"
    local max_bytes="$2"
    local keep_bytes="$3"
    [ -f "$file" ] || return 0
    local size
    size=$(stat -c%s "$file" 2>/dev/null || echo 0)
    if [ "$size" -gt "$max_bytes" ]; then
        tail -c "$keep_bytes" "$file" > "$file.tmp" 2>/dev/null && cat "$file.tmp" > "$file"
        rm -f "$file.tmp" 2>/dev/null || true
    fi
}

for f in \
    "$LOG_DIR/gvproxy.log" \
    "$LOG_DIR/gvproxy-err.log" \
    "$LOG_DIR/scheduled-backup.log" \
    "$LOG_DIR/cloudflared-edge-stdout.log" \
    "$LOG_DIR/cloudflared-edge-stderr.log" \
    "$LOG_DIR/telegram-proxy-bridge.log" \
    "$LOG_DIR/config-server.log" \
    "$LOG_DIR/dot-bridge.log"; do
    cap_file "$f" 5242880 1048576
done

cap_file /tmp/restore-enclave-debug.log 5242880 1048576
cap_file /tmp/launch.log 5242880 1048576
cap_file /tmp/eif-download.log 2097152 524288

# Keep recent boot traces and failed encrypted restore artifacts for diagnosis.
find "$LOG_DIR" -maxdepth 1 -type f -name 'boot-trace-*.log' -mtime +7 -delete 2>/dev/null || true
find "$LOG_DIR" -maxdepth 1 -type f -name 'boot-trace-*.log' -printf '%T@ %p\n' 2>/dev/null \
    | sort -rn | tail -n +11 | cut -d' ' -f2- | xargs -r rm -f
find "$RESTORE_FAILED_DIR" -type f -mtime +3 -delete 2>/dev/null || true
find "$RESTORE_FAILED_DIR" -maxdepth 1 -type f -printf '%T@ %p\n' 2>/dev/null \
    | sort -rn | tail -n +4 | cut -d' ' -f2- | xargs -r rm -f
