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
    "MATRIX_HOMESERVER_SHARED_SECRET"
    "SYNAPSE_DB_PASSWORD"
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

echo "✓ All required environment variables are set"
echo ""

# Clean up old generated files that may have been created by Docker containers
# (Docker containers run as different users, making files unwritable by host user)
echo "Cleaning up old generated config files..."
rm -f synapse/homeserver.yaml 2>/dev/null || true
rm -f bridges/whatsapp/config.yaml 2>/dev/null || true
rm -f bridges/signal/config.yaml 2>/dev/null || true
rm -f bridges/messenger/config.yaml 2>/dev/null || true
rm -f bridges/instagram/config.yaml 2>/dev/null || true
rm -f bridges/telegram/config.yaml 2>/dev/null || true
rm -f bridges/doublepuppet.yaml 2>/dev/null || true
rm -f bridges/whatsapp/whatsapp-registration.yaml 2>/dev/null || true
rm -f bridges/signal/signal-registration.yaml 2>/dev/null || true
rm -f bridges/messenger/messenger-registration.yaml 2>/dev/null || true
rm -f bridges/instagram/instagram-registration.yaml 2>/dev/null || true
rm -f bridges/telegram/telegram-registration.yaml 2>/dev/null || true
rm -f postgres-init/init-databases.sh 2>/dev/null || true

echo "Generating configuration files from templates..."

# Function to substitute environment variables in a file using envsubst
substitute_vars() {
    local template_file="$1"
    local output_file="$2"

    # Use envsubst to substitute all environment variables
    envsubst < "$template_file" > "$output_file"
}

# Generate Synapse homeserver.yaml
if [ -f "synapse/homeserver.yaml.template" ]; then
    substitute_vars "synapse/homeserver.yaml.template" "synapse/homeserver.yaml"
    echo "✓ Generated synapse/homeserver.yaml"
fi

# Generate bridge configs
for bridge in whatsapp signal messenger instagram telegram; do
    if [ -f "bridges/${bridge}/config.yaml.template" ]; then
        substitute_vars "bridges/${bridge}/config.yaml.template" "bridges/${bridge}/config.yaml"
        echo "✓ Generated bridges/${bridge}/config.yaml"
    fi
done

# Generate doublepuppet registration (still using template as it's not bridge-specific)
if [ -f "bridges/doublepuppet.yaml.template" ]; then
    substitute_vars "bridges/doublepuppet.yaml.template" "bridges/doublepuppet.yaml"
    echo "✓ Generated bridges/doublepuppet.yaml"
fi

# Generate postgres init script
if [ -f "postgres-init/init-databases.sh.template" ]; then
    substitute_vars "postgres-init/init-databases.sh.template" "postgres-init/init-databases.sh"
    chmod +x "postgres-init/init-databases.sh"
    echo "✓ Generated postgres-init/init-databases.sh"
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
        messenger|instagram)
            echo "dock.mau.dev/mautrix/meta:latest"
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
        messenger|instagram)
            echo "/usr/bin/mautrix-meta"
            ;;
        telegram)
            echo "/usr/bin/mautrix-telegram"
            ;;
        *)
            echo "Unknown bridge: $1" >&2
            return 1
            ;;
    esac
}

# Generate registration file for each bridge
for bridge in whatsapp signal messenger instagram telegram; do
    config_file="bridges/${bridge}/config.yaml"
    reg_file="bridges/${bridge}/${bridge}-registration.yaml"

    if [ -f "$config_file" ]; then
        bridge_image=$(get_bridge_image "$bridge")
        bridge_executable=$(get_bridge_executable "$bridge")
        echo "Generating ${bridge}-registration.yaml..."

        # Run bridge container with -g flag to generate registration
        docker run --rm \
            --entrypoint="" \
            -v "$(realpath bridges/${bridge}):/data:rw" \
            "$bridge_image" \
            "$bridge_executable" -g -c /data/config.yaml -r /data/${bridge}-registration.yaml

        if [ $? -eq 0 ]; then
            # Fix permissions so synapse and validation can read the files
            chmod 644 "bridges/${bridge}/${bridge}-registration.yaml" 2>/dev/null || true
            chmod 644 "bridges/${bridge}/config.yaml" 2>/dev/null || true
            echo "✓ Generated bridges/${bridge}/${bridge}-registration.yaml"
        else
            echo "✗ Failed to generate ${bridge}-registration.yaml"
            exit 1
        fi
    fi
done

echo ""
echo "Validating generated configuration files..."

# Check for placeholder values in config files
validation_errors=()
for bridge in whatsapp signal messenger instagram telegram; do
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
        echo "  ✗ $error"
    done
    echo ""
    echo "Please check your template files and .env configuration."
    exit 1
fi

echo "✓ All configuration files are valid"

echo ""
echo "All configuration files generated successfully!"
echo "You can now run: just up"
