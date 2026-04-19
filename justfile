# Lightfriend

# Default recipe (show available commands)
default:
    @just --list

# ── Docker (enclave) ────────────────────────────────────────────────────

# Build enclave image for current platform (for local testing)
build-local:
    cd enclave && BUILD_MODE=local docker compose build

# Build enclave image for production (linux/amd64)
build:
    cd enclave && docker buildx build \
        --platform linux/amd64 \
        --build-arg BUILDKIT_INLINE_CACHE=1 \
        -t lightfriend-enclave:latest \
        -f Dockerfile \
        --load \
        ..

# Start enclave (pulls latest image if available)
up:
    cd enclave && docker compose pull --ignore-pull-failures && docker compose up -d

# Pull latest enclave image from registry
pull:
    cd enclave && docker compose pull

# Stop enclave
down:
    cd enclave && docker compose down

# View enclave logs
logs:
    cd enclave && docker compose logs -f

# Full encrypted export of all data stores from running enclave
export:
    docker exec lightfriend-enclave /app/export.sh
    @ls -lh enclave/seed/lightfriend-full-backup-*.tar.gz.enc 2>/dev/null | tail -1

# Import: place backup in seed/ and start new enclave
import BACKUP:
    cp {{BACKUP}} enclave/seed/
    just up
    @echo "Waiting for restore and verification..."
    @echo "Check logs: just logs"
    @echo "Check verify result: just verify"
    @echo "Only shut down old enclave AFTER verification passes."

# Run post-restore verification (or check last result)
verify:
    docker exec lightfriend-enclave /app/verify.sh

# ── Development ─────────────────────────────────────────────────────────

# Install macOS Homebrew prerequisites (idempotent; no-op on non-macOS)
setup-mac:
    #!/usr/bin/env bash
    # Installs the keg-only Homebrew formulas the backend links against at
    # runtime. rpaths to their `brew --prefix`/lib dirs are baked into
    # backend/.cargo/config.toml. brew install is fast when already present.
    set -euo pipefail
    if [ "$(uname)" != "Darwin" ]; then
      exit 0
    fi
    if ! command -v brew >/dev/null 2>&1; then
      echo "ERROR: Homebrew not installed. Install from https://brew.sh first."
      exit 1
    fi
    brew install sqlite openssl@3 libiconv postgresql@14

# Development: Run backend locally (auto-installs macOS deps on first run)
dev-backend: setup-mac
    cd backend && cargo run

# Development: Run frontend locally (not in Docker)
dev-frontend:
    cd frontend && trunk serve

# Development: Run tests for backend
test-backend:
    cd backend && cargo test

# Development: Run tests for frontend
test-frontend:
    cd frontend && cargo test
