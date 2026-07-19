# Lightfriend

AI assistant for dumbphones, designed so no one else can see your chats or personal data, including while AI processes them. All of the code is open source, and cryptographic evidence lets anyone independently verify which code is running in production.

Access WhatsApp, Telegram, Signal, email, calendar, web search, and more via SMS and voice calls - no apps or smartphone required.

Full-stack Rust (Axum backend, Yew WebAssembly frontend) with Matrix homeserver for multi-platform messaging.

## Verifiable Privacy Architecture

The privacy goal shapes the entire architecture: hardware isolation, encrypted storage, independent key management, remote attestation, and verifiable AI inference. The sections below describe the implemented controls and what the available evidence establishes.

- **Hardware isolation**: The production application runs in an **AWS Nitro Enclave** with no SSH login, interactive debugger, persistent storage, or direct external networking.
- **Remote attestation**: AWS hardware signs an attestation document containing the enclave measurement (PCR0/PCR1/PCR2).
- **Reproducible builds**: GitHub Actions builds the enclave image and publishes PCR values so they can be compared with the measurement reported by production.
- **Public code registry**: Approved image fingerprints are published to an [Arbitrum smart contract](https://lightfriend.ai/trust-chain).
- **Independent key management**: [Marlin KMS](https://github.com/marlinprotocol/oyster-monorepo) evaluates enclave attestation before releasing encryption keys. The Lightfriend operator does not manually provision the master key.
- **Verifiable AI inference**: [Tinfoil](https://tinfoil.sh) publishes source code and attestation evidence for its confidential-computing inference environment.

### Verify it yourself

```bash
./scripts/verify_live_attestation.sh https://lightfriend.ai --rpc-url https://arb1.arbitrum.io/rpc
```

This checks the AWS attestation signature, compares reported PCR values with the public build, and checks the public approval list. Attestation verifies deployment identity; it does not prove that the software is bug-free.

### Optional voice calls

Voice calls currently use OpenAI Realtime for a faster, more natural experience. Call audio and transcripts are processed outside Lightfriend's independently verifiable trust chain. OpenAI states that API data is not used for training unless the customer opts in, but default Realtime abuse-monitoring logs may retain customer content for up to 30 days. Voice calls are optional, and Lightfriend will switch as soon as a suitable open-source, attested voice alternative can provide a comparable experience.

- Live attestation: `https://lightfriend.ai/.well-known/lightfriend/attestation`
- Full explanation: [lightfriend.ai/trustless](https://lightfriend.ai/trustless)
- Trust chain dashboard: [lightfriend.ai/trust-chain](https://lightfriend.ai/trust-chain)

## Local Development

```bash
# Terminal 1: Backend
cd backend && cargo run

# Terminal 2: Frontend
cd frontend && trunk serve
```

- **Backend API**: http://localhost:3000
- **Frontend**: http://localhost:8080

## Docker (Enclave)

The enclave image bundles everything (PostgreSQL, Tuwunel, mautrix bridges, Lightfriend backend) into a single container under supervisord.

```bash
# Build for current platform (local testing)
just build-local

# Start
just up

# View logs
just logs
```

See `just --list` for all available commands.

## Documentation

- [Matrix Setup Guide](docs/MATRIX_SETUP_GUIDE.md) - manual Matrix setup for local dev
- [Infrastructure Setup](docs/INFRASTRUCTURE_SETUP.md) - cloud deployment with Terraform
- [CLAUDE.md](CLAUDE.md) - project architecture and development guide

## License

This project is licensed under the **GNU Affero General Public License v3**. See the LICENSE file for details.

The name "Lightfriend" and any associated branding (including logos, icons, or visual elements) are owned by Rasmus Ahtava. These elements are not included in the AGPLv3 license and may not be used without permission, especially for commercial purposes or in ways that imply endorsement or affiliation. Forks or derivatives should use a different name and branding to avoid confusion.
