# Build stage for backend
FROM rust:1.85-bookworm as backend-builder

# Set environment variables to reduce memory usage
ENV CARGO_BUILD_JOBS=1
ENV CARGO_NET_GIT_FETCH_WITH_CLI=true
ENV CARGO_HTTP_MULTIPLEXING=false

# Install build dependencies including lld linker
RUN apt-get update && apt-get install -y \
    libsqlite3-dev \
    pkg-config \
    libssl-dev \
    lld \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app/backend

# Copy backend manifests and cargo config
COPY backend/Cargo.toml backend/Cargo.lock ./
COPY backend/.cargo ./.cargo

# Create dummy main to cache dependencies
# CARGO_BUILD_JOBS=1 limits to single-threaded compilation
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy actual source code
COPY backend/src ./src
COPY backend/migrations ./migrations
COPY backend/diesel.toml ./

# Build backend (this will use cached dependencies)
# Single-threaded to minimize memory usage
RUN touch src/main.rs && cargo build --release && \
    strip target/release/backend

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libsqlite3-0 \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy backend binary
COPY --from=backend-builder /app/backend/target/release/backend ./backend

# Copy migrations
COPY --from=backend-builder /app/backend/migrations ./migrations
COPY --from=backend-builder /app/backend/diesel.toml ./

# Create directories for runtime data
RUN mkdir -p uploads matrix_store

# Expose backend port
EXPOSE 3000

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD ["/usr/bin/curl", "-f", "http://localhost:3000/api/health", "||", "exit", "1"]

# Run backend
CMD ["./backend"]
