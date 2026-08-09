#!/usr/bin/env bash
set -euo pipefail

INSTANCE_ID="${1:?usage: install-host-log-maintenance.sh INSTANCE_ID}"
REPO_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
SCRIPT_PATH="$REPO_ROOT/terraform/modules/compute/host-log-maintenance.sh"
SERVICE_PATH="$REPO_ROOT/terraform/modules/compute/host-log-maintenance.service"
TIMER_PATH="$REPO_ROOT/terraform/modules/compute/host-log-maintenance.timer"
SCRIPT_PAYLOAD=$(base64 < "$SCRIPT_PATH" | tr -d '\n')
SERVICE_PAYLOAD=$(base64 < "$SERVICE_PATH" | tr -d '\n')
TIMER_PAYLOAD=$(base64 < "$TIMER_PATH" | tr -d '\n')
REMOTE_COMMAND="printf '%s' '$SCRIPT_PAYLOAD' | base64 -d > /opt/lightfriend/host-log-maintenance.sh && printf '%s' '$SERVICE_PAYLOAD' | base64 -d > /etc/systemd/system/lightfriend-log-maintenance.service && printf '%s' '$TIMER_PAYLOAD' | base64 -d > /etc/systemd/system/lightfriend-log-maintenance.timer && chmod 0755 /opt/lightfriend/host-log-maintenance.sh && systemctl daemon-reload && systemctl enable --now lightfriend-log-maintenance.timer && systemctl start lightfriend-log-maintenance.service && systemctl show lightfriend-log-maintenance.service -p Result -p ExecMainStatus --no-pager"
PARAMETERS=$(jq -cn --arg command "$REMOTE_COMMAND" '{commands:[$command]}')

COMMAND_ID=$(aws ssm send-command \
    --instance-ids "$INSTANCE_ID" \
    --document-name AWS-RunShellScript \
    --parameters "$PARAMETERS" \
    --query Command.CommandId \
    --output text)
if ! aws ssm wait command-executed --command-id "$COMMAND_ID" --instance-id "$INSTANCE_ID"; then
    aws ssm get-command-invocation \
        --command-id "$COMMAND_ID" \
        --instance-id "$INSTANCE_ID" \
        --query '{Status:Status,StandardOutput:StandardOutputContent,StandardError:StandardErrorContent}' \
        --output json || true
    exit 1
fi
aws ssm get-command-invocation \
    --command-id "$COMMAND_ID" \
    --instance-id "$INSTANCE_ID" \
    --query StandardOutputContent \
    --output text
