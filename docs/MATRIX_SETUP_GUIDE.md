# Matrix + Mautrix Bridges Setup Guide

Complete guide to set up Synapse Matrix homeserver with mautrix bridges for WhatsApp, Signal, Telegram, and Instagram.

**Documentation Sources:**
- [Mautrix Bridge Setup](https://docs.mau.fi/bridges/go/setup.html)
- [Mautrix Python Bridge Setup](https://docs.mau.fi/bridges/python/setup.html)
- [Double Puppeting](https://docs.mau.fi/bridges/general/double-puppeting.html)

---

## Prerequisites

```bash
# Install dependencies (macOS)
brew install python3 go postgresql libolm ffmpeg

# Install Synapse
pip3 install matrix-synapse

# Install shared secret authenticator (for double puppeting)
pip3 install git+https://github.com/devture/matrix-synapse-shared-secret-auth.git
```

---

## 1. PostgreSQL Setup

```bash
# Start PostgreSQL
brew services start postgresql

# Create Synapse database
psql postgres -c "CREATE USER synapse_user WITH PASSWORD '<synapse_password>';"
psql postgres -c "CREATE DATABASE synapse_db OWNER synapse_user;"

# Create WhatsApp bridge database
psql postgres -c "CREATE USER whatsapp_user WITH PASSWORD '<whatsapp_db_password>';"
psql postgres -c "CREATE DATABASE whatsapp_db OWNER whatsapp_user;"

# Create Signal bridge database
psql postgres -c "CREATE USER signal_user WITH PASSWORD '<signal_db_password>';"
psql postgres -c "CREATE DATABASE signal_db OWNER signal_user;"

# Create Telegram bridge database
psql postgres -c "CREATE USER telegram_user WITH PASSWORD '<telegram_db_password>';"
psql postgres -c "CREATE DATABASE telegram_db OWNER telegram_user;"

# Create Instagram bridge database
psql postgres -c "CREATE USER instagram_user WITH PASSWORD '<instagram_db_password>';"
psql postgres -c "CREATE DATABASE instagram_db OWNER instagram_user;"
```

---

## 2. Synapse Homeserver Setup

```bash
cd <matrix-dev-dir>

# Generate initial config (only if homeserver.yaml doesn't exist)
python3 -m synapse.app.homeserver \
    --server-name localhost \
    --config-path homeserver.yaml \
    --generate-config \
    --report-stats=no
```

### Key homeserver.yaml settings:

```yaml
server_name: "localhost"
pid_file: <matrix-dev-dir>/homeserver.pid

listeners:
  - port: 8008
    tls: false
    type: http
    x_forwarded: false
    bind_addresses: ['::1', '127.0.0.1']
    resources:
      - names: [client, federation]
        compress: false

database:
  name: psycopg2
  args:
    user: synapse_user
    password: "<synapse_password>"
    database: synapse_db
    host: 127.0.0.1
    port: 5432
    cp_min: 5
    cp_max: 10

enable_registration: true
enable_registration_without_verification: true

# Generate these with: openssl rand -base64 32
registration_shared_secret: "<registration_secret>"
login_shared_secret: "<login_secret>"

# Bridge registrations - ADD ALL OF THESE
app_service_config_files:
  - <matrix-dev-dir>/mautrix-whatsapp-registration.yaml
  - <matrix-dev-dir>/mautrix-telegram-registration.yaml
  - <matrix-dev-dir>/mautrix-signal-registration.yaml
  - <matrix-dev-dir>/mautrix-instagram-registration.yaml
  - <matrix-dev-dir>/doublepuppet.yaml

# Shared secret authenticator for double puppeting
modules:
  - module: shared_secret_authenticator.SharedSecretAuthProvider
    config:
      shared_secret: "<shared_secret>"  # Generate with: openssl rand -hex 32
      m_login_password_support_enabled: true
```

---

## 3. Double Puppet Setup

Create `doublepuppet.yaml` in the matrix-dev folder:

```bash
# Generate tokens
openssl rand -base64 32  # Use this for as_token
```

**doublepuppet.yaml:**
```yaml
id: doublepuppet
url: null
as_token: <doublepuppet_as_token>
hs_token: <random_string>
sender_localpart: <random_string>
rate_limited: false
namespaces:
  users:
  - regex: '@.*:localhost'
    exclusive: false
```

**Important:** The `as_token` from doublepuppet.yaml must be added to each bridge's config.yaml under `double_puppet.secrets.localhost`.

---

## 4. mautrix-whatsapp Bridge

```bash
cd <matrix-dev-dir>

# Clone and build
git clone https://github.com/mautrix/whatsapp.git mautrix-whatsapp
cd mautrix-whatsapp
./build.sh

# Verify build
./mautrix-whatsapp --version

# Generate example config
./mautrix-whatsapp -e

# Generate registration
./mautrix-whatsapp -g

# Copy registration to main dir
cp registration.yaml ../mautrix-whatsapp-registration.yaml

# Create logs directory
mkdir -p logs
```

### Key config.yaml changes for WhatsApp:

```yaml
database:
    type: postgres
    uri: postgres://whatsapp_user:<whatsapp_db_password>@127.0.0.1/whatsapp_db?sslmode=disable

homeserver:
    address: http://127.0.0.1:8008
    domain: localhost

appservice:
    address: http://localhost:29318
    hostname: 0.0.0.0
    port: 29318
    id: whatsapp
    bot:
        username: whatsappbot

bridge:
    permissions:
        "*": user
        "@adminuser:localhost": admin

double_puppet:
    secrets:
        localhost: as_token:<doublepuppet_as_token>
```

---

## 5. mautrix-signal Bridge

```bash
cd <matrix-dev-dir>

# Clone and build
git clone https://github.com/mautrix/signal.git mautrix-signal
cd mautrix-signal
./build.sh

# Verify build
./mautrix-signal --version

# Generate example config
./mautrix-signal -e

# Generate registration
./mautrix-signal -g

# Copy registration to main dir
cp registration.yaml ../mautrix-signal-registration.yaml

# Create logs directory
mkdir -p logs
```

### Key config.yaml changes for Signal:

```yaml
database:
    type: postgres
    uri: postgres://signal_user:<signal_db_password>@127.0.0.1/signal_db?sslmode=disable

homeserver:
    address: http://localhost:8008
    domain: localhost

appservice:
    address: http://localhost:29328
    hostname: 127.0.0.1
    port: 29328
    id: signal
    bot:
        username: signalbot

bridge:
    permissions:
        "*": user
        "@adminuser:localhost": admin

double_puppet:
    secrets:
        localhost: as_token:<doublepuppet_as_token>
```

---

## 6. mautrix-telegram Bridge (Python)

```bash
cd <matrix-dev-dir>

# Create directory and virtual environment
mkdir -p mautrix-telegram
cd mautrix-telegram
python3 -m venv .

# Activate and install
source bin/activate
pip install --upgrade mautrix-telegram[all]

# Generate example config
python -m mautrix_telegram -e > example-config.yaml
cp example-config.yaml config.yaml

# Generate registration (after editing config.yaml)
python -m mautrix_telegram -g -c config.yaml -r registration.yaml

# Copy registration to main dir
cp registration.yaml ../mautrix-telegram-registration.yaml

deactivate
```

### Key config.yaml changes for Telegram:

**Get API credentials from:** https://my.telegram.org/apps

```yaml
homeserver:
    address: http://localhost:8008
    domain: localhost

appservice:
    address: http://localhost:29317
    hostname: 0.0.0.0
    port: 29317
    database: postgres://telegram_user:<telegram_db_password>@localhost/telegram_db?sslmode=disable
    id: telegram
    bot_username: telegrambot

bridge:
    permissions:
        '*': puppeting
        '@adminuser:localhost': admin

    double_puppet_server_map:
        localhost: http://localhost:8008

    login_shared_secret_map:
        localhost: <shared_secret>  # Same as in homeserver.yaml modules section

telegram:
    api_id: <telegram_api_id>
    api_hash: <telegram_api_hash>
```

---

## 7. mautrix-meta Bridge (Instagram/Messenger)

```bash
cd <matrix-dev-dir>

# Clone and build
git clone https://github.com/mautrix/meta.git mautrix-meta
cd mautrix-meta
./build.sh

# Verify build
./mautrix-meta --version

# Generate example config
./mautrix-meta -e

# Generate registration
./mautrix-meta -g

# Copy registration to main dir
cp registration.yaml ../mautrix-instagram-registration.yaml

# Create logs directory
mkdir -p logs
```

### Key config.yaml changes for Meta (Instagram):

```yaml
network:
    mode: instagram  # or 'facebook' or 'messenger'

database:
    type: postgres
    uri: postgres://instagram_user:<instagram_db_password>@localhost/instagram_db?sslmode=disable

homeserver:
    address: http://localhost:8008
    domain: localhost

appservice:
    address: http://localhost:29319
    hostname: 127.0.0.1
    port: 29319
    id: instagram
    bot:
        username: igbot

bridge:
    permissions:
        "*": user
        "@adminuser:localhost": admin

double_puppet:
    secrets:
        localhost: as_token:<doublepuppet_as_token>
```

---

## 8. Starting Services

### Start Synapse
```bash
cd <matrix-dev-dir>
synctl start

# Verify it's running
curl http://localhost:8008/_matrix/client/versions
```

### Create Admin User
```bash
cd <matrix-dev-dir>
register_new_matrix_user \
    -c homeserver.yaml \
    -u adminuser \
    -p <admin_password> \
    --admin
```

### Start WhatsApp Bridge
```bash
cd <matrix-dev-dir>/mautrix-whatsapp
./mautrix-whatsapp
```

### Start Signal Bridge
```bash
cd <matrix-dev-dir>/mautrix-signal
./mautrix-signal
```

### Start Telegram Bridge
```bash
cd <matrix-dev-dir>/mautrix-telegram
source bin/activate
python -m mautrix_telegram
```

### Start Meta/Instagram Bridge
```bash
cd <matrix-dev-dir>/mautrix-meta
./mautrix-meta
```

---

## 9. Stopping Services

```bash
# Stop Synapse
cd <matrix-dev-dir>
synctl stop

# Stop bridges (Ctrl+C or)
pkill mautrix-whatsapp
pkill mautrix-signal
pkill -f mautrix_telegram
pkill mautrix-meta
```

---

## 10. Testing

1. Open https://app.element.io
2. Change homeserver to `http://localhost:8008`
3. Login with your admin user
4. Start a DM with the bridge bot:
   - WhatsApp: `@whatsappbot:localhost`
   - Signal: `@signalbot:localhost`
   - Telegram: `@telegrambot:localhost`
   - Instagram: `@igbot:localhost`
5. Send `help` to see available commands
6. Send `login` to start the login process

---

## Port Reference

| Service | Port |
|---------|------|
| Synapse | 8008 |
| mautrix-whatsapp | 29318 |
| mautrix-signal | 29328 |
| mautrix-telegram | 29317 |
| mautrix-meta | 29319 |
| PostgreSQL | 5432 |

---

## Troubleshooting

### Check if services are running
```bash
# Check Synapse
curl http://localhost:8008/_matrix/client/versions

# Check if ports are in use
lsof -i :8008
lsof -i :29318
lsof -i :29328
lsof -i :29317
lsof -i :29319
```

### View logs
```bash
# Synapse logs
tail -f <matrix-dev-dir>/homeserver.log

# Bridge logs (in their respective directories)
tail -f mautrix-whatsapp/logs/bridge.log
tail -f mautrix-signal/logs/bridge.log
tail -f mautrix-telegram/mautrix-telegram.log
tail -f mautrix-meta/logs/bridge.log
```

### Regenerate registration after config changes
```bash
# For Go bridges (whatsapp, signal, meta)
./mautrix-BRIDGE -g

# For Telegram (Python)
python -m mautrix_telegram -g -c config.yaml -r registration.yaml

# Then restart Synapse to reload registrations
synctl restart
```
