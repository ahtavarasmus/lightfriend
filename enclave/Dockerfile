# syntax=docker/dockerfile:1.4

# Single-container build for Nitro Enclave deployment
# Runs PostgreSQL, Tuwunel, mautrix bridges, and Lightfriend backend
# all under supervisord in one container.

# ── Stage 1: Build Lightfriend backend ──────────────────────────────────────

FROM rust:1.88-bookworm AS backend-builder

RUN cargo install cargo-chef sccache --locked

ENV RUSTC_WRAPPER=/usr/local/cargo/bin/sccache
ENV SCCACHE_DIR=/sccache
ENV SCCACHE_CACHE_SIZE="2G"

WORKDIR /app

# Generate recipe for dependency caching
FROM backend-builder AS planner
COPY backend/Cargo.toml backend/Cargo.lock ./
COPY backend/src ./src
RUN cargo chef prepare --recipe-path recipe.json

# Build dependencies (cached unless Cargo.toml/lock changes)
FROM backend-builder AS cacher
COPY --from=planner /app/recipe.json recipe.json
RUN apt-get update && apt-get install -y \
    libpq-dev \
    pkg-config \
    libssl-dev \
    mold \
    clang \
    && rm -rf /var/lib/apt/lists/*

ENV RUSTC_WRAPPER=/usr/local/cargo/bin/sccache
ENV SCCACHE_DIR=/sccache

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/sccache \
    cargo chef cook --release --recipe-path recipe.json && \
    sccache --show-stats

# Build application
FROM backend-builder AS builder
COPY --from=cacher /app/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo
RUN apt-get update && apt-get install -y \
    libpq-dev \
    pkg-config \
    libssl-dev \
    mold \
    clang \
    && rm -rf /var/lib/apt/lists/*
COPY backend /app
WORKDIR /app

ENV RUSTC_WRAPPER=/usr/local/cargo/bin/sccache
ENV SCCACHE_DIR=/sccache
ENV RUSTFLAGS="-C link-arg=-fuse-ld=mold -C linker=clang"

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/sccache \
    --mount=type=cache,target=/app/target \
    cargo build --release && \
    sccache --show-stats && \
    cp target/release/backend /backend && \
    cp target/release/tuwunel_checkpoint /tuwunel_checkpoint && \
    strip /tuwunel_checkpoint && \
    strip /backend

# ── Stage 2: Build Frontend (Yew/WASM) ──────────────────────────────────────

FROM rust:1.88-bookworm AS frontend-builder

RUN rustup target add wasm32-unknown-unknown
RUN cargo install trunk --locked

WORKDIR /app
COPY frontend/Cargo.toml frontend/Cargo.lock ./
COPY frontend/src ./src
COPY frontend/index.html ./
COPY frontend/assets ./assets
COPY frontend/Trunk.toml ./
COPY frontend/robots.txt ./

RUN trunk build --release

# ── Stage 2b: Build pinned Marlin Oyster KMS helpers ────────────────────────

FROM rust:1.88-bookworm AS marlin-builder

ARG OYSTER_MONOREPO_COMMIT=f60874a27f56eee2974cf0099ff80e9627c8f1dd

RUN apt-get update && apt-get install -y \
    git \
    libsodium-dev \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src/oyster

RUN git clone https://github.com/marlinprotocol/oyster-monorepo . && \
    git checkout "${OYSTER_MONOREPO_COMMIT}"

RUN cargo build --release --target-dir /src/oyster/target --manifest-path initialization/keygen/Cargo.toml --bin keygen-x25519 && \
    cargo build --release --target-dir /src/oyster/target --manifest-path attestation/server-custom/Cargo.toml && \
    cargo build --release --target-dir /src/oyster/target --manifest-path kms/derive-server/Cargo.toml && \
    ls -la /src/oyster/target/release/keygen-x25519 /src/oyster/target/release/oyster-attestation-server-custom /src/oyster/target/release/kms-derive-server

# ── Stage 3: Collect bridge binaries from official images ───────────────────

FROM jevolk/tuwunel:latest AS tuwunel-src
FROM dock.mau.dev/mautrix/whatsapp:latest AS whatsapp-src
FROM dock.mau.dev/mautrix/signal:latest AS signal-src
FROM dock.mau.dev/mautrix/telegram:latest AS telegram-src

# ── Stage 4: Runtime - single container with all services ───────────────────

FROM debian:bookworm-slim

# BUILD_MODE: "production" (default) includes cloudflared for tunnel access.
#             "local" skips it (not needed for local testing).
ARG BUILD_MODE=production

# Install runtime dependencies + PostgreSQL + supervisord
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libpq5 \
    libssl3 \
    libmagic1 \
    curl \
    socat \
    sqlite3 \
    postgresql-15 \
    openssl \
    supervisor \
    python3 \
    python3-pip \
    python3-venv \
    ffmpeg \
    iproute2 \
    && rm -rf /var/lib/apt/lists/*

# ── Install cloudflared for Cloudflare tunnel (production only) ──────────────
ARG TARGETARCH
RUN if [ "$BUILD_MODE" = "production" ]; then \
        curl -L "https://github.com/cloudflare/cloudflared/releases/latest/download/cloudflared-linux-${TARGETARCH}" \
            -o /usr/local/bin/cloudflared && chmod +x /usr/local/bin/cloudflared; \
    fi

# ── Copy Tuwunel binary (path varies by arch: /usr/local/bin on amd64, /usr/bin on arm64)
RUN --mount=from=tuwunel-src,target=/tuwunel-img \
    cp /tuwunel-img/usr/local/bin/tuwunel /usr/local/bin/tuwunel 2>/dev/null || \
    cp /tuwunel-img/usr/bin/tuwunel /usr/local/bin/tuwunel

# ── Copy mautrix-whatsapp binary ───────────────────────────────────────────
COPY --from=whatsapp-src /usr/bin/mautrix-whatsapp /usr/local/bin/mautrix-whatsapp

# ── Copy mautrix-signal binary ────────────────────────────────────────────
COPY --from=signal-src /usr/bin/mautrix-signal /usr/local/bin/mautrix-signal

# ── Install mautrix-telegram into a local venv ────────────────────────────
ARG MAUTRIX_TELEGRAM_VERSION=0.15.3
COPY --from=telegram-src /opt/mautrix-telegram /opt/mautrix-telegram
RUN python3 -m venv /opt/mautrix-telegram-venv && \
    /opt/mautrix-telegram-venv/bin/pip install --upgrade pip && \
    /opt/mautrix-telegram-venv/bin/pip install "mautrix-telegram[all]==${MAUTRIX_TELEGRAM_VERSION}"

# ── Copy Lightfriend backend ──────────────────────────────────────────────
WORKDIR /app
COPY --from=builder /backend ./backend
COPY --from=builder /tuwunel_checkpoint /usr/local/bin/tuwunel_checkpoint
# ── Copy Marlin Oyster binaries (production only, skipped for local builds) ──
RUN --mount=from=marlin-builder,target=/marlin-img \
    if [ "$BUILD_MODE" = "production" ]; then \
        echo "Searching for Marlin binaries..." && \
        find /marlin-img -type f -name 'keygen-x25519' -o -name 'oyster-attestation-server*' -o -name 'kms-derive*' 2>/dev/null && \
        KEYGEN_BIN="$(find /marlin-img -type f -path '*/release/keygen-x25519' | head -1)" && \
        ATTESTATION_BIN="$(find /marlin-img -type f -path '*/release/oyster-attestation-server-custom' | head -1)" && \
        KMS_DERIVE_BIN="$(find /marlin-img -type f -path '*/release/kms-derive-server' | head -1)" && \
        echo "Found: keygen=$KEYGEN_BIN attestation=$ATTESTATION_BIN derive=$KMS_DERIVE_BIN" && \
        [ -n "$KEYGEN_BIN" ] || { echo "FATAL: keygen-x25519 not found"; exit 1; } && \
        [ -n "$ATTESTATION_BIN" ] || { echo "FATAL: attestation server not found"; exit 1; } && \
        [ -n "$KMS_DERIVE_BIN" ] || { echo "FATAL: kms-derive-server not found"; exit 1; } && \
        cp "$KEYGEN_BIN" /usr/local/bin/keygen-x25519 && \
        cp "$ATTESTATION_BIN" /usr/local/bin/oyster-attestation-server && \
        cp "$KMS_DERIVE_BIN" /usr/local/bin/kms-derive-server && \
        ls -la /usr/local/bin/keygen-x25519 /usr/local/bin/oyster-attestation-server /usr/local/bin/kms-derive-server; \
    fi
COPY --from=frontend-builder /app/dist ./public/

# Create runtime directories
RUN mkdir -p \
    data \
    uploads \
    matrix_store \
    /data/bridges/whatsapp \
    /data/bridges/signal \
    /data/bridges/telegram \
    /data/seed \
    /var/lib/tuwunel \
    /etc/tuwunel \
    /var/log/supervisor \
    /run/postgresql

# Set PostgreSQL data dir ownership and log dir permissions
RUN chown -R postgres:postgres /run/postgresql && \
    chown -R postgres:postgres /var/log/supervisor

# ── Copy config templates and scripts ─────────────────────────────────────
COPY enclave/configs/ /etc/enclave-configs/
COPY enclave/supervisord.conf /etc/supervisor/conf.d/lightfriend.conf
COPY enclave/entrypoint.sh /entrypoint.sh
COPY enclave/derive-backup-key.sh /usr/local/bin/derive-backup-key.sh
COPY enclave/start-marlin-kms.sh /usr/local/bin/start-marlin-kms.sh
COPY enclave/export.sh /app/export.sh
COPY enclave/pg-backup.sh /app/pg-backup.sh
COPY enclave/verify.sh /app/verify.sh
COPY enclave/enter-maintenance.sh /app/enter-maintenance.sh
COPY enclave/diagnose-enclave.sh /app/diagnose-enclave.sh
COPY enclave/service-watchdog.sh /app/service-watchdog.sh
# Prepare resolv.conf for runtime. Docker mounts /etc/resolv.conf during build
# so we can't modify it here. Store a template and copy at container start.
RUN echo "nameserver 127.0.0.1" > /etc/resolv.conf.enclave

RUN chmod +x /entrypoint.sh /usr/local/bin/derive-backup-key.sh /usr/local/bin/start-marlin-kms.sh /app/export.sh /app/pg-backup.sh /app/verify.sh /app/enter-maintenance.sh /app/service-watchdog.sh /app/diagnose-enclave.sh

# Copy bridge bot registration script
COPY enclave/register-bridge-bots.sh /app/register-bridge-bots.sh
RUN chmod +x /app/register-bridge-bots.sh

EXPOSE 3000 8008

HEALTHCHECK --interval=30s --timeout=3s --start-period=30s --retries=3 \
    CMD curl -f http://localhost:8008/_matrix/client/versions || exit 1

ENTRYPOINT ["/entrypoint.sh"]
