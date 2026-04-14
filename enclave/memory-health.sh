#!/bin/bash
# Memory diagnostics and trend checks for the enclave.
#
# Reports per-supervisor-service RSS, total/available memory, and whether any
# service is growing fast enough to exhaust available RAM within the warning
# horizon. The output intentionally contains only process names, PIDs, and
# resource counters.

set -uo pipefail

STATE_FILE="${MEMORY_HEALTH_STATE_FILE:-/tmp/memory-health-state.tsv}"
HISTORY_FILE="${MEMORY_HEALTH_HISTORY_FILE:-/tmp/memory-health-history.log}"
WARN_HOURS="${MEMORY_WARN_HOURS:-6}"
MIN_GROWTH_KB_PER_HOUR="${MEMORY_MIN_GROWTH_KB_PER_HOUR:-10240}" # 10 MiB/h
MAX_HISTORY_BYTES="${MEMORY_MAX_HISTORY_BYTES:-1048576}"

now_epoch() {
    date +%s
}

meminfo_kb() {
    awk -v key="$1:" '$1 == key {print $2}' /proc/meminfo 2>/dev/null
}

supervisor_rows() {
    supervisorctl status 2>/dev/null | awk '
        $2 == "RUNNING" {
            pid = ""
            for (i = 1; i <= NF; i++) {
                if ($i == "pid") {
                    pid = $(i + 1)
                    gsub(",", "", pid)
                }
            }
            if (pid != "") {
                print $1 "\t" pid
            }
        }'
}

descendant_pids() {
    local root="$1"
    local queue="$root"
    local all="$root"
    local current children child

    while [ -n "$queue" ]; do
        current="${queue%% *}"
        if [ "$queue" = "$current" ]; then
            queue=""
        else
            queue="${queue#* }"
        fi

        children=$(children_for_pid "$current" | tr '\n' ' ' || true)
        for child in $children; do
            all="$all $child"
            queue="$queue $child"
        done
    done

    echo "$all"
}

children_for_pid() {
    local parent="$1"

    if command -v pgrep >/dev/null 2>&1; then
        pgrep -P "$parent" 2>/dev/null || true
        return
    fi

    for stat in /proc/[0-9]*/stat; do
        [ -r "$stat" ] || continue
        awk -v want="$parent" '
            {
                # /proc/<pid>/stat field 2 can contain spaces inside parens.
                close = match($0, /\) /)
                if (close == 0) next
                rest = substr($0, close + 2)
                split(rest, fields, " ")
                if (fields[2] == want) {
                    split(FILENAME, parts, "/")
                    print parts[3]
                }
            }' "$stat" 2>/dev/null
    done
}

rss_for_pid_kb() {
    local pid="$1"
    local rss_pages page_kb

    if [ -r "/proc/$pid/statm" ]; then
        rss_pages=$(awk '{print $2}' "/proc/$pid/statm" 2>/dev/null)
        page_kb=$(getconf PAGESIZE 2>/dev/null | awk '{print int($1 / 1024)}')
        if [ -n "$rss_pages" ] && [ -n "$page_kb" ] && [ "$page_kb" -gt 0 ]; then
            echo $((rss_pages * page_kb))
            return
        fi
    fi

    awk '/^VmRSS:/ {print $2}' "/proc/$pid/status" 2>/dev/null || true
}

rss_for_tree_kb() {
    local root="$1"
    local total=0
    local rss pid

    for pid in $(descendant_pids "$root"); do
        rss=$(rss_for_pid_kb "$pid")
        if [ -n "$rss" ]; then
            total=$((total + rss))
        fi
    done

    echo "$total"
}

rotate_history_if_needed() {
    if [ -f "$HISTORY_FILE" ] && [ "$(stat -c%s "$HISTORY_FILE" 2>/dev/null || echo 0)" -gt "$MAX_HISTORY_BYTES" ]; then
        tail -c $((MAX_HISTORY_BYTES / 2)) "$HISTORY_FILE" > "${HISTORY_FILE}.tmp" 2>/dev/null && mv "${HISTORY_FILE}.tmp" "$HISTORY_FILE"
    fi
}

write_snapshot() {
    local ts="$1"
    local tmp="${STATE_FILE}.tmp"

    mkdir -p "$(dirname "$STATE_FILE")" 2>/dev/null || true
    : > "$tmp"
    supervisor_rows | while IFS="$(printf '\t')" read -r name pid; do
        [ -n "$name" ] && [ -n "$pid" ] || continue
        rss=$(rss_for_tree_kb "$pid")
        printf '%s\t%s\t%s\t%s\n' "$name" "$pid" "$rss" "$ts" >> "$tmp"
    done
    mv "$tmp" "$STATE_FILE"
}

previous_rss_for() {
    local name="$1"
    [ -f "$STATE_FILE" ] || return 1
    awk -F '\t' -v svc="$name" '$1 == svc {print $3 "\t" $4}' "$STATE_FILE" 2>/dev/null | tail -1
}

report_memory() {
    local ts mem_total mem_available mem_used
    ts=$(now_epoch)
    mem_total=$(meminfo_kb MemTotal)
    mem_available=$(meminfo_kb MemAvailable)
    mem_used=$((mem_total - mem_available))

    echo "=== Memory Health $(date -u +%Y-%m-%dT%H:%M:%SZ) ==="
    echo "total_kb=${mem_total} used_kb=${mem_used} available_kb=${mem_available}"
    echo "--- free -h ---"
    free -h 2>/dev/null || true
    echo "--- per-service rss (including child processes) ---"
    printf '%-30s %8s %12s %12s %12s\n' "service" "pid" "rss_mb" "growth_mb_h" "eta_h"

    supervisor_rows | while IFS="$(printf '\t')" read -r name pid; do
        [ -n "$name" ] && [ -n "$pid" ] || continue
        rss=$(rss_for_tree_kb "$pid")
        prev=$(previous_rss_for "$name" || true)
        growth_kb_h=0
        eta="inf"

        if [ -n "$prev" ]; then
            prev_rss=$(echo "$prev" | awk '{print $1}')
            prev_ts=$(echo "$prev" | awk '{print $2}')
            dt=$((ts - prev_ts))
            drss=$((rss - prev_rss))
            if [ "$dt" -gt 0 ] && [ "$drss" -gt 0 ]; then
                growth_kb_h=$((drss * 3600 / dt))
                if [ "$growth_kb_h" -gt 0 ]; then
                    eta=$(awk -v avail="$mem_available" -v growth="$growth_kb_h" 'BEGIN { printf "%.1f", avail / growth }')
                fi
            fi
        fi

        rss_mb=$(awk -v kb="$rss" 'BEGIN { printf "%.1f", kb / 1024 }')
        growth_mb_h=$(awk -v kb="$growth_kb_h" 'BEGIN { printf "%.1f", kb / 1024 }')
        printf '%-30s %8s %12s %12s %12s\n' "$name" "$pid" "$rss_mb" "$growth_mb_h" "$eta"
    done

    echo "--- top processes by rss ---"
    ps -eo pid,ppid,rss,comm,args --sort=-rss 2>/dev/null | head -25 || true
}

check_memory() {
    local ts mem_available rc
    ts=$(now_epoch)
    mem_available=$(meminfo_kb MemAvailable)
    rc=0

    rotate_history_if_needed
    report_memory >> "$HISTORY_FILE" 2>&1

    while IFS="$(printf '\t')" read -r name pid; do
        [ -n "$name" ] && [ -n "$pid" ] || continue
        rss=$(rss_for_tree_kb "$pid")
        prev=$(previous_rss_for "$name" || true)

        if [ -n "$prev" ]; then
            prev_rss=$(echo "$prev" | awk '{print $1}')
            prev_ts=$(echo "$prev" | awk '{print $2}')
            dt=$((ts - prev_ts))
            drss=$((rss - prev_rss))
            if [ "$dt" -gt 0 ] && [ "$drss" -gt 0 ]; then
                growth_kb_h=$((drss * 3600 / dt))
                if [ "$growth_kb_h" -ge "$MIN_GROWTH_KB_PER_HOUR" ]; then
                    eta_tenths=$(awk -v avail="$mem_available" -v growth="$growth_kb_h" 'BEGIN { printf "%d", (avail * 10) / growth }')
                    warn_tenths=$((WARN_HOURS * 10))
                    if [ "$eta_tenths" -le "$warn_tenths" ]; then
                        eta=$(awk -v tenths="$eta_tenths" 'BEGIN { printf "%.1f", tenths / 10 }')
                        growth_mb_h=$(awk -v kb="$growth_kb_h" 'BEGIN { printf "%.1f", kb / 1024 }')
                        rss_mb=$(awk -v kb="$rss" 'BEGIN { printf "%.1f", kb / 1024 }')
                        echo "MEMORY_GROWTH_RISK service=$name pid=$pid rss_mb=$rss_mb growth_mb_h=$growth_mb_h eta_h=$eta available_kb=$mem_available"
                        rc=1
                    fi
                fi
            fi
        fi
    done < <(supervisor_rows)

    # Preserve the current snapshot after checks so each watchdog loop compares
    # against the previous loop.
    write_snapshot "$ts"
    return "$rc"
}

log_snapshot() {
    rotate_history_if_needed
    report_memory >> "$HISTORY_FILE" 2>&1
    write_snapshot "$(now_epoch)"
}

case "${1:-report}" in
    report)
        report_memory
        ;;
    check)
        check_memory
        ;;
    snapshot)
        log_snapshot
        ;;
    *)
        echo "Usage: $0 [report|check|snapshot]" >&2
        exit 2
        ;;
esac
