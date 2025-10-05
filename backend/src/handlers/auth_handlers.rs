use std::sync::Arc;
use crate::handlers::auth_middleware::AuthUser;
use axum::{
    Json,
    extract::State,
    response::Response,
    http::StatusCode,
};
use rand::Rng;
use std::time::{SystemTime, UNIX_EPOCH};
use serde_json::json;
use jsonwebtoken::{encode, decode, Header, EncodingKey, DecodingKey, Validation};
use chrono::{Duration, Utc};
use serde::Deserialize;
use std::num::NonZeroU32;
use std::env;

use crate::{
    AppState
};

pub async fn refresh_token(
    State(state): State<Arc<AppState>>,
    headers: reqwest::header::HeaderMap,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let refresh_token = match headers.get("cookie") {
        Some(cookie_header) => {
            let cookies = cookie_header.to_str().unwrap_or("");
            cookies.split(';').find(|c| c.trim().starts_with("refresh_token="))
                .and_then(|c| c.split('=').nth(1))
                .map(|t| t.to_string())
                .ok_or((
                    StatusCode::UNAUTHORIZED,
                    Json(json!({"error": "Missing refresh token"}))
                ))?
        }
        None => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Missing cookies"}))
            ));
        }
    };

    // Validate refresh token
    let validation = Validation::default();
    let token_data = decode::<serde_json::Value>(
        &refresh_token,
        &DecodingKey::from_secret(env::var("JWT_REFRESH_KEY").expect("JWT_REFRESH_KEY must be set").as_ref()),
        &validation,
    ).map_err(|_| (
        StatusCode::UNAUTHORIZED,
        Json(json!({"error": "Invalid refresh token"}))
    ))?;

    let user_id: i32 = token_data.claims["sub"].as_i64().unwrap_or(0) as i32;
    if user_id == 0 {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Invalid user in token"}))
        ));
    }

    // Optional: Rotate refresh token by generating a new one
    generate_tokens_and_response(user_id)
}

pub async fn testing_handler(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(params): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    println!("Testing route called by user ID: {}", auth_user.user_id);
    println!("Received params: {:?}", params);

    let location = "Vuores, Tampere, Finland";

    match crate::utils::tool_exec::get_nearby_towns(location).await {
        Ok(towns) => {
            println!("Nearby towns: {:?}", towns);
            println!("Location: {}", location);
            Ok(Json(json!({"message": "Test successful"})))
        }
        Err(e) => {
            println!("Error in get_nearby_towns: {:?}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to get nearby towns: {}", e)}))
            ))
        }
    }
}

pub fn generate_tokens_and_response(user_id: i32) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    // Generate access token (short-lived)
    let access_token = encode(
        &Header::default(),
        &json!({
            "sub": user_id,
            "exp": (Utc::now() + Duration::minutes(15)).timestamp(),
            "type": "access"
        }),
        &EncodingKey::from_secret(std::env::var("JWT_SECRET_KEY")
            .expect("JWT_SECRET_KEY must be set in environment")
            .as_bytes()),
    ).map_err(|_| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": "Token generation failed"}))
    ))?;

    // Generate refresh token (long-lived)
    let refresh_token = encode(
        &Header::default(),
        &json!({
            "sub": user_id,
            "exp": (Utc::now() + Duration::days(7)).timestamp(),
            "type": "refresh"
        }),
        &EncodingKey::from_secret(std::env::var("JWT_REFRESH_KEY")
            .expect("JWT_REFRESH_KEY must be set in environment")
            .as_bytes()),
    ).map_err(|_| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": "Token generation failed"}))
    ))?;

    // Create response with HttpOnly cookies
    let mut response = Response::new(
        axum::body::Body::from(
            Json(json!({"message": "Tokens generated", "token": access_token.clone()})).to_string()
        )
    );
    let cookie_options = "; HttpOnly; Secure; SameSite=Strict; Path=/";
    response.headers_mut().insert(
        "Set-Cookie",
        format!("access_token={}{}; Max-Age=900", access_token, cookie_options)
            .parse()
            .unwrap(),
    );
    response.headers_mut().insert(
        "Set-Cookie",
        format!("refresh_token={}{}; Max-Age=604800", refresh_token, cookie_options)
            .parse()
            .unwrap(),
    );
    response.headers_mut().insert(
        "Content-Type",
        "application/json".parse().unwrap()
    );
    Ok(response)
}
