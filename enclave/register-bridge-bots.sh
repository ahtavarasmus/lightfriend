#!/bin/bash
# Register bridge bot users in Tuwunel
#
# Tuwunel (unlike Synapse) does not auto-create appservice bot users.
# This script registers them via the Matrix appservice API.
# Only needs to run once after first 'docker compose up'.

set -e

HOMESERVER_URL="${1:-http://localhost:8008}"

echo "Registering bridge bot users in Tuwunel at ${HOMESERVER_URL}..."
echo ""

# Wait for Tuwunel to be ready
echo "Waiting for Tuwunel to be ready..."
for i in $(seq 1 30); do
    if curl -sf "${HOMESERVER_URL}/_matrix/client/versions" > /dev/null 2>&1; then
        echo "Tuwunel is ready"
        break
    fi
    if [ "$i" -eq 30 ]; then
        echo "ERROR: Tuwunel not responding after 30 seconds"
        exit 1
    fi
    sleep 1
done

# Function to extract a YAML value by key
extract_yaml_value() {
    local file="$1"
    local key="$2"
    grep "^${key}:" "$file" | sed "s/^${key}: *//" | tr -d '"' | tr -d "'"
}

# Bridge bot configs: registration_file bot_username
bridges=(
    "bridges/whatsapp/whatsapp-registration.yaml whatsappbot"
    "bridges/signal/signal-registration.yaml signalbot"
    "bridges/telegram/telegram-registration.yaml telegrambot"
)

for entry in "${bridges[@]}"; do
    reg_file=$(echo "$entry" | awk '{print $1}')
    bot_name=$(echo "$entry" | awk '{print $2}')

    if [ ! -f "$reg_file" ]; then
        echo "WARNING: ${reg_file} not found, skipping ${bot_name}"
        continue
    fi

    as_token=$(extract_yaml_value "$reg_file" "as_token")

    if [ -z "$as_token" ]; then
        echo "WARNING: No as_token found in ${reg_file}, skipping ${bot_name}"
        continue
    fi

    # Try to register the bot user via appservice API
    http_code=$(curl -s -o /tmp/bridge_reg_response.json -w "%{http_code}" -X POST \
        "${HOMESERVER_URL}/_matrix/client/v3/register" \
        -H "Content-Type: application/json" \
        -H "Authorization: Bearer ${as_token}" \
        -d "{\"username\":\"${bot_name}\",\"type\":\"m.login.application_service\"}" 2>&1) || true

    body=$(cat /tmp/bridge_reg_response.json 2>/dev/null || echo "")

    if [ "$http_code" = "200" ]; then
        echo "Registered @${bot_name}:localhost"
    elif echo "$body" | grep -q "M_USER_IN_USE"; then
        echo "Already exists: @${bot_name}:localhost (OK)"
    else
        echo "WARNING: Failed to register @${bot_name}:localhost (HTTP ${http_code}): ${body}"
    fi
done

echo ""
echo "Bridge bot registration complete."
echo "You may need to restart the bridges: docker compose restart mautrix-whatsapp mautrix-signal mautrix-telegram"
