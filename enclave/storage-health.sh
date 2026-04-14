#!/bin/bash
# Storage diagnostics and cleanup for the enclave.
#
# Postgres can fail with "could not write init file" when the enclave's
# writable filesystem, temp space, or inodes are exhausted. Keep this script
# dependency-light so watchdogs can run it even during partial failures.

set -uo pipefail

MIN_FREE_KB="${MIN_FREE_KB:-262144}"          # 256 MiB
MIN_FREE_INODES="${MIN_FREE_INODES:-1024}"
HISTORY_FILE="${STORAGE_HEALTH_HISTORY_FILE:-/tmp/storage-health-history.log}"
MAX_HISTORY_BYTES="${STORAGE_MAX_HISTORY_BYTES:-1048576}"
PATHS="/ /tmp /var/lib/postgresql /var/log /data/seed /var/lib/tuwunel /app"

rotate_history_if_needed() {
    if [ -f "$HISTORY_FILE" ] && [ "$(stat -c%s "$HISTORY_FILE" 2>/dev/null || echo 0)" -gt "$MAX_HISTORY_BYTES" ]; then
        tail -c $((MAX_HISTORY_BYTES / 2)) "$HISTORY_FILE" > "${HISTORY_FILE}.tmp" 2>/dev/null && mv "${HISTORY_FILE}.tmp" "$HISTORY_FILE"
    fi
}

print_report() {
    echo "=== Storage Health $(date -u +%Y-%m-%dT%H:%M:%SZ) ==="
    echo "--- df -h ---"
    df -h ${PATHS} 2>/dev/null || df -h 2>/dev/null || echo "df unavailable"
    echo "--- df -i ---"
    df -i ${PATHS} 2>/dev/null || df -i 2>/dev/null || echo "df -i unavailable"
    echo "--- top writable dirs ---"
    du -xh -d 2 /tmp /var/log /data/seed /var/lib/postgresql /var/lib/tuwunel /app/data /app/uploads /app/matrix_store 2>/dev/null \
        | sort -h | tail -40 || true
    echo "--- large supervisor logs ---"
    find /var/log/supervisor -type f -size +1M -printf '%s %p\n' 2>/dev/null | sort -n | tail -20 || true
    echo "--- local backup artifacts ---"
    find /data/seed /tmp -maxdepth 2 -type f \( -name 'lightfriend-full-backup-*.tar.gz.enc' -o -name 'lightfriend-full-backup-*.tar.gz' -o -name 'verify.tar.gz' \) \
        -printf '%TY-%Tm-%TdT%TH:%TM:%TSZ %s %p\n' 2>/dev/null | sort | tail -30 || true
}

check_path() {
    local path="$1"
    [ -e "$path" ] || return 0

    local avail_kb avail_inodes
    avail_kb=$(df -Pk "$path" 2>/dev/null | awk 'NR==2 {print $4}')
    avail_inodes=$(df -Pi "$path" 2>/dev/null | awk 'NR==2 {print $4}')

    if [ -n "$avail_kb" ] && [ "$avail_kb" -lt "$MIN_FREE_KB" ]; then
        echo "LOW_SPACE path=$path avail_kb=$avail_kb threshold_kb=$MIN_FREE_KB"
        return 1
    fi

    if [ -n "$avail_inodes" ] && [ "$avail_inodes" -lt "$MIN_FREE_INODES" ]; then
        echo "LOW_INODES path=$path avail_inodes=$avail_inodes threshold=$MIN_FREE_INODES"
        return 1
    fi

    return 0
}

check_storage() {
    local rc=0
    rotate_history_if_needed
    print_report >> "$HISTORY_FILE" 2>&1
    for path in /tmp /var/lib/postgresql /var/log /data/seed; do
        check_path "$path" || rc=1
    done
    return "$rc"
}

cleanup_storage() {
    echo "=== Storage Cleanup $(date -u +%Y-%m-%dT%H:%M:%SZ) ==="

    rm -rf /tmp/backup-staging /tmp/verify.tar.gz /tmp/lightfriend-full-backup-*.tar.gz 2>/dev/null || true
    find /data/seed -name 'lightfriend-full-backup-*.tar.gz.enc' -mmin +30 -delete 2>/dev/null || true

    # The tunnel and bridge logs can grow quickly during scans/outages. Keep
    # recent logs, but cap any single supervisor log to the last 1 MiB.
    find /var/log/supervisor -type f -size +5M -print 2>/dev/null | while read -r log; do
        tmp="${log}.tmp"
        tail -c 1048576 "$log" > "$tmp" 2>/dev/null && cat "$tmp" > "$log"
        rm -f "$tmp" 2>/dev/null || true
    done

    find /var/log/supervisor -type f \( -name '*.1' -o -name '*.2' -o -name '*.3' \) -mtime +1 -delete 2>/dev/null || true
    print_report
}

case "${1:-report}" in
    report)
        print_report
        ;;
    check)
        check_storage
        ;;
    cleanup)
        cleanup_storage
        ;;
    *)
        echo "Usage: $0 [report|check|cleanup]" >&2
        exit 2
        ;;
esac
