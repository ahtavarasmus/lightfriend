
use std::sync::Arc;
use axum::{
    Json,
    extract::State,
    http::{StatusCode, HeaderMap},
    response::Redirect,
};
use serde::{Deserialize, Serialize};
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use chrono::{Utc, Duration};
use reqwest::Client;
use serde_json::json;

use crate::{
    AppState,
    handlers::auth_dtos::Claims,
    models::user_models::NewUnipileConnection,
};

#[derive(Serialize)]
struct UnipileAuthLinkRequest {
    #[serde(rename = "type")]
    request_type: String,
    providers: String,
    #[serde(rename = "expiresOn")]
    expires_on: String,
    name: String,
    #[serde(rename = "success_redirect_url")]
    success_redirect_url: String,
    #[serde(rename = "failure_redirect_url")]
    failure_redirect_url: String,
    #[serde(rename = "notify_url")]
    notify_url: String,
    #[serde(rename = "api_url")]
    api_url: String,
}

#[derive(Deserialize)]
struct UnipileAuthLinkResponse {
    object: String,
    url: String,
}

#[derive(Deserialize)]
pub struct UnipileConnectionPayload {
    status: String,
    account_id: String,
    name: String,
}

// Handler to generate and redirect to Unipile auth link
pub async fn get_auth_link(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Starting get_auth_link handler");
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
    tracing::info!("Attempting to get JWT_SECRET_KEY");
    let jwt_secret = std::env::var("JWT_SECRET_KEY").map_err(|e| {
        tracing::error!("Failed to get JWT_SECRET_KEY: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "JWT_SECRET_KEY environment variable is not set"}))
        )
    })?;

    let claims = match decode::<Claims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &Validation::new(Algorithm::HS256)
    ) {
        Ok(token_data) => token_data.claims,
        Err(_) => return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Invalid token"}))
        )),
    };

    // Prepare the request to Unipile
    let expires_on = (Utc::now() + Duration::hours(1)).format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();
    tracing::info!("Attempting to get SERVER_URL");
    let base_url = std::env::var("SERVER_URL").expect("SERVER_URL not set");
    let frontend_url= std::env::var("FRONTEND_URL").expect("FRONTEND_URL not set");
    
    let request_body = UnipileAuthLinkRequest {
        request_type: "create".to_string(),
        providers: "*".to_string(),
        expires_on,
        name: claims.sub.to_string(),
        success_redirect_url: format!("{}/?success=true", frontend_url),
        failure_redirect_url: format!("{}/?success=false", frontend_url),
        notify_url: format!("{}/api/unipile/connection", base_url),
        api_url: std::env::var("UNIPILE_API_URL").expect("UNIPILE_API_URL not set"),
    };

    // Make request to Unipile API
    let client = Client::new();
    tracing::info!("Attempting to get UNIPILE_API_URL");
    let unipile_api_url = std::env::var("UNIPILE_API_URL").map_err(|e| {
        tracing::error!("Failed to get UNIPILE_API_URL: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "UNIPILE_API_URL environment variable is not set"}))
        )
    })?;
    
    let full_url = format!("{}/api/v1/hosted/accounts/link", unipile_api_url);
    tracing::info!("Making request to Unipile API at: {}", full_url);
    
    // Get the Unipile API key from environment variables
    let api_key = std::env::var("UNIPILE_API_KEY").map_err(|e| {
        tracing::error!("Failed to get UNIPILE_API_KEY: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "UNIPILE_API_KEY environment variable is not set"}))
        )
    })?;

    let response = client
        .post(&full_url)
        .header("X-API-KEY", api_key)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to contact Unipile API: {}", e)}))
        ))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_body = response.text().await.unwrap_or_else(|_| "Could not read error response".to_string());
        tracing::error!("Unipile API request failed. Status: {}, Body: {}", status, error_body);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to get auth link from Unipile: Status {}", status)}))
        ));
    }

    let auth_link: UnipileAuthLinkResponse = response.json().await.map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": format!("Failed to parse Unipile response: {}", e)}))
    ))?;

    // Instead of redirecting, return the URL to the frontend
    Ok(Json(json!({ "url": auth_link.url })))
}

// Handler for the webhook notification from Unipile
pub async fn handle_connection_webhook(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<UnipileConnectionPayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Extract user_id from the name field (which we set to user_id earlier)
    let user_id = payload.name.parse::<i32>().map_err(|_| (
        StatusCode::BAD_REQUEST,
        Json(json!({"error": "Invalid user ID in payload"}))
    ))?;

    // Verify the user exists
    let user = state.user_repository.find_by_id(user_id).map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": format!("Database error: {}", e)}))
    ))?.ok_or_else(|| (
        StatusCode::NOT_FOUND,
        Json(json!({"error": "User not found"}))
    ))?;

    let current_time = Utc::now().timestamp() as i32;

    // Create new Unipile connection
    let new_connection = NewUnipileConnection {
        user_id,
        account_type: "UNIPILE".to_string(), // You might want to make this dynamic based on the actual service
        account_id: payload.account_id,
        status: payload.status,
        last_update: current_time,
        created_on: current_time,
        description: "Unipile Integration".to_string(),
    };

    // Save the connection to database
    state.user_repository.create_unipile_connection(&new_connection).map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": format!("Failed to save connection: {}", e)}))
    ))?;

    tracing::info!("Connection successfully created");

    Ok(Json(json!({
        "message": "Connection successfully created"
    })))
}
