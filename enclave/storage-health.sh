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
SNAPSHOT_FILE="${STORAGE_HEALTH_SNAPSHOT_FILE:-/tmp/storage-health-snapshot.tsv}"
TOP_DIR_LINES="${STORAGE_HEALTH_TOP_DIR_LINES:-80}"
TOP_FILE_LINES="${STORAGE_HEALTH_TOP_FILE_LINES:-40}"
LARGE_FILE_MIN_KB="${STORAGE_HEALTH_LARGE_FILE_MIN_KB:-1024}"
GROWTH_REPORT_MIN_KB="${STORAGE_HEALTH_GROWTH_REPORT_MIN_KB:-10240}"

WATCH_PATHS=(
    /
    /tmp
    /var
    /var/lib
    /var/lib/postgresql
    /var/lib/tuwunel
    /var/lib/tuwunel-backup
    /var/lib/lightfriend-reserve
    /var/log
    /data
    /data/seed
    /app
    /app/data
    /app/uploads
    /app/matrix_store
)

rotate_history_if_needed() {
    if [ -f "$HISTORY_FILE" ] && [ "$(stat -c%s "$HISTORY_FILE" 2>/dev/null || echo 0)" -gt "$MAX_HISTORY_BYTES" ]; then
        tail -c $((MAX_HISTORY_BYTES / 2)) "$HISTORY_FILE" > "${HISTORY_FILE}.tmp" 2>/dev/null && mv "${HISTORY_FILE}.tmp" "$HISTORY_FILE"
    fi
}

discover_mount_roots() {
    df -P 2>/dev/null | awk 'NR > 1 {print $6}' | while IFS= read -r mount; do
        [ -d "$mount" ] || continue
        case "$mount" in
            /dev|/dev/*|/proc|/proc/*|/sys|/sys/*)
                continue
                ;;
        esac
        printf '%s\n' "$mount"
    done | sort -u
}

discover_check_paths() {
    {
        for path in "${WATCH_PATHS[@]}"; do
            printf '%s\n' "$path"
        done
        discover_mount_roots
    } | sort -u
}

print_known_path_df() {
    local existing=()

    for path in "${WATCH_PATHS[@]}"; do
        if [ -e "$path" ]; then
            existing+=("$path")
        fi
    done

    if [ "${#existing[@]}" -gt 0 ]; then
        df -h "${existing[@]}" 2>/dev/null || true
        df -i "${existing[@]}" 2>/dev/null || true
    fi
}

du_depth_for_mount() {
    local mount="$1"
    if [ "$mount" = "/" ]; then
        echo 3
    else
        echo 2
    fi
}

print_largest_dirs_by_filesystem() {
    while IFS= read -r mount; do
        local depth
        depth="$(du_depth_for_mount "$mount")"
        echo "--- largest dirs on ${mount} filesystem (du -xhd${depth}) ---"
        du -xh -d "$depth" "$mount" 2>/dev/null | sort -h | tail -n "$TOP_DIR_LINES" || true
    done < <(discover_mount_roots)
}

print_largest_files_by_filesystem() {
    while IFS= read -r mount; do
        echo "--- largest files on ${mount} filesystem (> ${LARGE_FILE_MIN_KB} KiB) ---"
        find "$mount" -xdev -type f -size +"${LARGE_FILE_MIN_KB}"k -printf '%s %p\n' 2>/dev/null \
            | sort -n | tail -n "$TOP_FILE_LINES" || true
    done < <(discover_mount_roots)
}

collect_size_snapshot() {
    {
        while IFS= read -r mount; do
            du -x -k -d "$(du_depth_for_mount "$mount")" "$mount" 2>/dev/null || true
        done < <(discover_mount_roots)

        for path in /var /var/lib /data /app /tmp; do
            [ -d "$path" ] || continue
            du -x -k -d 2 "$path" 2>/dev/null || true
        done
    } | awk '
        {
            size = $1
            $1 = ""
            sub(/^ /, "")
            path = $0
            sizes[path] = size
        }
        END {
            for (path in sizes) {
                print path "\t" sizes[path]
            }
        }
    ' | sort
}

print_growth_since_last_snapshot() {
    local tmp_snapshot
    tmp_snapshot="${SNAPSHOT_FILE}.$$"

    collect_size_snapshot > "$tmp_snapshot" 2>/dev/null || true

    echo "--- growth since previous storage snapshot (>= ${GROWTH_REPORT_MIN_KB} KiB) ---"
    if [ -s "$SNAPSHOT_FILE" ] && [ -s "$tmp_snapshot" ]; then
        awk -F '\t' -v min_kb="$GROWTH_REPORT_MIN_KB" '
            FNR == NR {
                previous[$1] = $2
                next
            }
            {
                old = (($1 in previous) ? previous[$1] : 0)
                delta = $2 - old
                if (delta >= min_kb) {
                    print delta "\t" $2 "\t" old "\t" $1
                }
            }
        ' "$SNAPSHOT_FILE" "$tmp_snapshot" \
            | sort -nr \
            | head -30 \
            | awk -F '\t' '{printf "%+10d KiB now=%d KiB was=%d KiB %s\n", $1, $2, $3, $4}' || true
    else
        echo "no previous snapshot"
    fi

    mv "$tmp_snapshot" "$SNAPSHOT_FILE" 2>/dev/null || rm -f "$tmp_snapshot"
}

print_report() {
    echo "=== Storage Health $(date -u +%Y-%m-%dT%H:%M:%SZ) ==="
    echo "--- df -hT all filesystems ---"
    df -hT 2>/dev/null || df -h 2>/dev/null || echo "df unavailable"
    echo "--- df -i all filesystems ---"
    df -i 2>/dev/null || echo "df -i unavailable"
    echo "--- watched path df ---"
    print_known_path_df
    print_largest_dirs_by_filesystem
    print_largest_files_by_filesystem
    print_growth_since_last_snapshot
    echo "--- tuwunel BackupEngine dir ---"
    if [ -d /var/lib/tuwunel-backup ]; then
        du -sh /var/lib/tuwunel-backup 2>/dev/null || true
        find /var/lib/tuwunel-backup -maxdepth 2 -type f -printf '%s %p\n' 2>/dev/null | sort -n | tail -20 || true
    else
        echo "not present"
    fi
    echo "--- manual rootfs reserve ---"
    if [ -e /var/lib/lightfriend-reserve/rootfs-reserve.bin ]; then
        ls -lh /var/lib/lightfriend-reserve/rootfs-reserve.bin 2>/dev/null || true
        du -h /var/lib/lightfriend-reserve/rootfs-reserve.bin 2>/dev/null || true
    else
        echo "released or not present"
    fi
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
    while IFS= read -r path; do
        check_path "$path" || rc=1
    done < <(discover_check_paths)
    return "$rc"
}

cleanup_storage() {
    echo "=== Storage Cleanup $(date -u +%Y-%m-%dT%H:%M:%SZ) ==="

    rm -rf /tmp/backup-staging /tmp/verify.tar.gz /tmp/lightfriend-full-backup-*.tar.gz 2>/dev/null || true
    find /tmp -maxdepth 1 -name 'lightfriend-full-backup-*.tar.gz.enc' -mmin +30 -delete 2>/dev/null || true
    rm -rf /tmp/backup-restore 2>/dev/null || true
    find /data/seed -name 'lightfriend-full-backup-*.tar.gz.enc' -mmin +30 -delete 2>/dev/null || true

    if [ -d /var/lib/tuwunel-backup ]; then
        if pgrep -f '/app/export.sh' >/dev/null 2>&1; then
            echo "Skipping /var/lib/tuwunel-backup cleanup while export.sh is running"
        else
            echo "Removing stale /var/lib/tuwunel-backup"
            rm -rf /var/lib/tuwunel-backup 2>/dev/null || true
        fi
    fi

    # The tunnel and bridge logs can grow quickly during scans/outages. Keep
    # recent logs, but cap any single supervisor log to the last 1 MiB.
    find /var/log/supervisor /tmp/marlin-kms -type f -size +5M -print 2>/dev/null | while read -r log; do
        tmp="${log}.tmp"
        tail -c 1048576 "$log" > "$tmp" 2>/dev/null && cat "$tmp" > "$log"
        rm -f "$tmp" 2>/dev/null || true
    done

    find /var/log/supervisor -type f \( -name '*.1' -o -name '*.2' -o -name '*.3' \) -mtime +1 -delete 2>/dev/null || true
    for log in /var/log/gvforwarder.log /data/seed/boot-trace.log /data/seed/startup-services.log /data/seed/startup-signal.log /tmp/export-watcher-last-run.log; do
        [ -f "$log" ] || continue
        if [ "$(stat -c%s "$log" 2>/dev/null || echo 0)" -gt 5242880 ]; then
            tmp="${log}.tmp"
            tail -c 1048576 "$log" > "$tmp" 2>/dev/null && cat "$tmp" > "$log"
            rm -f "$tmp" 2>/dev/null || true
        fi
    done
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
