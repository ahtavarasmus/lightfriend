#!/usr/bin/env bash
set -euo pipefail

INSTANCE_ID="${1:?usage: install-host-log-maintenance.sh INSTANCE_ID}"
S3_BUCKET="${2:?usage: install-host-log-maintenance.sh INSTANCE_ID S3_BUCKET}"
if ! [[ "$INSTANCE_ID" =~ ^i-[0-9a-f]+$ && "$S3_BUCKET" =~ ^[a-z0-9][a-z0-9.-]{1,61}[a-z0-9]$ ]]; then
    echo "invalid instance ID or S3 bucket" >&2
    exit 2
fi
REPO_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
SCRIPT_PATH="$REPO_ROOT/terraform/modules/compute/host-log-maintenance.sh"
SERVICE_PATH="$REPO_ROOT/terraform/modules/compute/host-log-maintenance.service"
TIMER_PATH="$REPO_ROOT/terraform/modules/compute/host-log-maintenance.timer"
SCRIPT_PAYLOAD=$(base64 < "$SCRIPT_PATH" | tr -d '\n')
SERVICE_PAYLOAD=$(base64 < "$SERVICE_PATH" | tr -d '\n')
TIMER_PAYLOAD=$(base64 < "$TIMER_PATH" | tr -d '\n')
MARKER_KEY="deploy/host-maintenance-$INSTANCE_ID.json"
MARKER_URI="s3://$S3_BUCKET/$MARKER_KEY"
REMOTE_COMMAND="printf '%s' '$SCRIPT_PAYLOAD' | base64 -d > /opt/lightfriend/host-log-maintenance.sh && printf '%s' '$SERVICE_PAYLOAD' | base64 -d > /etc/systemd/system/lightfriend-log-maintenance.service && printf '%s' '$TIMER_PAYLOAD' | base64 -d > /etc/systemd/system/lightfriend-log-maintenance.timer && chmod 0755 /opt/lightfriend/host-log-maintenance.sh && systemctl daemon-reload && systemctl enable --now lightfriend-log-maintenance.timer && systemctl start lightfriend-log-maintenance.service && printf '{\"status\":\"SUCCESS\"}' | aws s3 cp - '$MARKER_URI'"
PARAMETERS=$(jq -cn --arg command "$REMOTE_COMMAND" '{commands:[$command]}')

aws s3 rm "$MARKER_URI" >/dev/null 2>&1 || true
aws ssm send-command \
    --instance-ids "$INSTANCE_ID" \
    --document-name AWS-RunShellScript \
    --parameters "$PARAMETERS" \
    --output json >/dev/null

for _ in $(seq 1 30); do
    RESULT=$(aws s3 cp "$MARKER_URI" - 2>/dev/null || true)
    if [ "$(printf '%s' "$RESULT" | jq -r '.status // empty')" = "SUCCESS" ]; then
        aws s3 rm "$MARKER_URI" >/dev/null
        echo "Host maintenance installed and verified on $INSTANCE_ID"
        exit 0
    fi
    sleep 2
done

echo "Host maintenance installation did not report success within 60 seconds" >&2
exit 1
