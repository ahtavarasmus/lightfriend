# Docker Setup Guide

Complete guide to run Lightfriend with Docker, including Matrix homeserver and mautrix bridges.

## Overview

The Docker setup consists of multiple services:

| Service | Description | Port |
|---------|-------------|------|
| **core** | Lightfriend Rust backend (API only) | 3000 |
| **synapse** | Matrix homeserver | 8008 |
| **postgres** | Database for Synapse and bridges | 5432 |
| **mautrix-whatsapp** | WhatsApp bridge | 29318 |
| **mautrix-signal** | Signal bridge | 29328 |
| **mautrix-messenger** | Messenger bridge | 29320 |
| **mautrix-instagram** | Instagram bridge | 29319 |
| **signald** | Signal daemon (required for Signal bridge) | - |

**Note**: Frontend (Yew/WASM) runs separately via `trunk serve` on port 8080 for Phase 1.

## Prerequisites

### Required Software

```bash
# Docker & Docker Compose
# Install from: https://docs.docker.com/get-docker/

# Just command runner (optional but recommended)
brew install just  # macOS
# OR
cargo install just  # via Rust
# OR use docker compose commands directly

# For generating secrets
openssl
```

### Frontend Development (Optional)

If you want to run the frontend locally (recommended for development):

```bash
# Rust toolchain (if not already installed)
# Install from: https://rustup.rs/

# Trunk (Yew/WASM build tool)
cargo install trunk

# WebAssembly target
rustup target add wasm32-unknown-unknown
```

### System Requirements

- **Memory**: 8GB+ RAM for Docker (12GB recommended)
- **CPU**: 4+ cores recommended (more cores = faster parallel compilation)
- **Disk**: 5-10GB free space (including build caches)
- **Time**: 15-20 minutes first build (native), 30-40 min (multi-platform)

**Limited local resources?** Use [GitHub Codespaces](https://github.com/codespaces) (free tier: 4 cores, 8GB RAM).

**Cross-Platform Compatibility:**
- `just build-native`: Builds for your current architecture only (faster)
  - Linux/Intel Mac/Windows → linux/amd64 image
  - Apple Silicon Mac → linux/arm64 image
  - **Images are NOT interchangeable** between architectures
- `just build`: Builds for BOTH amd64 and arm64 (slower but works everywhere)
  - Use this if you need to run the same image on different CPU architectures
  - Requires `docker buildx` (included in Docker Desktop)

---

## Quick Start

### 1. Build and Start Services

```bash
cd lightfriend-cloud

# Build for current platform only (RECOMMENDED for local dev - fastest)
just build-native

# Start all services (auto-creates .env with secrets if missing)
just up
```

**That's it!** The `just up` command will:
1. **Auto-create `.env`** with generated secrets if it doesn't exist
2. Generate all configuration files (Synapse, bridges, registration)
3. Start all Docker services

**Optional**: Edit `.env` to add API keys for optional features:
- Twilio (SMS/Voice)
- Stripe (Payments)
- Google OAuth (Calendar/Tasks)
- OpenRouter/Perplexity (AI)

**Note**: Bridge tokens (as_token, hs_token) are **auto-generated** - no manual token management needed!

### 2. Build Options

| Command | Platforms | Build Time | Use Case |
|---------|-----------|------------|----------|
| `just build-native` | Current only | 15-20 min (first), 30-60s (changes) | ✅ **Local development** |
| `just build` | amd64 + arm64 | 30-40 min (first), 60-90s (changes) | Cross-platform distribution |
| `just build-fast` | Current only | 10-15 min (first), 20-40s (changes) | Quick testing (less optimized) |

**Build time improvements** (vs original 30min build):
- ✅ **sccache**: Caches compiled Rust code across builds (~30% faster rebuilds)
- ✅ **cargo-chef**: Caches dependencies separately from source code
- ✅ **BuildKit cache mounts**: Cargo registry cached between builds
- ✅ **mold linker**: 2-3x faster linking than default linker
- ✅ **Optimized Cargo profile**: Thin LTO + parallel codegen
- ✅ **Parallel compilation**: Uses all CPU cores automatically

**First build**: Expect 15-20 minutes for `build-native`, 30-40 minutes for multi-platform `build`
**Subsequent builds** (code changes only): 30-60 seconds with sccache cache hits

**Requirements**:
- Run `just` commands from project root
- Docker memory: 8GB+ (Docker Desktop → Settings → Resources)
- BuildKit enabled (default in modern Docker)
- For multi-platform builds: `docker buildx` installed (included in Docker Desktop)

### 3. Create Matrix Admin User

Once Synapse is running, create an admin user:

```bash
just create-admin adminuser YourSecurePassword123
```

This user can log into bridges and manage the Matrix homeserver.

### 4. Run Frontend (Separate Terminal)

For Phase 1, the frontend runs separately from Docker.

**First time setup** (if not already done):
```bash
cargo install trunk
rustup target add wasm32-unknown-unknown
```

**Start the frontend:**
```bash
cd frontend && trunk serve
```

This will start the frontend dev server on http://localhost:8080.
First build takes ~3 minutes; subsequent builds are fast with hot-reload.

### 5. Access the Application

- **Frontend**: http://localhost:8080 (trunk serve)
- **Backend API**: http://localhost:3000 (Docker)
- **Matrix Homeserver**: http://localhost:8008 (Docker)

---

## Configuration Details

### Auto-Generated Bridge Configuration

**Simplified Setup**: Bridge tokens are now automatically generated!

Previously required **12 environment variables**:
- 4 secrets (Matrix, Database, Double Puppet)
- 8 bridge tokens (as_token + hs_token for 4 bridges)

Now requires only **4 environment variables**:
- ✅ `MATRIX_HOMESERVER_SHARED_SECRET`
- ✅ `SYNAPSE_DB_PASSWORD`
- ✅ `POSTGRES_PASSWORD`
- ✅ `DOUBLE_PUPPET_SECRET`

**How it works**:
```bash
# When you run: just up
# The setup-configs.sh script:

1. Generates config.yaml for each bridge (from templates)
2. Runs each bridge with -g flag to auto-generate registration:
   docker run --rm \
     -v ./bridges/whatsapp:/data \
     dock.mau.dev/mautrix/whatsapp:latest \
     /usr/bin/mautrix-whatsapp -g \
     -c /data/config.yaml \
     -r /data/whatsapp-registration.yaml

3. Bridge creates random tokens and writes registration.yaml
4. Synapse loads registration files on startup
```

**What gets generated**:
- `docker/bridges/*/config.yaml` - Bridge configuration (deterministic from templates)
- `docker/bridges/*/*-registration.yaml` - Auto-generated with random tokens
- `docker/synapse/homeserver.yaml` - Synapse configuration (from template)

All generated files are git-ignored and can be regenerated anytime with `cd docker && ./setup-configs.sh`.

### Database Initialization

PostgreSQL automatically creates all required databases on first startup:
- `synapse_db` - Synapse homeserver
- `whatsapp_db` - WhatsApp bridge
- `signal_db` - Signal bridge
- `messenger_db` - Messenger bridge
- `instagram_db` - Instagram bridge

The initialization script is in `docker/postgres-init/init-databases.sh`.

### Data Persistence

All data is stored in Docker volumes:
- `postgres_data` - All PostgreSQL databases
- `synapse_data` - Synapse configuration and media
- `signald_data` - Signal daemon data
- `whatsapp_data` - WhatsApp bridge data
- `signal_data` - Signal bridge data
- `messenger_data` - Messenger bridge data
- `instagram_data` - Instagram bridge data
- `core_data` - Lightfriend SQLite database
- `core_uploads` - User uploads
- `core_matrix_store` - Matrix client state

Data persists across container restarts. To completely reset:

```bash
just down-volumes  # WARNING: Deletes all data!
```

### Backup Strategy

**Important**: Always back up your data before major changes, upgrades, or migrations.

#### What to Backup

**Critical volumes** (contains all your data):
- `postgres_data` - All databases (Synapse + bridges)
- `synapse_data` - Synapse signing keys and media
- `core_data` - Lightfriend SQLite database
- `core_uploads` - User uploads
- `signald_data` - Signal device registration (needed to avoid re-linking)

**Optional volumes** (can be regenerated):
- `whatsapp_data`, `signal_data`, `messenger_data`, `instagram_data` - Bridge session data
- `core_matrix_store` - Matrix client cache (regenerates on restart)

#### Backup Commands

**Backup all critical volumes:**

```bash
# Create backup directory
mkdir -p ../backups/$(date +%Y%m%d)

# Backup postgres (all databases)
docker run --rm \
  -v lightfriend_postgres_data:/data \
  -v $(pwd)/../backups/$(date +%Y%m%d):/backup \
  alpine tar czf /backup/postgres_data.tar.gz /data

# Backup Synapse
docker run --rm \
  -v lightfriend_synapse_data:/data \
  -v $(pwd)/../backups/$(date +%Y%m%d):/backup \
  alpine tar czf /backup/synapse_data.tar.gz /data

# Backup core data
docker run --rm \
  -v lightfriend_core_data:/data \
  -v $(pwd)/../backups/$(date +%Y%m%d):/backup \
  alpine tar czf /backup/core_data.tar.gz /data

# Backup core uploads
docker run --rm \
  -v lightfriend_core_uploads:/data \
  -v $(pwd)/../backups/$(date +%Y%m%d):/backup \
  alpine tar czf /backup/core_uploads.tar.gz /data

# Backup signald (Signal device registration)
docker run --rm \
  -v lightfriend_signald_data:/data \
  -v $(pwd)/../backups/$(date +%Y%m%d):/backup \
  alpine tar czf /backup/signald_data.tar.gz /data
```

**Automated backup script:**

Add this to `docker/backup.sh`:

```bash
#!/bin/bash
# Automated backup script for Lightfriend Docker volumes

BACKUP_DIR="../backups/$(date +%Y%m%d-%H%M%S)"
mkdir -p "$BACKUP_DIR"

echo "Creating backups in $BACKUP_DIR..."

volumes=(
  "postgres_data"
  "synapse_data"
  "core_data"
  "core_uploads"
  "signald_data"
)

for volume in "${volumes[@]}"; do
  echo "Backing up lightfriend_${volume}..."
  docker run --rm \
    -v "lightfriend_${volume}:/data" \
    -v "$(pwd)/${BACKUP_DIR}:/backup" \
    alpine tar czf "/backup/${volume}.tar.gz" /data

  if [ $? -eq 0 ]; then
    echo "✓ Backed up ${volume}"
  else
    echo "✗ Failed to backup ${volume}"
  fi
done

echo ""
echo "Backup complete! Files saved to: $BACKUP_DIR"
echo "Total size: $(du -sh $BACKUP_DIR | cut -f1)"
```

Make it executable: `chmod +x docker/backup.sh`

#### Restore Commands

**To restore from backup:**

```bash
# Stop services first
just down

# Restore postgres
docker run --rm \
  -v lightfriend_postgres_data:/data \
  -v $(pwd)/../backups/20250128:/backup \
  alpine sh -c "cd / && tar xzf /backup/postgres_data.tar.gz"

# Restore Synapse
docker run --rm \
  -v lightfriend_synapse_data:/data \
  -v $(pwd)/../backups/20250128:/backup \
  alpine sh -c "cd / && tar xzf /backup/synapse_data.tar.gz"

# Restore core data
docker run --rm \
  -v lightfriend_core_data:/data \
  -v $(pwd)/../backups/20250128:/backup \
  alpine sh -c "cd / && tar xzf /backup/core_data.tar.gz"

# Restore core uploads
docker run --rm \
  -v lightfriend_core_uploads:/data \
  -v $(pwd)/../backups/20250128:/backup \
  alpine sh -c "cd / && tar xzf /backup/core_uploads.tar.gz"

# Restore signald
docker run --rm \
  -v lightfriend_signald_data:/data \
  -v $(pwd)/../backups/20250128:/backup \
  alpine sh -c "cd / && tar xzf /backup/signald_data.tar.gz"

# Start services
just up
```

#### Backup Best Practices

1. **Schedule regular backups**: Use cron to run backups daily
   ```bash
   # Example crontab entry (runs at 2 AM daily)
   0 2 * * * cd /path/to/lightfriend-cloud/docker && ./backup.sh >> backup.log 2>&1
   ```

2. **Keep multiple versions**: Don't overwrite old backups immediately
   - Daily backups: Keep last 7 days
   - Weekly backups: Keep last 4 weeks
   - Monthly backups: Keep last 12 months

3. **Test restores regularly**: Verify backups work before you need them

4. **Store off-site**: Copy backups to S3, external drive, or another server
   ```bash
   # Example: Upload to S3
   aws s3 sync ../backups s3://your-bucket/lightfriend-backups
   ```

5. **Backup before upgrades**: Always backup before updating Docker images
   ```bash
   ./backup.sh && just down && docker compose pull && just up
   ```

6. **Monitor backup size**: Large backups indicate media growth
   ```bash
   du -sh ../backups/*
   ```

---

## Common Commands

```bash
# Build Commands
just build-native   # Build for current platform (RECOMMENDED - fastest)
just build          # Build multi-platform (amd64 + arm64)
just build-fast     # Fast build with fewer optimizations
just build-core     # Build only core service
just build-push     # Build & push multi-platform to registry

# Lifecycle
just up             # Start all services
just down           # Stop all services
just restart        # Restart all services

# Logs
just logs           # View all logs
just logs-core      # View core logs
just logs-synapse   # View synapse logs
just logs-bridge whatsapp  # View bridge logs

# Status
just status         # Check service status

# Admin
just create-admin <username> <password>  # Create Matrix admin

# Maintenance
just clean          # Clean up Docker resources
just rebuild        # Full rebuild
```

**Don't have `just` installed?** Install it: `brew install just` or `cargo install just`

---

## Connecting Bridges

### 1. Log into Matrix

Open https://app.element.io and:
1. Click "Sign In"
2. Click "Edit" next to homeserver
3. Enter: `http://localhost:8008`
4. Sign in with your admin user

### 2. Connect to Bridges

Start a DM with each bridge bot:

- **WhatsApp**: `@whatsappbot:matrix.local`
- **Signal**: `@signalbot:matrix.local`
- **Messenger**: `@messengerbot:matrix.local`
- **Instagram**: `@igbot:matrix.local`

Send `help` to see available commands, then `login` to connect.

### Bridge-Specific Notes

**Signal**: Requires QR code scanning. Send `login` and scan the QR code with Signal on your phone.

**Messenger/Instagram**: 
- May ban datacenter IPs
- Consider using residential proxy
- Both use the mautrix-meta bridge with different modes

---

## Troubleshooting

### Check Service Health

```bash
just status

# Or
cd docker && docker compose ps
```

All services should show "Up" or "Up (healthy)".

### View Logs

```bash
just logs           # All services
just logs-core      # Core service
just logs-synapse   # Synapse
just logs-bridge whatsapp  # Specific bridge
```

### Common Issues

**Build takes longer than expected**
- Expected: 15-20 min first build (native), 30-40 min (multi-platform)
- Subsequent builds: 30-60s with sccache cache hits
- Using sufficient resources? (4+ CPU cores, 8GB+ RAM recommended)
- Check BuildKit is enabled: `docker buildx version`
- Use `just build-native` for faster local development builds
- Use `just build-fast` for even faster builds (less optimized binary)

**Build fails with "Killed" (exit code 137)**
- Docker out of memory
- Fix: Docker Desktop → Settings → Resources → Memory → Set to 8GB+
- Or use GitHub Codespaces instead

**"cd: docker: No such file or directory"**
- Run `just` commands from project root, not from inside `docker/` directory

**"Connection refused" to backend**
- Run `just status` to check if core is running
- Run `just logs-core` to see errors

**Bridge not connecting**
- Run `just logs-bridge <name>` to see bridge errors
- Ensure you created Matrix admin: `just create-admin <user> <pass>`

**Out of disk space**
- Run `just clean` to clean up Docker resources

### Reset Everything

```bash
just rebuild        # Stop, clean, rebuild, start
just down-volumes   # WARNING: Deletes all data!
```

---

## Development

```bash
# After changing Rust code
just build-core && just restart-core

# Run locally without Docker
just dev-backend    # Terminal 1
just dev-frontend   # Terminal 2

# Tests
just test-backend
just test-frontend
```

---

## Architecture Notes

### Phase 1 (Current)

This Docker setup is Phase 1 toward Nitro Enclave deployment:

- **Separate containers**: Core (backend only), Synapse, and each bridge run independently
- **Frontend runs separately**: Yew/WASM frontend via `trunk serve` (not containerized yet)
- **SQLite in core**: Main app database is SQLite (bridges use Postgres)
- **No encryption at rest**: Data is stored unencrypted in volumes
- **Standard Docker**: Uses regular Docker Compose orchestration
- **Official bridge images**: Using mautrix's published images from dock.mau.dev

### Future (Nitro Enclave)

The design allows migration to:

- **Core in enclave**: Backend runs in AWS Nitro Enclave with encryption keys
- **Bridges in enclave**: Message bridges move inside enclave for E2E privacy
- **Encrypted SQLite snapshots**: Database encrypted at rest, loaded into enclave
- **Matrix E2E encryption**: Synapse only sees encrypted message blobs
- **VSOCK communication**: Enclave communicates via VSOCK instead of TCP

Current architecture makes these migrations possible without major rewrites.

---

## Security Considerations

### Development vs Production

This setup is configured for **local development**:

- Uses `matrix.local` as server name (not routable)
- Exposes all ports on localhost
- No TLS/HTTPS configured
- No reverse proxy
- Registration enabled without verification

**For production**, you need:

1. **Domain name** and DNS
2. **TLS certificates** (Let's Encrypt)
3. **Reverse proxy** (Nginx/Caddy)
4. **Federation setup** (if connecting to matrix.org)
5. **Disable open registration**
6. **Firewall rules**
7. **Backup strategy**

### Secrets Management

- Never commit `.env` to git
- Rotate secrets regularly
- Use strong, random secrets (32+ bytes)
- Consider using Docker secrets or external secret managers for production

---

## Support

For issues or questions:

1. Check logs: `just logs` or `just logs-<service>`
2. Review [mautrix docs](https://docs.mau.fi/)
3. Check [Synapse docs](https://matrix-org.github.io/synapse/)
4. Open an issue on GitHub

---

## License

This project is licensed under GNU AGPLv3. The name "Lightfriend" and branding are owned by Rasmus Ähtävä and not included in the license.
