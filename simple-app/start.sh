#!/bin/bash
# Lightfriend Simple App - Käynnistysskripti
# Generoi avaimet automaattisesti ja käynnistää sovelluksen

set -e

echo "============================================"
echo "  Lightfriend - Yksinkertainen asennus"
echo "============================================"
echo ""

# Tarkista että Docker on asennettu
if ! command -v docker &> /dev/null; then
    echo "VIRHE: Docker ei ole asennettu!"
    echo ""
    echo "Asenna Docker:"
    echo "  Mac:   https://docs.docker.com/desktop/install/mac-install/"
    echo "  Win:   https://docs.docker.com/desktop/install/windows-install/"
    echo "  Linux: https://docs.docker.com/engine/install/"
    exit 1
fi

# Tarkista että docker compose toimii
if ! docker compose version &> /dev/null; then
    echo "VIRHE: 'docker compose' ei toimi!"
    echo "Päivitä Docker uusimpaan versioon."
    exit 1
fi

# Luo .env jos sitä ei ole
if [ ! -f .env ]; then
    echo "Luodaan asetustiedosto (.env)..."
    echo ""

    # Generoi salaiset avaimet automaattisesti
    JWT_SECRET=$(openssl rand -base64 32)
    JWT_REFRESH=$(openssl rand -base64 32)
    ENC_KEY=$(openssl rand -base64 32)
    MATRIX_REG_TOKEN=$(openssl rand -hex 32)
    MATRIX_SHARED=$(openssl rand -hex 32)

    # Kysy AI API-avain
    echo "Tarvitset AI API-avaimen. Vaihtoehdot:"
    echo "  1) OpenRouter  - https://openrouter.ai/keys"
    echo "  2) Tinfoil     - https://tinfoil.sh"
    echo "  3) Perplexity  - https://perplexity.ai"
    echo ""
    read -p "Syötä AI API-avain (tai paina Enter ohittaaksesi): " AI_KEY
    echo ""

    # Kysy admin-sähköposti
    read -p "Syötä admin-sähköpostiosoitteesi: " ADMIN_EMAIL
    if [ -z "$ADMIN_EMAIL" ]; then
        ADMIN_EMAIL="admin@localhost"
    fi

    # Kysy admin-salasana
    read -p "Valitse admin-salasana (oletus: 12345678): " ADMIN_PASS
    if [ -z "$ADMIN_PASS" ]; then
        ADMIN_PASS="12345678"
    fi

    cat > .env << EOF
# Lightfriend - Automaattisesti generoitu
# Luotu: $(date)

# JWT-avaimet (generoitu automaattisesti)
JWT_SECRET_KEY=${JWT_SECRET}
JWT_REFRESH_KEY=${JWT_REFRESH}

# Salausavain (generoitu automaattisesti)
ENCRYPTION_KEY=${ENC_KEY}

# Matrix-asetukset (generoitu automaattisesti)
MATRIX_SHARED_SECRET=${MATRIX_SHARED}
MATRIX_REGISTRATION_TOKEN=${MATRIX_REG_TOKEN}

# AI API-avaimet (lisää ainakin yksi)
OPENROUTER_API_KEY=${AI_KEY}
TINFOIL_API_KEY=
PERPLEXITY_API_KEY=

# Admin-asetukset
ADMIN_EMAILS=${ADMIN_EMAIL}
BOOTSTRAP_ADMIN_PASSWORD=${ADMIN_PASS}
BOOTSTRAP_ADMIN_PHONE=+12345678

# Tyhjät valinnaiset asetukset (ei tarvita peruskäyttöön)
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

    echo "Asetustiedosto luotu!"
    echo ""
else
    echo "Asetustiedosto (.env) löytyy jo."
    echo ""
fi

# Luo Tuwunel-konfiguraatio jos sitä ei ole
if [ ! -f tuwunel.toml ]; then
    # Lue Matrix-tokeni .env-tiedostosta
    MATRIX_REG_TOKEN=$(grep "^MATRIX_REGISTRATION_TOKEN=" .env | cut -d= -f2)

    cat > tuwunel.toml << EOF
[global]
server_name = "localhost"
database_path = "/var/lib/tuwunel"
address = "0.0.0.0"
port = 8008

# Rekisteröinti tokenilla
allow_registration = true
registration_token = "${MATRIX_REG_TOKEN}"

# Ensimmäinen käyttäjä saa admin-oikeudet
grant_admin_to_first_user = true

# Ei federointia (yksityinen palvelin)
allow_federation = false
EOF

    echo "Tuwunel-konfiguraatio luotu."
fi

echo ""
echo "Käynnistetään Lightfriend..."
echo ""
docker compose pull
docker compose up -d

echo ""
echo "============================================"
echo "  Lightfriend käynnistyy!"
echo "============================================"
echo ""
echo "  Avaa selaimessa: http://localhost:3000"
echo ""
echo "  Kirjaudu sisään:"
ADMIN_EMAIL=$(grep "^ADMIN_EMAILS=" .env | cut -d= -f2)
echo "    Sähköposti: ${ADMIN_EMAIL}"
echo "    Salasana:   (valitsemasi salasana)"
echo ""
echo "  Komennot:"
echo "    Pysäytä:    docker compose down"
echo "    Logit:      docker compose logs -f core"
echo "    Käynnistä:  docker compose up -d"
echo ""
