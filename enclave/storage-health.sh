#!/bin/bash
# Storage diagnostics and cleanup for the enclave.
#
# Postgres can fail with "could not write init file" when the enclave's
# writable filesystem, temp space, or inodes are exhausted. Keep this script
# dependency-light so watchdogs can run it even during partial failures.

set -uo pipefail

MIN_FREE_KB="${MIN_FREE_KB:-524288}"          # 512 MiB
MIN_FREE_INODES="${MIN_FREE_INODES:-1024}"
HISTORY_FILE="${STORAGE_HEALTH_HISTORY_FILE:-/tmp/storage-health-history.log}"
MAX_HISTORY_BYTES="${STORAGE_MAX_HISTORY_BYTES:-1048576}"
SNAPSHOT_FILE="${STORAGE_HEALTH_SNAPSHOT_FILE:-/tmp/storage-health-snapshot.tsv}"
TOP_DIR_LINES="${STORAGE_HEALTH_TOP_DIR_LINES:-80}"
TOP_FILE_LINES="${STORAGE_HEALTH_TOP_FILE_LINES:-40}"
LARGE_FILE_MIN_KB="${STORAGE_HEALTH_LARGE_FILE_MIN_KB:-1024}"
GROWTH_REPORT_MIN_KB="${STORAGE_HEALTH_GROWTH_REPORT_MIN_KB:-10240}"
TUWUNEL_MEDIA_DIR="${TUWUNEL_MEDIA_DIR:-/var/lib/tuwunel/media}"
TUWUNEL_MEDIA_MAX_BYTES="${TUWUNEL_MEDIA_MAX_BYTES:-67108864}"       # 64 MiB
TUWUNEL_MEDIA_MIN_AGE_SECS="${TUWUNEL_MEDIA_MIN_AGE_SECS:-1800}"     # 30 minutes

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

print_rootfs_backup_headroom() {
    echo "--- rootfs backup headroom ---"

    local reserve_file
    reserve_file="${LIGHTFRIEND_ROOTFS_RESERVE_FILE:-/var/lib/lightfriend-reserve/rootfs-reserve.bin}"

    local root_avail_kib tmp_avail_kib reserve_bytes reserve_kib projected_kib
    root_avail_kib=$(df -Pk / 2>/dev/null | awk 'NR==2 {print $4 + 0}')
    tmp_avail_kib=$(df -Pk /tmp 2>/dev/null | awk 'NR==2 {print $4 + 0}')
    [ -n "$root_avail_kib" ] || root_avail_kib=0
    [ -n "$tmp_avail_kib" ] || tmp_avail_kib=0
    reserve_bytes=0

    if [ -e "$reserve_file" ]; then
        reserve_bytes=$(stat -c%s "$reserve_file" 2>/dev/null || echo 0)
    fi

    reserve_kib=$(((reserve_bytes + 1023) / 1024))
    projected_kib=$((root_avail_kib + reserve_kib))

    printf "root_avail_kib=%d root_avail_mib=%.1f\n" "$root_avail_kib" "$(awk -v kb="$root_avail_kib" 'BEGIN {print kb / 1024}')"
    printf "tmp_avail_kib=%d tmp_avail_mib=%.1f\n" "$tmp_avail_kib" "$(awk -v kb="$tmp_avail_kib" 'BEGIN {print kb / 1024}')"
    printf "reserve_file=%s reserve_bytes=%d reserve_mib=%.1f\n" "$reserve_file" "$reserve_bytes" "$(awk -v b="$reserve_bytes" 'BEGIN {print b / 1048576}')"
    printf "projected_root_avail_after_release_kib=%d projected_root_avail_after_release_mib=%.1f\n" "$projected_kib" "$(awk -v kb="$projected_kib" 'BEGIN {print kb / 1024}')"
    echo "note: final encrypted backup uses /tmp, but Tuwunel BackupEngine uses rootfs at /var/lib/tuwunel-backup"
}

print_tuwunel_detailed_breakdown() {
    echo "--- tuwunel detailed storage ---"
    if [ ! -d /var/lib/tuwunel ]; then
        echo "not present"
        return 0
    fi

    du -sh /var/lib/tuwunel /var/lib/tuwunel/media 2>/dev/null || true

    echo "--- tuwunel file type buckets ---"
    find /var/lib/tuwunel -xdev -type f -printf '%s %p\n' 2>/dev/null \
        | awk '
            function add(kind, size) {
                count[kind] += 1
                bytes[kind] += size
            }
            {
                size = $1
                $1 = ""
                sub(/^ /, "")
                path = $0

                if (path ~ /\/media\//) {
                    add("media", size)
                } else if (path ~ /\.sst$/) {
                    add("rocksdb_sst", size)
                } else if (path ~ /\/(LOG|LOG\.old|MANIFEST-)/ || path ~ /\/OPTIONS-/) {
                    add("rocksdb_meta_logs", size)
                } else {
                    add("other", size)
                }
            }
            END {
                for (kind in count) {
                    printf "%s count=%d bytes=%d mib=%.1f\n", kind, count[kind], bytes[kind], bytes[kind] / 1048576
                }
            }
        ' | sort || true

    echo "--- tuwunel top dirs (du -xhd2) ---"
    du -xh -d 2 /var/lib/tuwunel 2>/dev/null | sort -h | tail -60 || true

    echo "--- tuwunel top files ---"
    find /var/lib/tuwunel -xdev -type f -printf '%s %p\n' 2>/dev/null | sort -n | tail -60 || true

    echo "--- tuwunel media janitor policy ---"
    printf "media_dir=%s max_bytes=%s max_mib=%.1f min_age_secs=%s\n" \
        "$TUWUNEL_MEDIA_DIR" \
        "$TUWUNEL_MEDIA_MAX_BYTES" \
        "$(awk -v b="$TUWUNEL_MEDIA_MAX_BYTES" 'BEGIN {print b / 1048576}')" \
        "$TUWUNEL_MEDIA_MIN_AGE_SECS"

    echo "--- tuwunel config storage/admin knobs ---"
    grep -E '^(database_path|database_backup_path|database_backups_to_keep|rocksdb_direct_io|allow_legacy_media|freeze_legacy_media|allow_federation|admin_signal_execute)' \
        /etc/tuwunel/tuwunel.toml 2>/dev/null || true
}

is_nonnegative_int() {
    case "${1:-}" in
        ''|*[!0-9]*)
            return 1
            ;;
        *)
            return 0
            ;;
    esac
}

check_tuwunel_media_cap() {
    if [ ! -d "$TUWUNEL_MEDIA_DIR" ]; then
        return 0
    fi

    if ! is_nonnegative_int "$TUWUNEL_MEDIA_MAX_BYTES" || [ "$TUWUNEL_MEDIA_MAX_BYTES" -eq 0 ]; then
        return 0
    fi

    local media_bytes media_count
    media_bytes=$(du_bytes "$TUWUNEL_MEDIA_DIR")
    media_count=$(find "$TUWUNEL_MEDIA_DIR" -xdev -type f 2>/dev/null | wc -l | awk '{print $1 + 0}')

    if [ "$media_bytes" -gt "$TUWUNEL_MEDIA_MAX_BYTES" ]; then
        printf "TUWUNEL_MEDIA_OVER_CAP path=%s bytes=%d count=%d cap_bytes=%d cap_mib=%.1f\n" \
            "$TUWUNEL_MEDIA_DIR" \
            "$media_bytes" \
            "$media_count" \
            "$TUWUNEL_MEDIA_MAX_BYTES" \
            "$(awk -v b="$TUWUNEL_MEDIA_MAX_BYTES" 'BEGIN {print b / 1048576}')"
        return 1
    fi

    return 0
}

cleanup_tuwunel_media() {
    echo "--- tuwunel media janitor ---"

    if [ ! -d "$TUWUNEL_MEDIA_DIR" ]; then
        echo "Tuwunel media dir not present: $TUWUNEL_MEDIA_DIR"
        return 0
    fi

    if ! is_nonnegative_int "$TUWUNEL_MEDIA_MAX_BYTES" || [ "$TUWUNEL_MEDIA_MAX_BYTES" -eq 0 ]; then
        echo "Tuwunel media janitor disabled: TUWUNEL_MEDIA_MAX_BYTES=$TUWUNEL_MEDIA_MAX_BYTES"
        return 0
    fi

    if ! is_nonnegative_int "$TUWUNEL_MEDIA_MIN_AGE_SECS"; then
        echo "Invalid TUWUNEL_MEDIA_MIN_AGE_SECS=$TUWUNEL_MEDIA_MIN_AGE_SECS; using 1800"
        TUWUNEL_MEDIA_MIN_AGE_SECS=1800
    fi

    if pgrep -f '/app/export.sh' >/dev/null 2>&1; then
        echo "Skipping Tuwunel media cleanup while export.sh is running"
        return 0
    fi

    local before_bytes before_count cutoff_epoch list_file current_bytes deleted_count deleted_bytes
    before_bytes=$(du_bytes "$TUWUNEL_MEDIA_DIR")
    before_count=$(find "$TUWUNEL_MEDIA_DIR" -xdev -type f 2>/dev/null | wc -l | awk '{print $1 + 0}')

    printf "before_bytes=%d before_mib=%.1f before_count=%d cap_bytes=%d cap_mib=%.1f min_age_secs=%s\n" \
        "$before_bytes" \
        "$(awk -v b="$before_bytes" 'BEGIN {print b / 1048576}')" \
        "$before_count" \
        "$TUWUNEL_MEDIA_MAX_BYTES" \
        "$(awk -v b="$TUWUNEL_MEDIA_MAX_BYTES" 'BEGIN {print b / 1048576}')" \
        "$TUWUNEL_MEDIA_MIN_AGE_SECS"

    if [ "$before_bytes" -le "$TUWUNEL_MEDIA_MAX_BYTES" ]; then
        echo "Tuwunel media is within cap"
        return 0
    fi

    cutoff_epoch=$(($(date +%s) - TUWUNEL_MEDIA_MIN_AGE_SECS))
    list_file="/tmp/tuwunel-media-prune.$$"
    find "$TUWUNEL_MEDIA_DIR" -xdev -type f -printf '%T@ %s %p\n' 2>/dev/null | sort -n > "$list_file" || true

    current_bytes="$before_bytes"
    deleted_count=0
    deleted_bytes=0

    while IFS=' ' read -r mtime size path; do
        [ -n "${path:-}" ] || continue
        [ "$current_bytes" -le "$TUWUNEL_MEDIA_MAX_BYTES" ] && break

        local mtime_epoch
        mtime_epoch="${mtime%.*}"
        if ! is_nonnegative_int "$mtime_epoch" || [ "$mtime_epoch" -gt "$cutoff_epoch" ]; then
            break
        fi

        if rm -f -- "$path" 2>/dev/null; then
            current_bytes=$((current_bytes - size))
            deleted_count=$((deleted_count + 1))
            deleted_bytes=$((deleted_bytes + size))
            printf "deleted_tuwunel_media bytes=%s path=%s\n" "$size" "$path"
        fi
    done < "$list_file"

    rm -f "$list_file" 2>/dev/null || true
    find "$TUWUNEL_MEDIA_DIR" -xdev -mindepth 1 -type d -empty -delete 2>/dev/null || true

    local after_bytes after_count
    after_bytes=$(du_bytes "$TUWUNEL_MEDIA_DIR")
    after_count=$(find "$TUWUNEL_MEDIA_DIR" -xdev -type f 2>/dev/null | wc -l | awk '{print $1 + 0}')

    printf "deleted_count=%d deleted_bytes=%d deleted_mib=%.1f after_bytes=%d after_mib=%.1f after_count=%d\n" \
        "$deleted_count" \
        "$deleted_bytes" \
        "$(awk -v b="$deleted_bytes" 'BEGIN {print b / 1048576}')" \
        "$after_bytes" \
        "$(awk -v b="$after_bytes" 'BEGIN {print b / 1048576}')" \
        "$after_count"

    if [ "$after_bytes" -gt "$TUWUNEL_MEDIA_MAX_BYTES" ]; then
        echo "Tuwunel media remains over cap; remaining files are newer than min_age_secs or could not be deleted"
    fi
}

du_bytes() {
    local path="$1"
    if [ ! -e "$path" ]; then
        echo 0
        return 0
    fi

    du -sb "$path" 2>/dev/null | awk 'NR == 1 {print $1 + 0; seen = 1} END {if (!seen) print 0}'
}

df_metrics() {
    local path="$1"
    df -Pk "$path" 2>/dev/null | awk '
        NR == 2 {
            gsub(/%/, "", $5)
            printf "%d %d %d %d\n", $2, $3, $4, $5
            seen = 1
        }
        END {
            if (!seen) {
                print "0 0 0 0"
            }
        }
    '
}

json_escape() {
    printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'
}

tuwunel_bucket_assignments() {
    if [ ! -d /var/lib/tuwunel ]; then
        cat <<'EOF'
TUWUNEL_MEDIA_COUNT=0
TUWUNEL_MEDIA_BYTES=0
TUWUNEL_ROCKSDB_SST_COUNT=0
TUWUNEL_ROCKSDB_SST_BYTES=0
TUWUNEL_ROCKSDB_META_LOGS_COUNT=0
TUWUNEL_ROCKSDB_META_LOGS_BYTES=0
TUWUNEL_OTHER_COUNT=0
TUWUNEL_OTHER_BYTES=0
EOF
        return 0
    fi

    find /var/lib/tuwunel -xdev -type f -printf '%s %p\n' 2>/dev/null \
        | awk '
            function add(kind, size) {
                count[kind] += 1
                bytes[kind] += size
            }
            {
                size = $1
                $1 = ""
                sub(/^ /, "")
                path = $0

                if (path ~ /\/media\//) {
                    add("MEDIA", size)
                } else if (path ~ /\.sst$/) {
                    add("ROCKSDB_SST", size)
                } else if (path ~ /\/(LOG|LOG\.old|MANIFEST-)/ || path ~ /\/OPTIONS-/) {
                    add("ROCKSDB_META_LOGS", size)
                } else {
                    add("OTHER", size)
                }
            }
            END {
                kinds[1] = "MEDIA"
                kinds[2] = "ROCKSDB_SST"
                kinds[3] = "ROCKSDB_META_LOGS"
                kinds[4] = "OTHER"
                for (i = 1; i <= 4; i++) {
                    kind = kinds[i]
                    printf "TUWUNEL_%s_COUNT=%d\n", kind, count[kind] + 0
                    printf "TUWUNEL_%s_BYTES=%d\n", kind, bytes[kind] + 0
                }
            }
        '
}

print_json_metrics() {
    local timestamp reserve_file reserve_file_json reserve_present
    local root_size root_used root_avail root_pct tmp_size tmp_used tmp_avail tmp_pct
    local reserve_bytes reserve_kib projected_root_avail_kib
    local tuwunel_total_bytes postgres_bytes tuwunel_backup_bytes supervisor_logs_bytes

    timestamp="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    reserve_file="${LIGHTFRIEND_ROOTFS_RESERVE_FILE:-/var/lib/lightfriend-reserve/rootfs-reserve.bin}"
    reserve_file_json="$(json_escape "$reserve_file")"

    read -r root_size root_used root_avail root_pct < <(df_metrics /)
    read -r tmp_size tmp_used tmp_avail tmp_pct < <(df_metrics /tmp)

    reserve_present=false
    reserve_bytes=0
    if [ -e "$reserve_file" ]; then
        reserve_present=true
        reserve_bytes=$(stat -c%s "$reserve_file" 2>/dev/null || echo 0)
    fi
    reserve_bytes=${reserve_bytes:-0}
    reserve_kib=$(((reserve_bytes + 1023) / 1024))
    projected_root_avail_kib=$((root_avail + reserve_kib))

    tuwunel_total_bytes=$(du_bytes /var/lib/tuwunel)
    postgres_bytes=$(du_bytes /var/lib/postgresql)
    tuwunel_backup_bytes=$(du_bytes /var/lib/tuwunel-backup)
    supervisor_logs_bytes=$(du_bytes /var/log/supervisor)

    local TUWUNEL_MEDIA_COUNT=0
    local TUWUNEL_MEDIA_BYTES=0
    local TUWUNEL_ROCKSDB_SST_COUNT=0
    local TUWUNEL_ROCKSDB_SST_BYTES=0
    local TUWUNEL_ROCKSDB_META_LOGS_COUNT=0
    local TUWUNEL_ROCKSDB_META_LOGS_BYTES=0
    local TUWUNEL_OTHER_COUNT=0
    local TUWUNEL_OTHER_BYTES=0
    eval "$(tuwunel_bucket_assignments)"

    cat <<EOF
{"timestamp":"${timestamp}","filesystems":{"root":{"size_kib":${root_size},"used_kib":${root_used},"avail_kib":${root_avail},"use_pct":${root_pct}},"tmp":{"size_kib":${tmp_size},"used_kib":${tmp_used},"avail_kib":${tmp_avail},"use_pct":${tmp_pct}}},"reserve":{"path":"${reserve_file_json}","present":${reserve_present},"bytes":${reserve_bytes},"projected_root_avail_kib":${projected_root_avail_kib}},"tuwunel":{"total_bytes":${tuwunel_total_bytes},"media":{"count":${TUWUNEL_MEDIA_COUNT},"bytes":${TUWUNEL_MEDIA_BYTES}},"rocksdb_sst":{"count":${TUWUNEL_ROCKSDB_SST_COUNT},"bytes":${TUWUNEL_ROCKSDB_SST_BYTES}},"rocksdb_meta_logs":{"count":${TUWUNEL_ROCKSDB_META_LOGS_COUNT},"bytes":${TUWUNEL_ROCKSDB_META_LOGS_BYTES}},"other":{"count":${TUWUNEL_OTHER_COUNT},"bytes":${TUWUNEL_OTHER_BYTES}}},"postgres":{"bytes":${postgres_bytes}},"tuwunel_backup_engine":{"bytes":${tuwunel_backup_bytes}},"supervisor_logs":{"bytes":${supervisor_logs_bytes}}}
EOF
}

print_report() {
    echo "=== Storage Health $(date -u +%Y-%m-%dT%H:%M:%SZ) ==="
    echo "--- df -hT all filesystems ---"
    df -hT 2>/dev/null || df -h 2>/dev/null || echo "df unavailable"
    echo "--- df -i all filesystems ---"
    df -i 2>/dev/null || echo "df -i unavailable"
    echo "--- watched path df ---"
    print_known_path_df
    print_rootfs_backup_headroom
    print_largest_dirs_by_filesystem
    print_largest_files_by_filesystem
    print_tuwunel_detailed_breakdown
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
    check_tuwunel_media_cap || rc=1
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

    cleanup_tuwunel_media

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
    json)
        print_json_metrics
        ;;
    *)
        echo "Usage: $0 [report|check|cleanup|json]" >&2
        exit 2
        ;;
esac
