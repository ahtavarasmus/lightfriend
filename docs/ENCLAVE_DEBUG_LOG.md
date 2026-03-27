# Enclave Boot Debug Log - 2026-03-26/27

## Bugs Found and Fixed (in order)

### 1. VSOCK Port Conflict (E36) - FIXED
**Symptom:** `nitro-cli run-enclave` fails with E36 "vsock bind error"
**Root cause:** Host socat VSOCK-LISTEN services on VMADDR_CID_ANY conflict with Nitro CLI's heartbeat bind
**Fix:** Stop all VSOCK services before launch, restart immediately after
**Commit:** db88ca1

### 2. Config Server socat Direction - FIXED
**Symptom:** Env loading gets 0 bytes from host config server
**Root cause:** `socat -u VSOCK-LISTEN FILE:` means left-to-right (read from VSOCK, write to file) - backwards
**Fix:** `socat VSOCK-LISTEN OPEN:file` (bidirectional, OPEN reads file sends to client)
**Commit:** d1b027c

### 3. `source .env` Unsafe with Special Chars - FIXED
**Symptom:** `source /tmp/host_env` crashes: "qR2: unbound variable", "hF4^w3Y: command not found"
**Root cause:** JWT keys contain `$`, `^`, newlines that bash interprets as commands/vars
**Fix:** Line-by-line `export "$line"` with KEY=VALUE regex filter
**Commit:** 9985d96

### 4. Loopback Interface Down - FIXED
**Symptom:** Proxy test fails with curl exit=7 (connection refused to 127.0.0.1)
**Root cause:** Nitro enclaves start with all network interfaces down, including loopback
**Fix:** `ip link set lo up` early in entrypoint
**Commit:** 89c389f

### 5. Missing iproute2 Package - FIXED
**Symptom:** `ip link set lo up` fails: "command not found"
**Root cause:** Docker image didn't include iproute2
**Fix:** Added `iproute2` to Dockerfile apt-get
**Commit:** 34b60b5

### 6. Attestation Server `--pub-key` Flag - FIXED
**Symptom:** oyster-attestation-server exits immediately
**Root cause:** `--pub-key` not a valid CLI flag for this version
**Fix:** Removed the flag
**Commit:** a4501ba

### 7. Missing kms-derive-server Binary - FIXED
**Symptom:** Derive server "died immediately" with empty log
**Root cause:** Without `--target-dir`, each cargo build uses its own target/, find doesn't locate binaries
**Fix:** Added `--target-dir /src/oyster/target` to all cargo builds, added verification
**Commit:** 56f14ee

### 8. Marlin KMS Image Approval (On-Chain) - FIXED
**Symptom:** `Error: failed to fetch seed` from derive server
**Root cause:** image_id not registered on Arbitrum KmsVerifiable contract. Deploy pipeline was passing raw PCR0 (48 bytes) instead of SHA-256(bitflags + PCR0-2 + PCR16) (32 bytes)
**Fix:** Added approve-kms.yml workflow, fixed image_id computation in deploy pipeline
**Commit:** dbe57b4

### 9. X25519 Public Key Missing from Attestation - FIXED
**Symptom:** `connection closed before message completed` during Scallop handshake
**Root cause:** Attestation document didn't contain the X25519 public key, root server couldn't bind attestation to handshake
**Fix:** Include hex-encoded public key as query param in attestation endpoint URL
**Commit:** 77a7471 (hex encoding fixed in e66b163)

### 10. Seed via Raw VSOCK Drops Large Payloads - FIXED
**Symptom:** 17MB seed arrives as 0 bytes, host logs "Connection served" but enclave gets nothing
**Root cause:** VSOCK unreliable for large one-shot transfers
**Fix:** Serve seed via HTTP (python3 http.server on host port 9080), fetch via curl through VSOCK-bridged HTTP
**Commit:** 3a28af1

### 11. Missing Runtime Directories - FIXED
**Symptom:** `chown: cannot access '/var/lib/postgresql/data'`, then `/run/postgresql`
**Root cause:** Directories not created in Docker image, Nitro strips volumes
**Fix:** `mkdir -p` before chown in entrypoint
**Commits:** f5eaf8e, e7bc59f

### 12. Cloudflared Can't Use HTTP Proxy for Edge - IN PROGRESS
**Symptom:** Cloudflared never connects to Cloudflare edge (no traffic on port 7844)
**Root cause:** Cloudflared uses raw TCP sockets for edge connections, ignores HTTPS_PROXY env var entirely (confirmed from source code, GitHub issues #1025, #350, #170)
**Attempted fixes:**
- `--protocol http2` flag: correct approach (QUIC uses UDP which can't be proxied)
- `HTTPS_PROXY` env in supervisord: doesn't work for edge connections
- iptables DNAT: fails because Nitro enclave kernel doesn't support iptables/netfilter
- `/etc/hosts` DNS override: resolves edge hostnames to 127.0.0.1 with socat bridge to VSOCK -> host -> Cloudflare edge. Go's net.ResolveTCPAddr confirmed to respect /etc/hosts.
**Current status:** `/etc/hosts` approach deployed, VSOCK bridge active on host, but cloudflared still not connecting. Zero connections seen on port 7844.
**Possible remaining issue:** Startup script may not be reaching cloudflared start due to health check going through proxy (see #13)

### 13. Health Check Goes Through Proxy (NO_PROXY Ignored) - INVESTIGATING
**Symptom:** `http://127.0.0.1:3000/api/health` appears in squid access log
**Root cause:** `NO_PROXY=localhost,127.0.0.1` set but curl inside startup script may not inherit it
**Impact:** Backend health check in signal script loops forever (proxied to host localhost where nothing listens)
**Status:** Added HTTP beacon diagnostics to startup script to trace exactly which steps complete

## Current Boot Progress (as of commit d4225d3)

Step 0a - Env loading: WORKS (197 vars, attempt 2)
Step 0c - Outbound proxy: WORKS
Step 0d - KMS key derivation: WORKS (Marlin backup key derived)
Step 0f - SQL seed: WORKS (17MB via HTTP)
Step 1  - PostgreSQL init: WORKS
Step 2  - PostgreSQL start + DB creation: WORKS
Step 2b - Seed restore: WORKS (226 users)
Step 3  - Config generation: WORKS
Step CF - /etc/hosts override: WORKS
Step FINAL - Supervisord launch: WORKS (exit=0)
Backend - Tesla API calls visible in squid: RUNNING
Cloudflared - No edge connections: NOT CONNECTING
Public endpoint - HTTP 530: NOT REACHABLE

## Diagnostic Tools Available

- Boot trace: sent via VSOCK port 9007 (entrypoint only, startup services log unreliable)
- Squid access log: shows all proxied HTTPS CONNECT and HTTP requests
- HTTP beacons: curl to httpbin.org/anything/enclave-{step} from startup script
- Host diagnose.sh: one-shot diagnostic script
- Host CF edge bridge log: /opt/lightfriend/logs/cloudflared-edge.log

## Host Services Running

- squid (HTTP proxy, 127.0.0.1:3128)
- vsock-proxy-bridge (VSOCK:8001 -> squid)
- vsock-config-server (VSOCK:9000 -> host-env file)
- vsock-seed-server (VSOCK:9003 -> seed SQL, legacy)
- vsock-boot-trace (VSOCK:9007 -> boot trace files)
- vsock-seed-http (VSOCK:9080 -> python3 HTTP seed server)
- vsock-cloudflared-edge (VSOCK:7844 -> region1.v2.argotunnel.com:7844)
- vsock-marlin-kms-bridge (VSOCK:9010 -> arbone-v4.kms.box:1100)
- seed-http-server (python3 http.server on port 9080)
