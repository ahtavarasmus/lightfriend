use axum::{
    extract::{Query, State},
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use crate::AppState;
use crate::models::user_models::CalendarConnection;

#[derive(Deserialize)]
pub struct OAuthCallback {
    code: String,
    state: String,
}

#[derive(Serialize)]
pub struct AuthParams {
    auth_url: String,
    state: String,
}

/*

pub async fn fetch_auth_params() -> impl IntoResponse {
    let client = Client::new();
    let api_key = std::env::var("COMPOSIO_API_KEY").expect("COMPOSIO_API_KEY must be set");
    
    let response = client
        .get("https://backend.composio.dev/api/v2/oauth/google-calendar/auth-url")
        .header("X-API-Key", api_key)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch auth params: {}", e))?;
    
    let params = response.json::<AuthParams>().await
        .map_err(|e| format!("Failed to parse auth params: {}", e))?;
    
    Ok(Json(params))
}

pub async fn oauth_callback(
    Query(params): Query<OAuthCallback>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let client = Client::new();
    let api_key = std::env::var("COMPOSIO_API_KEY").expect("COMPOSIO_API_KEY must be set");

    // Exchange the code for access token
    let token_response = client
        .post("https://backend.composio.dev/api/v2/oauth/google-calendar/token")
        .header("X-API-Key", api_key)
        .json(&json!({
            "code": params.code,
            "state": params.state
        }))
        .send()
        .await
        .map_err(|e| format!("Failed to exchange code: {}", e))?;

    let token_data = token_response.json::<serde_json::Value>().await
        .map_err(|e| format!("Failed to parse token response: {}", e))?;

    let access_token = token_data["access_token"]
        .as_str()
        .ok_or("No access token in response")?;

    // Store the encrypted access token in the database
    use diesel::prelude::*;
    let conn = &mut state.db_pool.get().unwrap();

    let calendar_connection = CalendarConnection {
        id: None,
        user_id: /* get from session */,
        name: "Google Calendar".to_string(),
        description: "Connected Google Calendar".to_string(),
        provider: "google".to_string(),
        encrypted_access_token: encrypt_token(access_token)?,
    };

    diesel::insert_into(crate::schema::calendar_connection::table)
        .values(&calendar_connection)
        .execute(conn)
        .map_err(|e| format!("Failed to store connection: {}", e))?;

    // Redirect back to the frontend
    let frontend_url = std::env::var("FRONTEND_URL").expect("FRONTEND_URL must be set");
    Ok(axum::response::Redirect::to(&format!("{}/calendar-connected", frontend_url)))
}

fn encrypt_token(token: &str) -> Result<String, String> {
    // Implement encryption using your preferred method
    // This is just a placeholder - you should use proper encryption!
    Ok(base64::encode(token))
}
*/
