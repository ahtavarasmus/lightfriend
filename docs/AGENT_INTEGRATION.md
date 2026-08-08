# Local agent integration

Lightfriend's local agent integration is deliberately write-only. Codex,
Claude Code, and other local clients can create a one-shot reminder or arm a
short, sender-scoped email reply watch. There is no agent endpoint for reading
messages, contacts, message history, existing reminder contents, account data,
payments, or summaries.

## Setup

```sh
git clone https://github.com/ahtavarasmus/lightfriend.git
cargo install --path lightfriend/lightfriend-cli
lightfriend login
```

The CLI shows a short-lived pairing code. In the Lightfriend dashboard, open
Settings, find **Connect an agent**, and approve that code. The device secret is
sent only in JSON request bodies and the resulting bearer is returned only once
to the polling CLI. It is then stored in the operating-system credential store.
Neither secret is placed in a URL or an agent prompt.

## Security boundary

- Bearers contain 256 random bits and are stored server-side only as SHA-256
  digests. The dashboard shows a short prefix, never the bearer.
- Credentials are bound to one user, expire after 90 days, are revocable from
  the dashboard or CLI, and grant exactly `reminders` and
  `reply_watch_email`.
- Every action requires an `Idempotency-Key`, has a strict schema and size
  limit, and counts against an atomic 20-action daily cap.
- Reminder times must be RFC 3339 with an explicit offset, at least one minute
  ahead, and no more than one year ahead.
- Reply watches accept one syntactically valid sender email, expire in 15
  minutes to 24 hours, fire once, and are limited to five active database rows
  per user.
- Audit rows contain only credential ID, user ID, action kind, outcome, and
  timestamp. Reminder text and email addresses are intentionally excluded.
- Action responses contain only `accepted`, `rejected`, or `failed`. They do
  not reveal whether a contact, message, reminder, or account datum exists.
- Action and credential endpoints reject query strings so a bearer leaked into
  a URL cannot be accepted. Responses use `Cache-Control: no-store`.

## CLI examples

```sh
lightfriend remind \
  --at '2026-08-09T09:00:00+03:00' \
  --message 'Call the dentist'

lightfriend watch-reply \
  --email person@example.com \
  --for-minutes 120

lightfriend logout
```

`lightfriend logout` revokes the server credential before deleting it from the
OS credential store. Rotation is intentionally explicit: revoke the old
credential, then run `lightfriend login` again.
