#!/bin/bash
# Called via VSOCK port 9005 from the host.
# Reads a command (enable/disable/status) from stdin and calls
# the backend's internal maintenance endpoint.

# Load env for MAINTENANCE_SECRET
if [ -f /etc/lightfriend/env ]; then
    while IFS= read -r line || [[ -n "$line" ]]; do
        [[ -z "$line" || "$line" == \#* ]] && continue
        if [[ "$line" =~ ^[A-Za-z_][A-Za-z_0-9]*= ]]; then
            export "${line?}"
        fi
    done < /etc/lightfriend/env
fi

if [ -z "${MAINTENANCE_SECRET:-}" ]; then
    echo '{"error": "MAINTENANCE_SECRET not set"}'
    exit 1
fi

read -r -t 5 COMMAND

case "$COMMAND" in
    enable)
        curl -sf -X POST -H "X-Maintenance-Secret: $MAINTENANCE_SECRET" \
            "http://localhost:${PORT:-3100}/api/internal/maintenance/enable" 2>&1
        ;;
    disable)
        curl -sf -X POST -H "X-Maintenance-Secret: $MAINTENANCE_SECRET" \
            "http://localhost:${PORT:-3100}/api/internal/maintenance/disable" 2>&1
        ;;
    *)
        curl -sf -H "X-Maintenance-Secret: $MAINTENANCE_SECRET" \
            "http://localhost:${PORT:-3100}/api/internal/maintenance/status" 2>&1
        ;;
esac
