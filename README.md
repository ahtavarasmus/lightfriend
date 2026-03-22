# Lightfriend

Full-stack AI assistant SaaS with Rust backend (Axum) and frontend (Yew WebAssembly), integrating with Matrix homeserver for multi-platform messaging.

The code is open source for verifiability - the production deployment runs inside an AWS Nitro Enclave.

## Local Development

```bash
# Terminal 1: Backend
cd backend && cargo run

# Terminal 2: Frontend
cd frontend && trunk serve
```

- **Backend API**: http://localhost:3000
- **Frontend**: http://localhost:8080

## Docker (Enclave)

The enclave image bundles everything (PostgreSQL, Tuwunel, mautrix bridges, Lightfriend backend) into a single container under supervisord.

```bash
# Build for current platform (local testing)
just build-local

# Start
just up

# View logs
just logs
```

See `just --list` for all available commands.

## Documentation

- [Matrix Setup Guide](docs/MATRIX_SETUP_GUIDE.md) - manual Matrix setup for local dev
- [Infrastructure Setup](docs/INFRASTRUCTURE_SETUP.md) - cloud deployment with Terraform
- [CLAUDE.md](CLAUDE.md) - project architecture and development guide

## License

This project is licensed under the **GNU Affero General Public License v3**. See the LICENSE file for details.

The name "Lightfriend" and any associated branding (including logos, icons, or visual elements) are owned by Rasmus Ahtava. These elements are not included in the AGPLv3 license and may not be used without permission, especially for commercial purposes or in ways that imply endorsement or affiliation. Forks or derivatives should use a different name and branding to avoid confusion.
