# Lightfriend Deployment Guide

## One-Click Coolify Deployment

This repository is configured for zero-configuration deployment on Coolify. Just paste the GitHub URL and deploy!

### What Happens Automatically

1. **Docker Build**: Coolify builds the frontend (Yew/WASM) and backend (Rust/Axum)
2. **Database Setup**: SQLite database is created in persistent volume on first run
3. **Migrations**: Database schema is automatically migrated
4. **Secret Generation**: All security secrets (JWT, encryption keys, Matrix shared secret) are auto-generated and persisted
5. **Service Start**: Nginx serves the frontend and proxies API requests to the backend

### Prerequisites

- Coolify instance running on Ubuntu server
- Docker and Docker Compose installed (handled by Coolify)
- Minimum 2GB RAM recommended for build process
- Persistent storage for database

### Deployment Steps

1. **In Coolify Dashboard**:
   - Click "New Resource" → "Public Repository"
   - Paste GitHub URL: `https://github.com/ahtavarasmus/lightfriend`
   - Select branch: `master`
   - Build method: Docker Compose
   - Click "Deploy"

2. **Wait for Build**:
   - Frontend build: ~5-10 minutes (compiles Rust to WASM)
   - Backend build: ~10-15 minutes (compiles Rust with all dependencies)
   - First build will be slower; subsequent builds use Docker cache

3. **Access Your Instance**:
   - Coolify will assign a URL (e.g., `https://lightfriend.your-domain.com`)
   - The app will be running on port 80
   - All API routes are proxied through `/api/`

### Persistent Data

The following data persists across container restarts and updates:

- **`/app/data/database.db`**: SQLite database with all user data
- **`/app/data/.env.secrets`**: Auto-generated security secrets
- **`/app/data/matrix_store`**: Matrix client persistent storage (for messaging bridges)
- **`/app/uploads`**: User-uploaded files (QR codes, images, etc.)

These are stored in Docker volumes managed by Coolify.

### Environment Variables

**No manual environment variables are required!** Everything is auto-configured.

However, you can optionally override these in Coolify:
- `COOLIFY_URL`: Automatically set by Coolify (your public URL)
- `PORT`: Default is 80, Coolify may override this

### Architecture

```
┌─────────────────────────────────────────┐
│           Nginx (Port 80)               │
│  - Serves frontend static files         │
│  - Proxies /api/* to backend            │
└─────────────────────────────────────────┘
            │                │
            ▼                ▼
   ┌─────────────┐   ┌─────────────────┐
   │  Frontend   │   │   Backend       │
   │  (Yew WASM) │   │  (Axum/Rust)    │
   │  Static     │   │  Port 3100      │
   └─────────────┘   └─────────────────┘
                              │
                              ▼
                     ┌─────────────────┐
                     │  SQLite DB      │
                     │  /app/data/     │
                     └─────────────────┘
```

### First Run

On the first container start, `init.sh` automatically:

1. Creates `/app/data` directory
2. Generates random secrets:
   - `JWT_SECRET_KEY` (64 hex chars)
   - `JWT_REFRESH_KEY` (64 hex chars)
   - `ENCRYPTION_KEY` (64 hex chars)
   - `MATRIX_SHARED_SECRET` (64 hex chars)
3. Saves secrets to `/app/data/.env.secrets` with 600 permissions
4. Runs Diesel database migrations
5. Starts the backend server on port 3100

### Updating the App

To update your deployment:

1. Push changes to GitHub
2. In Coolify, click "Redeploy" on your service
3. Coolify will rebuild the containers
4. Database and secrets persist automatically
5. Migrations run automatically on startup

### Health Checks

The deployment includes health checks:
- Backend: `http://localhost:3100/api/health`
- Nginx: `http://localhost/api/health`

Coolify uses these to ensure services are running correctly.

### Troubleshooting

#### Build Fails with "Out of Memory"
- Increase server RAM to at least 2GB
- Or enable swap space on your server
- The build uses serial compilation to reduce memory usage

#### Container Restarts Continuously
- Check Coolify logs: `docker logs <container_name>`
- Ensure `/app/data` volume is writable
- Verify database migrations completed successfully

#### Can't Access the App
- Check Coolify URL configuration
- Ensure port 80 is exposed
- Check Nginx logs: `docker exec <nginx_container> cat /var/log/nginx/error.log`

#### Database Issues
- Database file: `/app/data/database.db`
- Check permissions: `docker exec <backend_container> ls -la /app/data`
- To reset: stop containers, delete volume, restart (WARNING: loses all data)

### Manual Database Access

To access the database directly:

```bash
# Get backend container name
docker ps | grep backend

# Access database
docker exec -it <backend_container> sqlite3 /app/data/database.db

# Run SQL queries
sqlite> .tables
sqlite> SELECT * FROM users;
sqlite> .quit
```

### Backup

To backup your data:

```bash
# Backup database
docker exec <backend_container> cat /app/data/database.db > backup-$(date +%Y%m%d).db

# Backup secrets
docker exec <backend_container> cat /app/data/.env.secrets > backup-secrets.env
```

Store these backups securely - they contain sensitive data!

### Security Notes

- Secrets are auto-generated with cryptographically secure randomness (OpenSSL)
- Secrets file has 600 permissions (owner read/write only)
- Secrets persist in Docker volume (not in Git)
- HTTPS is handled by Coolify's reverse proxy
- Database encryption is done at the application level using AES-GCM

### Future Enhancements

This deployment is designed to be extensible:
- **Matrix Homeserver**: Can be added for self-hosted messaging
- **Mautrix Bridges**: WhatsApp, Telegram, Signal, Instagram bridges ready to add
- **Custom Domains**: Configure in Coolify settings
- **SSL Certificates**: Automatic via Coolify + Let's Encrypt

See `notes-for-matrix-bridges.md` for Matrix setup instructions (when Matrix services are added).

### Support

- GitHub Issues: https://github.com/ahtavarasmus/lightfriend/issues
- License: AGPLv3 (see LICENSE file)
- Branding: "Lightfriend" name and branding are proprietary
