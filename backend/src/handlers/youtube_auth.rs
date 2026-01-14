use crate::handlers::auth_middleware::AuthUser;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Json, Redirect},
};
use oauth2::{
    AuthorizationCode, CsrfToken, PkceCodeChallenge, PkceCodeVerifier, Scope, TokenResponse,
};
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

/// Returns the YouTube connection status for the authenticated user
pub async fn youtube_status(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let connected = match state.user_repository.has_active_youtube(auth_user.user_id) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to check YouTube connection status: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to check connection status"})),
            ));
        }
    };

    if !connected {
        return Ok(Json(
            json!({ "connected": false, "scope": null, "available": true }),
        ));
    }

    // Get the scope
    let scope = match state.user_repository.get_youtube_scope(auth_user.user_id) {
        Ok(Some(s)) => s,
        Ok(None) => "readonly".to_string(),
        Err(e) => {
            tracing::error!("Failed to get YouTube scope: {}", e);
            "readonly".to_string()
        }
    };

    // can_subscribe is true if scope is "write"
    let can_subscribe = scope == "write";

    Ok(Json(json!({
        "connected": true,
        "scope": scope,
        "can_subscribe": can_subscribe,
        "available": true
    })))
}

/// Initiates the OAuth flow for YouTube
pub async fn youtube_login(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Received request to /api/auth/youtube/login");

    let session_key = Uuid::new_v4().to_string();

    let csrf_token = CsrfToken::new_random();
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let mut record = Record {
        id: Id(Uuid::new_v4().as_u128() as i128),
        data: Default::default(),
        expiry_date: OffsetDateTime::now_utc() + time::Duration::hours(1),
    };
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
        .insert("oauth_type".to_string(), json!("youtube"));

    if let Err(e) = state.session_store.create(&mut record).await {
        tracing::error!("Failed to store session record: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to store session record: {}", e)})),
        ));
    }

    let state_token = format!("{}:{}", record.id.0, csrf_token.secret());

    // Use the YouTube OAuth client with its own redirect URL
    // Default to readonly scope - only request write access when user wants to subscribe
    let (auth_url, _) = state
        .youtube_oauth_client
        .authorize_url(|| CsrfToken::new(state_token.clone()))
        .add_scope(Scope::new(
            "https://www.googleapis.com/auth/youtube.readonly".to_string(),
        ))
        .add_extra_param("access_type", "offline")
        .add_extra_param("prompt", "consent")
        .set_pkce_challenge(pkce_challenge)
        .url();

    tracing::info!("Generated YouTube auth URL");
    Ok(Json(json!({
        "auth_url": auth_url.to_string(),
        "message": "OAuth flow initiated successfully"
    })))
}

/// Initiates OAuth flow with write scope for subscribing to channels
pub async fn youtube_upgrade_scope(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!(
        "Received request to upgrade YouTube scope for user {}",
        auth_user.user_id
    );

    let session_key = Uuid::new_v4().to_string();

    let csrf_token = CsrfToken::new_random();
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let mut record = Record {
        id: Id(Uuid::new_v4().as_u128() as i128),
        data: Default::default(),
        expiry_date: OffsetDateTime::now_utc() + time::Duration::hours(1),
    };
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
        .insert("oauth_type".to_string(), json!("youtube_upgrade"));

    if let Err(e) = state.session_store.create(&mut record).await {
        tracing::error!("Failed to store session record: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to store session record: {}", e)})),
        ));
    }

    let state_token = format!("{}:{}", record.id.0, csrf_token.secret());

    // Request youtube.force-ssl scope for write access (subscribe/unsubscribe)
    let (auth_url, _) = state
        .youtube_oauth_client
        .authorize_url(|| CsrfToken::new(state_token.clone()))
        .add_scope(Scope::new(
            "https://www.googleapis.com/auth/youtube.force-ssl".to_string(),
        ))
        .add_extra_param("access_type", "offline")
        .add_extra_param("prompt", "consent")
        .set_pkce_challenge(pkce_challenge)
        .url();

    tracing::info!("Generated YouTube upgrade auth URL");
    Ok(Json(json!({
        "auth_url": auth_url.to_string(),
        "message": "OAuth upgrade flow initiated successfully"
    })))
}

/// Initiates OAuth flow to downgrade back to readonly scope
pub async fn youtube_downgrade_scope(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!(
        "Received request to downgrade YouTube scope for user {}",
        auth_user.user_id
    );

    let session_key = Uuid::new_v4().to_string();

    let csrf_token = CsrfToken::new_random();
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let mut record = Record {
        id: Id(Uuid::new_v4().as_u128() as i128),
        data: Default::default(),
        expiry_date: OffsetDateTime::now_utc() + time::Duration::hours(1),
    };
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
        .insert("oauth_type".to_string(), json!("youtube_downgrade"));

    if let Err(e) = state.session_store.create(&mut record).await {
        tracing::error!("Failed to store session record: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to store session record: {}", e)})),
        ));
    }

    let state_token = format!("{}:{}", record.id.0, csrf_token.secret());

    // Request youtube.readonly scope only
    let (auth_url, _) = state
        .youtube_oauth_client
        .authorize_url(|| CsrfToken::new(state_token.clone()))
        .add_scope(Scope::new(
            "https://www.googleapis.com/auth/youtube.readonly".to_string(),
        ))
        .add_extra_param("access_type", "offline")
        .add_extra_param("prompt", "consent")
        .set_pkce_challenge(pkce_challenge)
        .url();

    tracing::info!("Generated YouTube downgrade auth URL");
    Ok(Json(json!({
        "auth_url": auth_url.to_string(),
        "message": "OAuth downgrade flow initiated successfully"
    })))
}

/// Handles the OAuth callback from Google for YouTube
pub async fn youtube_callback(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AuthRequest>,
) -> Result<Redirect, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("YouTube callback received with state: {}", query.state);

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

    tracing::info!("Exchanging code for YouTube token");
    let token_result = state
        .youtube_oauth_client
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

    let access_token = token_result.access_token().secret();
    let refresh_token = token_result.refresh_token().map(|rt| rt.secret());
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

    // Check if this is a scope upgrade or downgrade
    let oauth_type = record
        .data
        .get("oauth_type")
        .and_then(|v| v.as_str())
        .unwrap_or("youtube");
    let scope = match oauth_type {
        "youtube_upgrade" => "write",
        "youtube_downgrade" => "readonly",
        _ => "readonly", // Default for initial connection
    };

    // Clean up session
    if let Err(e) = state.session_store.delete(&session_id).await {
        tracing::error!("Failed to delete session record: {}", e);
    }

    // Store the YouTube connection with appropriate scope
    if let Err(e) = state.user_repository.create_youtube_connection_with_scope(
        user_id,
        access_token,
        refresh_token.as_ref().map(|s| s.as_str()),
        expires_in,
        scope,
    ) {
        tracing::error!("Failed to store YouTube connection: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to store YouTube connection"})),
        ));
    }

    tracing::info!(
        "Successfully stored YouTube connection for user {} with scope {}",
        user_id,
        scope
    );

    let frontend_url = std::env::var("FRONTEND_URL").expect("FRONTEND_URL must be set");

    // Redirect with appropriate success message
    let redirect_param = match oauth_type {
        "youtube_upgrade" => "youtube_upgraded",
        "youtube_downgrade" => "youtube_downgraded",
        _ => "youtube",
    };
    Ok(Redirect::to(&format!(
        "{}/?{}=success",
        frontend_url, redirect_param
    )))
}

/// Deletes the YouTube connection and revokes tokens
pub async fn delete_youtube_connection(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Received request to delete YouTube connection");

    // Get the tokens before deleting them
    let tokens = match state.user_repository.get_youtube_tokens(auth_user.user_id) {
        Ok(Some(tokens)) => tokens,
        Ok(None) => {
            tracing::info!("No tokens found to revoke for user {}", auth_user.user_id);
            return Ok(Json(json!({
                "message": "No YouTube connection found to delete"
            })));
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

    // Create HTTP client
    let http_client = reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Client should build");

    // Revoke access token
    let revoke_result = http_client
        .post("https://oauth2.googleapis.com/revoke")
        .query(&[("token", access_token)])
        .send()
        .await;

    if let Err(e) = revoke_result {
        tracing::error!("Failed to revoke access token: {}", e);
        // Continue with deletion even if revocation fails
    }

    // Revoke refresh token if it exists
    if !refresh_token.is_empty() {
        let revoke_refresh_result = http_client
            .post("https://oauth2.googleapis.com/revoke")
            .query(&[("token", refresh_token)])
            .send()
            .await;

        if let Err(e) = revoke_refresh_result {
            tracing::error!("Failed to revoke refresh token: {}", e);
            // Continue with deletion even if revocation fails
        }
    }

    // Delete the connection from our database
    match state
        .user_repository
        .delete_youtube_connection(auth_user.user_id)
    {
        Ok(_) => {
            tracing::info!(
                "Successfully deleted YouTube connection for user {}",
                auth_user.user_id
            );
            Ok(Json(json!({
                "message": "YouTube connection deleted and permissions revoked successfully"
            })))
        }
        Err(e) => {
            tracing::error!("Failed to delete YouTube connection: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to delete YouTube connection"})),
            ))
        }
    }
}
