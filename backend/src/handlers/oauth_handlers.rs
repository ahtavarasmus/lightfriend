// src/handlers/oauth_handlers.rs
use axum::{
    extract::{Extension, Query, State},
    http::{StatusCode, HeaderMap},
    response::Redirect,
    Json,
};
use jsonwebtoken::{encode, decode, Header, EncodingKey, DecodingKey, Validation, Algorithm};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use chrono::{Duration, Utc};
use std::env;
use std::sync::Arc;
use uuid::Uuid;
use crate::AppState;

use crate::models::user::User;

// JWT Claims for state parameter
#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String, // User ID
    exp: usize,  // Expiration timestamp
}

// Response from Nylas token endpoint
#[derive(Debug, Deserialize)]
struct TokenResponse {
    grant_id: String,
}

// Handler to initiate OAuth flow
pub async fn initiate_oauth(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    println!("Initiating OAuth flow");

    // Extract token from Authorization header
    let auth_header = headers
        .get("Authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "));

    let token = match auth_header {
        Some(token) => token,
        None => {
            println!("No authorization token provided in headers");
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "No authorization token provided"})),
            ));
        }
    };

    // Decode and validate JWT token
    let claims = match decode::<Claims>(
        token,
        &DecodingKey::from_secret(
            std::env::var("JWT_SECRET_KEY")
                .expect("JWT_SECRET_KEY must be set in environment")
                .as_bytes(),
        ),
        &Validation::new(Algorithm::HS256),
    ) {
        Ok(token_data) => {
            println!("JWT token decoded successfully for user: {}", token_data.claims.sub);
            token_data.claims
        }
        Err(e) => {
            println!("Failed to decode JWT token: {:?}", e);
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Invalid token"})),
            ));
        }
    };

    // Generate state token with user ID
    let state_token = encode(
        &Header::default(),
        &json!({
            "sub": claims.sub,
            "exp": (Utc::now() + Duration::minutes(10)).timestamp(),
            "type": "state"
        }),
        &EncodingKey::from_secret(
            std::env::var("JWT_SECRET_KEY")
                .expect("JWT_SECRET_KEY must be set in environment")
                .as_bytes(),
        ),
    )
    .map_err(|e| {
        println!("Failed to generate state token: {:?}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to generate state token"})),
        )
    })?;
    println!("State token generated successfully");

    // Construct Nylas authorization URL
    let client_id = std::env::var("NYLAS_CLIENT_ID")
        .expect("NYLAS_CLIENT_ID must be set in environment");
    let auth_url = format!(
        "https://api.us.nylas.com/v3/connect/auth?client_id={}&redirect_uri={}&response_type=code&scope=calendar&state={}",
        client_id,
        "http://localhost:3000/auth/nylas/callback",
        state_token
    );
    println!("Nylas auth URL constructed: {}", auth_url);

    Ok(Json(json!({
        "message": "OAuth initiated successfully",
        "auth_url": auth_url
    })))
}


// Handler for OAuth callback
pub async fn oauth_callback(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Redirect, (StatusCode, Json<serde_json::Value>)> {
    println!("Handling OAuth callback");

    // Extract code and state from query parameters
    let code = match params.get("code") {
        Some(code) => code,
        None => {
            println!("Missing code parameter in callback");
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Missing code parameter"})),
            ));
        }
    };
    let state_token = match params.get("state") {
        Some(state) => state,
        None => {
            println!("Missing state parameter in callback");
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Missing state parameter"})),
            ));
        }
    };
    println!("Received code: {} and state: {}", code, state_token);

    // Verify state JWT
    let claims = match decode::<Claims>(
        state_token,
        &DecodingKey::from_secret(
            std::env::var("JWT_SECRET_KEY")
                .expect("JWT_SECRET_KEY must be set in environment")
                .as_bytes(),
        ),
        &Validation::new(Algorithm::HS256),
    ) {
        Ok(token_data) => {
            println!("State token verified for user: {}", token_data.claims.sub);
            token_data.claims
        }
        Err(e) => {
            println!("Invalid state token: {:?}", e);
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Invalid state token"})),
            ));
        }
    };

    let user_id = match claims.sub.parse::<i32>() {
        Ok(id) => id,
        Err(e) => {
            println!("Invalid user ID in state token: {:?}", e);
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Invalid user ID in state token"})),
            ));
        }
    };

    // Exchange authorization code for grant ID
    let client = Client::new();
    let api_key = std::env::var("NYLAS_API_KEY")
        .expect("NYLAS_API_KEY must be set in environment");
    let client_id = std::env::var("NYLAS_CLIENT_ID")
        .expect("NYLAS_CLIENT_ID must be set in environment");

    let res = client
        .post("https://api.us.nylas.com/v3/connect/token")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&json!({
            "client_id": client_id,
            "code": code,
            "grant_type": "authorization_code",
            "redirect_uri": "http://localhost:3000/auth/nylas/callback"
        }))
        .send()
        .await
        .map_err(|e| {
            println!("Failed to exchange code for grant ID: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to exchange code for grant ID"})),
            )
        })?;

    if res.status().is_success() {
        let token_response: TokenResponse = res.json().await.map_err(|e| {
            println!("Failed to parse token response: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to parse token response"})),
            )
        })?;
        let grant_id = token_response.grant_id;
        println!("Grant ID obtained: {}", grant_id);

        // Update user with grant ID
        let user = match state.user_repository.find_by_id(user_id) {
            Ok(Some(user)) => user,
            Ok(None) => {
                println!("User not found for ID: {}", user_id);
                return Err((
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": "User not found"})),
                ));
            }
            Err(e) => {
                println!("Database error finding user: {:?}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Database error: {}", e)})),
                ));
            }
        };

        let mut updated_user = user.clone();
        updated_user.nylas_grant_id = Some(grant_id);
        state.user_repository.update(&updated_user).map_err(|e| {
            println!("Failed to update user with grant ID: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to update user: {}", e)})),
            )
        })?;
        println!("User updated with grant ID successfully");

        // Redirect to success page
        println!("Redirecting to /calendar-connected");
        Ok(Redirect::to("/calendar-connected"))
    } else {
        println!("Failed to obtain grant ID from Nylas");
        Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Failed to obtain grant ID"})),
        ))
    }
}
