#!/bin/bash
# S3 signal-based export watcher for Nitro enclave.
#
# Polls the host's seed HTTP server (port 9080, bridged via VSOCK) for an
# export-request.json file. When found, runs export.sh, then writes a
# completion marker back to the host via HTTP PUT (port 9081, bridged via
# VSOCK).
#
# This replaces the old VSOCK port 9003 socat-based trigger which was
# unreliable for long-running exports because VSOCK drops idle connections.

set -uo pipefail

POLL_URL="http://127.0.0.1:9080/export-request.json"
RESULT_UPLOAD_URL="http://127.0.0.1:9081/upload"
POLL_INTERVAL=5

LAST_PROCESSED=""

echo "export-watcher: starting (poll every ${POLL_INTERVAL}s)"
echo "export-watcher: poll URL = ${POLL_URL}"
echo "export-watcher: result upload URL = ${RESULT_UPLOAD_URL}"

while true; do
    # Check for export request via the seed HTTP server
    REQUEST=$(curl -sf --max-time 5 "${POLL_URL}" 2>/dev/null || true)

    if [ -n "${REQUEST}" ] && [ "${REQUEST}" != "${LAST_PROCESSED}" ] && echo "${REQUEST}" | jq -e '.action == "export"' >/dev/null 2>&1; then
        TIMESTAMP=$(date -u +%Y-%m-%dT%H:%M:%SZ)
        echo "export-watcher: export request received at ${TIMESTAMP}"
        echo "export-watcher: request payload: ${REQUEST}"

        # Write in-progress marker so the host knows we started
        echo "{\"status\":\"IN_PROGRESS\",\"started_at\":\"${TIMESTAMP}\"}" > /tmp/export-complete-payload.json
        curl -sf --max-time 10 -T /tmp/export-complete-payload.json "${RESULT_UPLOAD_URL}/export-complete.json" 2>/dev/null || true

        # Run the existing export logic
        /app/export.sh 2>&1 | tee /tmp/export-watcher-last-run.log
        EXIT_CODE=${PIPESTATUS[0]}

        FINISHED_AT=$(date -u +%Y-%m-%dT%H:%M:%SZ)

        if [ ${EXIT_CODE} -eq 0 ]; then
            echo "export-watcher: export succeeded at ${FINISHED_AT}"

            # Read the status file that export.sh wrote for detailed info
            STATUS_JSON=""
            if [ -f /data/seed/export-status.json ]; then
                STATUS_JSON=$(cat /data/seed/export-status.json 2>/dev/null || echo "")
            fi

            # Build completion payload with details from export-status.json
            if [ -n "${STATUS_JSON}" ]; then
                BACKUP_FILE=$(echo "${STATUS_JSON}" | jq -r '.file // ""')
                BACKUP_SIZE=$(echo "${STATUS_JSON}" | jq -r '.size_bytes // 0')
                USER_COUNT=$(echo "${STATUS_JSON}" | jq -r '.user_count // 0')
                COMPLETION="{\"status\":\"SUCCESS\",\"started_at\":\"${TIMESTAMP}\",\"finished_at\":\"${FINISHED_AT}\",\"file\":\"${BACKUP_FILE}\",\"size_bytes\":${BACKUP_SIZE},\"user_count\":${USER_COUNT}}"
            else
                COMPLETION="{\"status\":\"SUCCESS\",\"started_at\":\"${TIMESTAMP}\",\"finished_at\":\"${FINISHED_AT}\"}"
            fi

            echo "${COMPLETION}" > /tmp/export-complete-payload.json && curl -sf --max-time 10 -T /tmp/export-complete-payload.json "${RESULT_UPLOAD_URL}/export-complete.json" 2>/dev/null
            if [ $? -ne 0 ]; then
                echo "export-watcher: WARNING - failed to upload completion marker, retrying..."
                sleep 2
                echo "${COMPLETION}" > /tmp/export-complete-payload.json && curl -sf --max-time 10 -T /tmp/export-complete-payload.json "${RESULT_UPLOAD_URL}/export-complete.json" 2>/dev/null || \
                    echo "export-watcher: ERROR - completion marker upload failed after retry"
            fi
        else
            echo "export-watcher: export FAILED with exit code ${EXIT_CODE} at ${FINISHED_AT}"

            # Grab last 40 lines of output for error context
            ERROR_TAIL=$(tail -40 /tmp/export-watcher-last-run.log 2>/dev/null | tr '\n' ' ' | head -c 2000 || echo "no output")

            FAILURE="{\"status\":\"FAILED\",\"exit_code\":${EXIT_CODE},\"started_at\":\"${TIMESTAMP}\",\"finished_at\":\"${FINISHED_AT}\",\"error\":\"${ERROR_TAIL}\"}"
            echo "${FAILURE}" > /tmp/export-complete-payload.json && curl -sf --max-time 10 -T /tmp/export-complete-payload.json "${RESULT_UPLOAD_URL}/export-complete.json" 2>/dev/null || \
                echo "export-watcher: ERROR - failed to upload failure marker"
        fi

        # Remember we processed this request (use timestamp as dedup key)
        LAST_PROCESSED="${REQUEST}"
        echo "export-watcher: cooldown 30s before resuming poll"
        sleep 30
    fi

    sleep ${POLL_INTERVAL}
done
