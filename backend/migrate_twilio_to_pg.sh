#!/bin/bash
# One-time migration: copy twilio credentials from SQLite user_settings to PG user_secrets.
# Safe to run multiple times - only updates rows where PG has NULL twilio creds.
#
# Usage: ./migrate_twilio_to_pg.sh [sqlite_db_path]
# Requires: sqlite3, psql, PG_DATABASE_URL env var

set -euo pipefail

SQLITE_DB="${1:-./database.db}"

if [ ! -f "$SQLITE_DB" ]; then
    echo "ERROR: SQLite database not found at $SQLITE_DB"
    exit 1
fi

if [ -z "${PG_DATABASE_URL:-}" ]; then
    echo "ERROR: PG_DATABASE_URL env var not set"
    exit 1
fi

echo "Reading twilio credentials from SQLite: $SQLITE_DB"

# Extract rows that have twilio credentials set
rows=$(sqlite3 "$SQLITE_DB" "SELECT user_id, encrypted_twilio_account_sid, encrypted_twilio_auth_token FROM user_settings WHERE encrypted_twilio_account_sid IS NOT NULL AND encrypted_twilio_auth_token IS NOT NULL;" 2>/dev/null)

if [ -z "$rows" ]; then
    echo "No twilio credentials found in SQLite. Nothing to migrate."
    exit 0
fi

count=0
skipped=0

while IFS='|' read -r user_id enc_sid enc_token; do
    # Update PG only if twilio creds are currently NULL (don't overwrite)
    result=$(psql "$PG_DATABASE_URL" -t -A -c "
        UPDATE user_secrets
        SET encrypted_twilio_account_sid = '$enc_sid',
            encrypted_twilio_auth_token = '$enc_token'
        WHERE user_id = $user_id
          AND encrypted_twilio_account_sid IS NULL;
    " 2>/dev/null)

    affected=$(echo "$result" | grep -o '[0-9]*' || echo "0")
    if [ "$affected" = "1" ]; then
        count=$((count + 1))
        echo "  Migrated twilio creds for user $user_id"
    else
        # Check if user_secrets row exists at all
        exists=$(psql "$PG_DATABASE_URL" -t -A -c "SELECT count(*) FROM user_secrets WHERE user_id = $user_id;" 2>/dev/null)
        if [ "$exists" = "0" ]; then
            # Insert new row
            psql "$PG_DATABASE_URL" -t -A -c "
                INSERT INTO user_secrets (user_id, encrypted_twilio_account_sid, encrypted_twilio_auth_token)
                VALUES ($user_id, '$enc_sid', '$enc_token');
            " >/dev/null 2>&1
            count=$((count + 1))
            echo "  Inserted twilio creds for user $user_id (new row)"
        else
            skipped=$((skipped + 1))
            echo "  Skipped user $user_id (already has twilio creds in PG)"
        fi
    fi
done <<< "$rows"

echo ""
echo "Done. Migrated: $count, Skipped: $skipped"
