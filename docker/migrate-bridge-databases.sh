#!/bin/bash
# Migrate bridge databases from old (chaotic) naming to new Docker setup
#
# Usage:
#   1. Run: ./migrate-bridge-databases.sh --dry-run   (to test)
#   2. Run: ./migrate-bridge-databases.sh             (to migrate)

set -e

BACKUP_DIR="./migration_backups"
OLD_PG_HOST="${OLD_PG_HOST:-127.0.0.1}"
OLD_PG_PORT="${OLD_PG_PORT:-5432}"
DRY_RUN=false

#############################################
# YOUR BRIDGE CREDENTIALS
#############################################

# WhatsApp - old: mw_whatsapp/mw_whatsapp -> new: whatsapp_user/whatsapp_db
OLD_WHATSAPP_USER="mw_whatsapp"
OLD_WHATSAPP_DB="mw_whatsapp"
OLD_WHATSAPP_PASS="this-is-a-password-for-the-mautrix-whatsapp-bridge"

# Signal - old: mw_signal/mv_signal -> new: signal_user/signal_db
OLD_SIGNAL_USER="mw_signal"
OLD_SIGNAL_DB="mv_signal"
OLD_SIGNAL_PASS="password-for-the-mautrix-signal-bridge"

# Telegram - old: mv_telegram/mv_telegram -> new: telegram_user/telegram_db
OLD_TELEGRAM_USER="mv_telegram"
OLD_TELEGRAM_DB="mv_telegram"
OLD_TELEGRAM_PASS="this-is-a-password-for-the-mautrix-telegram-bridge"

# Instagram - fill in if you have it
OLD_INSTAGRAM_USER=""
OLD_INSTAGRAM_DB=""
OLD_INSTAGRAM_PASS=""

# Messenger - fill in if you have it
OLD_MESSENGER_USER=""
OLD_MESSENGER_DB=""
OLD_MESSENGER_PASS=""

#############################################
# END OF CONFIGURATION
#############################################

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_info() { echo -e "${GREEN}[INFO]${NC} $1"; }
log_warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
log_error() { echo -e "${RED}[ERROR]${NC} $1"; }

[[ "$1" == "--dry-run" ]] && DRY_RUN=true && log_warn "DRY RUN MODE"

mkdir -p "$BACKUP_DIR"

echo "============================================"
echo "Bridge Database Migration"
echo "============================================"
echo ""

# Function to migrate a single bridge
migrate_bridge() {
    local name="$1"
    local old_user="$2"
    local old_db="$3"
    local old_pass="$4"
    local new_user="${name}_user"
    local new_db="${name}_db"
    local backup_file="${BACKUP_DIR}/${name}_backup.sql"

    if [[ -z "$old_user" || -z "$old_db" || -z "$old_pass" ]]; then
        log_warn "${name}: credentials not configured - skipping"
        return
    fi

    log_info "${name}: ${old_user}/${old_db} -> ${new_user}/${new_db}"

    # Export
    if [[ "$DRY_RUN" == false ]]; then
        log_info "  Exporting ${old_db}..."
        PGPASSWORD="$old_pass" pg_dump \
            -h "$OLD_PG_HOST" -p "$OLD_PG_PORT" \
            -U "$old_user" -d "$old_db" \
            --no-owner --no-acl \
            > "$backup_file" 2>&1

        if [[ -s "$backup_file" ]]; then
            log_info "  Exported ($(du -h "$backup_file" | cut -f1))"
        else
            log_error "  Export failed or empty"
            return
        fi

        # Import
        log_info "  Importing into ${new_db}..."
        docker exec -i lightfriend-postgres \
            psql -U "$new_user" -d "$new_db" < "$backup_file" 2>&1

        if [[ $? -eq 0 ]]; then
            log_info "  Success!"
        else
            log_error "  Import failed"
        fi
    else
        log_info "  [DRY RUN] Would export and import"
    fi
    echo ""
}

# Ensure Docker PostgreSQL is running
if [[ "$DRY_RUN" == false ]]; then
    if ! docker ps | grep -q lightfriend-postgres; then
        log_info "Starting Docker PostgreSQL..."
        docker compose up -d postgres
        sleep 5
        for i in {1..30}; do
            docker exec lightfriend-postgres pg_isready -U postgres > /dev/null 2>&1 && break
            sleep 1
        done
    fi
    log_info "Docker PostgreSQL is ready"
    echo ""
fi

# Migrate each bridge
migrate_bridge "whatsapp" "$OLD_WHATSAPP_USER" "$OLD_WHATSAPP_DB" "$OLD_WHATSAPP_PASS"
migrate_bridge "signal" "$OLD_SIGNAL_USER" "$OLD_SIGNAL_DB" "$OLD_SIGNAL_PASS"
migrate_bridge "telegram" "$OLD_TELEGRAM_USER" "$OLD_TELEGRAM_DB" "$OLD_TELEGRAM_PASS"
migrate_bridge "instagram" "$OLD_INSTAGRAM_USER" "$OLD_INSTAGRAM_DB" "$OLD_INSTAGRAM_PASS"
migrate_bridge "messenger" "$OLD_MESSENGER_USER" "$OLD_MESSENGER_DB" "$OLD_MESSENGER_PASS"

echo "============================================"
echo "Done! Backups saved in: ${BACKUP_DIR}/"
echo "============================================"
echo ""
echo "Verify: docker exec -it lightfriend-postgres psql -U whatsapp_user -d whatsapp_db -c '\\dt'"
echo "Start:  just up"
