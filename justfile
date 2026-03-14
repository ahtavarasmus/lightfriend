# Lightfriend

# Default recipe (show available commands)
default:
    @just --list

# ── Docker (enclave) ────────────────────────────────────────────────────

# Build enclave image for current platform (for local testing)
build-local:
    cd enclave && docker compose build

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

# Development: Run backend locally (not in Docker)
dev-backend:
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
