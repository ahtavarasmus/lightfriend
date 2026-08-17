use axum::http::{header::COOKIE, HeaderMap, HeaderValue};
use backend::handlers::stripe_handlers::{
    datafast_checkout_success_url, datafast_metadata_from_headers,
    is_valid_stripe_checkout_session_id,
};

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
fn adds_checkout_session_id_to_success_urls() {
    assert_eq!(
        datafast_checkout_success_url("https://lightfriend.ai/", "/subscription-success"),
        "https://lightfriend.ai/subscription-success?session_id={CHECKOUT_SESSION_ID}"
    );
    assert_eq!(
        datafast_checkout_success_url("https://lightfriend.ai", "/?subscription=success"),
        "https://lightfriend.ai/?subscription=success&session_id={CHECKOUT_SESSION_ID}"
    );
}

#[test]
fn validates_checkout_session_ids_before_calling_stripe() {
    assert!(is_valid_stripe_checkout_session_id("cs_live_a1B2C3_d4E5F6"));
    assert!(!is_valid_stripe_checkout_session_id("pi_not_a_session"));
    assert!(!is_valid_stripe_checkout_session_id("cs_bad-value"));
    assert!(!is_valid_stripe_checkout_session_id("cs_"));
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

#[test]
fn subscription_success_surfaces_missing_pricing_table_session_ids() {
    let subscription_success = include_str!("../../frontend/src/pages/subscription_success.rs");

    assert!(subscription_success.contains("checkout_attribution_missing_session_id"));
    assert!(subscription_success.contains("missing_session_id"));
}
