# Parent Proxy

Lightweight Rust proxy that runs on the parent EC2 instance outside Nitro Enclaves. It routes incoming webhooks (Twilio SMS, ElevenLabs voice) to the appropriate enclave based on user migration status.

## Architecture

```
Internet → Parent EC2 Instance → parent-proxy --VSOCK--> [Old Enclave | New Enclave]
                                      ↓
                              SQLite DB (read-only)
                              (checks active_enclave)
```

## How Routing Works

1. Webhook arrives at parent-proxy
2. Proxy extracts user identifier:
   - Twilio SMS: `From` phone number from form body
   - ElevenLabs: `user_id` from JSON at `data.conversation_initiation_client_data.dynamic_variables.user_id`
3. Proxy looks up `active_enclave` column in users table
4. Routes to appropriate enclave:
   - `"old"` → Old enclave
   - `NULL`, `"new"`, or user not found → New enclave
5. Forwards request with all headers intact (preserves signatures for verification in enclave)

## Configuration

Copy `.env.example` to `.env` and configure:

```bash
# Path to SQLite database (read-only)
DATABASE_PATH=/path/to/database.db

# VSOCK addresses for enclaves
OLD_ENCLAVE_CID=16
OLD_ENCLAVE_PORT=5000
NEW_ENCLAVE_CID=17
NEW_ENCLAVE_PORT=5000

# HTTP listen port
LISTEN_PORT=3000
```

## Building

```bash
cargo build --release
```

## Running

```bash
./target/release/parent-proxy
```

## Endpoints

- `POST /api/sms/server` - Twilio SMS webhook
- `POST /api/webhook/elevenlabs` - ElevenLabs voice webhook
- `GET /health` - Health check

## Development

On non-Linux systems, the proxy uses HTTP instead of VSOCK for testing. Set these env vars:

```bash
OLD_ENCLAVE_URL=http://localhost:3001
NEW_ENCLAVE_URL=http://localhost:3002
```

## Tests

```bash
cargo test
```
