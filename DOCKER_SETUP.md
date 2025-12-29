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

### System Requirements

- **Disk Space**: ~5GB for images + data volumes
- **Memory**: 6-8GB RAM for Docker (recommended: set Docker Desktop to 6GB)
- **CPU**: 2+ cores recommended
- **Rust toolchain**: Required for default build method (install via [rustup.rs](https://rustup.rs))

**Note**: The default build method compiles Rust on your local machine (using full system RAM + swap) and packages the binary in Docker. This avoids OOM errors on memory-constrained systems like Mac M1 with 8GB RAM.

---

## Quick Start

### 1. Setup Environment Variables

```bash
cd lightfriend-cloud

# Copy example environment file
cp .env.example .env

# Generate secrets and append to .env
just generate-secrets >> .env
```

Edit `.env` and verify the generated values:
- **Required secrets** (4 total): Already added by `generate-secrets` ✓
  - `MATRIX_HOMESERVER_SHARED_SECRET`
  - `SYNAPSE_DB_PASSWORD`
  - `POSTGRES_PASSWORD`
  - `DOUBLE_PUPPET_SECRET`
- **Optional API keys**: Add your Twilio, Stripe, Google, OpenRouter keys (or leave blank for now)

**Note**: Bridge tokens (as_token, hs_token) are now **auto-generated**! You no longer need to manually create 8 bridge tokens. The setup script uses each bridge's `-g` flag to generate random tokens automatically.

That's it! The configuration files (Synapse, bridges, registration) will be automatically generated from your `.env` when you start the services.

### 2. Build and Start Services

```bash
# Build backend locally, then create Docker images (recommended)
just build-prebuilt

# Start all services in background
just up

# OR start with logs visible
just up-logs
```

First startup will take 10-15 minutes as it:
- Compiles Rust backend on your local machine (single-threaded, memory-optimized)
- Creates Docker image with pre-built binary
- Downloads other base images (~2GB)
- Initializes databases
- Generates Synapse signing keys

**Why prebuilt?** Building Rust inside Docker can cause OOM errors on systems with 8GB RAM. The prebuilt approach compiles locally (using full system memory + swap) and Docker only packages the binary.

**Alternative (for 16GB+ RAM systems):** If you have plenty of RAM, you can build entirely in Docker with `just build` instead. This may take longer but keeps everything containerized.

### 3. Create Matrix Admin User

Once Synapse is running, create an admin user:

```bash
just create-admin adminuser YourSecurePassword123
```

This user can log into bridges and manage the Matrix homeserver.

### 4. Run Frontend (Separate Terminal)

For Phase 1, the frontend runs separately from Docker:

```bash
cd frontend && trunk serve
```

This will start the frontend dev server on http://localhost:8080.

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

## Common Commands (with Just)

```bash
# Build (recommended: prebuilt method)
just build-prebuilt # Build backend locally + create Docker image (default method)
just build          # Alternative: Build all images in Docker (requires 16GB+ RAM)
just build-core     # Alternative: Build only core in Docker (requires 16GB+ RAM)

# Lifecycle
just up             # Start all services
just down           # Stop all services
just restart        # Restart all services
just restart-core   # Restart only core

# Logs
just logs           # View all logs
just logs-core      # View core logs
just logs-synapse   # View synapse logs
just logs-bridge whatsapp  # View specific bridge logs

# Status
just status         # Check service status

# Admin
just create-admin <username> <password>  # Create Matrix admin
just shell-core     # Enter core container
just shell-synapse  # Enter synapse container
just shell-postgres # Enter postgres container

# Maintenance
just clean          # Clean up Docker resources
just rebuild        # Full rebuild (stop, clean, build, start)
```

### Without Just

If you don't have `just` installed:

```bash
# Recommended: Prebuilt method
cd backend && cargo build --release && cd ..
cd docker && docker build -f core/Dockerfile.prebuilt -t lightfriend-core .. && cd ..

# Alternative: Build in Docker (requires 16GB+ RAM)
cd docker
docker compose build

# Lifecycle
docker compose up -d
docker compose down
docker compose restart
docker compose restart core

# Logs
docker compose logs -f
docker compose logs -f core
docker compose logs -f mautrix-whatsapp

# Status
docker compose ps

# Admin
docker compose exec synapse register_new_matrix_user -c /data/homeserver.yaml -u admin -p password --admin

# Shell
docker compose exec core /bin/bash
docker compose exec synapse /bin/bash
docker compose exec postgres psql -U postgres
```

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
# All services
just logs

# Specific service
just logs-core
just logs-synapse
just logs-bridge whatsapp
```

### Common Issues

**"Connection refused" to backend**
- Check core is running: `just status`
- Check logs: `just logs-core`
- Ensure .env has all required variables

**Bridge not connecting**
- Check bridge logs: `just logs-bridge <name>`
- Verify bridge config has correct tokens
- Ensure Synapse can reach bridge (check network)

**Signal bridge failing**
- Check signald is running: `just status`
- Check signald logs: `cd docker && docker compose logs signald`
- Ensure signal bridge can access signald socket

**Database connection errors**
- Check postgres is running: `just status`
- Check postgres logs: `cd docker && docker compose logs postgres`
- Verify database credentials in configs match postgres-init script

**Out of disk space**
- Check usage: `just disk-usage`
- Clean up: `just clean`
- Remove old volumes: `cd docker && docker volume prune`

### Reset Everything

If something is seriously broken:

```bash
# Stop everything
just down

# Remove all volumes (WARNING: deletes all data!)
just down-volumes

# Clean Docker cache
just clean

# Rebuild and start
just rebuild
```

---

## Development

### Running Locally (without Docker)

You can still run backend and frontend locally for development:

```bash
# Terminal 1: Backend
just dev-backend

# Terminal 2: Frontend  
just dev-frontend
```

This requires local setup as documented in `MATRIX_SETUP_GUIDE.md`.

### Running Tests

```bash
just test-backend
just test-frontend
```

### Modifying Docker Images

After changing Rust code:

```bash
# Rebuild backend and recreate Docker image
just build-prebuilt

# Restart the service
just restart-core
```

After changing configs:

```bash
# Just restart the service
cd docker && docker compose restart <service-name>
```

### Build Methods Comparison

| Method | RAM Required | Build Time | Use Case |
|--------|--------------|------------|----------|
| **`just build-prebuilt`** (default) | 6GB Docker + system swap | 10-15 min | Mac M1 8GB, memory-constrained systems |
| `just build` (alternative) | 16GB+ Docker memory | 15-20 min | High-memory systems, full containerization |

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
