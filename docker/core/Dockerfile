# syntax=docker/dockerfile:1.4

# Build stage for backend
FROM rust:1.85-bookworm AS backend-builder

# Install cargo-chef for dependency caching
RUN cargo install cargo-chef

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
    && rm -rf /var/lib/apt/lists/*
# Use cache mounts for cargo registry and git checkouts
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo chef cook --release --recipe-path recipe.json

# Stage 3: Build application
FROM backend-builder AS builder
COPY --from=cacher /app/target target
COPY --from=cacher /usr/local/cargo /usr/local/cargo
# Install mold linker
RUN apt-get update && apt-get install -y \
    libsqlite3-dev \
    pkg-config \
    libssl-dev \
    mold \
    && rm -rf /var/lib/apt/lists/*
COPY backend /app
WORKDIR /app
# Use mold linker for faster linking
ENV RUSTFLAGS="-C link-arg=-fuse-ld=mold"
# Build with cache mounts and parallel compilation
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/app/target \
    cargo build --release && \
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

# Copy backend binary
COPY --from=builder /backend ./backend

# Copy migrations
COPY backend/migrations ./migrations
COPY backend/diesel.toml ./

# Create directories for runtime data
RUN mkdir -p uploads matrix_store

# Expose backend port
EXPOSE 3000

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/api/health || exit 1

# Run backend
CMD ["./backend"]
