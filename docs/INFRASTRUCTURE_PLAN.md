# Lightfriend AWS Nitro Enclave Infrastructure Plan

## Overview

Privacy-preserving infrastructure using AWS Nitro Enclaves. The developer cannot access user message content - only encrypted blobs.

**Key design:**
- One enclave runs normally (using half instance resources)
- During updates: new enclave starts alongside old
- Users migrate when they open app (frontend verifies new enclave)
- Old enclave terminated after migration window
- No key transfer between enclaves - user's frontend always verifies
- No browser extension required - keys managed via Web Crypto API in frontend

## Privacy Model

| Data | Location | Developer Sees |
|------|----------|----------------|
| Accounts, subscriptions | Parent SQLite | Yes |
| Message content | Enclave only | No (encrypted) |
| OAuth tokens | Enclave only | No (encrypted) |
| Bridge state | Parent PostgreSQL | No (encrypted with user key) |

## Architecture

### Normal Operation (1 Enclave)

```
┌─────────────────────────────────────────────────────────────────┐
│                    c6a.2xlarge (~$140/month)                     │
│                    8 vCPU, 16 GB RAM                             │
│                    (buffer for updates + Synapse headroom)       │
│                                                                  │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │  PARENT (~2 vCPU, ~4 GB)                                   │  │
│  │  - cloudflared (tunnel)                                    │  │
│  │  - VSOCK proxy + user router                               │  │
│  │  - Synapse (~2-3 GB) - sees only E2E encrypted messages    │  │
│  │  - encrypted_user_state table (blobs, unreadable)          │  │
│  │  - SQLite (user metadata)                                  │  │
│  │                                                            │  │
│  │  Note: Synapse can move to separate server if needed -     │  │
│  │  no privacy impact since it only sees encrypted messages   │  │
│  └───────────────────────────────────────────────────────────┘  │
│                                                                  │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │  ENCLAVE (~3 vCPU, ~6 GB)                                  │  │
│  │  - Lightfriend Core (~500 MB)                              │  │
│  │  - mautrix bridges: WA/Signal/Messenger/IG (~1.5 GB)       │  │
│  │  - PostgreSQL for bridge state (~500 MB)                   │  │
│  │  - Session keys in memory (per-user)                       │  │
│  └───────────────────────────────────────────────────────────┘  │
│                                                                  │
│  RESERVED FOR UPDATES: ~3 vCPU, ~6 GB                           │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### During Update (2 Enclaves)

```
┌─────────────────────────────────────────────────────────────────┐
│                    c6a.2xlarge                                   │
│                                                                  │
│  ┌───────────────────────────────────────────────────────────┐  │
│  │  PARENT (~2 vCPU, ~4 GB)                                   │  │
│  └───────────────────────────────────────────────────────────┘  │
│                                                                  │
│  ┌─────────────────────────┐    ┌─────────────────────────┐     │
│  │  OLD ENCLAVE            │    │  NEW ENCLAVE            │     │
│  │  v1.2.3                 │    │  v1.2.4                 │     │
│  │  ~3 vCPU, ~6 GB         │    │  ~3 vCPU, ~6 GB         │     │
│  │                         │    │                         │     │
│  │  Serves users who       │    │  Receives users as      │     │
│  │  haven't migrated yet   │    │  they open app          │     │
│  └─────────────────────────┘    └─────────────────────────┘     │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘

Migration window: 3-7 days
After: Old enclave terminated, back to single enclave
```

## Update Flow

```
STEP 1: Normal - single enclave running v1.2.3
        ┌────────────────┐
        │  ENCLAVE       │
        │  v1.2.3        │
        │  100 users     │
        └────────────────┘

STEP 2: Deploy - start new enclave v1.2.4 alongside old
        ┌────────────────┐     ┌────────────────┐
        │  OLD ENCLAVE   │     │  NEW ENCLAVE   │
        │  v1.2.3        │     │  v1.2.4        │
        │  100 users     │     │  0 users       │
        └────────────────┘     └────────────────┘

STEP 3: Migrate - users move as they open app
        - Frontend detects new version available
        - Fetches and verifies new enclave attestation (PCRs)
        - Derives session key, encrypts with enclave pubkey
        - Sends encrypted session key to new enclave
        - New enclave loads user's encrypted state

        ┌────────────────┐     ┌────────────────┐
        │  OLD ENCLAVE   │     │  NEW ENCLAVE   │
        │  v1.2.3        │     │  v1.2.4        │
        │  20 users      │     │  80 users      │
        └────────────────┘     └────────────────┘

STEP 4: Complete - terminate old enclave after 3-7 days
        ┌────────────────┐
        │  ENCLAVE       │
        │  v1.2.4        │
        │  100 users     │
        └────────────────┘

RAPID UPDATE HANDLING:
- Policy: No new deploys until migration window closes
- If urgent: Force-migrate remaining users (email notification,
  terminate old enclave, users re-auth on next visit)
- c6a.2xlarge provides buffer for 3 enclaves in emergencies
```

## Client-Side Key Management (No Extension Required)

Instead of a browser extension, keys are managed directly in the frontend using Web Crypto API and IndexedDB. This provides better UX than requiring users to install a browser extension.

### Key Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                    KEY GENERATION (First Visit)                  │
└─────────────────────────────────────────────────────────────────┘

1. Frontend generates wrapper_key via crypto.subtle.generateKey()
   - AES-256-GCM, extractable: false (JS cannot read raw bytes)

2. Frontend generates master_key via crypto.getRandomValues(32)

3. Encrypt master_key with wrapper_key, store in IndexedDB
   - Even XSS cannot extract wrapper_key (only use it)

4. Master key persists until browser data cleared

┌─────────────────────────────────────────────────────────────────┐
│                    SESSION ESTABLISHMENT                         │
└─────────────────────────────────────────────────────────────────┘

┌──────────┐                    ┌───────────┐
│ Frontend │                    │ Enclave   │
└────┬─────┘                    └─────┬─────┘
     │                                │
     │ 1. GET /api/enclave/attestation│
     │───────────────────────────────>│
     │                                │
     │ 2. Return attestation doc      │
     │   (signed by AWS Nitro)        │
     │<───────────────────────────────│
     │                                │
     │ 3. Verify PCRs match           │
     │    published values            │
     │                                │
     │ 4. Derive session key:         │
     │    HKDF(master_key,            │
     │         "lightfriend_v1")      │
     │                                │
     │ 5. Encrypt session key with    │
     │    enclave's public key        │
     │                                │
     │ 6. POST /api/session/establish │
     │    { encrypted_session_key }   │
     │───────────────────────────────>│
     │                                │
     │                                │ 7. Decrypt with private key
     │                                │    Store: session_keys[user_id]
     │                                │
     │ 8. Session established         │
     │<───────────────────────────────│
     │                                │

┌─────────────────────────────────────────────────────────────────┐
│                    KEY LOSS FLOW                                 │
└─────────────────────────────────────────────────────────────────┘

If user clears browser data / switches device:
1. Login normally (JWT auth)
2. Frontend generates NEW master key
3. Establishes new session with enclave
4. User must re-connect integrations (OAuth re-auth)
5. Previous encrypted data orphaned (unrecoverable by design)

Simple UX: no backup phrases, just re-setup integrations.
```

### Security Model

**Developer CANNOT see:**
- Master key (lives in user's browser/device only)
- Session key (encrypted in transit, only enclave can decrypt)
- Enclave private key (generated inside, never leaves)
- Message content, OAuth tokens (encrypted with session key)

**Developer CAN see:**
- Encrypted blobs (useless without keys)
- User metadata (accounts, subscriptions, timestamps)
- Which integrations connected (but not credentials)

**Trade-off accepted:** XSS on frontend could use wrapper key to decrypt master key. Mitigated by strong CSP and XSS prevention. This is acceptable because:
- Primary threat model is "developer cannot spy on users"
- XSS is a serious bug regardless (could steal JWTs, modify UI)

## Session Key Design

```
session_key = HKDF(master_key, "lightfriend_session_v1")

- master_key: 256-bit, generated by frontend, stored encrypted in IndexedDB
- Stable: same key as long as master_key exists
- Rotatable: change version string to force rotation
- Enclave stores: session_keys[user_id]
```

## Encrypted State Persistence

```
encrypted_user_state table (parent PostgreSQL):
┌─────────────────────────────────────────────────────────────┐
│ user_id │ encrypted_blob                    │ updated_at    │
│─────────│───────────────────────────────────│───────────────│
│ usr_001 │ AES-GCM(session_key, {            │ 2026-01-12    │
│         │   whatsapp_state, signal_state,   │               │
│         │   oauth_tokens, ...               │               │
│         │ })                                │               │
└─────────────────────────────────────────────────────────────┘

- Snapshots every 5 min + on graceful shutdown
- Both enclaves can read, only decrypt with user's key
- On crash: up to 5 min of state may be lost (acceptable trade-off)
```

## Scaling Path

| Phase | Users | Instance | Cost | Notes |
|-------|-------|----------|------|-------|
| MVP | 0-200 | c6a.2xlarge (8 vCPU, 16 GB) | ~$140/mo | Buffer for updates + Synapse |
| Growth | 200-500 | c6a.4xlarge (16 vCPU, 32 GB) | ~$280/mo | |
| Scale | 500-1500 | c6a.8xlarge (32 vCPU, 64 GB) | ~$560/mo | |
| Large | 1500-4000 | c6a.16xlarge (64 vCPU, 128 GB) | ~$1,120/mo | |

**Vertical scaling rationale:**
- Simpler operations than horizontal
- No state coordination needed
- AWS instances scale to c6a.48xlarge (192 vCPU, 384 GB)
- Horizontal scaling only needed for: geographic distribution, active-active redundancy, or exceeding largest instance

Each phase: one enclave uses half resources, other half reserved for updates.
Synapse can move to separate server (~$15/mo) if parent resources constrained.

## Implementation Epics

**Infrastructure:** Terraform for VPC, EC2, security groups. Cloudflare Zero Trust tunnel.

**Enclave:** Dockerfile with Core + bridges + PostgreSQL. VSOCK communication. Attestation endpoint.

**Frontend Key Management:**
- Web: IndexedDB + Web Crypto API wrapper
- Attestation verification (PCR validation in WASM)
- Session key derivation (HKDF)
- Auto-migration on version change

**Backend:**
- Per-user session keys
- Encrypted state persistence
- State restoration on key provision

**Deployment:** User router (routes to correct enclave). Migration window management. Force-migration mechanism.

## Verification

1. Single enclave running, serving all users
2. Deploy new enclave alongside old
3. User opens app, frontend auto-migrates them
4. Encrypted state restored in new enclave
5. Old enclave terminated after window
6. Logs contain no user data

## Decisions Log

| Decision | Rationale |
|----------|-----------|
| No browser extension | Better UX. Web Crypto API sufficient for threat model. |
| Web Crypto API for keys | Non-extractable wrapper key protects master key at rest. Trade-off: XSS could use key, but XSS is catastrophic anyway. |
| Start with c6a.2xlarge | Buffer for dual-enclave updates, Synapse memory headroom. ~$70/mo more than minimum. |
| Vertical scaling first | Simpler ops, no state coordination. Horizontal only when exceeding largest instance or need geo-distribution. |
| Force-migration for rapid updates | Policy: no concurrent deploys. Emergency: terminate old enclave, users re-auth. |
| No backup phrases | Key loss = re-setup integrations. Simpler UX than BIP39. Email notification prompts re-login. |
