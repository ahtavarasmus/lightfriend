use std::sync::Arc;
use axum::{
    Json,
    extract::State,
    http::{StatusCode, HeaderMap}
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};

use crate::repositories::user_repository::UsageDataPoint;
use crate::{
    AppState,
    handlers::auth_dtos::Claims,
};

#[derive(Deserialize)]
pub struct AutoTopupSettings {
    pub active: bool,
    pub amount: Option<f32>,
}


#[derive(Serialize)]
pub struct SubscriptionInfo {
    id: String,
    status: String,
    next_bill_date: i32,
    stage: String,
    is_scheduled_to_cancel: Option<bool>,
}

#[derive(Serialize)]
pub struct AutoTopupInfo {
    pub active: bool,
    pub amount: String,
}

#[derive(Serialize)]
pub struct ProfileResponse {
    id: i32,
    email: String,
    phone_number: String,
    nickname: Option<String>,
    verified: bool,
    time_to_live: i32,
    time_to_delete: bool,
    credits: f32,
    notify: bool,
    info: Option<String>,
    preferred_number: Option<String>,
    auto_topup: Option<AutoTopupInfo>,
}
#[derive(Deserialize)]
pub struct UsageDataRequest {
    pub user_id: i32,
    pub from: i32,
}


#[derive(Deserialize)]
pub struct CreateCheckoutRequest {
    pub amount: u64, // amount in cents
}

#[derive(Serialize)]
pub struct CheckoutResponse {
    pub session_id: String,
}



pub async fn get_usage_data(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<UsageDataRequest>,
) -> Result<Json<Vec<UsageDataPoint>>, (StatusCode, Json<serde_json::Value>)> {
    println!("in get_usage_data route");
    // Extract and validate token
    let auth_header = headers.get("Authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "));

    let token = match auth_header {
        Some(token) => token,
        None => return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "No authorization token provided"}))
        )),
    };

    // Decode JWT token
    let claims = match decode::<Claims>(
        token,
        &DecodingKey::from_secret(std::env::var("JWT_SECRET_KEY")
            .expect("JWT_SECRET_KEY must be set in environment")
            .as_bytes()),
        &Validation::new(Algorithm::HS256)
    ) {
        Ok(token_data) => token_data.claims,
        Err(_) => return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Invalid token"}))
        )),
    };

    // Get usage data using the provided 'from' timestamp
    let usage_data = state.user_repository.get_usage_data(claims.sub, request.from)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)}))
        ))?;

    Ok(Json(usage_data))
}


pub async fn reset_credits(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    axum::extract::Path(user_id): axum::extract::Path<i32>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Extract and validate token
    let auth_header = headers.get("Authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "));

    let token = match auth_header {
        Some(token) => token,
        None => return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "No authorization token provided"}))
        )),
    };

    // Decode JWT token
    let claims = match decode::<Claims>(
        token,
        &DecodingKey::from_secret(std::env::var("JWT_SECRET_KEY")
            .expect("JWT_SECRET_KEY must be set in environment")
            .as_bytes()),
        &Validation::new(Algorithm::HS256)
    ) {
        Ok(token_data) => token_data.claims,
        Err(_) => return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Invalid token"}))
        )),
    };

    // Check if user is an admin
    if !state.user_repository.is_admin(claims.sub).map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": format!("Database error: {}", e)}))
    ))? {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Only admins can reset credits"}))
        ));
    }

    // Reset user's credits to zero in database
    state.user_repository.update_user_credits(user_id, 0.00)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)}))
    ))?;

    Ok(Json(json!({
        "message": "credits reset successfully"
    })))
}


pub async fn increase_credits(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    axum::extract::Path(user_id): axum::extract::Path<i32>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Extract and validate token
    let auth_header = headers.get("Authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "));

    let token = match auth_header {
        Some(token) => token,
        None => return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "No authorization token provided"}))
        )),
    };

    // Decode JWT token
    let claims = match decode::<Claims>(
        token,
        &DecodingKey::from_secret(std::env::var("JWT_SECRET_KEY")
            .expect("JWT_SECRET_KEY must be set in environment")
            .as_bytes()),
        &Validation::new(Algorithm::HS256)
    ) {
        Ok(token_data) => token_data.claims,
        Err(_) => return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Invalid token"}))
        )),
    };

    // Check if user is modifying their own credits or is an admin
    if claims.sub != user_id && !state.user_repository.is_admin(claims.sub).map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": format!("Database error: {}", e)}))
    ))? {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "You can only modify your own credits unless you're an admin"}))
        ));
    }

    // Update user's credits in database
    state.user_repository.increase_credits(user_id, 2.00)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)}))
    ))?;

    Ok(Json(json!({
        "message": "credits increased successfully"
    })))
}


pub async fn update_topup(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(settings): Json<AutoTopupSettings>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Extract and validate token
    let auth_header = headers.get("Authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "));

    let token = match auth_header {
        Some(token) => token,
        None => return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({"success": false, "message": "No authorization token provided"}))
        )),
    };

    // Decode JWT token
    let claims = match decode::<Claims>(
        token,
        &DecodingKey::from_secret(std::env::var("JWT_SECRET_KEY")
            .expect("JWT_SECRET_KEY must be set in environment")
            .as_bytes()),
        &Validation::new(Algorithm::HS256)
    ) {
        Ok(token_data) => token_data.claims,
        Err(_) => return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({"success": false, "message": "Invalid token"}))
        )),
    };

    println!("here");

    // Update the user's auto-topup settings with fixed threshold of 3.00
    match state.user_repository.update_auto_topup(
        claims.sub, 
        settings.active, 
        settings.amount, 
    ) {
        Ok(_) => Ok(Json(json!({
            "success": true,
            "message": "Auto top-up settings updated successfully"
        }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": format!("Failed to update auto top-up settings: {}", e)}))
        )),
    }
}

