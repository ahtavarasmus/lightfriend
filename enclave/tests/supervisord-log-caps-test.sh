#!/bin/bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
SUPERVISORD_CONF="${SUPERVISORD_CONF:-$REPO_ROOT/enclave/supervisord.conf}"

awk '
function validate_program() {
    if (program == "") {
        return
    }

    if (stdout_logfile && (!stdout_maxbytes || !stdout_backups)) {
        printf "program %s has an uncapped stdout log\n", program > "/dev/stderr"
        failures++
    }
    if (stderr_logfile && (!stderr_maxbytes || !stderr_backups)) {
        printf "program %s has an uncapped stderr log\n", program > "/dev/stderr"
        failures++
    }
}

/^\[program:[^]]+\]$/ {
    validate_program()
    program = $0
    sub(/^\[program:/, "", program)
    sub(/\]$/, "", program)
    stdout_logfile = 0
    stdout_maxbytes = 0
    stdout_backups = 0
    stderr_logfile = 0
    stderr_maxbytes = 0
    stderr_backups = 0
    next
}

/^\[/ {
    validate_program()
    program = ""
    next
}

program != "" {
    if ($0 ~ /^stdout_logfile=/) stdout_logfile = 1
    if ($0 ~ /^stdout_logfile_maxbytes=/) stdout_maxbytes = 1
    if ($0 ~ /^stdout_logfile_backups=/) stdout_backups = 1
    if ($0 ~ /^stderr_logfile=/) stderr_logfile = 1
    if ($0 ~ /^stderr_logfile_maxbytes=/) stderr_maxbytes = 1
    if ($0 ~ /^stderr_logfile_backups=/) stderr_backups = 1
}

END {
    validate_program()
    exit failures > 0
}
' "$SUPERVISORD_CONF"

echo "supervisord log cap regression test passed"
