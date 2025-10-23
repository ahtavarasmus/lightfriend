#!/bin/bash
set -e

echo "🚀 Starting Lightfriend backend initialization..."

# Ensure data directory exists
mkdir -p /app/data

# Path to secrets file
SECRETS_FILE="/app/data/.env.secrets"
DATABASE_PATH="/app/data/database.db"

# Function to generate random secret
generate_secret() {
    openssl rand -hex 32
}

# Generate secrets on first run
if [ ! -f "$SECRETS_FILE" ]; then
    echo "🔐 First run detected! Generating secure secrets..."

    JWT_SECRET=$(generate_secret)
    JWT_REFRESH=$(generate_secret)
    ENCRYPTION_KEY=$(generate_secret)
    MATRIX_SECRET=$(generate_secret)

    cat > "$SECRETS_FILE" << EOF
# Auto-generated secrets - DO NOT SHARE OR COMMIT
# Generated on: $(date -u +"%Y-%m-%d %H:%M:%S UTC")
JWT_SECRET_KEY=$JWT_SECRET
JWT_REFRESH_KEY=$JWT_REFRESH
ENCRYPTION_KEY=$ENCRYPTION_KEY
MATRIX_SHARED_SECRET=$MATRIX_SECRET
EOF

    chmod 600 "$SECRETS_FILE"
    echo "✅ Secrets generated and saved to $SECRETS_FILE"
else
    echo "🔑 Loading existing secrets from $SECRETS_FILE"
fi

# Load secrets into environment
set -a
source "$SECRETS_FILE"
set +a

# Set required environment variables with sensible defaults
export DATABASE_URL="$DATABASE_PATH"
export ENVIRONMENT="${ENVIRONMENT:-staging}"
export FRONTEND_URL="${COOLIFY_URL:-http://localhost}"
export SERVER_URL="${COOLIFY_URL:-http://localhost}"
export SERVER_URL_OAUTH="${COOLIFY_URL:-http://localhost}"
export MATRIX_HOMESERVER="${MATRIX_HOMESERVER:-http://localhost:8008}"
export MATRIX_HOMESERVER_PERSISTENT_STORE_PATH="/app/data/matrix_store"

echo "📊 Database: $DATABASE_URL"
echo "🌐 Server URL: $SERVER_URL"
echo "🔗 Matrix Homeserver: $MATRIX_HOMESERVER"

# Run database migrations
echo "🔄 Running database migrations..."
cd /app

# Check if database exists, if not create it
if [ ! -f "$DATABASE_PATH" ]; then
    echo "📝 Creating new database..."
fi

diesel migration run --database-url="$DATABASE_URL" || {
    echo "⚠️  Migration warning (database might already be up to date)"
}

echo "✅ Initialization complete! Starting backend server..."
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

# Start the backend service
exec backend
