use axum::http::{HeaderMap, HeaderValue};
use backend::rate_limits::{client_identity, RateLimitScope, SecurityRateLimiters};

#[test]
fn login_limit_uses_one_normalized_fixed_size_identity() {
    let limiters = SecurityRateLimiters::default();

    for email in [
        "user@example.com",
        " USER@example.com ",
        "User@Example.Com",
        "user@EXAMPLE.com",
        "USER@EXAMPLE.COM",
    ] {
        assert!(limiters.check(RateLimitScope::Login, email));
    }

    assert!(!limiters.check(RateLimitScope::Login, "user@example.com"));
}

#[test]
fn phone_limit_normalizes_common_formatting_variants() {
    let limiters = SecurityRateLimiters::default();

    assert!(limiters.check(RateLimitScope::PhoneVerifyRequest, "+1 202 555 0100"));
    assert!(limiters.check(RateLimitScope::PhoneVerifyRequest, "+1 (202) 555-0100"));
    assert!(limiters.check(RateLimitScope::PhoneVerifyRequest, "+12025550100"));
    assert!(!limiters.check(RateLimitScope::PhoneVerifyRequest, "+1-202-555-0100"));
}

#[test]
fn client_identity_prefers_cloudflare_and_uses_first_forwarded_address() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "x-forwarded-for",
        HeaderValue::from_static("192.0.2.2, 10.0.0.1"),
    );
    headers.insert("x-real-ip", HeaderValue::from_static("192.0.2.3"));
    headers.insert("cf-connecting-ip", HeaderValue::from_static("192.0.2.4"));

    assert_eq!(client_identity(&headers), "192.0.2.4");
}
