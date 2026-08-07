use axum::http::{header::COOKIE, HeaderMap, HeaderValue};
use backend::handlers::stripe_handlers::datafast_metadata_from_headers;

#[test]
fn extracts_datafast_cookies_for_stripe_metadata() {
    let mut headers = HeaderMap::new();
    headers.insert(
        COOKIE,
        HeaderValue::from_static(
            "other=value; datafast_visitor_id=visitor-123; datafast_session_id=\"session=456\"",
        ),
    );

    let metadata = datafast_metadata_from_headers(&headers);

    assert_eq!(metadata.len(), 2);
    assert_eq!(
        metadata.get("datafast_visitor_id").map(String::as_str),
        Some("visitor-123")
    );
    assert_eq!(
        metadata.get("datafast_session_id").map(String::as_str),
        Some("session=456")
    );
}

#[test]
fn ignores_missing_empty_and_oversized_datafast_cookies() {
    let oversized = "x".repeat(501);
    let mut headers = HeaderMap::new();
    headers.insert(
        COOKIE,
        HeaderValue::from_str(&format!(
            "datafast_visitor_id=; datafast_session_id={oversized}; unrelated=cookie"
        ))
        .expect("valid cookie header"),
    );

    let metadata = datafast_metadata_from_headers(&headers);

    assert!(metadata.is_empty());
}

#[test]
fn uses_the_latest_datafast_cookie_value_across_headers() {
    let mut headers = HeaderMap::new();
    headers.append(
        COOKIE,
        HeaderValue::from_static("datafast_visitor_id=visitor-old"),
    );
    headers.append(
        COOKIE,
        HeaderValue::from_static(
            "datafast_visitor_id=visitor-new; datafast_session_id=session-new",
        ),
    );

    let metadata = datafast_metadata_from_headers(&headers);

    assert_eq!(
        metadata.get("datafast_visitor_id").map(String::as_str),
        Some("visitor-new")
    );
    assert_eq!(
        metadata.get("datafast_session_id").map(String::as_str),
        Some("session-new")
    );
}
