//! Migration proxy utilities for VPS to AWS gradual migration.
//!
//! This module provides functionality to forward webhook requests to the old VPS
//! server for users who haven't migrated yet.

use reqwest::Client;
use serde::Serialize;
use std::sync::LazyLock;

static HTTP_CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client")
});

#[derive(Debug)]
pub enum ProxyError {
    MissingOldVpsUrl,
    MissingApiKey,
    RequestFailed(String),
}

impl std::fmt::Display for ProxyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProxyError::MissingOldVpsUrl => write!(f, "OLD_VPS_URL environment variable not set"),
            ProxyError::MissingApiKey => {
                write!(f, "INTERNAL_ROUTING_API_KEY environment variable not set")
            }
            ProxyError::RequestFailed(msg) => write!(f, "Proxy request failed: {}", msg),
        }
    }
}

impl std::error::Error for ProxyError {}

/// Proxy a JSON request to the old VPS server.
/// Used during migration to forward webhooks for users who haven't migrated yet.
pub async fn proxy_to_old_vps<T: Serialize>(
    path: &str,
    payload: &T,
) -> Result<reqwest::Response, ProxyError> {
    let old_vps_url = std::env::var("OLD_VPS_URL").map_err(|_| ProxyError::MissingOldVpsUrl)?;
    let api_key =
        std::env::var("INTERNAL_ROUTING_API_KEY").map_err(|_| ProxyError::MissingApiKey)?;

    let url = format!("{}{}", old_vps_url.trim_end_matches('/'), path);

    tracing::info!("Proxying request to old VPS: {}", url);

    HTTP_CLIENT
        .post(&url)
        .header("X-Internal-Api-Key", api_key)
        .header("Content-Type", "application/json")
        .json(payload)
        .send()
        .await
        .map_err(|e| ProxyError::RequestFailed(e.to_string()))
}

/// Proxy a form-urlencoded request to the old VPS server.
/// Used for Twilio webhooks which use form encoding.
pub async fn proxy_form_to_old_vps(
    path: &str,
    body: &str,
) -> Result<reqwest::Response, ProxyError> {
    let old_vps_url = std::env::var("OLD_VPS_URL").map_err(|_| ProxyError::MissingOldVpsUrl)?;
    let api_key =
        std::env::var("INTERNAL_ROUTING_API_KEY").map_err(|_| ProxyError::MissingApiKey)?;

    let url = format!("{}{}", old_vps_url.trim_end_matches('/'), path);

    tracing::info!("Proxying form request to old VPS: {}", url);

    HTTP_CLIENT
        .post(&url)
        .header("X-Internal-Api-Key", api_key)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body.to_string())
        .send()
        .await
        .map_err(|e| ProxyError::RequestFailed(e.to_string()))
}

/// Proxy raw bytes to the old VPS server.
/// Used for JSON payloads where we want to preserve the exact original bytes.
pub async fn proxy_bytes_to_old_vps(
    path: &str,
    body: &[u8],
) -> Result<reqwest::Response, ProxyError> {
    let old_vps_url = std::env::var("OLD_VPS_URL").map_err(|_| ProxyError::MissingOldVpsUrl)?;
    let api_key =
        std::env::var("INTERNAL_ROUTING_API_KEY").map_err(|_| ProxyError::MissingApiKey)?;

    let url = format!("{}{}", old_vps_url.trim_end_matches('/'), path);

    tracing::info!("Proxying bytes request to old VPS: {}", url);

    HTTP_CLIENT
        .post(&url)
        .header("X-Internal-Api-Key", api_key)
        .header("Content-Type", "application/json")
        .body(body.to_vec())
        .send()
        .await
        .map_err(|e| ProxyError::RequestFailed(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use wiremock::matchers::{body_string, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn clear_env() {
        std::env::remove_var("OLD_VPS_URL");
        std::env::remove_var("INTERNAL_ROUTING_API_KEY");
    }

    // ========================
    // ProxyError Display tests
    // ========================

    #[test]
    fn test_proxy_error_display_missing_old_vps_url() {
        let error = ProxyError::MissingOldVpsUrl;
        assert_eq!(
            error.to_string(),
            "OLD_VPS_URL environment variable not set"
        );
    }

    #[test]
    fn test_proxy_error_display_missing_api_key() {
        let error = ProxyError::MissingApiKey;
        assert_eq!(
            error.to_string(),
            "INTERNAL_ROUTING_API_KEY environment variable not set"
        );
    }

    #[test]
    fn test_proxy_error_display_request_failed() {
        let error = ProxyError::RequestFailed("connection refused".to_string());
        assert_eq!(
            error.to_string(),
            "Proxy request failed: connection refused"
        );
    }

    // ====================================
    // Environment variable error tests
    // ====================================

    #[tokio::test]
    #[serial]
    async fn test_proxy_to_old_vps_missing_old_vps_url() {
        clear_env();
        std::env::set_var("INTERNAL_ROUTING_API_KEY", "some-key");

        let result = proxy_to_old_vps("/test", &serde_json::json!({"test": "value"})).await;

        assert!(matches!(result, Err(ProxyError::MissingOldVpsUrl)));
        clear_env();
    }

    #[tokio::test]
    #[serial]
    async fn test_proxy_to_old_vps_missing_api_key() {
        clear_env();
        std::env::set_var("OLD_VPS_URL", "http://localhost:9999");

        let result = proxy_to_old_vps("/test", &serde_json::json!({"test": "value"})).await;

        assert!(matches!(result, Err(ProxyError::MissingApiKey)));
        clear_env();
    }

    #[tokio::test]
    #[serial]
    async fn test_proxy_form_to_old_vps_missing_old_vps_url() {
        clear_env();
        std::env::set_var("INTERNAL_ROUTING_API_KEY", "some-key");

        let result = proxy_form_to_old_vps("/test", "From=%2B1234567890").await;

        assert!(matches!(result, Err(ProxyError::MissingOldVpsUrl)));
        clear_env();
    }

    #[tokio::test]
    #[serial]
    async fn test_proxy_form_to_old_vps_missing_api_key() {
        clear_env();
        std::env::set_var("OLD_VPS_URL", "http://localhost:9999");

        let result = proxy_form_to_old_vps("/test", "From=%2B1234567890").await;

        assert!(matches!(result, Err(ProxyError::MissingApiKey)));
        clear_env();
    }

    #[tokio::test]
    #[serial]
    async fn test_proxy_bytes_to_old_vps_missing_old_vps_url() {
        clear_env();
        std::env::set_var("INTERNAL_ROUTING_API_KEY", "some-key");

        let result = proxy_bytes_to_old_vps("/test", b"{\"test\": \"value\"}").await;

        assert!(matches!(result, Err(ProxyError::MissingOldVpsUrl)));
        clear_env();
    }

    #[tokio::test]
    #[serial]
    async fn test_proxy_bytes_to_old_vps_missing_api_key() {
        clear_env();
        std::env::set_var("OLD_VPS_URL", "http://localhost:9999");

        let result = proxy_bytes_to_old_vps("/test", b"{\"test\": \"value\"}").await;

        assert!(matches!(result, Err(ProxyError::MissingApiKey)));
        clear_env();
    }

    // ====================================
    // Successful proxy tests with wiremock
    // ====================================

    #[tokio::test]
    #[serial]
    async fn test_proxy_to_old_vps_success() {
        clear_env();
        let mock_server = MockServer::start().await;

        std::env::set_var("OLD_VPS_URL", mock_server.uri());
        std::env::set_var("INTERNAL_ROUTING_API_KEY", "test-api-key");

        Mock::given(method("POST"))
            .and(path("/api/test"))
            .and(header("Content-Type", "application/json"))
            .and(header("X-Internal-Api-Key", "test-api-key"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"status": "ok"})),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let payload = serde_json::json!({"user_id": 123, "message": "hello"});
        let result = proxy_to_old_vps("/api/test", &payload).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.status(), 200);

        clear_env();
    }

    #[tokio::test]
    #[serial]
    async fn test_proxy_to_old_vps_sends_correct_json_body() {
        clear_env();
        let mock_server = MockServer::start().await;

        std::env::set_var("OLD_VPS_URL", mock_server.uri());
        std::env::set_var("INTERNAL_ROUTING_API_KEY", "test-api-key");

        // Use wiremock's body_json matcher which handles JSON comparison properly
        Mock::given(method("POST"))
            .and(path("/api/webhook"))
            .and(wiremock::matchers::body_json(
                serde_json::json!({"user_id": 42, "action": "test"}),
            ))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let payload = serde_json::json!({"user_id": 42, "action": "test"});
        let result = proxy_to_old_vps("/api/webhook", &payload).await;

        assert!(result.is_ok());
        clear_env();
    }

    #[tokio::test]
    #[serial]
    async fn test_proxy_form_to_old_vps_success() {
        clear_env();
        let mock_server = MockServer::start().await;

        std::env::set_var("OLD_VPS_URL", mock_server.uri());
        std::env::set_var("INTERNAL_ROUTING_API_KEY", "test-api-key");

        Mock::given(method("POST"))
            .and(path("/api/sms/server"))
            .and(header("Content-Type", "application/x-www-form-urlencoded"))
            .and(header("X-Internal-Api-Key", "test-api-key"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<Response></Response>"))
            .expect(1)
            .mount(&mock_server)
            .await;

        let form_body = "From=%2B15551234567&Body=Hello";
        let result = proxy_form_to_old_vps("/api/sms/server", form_body).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.status(), 200);

        clear_env();
    }

    #[tokio::test]
    #[serial]
    async fn test_proxy_form_to_old_vps_sends_correct_body() {
        clear_env();
        let mock_server = MockServer::start().await;

        std::env::set_var("OLD_VPS_URL", mock_server.uri());
        std::env::set_var("INTERNAL_ROUTING_API_KEY", "test-api-key");

        let form_body = "AccountSid=AC123&From=%2B15551234567&Body=Test+message";

        Mock::given(method("POST"))
            .and(path("/api/sms/server"))
            .and(body_string(form_body))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let result = proxy_form_to_old_vps("/api/sms/server", form_body).await;

        assert!(result.is_ok());
        clear_env();
    }

    #[tokio::test]
    #[serial]
    async fn test_proxy_bytes_to_old_vps_success() {
        clear_env();
        let mock_server = MockServer::start().await;

        std::env::set_var("OLD_VPS_URL", mock_server.uri());
        std::env::set_var("INTERNAL_ROUTING_API_KEY", "test-api-key");

        Mock::given(method("POST"))
            .and(path("/api/webhook/elevenlabs"))
            .and(header("Content-Type", "application/json"))
            .and(header("X-Internal-Api-Key", "test-api-key"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"status": "received"})),
            )
            .expect(1)
            .mount(&mock_server)
            .await;

        let raw_bytes = b"{\"type\":\"post_call_transcription\",\"data\":{}}";
        let result = proxy_bytes_to_old_vps("/api/webhook/elevenlabs", raw_bytes).await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.status(), 200);

        clear_env();
    }

    #[tokio::test]
    #[serial]
    async fn test_proxy_bytes_to_old_vps_sends_exact_bytes() {
        clear_env();
        let mock_server = MockServer::start().await;

        std::env::set_var("OLD_VPS_URL", mock_server.uri());
        std::env::set_var("INTERNAL_ROUTING_API_KEY", "test-api-key");

        let raw_bytes = b"{\"exact\":\"bytes\",\"num\":123}";

        Mock::given(method("POST"))
            .and(path("/api/webhook"))
            .and(body_string(std::str::from_utf8(raw_bytes).unwrap()))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let result = proxy_bytes_to_old_vps("/api/webhook", raw_bytes).await;

        assert!(result.is_ok());
        clear_env();
    }

    // ====================================
    // URL construction tests
    // ====================================

    #[tokio::test]
    #[serial]
    async fn test_url_construction_with_trailing_slash() {
        clear_env();
        let mock_server = MockServer::start().await;

        // Set URL WITH trailing slash
        std::env::set_var("OLD_VPS_URL", format!("{}/", mock_server.uri()));
        std::env::set_var("INTERNAL_ROUTING_API_KEY", "test-key");

        // Expect the path to be /api/test (no double slash)
        Mock::given(method("POST"))
            .and(path("/api/test"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let result = proxy_to_old_vps("/api/test", &serde_json::json!({})).await;

        assert!(result.is_ok());
        clear_env();
    }

    #[tokio::test]
    #[serial]
    async fn test_url_construction_without_trailing_slash() {
        clear_env();
        let mock_server = MockServer::start().await;

        // Set URL WITHOUT trailing slash
        std::env::set_var("OLD_VPS_URL", mock_server.uri());
        std::env::set_var("INTERNAL_ROUTING_API_KEY", "test-key");

        Mock::given(method("POST"))
            .and(path("/api/test"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let result = proxy_to_old_vps("/api/test", &serde_json::json!({})).await;

        assert!(result.is_ok());
        clear_env();
    }

    // ====================================
    // Error response tests
    // ====================================

    #[tokio::test]
    #[serial]
    async fn test_proxy_returns_error_status_from_old_vps() {
        clear_env();
        let mock_server = MockServer::start().await;

        std::env::set_var("OLD_VPS_URL", mock_server.uri());
        std::env::set_var("INTERNAL_ROUTING_API_KEY", "test-key");

        Mock::given(method("POST"))
            .and(path("/api/test"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .expect(1)
            .mount(&mock_server)
            .await;

        let result = proxy_to_old_vps("/api/test", &serde_json::json!({})).await;

        assert!(result.is_ok()); // The proxy itself succeeded
        let response = result.unwrap();
        assert_eq!(response.status(), 500); // But old VPS returned 500

        clear_env();
    }

    #[tokio::test]
    #[serial]
    async fn test_proxy_handles_network_error() {
        clear_env();

        // Point to a non-existent server
        std::env::set_var("OLD_VPS_URL", "http://127.0.0.1:59999");
        std::env::set_var("INTERNAL_ROUTING_API_KEY", "test-key");

        let result = proxy_to_old_vps("/api/test", &serde_json::json!({})).await;

        assert!(matches!(result, Err(ProxyError::RequestFailed(_))));
        if let Err(ProxyError::RequestFailed(msg)) = result {
            // Should contain some error message about connection
            assert!(!msg.is_empty());
        }

        clear_env();
    }
}
