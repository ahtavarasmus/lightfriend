use std::sync::Arc;
use diesel::result::Error as DieselError;
use axum::{
    Json,
    extract::State,
    http::{StatusCode, HeaderMap}
};
use serde::{Deserialize, Serialize};
use axum::extract::Path;
use serde_json::json;
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use reqwest::header::{HeaderValue, AUTHORIZATION, CONTENT_TYPE};

use crate::repositories::user_repository::UsageDataPoint;
use crate::{
    AppState,
    handlers::auth_dtos::Claims,
};

#[derive(Deserialize)]
pub struct AutoTopupSettings {
    pub active: bool,
    pub amount: Option<i32>,
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
    iq: i32,
    notify_credits: bool,
    local_phone_number: String,
    info: Option<String>,
    preferred_number: Option<String>,
    subscription: Option<SubscriptionInfo>,
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


pub async fn reset_iq(
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
            Json(json!({"error": "Only admins can reset IQ"}))
        ));
    }

    // Reset user's IQ to zero in database
    state.user_repository.update_user_iq(user_id, 0)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)}))
    ))?;

    Ok(Json(json!({
        "message": "IQ reset successfully"
    })))
}


pub async fn increase_iq(
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

    // Check if user is modifying their own IQ or is an admin
    if claims.sub != user_id && !state.user_repository.is_admin(claims.sub).map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": format!("Database error: {}", e)}))
    ))? {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "You can only modify your own IQ unless you're an admin"}))
        ));
    }

    // Update user's IQ in database
    state.user_repository.increase_iq(user_id, 500)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)}))
    ))?;

    Ok(Json(json!({
        "message": "IQ increased successfully"
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


#[derive(Serialize)]
pub struct PaddlePortalSessionResponse {
    pub portal_url: String,
}

#[derive(Deserialize)]
pub struct PaddleResponse {
    pub data: PaddlePortalData,
}

#[derive(Deserialize)]
pub struct PaddlePortalData {
    pub urls: PaddleUrls,
}

#[derive(Deserialize)]
pub struct PaddleUrls {
    pub general: PaddleGeneralUrls,
}

#[derive(Deserialize)]
pub struct PaddleGeneralUrls {
    pub overview: String,
}

pub async fn get_customer_portal_link(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(user_id): Path<i32>,
) -> Result<Json<PaddlePortalSessionResponse>, (StatusCode, Json<serde_json::Value>)> {
    println!("getting the link");
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

    // Check if user is requesting their own portal or is an admin
    if claims.sub != user_id && !state.user_repository.is_admin(claims.sub).map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": format!("Database error: {}", e)}))
    ))? {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "You can only access your own customer portal unless you're an admin"}))
        ));
    }

    // Get subscription for the user
    let subscription = match state.user_subscriptions.find_by_user_id(user_id) {
        Ok(Some(sub)) => sub,
        Ok(None) => return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "No subscription found for this user"}))
        )),
        Err(e) => return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)}))
        )),
    };

    // Get customer ID from subscription
    let customer_id = subscription.paddle_customer_id;
    if customer_id.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "No customer ID found for this subscription"}))
        ));
    }

    // Create HTTP client
    let client = reqwest::Client::new();
    
    // Get Paddle API key from environment
    let paddle_api_key = std::env::var("PADDLE_API_KEY")
        .map_err(|_| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "PADDLE_API_KEY not set in environment"}))
        ))?;

    // Create request to Paddle API
    let url = format!("https://sandbox-api.paddle.com/customers/{}/portal-sessions", customer_id);
    
    // Make the request to Paddle API
    let response = client.post(&url)
        .header(AUTHORIZATION, format!("Bearer {}", paddle_api_key))
        .header(CONTENT_TYPE, "application/json")
        .send()
        .await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to connect to Paddle API: {}", e)}))
        ))?;
    

    // Check if request was successful
    if !response.status().is_success() {
        let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
        println!("Error from Paddle API: {}", error_text);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Paddle API error: {}", error_text)}))
        ));
    }

    // Parse response
    let paddle_response = response.json::<PaddleResponse>().await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to parse Paddle API response: {}", e)}))
        ))?;
    println!("paddle_response success");

    // Return the overview URL
    Ok(Json(PaddlePortalSessionResponse {
        portal_url: paddle_response.data.urls.general.overview,
    }))
}
