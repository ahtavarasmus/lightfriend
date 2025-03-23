use std::sync::Arc;
use crate::handlers::auth_middleware::AuthUser;
use axum::{
    extract::State,
    response::Json,
    http::StatusCode,
};
use serde_json::json;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use oauth2::TokenResponse;
use reqwest::header::{AUTHORIZATION, ACCEPT};

use crate::AppState;


#[derive(Debug, Deserialize, Serialize)]
pub struct GmailMessage {
    pub id: String,
    pub thread_id: String,
    pub subject: Option<String>,
    pub from: Option<String>,
    pub date: Option<DateTime<Utc>>,
    pub snippet: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GmailResponse {
    pub messages: Option<Vec<GmailMessageId>>,
    pub next_page_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GmailMessageId {
    pub id: String,
    pub thread_id: String,
}

#[derive(Debug, Deserialize)]
struct GmailMessageDetail {
    pub id: String,
    pub thread_id: String,
    pub snippet: Option<String>,
    pub payload: GmailPayload,
    #[serde(with = "gmail_date_format")]
    pub internal_date: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct GmailPayload {
    pub headers: Vec<GmailHeader>,
}

#[derive(Debug, Deserialize)]
struct GmailHeader {
    pub name: String,
    pub value: String,
}

mod gmail_date_format {
    use chrono::{DateTime, TimeZone, Utc};
    use serde::{self, Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let timestamp_ms: i64 = Deserialize::deserialize(deserializer)?;
        Utc.timestamp_millis_opt(timestamp_ms)
            .single()
            .ok_or_else(|| serde::de::Error::custom("invalid timestamp"))
    }
}

#[derive(Debug)]
pub enum GmailError {
    NoConnection,
    TokenError(String),
    ApiError(String),
    ParseError(String),
}

pub async fn test_gmail_fetch(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Starting test Gmail fetch");


    // Test fetch Gmail messages
    match fetch_gmail_messages(&state, auth_user.user_id).await {
        Ok(messages) => {
            tracing::info!("Successfully fetched {} messages in test", messages.len());
            Ok(Json(json!({ "success": true, "message_count": messages.len() })))
        },
        Err(e) => {
            let error_message = match e {
                GmailError::NoConnection => "No Gmail connection found".to_string(),
                GmailError::TokenError(msg) => msg,
                GmailError::ApiError(msg) => msg,
                GmailError::ParseError(msg) => msg,
            };
            tracing::error!("Test fetch failed: {}", error_message);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": error_message
                }))
            ))
        }
    }
}

pub async fn gmail_status(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Checking Gmail connection status");

    // Check if user has active Gmail connection
    match state.user_repository.has_active_gmail(auth_user.user_id) {
        Ok(has_connection) => {
            tracing::info!("Successfully checked Gmail connection status for user {}: {}", auth_user.user_id, has_connection);
            Ok(Json(json!({
                "connected": has_connection,
                "user_id": auth_user.user_id,
            })))
        },
        Err(e) => {
            tracing::error!("Failed to check Gmail connection status: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to check Gmail connection status",
                    "details": e.to_string()
                }))
            ))
        }
    }
}

pub async fn handle_gmail_fetching(
    state: &AppState,
    user_id: i32,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Starting Gmail tool call for user: {}", user_id);
    
    // Check if user has active Gmail connection
    tracing::info!("Checking if user has active Gmail connection");
    match state.user_repository.has_active_gmail(user_id) {
        Ok(has_connection) => {
            tracing::info!("No errors checking active Gmail connection");
            if !has_connection {
                tracing::info!("User does not have active Gmail connection");
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "error": "No active Gmail connection found"
                    }))
                ));
            }
            tracing::info!("User has active Gmail connection");
        },
        Err(e) => {
            tracing::error!("Error checking Gmail connection status: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to check Gmail connection status",
                    "details": e.to_string()
                }))
            ));
        }
    }


    // Fetch Gmail messages
    tracing::info!("Fetching Gmail messages");
    match fetch_gmail_messages(state, user_id).await {
        Ok(messages) => {
            tracing::info!("Successfully fetched {} messages", messages.len());
            
            // Format messages into a more readable response
            let formatted_messages: Vec<serde_json::Value> = messages.into_iter()
                .map(|message| {
                    let subject = message.subject.unwrap_or_else(|| "No subject".to_string());
                    tracing::info!("Formatting message: {}", subject);

                    json!({
                        "id": message.id,
                        "thread_id": message.thread_id,
                        "subject": subject,
                        "from": message.from.unwrap_or_else(|| "Unknown sender".to_string()),
                        "date": message.date.map(|dt| dt.to_rfc3339()),
                        "snippet": message.snippet.unwrap_or_else(|| "No preview available".to_string())
                    })
                })
                .collect();

            tracing::info!("Returning {} formatted messages", formatted_messages.len());
            Ok(Json(json!({
                "messages": formatted_messages
            })))
        },
        Err(e) => {
            let error_message = match e {
                GmailError::NoConnection => {
                    tracing::error!("Error: No Gmail connection found");
                    "No Gmail connection found".to_string()
                },
                GmailError::TokenError(msg) => {
                    tracing::error!("Error: Token error - {}", msg);
                    format!("Token error: {}", msg)
                },
                GmailError::ApiError(msg) => {
                    tracing::error!("Error: API error - {}", msg);
                    format!("API error: {}", msg)
                },
                GmailError::ParseError(msg) => {
                    tracing::error!("Error: Parse error - {}", msg);
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


async fn fetch_gmail_messages(
    state: &AppState,
    user_id: i32,
) -> Result<Vec<GmailMessage>, GmailError> {
    tracing::info!("Starting to fetch Gmail messages...");

    // Get Gmail tokens
    tracing::info!("Getting Gmail tokens for user_id: {}", user_id);
    let (access_token, refresh_token) = match state.user_repository.get_gmail_tokens(user_id) {
        Ok(Some((access, refresh))) => {
            tracing::info!("Successfully retrieved and decrypted tokens");
            tracing::debug!("Access token length: {}, Refresh token length: {}", 
                access.len(), refresh.len());
            (access, refresh)
        },
        Ok(None) => {
            tracing::error!("No active Gmail connection found");
            return Err(GmailError::NoConnection);
        },
        Err(e) => {
            tracing::error!("Failed to get Gmail tokens: {}", e);
            return Err(GmailError::TokenError(format!("Failed to decrypt tokens: {}", e)));
        }
    };

    // Create HTTP client for Gmail API
    let client = reqwest::Client::new();
    

    // First, get message IDs
    let mut all_messages = Vec::new();
    let mut page_token: Option<String> = None;

    loop {
        tracing::info!("Making request to Gmail API for message IDs...");
        let mut request = client
            .get("https://www.googleapis.com/gmail/v1/users/me/messages")
            .header(AUTHORIZATION, format!("Bearer {}", access_token))
            .header(ACCEPT, "application/json")
            .query(&[("maxResults", 10)]);

        if let Some(token) = &page_token {
            request = request.query(&[("pageToken", token)]);
        }

        let response = request
            .send()
            .await
            .map_err(|e| {
                tracing::error!("Failed to send request to Gmail API: {}", e);
                GmailError::ApiError(e.to_string())
            })?;


        let status = response.status();
        tracing::info!("Gmail API Response Status: {}", status);

        if status == StatusCode::UNAUTHORIZED {
            tracing::info!("Token expired, starting refresh process...");
            
            let http_client = reqwest::ClientBuilder::new()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("Client should build");

            let token_result = state
                .gmail_oauth_client
                .exchange_refresh_token(&oauth2::RefreshToken::new(refresh_token.clone()))
                .request_async(&http_client)
                .await
                .map_err(|e| GmailError::TokenError(e.to_string()))?;

            let new_access_token = token_result.access_token().secret();
            let expires_in = token_result.expires_in()
                .unwrap_or_default()
                .as_secs() as i32;

            tracing::info!("New token received, expires in {} seconds", expires_in);

            // Update the access token in the database
            tracing::info!("Updating access token in database...");
            state.user_repository.update_gmail_access_token(
                user_id,
                &new_access_token,
                expires_in,
            ).map_err(|e| GmailError::TokenError(e.to_string()))?;

            // Retry the request with new token
            tracing::info!("Retrying Gmail request with new token...");
            let retry_response = client
                .get("https://www.googleapis.com/gmail/v1/users/me/messages")
                .header(AUTHORIZATION, format!("Bearer {}", new_access_token))
                .header(ACCEPT, "application/json")
                .query(&[("maxResults", 10)])
                .send()
                .await
                .map_err(|e| GmailError::ApiError(e.to_string()))?;

            let retry_status = retry_response.status();
            tracing::info!("Retry response status: {}", retry_status);

            if !retry_status.is_success() {
                let error_body = retry_response.text().await.unwrap_or_default();
                tracing::error!("Gmail API Error Response after token refresh: {}", error_body);
                return Err(GmailError::ApiError(format!(
                    "Failed to fetch messages after token refresh: {}",
                    retry_status
                )));
            }

            let gmail_data: GmailResponse = retry_response.json().await
                .map_err(|e| GmailError::ParseError(e.to_string()))?;

            if let Some(messages) = gmail_data.messages {
                all_messages.extend(messages);
            }
            page_token = gmail_data.next_page_token;

        } else if !status.is_success() {
            let error_body = response.text().await.unwrap_or_default();
            tracing::error!("Gmail API Error Response: {}", error_body);
            return Err(GmailError::ApiError(format!(
                "Failed to fetch messages: {}",
                status
            )));
        } else {
            let gmail_data: GmailResponse = response.json().await
                .map_err(|e| GmailError::ParseError(e.to_string()))?;

            if let Some(messages) = gmail_data.messages {
                all_messages.extend(messages);
            }
            page_token = gmail_data.next_page_token;
        }

        if page_token.is_none() {
            break;
        }
    }

    tracing::info!("Retrieved {} message IDs, fetching details...", all_messages.len());

    // Now fetch details for each message
    let mut detailed_messages = Vec::new();
    for message_id in all_messages {
        let detail_url = format!(
            "https://www.googleapis.com/gmail/v1/users/me/messages/{}",
            message_id.id
        );


        let response = client
            .get(&detail_url)
            .header(AUTHORIZATION, format!("Bearer {}", access_token))
            .header(ACCEPT, "application/json")
            .send()
            .await
            .map_err(|e| {
                tracing::error!("Failed to fetch message detail for {}: {}", message_id.id, e);
                GmailError::ApiError(e.to_string())
            })?;

        if !response.status().is_success() {
            tracing::error!("Failed to fetch message detail for {}: {}", message_id.id, response.status());
            continue;
        }

        let message_detail: GmailMessageDetail = response.json().await
            .map_err(|e| {
                tracing::error!("Failed to parse message detail for {}: {}", message_id.id, e);
                GmailError::ParseError(e.to_string())
            })?;

        let get_header = |name: &str| -> Option<String> {
            message_detail.payload.headers.iter()
                .find(|h| h.name.eq_ignore_ascii_case(name))
                .map(|h| h.value.clone())
        };

        detailed_messages.push(GmailMessage {
            id: message_detail.id,
            thread_id: message_detail.thread_id,
            subject: get_header("subject"),
            from: get_header("from"),
            date: Some(message_detail.internal_date),
            snippet: message_detail.snippet,
        });
    }

    tracing::info!("Successfully retrieved {} detailed messages", detailed_messages.len());
    Ok(detailed_messages)
}

