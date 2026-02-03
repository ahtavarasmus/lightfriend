# CLAUDE.md

Lightfriend is a full-stack AI assistant SaaS with Rust on both backend (Axum) and frontend (Yew/WebAssembly). Integrates with Matrix for messaging, Twilio for SMS/voice, ElevenLabs for voice AI, Stripe for payments, and various OAuth services.

## Development Commands

```bash
# Backend
cd backend && cargo run              # Server on port 3000
cd backend && cargo test             # Run tests
cd backend && diesel migration run   # Apply migrations

# Frontend
cd frontend && trunk serve           # Dev server on port 8080

# Docker (recommended)
just build-native                    # Build for current platform
just up                              # Start all services
just logs-core                       # View logs
```

## Directory Conventions

**Backend (`backend/src/`):**
- `handlers/` - HTTP endpoints, named by feature
- `services/` - Business logic, stateless
- `repositories/` - Data access layer, one per domain entity
- `api/` - External service clients (Anthropic, ElevenLabs, etc.)
- `utils/` - Shared utilities (encryption, validation, matrix)
- `tool_call_utils/` - AI tool implementations
- `jobs/` - Background/scheduled tasks
- `models/` - Diesel ORM models
- `schema.rs` - Auto-generated from migrations

**Frontend (`frontend/src/`):**
- `pages/` - Page components
- `auth/` - Authentication UI
- `connections/` - Integration UIs
- `components/` - Reusable components

**Migrations:** `backend/migrations/` - Diesel timestamps with `up.sql`/`down.sql`

## Naming Patterns

```
handlers:     <feature>_handlers.rs or <feature>.rs
repositories: <entity>_repository.rs
services:     <domain>_service.rs
models:       <entity>_models.rs (or in user_models.rs)
```

## Key Search Terms

Find important patterns by grepping:
- Auth middleware: `require_auth`, `require_admin`, `check_subscription_access`
- Encryption: `encrypt_`, `decrypt_`, `AES`, `ENCRYPTION_KEY`
- Webhook validation: `verify_signature`, `validate_hmac`
- AppState: search `struct AppState` in `main.rs`
- Routes: search `.route(` in `main.rs`
- Background jobs: search `scheduler` or `jobs/`

## Architectural Patterns

**Repository Pattern:** All data access goes through repositories, never raw Diesel in handlers.

**Authentication:** JWT tokens (access + refresh) with middleware layers.

**Encryption:** AES-256-GCM for sensitive data, key from `ENCRYPTION_KEY` env var.

**Error Handling:** Return `Result<T, E>`, use `?` operator, map to HTTP status codes.

**Async:** All I/O is async (Tokio). Use `async fn` and `.await` consistently.

## Git Commits

No AI attribution or co-author lines. Keep messages clean and focused on what changed.

## Git Worktrees

```bash
# Create worktree + open Claude in new tab
git worktree add ../lf-<short-name> -b <branch-name> master

osascript -e 'tell application "iTerm2"
    tell current window
        create tab with default profile
        tell current session
            write text "cd ~/Developer/sites/lf-<short-name> && claude"
        end tell
    end tell
end tell'

# Remove worktree
git worktree remove ../lf-<short-name> && git branch -d <branch-name>

# List worktrees
git worktree list
```

## Safety Guards

Hooks protect against accidental destructive changes:

1. **Migration Guard** - Blocks destructive SQL (DROP, TRUNCATE, DELETE, RENAME, ALTER TYPE)
2. **Protected Files Guard** - Blocks edits to `encryption.rs`, `auth_middleware.rs`, `stripe_webhooks.rs`

**When blocked with `OVERRIDE: touch <path>`:**
1. Ask user for approval with description of change
2. If approved: `touch <path>` then retry
3. Flag auto-deletes after one use

## Skills

Step-by-step guides in `.claude/skills/`:
- `lightfriend-db-migration` - Database schema changes
- `lightfriend-add-integration` - New OAuth integrations
- `lightfriend-add-frontend-page` - New Yew pages

## License

GNU AGPLv3. "Lightfriend" name and branding owned by Rasmus Ahtava, not included in license.
