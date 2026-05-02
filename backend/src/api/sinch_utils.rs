//! Sinch callback authentication.
//!
//! Sinch's SMS REST API supports configuring a static `Authorization` header
//! on the callback URL via the dashboard ("Callback URL Authentication"
//! → "Bearer Token"). We treat that as a shared secret: every inbound MO
//! and delivery-report POST must arrive with `Authorization: Bearer <SINCH_CALLBACK_SECRET>`,
//! constant-time compared against the env-configured value.
//!
//! If `SINCH_CALLBACK_SECRET` is unset or empty, Sinch is considered "off"
//! and every webhook request is rejected with 401. This is the env-var
//! switch that gates the whole Sinch inbound surface.

use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware,
    response::Response,
};

/// Constant-time byte comparison. Length-leak is acceptable; byte values
/// don't leak. Avoids pulling in `subtle` for a single comparison.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Validate `Authorization: Bearer <SINCH_CALLBACK_SECRET>` on Sinch
/// webhook requests. Returns 401 when the env var is unset, the header
/// is missing/malformed, or the bearer doesn't match.
pub async fn validate_sinch_auth(
    request: Request<Body>,
    next: middleware::Next,
) -> Result<Response, StatusCode> {
    let expected = match std::env::var("SINCH_CALLBACK_SECRET") {
        Ok(s) if !s.is_empty() => s,
        _ => {
            tracing::error!("Sinch webhook hit with SINCH_CALLBACK_SECRET unset");
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    let header_value = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| {
            tracing::warn!("Sinch webhook missing or non-ASCII Authorization header");
            StatusCode::UNAUTHORIZED
        })?;

    let token = header_value
        .strip_prefix("Bearer ")
        .or_else(|| header_value.strip_prefix("bearer "))
        .ok_or_else(|| {
            tracing::warn!("Sinch webhook Authorization header missing Bearer prefix");
            StatusCode::UNAUTHORIZED
        })?;

    if !constant_time_eq(token.as_bytes(), expected.as_bytes()) {
        tracing::warn!("Sinch webhook bearer token mismatch");
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(next.run(request).await)
}
