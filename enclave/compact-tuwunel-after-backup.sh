#!/bin/bash
# Request one bounded RocksDB compaction after a verified hourly S3 export.
# Compaction is skipped unless rootfs has room for a full second Tuwunel copy
# plus an additional safety margin.

set -uo pipefail

if [ -f /etc/lightfriend/env ]; then
    while IFS= read -r line || [[ -n "$line" ]]; do
        [[ -z "$line" || "$line" == \#* ]] && continue
        if [[ "$line" =~ ^[A-Za-z_][A-Za-z_0-9]*= ]]; then
            export "${line?}"
        fi
    done < /etc/lightfriend/env
fi

ENABLED="${TUWUNEL_COMPACTION_AFTER_BACKUP_ENABLED:-true}"
COOLDOWN_SECS="${TUWUNEL_COMPACTION_COOLDOWN_SECS:-86400}"
SAFETY_BYTES="${TUWUNEL_COMPACTION_SAFETY_BYTES:-134217728}"
MAX_BACKUP_AGE_SECS="${TUWUNEL_COMPACTION_MAX_BACKUP_AGE_SECS:-600}"
STATUS_FILE="${TUWUNEL_COMPACTION_STATUS_FILE:-/data/seed/tuwunel-compaction-status.json}"
BACKUP_STATUS_FILE="${TUWUNEL_BACKUP_STATUS_FILE:-/data/seed/export-status.json}"
BACKUP_LOCK="${LIGHTFRIEND_BACKUP_ARTIFACT_LOCK_FILE:-/tmp/lightfriend-backup-artifacts.lock}"
COMPACTION_LOCK="${TUWUNEL_COMPACTION_LOCK_FILE:-/tmp/tuwunel-compaction.lock}"
PORT="${PORT:-3100}"

case "$(printf '%s' "$ENABLED" | tr '[:upper:]' '[:lower:]')" in
    1|true|yes|on) ;;
    *) echo "tuwunel-compaction: disabled"; exit 0 ;;
esac

command -v jq >/dev/null 2>&1 || {
    echo "tuwunel-compaction: jq unavailable; skipping"
    exit 0
}
command -v flock >/dev/null 2>&1 || {
    echo "tuwunel-compaction: flock unavailable; skipping"
    exit 0
}

if [ "${EXPORT_TYPE:-}" != "hourly" ]; then
    echo "tuwunel-compaction: export type ${EXPORT_TYPE:-unknown} is not hourly; skipping"
    exit 0
fi
if [ -z "${MAINTENANCE_SECRET:-}" ]; then
    echo "tuwunel-compaction: MAINTENANCE_SECRET unavailable; skipping"
    exit 0
fi
if ! jq -e '.status == "SUCCESS" and .s3_uploaded == true and .archive_verified == true' \
    "$BACKUP_STATUS_FILE" >/dev/null 2>&1; then
    echo "tuwunel-compaction: latest local export status is not verified S3 success; skipping"
    exit 0
fi

now=$(date +%s)
backup_epoch=$(jq -r '.completed_epoch // 0' "$BACKUP_STATUS_FILE" 2>/dev/null || echo 0)
backup_age=$((now - backup_epoch))
if [ "$backup_epoch" -le 0 ] || [ "$backup_age" -lt 0 ] || [ "$backup_age" -gt "$MAX_BACKUP_AGE_SECS" ]; then
    echo "tuwunel-compaction: verified backup is ${backup_age}s old; skipping"
    exit 0
fi

exec 8>"$COMPACTION_LOCK" || exit 0
if ! flock -n 8; then
    echo "tuwunel-compaction: another compaction request is active; skipping"
    exit 0
fi
exec 9>"$BACKUP_LOCK" || exit 0
if ! flock -n 9; then
    echo "tuwunel-compaction: export artifact lock is busy; skipping"
    exit 0
fi

last_requested=$(jq -r 'if .status == "accepted" then (.requested_epoch // 0) else 0 end' \
    "$STATUS_FILE" 2>/dev/null || echo 0)
if [ "$last_requested" -gt 0 ] && [ $((now - last_requested)) -lt "$COOLDOWN_SECS" ]; then
    echo "tuwunel-compaction: cooldown active; skipping"
    exit 0
fi

tuwunel_bytes=$(du -sb /var/lib/tuwunel 2>/dev/null | awk '{print $1}' || echo 0)
root_avail_bytes=$(df -Pk / 2>/dev/null | awk 'NR == 2 {print $4 * 1024}' || echo 0)
required_bytes=$((tuwunel_bytes + SAFETY_BYTES))
if [ "$root_avail_bytes" -lt "$required_bytes" ]; then
    echo "tuwunel-compaction: insufficient scratch root_avail_bytes=${root_avail_bytes} required_bytes=${required_bytes} tuwunel_bytes=${tuwunel_bytes}; skipping"
    jq -n \
        --arg status "skipped_insufficient_space" \
        --argjson checked_epoch "$now" \
        --argjson root_avail_bytes "$root_avail_bytes" \
        --argjson required_bytes "$required_bytes" \
        --argjson tuwunel_bytes "$tuwunel_bytes" \
        '{status:$status,checked_epoch:$checked_epoch,root_avail_bytes:$root_avail_bytes,required_bytes:$required_bytes,tuwunel_bytes:$tuwunel_bytes}' \
        > "${STATUS_FILE}.tmp" && mv "${STATUS_FILE}.tmp" "$STATUS_FILE"
    exit 0
fi

echo "tuwunel-compaction: requesting exhaustive single-column compaction after verified backup"
response=$(curl -sf --max-time 30 -X POST \
    -H "X-Maintenance-Secret: $MAINTENANCE_SECRET" \
    "http://localhost:${PORT}/api/internal/tuwunel/compact" 2>&1) || {
    error=$(printf '%s' "$response" | cut -c1-1000)
    jq -n \
        --arg status "request_failed" \
        --arg error "$error" \
        --argjson requested_epoch "$now" \
        --argjson root_avail_bytes "$root_avail_bytes" \
        --argjson tuwunel_bytes "$tuwunel_bytes" \
        '{status:$status,requested_epoch:$requested_epoch,error:$error,root_avail_bytes:$root_avail_bytes,tuwunel_bytes:$tuwunel_bytes}' \
        > "${STATUS_FILE}.tmp" && mv "${STATUS_FILE}.tmp" "$STATUS_FILE"
    echo "tuwunel-compaction: request failed: $error"
    exit 0
}

jq -n \
    --arg status "accepted" \
    --argjson requested_epoch "$now" \
    --argjson root_avail_bytes "$root_avail_bytes" \
    --argjson tuwunel_bytes "$tuwunel_bytes" \
    --argjson response "$response" \
    '{status:$status,requested_epoch:$requested_epoch,root_avail_bytes:$root_avail_bytes,tuwunel_bytes:$tuwunel_bytes,response:$response}' \
    > "${STATUS_FILE}.tmp" && mv "${STATUS_FILE}.tmp" "$STATUS_FILE"
echo "tuwunel-compaction: request accepted"
