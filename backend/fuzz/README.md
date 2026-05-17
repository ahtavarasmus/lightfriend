# Fuzz targets

Property-based, coverage-guided fuzz tests for security-critical pure
functions in `backend`. Each target asserts invariants that must hold
for **every** input an attacker can supply.

> **Companion**: bounded-model-checking proofs for the same functions
> live in `backend/tests/kani_signature_proofs.rs`. See the "Kani" section
> at the bottom for what those prove and how they complement fuzzing.

Today's targets:

| Target              | Function under test          | Why it matters |
| ------------------- | ---------------------------- | -------------- |
| `twilio_signature`  | `verify_twilio_signature`    | Webhook forgery would let an attacker impersonate Twilio (inject SMS, fire status callbacks, manipulate user state). |
| `telnyx_signature`  | `verify_telnyx_signature`    | Same attack surface for the Telnyx provider. |

## One-time setup

`cargo fuzz` needs nightly Rust and the `cargo-fuzz` subcommand:

```bash
rustup install nightly
cargo install cargo-fuzz
```

## Running

From this directory (`backend/fuzz`):

```bash
# Run forever (Ctrl-C to stop)
cargo +nightly fuzz run twilio_signature

# Time-boxed run (60 seconds) - what CI / pre-push should use
cargo +nightly fuzz run twilio_signature -- -max_total_time=60
cargo +nightly fuzz run telnyx_signature -- -max_total_time=60

# Reproduce a crash from an artifact
cargo +nightly fuzz run twilio_signature artifacts/twilio_signature/crash-<hash>
```

Or via the justfile recipes from the repo root:

```bash
just fuzz-twilio        # 60-second batch
just fuzz-telnyx        # 60-second batch
just fuzz-all           # 60 seconds each, sequentially
```

## What a finding looks like

If a fuzz run finds an input that violates an invariant, libFuzzer prints
the crashing input and writes it to `artifacts/<target>/crash-<hash>`.
Read that file, reproduce locally with `cargo fuzz run <target>
artifacts/<target>/crash-<hash>`, then write a unit test in
`backend/tests/` pinning the regression before fixing the bug.

The three invariant categories every target checks:

1. **No panic** - the verifier must return `Err` rather than panic on any
   malformed input. A panic in middleware is a denial-of-service primitive.
2. **Round-trip** - a signature we just computed with a given key must
   verify under that same key. If this fails, legitimate webhooks would
   be rejected in production.
3. **Tamper detection** - flipping any byte of the signed payload (or the
   key) must invalidate the signature. If this ever passes, the underlying
   crypto is broken (which would be Big News, but worth verifying).

## Adding a new target

1. Make the function under test **pure** (no I/O, no env vars, no
   `AppState`). Extract a helper if needed - see how `telnyx_utils.rs`
   exposes `verify_telnyx_signature` separately from the middleware.
2. Add a new file under `fuzz_targets/` with a `fuzz_target!` macro.
3. Add a corresponding `[[bin]]` entry to `Cargo.toml`.
4. Add a justfile recipe at the repo root.
5. Document the new target in the table above.

Prefer targets that take adversarial bytes from untrusted sources:
webhook signature verifiers, base64/url/hex decoders, OAuth callback
parsers, JWT validators, encryption round-trips.

## Kani (bounded model checking)

Where fuzzing is "probabilistic search for counterexamples," Kani is
"exhaustive symbolic execution that PROVES no counterexample exists
within a stated input bound." A Kani proof that finishes is a real
mathematical result, not an absence-of-evidence claim.

Proofs live in `backend/tests/kani_signature_proofs.rs` and are gated
by `#[cfg(kani)]` so they're invisible to `cargo test` / `cargo check`.

**One-time install** (separate from cargo-fuzz):

```bash
cargo install --locked kani-verifier
cargo kani setup           # downloads CBMC + the Kani toolchain
```

**Run all proofs**:

```bash
just kani                  # ~minutes; runs every #[kani::proof] in the crate
```

**Run a single proof** (useful when debugging timeouts):

```bash
cd backend && cargo kani --tests --harness proof_telnyx_short_public_key_rejected
```

**What the current proofs cover**:

| Proof | What it proves for ALL bounded inputs |
| ----- | -------------------------------------- |
| `proof_telnyx_short_public_key_rejected` | A pubkey shorter than 32 bytes ALWAYS returns Err("public key...") - no panic, no Ok path. |
| `proof_telnyx_long_public_key_rejected`  | Same for pubkeys longer than 32 bytes. |
| `proof_telnyx_wrong_signature_length_rejected` | Any signature length other than 64 ALWAYS returns Err. |
| `proof_twilio_invalid_base64_signature_rejected` | Any signature string containing a non-base64 byte ALWAYS returns Err. |
| `proof_twilio_empty_params_no_panic` | The verifier never panics on empty params + bounded URL/token. |

**What the current proofs do NOT cover** (and why):

- **HMAC-SHA1 / Ed25519 internals**: CBMC cannot symbolically execute
  the SHA-1 compression function or Ed25519 scalar multiplication in
  bounded time. Those primitives are covered by the audited upstream
  crates (`hmac`, `sha1`, `ed25519-dalek`) and by the fuzz harnesses,
  not by Kani.
- **Round-trip and tamper-detection**: same reason - these require
  symbolically executing the full crypto pipeline twice. The fuzz
  harnesses in `fuzz_targets/` cover them probabilistically.

**Adding a new Kani proof**:

1. Pick a property about non-crypto control flow (length checks,
   parsing, state-machine transitions, branch coverage).
2. Add a `#[kani::proof]` function in
   `backend/tests/kani_signature_proofs.rs`.
3. Use `kani::any()` for symbolic inputs, `kani::assume(...)` to bound
   the input space, and `#[kani::unwind(N)]` to bound any loops.
4. Run it. If it doesn't finish in ~5 minutes, tighten the bounds.
