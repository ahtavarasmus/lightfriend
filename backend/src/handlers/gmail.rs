use std::sync::Arc;

use crate::handlers::auth_middleware::AuthUser;
use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
};
use chrono::{DateTime, Utc};
use oauth2::TokenResponse;
use reqwest::header::{ACCEPT, AUTHORIZATION};
use serde::{Deserialize, Serialize};
use serde_json::json;

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
    InvalidRefreshToken,
}

pub async fn test_gmail_fetch(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Starting test Gmail fetch for user {}", auth_user.user_id);

    match fetch_gmail_messages(&state, auth_user.user_id, Some(10)).await {
        Ok(messages) => {
            tracing::info!("Fetched {} messages in test", messages.len());
            Ok(Json(json!({ "success": true, "message_count": messages.len() })))
        }
        Err(e) => {
            let (status, message) = match e {
                GmailError::NoConnection => (StatusCode::BAD_REQUEST, "No Gmail connection found"),
                GmailError::TokenError(msg) => (StatusCode::UNAUTHORIZED, &msg),
                GmailError::InvalidRefreshToken => {
                    (StatusCode::UNAUTHORIZED, "Refresh token invalid")
                }
                GmailError::ApiError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, &msg),
                GmailError::ParseError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, &msg),
            };
            tracing::error!("Test fetch failed: {}", message);
            Err((status, Json(json!({ "error": message }))))
        }
    }
}

pub async fn gmail_status(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Checking Gmail status for user {}", auth_user.user_id);

    match state.user_repository.has_active_gmail(auth_user.user_id) {
        Ok(has_connection) => Ok(Json(json!({
            "connected": has_connection,
            "user_id": auth_user.user_id,
        }))),
        Err(e) => {
            tracing::error!("Failed to check Gmail status: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Failed to check Gmail status", "details": e.to_string() })),
            ))
        }
    }
}

pub async fn handle_gmail_fetching(
    state: &AppState,
    user_id: i32,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Starting Gmail fetch for user {}", user_id);

    if !state
        .user_repository
        .has_active_gmail(user_id)
        .map_err(|e| {
            tracing::error!("Error checking Gmail connection: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
        })?
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "No active Gmail connection" })),
        ));
    }

    match fetch_gmail_messages(state, user_id, Some(10)).await {
        Ok(messages) => {
            let formatted_messages: Vec<_> = messages
                .into_iter()
                .map(|m| {
                    json!({
                        "id": m.id,
                        "thread_id": m.thread_id,
                        "subject": m.subject.unwrap_or_else(|| "No subject".to_string()),
                        "from": m.from.unwrap_or_else(|| "Unknown sender".to_string()),
                        "date": m.date.map(|dt| dt.to_rfc3339()),
                        "snippet": m.snippet.unwrap_or_else(|| "No preview".to_string())
                    })
                })
                .collect();

            tracing::info!("Returning {} messages", formatted_messages.len());
            Ok(Json(json!({ "messages": formatted_messages })))
        }
        Err(e) => {
            let (status, message) = match e {
                GmailError::NoConnection => (StatusCode::BAD_REQUEST, "No Gmail connection"),
                GmailError::TokenError(msg) => (StatusCode::UNAUTHORIZED, msg.as_str()),
                GmailError::InvalidRefreshToken => {
                    (StatusCode::UNAUTHORIZED, "Refresh token invalid")
                }
                GmailError::ApiError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.as_str()),
                GmailError::ParseError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg.as_str()),
            };
            tracing::error!("Fetch failed: {}", message);
            Err((status, Json(json!({ "error": message }))))
        }
    }
}

/// Fetches Gmail messages with configurable max results.
async fn fetch_gmail_messages(
    state: &AppState,
    user_id: i32,
    max_results: Option<u32>,
) -> Result<Vec<GmailMessage>, GmailError> {
    tracing::info!("Fetching Gmail messages for user {}", user_id);

    let client = reqwest::Client::new();
    let mut access_token = get_valid_access_token(state, user_id, &client).await?;
    let mut all_message_ids = Vec::new();
    let mut page_token: Option<String> = None;
    let max_retries = 3;
    let mut retries = 0;

    // Fetch message IDs with pagination
    loop {
        let mut request = client
            .get("https://www.googleapis.com/gmail/v1/users/me/messages")
            .header(AUTHORIZATION, format!("Bearer {}", access_token))
            .header(ACCEPT, "application/json");

        if let Some(max) = max_results {
            request = request.query(&[("maxResults", max)]);
        }
        if let Some(token) = &page_token {
            request = request.query(&[("pageToken", token)]);
        }

        let response = request.send().await.map_err(|e| {
            tracing::error!("API request failed: {}", e);
            GmailError::ApiError(e.to_string())
        })?;

        match response.status() {
            StatusCode::OK => {
                let gmail_data: GmailResponse = response.json().await.map_err(|e| {
                    tracing::error!("Failed to parse response: {}", e);
                    GmailError::ParseError(e.to_string())
                })?;
                if let Some(messages) = gmail_data.messages {
                    all_message_ids.extend(messages);
                }
                page_token = gmail_data.next_page_token;
                retries = 0; // Reset retries on success
            }
            StatusCode::UNAUTHORIZED => {
                if retries >= max_retries {
                    return Err(GmailError::TokenError(
                        "Max token refresh retries exceeded".to_string(),
                    ));
                }
                tracing::info!("Token expired, refreshing (attempt {}/{})", retries + 1, max_retries);
                access_token = get_valid_access_token(state, user_id, &client).await?;
                retries += 1;
                continue;
            }
            status => {
                let error_body = response.text().await.unwrap_or_default();
                tracing::error!("API error {}: {}", status, error_body);
                return Err(GmailError::ApiError(format!(
                    "Failed with status {}: {}",
                    status, error_body
                )));
            }
        }

        if page_token.is_none() || max_results.is_some() && all_message_ids.len() >= max_results.unwrap() as usize {
            break;
        }
    }

    tracing::info!("Fetched {} message IDs", all_message_ids.len());

    // Fetch message details (simplified; consider batching in production)
    let mut detailed_messages = Vec::new();
    for message_id in all_message_ids {
        let url = format!(
            "https://www.googleapis.com/gmail/v1/users/me/messages/{}?fields=id,threadId,snippet,payload(headers),internalDate",
            message_id.id
        );
        let response = client
            .get(&url)
            .header(AUTHORIZATION, format!("Bearer {}", access_token))
            .header(ACCEPT, "application/json")
            .send()
            .await
            .map_err(|e| GmailError::ApiError(e.to_string()))?;

        if response.status() == StatusCode::UNAUTHORIZED {
            access_token = get_valid_access_token(state, user_id, &client).await?;
            continue; // Retry with new token in next iteration
        } else if !response.status().is_success() {
            tracing::warn!("Skipping message {} due to status {}", message_id.id, response.status());
            continue;
        }

        let detail: GmailMessageDetail = response.json().await.map_err(|e| GmailError::ParseError(e.to_string()))?;
        let get_header = |name: &str| {
            detail
                .payload
                .headers
                .iter()
                .find(|h| h.name.eq_ignore_ascii_case(name))
                .map(|h| h.value.clone())
        };

        detailed_messages.push(GmailMessage {
            id: detail.id,
            thread_id: detail.thread_id,
            subject: get_header("subject"),
            from: get_header("from"),
            date: Some(detail.internal_date),
            snippet: detail.snippet,
        });
    }

    tracing::info!("Fetched {} detailed messages", detailed_messages.len());
    Ok(detailed_messages)
}

/// Retrieves or refreshes an access token.
async fn get_valid_access_token(
    state: &AppState,
    user_id: i32,
    client: &reqwest::Client,
) -> Result<String, GmailError> {
    let (access_token, refresh_token) = state
        .user_repository
        .get_gmail_tokens(user_id)
        .map_err(|e| GmailError::TokenError(e.to_string()))?
        .ok_or(GmailError::NoConnection)?;

    // Attempt a quick validation request (optional; here we assume refresh on 401)
    let token_result = state
        .gmail_oauth_client
        .exchange_refresh_token(&oauth2::RefreshToken::new(refresh_token))
        .request_async(client)
        .await;

    match token_result {
        Ok(token) => {
            let new_access_token = token.access_token().secret().to_string();
            let expires_in = token.expires_in().unwrap_or_default().as_secs() as i32;
            state
                .user_repository
                .update_gmail_access_token(user_id, &new_access_token, expires_in)
                .map_err(|e| GmailError::TokenError(e.to_string()))?;
            Ok(new_access_token)
        }
        Err(e) => {
            tracing::error!("Refresh token failed: {}", e);
            state
                .user_repository
                .remove_gmail_connection(user_id)
                .map_err(|e| GmailError::TokenError(e.to_string()))?;
            Err(GmailError::InvalidRefreshToken)
        }
    }
}
