#!/bin/bash

# Show current Lightfriend update/rollback status
# Run as: ./lightfriend_status.sh

LIGHTFRIEND_DIR="$HOME/lightfriend-cloud"
STATE_DIR="$HOME/.lightfriend-update"

echo "=== Lightfriend Status ==="
echo ""

# Current commit
cd "$LIGHTFRIEND_DIR" 2>/dev/null
if [ $? -eq 0 ]; then
    echo "Current commit: $(git rev-parse --short HEAD) ($(git log -1 --format='%s' | head -c 50))"
    echo "Branch: $(git branch --show-current 2>/dev/null || echo 'detached HEAD')"
else
    echo "Error: Can't access $LIGHTFRIEND_DIR"
fi

echo ""

# State info
if [ -d "$STATE_DIR" ]; then
    echo "Last update state:"
    [ -f "${STATE_DIR}/pre_update_commit" ] && echo "  Pre-update commit:  $(cat ${STATE_DIR}/pre_update_commit | head -c 7)"
    [ -f "${STATE_DIR}/post_update_commit" ] && echo "  Post-update commit: $(cat ${STATE_DIR}/post_update_commit | head -c 7)"
    [ -f "${STATE_DIR}/last_update_timestamp" ] && echo "  Timestamp: $(cat ${STATE_DIR}/last_update_timestamp)"
    [ -f "${STATE_DIR}/last_update_status" ] && echo "  Status: $(cat ${STATE_DIR}/last_update_status)"

    echo ""
    echo "Available backups:"
    ls -lh "${STATE_DIR}"/database_*.db 2>/dev/null | awk '{print "  " $9 " (" $5 ")"}'
else
    echo "No update state found (first run?)"
fi

echo ""
echo "Service status:"
systemctl is-active lightfriend.service 2>/dev/null | xargs -I {} echo "  lightfriend: {}"
systemctl is-active matrix-homeserver.service 2>/dev/null | xargs -I {} echo "  homeserver:  {}"
systemctl is-active tesla-proxy.service 2>/dev/null | xargs -I {} echo "  tesla-proxy: {}"
