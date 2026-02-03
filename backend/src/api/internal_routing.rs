use axum::http::HeaderMap;

/// Check if request has valid internal routing API key
/// Returns true if this is a trusted internal request from another Lightfriend server
pub fn is_valid_internal_request(headers: &HeaderMap) -> bool {
    let expected = match std::env::var("INTERNAL_ROUTING_API_KEY") {
        Ok(key) if !key.is_empty() => key,
        _ => return false,
    };

    match headers.get("X-Internal-Api-Key") {
        Some(header) => header.to_str().map(|h| h == expected).unwrap_or(false),
        None => false,
    }
}
