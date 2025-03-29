use std::sync::Arc;
use crate::handlers::auth_middleware::AuthUser;
use axum::{
    extract::{State, Query},
    response::Json,
    http::StatusCode,
};
use serde_json::json;
use serde::{Deserialize, Serialize};
use oauth2::TokenResponse;
use reqwest::header::{AUTHORIZATION, ACCEPT};
use chrono::{DateTime, Utc};

use crate::AppState;

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
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Checking Google Calendar connection status");

    // Check if user has active Google Calendar connection
    match state.user_repository.has_active_google_calendar(auth_user.user_id) {
        Ok(has_connection) => {
            tracing::info!("Successfully checked calendar connection status for user {}: {}", auth_user.user_id, has_connection);
            Ok(Json(json!({
                "connected": has_connection,
                "user_id": auth_user.user_id,
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

pub async fn get_calendar_email(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    match state.user_repository.get_google_calendar_tokens(auth_user.user_id) {
        Ok(Some((access_token, _))) => {
            let client = reqwest::Client::new();
            let response = client
                .get("https://www.googleapis.com/oauth2/v2/userinfo")
                .header("Authorization", format!("Bearer {}", access_token))
                .send()
                .await;

            match response {
                Ok(resp) => {
                    if resp.status().is_success() {
                        if let Ok(user_info) = resp.json::<serde_json::Value>().await {
                            if let Some(email) = user_info.get("email").and_then(|e| e.as_str()) {
                                return Ok(Json(json!({
                                    "email": email
                                })));
                            }
                        }
                    }
                    Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                        "error": "Failed to get user email"
                    }))))
                }
                Err(_) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
                    "error": "Failed to fetch user info"
                }))))
            }
        }
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(json!({
            "error": "No active Google Calendar connection found"
        })))),
        Err(_) => Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({
            "error": "Failed to get calendar tokens"
        }))))
    }
}

#[derive(Debug)]
pub enum CalendarError {
    NoConnection,
    TokenError(String),
    ApiError(String),
    ParseError(String),
}
pub async fn handle_calendar_fetching_route(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {

    // Extract start and end times from query parameters
    let start = match params.get("start") {
        Some(s) => s,
        None => return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Missing start parameter"}))
        )),
    };

    let end = match params.get("end") {
        Some(e) => e,
        None => return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Missing end parameter"}))
        )),
    };

    // Call the existing handler function
    handle_calendar_fetching(&state, auth_user.user_id, start, end).await
}

pub async fn handle_calendar_fetching(
    state: &AppState,
    user_id: i32,
    start: &str,
    end: &str,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    println!("Starting calendar tool call for user: {}", user_id);
    
    // Parse start and end times
    println!("Parsing datetime strings");
    let parse_datetime = |datetime_str: &str| {
        chrono::DateTime::parse_from_rfc3339(datetime_str)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .map_err(|_| "Invalid datetime format")
    };

    let start_time = match parse_datetime(start) {
        Ok(time) => {
            println!("Successfully parsed start time: {}", time);
            time
        },
        Err(e) => {
            println!("Failed to parse start time: {}", e);
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": format!("Invalid start time: {}", e)
                }))
            ));
        }
    };

    let end_time = match parse_datetime(end) {
        Ok(time) => {
            println!("Successfully parsed end time: {}", time);
            time
        },
        Err(e) => {
            println!("Failed to parse end time: {}", e);
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": format!("Invalid end time: {}", e)
                }))
            ));
        }
    };

    // Check if user has active Google Calendar connection
    println!("Checking if user has active Google Calendar connection");
    match state.user_repository.has_active_google_calendar(user_id) {
        Ok(has_connection) => {
            println!("no errors checking active google calendar connection");
            if !has_connection {
                println!("User does not have active Google Calendar connection");
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "error": "No active Google Calendar connection found"
                    }))
                ));
            }
            println!("User has active Google Calendar connection");
        },
        Err(e) => {
            println!("Error checking calendar connection status: {}", e);
            tracing::error!("Failed to check calendar connection status: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to check calendar connection status",
                    "details": e.to_string()
                }))
            ));
        }
    }

    let timeframe = TimeframeQuery {
        start: start_time,
        end: end_time,
    };

    // Fetch calendar events
    println!("Fetching calendar events");
    match fetch_calendar_events(state, user_id, timeframe).await {
        Ok(events) => {
            println!("Successfully fetched {} events", events.len());
            // Format events into a more readable response
            let formatted_events: Vec<serde_json::Value> = events.into_iter()
                .map(|event| {
                    let start_time = event.start.date_time
                        .map(|dt| dt.to_rfc3339())
                        .or(event.start.date);
                    
                    let end_time = event.end.date_time
                        .map(|dt| dt.to_rfc3339())
                        .or(event.end.date);

                    let summary = event.summary.unwrap_or_else(|| "No title".to_string());
                    println!("Formatting event: {}", summary);

                    json!({
                        "summary": summary,
                        "start": start_time,
                        "end": end_time,
                        "status": event.status.unwrap_or_else(|| "confirmed".to_string())
                    })
                })
                .collect();

            println!("Returning {} formatted events", formatted_events.len());
            Ok(Json(json!({
                "events": formatted_events
            })))
        },
        Err(e) => {
            let error_message = match e {
                CalendarError::NoConnection => {
                    println!("Error: No Google Calendar connection found");
                    "No Google Calendar connection found".to_string()
                },
                CalendarError::TokenError(msg) => {
                    println!("Error: Token error - {}", msg);
                    format!("Token error: {}", msg)
                },
                CalendarError::ApiError(msg) => {
                    println!("Error: API error - {}", msg);
                    format!("API error: {}", msg)
                },
                CalendarError::ParseError(msg) => {
                    println!("Error: Parse error - {}", msg);
                    format!("Parse error: {}", msg)
                },
            };

            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": error_message
                }))
            ))
        }
    }
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
            .google_calendar_oauth_client
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

