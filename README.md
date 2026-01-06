# Lightfriend

Full-stack AI assistant SaaS with Rust backend (Axum) and frontend (Yew WebAssembly), integrating with Matrix homeserver for multi-platform messaging.

## Quick Start (Docker - Recommended)

The easiest way to run Lightfriend with all dependencies:

```bash
# 1. Copy environment template
cp .env.example .env

# 2. Generate secrets (only 4 required!)
just generate-secrets >> .env

# 3. Edit .env and verify the generated values
# Note: Bridge tokens are now AUTO-GENERATED - you only need these 4 secrets!

# 4. Build and start all services
just build-prebuilt  # Builds Rust locally, packages in Docker (avoids OOM)
just up              # Starts all services and auto-generates bridge configs

# 5. Create Matrix admin user
just create-admin adminuser YourPassword
```

**That's it!** Services running at:
- Frontend: http://localhost:8080 (run separately: `cd frontend && trunk serve`)
- Backend API: http://localhost:3000
- Synapse: http://localhost:8008

See [DOCKER_SETUP.md](DOCKER_SETUP.md) for complete setup guide.

---

## What's Included

| Component | Description | Port |
|-----------|-------------|------|
| **Backend** | Rust (Axum) API server | 3000 |
| **Frontend** | Yew (WebAssembly) UI | 8080 |
| **Synapse** | Matrix homeserver | 8008 |
| **Bridges** | WhatsApp, Signal, Messenger, Instagram | - |
| **PostgreSQL** | Database for Synapse and bridges | 5432 |

### Key Features

- 🤖 **Multi-Platform Messaging**: Connect WhatsApp, Signal, Messenger, Instagram via Matrix bridges
- 📞 **Voice AI**: ElevenLabs integration for phone calls
- 📧 **Smart Email**: AI-powered email monitoring and notifications
- 📅 **Calendar & Tasks**: Google Calendar/Tasks integration
- 💳 **Payments**: Stripe integration for subscriptions
- 🔐 **Security**: AES-256-GCM encryption, JWT auth, webhook validation

---

## Architecture

### Simplified Bridge Configuration (Auto-Generation)

Previously required **12 environment variables** (4 secrets + 8 bridge tokens).

Now requires only **4 environment variables**:
```bash
MATRIX_HOMESERVER_SHARED_SECRET=...  # Matrix admin
SYNAPSE_DB_PASSWORD=...              # Database
POSTGRES_PASSWORD=...                # Database
DOUBLE_PUPPET_SECRET=...             # Bridge feature
```

**Bridge tokens are auto-generated!** The setup script runs each bridge with `-g` flag to create random tokens automatically. No manual token management needed.

### Build Methods

| Method | RAM Required | Use Case |
|--------|--------------|----------|
| **`just build-prebuilt`** (default) | 6GB Docker + system swap | Mac M1 8GB, memory-constrained systems |
| `just build` (alternative) | 16GB+ Docker memory | High-memory systems, full containerization |

The prebuilt method compiles Rust on your local machine (using full system RAM + swap) and packages the binary in Docker, avoiding OOM errors.

---

## Requirements

### Required Software
- Docker & Docker Compose ([install](https://docs.docker.com/get-docker/))
- Rust toolchain ([rustup.rs](https://rustup.rs)) - for default build method
- Just command runner (optional): `brew install just` or `cargo install just`

### System Requirements
- **Disk**: ~5GB for images + data volumes
- **Memory**: 6-8GB RAM for Docker (set in Docker Desktop preferences)
- **CPU**: 2+ cores recommended

### API Keys (Optional)
- **OpenRouter**: AI model access (required for AI features)
- **Twilio**: SMS/voice integration (optional)
- **Stripe**: Payment processing (optional)
- **Google OAuth**: Calendar/Tasks integration (optional)
- **ElevenLabs**: Voice AI for calls (optional)

---

## Common Commands

```bash
# Lifecycle
just up                          # Start all services
just down                        # Stop all services
just restart                     # Restart all services
just restart-core                # Restart only backend

# Logs
just logs                        # View all logs
just logs-core                   # View backend logs
just logs-synapse                # View Matrix homeserver logs
just logs-bridge whatsapp        # View specific bridge logs

# Status & Maintenance
just status                      # Check service status
just clean                       # Clean up Docker resources
just rebuild                     # Full rebuild (stop, clean, build, start)

# Admin
just create-admin <user> <pass>  # Create Matrix admin user
just shell-core                  # Enter backend container shell
just shell-synapse               # Enter Synapse container shell

# Development (without Docker)
just dev-backend                 # Run backend locally
just dev-frontend                # Run frontend locally
just test-backend                # Run backend tests
just test-frontend               # Run frontend tests
```

---

## Connecting Bridges

After starting services:

1. **Open Element Web**: https://app.element.io
2. **Configure homeserver**: Click "Edit" → Enter `http://localhost:8008`
3. **Sign in** with your Matrix admin user
4. **Start DMs with bridge bots**:
   - WhatsApp: `@whatsappbot:matrix.local`
   - Signal: `@signalbot:matrix.local`
   - Messenger: `@messengerbot:matrix.local`
   - Instagram: `@igbot:matrix.local`
5. **Send `login`** to each bot and follow instructions

See [DOCKER_SETUP.md](DOCKER_SETUP.md) for detailed bridge setup.

---

## Development

### Local Development (without Docker)

For backend/frontend development:

```bash
# Terminal 1: Backend
cd backend && cargo run

# Terminal 2: Frontend
cd frontend && trunk serve
```

This requires manual Matrix server setup. See [MATRIX_SETUP_GUIDE.md](MATRIX_SETUP_GUIDE.md).

### Modifying Code

After changing Rust code:

```bash
# Rebuild and restart
just build-prebuilt
just restart-core
```

After changing configs:

```bash
# Regenerate and restart
cd docker && ./setup-configs.sh
just restart
```

---

## Troubleshooting

### Out of Memory During Build

Use the prebuilt method (default):
```bash
just build-prebuilt  # Compiles on host, packages in Docker
```

If still having issues, increase Docker memory:
- Docker Desktop → Preferences → Resources → Memory → Set to 6-8GB

### Bridge Not Connecting

Check bridge logs:
```bash
just logs-bridge whatsapp
```

Common issues:
- Synapse not started: `just status` → Ensure Synapse is healthy
- Wrong tokens: Regenerate configs with `cd docker && ./setup-configs.sh` then `docker restart lightfriend-synapse`

### Database Errors

Check postgres logs:
```bash
cd docker && docker compose logs postgres
```

Reset database (⚠️ deletes all data):
```bash
just down-volumes
just up
```

See [DOCKER_SETUP.md](DOCKER_SETUP.md) for complete troubleshooting guide.

---

## Data & Backups

All data stored in Docker volumes:
- `postgres_data` - All databases (Synapse + bridges) ← **BACKUP THIS**
- `synapse_data` - Synapse signing keys and media ← **BACKUP THIS**
- `core_data` - Lightfriend SQLite database ← **BACKUP THIS**
- `signald_data` - Signal device registration ← **BACKUP THIS**

See [DOCKER_SETUP.md](DOCKER_SETUP.md) for backup commands and automated backup script.

---

## Documentation

- **[DOCKER_SETUP.md](DOCKER_SETUP.md)** - Complete Docker setup, commands, troubleshooting
- **[MATRIX_SETUP_GUIDE.md](MATRIX_SETUP_GUIDE.md)** - Manual Matrix setup (for local dev)
- **[CLAUDE.md](CLAUDE.md)** - Project architecture and development guide

---

## License

This project is licensed under the **GNU Affero General Public License v3**. See the LICENSE file for details.

The name "Lightfriend" and any associated branding (including logos, icons, or visual elements) are owned by Rasmus Ähtävä. These elements are not included in the AGPLv3 license and may not be used without permission, especially for commercial purposes or in ways that imply endorsement or affiliation. Forks or derivatives should use a different name and branding to avoid confusion.
