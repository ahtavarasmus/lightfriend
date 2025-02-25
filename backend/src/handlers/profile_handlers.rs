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
pub struct UpdateProfileRequest {
    email: String,
    phone_number: String,
    nickname: String,
    info: String,
}

#[derive(Serialize)]
pub struct SubscriptionInfo {
    id: String,
    status: String,
    next_bill_date: i32,
    stage: String,
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
}

pub async fn get_profile(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ProfileResponse>, (StatusCode, Json<serde_json::Value>)> {
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

    // Get user profile from database
    let user = state.user_repository.find_by_id(claims.sub).map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": format!("Database error: {}", e)}))
    ))?;

    match user {
        Some(user) => {
            let current_time = std::time::SystemTime::now()
                                                    .duration_since(std::time::UNIX_EPOCH)
                                                    .unwrap()
                                                    .as_secs() as i32;

            let ttl = user.time_to_live.unwrap_or(0);
            let time_to_delete = current_time > ttl;
            let local_phone_number = match user.locality.as_str() {
                "fin" => std::env::var("FIN_PHONE").unwrap_or_default(),
                "usa" => std::env::var("USA_PHONE").unwrap_or_default(), 
                _ => std::env::var("FIN_PHONE").unwrap_or_default(), // Default to Finnish number
            };

            // Check subscription status
            let subscription_info = match state.user_subscriptions.find_by_user_id(user.id) {
                Ok(Some(sub)) => Some(SubscriptionInfo {
                    id: sub.paddle_subscription_id,
                    status: sub.status,
                    next_bill_date: sub.next_bill_date,
                    stage: sub.stage,
                }),
                _ => None,
            };

            Ok(Json(ProfileResponse {
                id: user.id,
                email: user.email,
                phone_number: user.phone_number,
                nickname: user.nickname,
                verified: user.verified,
                time_to_live: ttl,
                time_to_delete: time_to_delete,
                iq: user.iq,
                notify_credits: user.notify_credits,
                local_phone_number: local_phone_number,
                info: user.info,
                preferred_number: user.preferred_number,
                subscription: subscription_info,
            }))
        }
        None => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found"}))
        )),
    }
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

#[derive(Deserialize)]
pub struct NotifyCreditsRequest {
    notify: bool,
}

#[derive(Deserialize)]
pub struct PreferredNumberRequest {
    preferred_number: String,
}

pub async fn update_preferred_number(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<PreferredNumberRequest>,
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
    let allowed_numbers = vec![
        std::env::var("USA_PHONE").expect("USA_PHONE must be set in environment"),
        std::env::var("FIN_PHONE").expect("FIN_PHONE must be set in environment"),
        std::env::var("NLD_PHONE").expect("NLD_PHONE must be set in environment"),
        std::env::var("CHZ_PHONE").expect("CHZ_PHONE must be set in environment"),
    ];

    if !allowed_numbers.contains(&request.preferred_number) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid preferred number"}))
        ));
    }

    // Update preferred number
    state.user_repository.update_preferred_number(claims.sub, &request.preferred_number)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)}))
    ))?;

    println!("Updated preferred number to: {}", request.preferred_number);
    Ok(Json(json!({
        "message": "Preferred number updated successfully"
    })))
}


#[derive(Deserialize)]
pub struct UsageDataRequest {
    user_id: i32,
    from: i32,
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

pub async fn update_notify_credits(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(user_id): Path<i32>,
    Json(request): Json<NotifyCreditsRequest>,
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

    // Check if user is modifying their own settings or is an admin
    if claims.sub != user_id && !state.user_repository.is_admin(claims.sub).map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": format!("Database error: {}", e)}))
    ))? {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "You can only modify your own settings unless you're an admin"}))
        ));
    }

    // Update notify_credits preference
    state.user_repository.update_notify_credits(user_id, request.notify)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)}))
    ))?;

    Ok(Json(json!({
        "message": "Notification preference updated successfully"
    })))
}

pub async fn update_profile(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(update_req): Json<UpdateProfileRequest>,
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

    // Validate phone number format
    if !update_req.phone_number.starts_with('+') {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Phone number must start with '+'" }))
        ));
    }

    // Update user profile in database
    match state.user_repository.update_profile(claims.sub, &update_req.email, &update_req.phone_number, &update_req.nickname, &update_req.info) {
        Ok(_) => (),
        Err(DieselError::RollbackTransaction) => {
            return Err((
                StatusCode::CONFLICT,
                Json(json!({"error": "Phone number already exists"}))
            ));
        }
        Err(DieselError::NotFound) => {
            return Err((
                StatusCode::CONFLICT,
                Json(json!({"error": "Email already exists"}))
            ));
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ));
        }
    }

    Ok(Json(json!({
        "message": "Profile updated successfully"
    })))
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
