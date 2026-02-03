use axum::http::HeaderMap;
use backend::api::internal_routing::is_valid_internal_request;
use serial_test::serial;

fn clear_env() {
    std::env::remove_var("INTERNAL_ROUTING_API_KEY");
}

#[test]
#[serial]
fn test_valid_internal_request_with_matching_key() {
    std::env::set_var("INTERNAL_ROUTING_API_KEY", "secret-key-123");
    let mut headers = HeaderMap::new();
    headers.insert("X-Internal-Api-Key", "secret-key-123".parse().unwrap());

    assert!(is_valid_internal_request(&headers));
    clear_env();
}

#[test]
#[serial]
fn test_valid_internal_request_with_wrong_key() {
    std::env::set_var("INTERNAL_ROUTING_API_KEY", "secret-key-123");
    let mut headers = HeaderMap::new();
    headers.insert("X-Internal-Api-Key", "wrong-key".parse().unwrap());

    assert!(!is_valid_internal_request(&headers));
    clear_env();
}

#[test]
#[serial]
fn test_valid_internal_request_missing_header() {
    std::env::set_var("INTERNAL_ROUTING_API_KEY", "secret-key-123");
    let headers = HeaderMap::new();

    assert!(!is_valid_internal_request(&headers));
    clear_env();
}

#[test]
#[serial]
fn test_valid_internal_request_missing_env_var() {
    clear_env();
    let mut headers = HeaderMap::new();
    headers.insert("X-Internal-Api-Key", "any-key".parse().unwrap());

    assert!(!is_valid_internal_request(&headers));
}

#[test]
#[serial]
fn test_valid_internal_request_empty_env_var() {
    std::env::set_var("INTERNAL_ROUTING_API_KEY", "");
    let mut headers = HeaderMap::new();
    headers.insert("X-Internal-Api-Key", "".parse().unwrap());

    // Empty env var should return false (handled by the !key.is_empty() check)
    assert!(!is_valid_internal_request(&headers));
    clear_env();
}

#[test]
#[serial]
fn test_valid_internal_request_case_sensitivity() {
    std::env::set_var("INTERNAL_ROUTING_API_KEY", "CaseSensitiveKey");
    let mut headers = HeaderMap::new();
    headers.insert("X-Internal-Api-Key", "casesensitivekey".parse().unwrap());

    // Keys should be case-sensitive
    assert!(!is_valid_internal_request(&headers));
    clear_env();
}

#[test]
#[serial]
fn test_valid_internal_request_with_spaces_in_key() {
    std::env::set_var("INTERNAL_ROUTING_API_KEY", "key with spaces");
    let mut headers = HeaderMap::new();
    headers.insert("X-Internal-Api-Key", "key with spaces".parse().unwrap());

    assert!(is_valid_internal_request(&headers));
    clear_env();
}

#[test]
#[serial]
fn test_valid_internal_request_with_special_characters() {
    std::env::set_var("INTERNAL_ROUTING_API_KEY", "key-with_special.chars!@#");
    let mut headers = HeaderMap::new();
    headers.insert(
        "X-Internal-Api-Key",
        "key-with_special.chars!@#".parse().unwrap(),
    );

    assert!(is_valid_internal_request(&headers));
    clear_env();
}
