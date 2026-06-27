#!/bin/bash
# Manually release the root filesystem reserve.
#
# This is intentionally not called by automatic cleanup. It is the emergency
# break-glass space buffer for cases where the live enclave needs just enough
# rootfs headroom to export and blue-green swap.

set -uo pipefail

RESERVE_FILE="${LIGHTFRIEND_ROOTFS_RESERVE_FILE:-/var/lib/lightfriend-reserve/rootfs-reserve.bin}"
STATUS_FILE="${LIGHTFRIEND_RESERVE_STATUS_FILE:-/data/seed/reserve-release-status.json}"
REQUESTED_AT="${1:-$(date -u +%Y-%m-%dT%H:%M:%SZ)}"

json_escape() {
    printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

root_avail_kb() {
    df -Pk / 2>/dev/null | awk 'NR==2 {print $4}'
}

write_status() {
    local status="$1"
    local released="$2"
    local bytes="$3"
    local before_kb="$4"
    local after_kb="$5"
    local message="$6"

    mkdir -p "$(dirname "$STATUS_FILE")"
    cat > "$STATUS_FILE" <<EOF
{"status":"$(json_escape "$status")","released":$released,"released_bytes":$bytes,"root_avail_kb_before":${before_kb:-0},"root_avail_kb_after":${after_kb:-0},"requested_at":"$(json_escape "$REQUESTED_AT")","finished_at":"$(date -u +%Y-%m-%dT%H:%M:%SZ)","message":"$(json_escape "$message")"}
EOF
}

echo "=== Release rootfs reserve ==="
echo "Reserve file: $RESERVE_FILE"

BEFORE_KB=$(root_avail_kb)

if [ ! -e "$RESERVE_FILE" ]; then
    AFTER_KB=$(root_avail_kb)
    echo "Reserve already released or missing"
    write_status "OK" false 0 "$BEFORE_KB" "$AFTER_KB" "reserve file already missing"
    exit 0
fi

RESERVE_BYTES=$(stat -c%s "$RESERVE_FILE" 2>/dev/null || echo 0)
echo "Releasing $RESERVE_BYTES bytes"

if rm -f "$RESERVE_FILE"; then
    sync 2>/dev/null || true
    AFTER_KB=$(root_avail_kb)
    echo "Reserve released; root avail ${BEFORE_KB:-unknown}KB -> ${AFTER_KB:-unknown}KB"
    write_status "OK" true "$RESERVE_BYTES" "$BEFORE_KB" "$AFTER_KB" "reserve released"
    exit 0
fi

AFTER_KB=$(root_avail_kb)
echo "Failed to delete reserve file"
write_status "FAILED" false 0 "$BEFORE_KB" "$AFTER_KB" "failed to delete reserve file"
exit 1
