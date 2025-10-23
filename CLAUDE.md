# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Lightfriend is a self-hostable AI assistant platform designed for deployment on Coolify with full zero-trust privacy. It integrates with messaging platforms (WhatsApp, Telegram, Signal, Instagram, Messenger), calendar, email, tasks, and phone services through Matrix bridges. The platform consists of a Rust backend (Axum) and a Rust frontend (Yew WASM), designed to be fully cloud-hosted while maintaining user privacy.

## Tech Stack

- **Backend**: Rust + Axum web framework
- **Frontend**: Rust + Yew (WebAssembly)
- **Database**: SQLite with Diesel ORM
- **Matrix Integration**: matrix-sdk for messaging bridges
- **Build Tools**: Cargo, Trunk (frontend), Docker
- **Deployment**: Docker Compose + Nginx reverse proxy

## Development Commands

### Backend Development

```bash
cd backend
cargo run                    # Run backend server (port 3001 dev, 3101 staging)
cargo build --release        # Build optimized backend
cargo check                  # Quick syntax check
diesel migration run         # Run database migrations
diesel migration revert      # Rollback last migration
```

### Frontend Development

```bash
cd frontend
trunk serve                  # Run dev server with hot reload (port 8080)
trunk build --release        # Build optimized WASM bundle
```

### Docker Deployment

```bash
docker-compose up --build    # Build and run all services
docker-compose down          # Stop all services
```

The stack runs:
- Nginx on port 80 (serves frontend + proxies API to backend)
- Backend on internal port 3100
- Frontend built as static WASM files

## Architecture

### Backend Structure (`backend/src/`)

- **`main.rs`**: Application entry point, router setup, middleware configuration
- **`handlers/`**: HTTP request handlers organized by feature
  - `auth_handlers.rs`, `auth_middleware.rs`: Authentication & JWT
  - `*_auth.rs` files: OAuth flows for various services (Google Calendar, Telegram, Signal, etc.)
  - `*_handlers.rs` files: Service-specific API handlers (WhatsApp, Telegram, Signal, Instagram, Messenger, IMAP, Uber)
  - `profile_handlers.rs`: User profile management
  - `filter_handlers.rs`: Message filtering (priority senders, keywords, waiting checks)
  - `self_host_handlers.rs`: Self-hosting configuration (Twilio, TextBee)
- **`repositories/`**: Database access layer
  - `user_core.rs`: Core user operations (CRUD, auth)
  - `user_repository.rs`: Extended user data operations
- **`models/`**: Data models and schema types
- **`utils/`**: Shared utilities
  - `encryption.rs`: AES-GCM encryption for sensitive data
  - `tool_exec.rs`: AI tool execution logic
  - `matrix_auth.rs`: Matrix homeserver registration/authentication
  - `bridge.rs`: Matrix bridge utilities
  - `imap_utils.rs`: Email parsing and operations
- **`tool_call_utils/`**: AI assistant tool implementations
  - `email.rs`, `calendar.rs`, `tasks.rs`, `bridge.rs`, `internet.rs`, `management.rs`
- **`api/`**: External API integrations
  - `twilio_sms.rs`, `twilio_utils.rs`: SMS handling
  - `elevenlabs.rs`, `elevenlabs_webhook.rs`: Voice assistant integration
- **`jobs/`**: Background job scheduler (cron-like tasks)
- **`proactive/`**: Proactive agent utilities

### Frontend Structure (`frontend/src/`)

- **`main.rs`**: Yew app entry point and routing
- **`pages/`**: Page components (home, privacy, setup instructions, changelog, etc.)
- **`connections/`**: Service connection UI (calendar, email, messaging platforms)
- **`auth/`**: Authentication UI (signup, connect)
- **`profile/`**: User settings (timezone detection, models)
- **`proactive/`**: Proactive agent settings (digests, critical contacts, waiting checks, monitoring)
- **`components/`**: Reusable UI components
- **`blog/`**: Blog post components
- **`config.rs`**: API endpoint configuration

### Database

- **ORM**: Diesel with SQLite
- **Migrations**: `backend/migrations/` (timestamped SQL up/down files)
- **Location**:
  - Development: `database.db` (hardcoded in main.rs:156)
  - Production: `/app/data/database.db` (Docker volume-mounted)
- **Key Tables**: users, google_calendar_connections, google_tasks_connections, imap_connections, telegram/signal/whatsapp/messenger/instagram status tables, subscriptions, usage_logs, waiting_checks, priority_senders, keywords

### Matrix Integration

The app uses Matrix protocol for unified messaging:
- **Homeserver**: Configurable via `MATRIX_HOMESERVER` env var
- **Bridges**: Designed to support mautrix bridges (WhatsApp, Telegram, Signal, Instagram, Messenger)
- **Authentication**: Shared secret registration (`MATRIX_SHARED_SECRET`)
- **Storage**: Persistent store path via `MATRIX_HOMESERVER_PERSISTENT_STORE_PATH`
- **Double Puppeting**: See `notes-for-matrix-bridges.md` for configuration details

### Environment Variables

Required for backend (see `validate_env()` in main.rs):
- `JWT_SECRET_KEY`, `JWT_REFRESH_KEY`: Authentication tokens
- `DATABASE_URL`: SQLite database path
- `ENVIRONMENT`: "development" or "staging"
- `FRONTEND_URL`: CORS-allowed origin
- `SERVER_URL`, `SERVER_URL_OAUTH`: Backend URLs for OAuth callbacks
- `ENCRYPTION_KEY`: AES-GCM key for sensitive data
- `MATRIX_HOMESERVER`, `MATRIX_SHARED_SECRET`, `MATRIX_HOMESERVER_PERSISTENT_STORE_PATH`: Matrix config
- Optional: Google OAuth (`GOOGLE_CALENDAR_CLIENT_ID`, `GOOGLE_CALENDAR_CLIENT_SECRET`), Uber OAuth, Twilio, TextBee, ElevenLabs

### Docker Configuration

- **`Dockerfile.backend`**: Multi-stage build (Rust builder + Debian runtime)
  - Low-memory optimizations: `CARGO_BUILD_JOBS=1`, serial compilation
  - Volume: `/app/data` for database persistence
  - Exposes port 3100 internally
- **`Dockerfile.frontend`**: Builds static WASM with Trunk, outputs to `/app/dist`
- **`docker-compose.yaml`**:
  - `backend` service (builds, mounts data volume)
  - `frontend` service (build-only, outputs to shared volume)
  - `nginx` service (serves frontend dist + backend static, proxies /api/ to backend)
  - Volumes: `data` (database), `frontend_dist` (built WASM)
  - Network: `app-network` bridge

### Ports

- **Development**:
  - Backend: 3001
  - Frontend (Trunk): 8080
- **Docker/Production**:
  - Nginx: 80 (external)
  - Backend: 3100 (internal)

## Coolify Deployment

The app is designed for one-click Coolify deployment:
1. Coolify pulls this repo
2. Runs `docker-compose up` (uses COOLIFY_FQDN for domain)
3. Database persists in `data` volume
4. Nginx handles SSL termination (Coolify manages this)

## Future Extensibility: Matrix Homeserver + Bridges

The architecture is prepared for adding Matrix homeserver and mautrix bridges:
- `notes-for-matrix-bridges.md` contains detailed configuration for:
  - Synapse homeserver setup
  - mautrix-whatsapp, mautrix-telegram, mautrix-signal, mautrix-instagram bridges
  - Double puppeting configuration
  - Database setup (PostgreSQL for multi-user, SQLite for single-user)
- Nginx config has placeholder comments for future mautrix proxy routes
- Backend already uses matrix-sdk for client operations

To add Matrix services to Docker:
1. Add `synapse` service to docker-compose.yaml (port 8008)
2. Add mautrix bridge services (e.g., mautrix-whatsapp on port 29318)
3. Mount bridge configs from `./matrix-bridges/`
4. Update nginx.conf to proxy `/public/` routes to bridges
5. Set up shared secrets and registration files per `notes-for-matrix-bridges.md`

## Code Patterns

### Authentication Flow
1. User registers via self-hosted login (`/api/self-hosted/login`)
2. Backend issues JWT access + refresh tokens
3. Frontend stores tokens, sends via Authorization header
4. `auth_middleware::require_auth` validates JWT on protected routes

### OAuth Flows
1. Frontend redirects to `/api/auth/{service}/login`
2. Backend generates OAuth URL, stores PKCE in session
3. User authorizes at provider
4. Provider redirects to `/api/auth/{service}/callback`
5. Backend exchanges code for tokens, encrypts and stores in DB

### Matrix Bridge Operations
1. User connects service (e.g., WhatsApp) via `/api/auth/whatsapp/connect`
2. Backend uses `matrix_auth::register_matrix_user` to create Matrix user
3. Backend initializes matrix-sdk Client, stores in `AppState.matrix_clients`
4. Sync loop started in background (`AppState.matrix_sync_tasks`)
5. Messages fetched via Matrix rooms, sent via `matrix-sdk` methods

### Encryption
- Sensitive tokens (OAuth, IMAP passwords) encrypted via `utils/encryption.rs`
- Uses AES-GCM-256 with key from `ENCRYPTION_KEY` env var
- Base64-encoded ciphertext stored in DB

### Database Migrations
- Create new migration: `cd backend && diesel migration generate migration_name`
- Edit `up.sql` and `down.sql` in generated folder
- Apply: `diesel migration run`
- Schema auto-updates in `backend/src/schema.rs`

## Key Dependencies

Backend:
- `axum`: Web framework
- `diesel`: ORM
- `matrix-sdk`: Matrix protocol client
- `jsonwebtoken`: JWT auth
- `oauth2`: OAuth flows
- `aes-gcm`: Encryption
- `reqwest`: HTTP client
- `tokio-cron-scheduler`: Background jobs
- `lettre`: Email sending
- `imap`: Email reading
- `openai-api-rs`: OpenAI integration

Frontend:
- `yew`: Reactive web framework (WASM)
- `yew-router`: Client-side routing
- `gloo-net`: HTTP requests
- `chrono`, `chrono-tz`: Date/time handling
- `stylist`: CSS-in-Rust

## License

AGPLv3 (code) + Proprietary branding (name "Lightfriend" and associated assets)
