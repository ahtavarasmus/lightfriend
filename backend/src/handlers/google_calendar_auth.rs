use std::sync::Arc;
use axum::{
    extract::{Query, State},
    response::{IntoResponse, Json, Redirect},
    http::{StatusCode, HeaderMap},
};
use tower_sessions::{MemoryStore, session_store::SessionStore, session::{Id, Record}};
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use oauth2::{
    basic::BasicClient,
    PkceCodeVerifier,
    AuthorizationCode,
    CsrfToken,
    PkceCodeChallenge,
    Scope,
    TokenResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;
use time::OffsetDateTime;

use crate::{
    AppState,
    handlers::auth_dtos::Claims,
};

#[derive(Deserialize)]
pub struct AuthRequest {
    code: String,
    state: String,
}

#[derive(Serialize)]
pub struct TokenInfo {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: u64,
}

pub async fn google_login(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Received request to /api/auth/google/login with headers: {:?}", headers);

    let session_key = Uuid::new_v4().to_string();
    tracing::info!("Generated session key: {}", session_key);

    let auth_header = headers.get("Authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "));
    
    let token = match auth_header {
        Some(token) => {
            tracing::info!("Found Authorization header with token: {}", token);
            token
        },
        None => {
            tracing::error!("No authorization token provided in headers: {:?}", headers);
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "No authorization token provided"}))
            ));
        },
    };

    let claims = match decode::<Claims>(
        token,
        &DecodingKey::from_secret(std::env::var("JWT_SECRET_KEY")
            .expect("JWT_SECRET_KEY must be set in environment")
            .as_bytes()),
        &Validation::new(Algorithm::HS256)
    ) {
        Ok(token_data) => {
            tracing::info!("JWT token decoded successfully");
            token_data.claims
        },
        Err(e) => {
            tracing::error!("Invalid token: {}", e);
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Invalid token"}))
            ));
        },
    };

    let csrf_token = CsrfToken::new_random();
    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();

    let mut record = Record {
        id: Id(Uuid::new_v4().as_u128() as i128),
        data: Default::default(),
        expiry_date: OffsetDateTime::now_utc() + time::Duration::hours(1),
    };
    record.data.insert("session_key".to_string(), json!(session_key.clone()));
    record.data.insert("pkce_verifier".to_string(), json!(pkce_verifier.secret().to_string()));
    record.data.insert("csrf_token".to_string(), json!(csrf_token.secret().to_string()));

    tracing::info!("Storing session record with ID: {}", record.id.0);
    if let Err(e) = state.session_store.create(&mut record).await {
        tracing::error!("Failed to store session record: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to store session record: {}", e)}))
        ));
    }

    let state_token = format!("{}:{}", record.id.0, csrf_token.secret());
    let (auth_url, _) = state
        .oauth_client
        .authorize_url(|| CsrfToken::new(state_token.clone()))
        .add_scope(Scope::new("https://www.googleapis.com/auth/calendar.events".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url();

    tracing::info!("Generated auth_url with state: {}", state_token);
    tracing::info!("Returning successful response with auth_url: {}", auth_url);
    Ok(Json(json!({
        "auth_url": auth_url.to_string(),
        "message": "OAuth flow initiated successfully"
    })))
}

pub async fn google_callback(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AuthRequest>,
) -> Result<Redirect, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Callback received with state: {}", query.state);

    let state_parts: Vec<&str> = query.state.split(':').collect();
    if state_parts.len() != 2 {
        tracing::error!("Invalid state format: {}", query.state);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid state format"}))
        ));
    }
    let session_id_str = state_parts[0];
    let state_csrf = state_parts[1];
    tracing::info!("Parsed session_id: {}, state_csrf: {}", session_id_str, state_csrf);

    let session_id = session_id_str.parse::<i128>()
        .map_err(|e| {
            tracing::error!("Invalid session ID format: {}", e);
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Invalid session ID format"}))
            )
        })?;
    let session_id = Id(session_id);

    tracing::info!("Loading session record for session_id: {}", session_id_str);
    let record = state.session_store.load(&session_id).await
        .map_err(|e| {
            tracing::error!("Session store error loading record: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Session store error: {}", e)}))
            )
        })?;

    let record = match record {
        Some(r) => r,
        None => {
            tracing::error!("Session record missing for session_id: {}", session_id_str);
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Session record not found"}))
            ));
        },
    };

    let stored_session_key = record.data.get("session_key")
        .and_then(|v| v.as_str().map(String::from))
        .ok_or_else(|| {
            tracing::error!("Session key missing from session record for session_id: {}", session_id_str);
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Session key missing from session"}))
            )
        })?;

    let stored_csrf_token = record.data.get("csrf_token")
        .and_then(|v| v.as_str().map(String::from))
        .ok_or_else(|| {
            tracing::error!("CSRF token missing from session record for session_id: {}", session_id_str);
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "CSRF token missing from session"}))
            )
        })?;

    if stored_csrf_token != state_csrf {
        tracing::error!(
            "CSRF token mismatch: stored={}, received={}",
            stored_csrf_token,
            state_csrf
        );
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "CSRF token mismatch"}))
        ));
    }

    let pkce_verifier = record.data.get("pkce_verifier")
        .and_then(|v| v.as_str().map(|s| PkceCodeVerifier::new(s.to_string())))
        .ok_or_else(|| {
            tracing::error!("PKCE verifier missing from session record for session_id: {}", session_id_str);
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "PKCE verifier missing from session"}))
            )
        })?;
    let http_client = reqwest::ClientBuilder::new()
        // Following redirects opens the client up to SSRF vulnerabilities.
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Client should build");

    tracing::info!("Exchanging code for token");
    let token_result = state
        .oauth_client
        .exchange_code(AuthorizationCode::new(query.code))
        .set_pkce_verifier(pkce_verifier)
        .request_async(&http_client) // Use async client
        .await
        .map_err(|e| {
            tracing::error!("Token exchange failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Token exchange failed: {}", e)}))
            )
        })?;

    tracing::info!("Token exchange successful, cleaning up session");
    if let Err(e) = state.session_store.delete(&session_id).await {
        tracing::error!("Failed to delete session record: {}", e);
    }

    let _access_token = token_result.access_token().secret();
    let _refresh_token = token_result.refresh_token().map(|rt| rt.secret().to_string());
    let _expires_in = token_result.expires_in().unwrap_or_default().as_secs();

    let frontend_url = std::env::var("FRONTEND_URL")
        .expect("FRONTEND_URL must be set");
    tracing::info!("Redirecting to frontend root: {}", frontend_url);
    Ok(Redirect::to(&frontend_url))
}
