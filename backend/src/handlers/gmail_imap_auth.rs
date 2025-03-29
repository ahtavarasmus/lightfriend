use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::{IntoResponse, Json as AxumJson},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use crate::{
    AppState,
    handlers::auth_middleware::AuthUser,
    utils::encryption::{encrypt_token, decrypt_token},
};
use imap::Session;
use native_tls::TlsConnector;
use std::error::Error;

// Struct to deserialize the incoming IMAP credentials from the frontend
#[derive(Deserialize)]
pub struct ImapCredentials {
    email: String,
    password: String,
}

// Struct to serialize the IMAP status response
#[derive(Serialize)]
pub struct ImapStatus {
    connected: bool,
}

use native_tls::TlsStream;

// Function to establish an IMAP connection to Gmail for credential verification
async fn connect_imap(email: &str, password: &str) -> Result<Session<TlsStream<std::net::TcpStream>>, Box<dyn Error>> {
    let tls = TlsConnector::builder().build()?;
    
    let client = imap::connect(
        ("imap.gmail.com", 993),
        "imap.gmail.com",
        &tls
    )?;

    match client.login(email, password) {
        Ok(session) => Ok(session),
        Err((err, _orig_client)) => {
            Err(Box::new(err))
        }
    }
}

// Handler to authenticate and store Gmail IMAP credentials
pub async fn gmail_imap_login(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(payload): Json<ImapCredentials>,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::info!("Received request to /api/auth/gmail/imap/login for user {}", auth_user.user_id);

    let email = payload.email;
    let password = payload.password;

    // Attempt to connect to Gmail's IMAP server to verify credentials
    match connect_imap(&email, &password).await {
        Ok(mut session) => {
            // Logout immediately after verification to avoid keeping the session open
            if let Err(e) = session.logout() {
                tracing::warn!("Failed to logout IMAP session: {}", e);
            }

            if let Err(e) = state.user_repository.set_gmail_imap_credentials(
                auth_user.user_id,
                &email,
                &password,
            ) {
                tracing::error!("Failed to store IMAP credentials: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    AxumJson(json!({"error": "Failed to store IMAP credentials"})),
                ));
            }

            tracing::info!("Successfully stored IMAP credentials for user {}", auth_user.user_id);
            Ok(AxumJson(json!({"message": "IMAP connected successfully"})))
        }
        Err(e) => {
            tracing::error!("IMAP connection failed for user {}: {}", auth_user.user_id, e);
            Err((
                StatusCode::UNAUTHORIZED,
                AxumJson(json!({"error": "Invalid IMAP credentials"})),
            ))
        }
    }
}

// Handler to check the IMAP connection status
pub async fn gmail_imap_status(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<ImapStatus>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::info!("Checking IMAP status for user {}", auth_user.user_id);

    let connected = state
        .user_repository
        .get_gmail_imap_credentials(auth_user.user_id)
        .map(|opt| opt.is_some())
        .unwrap_or(false);

    Ok(AxumJson(ImapStatus { connected }))
}

// Handler to delete the IMAP connection
pub async fn delete_gmail_imap_connection(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::info!("Received request to delete IMAP connection for user {}", auth_user.user_id);

    if let Err(e) = state.user_repository.delete_gmail_imap_credentials(auth_user.user_id) {
        tracing::error!("Failed to delete IMAP credentials: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({"error": "Failed to delete IMAP credentials"})),
        ));
    }

    tracing::info!("Successfully deleted IMAP connection for user {}", auth_user.user_id);
    Ok(AxumJson(json!({"message": "IMAP connection deleted successfully"})))
}
