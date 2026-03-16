# Lightfriend Trustless Architecture

## Goal

Users don't need to trust the developer/operator. All user data is encrypted inside a Nitro Enclave, and the encryption key is only accessible to verified, attested code.

## Architecture Overview

```
Open source code (GitHub)
    -> GitHub Actions builds enclave image
    -> Publishes image ID (PCR0)
    -> Proposes image ID to timelocked smart contract (Arbitrum)
    -> 24h window for anyone to verify
    -> Nautilus KMS root server checks contract
    -> Only releases derived key to enclave matching approved image ID
```

## Components

### Nitro Enclave Contents

- Lightfriend backend (Rust/Axum)
- PostgreSQL (data encrypted at rest with KMS-derived key)
- Tuwunel Matrix server (RocksDB - lightweight Rust Matrix server, embedded database)
- Mautrix bridges (WhatsApp, Signal, Telegram)
- KMS derive server (sidecar, connects to Marlin's root server via Scallop protocol)

### Persistence (Encrypt/Decrypt Cycle)

Nitro Enclaves have no persistent storage. Data survives restarts via encrypted snapshots:

- **On shutdown**: tar PG data dir + Tuwunel RocksDB dir, encrypt with KMS-derived key, push to S3
- **On startup**: pull encrypted blobs from S3, decrypt with KMS-derived key, untar, start services

RocksDB is crash-resilient, so even non-clean snapshots should recover. PostgreSQL should be cleanly shut down before snapshotting.

### Key Custody - Marlin Nautilus KMS

Using the **contract variant** of Nautilus:

- Key = f(root_seed, chain_id, contract_address)
- Keys are deterministic - same contract + same root seed = same key every restart
- Deploy a KmsVerifiable contract on Arbitrum with 24h timelock
- Marlin's KMS root server (running in its own Nitro Enclave, verified on-chain) calls our contract to check if an image ID is approved before deriving a key
- The developer never sees the key. Marlin never sees the key. Only the attested enclave does.

**How Nautilus works internally:**
1. KMS root seed was generated inside a Nitro Enclave, encrypted against Threshold Network's TACo DKG key
2. Root server decrypts the seed only if verified on the KmsRoot contract (on-chain)
3. Derive server (inside our enclave) connects to root server via Scallop (Noise IX protocol)
4. Root server verifies our attestation, calls our KmsVerifiable contract
5. If approved, derives key: HMAC-SHA512(HMAC-SHA512(root, chain_id||address), chain_id||address)
6. Our app gets the key via simple HTTP: `GET http://127.0.0.1:1100/derive/x25519?path=encryption-key`

### Smart Contract (Arbitrum)

Solidity contract with 24h timelock:

```solidity
struct PendingImage {
    bytes32 imageId;
    uint256 activatesAt;
    string commitHash;
}

PendingImage public pendingApproval;
mapping(bytes32 => bool) public approvedImages;

function proposeImage(bytes32 imageId, string calldata commitHash) external onlyAuthorized {
    pendingApproval = PendingImage(imageId, block.timestamp + 24 hours, commitHash);
    emit ImageProposed(imageId, commitHash, block.timestamp + 24 hours);
}

function activateImage() external {
    require(block.timestamp >= pendingApproval.activatesAt, "timelock active");
    approvedImages[pendingApproval.imageId] = true;
    emit ImageActivated(pendingApproval.imageId);
}

function revokeImage(bytes32 imageId) external onlyAuthorized {
    approvedImages[imageId] = false;
    emit ImageRevoked(imageId);
}

function oysterKMSVerify(bytes32 imageId) external returns (bool) {
    return approvedImages[imageId];
}
```

### CI/CD (GitHub Actions)

On push to master:
1. Build enclave image (pin base images by digest, use lockfiles)
2. Compute image ID (PCR0 hash)
3. Publish image ID as build artifact (public logs)
4. Submit `proposeImage(imageId, commitHash)` transaction to Arbitrum
5. After 24h, `activateImage()` can be called
6. Deploy new enclave

### Enclave Networking

The enclave has no direct network access. All traffic flows through VSOCK channels to the host:

```
Inbound:  Internet -> Cloudflare -> tunnel -> cloudflared (inside enclave) -> localhost:3000
Outbound: app -> HTTP_PROXY:3128 -> socat/VSOCK:3:8001 -> host socat -> tinyproxy -> Internet
Env vars: entrypoint.sh -> VSOCK:3:9000 -> host config server -> .env
Backup out: export.sh -> VSOCK:3:9001 -> host backup receiver -> S3
Backup in:  entrypoint.sh -> VSOCK:3:9002 -> host seed server -> /data/seed/
```

#### VSOCK Port Map

| Port | Direction | Purpose |
|------|-----------|---------|
| 8001 | Enclave -> Host | HTTP proxy (outbound internet via tinyproxy) |
| 9000 | Enclave <- Host | Environment variable injection (.env) |
| 9001 | Enclave -> Host | Encrypted backup transfer out |
| 9002 | Enclave <- Host | Encrypted backup transfer in (for restore) |

- **Inbound**: cloudflared runs inside the enclave, connects to Cloudflare's edge via the outbound proxy. Cloudflare tunnels support multiple connectors - both old and new enclaves connect simultaneously during updates.
- **Outbound**: HTTP/HTTPS traffic goes through socat VSOCK bridge to tinyproxy on the host.
- **KMS**: Derive server connects to Marlin's root server at `image-v4.kms.box:1100` (Phase 2 - not yet implemented)

### State Persistence

- Encrypted with AES-256-CBC (Phase 1, env-based key) / AES-256-GCM (Phase 2, KMS-derived key)
- Backup transfer: enclave -> VSOCK:9001 -> host -> S3
- Restore transfer: S3 -> host -> VSOCK:9002 -> enclave
- Encrypted blobs contain: PostgreSQL dumps, Tuwunel RocksDB, bridge state, matrix store, uploads, core data

## Trust Chain

What users can independently verify:

| Link | How to verify |
|------|---------------|
| Source code | Open source on GitHub |
| Build integrity | GitHub Actions public logs, workflow is in the repo |
| Image approval | KmsVerifiable contract on Arbitrum, publicly readable |
| Timelock | 24h delay visible on-chain, anyone can check |
| Running enclave | Hit attestation endpoint, verify PCRs match approved image ID |
| Key custody | Marlin's root server verified on-chain via KmsRoot contract |
| Key release | Root server calls our contract, only serves approved images |

## Security Properties

### What the operator cannot do
- Access the encryption key outside the enclave
- Read enclave memory (Nitro hardware prevents it)
- Modify code inside a running enclave
- Silently approve a malicious image (24h timelock, public on Arbitrum)

### What the operator could theoretically do (residual risk)
- Propose a malicious image - visible for 24h before activation, community can verify
- Compromise GitHub Actions secrets - same 24h timelock protection applies
- Shut down the service (denial of service, not data theft)

### Trust assumptions
- AWS Nitro hardware provides integrity and confidentiality
- AMD/AWS are not colluding to break TEE isolation
- Marlin's KMS root server code is correct (open source, verified on-chain)
- Threshold Network (TACo) is available for root seed decryption
- GitHub Actions environment is not compromised (Microsoft-hosted runners)

## Update Flow

### Phase 1 (Current - env-based key)

1. SSM to old host: trigger `export.sh` in enclave (runs via supervisorctl exec or VSOCK)
2. Backup appears on host via VSOCK:9001, upload to S3 via `upload-backup.sh`
3. Create new EC2 with `terraform apply` (or AWS CLI)
4. SSM to new host: create `/opt/lightfriend/.env`, download backup from S3 via `download-backup.sh`
5. Launch enclave: `/opt/lightfriend/launch-enclave.sh`
6. Enclave fetches env via VSOCK:9000, backup via VSOCK:9002, restores, starts services
7. `verify.sh` runs automatically, produces `/data/seed/verify-result.json`
8. Both old and new enclaves serve traffic via Cloudflare tunnel (multiple connectors)
9. Verify new enclave is healthy
10. Terminate old EC2

**Never stop old enclave until new one is confirmed working.**

### Phase 2 (Future - Marlin KMS)

Same flow plus:
- Wait 24h for image approval timelock on Arbitrum
- New enclave derives key from Marlin KMS instead of env var
- Pre-flight checks verify KMS reachability and contract approval

For critical security fixes with KMS, the 24h delay is unavoidable - this is the cost of trustlessness.

## Open Questions

- **Marlin root server access**: Confirm the derive server can connect to Marlin's root server from a self-hosted Nitro Enclave (not deployed via Oyster marketplace)
- **Marlin KMS pricing**: Verify if there's a fee for KMS access on self-hosted enclaves (not deployed via Oyster marketplace)
- **TACo dependency**: Marlin's root server depends on Threshold Network for seed decryption. If TaCo is down, new enclave startups cannot get keys. Running enclaves are unaffected. Accepted risk: if TaCo disappears permanently, customer data recoverable from Stripe.
- **Reproducible builds**: Starting with GitHub Actions as the trusted builder. Can add Nix later if users demand full local build reproducibility.
- **Tuwunel inside enclave**: Memory requirements for running PG + Tuwunel + bridges + backend in a single enclave. Current allocation: 8GB RAM, 4 vCPUs.

## Resolved Decisions

- **Update strategy**: Blue-green deployment with pre-flight checks. Never stop old enclave until new one is confirmed. See Update Flow above.
- **Key backup**: No operator key backup. Zero-trust preserved. Data loss risk accepted per Stripe recovery path.
- **Cost**: EC2 enclave-capable instance ~$150-250/mo on-demand. Accepted.

## Tech Stack

| Component | Technology |
|-----------|------------|
| Smart contract | Solidity on Arbitrum |
| Key derivation | Marlin Nautilus KMS (contract variant) |
| Attestation | AWS Nitro native + Marlin on-chain verification |
| Enclave networking | vsock proxies + Cloudflare tunnel |
| State persistence | AES-256-GCM encrypted blobs on S3 |
| Matrix server | Tuwunel (Rust, embedded RocksDB) |
| Database | PostgreSQL |
| CI/CD | GitHub Actions |

## Key Resources

- [Marlin Nautilus KMS docs](https://docs.marlin.org/oyster/nautilus/)
- [Marlin Oyster monorepo](https://github.com/marlinprotocol/oyster-monorepo)
- [Nautilus security analysis](https://docs.marlin.org/oyster/nautilus/security)
- [KmsVerifiable contract](https://github.com/marlinprotocol/oyster-monorepo/blob/master/contracts/contracts-foundry/src/kms/KmsVerifiable.sol)
- [Scallop protocol](https://github.com/marlinprotocol/oyster-monorepo/blob/master/sdks/rs/src/scallop.rs)
- [AWS Nitro attestation docs](https://docs.aws.amazon.com/enclaves/latest/user/set-up-attestation.html)
- [NitroProver on-chain verification](https://github.com/marlinprotocol/NitroProver)
