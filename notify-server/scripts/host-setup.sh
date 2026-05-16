#!/usr/bin/env bash
# One-time bootstrap on the Hetzner host.
#
# Sets up:
#   - ~/notify-server/.env  (created from .env.example if missing)
#   - ~/.config/systemd/user/notify-server.service  (user systemd unit)
#   - loginctl linger so the service runs without an interactive login
#
# Usage:
#   bash scripts/host-setup.sh
#
# After running this once, set the values in ~/notify-server/.env, then:
#   systemctl --user start notify-server
#   systemctl --user enable notify-server
#   systemctl --user status notify-server

set -euo pipefail

ROOT="$HOME/notify-server"
ENV_FILE="$ROOT/.env"
UNIT_DIR="$HOME/.config/systemd/user"
UNIT_FILE="$UNIT_DIR/notify-server.service"

mkdir -p "$ROOT" "$UNIT_DIR"

# ── .env scaffold ──
if [ ! -f "$ENV_FILE" ]; then
  TOKEN=$(openssl rand -hex 32)
  cat > "$ENV_FILE" <<ENV
# notify-server config. KEEP SECRET. Mode 600.
BIND_ADDR=127.0.0.1:8088
NOTIFY_BEARER_TOKEN=$TOKEN

# Twilio (use a separate sub-account from prod so a ban on prod
# numbers does not also kill alerting)
TWILIO_ACCOUNT_SID=
TWILIO_AUTH_TOKEN=
TWILIO_FROM_NUMBER=
ADMIN_PHONE_NUMBER=

# Resend (only used for /digest, not /alert)
RESEND_API_KEY=
RESEND_FROM_EMAIL=notify@lightfriend.ai
ADMIN_EMAIL=rasmus@lightfriend.ai

# Dedup TTLs in seconds
DEDUP_TTL_CRITICAL_SECS=3600
DEDUP_TTL_ERROR_SECS=21600
DEDUP_TTL_WARNING_SECS=86400

RUST_LOG=info,notify_server=debug
ENV
  chmod 600 "$ENV_FILE"
  echo "Created $ENV_FILE with a fresh NOTIFY_BEARER_TOKEN."
  echo
  echo "Bearer token (also save in GitHub secret NOTIFY_SERVER_TOKEN):"
  grep NOTIFY_BEARER_TOKEN "$ENV_FILE"
  echo
  echo "Now fill in TWILIO_ACCOUNT_SID, TWILIO_AUTH_TOKEN, TWILIO_FROM_NUMBER,"
  echo "ADMIN_PHONE_NUMBER, and (optionally) RESEND_API_KEY."
else
  echo "$ENV_FILE already exists; leaving it alone."
fi

# ── systemd user unit ──
cat > "$UNIT_FILE" <<'UNIT'
[Unit]
Description=Lightfriend notify-server (out-of-band SMS alerting)
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
EnvironmentFile=%h/notify-server/.env
ExecStart=%h/notify-server/notify-server
Restart=always
RestartSec=5

# Hardening
NoNewPrivileges=yes
PrivateTmp=yes
ProtectSystem=strict
ProtectHome=read-only
ReadWritePaths=%h/notify-server
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectControlGroups=yes

[Install]
WantedBy=default.target
UNIT
echo "Wrote $UNIT_FILE"

systemctl --user daemon-reload

cat <<EOM

Setup almost done. One more step that needs root (run once):

  sudo loginctl enable-linger $USER

This lets the service keep running when you're not logged in. After that:

  1. Edit ~/notify-server/.env and fill in the empty values
  2. Make sure the binary exists at ~/notify-server/notify-server
     (it will be there after the first CI deploy)
  3. systemctl --user enable --now notify-server
  4. systemctl --user status notify-server
  5. curl -fsS http://127.0.0.1:8088/health   # expects "OK"
  6. Add a cloudflared tunnel route: status.lightfriend.ai -> http://localhost:8088

To smoke-test once running:

  curl -sS -X POST http://127.0.0.1:8088/alert \\
    -H "Authorization: Bearer \$NOTIFY_BEARER_TOKEN" \\
    -H "Content-Type: application/json" \\
    -d '{"severity":"critical","title":"smoke test","body":"hello from $(hostname)","dedup_key":"smoke"}'

EOM
