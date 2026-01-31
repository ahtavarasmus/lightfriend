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
