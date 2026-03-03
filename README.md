# Lightfriend

Full-stack AI assistant SaaS with Rust backend (Axum) and frontend (Yew WebAssembly), integrating with Matrix homeserver for multi-platform messaging.

## Quick Start (Docker)

```bash
# 1. Copy environment template
cp .env.example .env

# 2. Generate secrets
just generate-secrets >> .env

# 3. Edit .env and verify the generated values

# 4. Build and start all services
just build-prebuilt
just up

# 5. Create Matrix admin user
just create-admin adminuser YourPassword
```

Services will be running at:
- **Frontend**: http://localhost:8080 (or run separately: `cd frontend && trunk serve`)
- **Backend API**: http://localhost:3000
- **Synapse**: http://localhost:8008

## Services

| Component | Description | Port |
|-----------|-------------|------|
| Backend | Rust (Axum) API server | 3000 |
| Frontend | Yew (WebAssembly) UI | 8080 |
| Synapse | Matrix homeserver | 8008 |
| Bridges | WhatsApp, Signal, Messenger, Instagram | - |
| PostgreSQL | Database for Synapse and bridges | 5432 |

## Local Development (without Docker)

```bash
# Terminal 1: Backend
cd backend && cargo run

# Terminal 2: Frontend
cd frontend && trunk serve
```

Requires manual Matrix server setup - see [Matrix Setup Guide](docs/MATRIX_SETUP_GUIDE.md).

## Documentation

- [Docker Setup](docs/DOCKER_SETUP.md) - commands, build methods, troubleshooting, backups
- [Matrix Setup Guide](docs/MATRIX_SETUP_GUIDE.md) - manual Matrix setup for local dev
- [Infrastructure Setup](docs/INFRASTRUCTURE_SETUP.md) - cloud deployment with Terraform
- [CLAUDE.md](CLAUDE.md) - project architecture and development guide

## License

This project is licensed under the **GNU Affero General Public License v3**. See the LICENSE file for details.

The name "Lightfriend" and any associated branding (including logos, icons, or visual elements) are owned by Rasmus Ahtava. These elements are not included in the AGPLv3 license and may not be used without permission, especially for commercial purposes or in ways that imply endorsement or affiliation. Forks or derivatives should use a different name and branding to avoid confusion.
