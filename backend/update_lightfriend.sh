#!/bin/bash

# Script to update Lightfriend: pull code, build, migrate, and restart services in safe order.
# Run as: ./update_lightfriend.sh [proxy_sleep] [homeserver_sleep] [bridges_sleep]
# Defaults: 10s after tesla proxy, 30s after homeserver, 10s after bridges.

# Get sudo password upfront and keep it alive
echo "This script requires sudo privileges. Please enter your password now."
sudo -v || { echo "Failed to get sudo privileges"; exit 1; }

# Keep sudo alive in background (refresh every 50 seconds)
while true; do sudo -n true; sleep 50; done 2>/dev/null &
SUDO_KEEPALIVE_PID=$!
trap "kill $SUDO_KEEPALIVE_PID 2>/dev/null" EXIT

# Configurable paths and sleeps
LIGHTFRIEND_DIR="$HOME/lightfriend-cloud"
DB_DIR="/var/lib/lightfriend"
STATE_DIR="$HOME/.lightfriend-update"
PROXY_SLEEP=${1:-10}
HOMESERVER_SLEEP=${2:-30}
BRIDGES_SLEEP=${3:-10}

# Create state directory if needed
mkdir -p "$STATE_DIR"

# Step 0: Save current state for rollback
cd "$LIGHTFRIEND_DIR" || { echo "Error: Can't cd to $LIGHTFRIEND_DIR"; exit 1; }

CURRENT_COMMIT=$(git rev-parse HEAD)
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_DB="${STATE_DIR}/database_${TIMESTAMP}.db"

echo "Saving rollback state..."
echo "$CURRENT_COMMIT" > "${STATE_DIR}/pre_update_commit"
echo "$TIMESTAMP" > "${STATE_DIR}/last_update_timestamp"

# Backup database with timestamp
cp "${DB_DIR}/database.db" "$BACKUP_DB" || { echo "Database backup failed"; exit 1; }
echo "$BACKUP_DB" > "${STATE_DIR}/last_backup_db"

# Also keep the simple backup for quick access
cp "${DB_DIR}/database.db" "$HOME/backup-lightfriend.db" || { echo "Database backup failed"; exit 1; }

echo "Rollback state saved:"
echo "  - Previous commit: $CURRENT_COMMIT"
echo "  - Database backup: $BACKUP_DB"

# Step 1: Pull and build
git pull || { echo "Git pull failed"; exit 1; }

NEW_COMMIT=$(git rev-parse HEAD)
echo "$NEW_COMMIT" > "${STATE_DIR}/post_update_commit"
echo "Updated from $CURRENT_COMMIT to $NEW_COMMIT"

cd backend || { echo "Error: Can't cd to backend"; exit 1; }
diesel migration run || { echo "Diesel migration failed"; exit 1; }

# Added: Check if target dir > 10GB and clean only if needed
if [ -d target ]; then
    target_size=$(du -sb target | cut -f1)
    if [ "$target_size" -gt 10737418240 ]; then
        echo "target dir is over 10GB ($target_size bytes); running cargo clean..."
        cargo clean || { echo "Cargo clean failed"; exit 1; }
    else
        echo "target dir is under 10GB ($target_size bytes); skipping cargo clean."
    fi
else
    echo "No target dir found; skipping clean."
fi
cargo build --release || { echo "Cargo build failed"; exit 1; }

cd ../frontend || { echo "Error: Can't cd to frontend"; exit 1; }
trunk build --release || { echo "Trunk build failed"; exit 1; }

cd ..  # Back to Lightfriend root

# Step 2: Restart services in order with waits
echo "Restarting tesla-proxy.service..."
sudo systemctl restart tesla-proxy.service || { echo "Restart tesla-proxy failed"; exit 1; }
echo "Waiting $PROXY_SLEEP seconds for Tesla proxy to stabilize..."
sleep "$PROXY_SLEEP"

# Verify Tesla proxy is healthy
echo "Checking Tesla proxy health..."
proxy_status=$(sudo docker ps --filter name=tesla-http-proxy --format "{{.Status}}")
if echo "$proxy_status" | grep -q "healthy"; then
    echo "Tesla proxy is healthy: $proxy_status"
elif echo "$proxy_status" | grep -q "Up"; then
    echo "Warning: Tesla proxy is up but healthcheck not yet complete: $proxy_status"
else
    echo "Error: Tesla proxy is not running properly: $proxy_status"
    exit 1
fi

echo "Restarting matrix-homeserver.service..."
sudo systemctl restart matrix-homeserver.service || { echo "Restart homeserver failed"; exit 1; }
echo "Waiting $HOMESERVER_SLEEP seconds for homeserver to stabilize..."
sleep "$HOMESERVER_SLEEP"

echo "Restarting bridges..."
sudo systemctl restart mautrix-whatsapp.service mautrix-telegram.service mautrix-signal.service || { echo "Restart bridges failed"; exit 1; }
echo "Waiting $BRIDGES_SLEEP seconds for bridges to connect..."
sleep "$BRIDGES_SLEEP"

echo "Restarting lightfriend.service..."
sudo systemctl restart lightfriend.service || { echo "Restart lightfriend failed"; exit 1; }

# Mark update as successful
echo "success" > "${STATE_DIR}/last_update_status"
date >> "${STATE_DIR}/update_history.log"
echo "  $CURRENT_COMMIT -> $NEW_COMMIT" >> "${STATE_DIR}/update_history.log"

echo ""
echo "Update successfully completed."
echo "To rollback, run: ./rollback_lightfriend.sh"

# Cleanup old backups (keep last 5)
cd "$STATE_DIR"
ls -t database_*.db 2>/dev/null | tail -n +6 | xargs -r rm
