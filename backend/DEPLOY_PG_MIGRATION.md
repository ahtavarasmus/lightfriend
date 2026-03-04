# Deploy: SQLite -> PostgreSQL Migration

## Pre-deploy setup (one-time on VPS)

1. Create lightfriend database on existing PostgreSQL:
```bash
sudo -u postgres psql -c "CREATE USER lightfriend WITH PASSWORD '<password>';"
sudo -u postgres psql -c "CREATE DATABASE lightfriend_db OWNER lightfriend;"
```

2. Add to backend `.env`:
```
PG_DATABASE_URL=postgres://lightfriend:<password>@localhost:5432/lightfriend_db
```

## Phase 1: Deploy new binary

```bash
# 1. Backup SQLite
cp backend/database.db backend/database.db.bak

# 2. Build new binary
cd backend && cargo build --release

# 3. Stop, deploy, start
sudo systemctl stop lightfriend
# (copy new binary into place)
sudo systemctl start lightfriend
```

On startup, PG migrations run automatically - creates all PG tables (empty).
SQLite still has all data intact.

## Phase 2: Run data migration

```bash
cd backend && cargo run --bin migrate_to_pg
```

This reads every row from SQLite sensitive tables and writes to PG.
Idempotent (ON CONFLICT DO NOTHING) - safe to run multiple times.

Verify:
```bash
psql -U lightfriend -d lightfriend_db -c "SELECT count(*) FROM user_secrets"
psql -U lightfriend -d lightfriend_db -c "SELECT count(*) FROM message_history"
psql -U lightfriend -d lightfriend_db -c "SELECT count(*) FROM items"
```

## Phase 3: SQLite cleanup (separate follow-up)

After confirming everything works on PG:
- Write SQLite migration to drop moved tables/columns
- Removes matrix/twilio secret columns from users/user_settings
- Drops 14 fully-moved tables from SQLite
- Regenerate schema.rs

## Rollback

Stop service, revert to old binary, restart. SQLite still has everything.
