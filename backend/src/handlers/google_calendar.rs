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
use reqwest::header::{AUTHORIZATION, ACCEPT, CONTENT_TYPE};
use chrono::{DateTime, Utc, Duration};

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
    pub reminders: Option<EventReminders>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct EventReminders {
    #[serde(rename = "useDefault")]
    pub use_default: bool,
    pub overrides: Vec<ReminderOverride>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ReminderOverride {
    pub method: String,
    pub minutes: i32,
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

#[derive(Debug, Deserialize, Serialize)]
struct CalendarListEntry {
    pub id: String,
    pub summary: String,
    #[serde(default)]
    pub primary: bool,
    #[serde(default)]
    pub selected: bool,
}

#[derive(Debug, Deserialize)]
struct CalendarListResponse {
    pub items: Vec<CalendarListEntry>,
}

#[derive(Debug, Deserialize)]
pub struct CreateEventRequest {
    pub start_time: DateTime<Utc>,
    pub duration_minutes: i32,
    pub summary: String,
    pub description: Option<String>,
    pub add_notification: bool,
}

#[derive(Debug, Serialize)]
struct GoogleCalendarEvent {
    summary: String,
    description: Option<String>,
    start: GoogleDateTime,
    end: GoogleDateTime,
    #[serde(skip_serializing_if = "Option::is_none")]
    reminders: Option<Reminders>,
}

#[derive(Debug, Serialize)]
struct GoogleDateTime {
    dateTime: String,
    timeZone: String,
}

#[derive(Debug, Serialize)]
struct Reminders {
    useDefault: bool,
    overrides: Vec<CreateEventReminderOverride>,
}

#[derive(Debug, Serialize)]
struct CreateEventReminderOverride {
    method: String,
    minutes: i32,
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

impl std::fmt::Display for CalendarError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CalendarError::NoConnection => write!(f, "No active Google Calendar connection"),
            CalendarError::TokenError(msg) => write!(f, "Token error: {}", msg),
            CalendarError::ApiError(msg) => write!(f, "API error: {}", msg),
            CalendarError::ParseError(msg) => write!(f, "Parse error: {}", msg),
        }
    }
}

pub async fn create_calendar_event(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(event_request): Json<CreateEventRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    println!("Creating new calendar event for user: {}", auth_user.user_id);

    // Check if user has active Google Calendar connection
    match state.user_repository.has_active_google_calendar(auth_user.user_id) {
        Ok(has_connection) => {
            if !has_connection {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "error": "No active Google Calendar connection found"
                    }))
                ));
            }
        },
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": format!("Failed to check calendar connection: {}", e)
                }))
            ));
        }
    }

    // Get tokens
    let (access_token, refresh_token) = match state.user_repository.get_google_calendar_tokens(auth_user.user_id) {
        Ok(Some((access, refresh))) => (access, refresh),
        Ok(None) => return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": "No active Google Calendar connection found"
            }))
        )),
        Err(e) => return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": format!("Failed to get calendar tokens: {}", e)
            }))
        )),
    };

    // Calculate end time
    let end_time = event_request.start_time + Duration::minutes(event_request.duration_minutes as i64);

    // Create event payload
    let event = GoogleCalendarEvent {
        summary: event_request.summary,
        description: event_request.description,
        start: GoogleDateTime {
            dateTime: event_request.start_time.to_rfc3339(),
            timeZone: "UTC".to_string(),
        },
        end: GoogleDateTime {
            dateTime: end_time.to_rfc3339(),
            timeZone: "UTC".to_string(),
        },
        reminders: if event_request.add_notification {
            Some(Reminders {
                useDefault: false,
                overrides: vec![
                    CreateEventReminderOverride {
                        method: "popup".to_string(),
                        minutes: 10,
                    }
                ],
            })
        } else {
            None
        },
    };

    // Create HTTP client
    let client = reqwest::Client::new();

    // Helper function to create event with given token
    async fn create_event_with_token(
        client: &reqwest::Client,
        access_token: &str,
        event: &GoogleCalendarEvent,
    ) -> Result<serde_json::Value, String> {
        let response = client
            .post("https://www.googleapis.com/calendar/v3/calendars/primary/events")
            .header(AUTHORIZATION, format!("Bearer {}", access_token))
            .header(CONTENT_TYPE, "application/json")
            .json(event)
            .send()
            .await
            .map_err(|e| format!("Failed to create event: {}", e))?;

        if response.status().is_success() {
            response.json().await
                .map_err(|e| format!("Failed to parse response: {}", e))
        } else {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            Err(format!("Failed to create event: {}", error_text))
        }
    }

    // First attempt with current access token
    let result = create_event_with_token(&client, &access_token, &event).await;

    match result {
        Ok(created_event) => {
            Ok(Json(json!({
                "message": "Event created successfully",
                "event": created_event
            })))
        },
        Err(e) => {
            // Check if error might be due to expired token
            if e.contains("401") {
                tracing::info!("Access token expired, attempting to refresh");
                
                // Create HTTP client for token refresh
                let http_client = reqwest::ClientBuilder::new()
                    .redirect(reqwest::redirect::Policy::none())
                    .build()
                    .expect("Client should build");

                // Try to refresh the token
                let token_result = state
                    .google_calendar_oauth_client
                    .exchange_refresh_token(&oauth2::RefreshToken::new(refresh_token.clone()))
                    .request_async(&http_client)
                    .await
                    .map_err(|e| (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "error": format!("Failed to refresh token: {}", e)
                        }))
                    ))?;

                let new_access_token = token_result.access_token().secret();
                let expires_in = token_result.expires_in()
                    .unwrap_or_default()
                    .as_secs() as i32;

                // Update the access token in the database
                state.user_repository.update_google_calendar_access_token(
                    auth_user.user_id,
                    new_access_token.as_str(),
                    expires_in,
                ).map_err(|e| (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "error": format!("Failed to update access token: {}", e)
                    }))
                ))?;

                // Retry with new token
                match create_event_with_token(&client, new_access_token.as_str(), &event).await {
                    Ok(created_event) => {
                        Ok(Json(json!({
                            "message": "Event created successfully after token refresh",
                            "event": created_event
                        })))
                    },
                    Err(retry_error) => {
                        Err((
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({
                                "error": format!("Failed to create event after token refresh: {}", retry_error)
                            }))
                        ))
                    }
                }
            } else {
                // If error is not token-related, return the original error
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "error": e
                    }))
                ))
            }
        }
    }
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
async fn fetch_calendar_list(
    client: &reqwest::Client,
    access_token: &str,
) -> Result<Vec<CalendarListEntry>, CalendarError> {
    println!("Starting to fetch calendar list...");
    let response = client
        .get("https://www.googleapis.com/calendar/v3/users/me/calendarList")
        .header(AUTHORIZATION, format!("Bearer {}", access_token))
        .header(ACCEPT, "application/json")
        .send()
        .await
        .map_err(|e| CalendarError::ApiError(e.to_string()))?;
    println!("fetching calendars");
    println!("Calendar API response status: {}", response.status());

    if response.status() == StatusCode::FORBIDDEN {
        // If we get a 403, we don't have permission to list calendars
        println!("No permission to fetch calendar list, defaulting to primary calendar only");
        return Ok(vec![CalendarListEntry {
            id: "primary".to_string(),
            summary: "Primary Calendar".to_string(),
            primary: true,
            selected: true,
        }]);
    } else if !response.status().is_success() {
        println!("Failed to fetch calendar list with status: {}", response.status());
        return Err(CalendarError::ApiError(format!(
            "Failed to fetch calendar list: {}",
            response.status()
        )));
    }

    let calendar_list: CalendarListResponse = response.json().await
        .map_err(|e| CalendarError::ParseError(e.to_string()))?;
    
    println!("Successfully fetched {} calendars", calendar_list.items.len());

    Ok(calendar_list.items)
}

async fn fetch_events_from_calendar(
    client: &reqwest::Client,
    access_token: &str,
    calendar_id: &str,
    start_time: &str,
    end_time: &str,
) -> Result<Vec<CalendarEvent>, CalendarError> {
    // URL encode the calendar ID to handle special characters
    let encoded_calendar_id = urlencoding::encode(calendar_id);
    
    let response = client
        .get(&format!(
            "https://www.googleapis.com/calendar/v3/calendars/{}/events",
            encoded_calendar_id
        ))
        .header(AUTHORIZATION, format!("Bearer {}", access_token))
        .header(ACCEPT, "application/json")
        .query(&[
            ("timeMin", start_time),
            ("timeMax", end_time),
            ("singleEvents", "true"),
            ("orderBy", "startTime"),
        ])
        .send()
        .await
        .map_err(|e| CalendarError::ApiError(e.to_string()))?;

    // Handle 404 errors for holiday calendars gracefully
    if response.status() == reqwest::StatusCode::NOT_FOUND && calendar_id.contains("#holiday@group") {
        tracing::debug!("Holiday calendar not found (expected): {}", calendar_id);
        return Ok(Vec::new());
    }

    if !response.status().is_success() {
        return Err(CalendarError::ApiError(format!(
            "Failed to fetch events for calendar {}: {}",
            calendar_id, response.status()
        )));
    }

    let calendar_data: CalendarResponse = response.json().await
        .map_err(|e| CalendarError::ParseError(e.to_string()))?;

    Ok(calendar_data.items)
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


    // Create HTTP client for Google Calendar API
    let client = reqwest::Client::new();
    
    // Format the dates for Google Calendar API
    let start_time = timeframe.start.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let end_time = timeframe.end.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    println!("Formatted time range: {} to {}", start_time, end_time);

    async fn fetch_with_token(
        client: &reqwest::Client,
        state: &AppState,
        user_id: i32,
        access_token: &str,
        refresh_token: &str,
        start_time: &str,
        end_time: &str
    ) -> Result<Vec<CalendarEvent>, CalendarError> {
        // First try to fetch calendar list to check permissions
        println!("Attempting to fetch calendar list...");
        match fetch_calendar_list(client, access_token).await {
            Ok(calendars) => {
                println!("Successfully fetched calendar list with {} calendars", calendars.len());
                let mut all_events = Vec::new();
                
                for calendar in calendars {
                    if calendar.selected {
                        println!("Fetching events from calendar: {}", calendar.id);
                        match fetch_events_from_calendar(
                            client,
                            access_token,
                            &calendar.id,
                            start_time,
                            end_time
                        ).await {
                            Ok(mut events) => {
                                println!("Successfully fetched {} events from calendar {}", events.len(), calendar.summary);
                                all_events.append(&mut events);
                            },
                            Err(e) => {
                                println!("Error fetching events from calendar {}: {}", calendar.id, e);
                                continue;
                            }
                        }
                    }
                }
                Ok(all_events)
            },
            Err(e) => {
                println!("Failed to fetch calendar list (possibly due to permissions), falling back to primary calendar: {}", e);
                // Fall back to fetching just the primary calendar
                fetch_events_from_calendar(
                    client,
                    access_token,
                    "primary",
                    start_time,
                    end_time
                ).await
            }
        }
    }

    let result = fetch_with_token(
        &client,
        state,
        user_id,
        &access_token,
        &refresh_token,
        &start_time,
        &end_time
    ).await;

    match result {
        Ok(events) => Ok(events),
        Err(CalendarError::ApiError(e)) if e.contains("401") => {
            println!("Token expired, starting refresh process...");
            tracing::info!("Access token expired, attempting to refresh");
            
            let http_client = reqwest::ClientBuilder::new()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("Client should build");

            println!("Exchanging refresh token...");
            let token_result = state
                .google_calendar_oauth_client
                .exchange_refresh_token(&oauth2::RefreshToken::new(refresh_token.clone()))
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
                new_access_token.clone().as_str(),
                expires_in,
            ).map_err(|e| CalendarError::TokenError(e.to_string()))?;

            // Retry with new token
            println!("Retrying calendar request with new token...");
            fetch_with_token(
                &client,
                state,
                user_id,
                &new_access_token,
                &refresh_token,
                &start_time,
                &end_time
            ).await
        },
        Err(e) => Err(e),
    }
}

