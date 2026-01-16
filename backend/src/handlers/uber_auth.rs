use crate::handlers::auth_middleware::AuthUser;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Json, Redirect},
};
use oauth2::{
    AuthorizationCode, CsrfToken, PkceCodeChallenge, PkceCodeVerifier, Scope, TokenResponse,
};
use reqwest;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use time::OffsetDateTime;
use tower_sessions::{
    session::{Id, Record},
    session_store::SessionStore,
};
use uuid::Uuid;

use crate::AppState;

#[derive(Deserialize)]
pub struct AuthRequest {
    code: String,
    state: String,
}

pub async fn uber_login(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Received request to /api/auth/uber/login");
    let session_key = Uuid::new_v4().to_string();
    tracing::info!("Generated session key: {}", session_key);
    let csrf_token = CsrfToken::new_random();
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let mut record = Record {
        id: Id(Uuid::new_v4().as_u128() as i128),
        data: Default::default(),
        expiry_date: OffsetDateTime::now_utc() + time::Duration::hours(1),
    };

    let server_url_oauth = std::env::var("SERVER_URL_OAUTH").expect("SERVER_URL_OAUTH must be set");
    let redirect_url = format!("{}/api/auth/google/calendar/callback", server_url_oauth);
    record
        .data
        .insert("session_key".to_string(), json!(session_key.clone()));
    record.data.insert(
        "pkce_verifier".to_string(),
        json!(pkce_verifier.secret().to_string()),
    );
    record.data.insert(
        "csrf_token".to_string(),
        json!(csrf_token.secret().to_string()),
    );
    record
        .data
        .insert("user_id".to_string(), json!(auth_user.user_id));
    record
        .data
        .insert("redirect_url".to_string(), json!(redirect_url));
    tracing::info!("Storing session record with ID: {}", record.id.0);
    if let Err(e) = state.session_store.create(&mut record).await {
        tracing::error!("Failed to store session record: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to store session record: {}", e)})),
        ));
    }
    let state_token = format!("{}:{}", record.id.0, csrf_token.secret());
    let auth_builder = state
        .uber_oauth_client
        .authorize_url(|| CsrfToken::new(state_token.clone()))
        .add_scope(Scope::new("profile".to_string()))
        .add_extra_param("prompt", "login");
    let (auth_url, _) = auth_builder.set_pkce_challenge(pkce_challenge).url();
    tracing::info!("Generated auth_url with state: {}", state_token);
    tracing::info!("Returning successful response with auth_url: {}", auth_url);
    Ok(Json(json!({
        "auth_url": auth_url.to_string(),
        "message": "OAuth flow initiated successfully"
    })))
}

pub async fn uber_callback(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AuthRequest>,
) -> Result<Redirect, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Callback received with state: {}", query.state);
    let state_parts: Vec<&str> = query.state.split(':').collect();
    if state_parts.len() != 2 {
        tracing::error!("Invalid state format: {}", query.state);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid state format"})),
        ));
    }
    let session_id_str = state_parts[0];
    let state_csrf = state_parts[1];
    let session_id = session_id_str.parse::<i128>().map_err(|e| {
        tracing::error!("Invalid session ID format: {}", e);
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid session ID format"})),
        )
    })?;
    let session_id = Id(session_id);
    tracing::info!("Loading session record");
    let record = state.session_store.load(&session_id).await.map_err(|e| {
        tracing::error!("Session store error loading record: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Session store error: {}", e)})),
        )
    })?;
    let record = match record {
        Some(r) => r,
        None => {
            tracing::error!("Session record missing");
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Session record not found"})),
            ));
        }
    };
    let stored_csrf_token = record
        .data
        .get("csrf_token")
        .and_then(|v| v.as_str().map(String::from))
        .ok_or_else(|| {
            tracing::error!("CSRF token missing from session record");
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "CSRF token missing from session"})),
            )
        })?;
    if stored_csrf_token != state_csrf {
        tracing::error!("CSRF token mismatch");
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "CSRF token mismatch"})),
        ));
    }
    let pkce_verifier = record
        .data
        .get("pkce_verifier")
        .and_then(|v| v.as_str().map(|s| PkceCodeVerifier::new(s.to_string())))
        .ok_or_else(|| {
            tracing::error!("PKCE verifier missing from session record");
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "PKCE verifier missing from session"})),
            )
        })?;
    let http_client = reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Client should build");
    tracing::info!("Exchanging code for token");
    let token_result = state
        .uber_oauth_client
        .exchange_code(AuthorizationCode::new(query.code))
        .set_pkce_verifier(pkce_verifier)
        .request_async(&http_client)
        .await
        .map_err(|e| {
            tracing::error!("Token exchange failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Token exchange failed: {}", e)})),
            )
        })?;
    let access_token = token_result.access_token().secret().clone();
    let refresh_token = token_result.refresh_token().map(|rt| rt.secret().clone());
    let expires_in = token_result.expires_in().unwrap_or_default().as_secs() as i32;
    let user_id = record
        .data
        .get("user_id")
        .and_then(|v| v.as_i64())
        .ok_or_else(|| {
            tracing::error!("User ID not found in session");
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "User ID not found in session"})),
            )
        })? as i32;
    tracing::info!("Token exchange successful, cleaning up session");
    if let Err(e) = state.session_store.delete(&session_id).await {
        tracing::error!("Failed to delete session record: {}", e);
    }
    // Store the connection in the database
    if let Err(e) = state.user_repository.create_uber_connection(
        user_id,
        &access_token,
        refresh_token.as_deref(),
        expires_in,
    ) {
        tracing::error!("Failed to store Uber connection: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to store uber connection"})),
        ));
    }
    tracing::info!("Successfully stored Uber connection for user {}", user_id);
    let frontend_url = std::env::var("FRONTEND_URL").expect("FRONTEND_URL must be set");
    tracing::info!("Redirecting to frontend with success: {}", frontend_url);
    Ok(Redirect::to(&format!("{}/?uber=success", frontend_url)))
}

pub async fn uber_disconnect(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!(
        "Received request to disconnect Uber for user {}",
        auth_user.user_id
    );

    // Get the tokens before deleting them
    let tokens = match state.user_repository.get_uber_tokens(auth_user.user_id) {
        Ok(Some(tokens)) => tokens,
        Ok(None) => {
            tracing::info!("No tokens found to revoke for user {}", auth_user.user_id);
            // Still attempt to delete from DB in case there's a record
            let _ = state
                .user_repository
                .delete_uber_connection(auth_user.user_id);
            return Ok(StatusCode::OK);
        }
        Err(e) => {
            tracing::error!("Failed to fetch tokens for revocation: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to fetch tokens"})),
            ));
        }
    };
    let (access_token, refresh_token) = tokens;

    let http_client = reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Client should build");

    let uber_client_id = std::env::var("UBER_CLIENT_ID").expect("UBER_CLIENT_ID must be set");
    let uber_client_secret =
        std::env::var("UBER_CLIENT_SECRET").expect("UBER_CLIENT_SECRET must be set");
    // Revoke access token
    let revoke_access = http_client
        .post("https://auth.uber.com/oauth/v2/revoke".to_string())
        .form(&[
            ("client_id", uber_client_id.as_str()),
            ("client_secret", uber_client_secret.as_str()),
            ("token", access_token.as_str()),
        ])
        .send()
        .await;

    if let Err(e) = revoke_access {
        tracing::warn!("Failed to revoke access token: {}", e);
        // Continue even if fails
    }

    // Revoke refresh token if present
    if !refresh_token.is_empty() {
        let revoke_refresh = http_client
            .post("https://auth.uber.com/oauth/v2/revoke".to_string())
            .form(&[
                ("client_id", uber_client_id.as_str()),
                ("client_secret", uber_client_secret.as_str()),
                ("token", refresh_token.as_str()),
            ])
            .send()
            .await;

        if let Err(e) = revoke_refresh {
            tracing::warn!("Failed to revoke refresh token: {}", e);
            // Continue even if fails
        }
    }

    // Delete the connection from the database
    state
        .user_repository
        .delete_uber_connection(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Failed to delete Uber connection: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to delete Uber connection"})),
            )
        })?;

    tracing::info!(
        "Successfully disconnected Uber for user {}",
        auth_user.user_id
    );
    Ok(StatusCode::OK)
}

pub async fn uber_status(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Checking Uber connection status");
    // Check if user has active Uber connection
    match state.user_repository.has_active_uber(auth_user.user_id) {
        Ok(has_connection) => {
            tracing::info!(
                "Successfully checked Uber connection status for user {}: {}",
                auth_user.user_id,
                has_connection
            );
            Ok(Json(json!({
                "connected": has_connection,
                "user_id": auth_user.user_id,
            })))
        }
        Err(e) => {
            tracing::error!("Failed to check Uber connection status: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                   "error": "Failed to check Uber connection status",
                   "details": e.to_string()
                })),
            ))
        }
    }
}
