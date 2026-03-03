#!/bin/bash
# Generate configuration files from templates using environment variables

set -e

# Check if .env exists
if [ ! -f ../.env ]; then
    echo "Error: .env file not found!"
    echo "Please create .env from .env.example and fill in the required values."
    exit 1
fi

# Load environment variables from .env
set -a
source ../.env
set +a

echo "Validating required environment variables..."

# List of required environment variables
required_vars=(
    "MATRIX_REGISTRATION_TOKEN"
    "MATRIX_HOMESERVER_SHARED_SECRET"
    "POSTGRES_PASSWORD"
    "DOUBLE_PUPPET_SECRET"
)

# Check each required variable
missing_vars=()
for var in "${required_vars[@]}"; do
    if [ -z "${!var}" ]; then
        missing_vars+=("$var")
    fi
done

# Report missing variables
if [ ${#missing_vars[@]} -gt 0 ]; then
    echo "Error: The following required environment variables are missing or empty in .env:"
    for var in "${missing_vars[@]}"; do
        echo "  - $var"
    done
    echo ""
    echo "Please add these variables to your .env file."
    echo "You can use .env.example as a reference."
    exit 1
fi

echo "All required environment variables are set"
echo ""

# Clean up old generated config files
echo "Cleaning up old generated config files..."
rm -f tuwunel/tuwunel.toml 2>/dev/null || true
rm -f bridges/whatsapp/config.yaml 2>/dev/null || true
rm -f bridges/signal/config.yaml 2>/dev/null || true
rm -f bridges/telegram/config.yaml 2>/dev/null || true
rm -f bridges/doublepuppet.yaml 2>/dev/null || true
rm -f bridges/whatsapp/whatsapp-registration.yaml 2>/dev/null || true
rm -f bridges/signal/signal-registration.yaml 2>/dev/null || true
rm -f bridges/telegram/telegram-registration.yaml 2>/dev/null || true
rm -f postgres-init/init-databases.sh 2>/dev/null || true

echo "Generating configuration files from templates..."

# Function to substitute environment variables in a file
substitute_vars() {
    local template_file="$1"
    local output_file="$2"

    # Read template and substitute variables
    local content=$(cat "$template_file")

    # List of variables to substitute
    local vars=(
        "MATRIX_HOMESERVER_SHARED_SECRET"
        "MATRIX_REGISTRATION_TOKEN"
        "POSTGRES_PASSWORD"
        "DOUBLE_PUPPET_SECRET"
        "TELEGRAM_API_ID"
        "TELEGRAM_API_HASH"
        "WHATSAPP_AS_TOKEN"
        "WHATSAPP_HS_TOKEN"
        "WHATSAPP_SENDER_LOCALPART"
        "SIGNAL_AS_TOKEN"
        "SIGNAL_HS_TOKEN"
        "SIGNAL_SENDER_LOCALPART"
        "TELEGRAM_AS_TOKEN"
        "TELEGRAM_HS_TOKEN"
        "TELEGRAM_SENDER_LOCALPART"
    )

    # Substitute each variable
    for var in "${vars[@]}"; do
        local value="${!var}"
        # Escape special characters in the value for sed
        value=$(printf '%s\n' "$value" | sed 's/[&/\]/\\&/g')
        content=$(echo "$content" | sed "s/\${${var}}/${value}/g")
    done

    # Write to output file
    echo "$content" > "$output_file"
}

# Generate bridge configs first (needed to extract tokens)
for bridge in whatsapp signal telegram; do
    if [ -f "bridges/${bridge}/config.yaml.template" ]; then
        substitute_vars "bridges/${bridge}/config.yaml.template" "bridges/${bridge}/config.yaml"
        echo "Generated bridges/${bridge}/config.yaml"
    fi
done

# Generate doublepuppet registration
if [ -f "bridges/doublepuppet.yaml.template" ]; then
    substitute_vars "bridges/doublepuppet.yaml.template" "bridges/doublepuppet.yaml"
    echo "Generated bridges/doublepuppet.yaml"
fi

# Generate postgres init script
if [ -f "postgres-init/init-databases.sh.template" ]; then
    substitute_vars "postgres-init/init-databases.sh.template" "postgres-init/init-databases.sh"
    chmod +x "postgres-init/init-databases.sh"
    echo "Generated postgres-init/init-databases.sh"
fi

echo ""
echo "Auto-generating bridge registration files from configs..."

# Function to get Docker image for a bridge
get_bridge_image() {
    case "$1" in
        whatsapp)
            echo "dock.mau.dev/mautrix/whatsapp:latest"
            ;;
        signal)
            echo "dock.mau.dev/mautrix/signal:latest"
            ;;
        telegram)
            echo "dock.mau.dev/mautrix/telegram:latest"
            ;;
        *)
            echo "Unknown bridge: $1" >&2
            return 1
            ;;
    esac
}

# Function to get bridge executable name
get_bridge_executable() {
    case "$1" in
        whatsapp)
            echo "/usr/bin/mautrix-whatsapp"
            ;;
        signal)
            echo "/usr/bin/mautrix-signal"
            ;;
        telegram)
            echo "python3 -m mautrix_telegram"
            ;;
        *)
            echo "Unknown bridge: $1" >&2
            return 1
            ;;
    esac
}

# Generate registration file for each bridge
for bridge in whatsapp signal telegram; do
    config_file="bridges/${bridge}/config.yaml"
    reg_file="bridges/${bridge}/${bridge}-registration.yaml"

    if [ -f "$config_file" ]; then
        bridge_image=$(get_bridge_image "$bridge")
        bridge_executable=$(get_bridge_executable "$bridge")
        echo "Generating ${bridge}-registration.yaml..."

        # Run bridge container with -g flag to generate registration
        if docker run --rm \
            --entrypoint="" \
            -v "$(realpath bridges/${bridge}):/data:rw" \
            "$bridge_image" \
            sh -c "$bridge_executable -g -c /data/config.yaml -r /data/${bridge}-registration.yaml"; then
            # Fix permissions so validation can read the files
            chmod 644 "bridges/${bridge}/${bridge}-registration.yaml" 2>/dev/null || true
            chmod 644 "bridges/${bridge}/config.yaml" 2>/dev/null || true
            echo "Generated bridges/${bridge}/${bridge}-registration.yaml"
        else
            echo "Failed to generate ${bridge}-registration.yaml (skipping)"
        fi
    fi
done

echo ""
echo "Extracting bridge tokens for Tuwunel config..."

# Function to extract a YAML value by key from a registration file
extract_yaml_value() {
    local file="$1"
    local key="$2"
    grep "^${key}:" "$file" | sed "s/^${key}: *//" | tr -d '"' | tr -d "'"
}

# Extract tokens from generated registration files
for bridge in whatsapp signal telegram; do
    reg_file="bridges/${bridge}/${bridge}-registration.yaml"
    if [ -f "$reg_file" ]; then
        bridge_upper=$(echo "$bridge" | tr '[:lower:]' '[:upper:]')
        as_token=$(extract_yaml_value "$reg_file" "as_token")
        hs_token=$(extract_yaml_value "$reg_file" "hs_token")
        sender_localpart=$(extract_yaml_value "$reg_file" "sender_localpart")

        if [ -n "$as_token" ] && [ -n "$hs_token" ]; then
            # Export for substitute_vars to use
            export "${bridge_upper}_AS_TOKEN=${as_token}"
            export "${bridge_upper}_HS_TOKEN=${hs_token}"
            export "${bridge_upper}_SENDER_LOCALPART=${sender_localpart}"
            echo "Extracted tokens for ${bridge}"
        else
            echo "WARNING: Could not extract tokens from ${reg_file}"
        fi
    else
        echo "WARNING: ${reg_file} not found, Tuwunel config will have empty tokens for ${bridge}"
    fi
done

# Generate Tuwunel config with extracted tokens
if [ -f "tuwunel/tuwunel.toml.template" ]; then
    substitute_vars "tuwunel/tuwunel.toml.template" "tuwunel/tuwunel.toml"
    echo "Generated tuwunel/tuwunel.toml"
fi

echo ""
echo "Validating generated configuration files..."

# Check for placeholder values in config files
validation_errors=()
for bridge in whatsapp signal telegram; do
    config_file="bridges/${bridge}/config.yaml"

    if [ -f "$config_file" ]; then
        # Check for CHANGE_ME placeholders
        if grep -q "CHANGE_ME" "$config_file"; then
            validation_errors+=("$bridge: config.yaml contains CHANGE_ME placeholder")
        fi
    fi
done

# Report validation errors
if [ ${#validation_errors[@]} -gt 0 ]; then
    echo ""
    echo "ERROR: Configuration validation failed:"
    for error in "${validation_errors[@]}"; do
        echo "  $error"
    done
    echo ""
    echo "Please check your template files and .env configuration."
    exit 1
fi

echo "All configuration files are valid"

echo ""
echo "All configuration files generated successfully!"

# Start services and register bridge bots if --start flag is passed
if [ "${1:-}" = "--start" ]; then
    echo ""
    echo "Starting Docker services..."
    docker compose up -d

    echo ""
    echo "Registering bridge bot users in Tuwunel..."
    bash "$(dirname "$0")/register-bridge-bots.sh"

    echo ""
    echo "Restarting bridges to pick up registered bot users..."
    docker compose restart mautrix-whatsapp mautrix-signal mautrix-telegram

    echo ""
    echo "All services started. Check status with: docker compose ps"
else
    echo ""
    echo "To start everything:"
    echo "  bash setup-configs.sh --start"
    echo ""
    echo "Or manually:"
    echo "  1. docker compose up -d"
    echo "  2. bash register-bridge-bots.sh   (first time only)"
fi
