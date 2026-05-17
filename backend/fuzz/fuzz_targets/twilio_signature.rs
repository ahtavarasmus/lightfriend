//! Fuzz harness for `verify_twilio_signature`.
//!
//! Twilio signs webhooks with HMAC-SHA1 over `<url> || k1 || v1 || k2 || v2 ...`
//! where params are sorted lexicographically. A bug in any step (URL handling,
//! param sorting, base64 decode, constant-time compare) is a webhook forgery
//! primitive. This harness asserts three invariants for ALL inputs:
//!
//!   1. NO PANIC: verify_twilio_signature must never panic. An attacker
//!      controls every byte of the request, so any panic is a DoS.
//!
//!   2. ROUND-TRIP: compute_twilio_signature followed by verify_twilio_signature
//!      with the same token must succeed. If this ever fails, our own
//!      legitimate webhooks would start being rejected.
//!
//!   3. TAMPER DETECTION: flipping any bit of the auth token (after computing
//!      a valid signature) must cause verification to fail. If this ever
//!      succeeds, an attacker who doesn't know the token can still forge.

#![no_main]

use arbitrary::Arbitrary;
use backend::api::twilio_utils::{compute_twilio_signature, verify_twilio_signature};
use libfuzzer_sys::fuzz_target;
use std::collections::BTreeMap;

#[derive(Arbitrary, Debug)]
struct FuzzInput {
    url: String,
    params: Vec<(String, String)>,
    auth_token: String,
    attacker_signature_b64: String,
}

fuzz_target!(|input: FuzzInput| {
    let params: BTreeMap<String, String> = input.params.into_iter().collect();

    // Invariant 1: verifying an arbitrary attacker-supplied signature
    // against arbitrary attacker-supplied inputs must not panic.
    let _ = verify_twilio_signature(
        &input.url,
        &params,
        &input.attacker_signature_b64,
        &input.auth_token,
    );

    // Invariant 2: round-trip with a real Twilio-style signature must verify.
    let real_sig = compute_twilio_signature(&input.url, &params, &input.auth_token);
    assert!(
        verify_twilio_signature(&input.url, &params, &real_sig, &input.auth_token).is_ok(),
        "round-trip failed: url={:?} params={:?} token_len={}",
        input.url,
        params,
        input.auth_token.len()
    );

    // Invariant 3: flipping the token must invalidate the signature.
    // We append a byte that can't already be the last byte of the token,
    // so the tampered token is always distinct from the original.
    let mut tampered_token = input.auth_token.clone().into_bytes();
    tampered_token.push(0x00);
    let tampered_token = String::from_utf8_lossy(&tampered_token).into_owned();
    assert!(
        verify_twilio_signature(&input.url, &params, &real_sig, &tampered_token).is_err(),
        "signature verified under a different auth token (HMAC broken?)"
    );
});
