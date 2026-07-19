#!/bin/bash
# Storage diagnostics and cleanup for the enclave.
#
# Postgres can fail with "could not write init file" when the enclave's
# writable filesystem, temp space, or inodes are exhausted. Keep this script
# dependency-light so watchdogs can run it even during partial failures.

set -uo pipefail

MIN_FREE_KB="${MIN_FREE_KB:-524288}"          # 512 MiB
MIN_FREE_INODES="${MIN_FREE_INODES:-1024}"
ROOTFS_RESERVE_FILE="${LIGHTFRIEND_ROOTFS_RESERVE_FILE:-/var/lib/lightfriend-reserve/rootfs-reserve.bin}"
HISTORY_FILE="${STORAGE_HEALTH_HISTORY_FILE:-/tmp/storage-health-history.log}"
MAX_HISTORY_BYTES="${STORAGE_MAX_HISTORY_BYTES:-1048576}"
SNAPSHOT_FILE="${STORAGE_HEALTH_SNAPSHOT_FILE:-/tmp/storage-health-snapshot.tsv}"
TUWUNEL_BUCKET_SNAPSHOT_FILE="${STORAGE_HEALTH_TUWUNEL_BUCKET_SNAPSHOT_FILE:-/tmp/tuwunel-storage-bucket-snapshot.tsv}"
TOP_DIR_LINES="${STORAGE_HEALTH_TOP_DIR_LINES:-80}"
TOP_FILE_LINES="${STORAGE_HEALTH_TOP_FILE_LINES:-40}"
HOT_PATH_DIR_LINES="${STORAGE_HEALTH_HOT_PATH_DIR_LINES:-60}"
HOT_PATH_FILE_LINES="${STORAGE_HEALTH_HOT_PATH_FILE_LINES:-30}"
LARGE_FILE_MIN_KB="${STORAGE_HEALTH_LARGE_FILE_MIN_KB:-1024}"
GROWTH_REPORT_MIN_KB="${STORAGE_HEALTH_GROWTH_REPORT_MIN_KB:-10240}"
TUWUNEL_BUCKET_GROWTH_REPORT_MIN_KB="${STORAGE_HEALTH_TUWUNEL_BUCKET_GROWTH_REPORT_MIN_KB:-1024}"
TUWUNEL_MEDIA_DIR="${TUWUNEL_MEDIA_DIR:-/var/lib/tuwunel/media}"
TUWUNEL_MEDIA_MAX_BYTES="${TUWUNEL_MEDIA_MAX_BYTES:-8388608}"        # 8 MiB alarm cap
TUWUNEL_MEDIA_RETENTION_SECS="${TUWUNEL_MEDIA_RETENTION_SECS:-${TUWUNEL_MEDIA_MIN_AGE_SECS:-60}}"
TUWUNEL_MEDIA_MIN_AGE_SECS="${TUWUNEL_MEDIA_MIN_AGE_SECS:-$TUWUNEL_MEDIA_RETENTION_SECS}" # backwards-compatible alias
TUWUNEL_MEDIA_DELETE_LOG_LIMIT="${TUWUNEL_MEDIA_DELETE_LOG_LIMIT:-200}"
TUWUNEL_ARCHIVE_LOG_DIR="${TUWUNEL_ARCHIVE_LOG_DIR:-/var/lib/tuwunel/archive}"
TUWUNEL_ARCHIVE_LOG_MAX_BYTES="${TUWUNEL_ARCHIVE_LOG_MAX_BYTES:-33554432}" # 32 MiB cap
TUWUNEL_ARCHIVE_LOG_RETENTION_SECS="${TUWUNEL_ARCHIVE_LOG_RETENTION_SECS:-21600}" # 6 hours
TUWUNEL_ARCHIVE_LOG_MIN_AGE_SECS="${TUWUNEL_ARCHIVE_LOG_MIN_AGE_SECS:-600}" # cap pruning skips very fresh logs
TUWUNEL_ARCHIVE_LOG_DELETE_LOG_LIMIT="${TUWUNEL_ARCHIVE_LOG_DELETE_LOG_LIMIT:-200}"
BACKUP_ARTIFACT_LOCK_FILE="${LIGHTFRIEND_BACKUP_ARTIFACT_LOCK_FILE:-/tmp/lightfriend-backup-artifacts.lock}"
BACKUP_STAGING_ROOT="${LIGHTFRIEND_BACKUP_STAGING_ROOT:-/tmp/backup-staging}"
BACKUP_TMP_ROOT="${LIGHTFRIEND_BACKUP_TMP_ROOT:-/tmp}"
BACKUP_RESTORE_ROOT="${LIGHTFRIEND_BACKUP_RESTORE_ROOT:-/tmp/backup-restore}"
BACKUP_SEED_DIR="${LIGHTFRIEND_BACKUP_SEED_DIR:-/data/seed}"
TUWUNEL_BACKUP_DIR="${TUWUNEL_BACKUP_DIR:-/var/lib/tuwunel-backup}"

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
    reserve_file="$ROOTFS_RESERVE_FILE"

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

print_storage_size_summary() {
    echo "--- mutable database/data size summary ---"
    for path in /var/lib/tuwunel /var/lib/tuwunel/media /var/lib/tuwunel-backup /var/lib/postgresql /var/log/supervisor /tmp; do
        if [ -e "$path" ]; then
            local bytes
            bytes=$(du_bytes "$path")
            printf "path=%s bytes=%d mib=%.1f\n" "$path" "$bytes" "$(awk -v b="$bytes" 'BEGIN {print b / 1048576}')"
        else
            printf "path=%s missing=true bytes=0 mib=0.0\n" "$path"
        fi
    done
}

print_exact_root_accounting() {
    local tmp_file df_used_kib df_used_bytes du_root_bytes gap_bytes

    echo "--- exact rootfs accounting ---"
    tmp_file="/tmp/storage-health-root-accounting.$$"
    du -x --block-size=1 -d 1 / > "$tmp_file" 2>/dev/null || true
    df_used_kib=$(df -Pk / 2>/dev/null | awk 'NR == 2 {print $3}')
    du_root_bytes=$(awk '$2 == "/" {print $1}' "$tmp_file" 2>/dev/null | tail -1)
    df_used_kib=${df_used_kib:-0}
    du_root_bytes=${du_root_bytes:-0}
    df_used_bytes=$((df_used_kib * 1024))
    gap_bytes=$((df_used_bytes - du_root_bytes))
    printf "df_used_bytes=%d df_used_mib=%.1f du_root_bytes=%d du_root_mib=%.1f df_minus_du_bytes=%d df_minus_du_mib=%.1f\n" \
        "$df_used_bytes" \
        "$(awk -v b="$df_used_bytes" 'BEGIN {print b / 1048576}')" \
        "$du_root_bytes" \
        "$(awk -v b="$du_root_bytes" 'BEGIN {print b / 1048576}')" \
        "$gap_bytes" \
        "$(awk -v b="$gap_bytes" 'BEGIN {print b / 1048576}')"

    echo "exact_bytes path"
    sort -nr "$tmp_file" 2>/dev/null || true
    rm -f "$tmp_file"
}

print_hot_path_accounting() {
    local path total_bytes
    local paths=(
        /home/rasmus/matrix-server
        /app/matrix_store
        /data/bridges
        /app/uploads
    )

    echo "--- mutable hot-path accounting ---"
    for path in "${paths[@]}"; do
        if [ ! -d "$path" ]; then
            printf "path=%s missing=true\n" "$path"
            continue
        fi

        total_bytes=$(du -x --block-size=1 -s "$path" 2>/dev/null | awk 'NR == 1 {print $1}')
        total_bytes=${total_bytes:-0}
        printf "path=%s bytes=%d mib=%.1f\n" \
            "$path" \
            "$total_bytes" \
            "$(awk -v b="$total_bytes" 'BEGIN {print b / 1048576}')"

        echo "largest_subdirs exact_bytes path"
        du -x --block-size=1 -d 4 "$path" 2>/dev/null \
            | sort -nr \
            | head -n "$HOT_PATH_DIR_LINES" || true

        echo "largest_files exact_bytes mtime_utc path"
        find "$path" -xdev -type f -printf '%s\t%TY-%Tm-%TdT%TH:%TM:%TSZ\t%p\n' 2>/dev/null \
            | sort -nr \
            | head -n "$HOT_PATH_FILE_LINES" || true
    done
}

print_deleted_open_file_accounting() {
    local tmp_file fd_path target metadata key size pid comm
    tmp_file="/tmp/storage-health-deleted-open.$$"
    : > "$tmp_file" 2>/dev/null || {
        echo "--- deleted-but-open files ---"
        echo "unable to create temporary accounting file"
        return 0
    }

    for fd_path in /proc/[0-9]*/fd/*; do
        [ -L "$fd_path" ] || continue
        target=$(readlink "$fd_path" 2>/dev/null || true)
        case "$target" in
            *" (deleted)") ;;
            *) continue ;;
        esac

        metadata=$(stat -Lc '%d:%i %s' "$fd_path" 2>/dev/null || true)
        [ -n "$metadata" ] || continue
        key=${metadata%% *}
        size=${metadata#* }
        case "$size" in
            ''|*[!0-9]*) continue ;;
        esac
        pid=${fd_path#/proc/}
        pid=${pid%%/*}
        comm=$(cat "/proc/$pid/comm" 2>/dev/null || echo unknown)
        printf '%s\t%s\t%s\t%s\t%s\n' "$key" "$size" "$pid" "$comm" "$target" >> "$tmp_file"
    done

    echo "--- deleted-but-open files ---"
    if [ ! -s "$tmp_file" ]; then
        echo "unique_files=0 total_bytes=0 total_mib=0.0"
        rm -f "$tmp_file"
        return 0
    fi

    awk -F '\t' '
        !seen[$1]++ { files += 1; bytes += $2 }
        END { printf "unique_files=%d total_bytes=%d total_mib=%.1f\n", files, bytes, bytes / 1048576 }
    ' "$tmp_file"
    echo "bytes pid process target"
    awk -F '\t' '!seen[$1]++ {print}' "$tmp_file" \
        | sort -t $'\t' -k2,2nr \
        | head -40 \
        | awk -F '\t' '{printf "%s %s %s %s\n", $2, $3, $4, $5}' || true
    rm -f "$tmp_file"
}

print_tuwunel_purge_audit() {
    echo "--- Tuwunel purge compact audit ---"
    echo "historical_audit_policy backfill_enabled=${TUWUNEL_EVENT_PURGE_BACKFILL_ENABLED:-false} audit_enabled=${TUWUNEL_EVENT_PURGE_BACKFILL_AUDIT_ENABLED:-true} execute_verified_enabled=${TUWUNEL_EVENT_PURGE_BACKFILL_EXECUTE_VERIFIED_ENABLED:-false} batch_size=${TUWUNEL_EVENT_PURGE_BACKFILL_BATCH_SIZE:-25} scan_secs=${TUWUNEL_EVENT_PURGE_BACKFILL_SCAN_SECS:-3600} min_age_secs=${TUWUNEL_EVENT_PURGE_BACKFILL_MIN_AGE_SECS:-86400} recheck_secs=${TUWUNEL_EVENT_PURGE_BACKFILL_AUDIT_RECHECK_SECS:-86400} max_pages=${TUWUNEL_EVENT_PURGE_BACKFILL_AUDIT_MAX_PAGES:-100} page_size=${TUWUNEL_EVENT_PURGE_BACKFILL_AUDIT_PAGE_SIZE:-100}"
    echo "disconnected_bridge_policy audit_enabled=${TUWUNEL_DISCONNECTED_BRIDGE_PURGE_AUDIT_ENABLED:-true} execute_enabled=${TUWUNEL_DISCONNECTED_BRIDGE_PURGE_ENABLED:-true} orphan_execute_enabled=${TUWUNEL_DISCONNECTED_BRIDGE_ORPHAN_PURGE_ENABLED:-false} grace_secs=${TUWUNEL_DISCONNECTED_BRIDGE_PURGE_GRACE_SECS:-120} batch_size=${TUWUNEL_DISCONNECTED_BRIDGE_PURGE_BATCH_SIZE:-5} room_delete_limit=${TUWUNEL_DISCONNECTED_BRIDGE_PURGE_ROOM_LIMIT:-1}"
    if ! command -v psql >/dev/null 2>&1 || [ -z "${PG_DATABASE_URL:-}" ]; then
        echo "psql or PG_DATABASE_URL unavailable"
        return 0
    fi

    timeout 8 psql "$PG_DATABASE_URL" -X -v ON_ERROR_STOP=1 -P pager=off -F '|' -c '
        SELECT status,
               count(*) AS rows,
               COALESCE(sum(commands_expected), 0) AS expected,
               COALESCE(sum(commands_accepted), 0) AS accepted,
               to_char(to_timestamp(max(updated_at)), '\''YYYY-MM-DD"T"HH24:MI:SS"Z"'\'') AS last_updated
          FROM tuwunel_cleanup_events
         GROUP BY status
         ORDER BY status;

        SELECT service,
               status,
               count(*) AS rows,
               count(DISTINCT room_id) AS rooms,
               to_char(to_timestamp(max(updated_at)), '\''YYYY-MM-DD"T"HH24:MI:SS"Z"'\'') AS last_updated
          FROM tuwunel_cleanup_events
         WHERE updated_at >= EXTRACT(EPOCH FROM NOW())::INT4 - (6 * 60 * 60)
         GROUP BY service, status
         ORDER BY rows DESC, service, status;

        SELECT status,
               COALESCE(last_error, '\''reason unavailable'\'') AS reason,
               count(*) AS rows,
               count(DISTINCT room_id) AS rooms
          FROM tuwunel_cleanup_events
         WHERE status IN ('\''ingesting'\'', '\''ingest_failed'\'', '\''purge_retrying'\'', '\''purge_exhausted'\'', '\''backfill_audit_verified'\'', '\''backfill_audit_blocked'\'')
         GROUP BY status, COALESCE(last_error, '\''reason unavailable'\'')
         ORDER BY rows DESC, status, reason;

        SELECT status,
               service,
               split_part(COALESCE(last_error, '\''reason unavailable'\''), '\'' '\'', 1) AS reason_code,
               count(*) AS rows,
               count(DISTINCT room_id) AS rooms,
               to_char(to_timestamp(max(updated_at)), '\''YYYY-MM-DD"T"HH24:MI:SS"Z"'\'') AS last_updated
          FROM tuwunel_cleanup_events
         WHERE status IN ('\''backfill_audit_verified'\'', '\''backfill_audit_blocked'\'')
         GROUP BY status, service, split_part(COALESCE(last_error, '\''reason unavailable'\''), '\'' '\'', 1)
         ORDER BY rows DESC, status, service, reason_code;

        SELECT cleanup.status,
               cleanup.user_id,
               cleanup.service,
               cleanup.room_id,
               cleanup.event_id AS boundary_event_id,
               cleanup.ontology_message_id,
               to_char(to_timestamp(messages.created_at), '\''YYYY-MM-DD"T"HH24:MI:SS"Z"'\'') AS boundary_created_at,
               to_char(to_timestamp(cleanup.updated_at), '\''YYYY-MM-DD"T"HH24:MI:SS"Z"'\'') AS audited_at,
               left(COALESCE(cleanup.last_error, '\''reason unavailable'\''), 1000) AS audit_summary
          FROM tuwunel_cleanup_events cleanup
          LEFT JOIN ont_messages messages
            ON messages.id = cleanup.ontology_message_id
         WHERE cleanup.status IN ('\''backfill_audit_verified'\'', '\''backfill_audit_blocked'\'')
         ORDER BY cleanup.updated_at DESC, cleanup.id DESC
         LIMIT 100;

        WITH message_coverage AS (
            SELECT messages.id,
                   messages.room_id,
                   cleanup.status,
                   EXISTS (
                       SELECT 1
                         FROM tuwunel_cleanup_events newer
                        WHERE newer.room_id = messages.room_id
                          AND newer.status = '\''purge_succeeded'\''
                          AND newer.ontology_message_id > messages.id
                   ) AS covered_by_newer_success
              FROM ont_messages messages
              LEFT JOIN tuwunel_cleanup_events cleanup
                ON cleanup.event_id = messages.matrix_event_id
             WHERE messages.matrix_event_id IS NOT NULL
        )
        SELECT count(*) AS ontology_events_with_matrix_id,
               count(*) FILTER (WHERE status IS NOT NULL) AS cleanup_rows_present,
               count(*) FILTER (WHERE status IS NULL) AS no_cleanup_row,
               count(*) FILTER (WHERE status IS NULL AND covered_by_newer_success) AS no_row_but_covered_by_newer_purge,
               count(*) FILTER (WHERE status IS NULL AND NOT covered_by_newer_success) AS no_row_without_purge_proof,
               count(DISTINCT room_id) FILTER (WHERE status IS NULL AND NOT covered_by_newer_success) AS rooms_without_purge_proof
          FROM message_coverage;

        WITH latest_by_room AS (
            SELECT DISTINCT ON (room_id)
                   room_id,
                   matrix_event_id
              FROM ont_messages
             WHERE matrix_event_id IS NOT NULL
             ORDER BY room_id, created_at DESC, id DESC
        )
        SELECT COALESCE(cleanup.status, '\''unaudited'\'') AS latest_boundary_state,
               count(*) AS rooms
          FROM latest_by_room latest
          LEFT JOIN tuwunel_cleanup_events cleanup
            ON cleanup.event_id = latest.matrix_event_id
         GROUP BY COALESCE(cleanup.status, '\''unaudited'\'')
         ORDER BY rooms DESC, latest_boundary_state;

        SELECT trigger_kind,
               bridge_type,
               status,
               portal_cleanup_status,
               count(*) AS jobs,
               to_char(to_timestamp(max(updated_at)), '\''YYYY-MM-DD"T"HH24:MI:SS"Z"'\'') AS last_updated
          FROM bridge_cleanup_jobs
         GROUP BY trigger_kind, bridge_type, status, portal_cleanup_status
         ORDER BY jobs DESC, trigger_kind, bridge_type, status, portal_cleanup_status;

        SELECT jobs.trigger_kind,
               jobs.bridge_type,
               rooms.status,
               count(*) AS rooms,
               left(COALESCE(rooms.last_error, '\''none'\''), 300) AS reason
          FROM bridge_cleanup_rooms rooms
          JOIN bridge_cleanup_jobs jobs ON jobs.id = rooms.job_id
         GROUP BY jobs.trigger_kind, jobs.bridge_type, rooms.status,
                  left(COALESCE(rooms.last_error, '\''none'\''), 300)
         ORDER BY rooms DESC, jobs.trigger_kind, jobs.bridge_type, rooms.status;

        SELECT id,
               user_id,
               bridge_type,
               trigger_kind,
               status,
               portal_cleanup_status,
               left(COALESCE(portal_cleanup_error, '\''none'\''), 200) AS portal_cleanup_error,
               round(rootfs_free_before_bytes / 1048576.0, 1) AS rootfs_before_mib,
               round(rootfs_free_after_bytes / 1048576.0, 1) AS rootfs_after_mib,
               round((rootfs_free_after_bytes - rootfs_free_before_bytes) / 1048576.0, 1) AS rootfs_delta_mib,
               round(tuwunel_before_bytes / 1048576.0, 1) AS tuwunel_before_mib,
               round(tuwunel_after_bytes / 1048576.0, 1) AS tuwunel_after_mib,
               round((tuwunel_after_bytes - tuwunel_before_bytes) / 1048576.0, 1) AS tuwunel_delta_mib,
               left(COALESCE(last_error, '\''none'\''), 200) AS job_result
          FROM bridge_cleanup_jobs
         ORDER BY updated_at DESC
         LIMIT 20;

        SELECT user_id,
               bridge_type,
               lease_kind,
               to_char(to_timestamp(lease_until), '\''YYYY-MM-DD"T"HH24:MI:SS"Z"'\'') AS lease_until,
               CASE WHEN lease_until > extract(epoch from now())::INT4 THEN '\''active'\'' ELSE '\''expired'\'' END AS lease_state
          FROM bridge_connection_leases
         ORDER BY updated_at DESC;
    ' 2>/dev/null || echo "purge audit query failed"
}

print_tuwunel_file_type_buckets() {
    find /var/lib/tuwunel -xdev -type f -printf '%s %T@ %p\n' 2>/dev/null \
        | awk '
            function classify(path) {
                if (path ~ /^\/var\/lib\/tuwunel\/media(\/|$)/) {
                    return "media"
                }
                if (path ~ /\.sst$/) {
                    return "rocksdb_sst"
                }
                if (path ~ /\.blob$/) {
                    return "rocksdb_blob"
                }
                if (path ~ /\/archive\/[0-9]+\.log$/) {
                    return "rocksdb_archive_log"
                }
                if (path ~ /\/[0-9]+\.log$/) {
                    return "rocksdb_wal"
                }
                if (path ~ /\/MANIFEST-[0-9]+$/) {
                    return "rocksdb_manifest"
                }
                if (path ~ /\/OPTIONS-[0-9]+$/) {
                    return "rocksdb_options"
                }
                if (path ~ /\/LOG(\.old(\.[0-9]+)?)?$/) {
                    return "rocksdb_info_log"
                }
                if (path ~ /\/(CURRENT|IDENTITY)$/) {
                    return "rocksdb_identity"
                }
                if (path ~ /\/LOCK$/) {
                    return "rocksdb_lock"
                }
                return "other"
            }
            function add(kind, size, mtime) {
                count[kind] += 1
                bytes[kind] += size
                if (!(kind in oldest) || mtime < oldest[kind]) {
                    oldest[kind] = mtime
                }
                if (!(kind in newest) || mtime > newest[kind]) {
                    newest[kind] = mtime
                }
            }
            {
                size = $1 + 0
                mtime = int($2)
                $1 = ""
                $2 = ""
                sub(/^  /, "")
                path = $0
                add(classify(path), size, mtime)
            }
            END {
                order[1] = "media"
                order[2] = "rocksdb_sst"
                order[3] = "rocksdb_blob"
                order[4] = "rocksdb_wal"
                order[5] = "rocksdb_archive_log"
                order[6] = "rocksdb_manifest"
                order[7] = "rocksdb_options"
                order[8] = "rocksdb_info_log"
                order[9] = "rocksdb_identity"
                order[10] = "rocksdb_lock"
                order[11] = "other"
                for (i = 1; i <= 11; i++) {
                    kind = order[i]
                    if (count[kind] > 0) {
                        printf "%s count=%d bytes=%d mib=%.1f oldest_mtime_epoch=%d newest_mtime_epoch=%d\n", kind, count[kind], bytes[kind], bytes[kind] / 1048576, oldest[kind], newest[kind]
                    }
                }
            }
        ' | sort || true
}

collect_tuwunel_bucket_snapshot() {
    if [ ! -d /var/lib/tuwunel ]; then
        return 0
    fi

    find /var/lib/tuwunel -xdev -type f -printf '%s %p\n' 2>/dev/null \
        | awk '
            function classify(path) {
                if (path ~ /^\/var\/lib\/tuwunel\/media(\/|$)/) {
                    return "media"
                }
                if (path ~ /\.sst$/) {
                    return "rocksdb_sst"
                }
                if (path ~ /\.blob$/) {
                    return "rocksdb_blob"
                }
                if (path ~ /\/archive\/[0-9]+\.log$/) {
                    return "rocksdb_archive_log"
                }
                if (path ~ /\/[0-9]+\.log$/) {
                    return "rocksdb_wal"
                }
                if (path ~ /\/MANIFEST-[0-9]+$/) {
                    return "rocksdb_manifest"
                }
                if (path ~ /\/OPTIONS-[0-9]+$/) {
                    return "rocksdb_options"
                }
                if (path ~ /\/LOG(\.old(\.[0-9]+)?)?$/) {
                    return "rocksdb_info_log"
                }
                if (path ~ /\/(CURRENT|IDENTITY)$/) {
                    return "rocksdb_identity"
                }
                if (path ~ /\/LOCK$/) {
                    return "rocksdb_lock"
                }
                return "other"
            }
            {
                size = $1 + 0
                $1 = ""
                sub(/^ /, "")
                kind = classify($0)
                count[kind] += 1
                bytes[kind] += size
            }
            END {
                for (kind in bytes) {
                    printf "%s\t%d\t%d\n", kind, bytes[kind], count[kind]
                }
            }
        ' | sort
}

print_tuwunel_bucket_growth_since_last_snapshot() {
    local tmp_snapshot
    tmp_snapshot="${TUWUNEL_BUCKET_SNAPSHOT_FILE}.$$"

    collect_tuwunel_bucket_snapshot > "$tmp_snapshot" 2>/dev/null || true

    echo "--- tuwunel bucket growth since previous storage snapshot (>= ${TUWUNEL_BUCKET_GROWTH_REPORT_MIN_KB} KiB) ---"
    if [ -s "$TUWUNEL_BUCKET_SNAPSHOT_FILE" ] && [ -s "$tmp_snapshot" ]; then
        awk -F '\t' -v min_kb="$TUWUNEL_BUCKET_GROWTH_REPORT_MIN_KB" '
            FNR == NR {
                previous_bytes[$1] = $2
                previous_count[$1] = $3
                next
            }
            {
                old_bytes = (($1 in previous_bytes) ? previous_bytes[$1] : 0)
                old_count = (($1 in previous_count) ? previous_count[$1] : 0)
                delta_bytes = $2 - old_bytes
                delta_count = $3 - old_count
                delta_kb = int(delta_bytes / 1024)
                if (delta_kb >= min_kb || delta_kb <= -min_kb) {
                    printf "%+10d KiB files_delta=%+d now_bytes=%d was_bytes=%d kind=%s\n", delta_kb, delta_count, $2, old_bytes, $1
                }
            }
        ' "$TUWUNEL_BUCKET_SNAPSHOT_FILE" "$tmp_snapshot" | sort -nr || true
    else
        echo "no previous tuwunel bucket snapshot"
    fi

    mv "$tmp_snapshot" "$TUWUNEL_BUCKET_SNAPSHOT_FILE" 2>/dev/null || rm -f "$tmp_snapshot"
}

print_tuwunel_detailed_breakdown() {
    echo "--- tuwunel detailed storage ---"
    if [ ! -d /var/lib/tuwunel ]; then
        echo "not present"
        return 0
    fi

    du -sh /var/lib/tuwunel /var/lib/tuwunel/media 2>/dev/null || true

    echo "--- tuwunel file type buckets ---"
    print_tuwunel_file_type_buckets
    print_tuwunel_bucket_growth_since_last_snapshot

    echo "--- tuwunel top dirs (du -xhd2) ---"
    du -xh -d 2 /var/lib/tuwunel 2>/dev/null | sort -h | tail -60 || true

    echo "--- tuwunel top files ---"
    find /var/lib/tuwunel -xdev -type f -printf '%s %p\n' 2>/dev/null | sort -n | tail -60 || true

    echo "--- tuwunel newest files ---"
    find /var/lib/tuwunel -xdev -type f -printf '%TY-%Tm-%TdT%TH:%TM:%TSZ %s %p\n' 2>/dev/null | sort | tail -60 || true

    echo "--- tuwunel largest non-media non-sst files ---"
    find /var/lib/tuwunel -xdev -type f ! -path '/var/lib/tuwunel/media/*' ! -name '*.sst' -printf '%s %p\n' 2>/dev/null \
        | sort -n | tail -80 || true

    echo "--- tuwunel media janitor policy ---"
    printf "media_dir=%s max_bytes=%s max_mib=%.1f retention_secs=%s min_age_secs_alias=%s delete_log_limit=%s\n" \
        "$TUWUNEL_MEDIA_DIR" \
        "$TUWUNEL_MEDIA_MAX_BYTES" \
        "$(awk -v b="$TUWUNEL_MEDIA_MAX_BYTES" 'BEGIN {print b / 1048576}')" \
        "$TUWUNEL_MEDIA_RETENTION_SECS" \
        "$TUWUNEL_MEDIA_MIN_AGE_SECS" \
        "$TUWUNEL_MEDIA_DELETE_LOG_LIMIT"

    echo "--- tuwunel archive log janitor policy ---"
    printf "archive_log_dir=%s max_bytes=%s max_mib=%.1f retention_secs=%s min_age_secs=%s delete_log_limit=%s\n" \
        "$TUWUNEL_ARCHIVE_LOG_DIR" \
        "$TUWUNEL_ARCHIVE_LOG_MAX_BYTES" \
        "$(awk -v b="$TUWUNEL_ARCHIVE_LOG_MAX_BYTES" 'BEGIN {print b / 1048576}')" \
        "$TUWUNEL_ARCHIVE_LOG_RETENTION_SECS" \
        "$TUWUNEL_ARCHIVE_LOG_MIN_AGE_SECS" \
        "$TUWUNEL_ARCHIVE_LOG_DELETE_LOG_LIMIT"

    echo "--- tuwunel config storage/admin knobs ---"
    grep -E '^(database_path|database_backup_path|database_backups_to_keep|rocksdb_direct_io|rocksdb_max_log_files|rocksdb_compression_algo|rocksdb_bottommost_compression|allow_legacy_media|freeze_legacy_media|allow_federation|admin_execute|admin_execute_errors_ignore|admin_signal_execute)' \
        /etc/tuwunel/tuwunel.toml 2>/dev/null || true

    echo "--- tuwunel latest RocksDB WAL options ---"
    local latest_options
    latest_options=$(find /var/lib/tuwunel -maxdepth 1 -type f -name 'OPTIONS-*' -printf '%T@ %p\n' 2>/dev/null \
        | sort -nr \
        | awk 'NR == 1 {$1 = ""; sub(/^ /, ""); print}')
    if [ -n "$latest_options" ] && [ -f "$latest_options" ]; then
        printf "options_file=%s\n" "$latest_options"
        grep -E '^(WAL_ttl_seconds|WAL_size_limit_MB|max_total_wal_size|delete_obsolete_files_period_micros|wal_dir|recycle_log_file_num)=' \
            "$latest_options" 2>/dev/null || true
    else
        echo "no OPTIONS-* file found"
    fi
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

normalized_tuwunel_media_retention_secs() {
    if is_nonnegative_int "$TUWUNEL_MEDIA_RETENTION_SECS"; then
        echo "$TUWUNEL_MEDIA_RETENTION_SECS"
    elif is_nonnegative_int "$TUWUNEL_MEDIA_MIN_AGE_SECS"; then
        echo "$TUWUNEL_MEDIA_MIN_AGE_SECS"
    else
        echo 60
    fi
}

normalized_tuwunel_media_delete_log_limit() {
    if is_nonnegative_int "$TUWUNEL_MEDIA_DELETE_LOG_LIMIT"; then
        echo "$TUWUNEL_MEDIA_DELETE_LOG_LIMIT"
    else
        echo 200
    fi
}

normalized_tuwunel_archive_log_retention_secs() {
    if is_nonnegative_int "$TUWUNEL_ARCHIVE_LOG_RETENTION_SECS"; then
        echo "$TUWUNEL_ARCHIVE_LOG_RETENTION_SECS"
    else
        echo 21600
    fi
}

normalized_tuwunel_archive_log_max_bytes() {
    if is_nonnegative_int "$TUWUNEL_ARCHIVE_LOG_MAX_BYTES"; then
        echo "$TUWUNEL_ARCHIVE_LOG_MAX_BYTES"
    else
        echo 33554432
    fi
}

normalized_tuwunel_archive_log_min_age_secs() {
    if is_nonnegative_int "$TUWUNEL_ARCHIVE_LOG_MIN_AGE_SECS"; then
        echo "$TUWUNEL_ARCHIVE_LOG_MIN_AGE_SECS"
    else
        echo 600
    fi
}

normalized_tuwunel_archive_log_delete_log_limit() {
    if is_nonnegative_int "$TUWUNEL_ARCHIVE_LOG_DELETE_LOG_LIMIT"; then
        echo "$TUWUNEL_ARCHIVE_LOG_DELETE_LOG_LIMIT"
    else
        echo 200
    fi
}

tuwunel_media_retention_stats() {
    local cutoff_epoch="$1"
    local now_epoch="$2"

    if [ ! -d "$TUWUNEL_MEDIA_DIR" ]; then
        echo "0 0 0 0 0"
        return 0
    fi

    find "$TUWUNEL_MEDIA_DIR" -xdev -type f -printf '%T@ %s\n' 2>/dev/null \
        | awk -v cutoff="$cutoff_epoch" -v now="$now_epoch" '
            {
                mtime = int($1)
                size = $2 + 0
                total_count += 1
                total_bytes += size
                if (mtime <= cutoff) {
                    eligible_count += 1
                    eligible_bytes += size
                    age = now - mtime
                    if (age > oldest_age) {
                        oldest_age = age
                    }
                }
            }
            END {
                printf "%d %d %d %d %d\n", total_count + 0, total_bytes + 0, eligible_count + 0, eligible_bytes + 0, oldest_age + 0
            }
        '
}

check_tuwunel_media_policy() {
    if [ ! -d "$TUWUNEL_MEDIA_DIR" ]; then
        return 0
    fi

    local media_bytes media_count rc now_epoch retention_secs cutoff_epoch
    local total_count total_bytes eligible_count eligible_bytes oldest_age_secs
    rc=0

    media_bytes=$(du_bytes "$TUWUNEL_MEDIA_DIR")
    media_count=$(find "$TUWUNEL_MEDIA_DIR" -xdev -type f 2>/dev/null | wc -l | awk '{print $1 + 0}')

    if is_nonnegative_int "$TUWUNEL_MEDIA_MAX_BYTES" && [ "$TUWUNEL_MEDIA_MAX_BYTES" -gt 0 ] && [ "$media_bytes" -gt "$TUWUNEL_MEDIA_MAX_BYTES" ]; then
        printf "TUWUNEL_MEDIA_OVER_CAP path=%s bytes=%d count=%d cap_bytes=%d cap_mib=%.1f\n" \
            "$TUWUNEL_MEDIA_DIR" \
            "$media_bytes" \
            "$media_count" \
            "$TUWUNEL_MEDIA_MAX_BYTES" \
            "$(awk -v b="$TUWUNEL_MEDIA_MAX_BYTES" 'BEGIN {print b / 1048576}')"
        rc=1
    fi

    now_epoch=$(date +%s)
    retention_secs=$(normalized_tuwunel_media_retention_secs)
    cutoff_epoch=$((now_epoch - retention_secs))
    read -r total_count total_bytes eligible_count eligible_bytes oldest_age_secs < <(tuwunel_media_retention_stats "$cutoff_epoch" "$now_epoch")

    if [ "$eligible_count" -gt 0 ]; then
        printf "TUWUNEL_MEDIA_RETENTION_DUE path=%s total_count=%d total_bytes=%d eligible_count=%d eligible_bytes=%d retention_secs=%d oldest_age_secs=%d\n" \
            "$TUWUNEL_MEDIA_DIR" \
            "$total_count" \
            "$total_bytes" \
            "$eligible_count" \
            "$eligible_bytes" \
            "$retention_secs" \
            "$oldest_age_secs"
        rc=1
    fi

    return "$rc"
}

check_tuwunel_archive_log_policy() {
    if [ ! -d "$TUWUNEL_ARCHIVE_LOG_DIR" ]; then
        return 0
    fi

    local max_bytes archive_bytes archive_count rc now_epoch retention_secs cutoff_epoch
    local total_count total_bytes eligible_count eligible_bytes oldest_age_secs
    rc=0
    max_bytes=$(normalized_tuwunel_archive_log_max_bytes)
    archive_bytes=$(du_bytes "$TUWUNEL_ARCHIVE_LOG_DIR")
    archive_count=$(find "$TUWUNEL_ARCHIVE_LOG_DIR" -xdev -type f -name '*.log' 2>/dev/null | wc -l | awk '{print $1 + 0}')

    if [ "$max_bytes" -gt 0 ] && [ "$archive_bytes" -gt "$max_bytes" ]; then
        printf "TUWUNEL_ARCHIVE_LOG_OVER_CAP path=%s bytes=%d count=%d cap_bytes=%d cap_mib=%.1f\n" \
            "$TUWUNEL_ARCHIVE_LOG_DIR" \
            "$archive_bytes" \
            "$archive_count" \
            "$max_bytes" \
            "$(awk -v b="$max_bytes" 'BEGIN {print b / 1048576}')"
        rc=1
    fi

    now_epoch=$(date +%s)
    retention_secs=$(normalized_tuwunel_archive_log_retention_secs)
    cutoff_epoch=$((now_epoch - retention_secs))
    read -r total_count total_bytes eligible_count eligible_bytes oldest_age_secs < <(
        find "$TUWUNEL_ARCHIVE_LOG_DIR" -xdev -type f -name '*.log' -printf '%T@ %s\n' 2>/dev/null \
            | awk -v cutoff="$cutoff_epoch" -v now="$now_epoch" '
                {
                    mtime = int($1)
                    size = $2 + 0
                    total_count += 1
                    total_bytes += size
                    if (mtime <= cutoff) {
                        eligible_count += 1
                        eligible_bytes += size
                        age = now - mtime
                        if (age > oldest_age) {
                            oldest_age = age
                        }
                    }
                }
                END {
                    printf "%d %d %d %d %d\n", total_count + 0, total_bytes + 0, eligible_count + 0, eligible_bytes + 0, oldest_age + 0
                }
            '
    )

    if [ "$eligible_count" -gt 0 ]; then
        printf "TUWUNEL_ARCHIVE_LOG_RETENTION_DUE path=%s total_count=%d total_bytes=%d eligible_count=%d eligible_bytes=%d retention_secs=%d oldest_age_secs=%d\n" \
            "$TUWUNEL_ARCHIVE_LOG_DIR" \
            "$total_count" \
            "$total_bytes" \
            "$eligible_count" \
            "$eligible_bytes" \
            "$retention_secs" \
            "$oldest_age_secs"
        rc=1
    fi

    return "$rc"
}

cleanup_tuwunel_media() {
    echo "--- tuwunel media janitor ---"

    if [ ! -d "$TUWUNEL_MEDIA_DIR" ]; then
        echo "Tuwunel media dir not present: $TUWUNEL_MEDIA_DIR"
        return 0
    fi

    local cap_enabled
    cap_enabled=true
    if ! is_nonnegative_int "$TUWUNEL_MEDIA_MAX_BYTES" || [ "$TUWUNEL_MEDIA_MAX_BYTES" -eq 0 ]; then
        cap_enabled=false
    fi

    local retention_secs delete_log_limit
    retention_secs=$(normalized_tuwunel_media_retention_secs)
    delete_log_limit=$(normalized_tuwunel_media_delete_log_limit)

    if export_running; then
        echo "Skipping Tuwunel media cleanup while export.sh is running"
        return 0
    fi

    print_tuwunel_cleanup_metrics "before"

    local before_bytes before_count cutoff_epoch list_file now_epoch
    local eligible_count eligible_bytes deleted_count deleted_bytes failed_count logged_count suppressed_count
    before_bytes=$(du_bytes "$TUWUNEL_MEDIA_DIR")
    before_count=$(find "$TUWUNEL_MEDIA_DIR" -xdev -type f 2>/dev/null | wc -l | awk '{print $1 + 0}')

    printf "before_bytes=%d before_mib=%.1f before_count=%d cap_enabled=%s cap_bytes=%s cap_mib=%.1f retention_secs=%s delete_log_limit=%s\n" \
        "$before_bytes" \
        "$(awk -v b="$before_bytes" 'BEGIN {print b / 1048576}')" \
        "$before_count" \
        "$cap_enabled" \
        "$TUWUNEL_MEDIA_MAX_BYTES" \
        "$(awk -v b="$TUWUNEL_MEDIA_MAX_BYTES" 'BEGIN {print b / 1048576}')" \
        "$retention_secs" \
        "$delete_log_limit"

    now_epoch=$(date +%s)
    cutoff_epoch=$((now_epoch - retention_secs))
    list_file="/tmp/tuwunel-media-prune.$$"
    find "$TUWUNEL_MEDIA_DIR" -xdev -type f -printf '%T@ %s %p\n' 2>/dev/null | sort -n > "$list_file" || true

    eligible_count=0
    eligible_bytes=0
    deleted_count=0
    deleted_bytes=0
    failed_count=0
    logged_count=0
    suppressed_count=0

    while IFS=' ' read -r mtime size path; do
        [ -n "${path:-}" ] || continue

        local mtime_epoch
        mtime_epoch="${mtime%.*}"
        if ! is_nonnegative_int "$mtime_epoch" || [ "$mtime_epoch" -gt "$cutoff_epoch" ]; then
            continue
        fi

        eligible_count=$((eligible_count + 1))
        eligible_bytes=$((eligible_bytes + size))

        if rm -f -- "$path" 2>/dev/null; then
            deleted_count=$((deleted_count + 1))
            deleted_bytes=$((deleted_bytes + size))
            if [ "$logged_count" -lt "$delete_log_limit" ]; then
                printf "deleted_tuwunel_media bytes=%s age_secs=%d path=%s\n" "$size" "$((now_epoch - mtime_epoch))" "$path"
                logged_count=$((logged_count + 1))
            else
                suppressed_count=$((suppressed_count + 1))
            fi
        else
            failed_count=$((failed_count + 1))
            printf "failed_delete_tuwunel_media bytes=%s age_secs=%d path=%s\n" "$size" "$((now_epoch - mtime_epoch))" "$path"
        fi
    done < "$list_file"

    rm -f "$list_file" 2>/dev/null || true
    find "$TUWUNEL_MEDIA_DIR" -xdev -mindepth 1 -type d -empty -delete 2>/dev/null || true

    local after_bytes after_count
    after_bytes=$(du_bytes "$TUWUNEL_MEDIA_DIR")
    after_count=$(find "$TUWUNEL_MEDIA_DIR" -xdev -type f 2>/dev/null | wc -l | awk '{print $1 + 0}')

    printf "eligible_count=%d eligible_bytes=%d eligible_mib=%.1f deleted_count=%d deleted_bytes=%d deleted_mib=%.1f failed_count=%d suppressed_delete_log_count=%d after_bytes=%d after_mib=%.1f after_count=%d\n" \
        "$eligible_count" \
        "$eligible_bytes" \
        "$(awk -v b="$eligible_bytes" 'BEGIN {print b / 1048576}')" \
        "$deleted_count" \
        "$deleted_bytes" \
        "$(awk -v b="$deleted_bytes" 'BEGIN {print b / 1048576}')" \
        "$failed_count" \
        "$suppressed_count" \
        "$after_bytes" \
        "$(awk -v b="$after_bytes" 'BEGIN {print b / 1048576}')" \
        "$after_count"

    print_tuwunel_cleanup_metrics "after"

    if [ "$eligible_count" -eq 0 ]; then
        echo "No Tuwunel media files older than retention_secs"
    fi

    if [ "$suppressed_count" -gt 0 ]; then
        echo "Suppressed per-file delete logs after limit; all suppressed files are included in deleted_count/deleted_bytes"
    fi

    if [ "$cap_enabled" = "true" ] && [ "$after_bytes" -gt "$TUWUNEL_MEDIA_MAX_BYTES" ]; then
        echo "Tuwunel media remains over cap; remaining files are newer than min_age_secs or could not be deleted"
    fi
}

cleanup_tuwunel_archive_logs() {
    echo "--- tuwunel archive log janitor ---"

    if [ ! -d "$TUWUNEL_ARCHIVE_LOG_DIR" ]; then
        echo "Tuwunel archive log dir not present: $TUWUNEL_ARCHIVE_LOG_DIR"
        return 0
    fi

    if export_running; then
        echo "Skipping Tuwunel archive log cleanup while export.sh is running"
        return 0
    fi

    local max_bytes retention_secs min_age_secs delete_log_limit
    max_bytes=$(normalized_tuwunel_archive_log_max_bytes)
    retention_secs=$(normalized_tuwunel_archive_log_retention_secs)
    min_age_secs=$(normalized_tuwunel_archive_log_min_age_secs)
    delete_log_limit=$(normalized_tuwunel_archive_log_delete_log_limit)

    print_tuwunel_cleanup_metrics "archive-before"

    local before_bytes before_count cutoff_epoch list_file now_epoch projected_bytes
    local eligible_count eligible_bytes deleted_count deleted_bytes failed_count logged_count suppressed_count
    before_bytes=$(du_bytes "$TUWUNEL_ARCHIVE_LOG_DIR")
    before_count=$(find "$TUWUNEL_ARCHIVE_LOG_DIR" -xdev -type f -name '*.log' 2>/dev/null | wc -l | awk '{print $1 + 0}')

    printf "before_bytes=%d before_mib=%.1f before_count=%d cap_bytes=%s cap_mib=%.1f retention_secs=%s min_age_secs=%s delete_log_limit=%s\n" \
        "$before_bytes" \
        "$(awk -v b="$before_bytes" 'BEGIN {print b / 1048576}')" \
        "$before_count" \
        "$max_bytes" \
        "$(awk -v b="$max_bytes" 'BEGIN {print b / 1048576}')" \
        "$retention_secs" \
        "$min_age_secs" \
        "$delete_log_limit"

    now_epoch=$(date +%s)
    cutoff_epoch=$((now_epoch - retention_secs))
    projected_bytes="$before_bytes"
    list_file="/tmp/tuwunel-archive-log-prune.$$"
    find "$TUWUNEL_ARCHIVE_LOG_DIR" -xdev -type f -name '*.log' -printf '%T@ %s %p\n' 2>/dev/null | sort -n > "$list_file" || true

    eligible_count=0
    eligible_bytes=0
    deleted_count=0
    deleted_bytes=0
    failed_count=0
    logged_count=0
    suppressed_count=0

    while IFS=' ' read -r mtime size path; do
        [ -n "${path:-}" ] || continue

        local mtime_epoch should_delete reason
        mtime_epoch="${mtime%.*}"
        should_delete=false
        reason=""
        if ! is_nonnegative_int "$mtime_epoch"; then
            continue
        fi

        if [ "$mtime_epoch" -le "$cutoff_epoch" ]; then
            should_delete=true
            reason="retention"
        elif [ "$max_bytes" -gt 0 ] && [ "$projected_bytes" -gt "$max_bytes" ] && [ "$((now_epoch - mtime_epoch))" -ge "$min_age_secs" ]; then
            should_delete=true
            reason="cap"
        fi

        if [ "$should_delete" != "true" ]; then
            continue
        fi

        eligible_count=$((eligible_count + 1))
        eligible_bytes=$((eligible_bytes + size))

        if rm -f -- "$path" 2>/dev/null; then
            deleted_count=$((deleted_count + 1))
            deleted_bytes=$((deleted_bytes + size))
            projected_bytes=$((projected_bytes - size))
            if [ "$logged_count" -lt "$delete_log_limit" ]; then
                printf "deleted_tuwunel_archive_log reason=%s bytes=%s age_secs=%d projected_bytes=%d path=%s\n" \
                    "$reason" \
                    "$size" \
                    "$((now_epoch - mtime_epoch))" \
                    "$projected_bytes" \
                    "$path"
                logged_count=$((logged_count + 1))
            else
                suppressed_count=$((suppressed_count + 1))
            fi
        else
            failed_count=$((failed_count + 1))
            printf "failed_delete_tuwunel_archive_log reason=%s bytes=%s age_secs=%d path=%s\n" "$reason" "$size" "$((now_epoch - mtime_epoch))" "$path"
        fi
    done < "$list_file"

    rm -f "$list_file" 2>/dev/null || true
    find "$TUWUNEL_ARCHIVE_LOG_DIR" -xdev -mindepth 1 -type d -empty -delete 2>/dev/null || true

    local after_bytes after_count
    after_bytes=$(du_bytes "$TUWUNEL_ARCHIVE_LOG_DIR")
    after_count=$(find "$TUWUNEL_ARCHIVE_LOG_DIR" -xdev -type f -name '*.log' 2>/dev/null | wc -l | awk '{print $1 + 0}')

    printf "eligible_count=%d eligible_bytes=%d eligible_mib=%.1f deleted_count=%d deleted_bytes=%d deleted_mib=%.1f failed_count=%d suppressed_delete_log_count=%d after_bytes=%d after_mib=%.1f after_count=%d\n" \
        "$eligible_count" \
        "$eligible_bytes" \
        "$(awk -v b="$eligible_bytes" 'BEGIN {print b / 1048576}')" \
        "$deleted_count" \
        "$deleted_bytes" \
        "$(awk -v b="$deleted_bytes" 'BEGIN {print b / 1048576}')" \
        "$failed_count" \
        "$suppressed_count" \
        "$after_bytes" \
        "$(awk -v b="$after_bytes" 'BEGIN {print b / 1048576}')" \
        "$after_count"

    print_tuwunel_cleanup_metrics "archive-after"

    if [ "$eligible_count" -eq 0 ]; then
        echo "No Tuwunel archive logs older than retention_secs or over cap"
    fi

    if [ "$suppressed_count" -gt 0 ]; then
        echo "Suppressed per-file archive log delete logs after limit; all suppressed files are included in deleted_count/deleted_bytes"
    fi
}

du_bytes() {
    local path="$1"
    if [ ! -e "$path" ]; then
        echo 0
        return 0
    fi

    local bytes
    bytes=$(du -sb "$path" 2>/dev/null | awk 'NR == 1 {print $1 + 0; seen = 1} END {if (!seen) print ""}')
    if [ -n "$bytes" ]; then
        echo "$bytes"
        return 0
    fi

    if [ -f "$path" ]; then
        bytes=$(stat -c%s "$path" 2>/dev/null || stat -f%z "$path" 2>/dev/null || echo 0)
        echo "${bytes:-0}"
        return 0
    fi

    bytes=$(find "$path" -xdev -type f -exec stat -c%s {} + 2>/dev/null | awk '{sum += $1} END {print sum + 0}')
    if [ -n "$bytes" ] && [ "$bytes" -gt 0 ]; then
        echo "$bytes"
        return 0
    fi

    bytes=$(find "$path" -xdev -type f -exec stat -f%z {} + 2>/dev/null | awk '{sum += $1} END {print sum + 0}')
    echo "${bytes:-0}"
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

export_running() {
    if command -v pgrep >/dev/null 2>&1 && pgrep -f '/app/export.sh' >/dev/null 2>&1; then
        return 0
    fi

    # shellcheck disable=SC2009
    ps aux 2>/dev/null | grep -F '/app/export.sh' | grep -v grep >/dev/null 2>&1
}

cleanup_backup_artifacts() {
    if ! command -v flock >/dev/null 2>&1; then
        echo "Skipping backup artifact cleanup: flock is unavailable"
        return 0
    fi

    # This is the authoritative synchronization boundary with export.sh.
    # Process-name checks are diagnostic only and cannot safely guard deletion.
    exec 9>"$BACKUP_ARTIFACT_LOCK_FILE" || {
        echo "Skipping backup artifact cleanup: cannot open lock $BACKUP_ARTIFACT_LOCK_FILE"
        return 0
    }
    if ! flock -n 9; then
        echo "Skipping backup artifact cleanup: export holds lock $BACKUP_ARTIFACT_LOCK_FILE"
        exec 9>&-
        return 0
    fi

    echo "Backup artifact cleanup lock acquired: $BACKUP_ARTIFACT_LOCK_FILE"
    rm -rf "$BACKUP_STAGING_ROOT" "$BACKUP_TMP_ROOT/verify.tar.gz" \
        "$BACKUP_TMP_ROOT"/lightfriend-full-backup-*.tar.gz 2>/dev/null || true
    find "$BACKUP_TMP_ROOT" -maxdepth 1 -name 'lightfriend-full-backup-*.verify.tar.gz' -delete 2>/dev/null || true
    find "$BACKUP_TMP_ROOT" -maxdepth 1 -name 'lightfriend-full-backup-*.tar.gz.enc' -mmin +30 -delete 2>/dev/null || true
    rm -rf "$BACKUP_RESTORE_ROOT" 2>/dev/null || true
    find "$BACKUP_SEED_DIR" -name 'lightfriend-full-backup-*.tar.gz.enc' -mmin +30 -delete 2>/dev/null || true

    if [ -d "$TUWUNEL_BACKUP_DIR" ]; then
        echo "Removing stale $TUWUNEL_BACKUP_DIR"
        rm -rf "$TUWUNEL_BACKUP_DIR" 2>/dev/null || true
    fi

    flock -u 9 || true
    exec 9>&-
}

rootfs_reserve_kib() {
    local reserve_bytes
    reserve_bytes=0

    if [ -e "$ROOTFS_RESERVE_FILE" ]; then
        reserve_bytes=$(stat -c%s "$ROOTFS_RESERVE_FILE" 2>/dev/null || echo 0)
    fi

    reserve_bytes=${reserve_bytes:-0}
    echo $(((reserve_bytes + 1023) / 1024))
}

path_uses_rootfs() {
    local path="$1"
    local root_source path_source

    root_source=$(df -Pk / 2>/dev/null | awk 'NR == 2 {print $1}')
    path_source=$(df -Pk "$path" 2>/dev/null | awk 'NR == 2 {print $1}')

    [ -n "$root_source" ] && [ -n "$path_source" ] && [ "$root_source" = "$path_source" ]
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
TUWUNEL_ROCKSDB_BLOB_COUNT=0
TUWUNEL_ROCKSDB_BLOB_BYTES=0
TUWUNEL_ROCKSDB_WAL_COUNT=0
TUWUNEL_ROCKSDB_WAL_BYTES=0
TUWUNEL_ROCKSDB_ARCHIVE_LOG_COUNT=0
TUWUNEL_ROCKSDB_ARCHIVE_LOG_BYTES=0
TUWUNEL_ROCKSDB_MANIFEST_COUNT=0
TUWUNEL_ROCKSDB_MANIFEST_BYTES=0
TUWUNEL_ROCKSDB_OPTIONS_COUNT=0
TUWUNEL_ROCKSDB_OPTIONS_BYTES=0
TUWUNEL_ROCKSDB_INFO_LOG_COUNT=0
TUWUNEL_ROCKSDB_INFO_LOG_BYTES=0
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
            function add_meta(kind, size) {
                add(kind, size)
                add("ROCKSDB_META_LOGS", size)
            }
            {
                size = $1 + 0
                $1 = ""
                sub(/^ /, "")
                path = $0

                if (path ~ /^\/var\/lib\/tuwunel\/media(\/|$)/) {
                    add("MEDIA", size)
                } else if (path ~ /\.sst$/) {
                    add("ROCKSDB_SST", size)
                } else if (path ~ /\.blob$/) {
                    add("ROCKSDB_BLOB", size)
                } else if (path ~ /\/archive\/[0-9]+\.log$/) {
                    add_meta("ROCKSDB_ARCHIVE_LOG", size)
                } else if (path ~ /\/[0-9]+\.log$/) {
                    add_meta("ROCKSDB_WAL", size)
                } else if (path ~ /\/MANIFEST-[0-9]+$/) {
                    add_meta("ROCKSDB_MANIFEST", size)
                } else if (path ~ /\/OPTIONS-[0-9]+$/) {
                    add_meta("ROCKSDB_OPTIONS", size)
                } else if (path ~ /\/LOG(\.old(\.[0-9]+)?)?$/) {
                    add_meta("ROCKSDB_INFO_LOG", size)
                } else {
                    add("OTHER", size)
                }
            }
            END {
                kinds[1] = "MEDIA"
                kinds[2] = "ROCKSDB_SST"
                kinds[3] = "ROCKSDB_META_LOGS"
                kinds[4] = "ROCKSDB_BLOB"
                kinds[5] = "ROCKSDB_WAL"
                kinds[6] = "ROCKSDB_ARCHIVE_LOG"
                kinds[7] = "ROCKSDB_MANIFEST"
                kinds[8] = "ROCKSDB_OPTIONS"
                kinds[9] = "ROCKSDB_INFO_LOG"
                kinds[10] = "OTHER"
                for (i = 1; i <= 10; i++) {
                    kind = kinds[i]
                    printf "TUWUNEL_%s_COUNT=%d\n", kind, count[kind] + 0
                    printf "TUWUNEL_%s_BYTES=%d\n", kind, bytes[kind] + 0
                }
            }
        '
}

print_tuwunel_cleanup_metrics() {
    local phase="$1"
    local root_size root_used root_avail root_pct tmp_size tmp_used tmp_avail tmp_pct
    local tuwunel_total_bytes postgres_bytes tuwunel_backup_bytes supervisor_logs_bytes

    read -r root_size root_used root_avail root_pct < <(df_metrics /)
    read -r tmp_size tmp_used tmp_avail tmp_pct < <(df_metrics /tmp)

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
    local TUWUNEL_ROCKSDB_BLOB_COUNT=0
    local TUWUNEL_ROCKSDB_BLOB_BYTES=0
    local TUWUNEL_ROCKSDB_WAL_COUNT=0
    local TUWUNEL_ROCKSDB_WAL_BYTES=0
    local TUWUNEL_ROCKSDB_ARCHIVE_LOG_COUNT=0
    local TUWUNEL_ROCKSDB_ARCHIVE_LOG_BYTES=0
    local TUWUNEL_ROCKSDB_MANIFEST_COUNT=0
    local TUWUNEL_ROCKSDB_MANIFEST_BYTES=0
    local TUWUNEL_ROCKSDB_OPTIONS_COUNT=0
    local TUWUNEL_ROCKSDB_OPTIONS_BYTES=0
    local TUWUNEL_ROCKSDB_INFO_LOG_COUNT=0
    local TUWUNEL_ROCKSDB_INFO_LOG_BYTES=0
    local TUWUNEL_OTHER_COUNT=0
    local TUWUNEL_OTHER_BYTES=0
    eval "$(tuwunel_bucket_assignments)"

    printf "tuwunel_cleanup_metrics phase=%s root_avail_kib=%d root_use_pct=%d tmp_avail_kib=%d tmp_use_pct=%d tuwunel_total_bytes=%d tuwunel_total_mib=%.1f media_count=%d media_bytes=%d media_mib=%.1f rocksdb_sst_count=%d rocksdb_sst_bytes=%d rocksdb_sst_mib=%.1f rocksdb_meta_logs_count=%d rocksdb_meta_logs_bytes=%d rocksdb_meta_logs_mib=%.1f rocksdb_blob_count=%d rocksdb_blob_bytes=%d rocksdb_blob_mib=%.1f rocksdb_wal_count=%d rocksdb_wal_bytes=%d rocksdb_wal_mib=%.1f rocksdb_archive_log_count=%d rocksdb_archive_log_bytes=%d rocksdb_archive_log_mib=%.1f rocksdb_manifest_count=%d rocksdb_manifest_bytes=%d rocksdb_manifest_mib=%.1f rocksdb_options_count=%d rocksdb_options_bytes=%d rocksdb_options_mib=%.1f rocksdb_info_log_count=%d rocksdb_info_log_bytes=%d rocksdb_info_log_mib=%.1f other_count=%d other_bytes=%d other_mib=%.1f postgres_bytes=%d postgres_mib=%.1f tuwunel_backup_engine_bytes=%d tuwunel_backup_engine_mib=%.1f supervisor_logs_bytes=%d supervisor_logs_mib=%.1f\n" \
        "$phase" \
        "$root_avail" \
        "$root_pct" \
        "$tmp_avail" \
        "$tmp_pct" \
        "$tuwunel_total_bytes" \
        "$(awk -v b="$tuwunel_total_bytes" 'BEGIN {print b / 1048576}')" \
        "$TUWUNEL_MEDIA_COUNT" \
        "$TUWUNEL_MEDIA_BYTES" \
        "$(awk -v b="$TUWUNEL_MEDIA_BYTES" 'BEGIN {print b / 1048576}')" \
        "$TUWUNEL_ROCKSDB_SST_COUNT" \
        "$TUWUNEL_ROCKSDB_SST_BYTES" \
        "$(awk -v b="$TUWUNEL_ROCKSDB_SST_BYTES" 'BEGIN {print b / 1048576}')" \
        "$TUWUNEL_ROCKSDB_META_LOGS_COUNT" \
        "$TUWUNEL_ROCKSDB_META_LOGS_BYTES" \
        "$(awk -v b="$TUWUNEL_ROCKSDB_META_LOGS_BYTES" 'BEGIN {print b / 1048576}')" \
        "$TUWUNEL_ROCKSDB_BLOB_COUNT" \
        "$TUWUNEL_ROCKSDB_BLOB_BYTES" \
        "$(awk -v b="$TUWUNEL_ROCKSDB_BLOB_BYTES" 'BEGIN {print b / 1048576}')" \
        "$TUWUNEL_ROCKSDB_WAL_COUNT" \
        "$TUWUNEL_ROCKSDB_WAL_BYTES" \
        "$(awk -v b="$TUWUNEL_ROCKSDB_WAL_BYTES" 'BEGIN {print b / 1048576}')" \
        "$TUWUNEL_ROCKSDB_ARCHIVE_LOG_COUNT" \
        "$TUWUNEL_ROCKSDB_ARCHIVE_LOG_BYTES" \
        "$(awk -v b="$TUWUNEL_ROCKSDB_ARCHIVE_LOG_BYTES" 'BEGIN {print b / 1048576}')" \
        "$TUWUNEL_ROCKSDB_MANIFEST_COUNT" \
        "$TUWUNEL_ROCKSDB_MANIFEST_BYTES" \
        "$(awk -v b="$TUWUNEL_ROCKSDB_MANIFEST_BYTES" 'BEGIN {print b / 1048576}')" \
        "$TUWUNEL_ROCKSDB_OPTIONS_COUNT" \
        "$TUWUNEL_ROCKSDB_OPTIONS_BYTES" \
        "$(awk -v b="$TUWUNEL_ROCKSDB_OPTIONS_BYTES" 'BEGIN {print b / 1048576}')" \
        "$TUWUNEL_ROCKSDB_INFO_LOG_COUNT" \
        "$TUWUNEL_ROCKSDB_INFO_LOG_BYTES" \
        "$(awk -v b="$TUWUNEL_ROCKSDB_INFO_LOG_BYTES" 'BEGIN {print b / 1048576}')" \
        "$TUWUNEL_OTHER_COUNT" \
        "$TUWUNEL_OTHER_BYTES" \
        "$(awk -v b="$TUWUNEL_OTHER_BYTES" 'BEGIN {print b / 1048576}')" \
        "$postgres_bytes" \
        "$(awk -v b="$postgres_bytes" 'BEGIN {print b / 1048576}')" \
        "$tuwunel_backup_bytes" \
        "$(awk -v b="$tuwunel_backup_bytes" 'BEGIN {print b / 1048576}')" \
        "$supervisor_logs_bytes" \
        "$(awk -v b="$supervisor_logs_bytes" 'BEGIN {print b / 1048576}')"
}

print_json_metrics() {
    local timestamp reserve_file reserve_file_json reserve_present
    local root_size root_used root_avail root_pct tmp_size tmp_used tmp_avail tmp_pct
    local reserve_bytes reserve_kib projected_root_avail_kib
    local tuwunel_total_bytes postgres_bytes tuwunel_backup_bytes supervisor_logs_bytes

    timestamp="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    reserve_file="$ROOTFS_RESERVE_FILE"
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
    local TUWUNEL_ROCKSDB_ARCHIVE_LOG_COUNT=0
    local TUWUNEL_ROCKSDB_ARCHIVE_LOG_BYTES=0
    local TUWUNEL_OTHER_COUNT=0
    local TUWUNEL_OTHER_BYTES=0
    eval "$(tuwunel_bucket_assignments)"

    cat <<EOF
{"timestamp":"${timestamp}","filesystems":{"root":{"size_kib":${root_size},"used_kib":${root_used},"avail_kib":${root_avail},"use_pct":${root_pct}},"tmp":{"size_kib":${tmp_size},"used_kib":${tmp_used},"avail_kib":${tmp_avail},"use_pct":${tmp_pct}}},"reserve":{"path":"${reserve_file_json}","present":${reserve_present},"bytes":${reserve_bytes},"projected_root_avail_kib":${projected_root_avail_kib}},"tuwunel":{"total_bytes":${tuwunel_total_bytes},"media":{"count":${TUWUNEL_MEDIA_COUNT},"bytes":${TUWUNEL_MEDIA_BYTES}},"rocksdb_sst":{"count":${TUWUNEL_ROCKSDB_SST_COUNT},"bytes":${TUWUNEL_ROCKSDB_SST_BYTES}},"rocksdb_archive_log":{"count":${TUWUNEL_ROCKSDB_ARCHIVE_LOG_COUNT},"bytes":${TUWUNEL_ROCKSDB_ARCHIVE_LOG_BYTES}},"rocksdb_meta_logs":{"count":${TUWUNEL_ROCKSDB_META_LOGS_COUNT},"bytes":${TUWUNEL_ROCKSDB_META_LOGS_BYTES}},"other":{"count":${TUWUNEL_OTHER_COUNT},"bytes":${TUWUNEL_OTHER_BYTES}}},"postgres":{"bytes":${postgres_bytes}},"tuwunel_backup_engine":{"bytes":${tuwunel_backup_bytes}},"supervisor_logs":{"bytes":${supervisor_logs_bytes}}}
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
    print_storage_size_summary
    print_exact_root_accounting
    print_hot_path_accounting
    print_deleted_open_file_accounting
    print_tuwunel_purge_audit
    print_tuwunel_detailed_breakdown
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

    local avail_kb avail_inodes effective_avail_kb reserve_kib
    avail_kb=$(df -Pk "$path" 2>/dev/null | awk 'NR==2 {print $4}')
    avail_inodes=$(df -Pi "$path" 2>/dev/null | awk 'NR==2 {print $4}')
    effective_avail_kb="$avail_kb"
    reserve_kib=0

    if [ -n "$avail_kb" ] && path_uses_rootfs "$path"; then
        reserve_kib=$(rootfs_reserve_kib)
        effective_avail_kb=$((avail_kb + reserve_kib))
    fi

    if [ -n "$avail_kb" ] && [ "$effective_avail_kb" -lt "$MIN_FREE_KB" ]; then
        echo "LOW_SPACE path=$path avail_kb=$avail_kb reserve_kib=$reserve_kib effective_avail_kb=$effective_avail_kb threshold_kb=$MIN_FREE_KB"
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
    check_tuwunel_media_policy || rc=1
    check_tuwunel_archive_log_policy || rc=1
    return "$rc"
}

cleanup_storage() {
    echo "=== Storage Cleanup $(date -u +%Y-%m-%dT%H:%M:%SZ) ==="

    cleanup_backup_artifacts

    cleanup_tuwunel_media
    cleanup_tuwunel_archive_logs

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
    cleanup-media)
        cleanup_tuwunel_media
        ;;
    cleanup-archive-logs)
        cleanup_tuwunel_archive_logs
        ;;
    cleanup-backup-artifacts)
        cleanup_backup_artifacts
        ;;
    json)
        print_json_metrics
        ;;
    *)
        echo "Usage: $0 [report|check|cleanup|cleanup-media|cleanup-archive-logs|cleanup-backup-artifacts|json]" >&2
        exit 2
        ;;
esac
