# Lightfriend

Verifiably private AI assistant for dumbphones. Access WhatsApp, Telegram, Signal, email, calendar, web search, and more via SMS and voice calls - no apps, no smartphone required. A practical solution for phone addiction: keep essential communication, ditch the addictive interfaces.

Full-stack Rust (Axum backend, Yew WebAssembly frontend) with Matrix homeserver for multi-platform messaging.

## Verifiable Privacy (Hardware Attestation)

The first verifiably private cloud AI assistant. Not "we promise not to look" - cryptographically proven. Nobody can access user data: not the developer, not the cloud provider. Anyone can verify this at any time.

- **Hardware isolation**: The entire application runs in an **AWS Nitro Enclave** - no SSH, no debugger, no memory access. Not by AWS, not by us, not by anyone.
- **Remote attestation**: Cryptographic proof signed by AWS hardware (PCR0/PCR1/PCR2) proving exactly which code is running. Cannot be faked.
- **Reproducible builds**: GitHub Actions builds the enclave image and publishes PCR values. Anyone can verify the live enclave matches this repo.
- **Blockchain code registry**: Approved image fingerprints are published to an [Arbitrum smart contract](https://lightfriend.ai/trust-chain) - immutable, public, tamper-proof.
- **Independent key management**: [Marlin KMS](https://github.com/marlinprotocol/oyster-monorepo) (itself running in an attested enclave) only releases encryption keys to enclaves running approved code. The developer never holds keys.
- **Verifiable AI inference**: AI processing runs inside [Tinfoil](https://tinfoil.sh)'s hardware-isolated enclaves with their own attestation.

### Verify it yourself

```bash
./scripts/verify_live_attestation.sh https://lightfriend.ai --rpc-url https://arb1.arbitrum.io/rpc
```

This checks: AWS attestation signature, PCR values match public build, code is on the blockchain approval list.

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
