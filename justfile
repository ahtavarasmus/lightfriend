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

# Optional: point git at .githooks/ when you want local checks.
install-hooks:
    #!/usr/bin/env bash
    set -euo pipefail
    current=$(git config --get core.hooksPath 2>/dev/null || echo "")
    if [ "$current" != ".githooks" ]; then
      git config core.hooksPath .githooks
      echo "Installed git hooks (core.hooksPath = .githooks)."
    fi

# Development: Run backend locally (auto-installs macOS deps on first run)
dev-backend: setup-mac
    cd backend && cargo run

# Development: Run frontend locally (not in Docker)
dev-frontend:
    cd frontend && trunk serve

# Development: Fast backend compile check without linking/running tests
check-backend:
    cd backend && cargo check --bin backend

# Development: Fast backend compile check for test code without linking tests
check-backend-tests:
    cd backend && cargo check --tests

# Development: Run tests for backend
test-backend ARGS="":
    cd backend && cargo test {{ARGS}}

# Development: Run one integration test crate, e.g. `just test-backend-file sms_sanitizer_test`
test-backend-file FILE:
    cd backend && cargo test --test {{FILE}}

# Development: Run tests for frontend
test-frontend:
    cd frontend && cargo test

# ── Fuzzing (security-critical pure functions) ──────────────────────────
# Requires: rustup install nightly && cargo install cargo-fuzz
# See backend/fuzz/README.md for what these check and how to triage findings.

# Fuzz the Twilio webhook signature verifier for 60 seconds
fuzz-twilio:
    cd backend/fuzz && cargo +nightly fuzz run twilio_signature -- -max_total_time=60

# Fuzz the Telnyx webhook signature verifier for 60 seconds
fuzz-telnyx:
    cd backend/fuzz && cargo +nightly fuzz run telnyx_signature -- -max_total_time=60

# Fuzz all webhook signature verifiers (60 seconds each, sequential)
fuzz-all: fuzz-twilio fuzz-telnyx

# ── Formal verification (Kani bounded model checking) ───────────────────
# Requires: cargo install --locked kani-verifier && cargo kani setup
# Proves properties hold for ALL bounded inputs - not just samples.
# See backend/fuzz/README.md (Kani section) for scope and rationale.

# Run every #[kani::proof] in the backend crate
kani:
    cd backend && cargo kani --tests
