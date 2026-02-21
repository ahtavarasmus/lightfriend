# syntax=docker/dockerfile:1.4

# Build stage for backend
FROM rust:1.86-bookworm AS backend-builder

# Install cargo-chef and sccache for faster builds
RUN cargo install cargo-chef sccache --locked

# Install diesel_cli with sqlite support
RUN cargo install diesel_cli --no-default-features --features sqlite --locked

# Configure sccache
ENV RUSTC_WRAPPER=/usr/local/cargo/bin/sccache
ENV SCCACHE_DIR=/sccache
ENV SCCACHE_CACHE_SIZE="2G"

WORKDIR /app

# Stage 1: Generate recipe file for dependencies
FROM backend-builder AS planner
COPY backend/Cargo.toml backend/Cargo.lock ./
COPY backend/src ./src
RUN cargo chef prepare --recipe-path recipe.json

# Stage 2: Build dependencies (this layer is cached unless dependencies change)
FROM backend-builder AS cacher
COPY --from=planner /app/recipe.json recipe.json
# Install mold linker (2-3x faster than lld)
RUN apt-get update && apt-get install -y \
    libsqlite3-dev \
    pkg-config \
    libssl-dev \
    mold \
    clang \
    && rm -rf /var/lib/apt/lists/*

# Configure sccache again
ENV RUSTC_WRAPPER=/usr/local/cargo/bin/sccache
ENV SCCACHE_DIR=/sccache

# Use cache mounts for cargo registry, git checkouts, and sccache
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/sccache \
    cargo chef cook --release --recipe-path recipe.json && \
    sccache --show-stats

# Stage 3: Build application
FROM backend-builder AS builder
COPY --from=cacher /app/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo
# Install mold linker and clang
RUN apt-get update && apt-get install -y \
    libsqlite3-dev \
    pkg-config \
    libssl-dev \
    mold \
    clang \
    && rm -rf /var/lib/apt/lists/*
COPY backend /app
WORKDIR /app

# Configure sccache and build optimizations
ENV RUSTC_WRAPPER=/usr/local/cargo/bin/sccache
ENV SCCACHE_DIR=/sccache
# Use mold linker for 2-3x faster linking
ENV RUSTFLAGS="-C link-arg=-fuse-ld=mold -C linker=clang"

# Build with cache mounts for cargo, sccache, and build artifacts
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/sccache \
    --mount=type=cache,target=/app/target \
    cargo build --release && \
    sccache --show-stats && \
    cp target/release/backend /backend && \
    strip /backend

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libsqlite3-0 \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy backend binary and diesel_cli
COPY --from=builder /backend ./backend
COPY --from=builder /usr/local/cargo/bin/diesel ./diesel

# Copy migrations
COPY backend/migrations ./migrations
COPY backend/diesel.toml ./

# Copy static files (robots.txt, sitemap.xml, llms.txt)
COPY backend/static ./static

# Create directories for runtime data
RUN mkdir -p data uploads matrix_store

# Expose backend port
EXPOSE 3000

# Copy entrypoint script
COPY docker/core/entrypoint.sh ./entrypoint.sh
RUN chmod +x ./entrypoint.sh

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/api/health || exit 1

# Run backend with migrations
ENTRYPOINT ["./entrypoint.sh"]
