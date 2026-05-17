//! Telnyx webhook authentication.
//!
//! Telnyx auto-signs every outbound webhook with Ed25519. They send two
//! headers on each request:
//!   - `telnyx-signature-ed25519`: base64-encoded 64-byte Ed25519 signature
//!   - `telnyx-timestamp`: unix timestamp (seconds, as a string)
//!
//! The signed payload is `<timestamp>|<raw_body>` (UTF-8 bytes). The
//! account's public key is available in Mission Control Portal (Account
//! Settings -> Public Key) as a base64-encoded 32-byte Ed25519 key.
//!
//! If `TELNYX_PUBLIC_KEY` is unset or empty, Telnyx is considered "off"
//! and every webhook request returns 401. This is the env-var switch
//! that gates the whole Telnyx inbound surface.

use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    middleware,
    response::Response,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};

const MAX_BODY_BYTES: usize = 1024 * 1024;

/// Pure function: verify an Ed25519 signature on a Telnyx-style payload.
///
/// The signed message is `<timestamp>|<body>` (UTF-8 bytes). `public_key`
/// must be exactly 32 bytes (raw Ed25519 public key) and `signature` must
/// be exactly 64 bytes. Returns Ok(()) only if the signature is valid for
/// the supplied public key and payload.
///
/// This function performs no I/O and reads no environment variables. It's
/// the verification core used by [`validate_telnyx_signature`] and is the
/// target of the `telnyx_signature` fuzz harness.
pub fn verify_telnyx_signature(
    public_key: &[u8],
    signature: &[u8],
    timestamp: &str,
    body: &[u8],
) -> Result<(), String> {
    let public_key_arr: [u8; 32] = public_key.try_into().map_err(|_| {
        format!(
            "public key wrong length: expected 32, got {}",
            public_key.len()
        )
    })?;
    let verifying_key = VerifyingKey::from_bytes(&public_key_arr)
        .map_err(|e| format!("public key parse failed: {}", e))?;

    let signature_arr: [u8; 64] = signature.try_into().map_err(|_| {
        format!(
            "signature wrong length: expected 64, got {}",
            signature.len()
        )
    })?;
    let signature = Signature::from_bytes(&signature_arr);

    let mut signed_message = Vec::with_capacity(timestamp.len() + 1 + body.len());
    signed_message.extend_from_slice(timestamp.as_bytes());
    signed_message.push(b'|');
    signed_message.extend_from_slice(body);

    verifying_key
        .verify(&signed_message, &signature)
        .map_err(|e| format!("signature verification failed: {}", e))
}

/// Pure function: compute the Ed25519 signature for a Telnyx-style payload.
/// Useful for tests and fuzz harnesses that need to produce known-good
/// signatures.
pub fn compute_telnyx_signature(
    signing_key: &SigningKey,
    timestamp: &str,
    body: &[u8],
) -> [u8; 64] {
    let mut signed_message = Vec::with_capacity(timestamp.len() + 1 + body.len());
    signed_message.extend_from_slice(timestamp.as_bytes());
    signed_message.push(b'|');
    signed_message.extend_from_slice(body);
    signing_key.sign(&signed_message).to_bytes()
}

/// Verify Ed25519 signature on Telnyx webhook requests. Reads the body,
/// validates `telnyx-signature-ed25519` against `<telnyx-timestamp>|<body>`
/// using the public key from `TELNYX_PUBLIC_KEY`, then re-injects the body
/// so downstream handlers can parse it normally.
pub async fn validate_telnyx_signature(
    request: Request<Body>,
    next: middleware::Next,
) -> Result<Response, StatusCode> {
    let public_key_b64 = match std::env::var("TELNYX_PUBLIC_KEY") {
        Ok(s) if !s.is_empty() => s,
        _ => {
            tracing::error!("Telnyx webhook hit with TELNYX_PUBLIC_KEY unset");
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    let (parts, body) = request.into_parts();

    let signature_b64 = parts
        .headers
        .get("telnyx-signature-ed25519")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| {
            tracing::warn!("Telnyx webhook missing telnyx-signature-ed25519 header");
            StatusCode::UNAUTHORIZED
        })?
        .to_string();

    let timestamp = parts
        .headers
        .get("telnyx-timestamp")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| {
            tracing::warn!("Telnyx webhook missing telnyx-timestamp header");
            StatusCode::UNAUTHORIZED
        })?
        .to_string();

    let body_bytes = to_bytes(body, MAX_BODY_BYTES).await.map_err(|e| {
        tracing::warn!("Telnyx webhook body read failed: {}", e);
        StatusCode::BAD_REQUEST
    })?;

    let public_key_bytes = BASE64.decode(public_key_b64.as_bytes()).map_err(|e| {
        tracing::error!("TELNYX_PUBLIC_KEY is not valid base64: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let signature_bytes = BASE64.decode(signature_b64.as_bytes()).map_err(|e| {
        tracing::warn!("Telnyx signature is not valid base64: {}", e);
        StatusCode::UNAUTHORIZED
    })?;

    if let Err(e) =
        verify_telnyx_signature(&public_key_bytes, &signature_bytes, &timestamp, &body_bytes)
    {
        // Length / parse problems on the env-supplied public key are
        // a config error (500); everything else is auth failure (401).
        if e.starts_with("public key") {
            tracing::error!("Telnyx public key invalid: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
        tracing::warn!("Telnyx signature rejected: {}", e);
        return Err(StatusCode::UNAUTHORIZED);
    }

    let new_request = Request::from_parts(parts, Body::from(body_bytes));
    Ok(next.run(new_request).await)
}
