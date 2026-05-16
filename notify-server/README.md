# notify-server

Out-of-band alert relay for Lightfriend. Runs on a separate host so the main
app dying does not kill its own alerting.

## What it does

- `POST /alert` — receives JSON `{severity, title, body, dedup_key}` and pushes
  an SMS to the admin via a dedicated Twilio sub-account. Critical only by
  default. Lower severities are logged but not pushed.
- `POST /digest` — receives JSON `{subject, plain_body, html_body?}` and
  sends an email via Resend. Used for daily rollups.
- `GET /health` — `OK`.

All write endpoints require `Authorization: Bearer <NOTIFY_BEARER_TOKEN>`.

## Dedup

Each alert has an optional `dedup_key`. Within the TTL for that severity,
subsequent alerts with the same key are silently dropped. TTLs default to
1h / 6h / 24h for Critical / Error / Warning.

## Deploy (Hetzner / any VPS)

1. Build:
   ```
   cd notify-server
   cargo build --release
   ```
2. Copy `target/release/notify-server` to the host.
3. Copy `.env.example` to `.env` on the host and fill in real values. Use a
   **separate Twilio sub-account** for `TWILIO_*` so a ban on the prod
   number does not break alerting.
4. Run under systemd. Example unit:
   ```
   [Unit]
   Description=Lightfriend notify-server
   After=network.target

   [Service]
   Type=simple
   EnvironmentFile=/etc/notify-server.env
   ExecStart=/usr/local/bin/notify-server
   Restart=always
   RestartSec=5

   [Install]
   WantedBy=multi-user.target
   ```

## Example requests

Push a critical alert:
```
curl -X POST https://notify.example.com/alert \
  -H "Authorization: Bearer $NOTIFY_BEARER_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "severity": "critical",
    "title": "Lightfriend down",
    "body": "External watchdog could not reach https://lightfriend.ai",
    "dedup_key": "site-unreachable"
  }'
```

Send a digest:
```
curl -X POST https://notify.example.com/digest \
  -H "Authorization: Bearer $NOTIFY_BEARER_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "subject": "Lightfriend daily alert digest",
    "plain_body": "12 alerts in the last 24h:\n- 8x 30007 (carrier filter)\n- 3x 30033 (10DLC throughput)\n- 1x WhatsApp bridge restart"
  }'
```
