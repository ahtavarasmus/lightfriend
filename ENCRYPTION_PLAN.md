# Encryption System Overhaul Plan

Replace the old per-user migration system with Lit Protocol + Nitro Enclave key custody.
Consolidate all data into PostgreSQL inside the enclave. Eliminate SQLite entirely.

Old approach: per-user encryption keys, browser store, migration tracking, dual databases.
New approach: single PG database inside enclave, single key (custody TBD), encrypt/decrypt whole DB dump during updates.

### Key custody decision: Lit Protocol only

Lit Protocol is the sole key custodian. If Lit fails permanently, data inside the enclave is lost. This is acceptable because:
- **User accounts + billing** are recoverable from Stripe
- **Customer emails** are stored in an external email service
- **Lost data** (items, message history, service connections, settings) is inconvenient but not catastrophic
- Users would need to re-link integrations - a "re-onboarding" event, not permanent data loss
- This gives the strongest privacy guarantee: operator never sees the plaintext key

---

## Part A: Cleanup - Remove Old Migration System

Each step is a standalone commit. Run `cargo build && cargo test` after each to verify.

### Step 1: Remove migration proxy module

- Delete `backend/src/utils/migration_proxy.rs`
- Remove `pub mod migration_proxy;` from `backend/src/lib.rs` (line 42 in utils block)

**Verify:** `cargo build` will fail showing which files reference the module.

### Step 2: Remove migration proxy usage from webhook handlers

- `backend/src/api/elevenlabs_webhook.rs`: Remove the `if !user.migrated_to_new_server` proxy block (~lines 244-270)
- `backend/src/api/twilio_utils.rs`: Remove the `if !user.migrated_to_new_server` proxy block (~lines 285-306)

**Verify:** `cargo build` - may still fail on `is_valid_internal_request` refs.

### Step 3: Remove internal routing module

- Delete `backend/src/api/internal_routing.rs`
- Remove `pub mod internal_routing;` from `backend/src/lib.rs` (line 67 in api block)
- Delete `backend/tests/internal_routing_test.rs`

**Verify:** `cargo build` will fail showing remaining `is_valid_internal_request` references.

### Step 4: Remove internal routing usage from webhook handlers

- `backend/src/api/elevenlabs_webhook.rs`: Remove import + `is_valid_internal_request` check block
- `backend/src/api/twilio_utils.rs`: Remove import + 2 `is_valid_internal_request` check blocks (in `validate_twilio_signature` and `validate_twilio_status_callback_signature`)

**Verify:** `cargo build` succeeds.

### Step 5: Drop migration tracking columns from users table

Create Diesel migration to remove `migrated_to_new_server`, `last_backup_at`, `backup_session_active` from users table. Run `diesel migration run`.

**Verify:** `cargo build` fails showing field references to fix.

### Step 6: Remove migration fields from code

- `backend/src/models/user_models.rs`: Remove 3 fields from User struct
- `backend/src/repositories/mock_signup_repository.rs`: Remove 3 field initializations
- `backend/src/test_utils.rs`: Remove migration field references
- Fix any other compiler errors

**Verify:** `cargo build && cargo test` passes.

### Step 7: Clean up env vars, comments, worktree

- `.env.example`: Remove Server Migration section
- Remove migration-related comments in repository files
- Remove `.claude/worktrees/feature/` (old worktree)

**Verify:** Grep confirms no references to old migration system remain.

---

## Part B: Migrate Remaining SQLite Tables to PostgreSQL

### What's left in SQLite (10 tables)

| Table | Used by | Effort |
|-------|---------|--------|
| users | UserCore, UserRepository (100+ queries) | High |
| user_settings | UserCore (15+ queries) | Medium |
| refund_info | UserRepository (5 queries) | Low |
| country_availability | CountryService (3 queries) | Low |
| message_status_log | TwilioStatusRepository (10 queries) | Low |
| admin_alerts | AdminAlertRepository (5 queries) | Low |
| disabled_alert_types | AdminAlertRepository (3 queries) | Low |
| site_metrics | MetricsRepository (2 queries) | Low |
| waitlist | SignupRepository (2 queries) | Low |

### What's already in PG (18 tables)

user_info, user_secrets, message_history, contact_profiles, contact_profile_exceptions, imap_connection, tesla, youtube, mcp_servers, totp_secrets, totp_backup_codes, webauthn_credentials, webauthn_challenges, items, usage_logs, processed_emails, bridges, bridge_disconnection_events

### Step 8: Create PG migrations for remaining tables

Create PostgreSQL migrations (using `diesel_pg.toml`) for: users, user_settings, refund_info, country_availability, message_status_log, admin_alerts, disabled_alert_types, site_metrics, waitlist.

**Verify:** `diesel migration run --config-file=diesel_pg.toml` succeeds.

### Step 9: Add PG models for migrated tables

Add structs in `backend/src/pg_models.rs` for the new PG tables.

**Verify:** `cargo build` succeeds.

### Step 10: Migrate UserCore to PG

Move all UserCore queries from `self.db_pool` (SQLite) to `self.pg_pool` (PG). This is the biggest change (~65 operations).

**Verify:** `cargo build && cargo test` passes.

### Step 11: Migrate UserRepository remaining SQLite queries to PG

Move remaining SQLite queries in UserRepository to PG (~50 operations that still use db_pool).

**Verify:** `cargo build && cargo test` passes.

### Step 12: Migrate small repositories to PG

- TwilioStatusRepository (message_status_log)
- AdminAlertRepository (admin_alerts, disabled_alert_types)
- SignupRepository (waitlist)
- CountryService (country_availability)
- MetricsRepository (site_metrics) - may already be PG

**Verify:** `cargo build && cargo test` passes.

### Step 13: Remove SQLite from AppState

- Remove `db_pool: DbPool` from AppState
- Remove SQLiteConnectionCustomizer
- Remove `SqliteDbPool` type alias
- Remove diesel SQLite dependency if possible
- Update `context.rs` to only initialize PG pool
- Remove `DATABASE_URL` env var usage

**Verify:** `cargo build && cargo test` passes. Grep for `db_pool` returns nothing.

### Step 14: Remove field-level encryption

Since all data lives inside the enclave, field-level AES-256-GCM encryption is no longer needed. The enclave provides isolation. Only the whole-DB dump needs encryption during updates.

- Remove `encrypt()`/`decrypt()` calls from repository methods
- Store values in plaintext in PG (they're protected by enclave isolation)
- Keep `backend/src/utils/encryption.rs` but repurpose it for whole-DB dump encryption only

**Verify:** `cargo build && cargo test` passes.

### Step 15: Write migration binary to move data

Update `backend/src/bin/migrate_to_pg.rs` to migrate the remaining SQLite tables to PG (users, user_settings, etc.).

**Verify:** Run migration binary against test data.

---

## Part C: Scaffold Lit Protocol Key Custody

### Step 16: Add Lit Action files

Create `nitro-lit-action/` with:
- `src/lit-action.js` - attestation verification + encrypt/decrypt
- `build.js` - esbuild bundler
- `package.json`

**Verify:** `cd nitro-lit-action && npm install && npm run build` produces bundle.

### Step 17: Add enclave client + host script

- `nitro-lit-action/enclave-client/fetch-key.js` - runs inside enclave, gets key from Lit
- `nitro-lit-action/host/manage-ciphertext.js` - runs on host, manages ciphertext

**Verify:** Files exist with clear TODO markers for vsock placeholders.

### Step 18: Add GitHub Actions workflow

Create `.github/workflows/build-enclave.yml` for building enclave image + publishing PCR values.

**Verify:** File exists with correct structure.

---

## Part D: Integration (future PRs)

- D1: Implement vsock communication (Rust + JS)
- D2: Create Dockerfile.enclave
- D3: Implement whole-DB dump encrypt/decrypt for update process
- D4: Modify backend startup to receive key from Lit sidecar
- D5: Set up Ethereum wallet, pin Lit Action to IPFS
- D6: First enclave deployment
- D7: Update `update_lightfriend.sh` for enclave-based update flow
