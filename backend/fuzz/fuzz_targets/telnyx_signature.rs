//! Fuzz harness for `verify_telnyx_signature`.
//!
//! Telnyx auto-signs every webhook with Ed25519 over `<timestamp>|<body>`.
//! The middleware strips base64 + length checks before calling the pure
//! verifier; this harness exercises the verifier directly so it sees
//! adversarial public keys, signatures, timestamps, and bodies.
//!
//! Invariants asserted for ALL inputs:
//!
//!   1. NO PANIC: verify_telnyx_signature never panics, even when the
//!      public key or signature are wrong length, malformed, or all-zero.
//!
//!   2. ROUND-TRIP: a signature produced by compute_telnyx_signature with
//!      a derived signing key must verify against that key's public half.
//!
//!   3. TAMPER DETECTION: flipping any single byte of the body must
//!      invalidate the signature. (Ed25519 is deterministic and collision-
//!      resistant; if this ever passes, the library is broken.)

#![no_main]

use arbitrary::Arbitrary;
use backend::api::telnyx_utils::{compute_telnyx_signature, verify_telnyx_signature};
use ed25519_dalek::SigningKey;
use libfuzzer_sys::fuzz_target;

#[derive(Arbitrary, Debug)]
struct FuzzInput {
    seed: [u8; 32],
    timestamp: String,
    body: Vec<u8>,
    attacker_public_key: Vec<u8>,
    attacker_signature: Vec<u8>,
}

fuzz_target!(|input: FuzzInput| {
    // Invariant 1: arbitrary attacker bytes must not panic the verifier.
    let _ = verify_telnyx_signature(
        &input.attacker_public_key,
        &input.attacker_signature,
        &input.timestamp,
        &input.body,
    );

    // Derive a real keypair from the seed and produce a real signature.
    let signing_key = SigningKey::from_bytes(&input.seed);
    let public_key = signing_key.verifying_key().to_bytes();
    let real_sig = compute_telnyx_signature(&signing_key, &input.timestamp, &input.body);

    // Invariant 2: real signatures must verify.
    assert!(
        verify_telnyx_signature(&public_key, &real_sig, &input.timestamp, &input.body).is_ok(),
        "round-trip failed: timestamp={:?} body_len={}",
        input.timestamp,
        input.body.len()
    );

    // Invariant 3: tampering the body must invalidate the signature.
    if !input.body.is_empty() {
        let mut tampered_body = input.body.clone();
        tampered_body[0] ^= 0xff;
        assert!(
            verify_telnyx_signature(&public_key, &real_sig, &input.timestamp, &tampered_body)
                .is_err(),
            "tampered body still verified (Ed25519 broken?)"
        );
    }

    // Invariant 3b: tampering the timestamp must invalidate the signature.
    let tampered_ts = format!("{}x", input.timestamp);
    assert!(
        verify_telnyx_signature(&public_key, &real_sig, &tampered_ts, &input.body).is_err(),
        "tampered timestamp still verified"
    );
});
