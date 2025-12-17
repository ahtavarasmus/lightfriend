#!/bin/bash

# Script to rollback Lightfriend to the previous update state
# Run as: ./rollback_lightfriend.sh [--force]

LIGHTFRIEND_DIR="$HOME/lightfriend-cloud"
DB_DIR="/var/lib/lightfriend"
STATE_DIR="$HOME/.lightfriend-update"
FORCE=${1:-""}

# Check if state exists
if [ ! -f "${STATE_DIR}/pre_update_commit" ]; then
    echo "Error: No rollback state found. Was update_lightfriend.sh run before?"
    exit 1
fi

PREVIOUS_COMMIT=$(cat "${STATE_DIR}/pre_update_commit")
BACKUP_DB=$(cat "${STATE_DIR}/last_backup_db" 2>/dev/null)
CURRENT_COMMIT=$(cd "$LIGHTFRIEND_DIR" && git rev-parse HEAD)

echo "=== Lightfriend Rollback ==="
echo "Current commit:  $CURRENT_COMMIT"
echo "Rollback to:     $PREVIOUS_COMMIT"
echo "Database backup: $BACKUP_DB"
echo ""

# Verify backup exists
if [ ! -f "$BACKUP_DB" ]; then
    echo "Error: Database backup not found at $BACKUP_DB"
    exit 1
fi

# Confirm unless --force
if [ "$FORCE" != "--force" ]; then
    read -p "Are you sure you want to rollback? This will restore the old database. (y/N) " confirm
    if [ "$confirm" != "y" ] && [ "$confirm" != "Y" ]; then
        echo "Rollback cancelled."
        exit 0
    fi
fi

echo ""
echo "Starting rollback..."

# Step 1: Stop lightfriend service first (it's using the DB)
echo "Stopping lightfriend.service..."
sudo systemctl stop lightfriend.service || { echo "Stop lightfriend failed"; exit 1; }

# Step 2: Restore database
echo "Restoring database from backup..."
cp "$BACKUP_DB" "${DB_DIR}/database.db" || { echo "Database restore failed"; exit 1; }

# Step 3: Checkout previous commit
echo "Checking out previous commit: $PREVIOUS_COMMIT"
cd "$LIGHTFRIEND_DIR" || { echo "Error: Can't cd to $LIGHTFRIEND_DIR"; exit 1; }
git checkout "$PREVIOUS_COMMIT" || { echo "Git checkout failed"; exit 1; }

# Step 4: Rebuild backend
echo "Rebuilding backend..."
cd backend || { echo "Error: Can't cd to backend"; exit 1; }
cargo build --release || { echo "Cargo build failed"; exit 1; }

# Step 5: Rebuild frontend
cd ../frontend || { echo "Error: Can't cd to frontend"; exit 1; }
echo "Rebuilding frontend..."
trunk build --release || { echo "Trunk build failed"; exit 1; }

cd ..

# Step 6: Restart all services
echo "Restarting services..."
sudo systemctl restart tesla-proxy.service || echo "Warning: tesla-proxy restart failed"
sleep 10

sudo systemctl restart matrix-homeserver.service || echo "Warning: homeserver restart failed"
sleep 30

sudo systemctl restart mautrix-whatsapp.service mautrix-telegram.service mautrix-signal.service || echo "Warning: bridges restart failed"
sleep 10

sudo systemctl restart lightfriend.service || { echo "Restart lightfriend failed"; exit 1; }

# Update state
echo "rollback" > "${STATE_DIR}/last_update_status"
date >> "${STATE_DIR}/update_history.log"
echo "  ROLLBACK to $PREVIOUS_COMMIT" >> "${STATE_DIR}/update_history.log"

echo ""
echo "=== Rollback completed ==="
echo "Now running commit: $PREVIOUS_COMMIT"
echo ""
echo "Note: You are now in 'detached HEAD' state."
echo "To return to master branch later: git checkout master"
