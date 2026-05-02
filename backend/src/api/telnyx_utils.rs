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
use ed25519_dalek::{Signature, Verifier, VerifyingKey};

const MAX_BODY_BYTES: usize = 1024 * 1024;

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
    let public_key_arr: [u8; 32] = public_key_bytes.as_slice().try_into().map_err(|_| {
        tracing::error!(
            "TELNYX_PUBLIC_KEY decoded to {} bytes, expected 32",
            public_key_bytes.len()
        );
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let verifying_key = VerifyingKey::from_bytes(&public_key_arr).map_err(|e| {
        tracing::error!("TELNYX_PUBLIC_KEY parse failed: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let signature_bytes = BASE64.decode(signature_b64.as_bytes()).map_err(|e| {
        tracing::warn!("Telnyx signature is not valid base64: {}", e);
        StatusCode::UNAUTHORIZED
    })?;
    let signature_arr: [u8; 64] = signature_bytes.as_slice().try_into().map_err(|_| {
        tracing::warn!(
            "Telnyx signature decoded to {} bytes, expected 64",
            signature_bytes.len()
        );
        StatusCode::UNAUTHORIZED
    })?;
    let signature = Signature::from_bytes(&signature_arr);

    let mut signed_message = Vec::with_capacity(timestamp.len() + 1 + body_bytes.len());
    signed_message.extend_from_slice(timestamp.as_bytes());
    signed_message.push(b'|');
    signed_message.extend_from_slice(&body_bytes);

    if let Err(e) = verifying_key.verify(&signed_message, &signature) {
        tracing::warn!("Telnyx signature verification failed: {}", e);
        return Err(StatusCode::UNAUTHORIZED);
    }

    let new_request = Request::from_parts(parts, Body::from(body_bytes));
    Ok(next.run(new_request).await)
}
