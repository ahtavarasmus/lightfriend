use std::sync::Arc;
use axum::{
    extract::{Query, State},
    response::{IntoResponse, Json, Redirect},
    http::{StatusCode, HeaderMap},
};
use tower_sessions::Session;
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use oauth2::{
    basic::BasicClient,
    PkceCodeVerifier,
    AuthorizationCode,
    CsrfToken,
    PkceCodeChallenge,
    Scope,
    TokenResponse,
    reqwest::blocking,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

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
    session: axum::extract::Extension<Session>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Received request to /api/auth/google/login with headers: {:?}", headers);
    let session_id = session.0.id().map(|id| id.to_string()).unwrap_or_else(|| "None".to_string());
    tracing::info!("Initial Session ID: {}", session_id);

    // Force session initialization by inserting a dummy value if no ID exists
    // Force session creation with a unique identifier
    let session_id = uuid::Uuid::new_v4().to_string();
    session.0.insert("session_id", &session_id).await.map_err(|e| {
        tracing::error!("Failed to initialize session: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to initialize session: {}", e)}))
        )
    })?;
    
    // Initialize session with a unique identifier
    let session_id = uuid::Uuid::new_v4().to_string();
    session.0.insert("session_id", &session_id).await.map_err(|e| {
        tracing::error!("Failed to initialize session: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to initialize session: {}", e)}))
        )
    })?;
    let session_id = session.0.id().map(|id| id.to_string()).unwrap_or_else(|| "None".to_string());
    tracing::info!("Session ID after initialization: {}", session_id);

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

    let flow_id = Uuid::new_v4().to_string();
    let csrf_token = CsrfToken::new_random();
    let state_token = format!("{}:{}", flow_id, csrf_token.secret());

    let (pkce_challenge, pkce_verifier) = PkceCodeChallenge::new_random_sha256();
    let (auth_url, _) = state
        .oauth_client
        .authorize_url(|| CsrfToken::new(state_token.clone()))
        .add_scope(Scope::new("https://www.googleapis.com/auth/calendar.events".to_string()))
        .set_pkce_challenge(pkce_challenge)
        .url();

    // Store flow data directly in session
    tracing::info!("Storing OAuth flow data in session");
    if let Err(e) = session.0.insert("pkce_verifier", pkce_verifier.secret().to_string()).await {
        tracing::error!("Failed to store PKCE verifier: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to store PKCE verifier in session: {}", e)}))
        ));
    }

    tracing::info!("Storing CSRF token in session");
    if let Err(e) = session.0.insert("csrf_token", csrf_token.secret().to_string()).await {
        tracing::error!("Failed to store CSRF token: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to store CSRF token in session: {}", e)}))
        ));
    }

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
    session: axum::extract::Extension<Session>,
) -> Result<Redirect, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Callback received with state: {}", query.state);
    let session_id = session.0.id().map(|id| id.to_string()).unwrap_or_else(|| "None".to_string());
    tracing::info!("Session ID: {}", session_id);

    let state_parts: Vec<&str> = query.state.split(':').collect();
    if state_parts.len() != 2 {
        tracing::error!("Invalid state format: {}", query.state);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid state format"}))
        ));
    }
    let flow_id = state_parts[0];
    let state_csrf = state_parts[1];
    tracing::info!("Parsed flow_id: {}, state_csrf: {}", flow_id, state_csrf);

    tracing::info!("Retrieving CSRF token for flow_id: {}", flow_id);
    let stored_csrf_token = session.0.get::<String>("csrf_token").await
        .map_err(|e| {
            tracing::error!("Session error retrieving CSRF token: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Session error: {}", e)}))
            )
        })?;

    let stored_csrf_token = match stored_csrf_token {
        Some(token) => {
            tracing::info!("Found CSRF token in session: {}", token);
            token
        },
        None => {
            tracing::error!("CSRF token missing from session for flow_id: {}", flow_id);
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "CSRF token missing from session"}))
            ));
        },
    };

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

    tracing::info!("Retrieving PKCE verifier for flow_id: {}", flow_id);
    let pkce_verifier = session.0.get::<String>("pkce_verifier").await
        .map_err(|e| {
            tracing::error!("Session error retrieving PKCE verifier: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Session error: {}", e)}))
            )
        })?;

    let pkce_verifier = match pkce_verifier {
        Some(verifier) => {
            tracing::info!("Found PKCE verifier in session");
            PkceCodeVerifier::new(verifier)
        },
        None => {
            tracing::error!("PKCE verifier missing from session for flow_id: {}", flow_id);
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "PKCE verifier missing from session"}))
            ));
        },
    };

    let http_client = blocking::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|e| {
            tracing::error!("Failed to build HTTP client: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to build HTTP client: {}", e)}))
            )
        })?;

    tracing::info!("Exchanging code for token");
    let token_result = state
        .oauth_client
        .exchange_code(AuthorizationCode::new(query.code))
        .set_pkce_verifier(pkce_verifier)
        .request(&http_client);

    match token_result {
        Ok(token) => {
            tracing::info!("Token exchange successful, cleaning up session");
            session.0.remove::<String>(&format!("pkce_verifier_{}", flow_id)).await
                .map_err(|e| {
                    tracing::error!("Failed to remove PKCE verifier: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": format!("Failed to remove PKCE verifier: {}", e)}))
                    )
                })?;

            session.0.remove::<String>(&format!("csrf_token_{}", flow_id)).await
                .map_err(|e| {
                    tracing::error!("Failed to remove CSRF token: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": format!("Failed to remove CSRF token: {}", e)}))
                    )
                })?;

            let _access_token = token.access_token().secret();
            let _refresh_token = token.refresh_token().map(|rt| rt.secret().to_string());
            let _expires_in = token.expires_in().unwrap_or_default().as_secs();

            tracing::info!("Redirecting to /dashboard");
            Ok(Redirect::to("/dashboard"))
        }
        Err(e) => {
            tracing::error!("Token exchange failed: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Token exchange failed: {}", e)}))
            ))
        },
    }
}
