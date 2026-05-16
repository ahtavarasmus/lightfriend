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

## Deploy

Auto-deployed on push to master via `.github/workflows/notify-server-deploy.yml`.
The workflow builds the binary on `ubuntu-latest` and ships it to the
Hetzner box over SSH. Future changes to `notify-server/**` redeploy
automatically.

### First-time host setup (one-time)

On the Hetzner box (run once):

```
bash notify-server/scripts/host-setup.sh
sudo loginctl enable-linger $USER   # the one sudo step
```

This creates `~/notify-server/.env` with a fresh bearer token, installs a
user-level systemd unit at `~/.config/systemd/user/notify-server.service`,
and reloads systemd.

Fill in the empty values in `~/notify-server/.env`. Critical:
**use a separate Twilio sub-account** from prod so a ban on the prod
number cannot also silence alerting.

Trigger the first deploy from GitHub Actions (`workflow_dispatch` on
`notify-server deploy`) to land the binary, then:

```
systemctl --user enable --now notify-server
systemctl --user status notify-server
curl -fsS http://127.0.0.1:8088/health   # expects "OK"
```

### GitHub secrets needed for auto-deploy

| Secret | What |
|---|---|
| `RETARD_SSH_HOST` | hostname/IP of the box |
| `RETARD_SSH_USER` | SSH user (e.g. `rasmus`) |
| `RETARD_SSH_KEY` | private SSH key with access |
| `RETARD_SSH_PORT` | optional, defaults to 22 |

### Public exposure

The server listens on `127.0.0.1:8088` by default. Expose via cloudflared
tunnel (already running on the box):

```
status.lightfriend.ai  ->  http://localhost:8088
```

Bearer auth is the only network-exposed boundary so keep the tunnel route
scoped to this single backend.

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
