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
use std::sync::Arc;

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
) -> Result<Session<TlsStream<std::net::TcpStream>>, Box<dyn Error>> {
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

    let client = imap::connect((server, port), server, &tls)?;

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
                StatusCode::UNAUTHORIZED,
                AxumJson(json!({"error": "Invalid IMAP credentials"})),
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
