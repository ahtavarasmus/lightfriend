# Lightfriend CLI

The Lightfriend CLI is the hardened, write-only core for local AI-agent
integrations. It can create one-shot reminders and sender-scoped email reply
watches. It has no command or API access for reading messages, contacts,
history, reminder contents, or account data.

Install from the repository:

```sh
git clone https://github.com/ahtavarasmus/lightfriend.git
cargo install --path lightfriend/lightfriend-cli
lightfriend login
```

`lightfriend login` uses a short-lived device pairing flow. The bearer token is
returned only to the local CLI and stored in the operating system credential
store. Never paste it into a prompt, chat, environment file, command URL, or
source file.

Examples:

```sh
lightfriend remind --at '2026-08-09T09:00:00+03:00' --message 'Call the dentist'
lightfriend watch-reply --email person@example.com --for-minutes 120
lightfriend logout
```

Every mutating request carries a fresh idempotency key. The server applies a
20-action daily cap per credential, limits reply watches to five active rows,
expires credentials after 90 days, and keeps a content-free audit trail.
