#!/bin/bash
# Export watcher for Nitro enclave.
#
# Polls the host's seed HTTP server for export-request.json.
# When found, runs export.sh with presigned URLs from the trigger.
# The enclave uploads everything directly to S3/R2 via curl - no host middleman.
#
# Two trigger types:
#   "deploy"  - CI generated presigned URLs, uploads backup + completion marker
#   "hourly"  - Host generated presigned URLs, uploads backup + health + R2 + tier promotions

set -uo pipefail

POLL_URL="http://127.0.0.1:9080/export-request.json"
POLL_INTERVAL=5
LAST_PROCESSED=""

echo "export-watcher: starting (poll every ${POLL_INTERVAL}s)"

storage_report_compact() {
    if [ -x /app/storage-health.sh ]; then
        /app/storage-health.sh report 2>&1 | sed 's|https://[^ ]*|[REDACTED_URL]|g' | tail -120
    else
        df -h 2>&1 || true
        df -i 2>&1 || true
    fi
}

while true; do
    REQUEST=$(curl -sf --max-time 5 "${POLL_URL}" 2>/dev/null || true)

    if [ -n "${REQUEST}" ] && [ "${REQUEST}" != "${LAST_PROCESSED}" ] && echo "${REQUEST}" | jq -e '.action == "export"' >/dev/null 2>&1; then
        TIMESTAMP=$(date -u +%Y-%m-%dT%H:%M:%SZ)
        EXPORT_TYPE=$(echo "${REQUEST}" | jq -r '.type // "unknown"')
        echo "export-watcher: ${EXPORT_TYPE} export request at ${TIMESTAMP}"

        # Extract all presigned URLs from trigger into env vars for export.sh
        export EXPORT_TYPE="${EXPORT_TYPE}"
        export BACKUP_S3_KEY=$(echo "${REQUEST}" | jq -r '.backup_s3_key // ""')
        export PRESIGNED_PUT_BACKUP_S3=$(echo "${REQUEST}" | jq -r '.presigned_put_backup_s3 // ""')
        export PRESIGNED_PUT_BACKUP_R2=$(echo "${REQUEST}" | jq -r '.presigned_put_backup_r2 // ""')
        export PRESIGNED_PUT_HEALTH=$(echo "${REQUEST}" | jq -r '.presigned_put_health // ""')
        export PRESIGNED_PUT_COMPLETE=$(echo "${REQUEST}" | jq -r '.presigned_put_complete // ""')
        export PROMOTE_JSON=$(echo "${REQUEST}" | jq -c '.promote // []')

        # Run export
        /app/export.sh 2>&1 | tee /tmp/export-watcher-last-run.log
        EXIT_CODE=${PIPESTATUS[0]}

        FINISHED_AT=$(date -u +%Y-%m-%dT%H:%M:%SZ)

        if [ ${EXIT_CODE} -eq 0 ]; then
            echo "export-watcher: ${EXPORT_TYPE} export succeeded at ${FINISHED_AT}"
        else
            echo "export-watcher: ${EXPORT_TYPE} export FAILED (exit ${EXIT_CODE}) at ${FINISHED_AT}"
            echo "export-watcher: storage report after failure:"
            storage_report_compact | tee -a /tmp/export-watcher-last-run.log >/dev/null

            # Try to upload failure status via presigned URL (use jq for safe JSON encoding)
            ERROR_TAIL=$(tail -40 /tmp/export-watcher-last-run.log 2>/dev/null | sed 's|https://[^ ]*|[REDACTED_URL]|g' | tr '\n' ' ' | head -c 4000 || echo "no output")
            STORAGE_TAIL=$(storage_report_compact | tr '\n' ' ' | head -c 4000 || echo "no storage report")

            # For deploy: write failure to completion URL
            if [ -n "${PRESIGNED_PUT_COMPLETE}" ]; then
                jq -n --arg error "$ERROR_TAIL" --arg storage "$STORAGE_TAIL" --argjson code "${EXIT_CODE}" \
                    '{"status":"FAILED","exit_code":$code,"error":$error,"storage":$storage}' | \
                    curl -sf --max-time 30 -X PUT -H "Content-Type: application/json" \
                    --data-binary @- -x http://127.0.0.1:3128 \
                    "${PRESIGNED_PUT_COMPLETE}" 2>/dev/null || true
            fi

            # For hourly: write failure to health URL
            if [ -n "${PRESIGNED_PUT_HEALTH}" ]; then
                jq -n --arg ts "${TIMESTAMP}" --arg storage "$STORAGE_TAIL" --argjson code "${EXIT_CODE}" \
                    '{"last_failure":$ts,"step":"export","exit_code":$code,"storage":$storage}' | \
                    curl -sf --max-time 30 -X PUT -H "Content-Type: application/json" \
                    --data-binary @- -x http://127.0.0.1:3128 \
                    "${PRESIGNED_PUT_HEALTH}" 2>/dev/null || true
            fi
        fi

        # Clean env vars
        unset EXPORT_TYPE BACKUP_S3_KEY PRESIGNED_PUT_BACKUP_S3 PRESIGNED_PUT_BACKUP_R2
        unset PRESIGNED_PUT_HEALTH PRESIGNED_PUT_COMPLETE PROMOTE_JSON

        LAST_PROCESSED="${REQUEST}"
        echo "export-watcher: cooldown 30s"
        sleep 30
    fi

    sleep ${POLL_INTERVAL}
done
