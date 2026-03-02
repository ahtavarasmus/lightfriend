use crate::{handlers::auth_middleware::AuthUser, AppState};
use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::Json as AxumJson,
};
use imap::Session;
use native_tls::TlsConnector;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::error::Error;
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

// ============================================================
// Pure Functions for Testability
// ============================================================

/// Validate email format using basic rules.
/// Returns true if email appears to be valid format.
/// Note: This is a basic validation, not a full RFC 5322 check.
pub fn is_valid_email(email: &str) -> bool {
    // Check for exactly one @ symbol
    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 {
        return false;
    }

    let local_part = parts[0];
    let domain_part = parts[1];

    // Local part cannot be empty
    if local_part.is_empty() {
        return false;
    }

    // Domain must contain at least one dot
    if !domain_part.contains('.') {
        return false;
    }

    // Domain cannot start or end with a dot
    if domain_part.starts_with('.') || domain_part.ends_with('.') {
        return false;
    }

    // Domain parts cannot be empty (no consecutive dots)
    let domain_parts: Vec<&str> = domain_part.split('.').collect();
    if domain_parts.iter().any(|p| p.is_empty()) {
        return false;
    }

    true
}

/// Extract domain from email address.
/// Returns None if email is invalid.
pub fn extract_email_domain(email: &str) -> Option<&str> {
    email.split('@').nth(1)
}

/// IMAP server configuration
#[derive(Debug, Clone, PartialEq)]
pub struct ImapConfig {
    pub host: &'static str,
    pub port: u16,
    pub tls: bool,
}

/// Detect IMAP server configuration from email address.
/// Returns ImapConfig with server details, or None if unknown provider.
/// This is an alternative to detect_imap_server that returns a struct.
pub fn detect_imap_config(email: &str) -> Option<ImapConfig> {
    let domain = extract_email_domain(email)?.to_lowercase();

    let config = match domain.as_str() {
        "icloud.com" | "me.com" | "mac.com" => ImapConfig {
            host: "imap.mail.me.com",
            port: 993,
            tls: true,
        },
        "gmail.com" | "googlemail.com" => ImapConfig {
            host: "imap.gmail.com",
            port: 993,
            tls: true,
        },
        "outlook.com" | "hotmail.com" | "live.com" | "msn.com" => ImapConfig {
            host: "outlook.office365.com",
            port: 993,
            tls: true,
        },
        "yahoo.com" | "yahoo.co.uk" | "yahoo.fr" | "yahoo.de" => ImapConfig {
            host: "imap.mail.yahoo.com",
            port: 993,
            tls: true,
        },
        "aol.com" => ImapConfig {
            host: "imap.aol.com",
            port: 993,
            tls: true,
        },
        "zoho.com" | "zohomail.com" => ImapConfig {
            host: "imap.zoho.com",
            port: 993,
            tls: true,
        },
        "fastmail.com" | "fastmail.fm" => ImapConfig {
            host: "imap.fastmail.com",
            port: 993,
            tls: true,
        },
        _ => return None,
    };

    Some(config)
}

/// Check if an email provider is a known supported provider.
pub fn is_known_email_provider(email: &str) -> bool {
    detect_imap_config(email).is_some()
}

// Struct to deserialize the incoming IMAP credentials from the frontend
#[derive(Deserialize)]
pub struct ImapCredentials {
    email: String,
    password: String,
    #[serde(default)]
    imap_server: Option<String>, // e.g., "mail.privateemail.com" or "imap.gmail.com"
    #[serde(default)]
    imap_port: Option<u16>, // e.g., 993
}

/// Detects the IMAP server based on email domain.
/// Returns (server, port) tuple.
pub fn detect_imap_server(email: &str) -> (&'static str, u16) {
    let domain = email.split('@').nth(1).unwrap_or("").to_lowercase();

    match domain.as_str() {
        // iCloud
        "icloud.com" | "me.com" | "mac.com" => ("imap.mail.me.com", 993),
        // Google
        "gmail.com" | "googlemail.com" => ("imap.gmail.com", 993),
        // Microsoft
        "outlook.com" | "hotmail.com" | "live.com" | "msn.com" => ("outlook.office365.com", 993),
        // Yahoo
        "yahoo.com" | "yahoo.co.uk" | "yahoo.fr" | "yahoo.de" => ("imap.mail.yahoo.com", 993),
        // AOL
        "aol.com" => ("imap.aol.com", 993),
        // Zoho
        "zoho.com" | "zohomail.com" => ("imap.zoho.com", 993),
        // FastMail
        "fastmail.com" | "fastmail.fm" => ("imap.fastmail.com", 993),
        // Default to Gmail (legacy behavior, but log warning)
        _ => ("imap.gmail.com", 993),
    }
}

// Struct to serialize the IMAP status response
#[derive(Serialize)]
pub struct ImapStatus {
    connected: bool,
    email: Option<String>,
}

use native_tls::TlsStream;

// Function to establish an IMAP connection for credential verification
async fn connect_imap(
    email: &str,
    password: &str,
    imap_server: Option<&str>,
    imap_port: Option<u16>,
) -> Result<Session<TlsStream<TcpStream>>, Box<dyn Error>> {
    let tls = TlsConnector::builder().build()?;

    // Use provided server/port or auto-detect from email domain
    let (detected_server, detected_port) = detect_imap_server(email);
    let server = imap_server.unwrap_or(detected_server);
    let port = imap_port.unwrap_or(detected_port);

    tracing::debug!(
        "Connecting to IMAP server {} on port {} for email {}",
        server,
        port,
        email
    );

    // Use TCP connect with timeout to avoid hanging on unreachable servers
    let tcp_stream = TcpStream::connect_timeout(
        &format!("{}:{}", server, port).parse()?,
        Duration::from_secs(15),
    )?;
    tcp_stream.set_read_timeout(Some(Duration::from_secs(15)))?;
    tcp_stream.set_write_timeout(Some(Duration::from_secs(15)))?;

    let tls_stream = tls.connect(server, tcp_stream)?;
    let client = imap::Client::new(tls_stream);

    match client.login(email, password) {
        Ok(session) => Ok(session),
        Err((err, _orig_client)) => Err(Box::new(err)),
    }
}

// Handler to authenticate and store Gmail IMAP credentials
pub async fn imap_login(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(payload): Json<ImapCredentials>,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::info!(
        "Received request to /api/auth/gmail/imap/login for user {}",
        auth_user.user_id
    );

    let email = payload.email;
    let password = payload.password;
    let imap_server = payload.imap_server.as_deref(); // Convert Option<String> to Option<&str>
    let imap_port = payload.imap_port;

    // Attempt to connect to Gmail's IMAP server to verify credentials
    match connect_imap(&email, &password, imap_server, imap_port).await {
        Ok(mut session) => {
            // Logout immediately after verification to avoid keeping the session open
            if let Err(e) = session.logout() {
                tracing::warn!("Failed to logout IMAP session: {}", e);
            }

            if let Err(e) = state.user_repository.set_imap_credentials(
                auth_user.user_id,
                &email,
                &password,
                imap_server,
                imap_port,
            ) {
                tracing::error!("Failed to store IMAP credentials: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    AxumJson(json!({"error": "Failed to store IMAP credentials"})),
                ));
            }

            tracing::info!(
                "Successfully stored IMAP credentials for user {}",
                auth_user.user_id
            );
            Ok(AxumJson(json!({"message": "IMAP connected successfully"})))
        }
        Err(e) => {
            tracing::error!(
                "IMAP connection failed for user {}: {}",
                auth_user.user_id,
                e
            );
            Err((
                StatusCode::BAD_REQUEST,
                AxumJson(json!({"error": format!("IMAP connection failed: {}", e)})),
            ))
        }
    }
}

// Handler to check the IMAP connection status
pub async fn imap_status(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<ImapStatus>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::info!("Checking IMAP status for user {}", auth_user.user_id);

    let credentials = state
        .user_repository
        .get_imap_credentials(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Failed to fetch IMAP credentials: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to fetch IMAP status"})),
            )
        })?;

    match credentials {
        Some((email, password, imap_server, imap_port)) => {
            // Actually test the connection instead of just checking if credentials exist
            tracing::debug!("Testing IMAP connection for user {}", auth_user.user_id);

            match connect_imap(
                &email,
                &password,
                imap_server.as_deref(),
                imap_port.map(|val| val as u16),
            )
            .await
            {
                Ok(mut session) => {
                    // Logout immediately after verification
                    if let Err(e) = session.logout() {
                        tracing::warn!("Failed to logout IMAP session during status check: {}", e);
                    }

                    tracing::info!(
                        "IMAP connection test successful for user {}",
                        auth_user.user_id
                    );
                    Ok(Json(ImapStatus {
                        connected: true,
                        email: Some(email),
                    }))
                }
                Err(e) => {
                    tracing::error!(
                        "IMAP connection test failed for user {}: {}",
                        auth_user.user_id,
                        e
                    );
                    // Return connected: false if test fails, so frontend shows accurate status
                    Ok(Json(ImapStatus {
                        connected: false,
                        email: Some(email),
                    }))
                }
            }
        }
        None => Ok(Json(ImapStatus {
            connected: false,
            email: None,
        })),
    }
}

// Handler to delete the IMAP connection
pub async fn delete_imap_connection(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::info!(
        "Received request to delete IMAP connection for user {}",
        auth_user.user_id
    );

    if let Err(e) = state
        .user_repository
        .delete_imap_credentials(auth_user.user_id)
    {
        tracing::error!("Failed to delete IMAP credentials: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({"error": "Failed to delete IMAP credentials"})),
        ));
    }

    tracing::info!(
        "Successfully deleted IMAP connection for user {}",
        auth_user.user_id
    );
    Ok(AxumJson(
        json!({"message": "IMAP connection deleted successfully"}),
    ))
}
