# CLAUDE.md

Lightfriend is a full-stack AI assistant SaaS with Rust on both backend (Axum web framework) and frontend (Yew WebAssembly framework). Integrates with Matrix homeserver for multi-platform messaging, Twilio for SMS/voice, Tinfoil for verifiable AI inference, Stripe for payments, and OAuth services.

## Development Commands

**One-command backend boot (macOS):**
```bash
just dev-backend                  # = cd backend && cargo run - port 3000
```

**Backend (Axum + Diesel):**
```bash
cd backend && cargo run --bin backend   # Run server (port 3000) - rpaths baked in via .cargo/config.toml
cd backend && cargo test                # Run tests
cd backend && diesel migration run
```

**Frontend (Yew + Trunk):**
```bash
cd frontend && trunk serve        # Dev server (port 8080)
```

**Docker (Enclave):**
```bash
just build-local                  # Build enclave image for current platform (validates prod-style build)
just up                          # Start enclave
just logs                        # View logs
```

## macOS Prerequisites

`just dev-backend` auto-runs `just setup-mac` which installs the Homebrew
keg-only libs the backend links against (sqlite, openssl@3, libiconv,
postgresql@14). `brew install` is idempotent so this is fast after the
first run. Non-macOS hosts no-op out of setup.

If a new crate ever pulls in a new dylib and boot fails with
`Library not loaded: @rpath/libX.dylib`:

1. Find the missing lib: `brew --prefix <formula>`
2. Add a `link-arg=-Wl,-rpath,<prefix>/lib` entry to
   `backend/.cargo/config.toml`
3. Add the formula to the `brew install` list in the `setup-mac` recipe
4. `touch backend/src/main.rs && cargo build` to relink

Homebrew keg-only means the lib is NOT symlinked into `/opt/homebrew/lib`,
so the default rpath doesn't find it. `DYLD_LIBRARY_PATH` is stripped by
macOS SIP at runtime and does not work as a workaround; rpaths must be
baked at link time.

To validate a prod-style build actually works, run `just build-local`.
That runs the same Docker/Linux build the deploy pipeline uses; macOS
dyld issues are irrelevant inside that container.

## Architecture

**Backend Structure:**
- Entry: `backend/src/main.rs` - Routing, AppState, middleware
- Handlers: `backend/src/handlers/` - HTTP request handlers (30+ modules)
- Services: `backend/src/services/` - Business logic layer (SignupService, CountryService)
- Repositories: `backend/src/repositories/` - Data access layer (UserCore, UserRepository, UserSubscriptions, ConnectionAuth)
- Models: `backend/src/models/user_models.rs` - Diesel ORM models
- Schema: `backend/src/schema.rs` - Auto-generated from migrations
- API: `backend/src/api/` - External service integrations
- Tool Calls: `backend/src/tool_call_utils/` - AI tool implementations
- Jobs: `backend/src/jobs/scheduler.rs` - Background cron jobs

**Frontend Structure:**
- Entry: `frontend/src/main.rs` - Yew app root, routing
- Pages: `frontend/src/pages/` - Page components
- Auth: `frontend/src/auth/` - Authentication UI
- Connections: `frontend/src/connections/` - Integration UIs
- Config: `frontend/src/config.rs` - Backend URL configuration

**Database:** SQLite + Diesel 2.1 with 129 migrations in `backend/migrations/`

## Testing - IMPORTANT

**ALL tests go in `backend/tests/` - NEVER use inline `#[cfg(test)] mod tests` blocks inside source files.** Integration tests live in `backend/tests/<module>_test.rs`. If a function is private and needs testing, either test it through the public API or make it `pub` so the test file can access it.

## Key Patterns

**Repository Pattern:** Always use repositories for data access, never raw Diesel queries in handlers.

**Authentication:** JWT tokens (access + refresh) with middleware:
- `require_auth` - JWT validation
- `require_admin` - Admin check
- `check_subscription_access` - Tier validation
- **NEVER use `Extension<i32>` for user_id in handlers.** Always use `AuthUser` extractor: `auth_user: AuthUser` then `let user_id = auth_user.user_id;`

**Security & Encryption:**
- AES-256-GCM encryption for all sensitive data
- Key from `ENCRYPTION_KEY` env var
- HMAC/signature validation for all webhooks

**Error Handling:**
- Return `Result<T, E>` types throughout
- Use `?` operator for error propagation
- Map errors to appropriate HTTP status codes

**Async:** All I/O operations are async (Tokio runtime). Use `async fn` and `.await` consistently.

## Important File Locations

**Backend:**
- Routing & AppState: `backend/src/main.rs:30-492`
- Auth middleware: `backend/src/handlers/auth_middleware.rs`
- User operations: `backend/src/repositories/user_core.rs`
- Matrix integration: `backend/src/utils/matrix_auth.rs`, `backend/src/utils/bridge.rs`
- Encryption: `backend/src/utils/encryption.rs`

**Frontend:**
- Routing & Nav: `frontend/src/main.rs:104-245`
- Main dashboard: `frontend/src/pages/home.rs`

## Git Commits

Do NOT add "Generated with Claude Code" or Co-Authored-By lines mentioning Claude/AI. Keep commit messages clean and focused on what changed.

## Safety Guards

Hooks protect against accidental destructive changes:

1. **Migration Guard** - Blocks destructive SQL (DROP, TRUNCATE, DELETE, RENAME, ALTER TYPE)
2. **Protected Files Guard** - Blocks edits to `encryption.rs`, `auth_middleware.rs`, `stripe_webhooks.rs`

**When a hook blocks your edit with `OVERRIDE: touch <path>`:**
1. Ask the user: "This change requires approval: [describe what you're changing]. Should I proceed?"
2. If user approves, run `touch <path>` to create the one-time override flag, then retry the edit
3. If user declines, abandon the change
4. The flag file is auto-deleted after one use

## Common Tasks

For step-by-step guides, use skills in `.claude/skills/`:
- `lightfriend-db-migration` - Database schema modifications using Diesel
- `lightfriend-add-integration` - Adding new OAuth integrations
- `lightfriend-add-frontend-page` - Adding new Yew frontend pages

## gstack

Use `/browse` from gstack for all web browsing. **Never use `mcp__claude-in-chrome__*` tools.**

Available skills: `/office-hours`, `/plan-ceo-review`, `/plan-eng-review`, `/plan-design-review`, `/design-consultation`, `/review`, `/ship`, `/browse`, `/qa`, `/qa-only`, `/design-review`, `/setup-browser-cookies`, `/retro`, `/investigate`, `/document-release`, `/codex`, `/careful`, `/freeze`, `/guard`, `/unfreeze`, `/gstack-upgrade`

## License

This project is licensed under GNU AGPLv3. The name "Lightfriend" and branding are owned by Rasmus Ähtävä and not included in the license.
