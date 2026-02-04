# VPS to AWS Migration: SQLite → PostgreSQL + User Migration

## Overview

Three-phase migration:
1. **Schema Migration**: Convert main app from SQLite to PostgreSQL (on parent EC2)
2. **VSOCK Access**: Enable enclaves to access parent PostgreSQL via VSOCK
3. **User Migration**: Restore encrypted backups for users when they open browser

## Architecture

- **BEFORE**: SQLite on parent EC2, enclaves can't access it yet
- **AFTER**: PostgreSQL on parent EC2 (same instance as Matrix DB), enclaves access via VSOCK
- **Backups**: Stored on parent filesystem at `/data/backups/{user_id}/`

## Flow Diagram (Phase 3 - User Migration)

```
User Browser              NEW Enclave                Parent (PostgreSQL)
     |                         |                          |
     |---(1) Login/Load------->|                          |
     |                         |                          |
     |                         |---(2) Check PostgreSQL---|
     |                         |      User migrated?      |
     |                         |      (via VSOCK)         |
     |                         |                          |
     |<--(3) needs_migration---|                          |
     |                         |                          |
     |---(4) Send session key->|                          |
     |      POST /api/migrate  |                          |
     |                         |                          |
     |                         |---(5) Read backup file---|
     |                         |      from parent FS      |
     |                         |      via VSOCK           |
     |                         |                          |
     |                         |---(6) Decrypt ALL--------|
     |                         |      data with           |
     |                         |      session key         |
     |                         |                          |
     |                         |---(7) Insert into--------|
     |                         |      PostgreSQL +        |
     |                         |      Matrix store        |
     |                         |                          |
     |                         |---(8) Update PostgreSQL->|
     |                         |      active_enclave      |
     |                         |      = "new"             |
     |                         |      (via VSOCK)         |
     |                         |                          |
     |<--(9) Migration done----|                          |
```

## Components

### 1. NEW Enclave - Migration Detection & Execution

**File: `backend/src/handlers/migration_handlers.rs`** (new)

Endpoints:
- `GET /api/migration/status` - Check if user needs migration (check PostgreSQL)
- `POST /api/migration/start` - Start migration with session key
- `GET /api/migration/progress/{id}` - Poll migration progress

**File: `backend/src/services/migration_service.rs`** (new)

Core logic:
```rust
async fn migrate_user(
    user_id: i32,
    email: &str,
    session_key: &[u8; 32],
) -> Result<(), MigrationError> {
    // 1. Read encrypted backup from parent FS via VSOCK
    // 2. Decrypt ALL data with session key
    // 3. Insert into PostgreSQL (all backed up tables)
    // 4. Restore Matrix store to enclave storage
    // 5. Update active_enclave = "new" in PostgreSQL via VSOCK
}
```

**File: `backend/src/services/backup_restore.rs`** (new)

Restore logic for parsing backup format and inserting into PostgreSQL.

### 2. Parent - VSOCK PostgreSQL Proxy (Phase 2)

Build a PostgreSQL wire protocol proxy that:
- Listens on VSOCK port for enclave connections
- Forwards queries to local PostgreSQL
- Returns results back over VSOCK

This enables standard Diesel/sqlx in the enclave without modification.

### 3. Frontend - Migration UI

**File: `frontend/src/components/migration_prompt.rs`** (new)

- Detect `needs_migration` from login response or `/api/migration/status`
- Show migration UI with progress
- Send session key to start migration
- Poll progress, redirect to app when complete

## Data to Migrate

Restore ALL data from the backup - both encrypted and unencrypted fields.

### From Backup (created by 5-minute job)
The backup contains:
- **Encrypted data** (with session key): Bridge data, Matrix store, sensitive fields
- **Unencrypted data**: User settings, preferences, task schedules

### Tables in Backup
All user-scoped data currently backed up by the 5-minute job:
- `bridges` - Matrix bridge connections (WhatsApp, Telegram, Signal)
- Bridge-related tables with JIDs, messages, keys
- Matrix store SQLite
- Any other tables included in current backup job

### Matrix Store
- Path: `{MATRIX_STORE_PATH}/appuser_{uuid}/`
- Contains: E2EE keys, room state, session data
- Decrypted and restored to NEW enclave's storage

## Encryption Handling

### Session Key Encrypted Fields (new system)
Already encrypted with user's session key during 5-minute backup job:
- Bridge data (JIDs, messages)
- Matrix store

### Legacy ENCRYPTION_KEY Fields
Fields encrypted with server-side key (need re-encryption):
- `users.encrypted_matrix_access_token`
- `users.encrypted_matrix_password`
- OAuth tokens in integration tables

**Strategy**: During backup creation, re-encrypt legacy fields with session key

## Database Changes

### PostgreSQL (parent EC2, after Phase 1)

All 31 existing tables migrated from SQLite, plus new migration tracking:

```sql
CREATE TABLE user_migrations (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL,
    email VARCHAR(255) NOT NULL,
    status VARCHAR(50) NOT NULL DEFAULT 'pending',
    started_at TIMESTAMP,
    completed_at TIMESTAMP,
    error_message TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

The `users` table already has `active_enclave` column for routing.

## Error Handling

| Scenario | Action |
|----------|--------|
| Backup fetch fails | Retry 3x, then show error, user stays on old |
| Decryption fails | Ask user to re-enter password (refresh session key) |
| PostgreSQL insert fails | Rollback transaction, return error |
| Parent update fails | Mark "completed_pending_parent", background retry |

## Implementation Phases

### Phase 1: SQLite → PostgreSQL Migration

**Strategy**: Fresh schema dump (not converting 129 migrations)

1. Generate PostgreSQL schema from current SQLite structure
2. Create fresh PostgreSQL migration with complete schema
3. Write data migration script (SQLite → PostgreSQL)
4. Update Diesel config and connection code
5. Test all existing functionality

**Steps:**
- Export SQLite schema: `sqlite3 database.db .schema > schema.sql`
- Convert to PostgreSQL syntax (AUTOINCREMENT → SERIAL, etc.)
- Create single `init` migration with full schema
- Migrate data using INSERT statements or pg_loader
- Update `diesel.toml` for PostgreSQL
- Update DATABASE_URL and connection code

### Phase 2: VSOCK PostgreSQL Access

Build mechanism for enclaves to access parent PostgreSQL:

**Option A: PostgreSQL wire protocol over VSOCK**
- Run pg_proxy on parent that bridges VSOCK → PostgreSQL
- Enclave connects to VSOCK, proxy forwards to PG

**Option B: Custom RPC layer**
- Build query RPC service on parent
- Enclave sends queries via VSOCK, parent executes and returns results

Recommend Option A - standard PG protocol means normal Diesel/sqlx works in enclave.

### Phase 3: User Migration Flow

1. **Backend**: Add migration status endpoint (`/api/migration/status`)
2. **Backend**: Add migration start endpoint (`/api/migration/start`)
3. **Backend**: Create migration service (read backup, decrypt, restore)
4. **Backend**: Create backup restore service (parse backup, insert to PostgreSQL)
5. **Backend**: Add PostgreSQL migration tracking table
6. **Frontend**: Add migration detection in app initialization
7. **Frontend**: Add migration prompt component with progress UI
8. **Integration**: Test full flow

## Verification

1. Create test user on OLD enclave with data and active backup
2. Deploy NEW enclave
3. Open browser, verify migration prompt appears
4. Enter password (to derive session key), start migration
5. Verify data restored to PostgreSQL
6. Verify `active_enclave = "new"` in PostgreSQL
7. Send SMS, verify routed to NEW enclave
8. Verify Matrix/bridge messages still work

## Critical Files

### Phase 1: SQLite → PostgreSQL
- `backend/diesel.toml` - Change database backend to PostgreSQL
- `backend/migrations/0000000000000_init/` (new) - Fresh PostgreSQL schema
- `backend/src/main.rs` - Update DB connection string
- `scripts/migrate_sqlite_to_pg.rs` (new) - Data migration script

### Phase 2: VSOCK PostgreSQL Access
- `parent-proxy/src/pg_proxy.rs` (new) - PostgreSQL wire protocol proxy over VSOCK
- Backend connection code - Connect to VSOCK instead of direct PG

### Phase 3: User Migration
- `backend/src/handlers/migration_handlers.rs` (new) - Migration endpoints
- `backend/src/services/migration_service.rs` (new) - Migration orchestration
- `backend/src/services/backup_restore.rs` (new) - Backup parsing and restore
- `frontend/src/components/migration_prompt.rs` (new) - Migration UI

### Reference (existing)
- `backend/src/services/bridge_encryption.rs` - Decryption patterns
- `backend/src/services/session_keys.rs` - Session key handling
- `frontend/src/utils/backup_crypto.rs` - Key derivation

## Architecture Details (Final State)

- **Database**: PostgreSQL on parent EC2 (all 31 app tables + Matrix homeserver DB)
- **Enclave-DB access**: VSOCK → PG wire protocol proxy → PostgreSQL
- **Backup storage**: Parent filesystem at `/data/backups/{user_id}/`
- **Parent proxy**: Uses same PostgreSQL for routing decisions
- **Matrix store**: Inside enclave, restored from encrypted backup during user migration
