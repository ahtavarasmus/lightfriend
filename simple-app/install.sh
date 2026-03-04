#!/bin/bash
# Lightfriend - Yhden komennon asennus palvelimelle
# Käyttö: curl -sSL https://raw.githubusercontent.com/ahtavarasmus/lightfriend/main/simple-app/install.sh | bash

set -e

echo ""
echo "========================================"
echo "  Lightfriend - Palvelinasennus"
echo "========================================"
echo ""

# 1. Asenna Docker jos puuttuu
if ! command -v docker &> /dev/null; then
    echo "[1/4] Asennetaan Docker..."
    curl -fsSL https://get.docker.com | sh
    systemctl enable docker
    systemctl start docker
    echo "  Docker asennettu."
else
    echo "[1/4] Docker on jo asennettu."
fi

# 2. Luo kansio
echo "[2/4] Luodaan tiedostot..."
mkdir -p /opt/lightfriend
cd /opt/lightfriend

# Generoi avaimet
JWT_SECRET=$(openssl rand -base64 32)
JWT_REFRESH=$(openssl rand -base64 32)
ENC_KEY=$(openssl rand -base64 32)
MATRIX_REG=$(openssl rand -hex 32)
MATRIX_SHARED=$(openssl rand -hex 32)

# docker-compose.yml
cat > docker-compose.yml << 'COMPOSE'
services:
  tuwunel:
    image: jevolk/tuwunel:latest
    container_name: lightfriend-tuwunel
    environment:
      TUWUNEL_CONFIG: /etc/tuwunel/tuwunel.toml
    volumes:
      - tuwunel_data:/var/lib/tuwunel
      - ./tuwunel.toml:/etc/tuwunel/tuwunel.toml:ro
    networks:
      - lightfriend
    restart: unless-stopped

  core:
    image: ahtavarasmus/lightfriend-core:latest
    container_name: lightfriend-core
    env_file:
      - .env
    environment:
      DATABASE_URL: /app/data/database.db
      MATRIX_HOMESERVER: http://tuwunel:8008
      MATRIX_HOMESERVER_PERSISTENT_STORE_PATH: /app/matrix_store
      FRONTEND_URL: http://localhost:3000
      PORT: 3000
      ENVIRONMENT: development
    volumes:
      - core_data:/app/data
      - core_uploads:/app/uploads
      - core_matrix_store:/app/matrix_store
    ports:
      - "3000:3000"
    networks:
      - lightfriend
    depends_on:
      tuwunel:
        condition: service_started
    restart: unless-stopped

networks:
  lightfriend:
    driver: bridge

volumes:
  tuwunel_data:
  core_data:
  core_uploads:
  core_matrix_store:
COMPOSE

# tuwunel.toml
cat > tuwunel.toml << EOF
[global]
server_name = "localhost"
database_path = "/var/lib/tuwunel"
address = "0.0.0.0"
port = 8008
allow_registration = true
registration_token = "${MATRIX_REG}"
grant_admin_to_first_user = true
allow_federation = false
EOF

# .env
cat > .env << EOF
JWT_SECRET_KEY=${JWT_SECRET}
JWT_REFRESH_KEY=${JWT_REFRESH}
ENCRYPTION_KEY=${ENC_KEY}
MATRIX_SHARED_SECRET=${MATRIX_SHARED}
MATRIX_REGISTRATION_TOKEN=${MATRIX_REG}
OPENROUTER_API_KEY=
TINFOIL_API_KEY=
PERPLEXITY_API_KEY=
ADMIN_EMAILS=admin@lightfriend.local
BOOTSTRAP_ADMIN_PASSWORD=12345678
BOOTSTRAP_ADMIN_PHONE=+12345678
TWILIO_ACCOUNT_SID=
TWILIO_AUTH_TOKEN=
TWILIO_PHONE_NUMBER=
STRIPE_SECRET_KEY=
STRIPE_PUBLISHABLE_KEY=
STRIPE_WEBHOOK_SECRET=
STRIPE_CREDITS_PRODUCT_ID=
STRIPE_SUBSCRIPTION_WORLD_PRICE_ID=
STRIPE_SUBSCRIPTION_HOSTED_PLAN_PRICE_ID_US=
STRIPE_SUBSCRIPTION_SELF_HOSTING_PRICE_ID=
STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_FI=
STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_NL=
STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_UK=
STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_AU=
STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_OTHER=
STRIPE_ASSISTANT_PLAN_PRICE_ID=
STRIPE_ASSISTANT_PLAN_PRICE_ID_US=
STRIPE_AUTOPILOT_PLAN_PRICE_ID=
STRIPE_AUTOPILOT_PLAN_PRICE_ID_US=
STRIPE_BYOT_PLAN_PRICE_ID=
ELEVENLABS_SERVER_URL_SECRET=
ELEVENLABS_WEBHOOK_SECRET=
RESEND_API_KEY=
RESEND_FROM_EMAIL=
SENTRY_DSN=
EOF

# 3. Käynnistä
echo "[3/4] Ladataan ja käynnistetään..."
docker compose pull
docker compose up -d

# 4. Hae palvelimen IP
SERVER_IP=$(curl -s ifconfig.me 2>/dev/null || curl -s icanhazip.com 2>/dev/null || echo "PALVELIMEN-IP")

# Avaa palomuuri
if command -v ufw &> /dev/null; then
    ufw allow 3000/tcp 2>/dev/null || true
fi

echo ""
echo "========================================"
echo "  VALMIS!"
echo "========================================"
echo ""
echo "  Avaa iPhonella: http://${SERVER_IP}:3000"
echo ""
echo "  Kirjaudu:"
echo "    Sahkoposti: admin@lightfriend.local"
echo "    Salasana:   12345678"
echo ""
echo "  TARKEA: Lisaa AI API-avain:"
echo "    nano /opt/lightfriend/.env"
echo "    -> Lisaa OPENROUTER_API_KEY=sk-or-..."
echo "    -> Tallenna: Ctrl+X, Y, Enter"
echo "    docker compose restart core"
echo ""
echo "  Vinkki: Lisaa Safarissa 'Lisaa alkunaytolle'"
echo "  niin toimii kuin appi!"
echo ""
