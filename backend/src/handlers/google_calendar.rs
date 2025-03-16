use std::sync::Arc;
use axum::{
    extract::{State, Query},
    response::Json,
    http::{StatusCode, HeaderMap},
};
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};
use serde_json::json;
use serde::{Deserialize, Serialize};
use oauth2::TokenResponse;
use reqwest::header::{AUTHORIZATION, ACCEPT};
use chrono::{DateTime, Utc};

use crate::{
    AppState,
    handlers::auth_dtos::Claims,
};

#[derive(Debug, Deserialize)]
pub struct TimeframeQuery {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CalendarEvent {
    pub id: String,
    pub summary: Option<String>,
    pub description: Option<String>,
    pub start: EventDateTime,
    pub end: EventDateTime,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct EventDateTime {
    #[serde(rename = "dateTime")]
    pub date_time: Option<DateTime<Utc>>,
    pub date: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CalendarResponse {
    pub items: Vec<CalendarEvent>,
}

pub async fn google_calendar_status(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Checking Google Calendar connection status");

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

    // Check if user has active Google Calendar connection
    match state.user_repository.has_active_google_calendar(claims.sub) {
        Ok(has_connection) => {
            tracing::info!("Successfully checked calendar connection status for user {}: {}", claims.sub, has_connection);
            Ok(Json(json!({
                "connected": has_connection,
                "user_id": claims.sub
            })))
        },
        Err(e) => {
            tracing::error!("Failed to check calendar connection status: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to check calendar connection status",
                    "details": e.to_string()
                 }))
            ))
        }
    }
}

#[derive(Debug)]
pub enum CalendarError {
    NoConnection,
    TokenError(String),
    ApiError(String),
    ParseError(String),
}
pub async fn fetch_calendar_events(
    state: &AppState,
    user_id: i32,
    timeframe: TimeframeQuery,
) -> Result<Vec<CalendarEvent>, CalendarError> {
    println!("Starting to fetch calendar events...");
    tracing::info!("Fetching calendar events for timeframe: {:?} to {:?}", timeframe.start, timeframe.end);

    // Get Google Calendar tokens
    println!("Getting Google Calendar tokens for user_id: {}", user_id);
    let (access_token, refresh_token) = match state.user_repository.get_google_calendar_tokens(user_id) {
        Ok(Some((access, refresh))) => {
            println!("Successfully retrieved and decrypted tokens");
            tracing::debug!("Access token length: {}, Refresh token length: {}", 
                access.len(), refresh.len());
            (access, refresh)
        },
        Ok(None) => {
            println!("No active Google Calendar connection found");
            return Err(CalendarError::NoConnection);
        },
        Err(e) => {
            println!("Error getting tokens: {}", e);
            tracing::error!("Failed to get calendar tokens: {}", e);
            return Err(CalendarError::TokenError(format!("Failed to decrypt tokens: {}", e)));
        }
    };
    println!("Successfully retrieved tokens");

    // Create HTTP client for Google Calendar API
    let client = reqwest::Client::new();
    
    // Format the dates for Google Calendar API
    let start_time = timeframe.start.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let end_time = timeframe.end.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    println!("Formatted time range: {} to {}", start_time, end_time);

    // Make request to Google Calendar API
    println!("Making initial request to Google Calendar API...");
    let response = client
        .get("https://www.googleapis.com/calendar/v3/calendars/primary/events")
        .header(AUTHORIZATION, format!("Bearer {}", access_token))
        .header(ACCEPT, "application/json")
        .query(&[
            ("timeMin", start_time.as_str()),
            ("timeMax", end_time.as_str()),
            ("singleEvents", "true"),
            ("orderBy", "startTime"),
        ])
        .send()
        .await
        .map_err(|e| CalendarError::ApiError(e.to_string()))?;
    println!("Initial response status: {}", response.status());

    if response.status() == StatusCode::UNAUTHORIZED {
        println!("Token expired, starting refresh process...");
        // Token expired, try to refresh
        tracing::info!("Access token expired, attempting to refresh");
        
        let http_client = reqwest::ClientBuilder::new()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("Client should build");

        println!("Exchanging refresh token...");
        let token_result = state
            .oauth_client
            .exchange_refresh_token(&oauth2::RefreshToken::new(refresh_token))
            .request_async(&http_client)
            .await
            .map_err(|e| CalendarError::TokenError(e.to_string()))?;

        let new_access_token = token_result.access_token().secret();
        let expires_in = token_result.expires_in()
            .unwrap_or_default()
            .as_secs() as i32;
        println!("New token received, expires in {} seconds", expires_in);

        // Update the access token in the database
        println!("Updating access token in database...");
        state.user_repository.update_google_calendar_access_token(
            user_id,
            new_access_token,
            expires_in,
        ).map_err(|e| CalendarError::TokenError(e.to_string()))?;

        // Retry the calendar request with new token
        println!("Retrying calendar request with new token...");
        let retry_response = client
            .get("https://www.googleapis.com/calendar/v3/calendars/primary/events")
            .header(AUTHORIZATION, format!("Bearer {}", new_access_token))
            .header(ACCEPT, "application/json")
            .query(&[
                ("timeMin", start_time.as_str()),
                ("timeMax", end_time.as_str()),
                ("singleEvents", "true"),
                ("orderBy", "startTime"),
            ])
            .send()
            .await
            .map_err(|e| CalendarError::ApiError(e.to_string()))?;
        println!("Retry response status: {}", retry_response.status());

        if !retry_response.status().is_success() {
            return Err(CalendarError::ApiError(format!(
                "Failed to fetch calendar events after token refresh: {}",
                retry_response.status()
            )));
        }

        let calendar_data: CalendarResponse = retry_response.json().await
            .map_err(|e| CalendarError::ParseError(e.to_string()))?;
        println!("Successfully parsed calendar data after token refresh");

        Ok(calendar_data.items)
    } else if !response.status().is_success() {
        println!("Request failed with status: {}", response.status());
        Err(CalendarError::ApiError(format!(
            "Failed to fetch calendar events: {}",
            response.status()
        )))
    } else {
        println!("Parsing successful response...");
        let calendar_data: CalendarResponse = response.json().await
            .map_err(|e| CalendarError::ParseError(e.to_string()))?;
        println!("Successfully retrieved {} events", calendar_data.items.len());

        Ok(calendar_data.items)
    }
}

// Handler that uses the fetch_calendar_events function
pub async fn get_calendar_events(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(timeframe): Query<TimeframeQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Extract and validate JWT token
    let auth_header = headers.get("Authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "))
        .ok_or_else(|| (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "No authorization token provided"}))
        ))?;
    
    let claims = decode::<Claims>(
        auth_header,
        &DecodingKey::from_secret(std::env::var("JWT_SECRET_KEY")
            .expect("JWT_SECRET_KEY must be set in environment")
            .as_bytes()),
        &Validation::new(Algorithm::HS256)
    )
    .map_err(|e| {
        tracing::error!("Invalid token: {}", e);
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Invalid token"}))
        )
    })?
    .claims;

    // Use the fetch_calendar_events function
    match fetch_calendar_events(&state, claims.sub, timeframe).await {
        Ok(events) => Ok(Json(json!({ "events": events }))),
        Err(e) => {
            let (status, message) = match e {
                CalendarError::NoConnection => (
                    StatusCode::NOT_FOUND,
                    "No Google Calendar connection found".to_string()
                ),
                CalendarError::TokenError(msg) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Token error: {}", msg)
                ),
                CalendarError::ApiError(msg) => (
                    StatusCode::BAD_GATEWAY,
                    format!("API error: {}", msg)
                ),
                CalendarError::ParseError(msg) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Parse error: {}", msg)
                ),
            };
            Err((status, Json(json!({"error": message}))))
        }
    }
}

