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

- **Inbound**: Cloudflare tunnel running inside the enclave for webhook endpoints (Twilio, Stripe, ElevenLabs)
- **Outbound**: vsock proxies route TCP through the host to the internet
- **DNS**: Forwarded to Cloudflare DoH (1.1.1.1)
- **KMS**: Derive server connects to Marlin's root server at `image-v4.kms.box:1100`

### State Persistence

- Encrypted with AES-256-GCM using the KMS-derived key
- Stored on S3 (or parent instance EBS)
- Encrypted blobs contain: PostgreSQL data directory, Tuwunel RocksDB directory, bridge state

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

## Update Flow (Blue-Green Deployment)

1. Push code to master
2. GitHub Actions builds enclave image, publishes image ID
3. GitHub Actions submits `proposeImage` to Arbitrum
4. Wait 24 hours (old enclave runs normally during this period - zero downtime)
5. Call `activateImage()`
6. **Pre-flight checks** (old enclave still running):
   - Verify new image ID is activated on contract
   - Verify Marlin KMS root server is reachable
   - Verify S3 bucket is accessible
   - If ANY check fails -> ABORT, old enclave keeps running
7. Old enclave: encrypt state -> push to S3 -> verify upload checksum
8. Launch new EC2 instance with new enclave image
9. New enclave: derive same key from Marlin KMS -> pull from S3 -> decrypt state -> start up
10. Health check new enclave
11. Switch Cloudflare tunnel to new enclave
12. Verify new enclave serves requests
13. Terminate old EC2 instance
14. Optionally revoke old image ID

**Never stop old enclave until new one is confirmed working.** If new enclave fails at any step, old enclave is unaffected. Old image stays approved on contract, so a fresh old-image instance can always recover from S3.

For critical security fixes, the 24h delay is unavoidable - this is the cost of trustlessness.

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
