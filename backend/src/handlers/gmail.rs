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
use base64;
use quoted_printable;

use crate::AppState;

#[derive(Debug, Deserialize, Serialize)]
pub struct GmailMessage {
    pub id: String,
    pub thread_id: String,
    pub subject: Option<String>,
    pub from: Option<String>,
    pub from_email: Option<String>,
    pub date: Option<DateTime<Utc>>,
    pub snippet: Option<String>,
    pub body: Option<String>,
    pub is_read: bool,
}

#[derive(Debug, Deserialize)]
struct GmailResponse {
    pub messages: Option<Vec<GmailMessageId>>,
    pub next_page_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GmailMessageId {
    pub id: String,
    #[serde(rename = "threadId")]
    pub thread_id: String,
}

#[derive(Debug, Deserialize)]
struct GmailMessageDetail {
    pub id: String,
    #[serde(rename = "threadId")]
    pub thread_id: String,
    pub snippet: Option<String>,
    pub payload: GmailPayload,
    #[serde(with = "gmail_date_format", rename = "internalDate")]
    pub internal_date: DateTime<Utc>,
    #[serde(default)]
    pub label_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct GmailPayload {
    pub headers: Vec<GmailHeader>,
    pub body: Option<GmailBody>,
    pub parts: Option<Vec<GmailPart>>,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
}

#[derive(Debug, Deserialize)]
struct GmailBody {
    pub data: Option<String>,
    pub size: i32,
}

#[derive(Debug, Deserialize)]
struct GmailPart {
    pub body: GmailBody,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
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
        let timestamp_str = String::deserialize(deserializer)?;
        let timestamp_ms = timestamp_str.parse::<i64>()
            .map_err(|e| serde::de::Error::custom(format!("failed to parse timestamp: {}", e)))?;
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
    tracing::info!("Starting test Gmail fetch for user {} at {}", auth_user.user_id, chrono::Utc::now());

    match fetch_gmail_messages(&state, auth_user.user_id, Some(3)).await {
        Ok(messages) => {
            tracing::info!("Fetched {} messages in test", messages.len());
            Ok(Json(json!({ "success": true, "message_count": messages.len() })))
        }
        Err(e) => {
            let (status, message) = match e {
                GmailError::NoConnection => (StatusCode::BAD_REQUEST, "No Gmail connection found".to_string()),
                GmailError::TokenError(msg) => (StatusCode::UNAUTHORIZED, msg),
                GmailError::InvalidRefreshToken => {
                    (StatusCode::UNAUTHORIZED, "Refresh token invalid".to_string())
                }
                GmailError::ApiError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
                GmailError::ParseError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
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

/// Extracts the email body from the payload, handling both plain text and HTML parts
fn extract_email_body(payload: &GmailPayload) -> Option<String> {
    // Helper function to decode base64 and quoted-printable content
    fn decode_content(data: &str, transfer_encoding: &str) -> Option<String> {
        // Decode base64 (Gmail uses URL-safe base64)
        let cleaned = data.replace('-', "+").replace('_', "/");
        let padding_needed = cleaned.len() % 4;
        let padded = if padding_needed > 0 {
            cleaned + &"=".repeat(4 - padding_needed)
        } else {
            cleaned
        };
        let base64_decoded = base64::decode(&padded).ok()?;

        // Convert to string, handling invalid UTF-8
        let decoded_str = String::from_utf8_lossy(&base64_decoded).into_owned();

        // Apply quoted-printable decoding if necessary
        let final_content = if transfer_encoding.to_lowercase() == "quoted-printable" {
            quoted_printable::decode(decoded_str.as_bytes(), quoted_printable::ParseMode::Robust)
                .ok()
                .and_then(|bytes| String::from_utf8_lossy(&bytes).into_owned().into())
        } else {
            Some(decoded_str)
        };

        final_content.map(|content| content.replace("\r\n", "\n"))
    }

    // Get content transfer encoding from headers
    let transfer_encoding = payload.headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case("Content-Transfer-Encoding"))
        .map(|h| h.value.as_str())
        .unwrap_or("7bit");

    // First try to get plain text content from parts
    if let Some(parts) = &payload.parts {
        for part in parts {
            if part.mime_type.to_lowercase().contains("text/plain") {
                if let Some(data) = &part.body.data {
                    if let Some(text) = decode_content(data, transfer_encoding) {
                        return Some(text);
                    }
                }
            }
        }
        // If no plain text, try HTML
        for part in parts {
            if part.mime_type.to_lowercase().contains("text/html") {
                if let Some(data) = &part.body.data {
                    if let Some(text) = decode_content(data, transfer_encoding) {
                        // Simple HTML cleanup for readability
                        let cleaned = text
                            .replace("<br>", "\n")
                            .replace("<br/>", "\n")
                            .replace("<br />", "\n")
                            .replace("</p>", "\n")
                            .replace("</div>", "\n");
                        return Some(cleaned);
                    }
                }
            }
        }
    }

    // If no parts or no text found, try the main body
    if let Some(body) = &payload.body {
        if let Some(data) = &body.data {
            if let Some(text) = decode_content(data, transfer_encoding) {
                return Some(text);
            }
        }
    }

    None
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

    match fetch_gmail_messages(state, user_id, Some(1)).await {
        Ok(messages) => {
            let formatted_messages: Vec<_> = messages
                .into_iter()
                .map(|m| {
                    json!({
                        "id": m.id,
                        "thread_id": m.thread_id,
                        "subject": m.subject.unwrap_or_else(|| "No subject".to_string()),
                        "from": m.from.unwrap_or_else(|| "Unknown sender".to_string()),
                        "from_email": m.from_email.unwrap_or_else(|| "unknown@email.com".to_string()),
                        "date": m.date.map(|dt| dt.to_rfc3339()),
                        "snippet": m.snippet.unwrap_or_else(|| "No preview".to_string()),
                        "body": m.body.unwrap_or_else(|| "No content".to_string()),
                        "is_read": m.is_read
                    })
                })
                .collect();
            // print the emails here 

            tracing::info!("Returning {} messages", formatted_messages.len());
            Ok(Json(json!({ "messages": formatted_messages })))
        }
        Err(e) => {
            let (status, message) = match e {
                GmailError::NoConnection => (StatusCode::BAD_REQUEST, "No Gmail connection found".to_string()),
                GmailError::TokenError(msg) => (StatusCode::UNAUTHORIZED, msg),
                GmailError::InvalidRefreshToken => {
                    (StatusCode::UNAUTHORIZED, "Refresh token invalid".to_string())
                }
                GmailError::ApiError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
                GmailError::ParseError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
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
    tracing::info!("Fetching Gmail messages for user {} with max_results: {:?}", user_id, max_results);

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
            tracing::error!("Gmail API request failed: {}", e);
            GmailError::ApiError(e.to_string())
        })?;

        tracing::debug!("Gmail API response status: {}", response.status());

        match response.status() {
            StatusCode::OK => {
                let response_text = response.text().await.map_err(|e| {
                    tracing::error!("Failed to get response text: {}", e);
                    GmailError::ParseError(e.to_string())
                })?;
                
                tracing::debug!("Gmail API response body: {}", response_text);
                
                let gmail_data: GmailResponse = serde_json::from_str(&response_text).map_err(|e| {
                    tracing::error!("Failed to parse Gmail response JSON: {}\nResponse body: {}", e, response_text);
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
            "https://www.googleapis.com/gmail/v1/users/me/messages/{}?fields=id,threadId,snippet,payload(headers,body,parts,mimeType),internalDate,labelIds",
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

        let response_text = response.text().await.map_err(|e| {
            tracing::error!("Failed to get message detail response text: {}", e);
            GmailError::ParseError(e.to_string())
        })?;
        
        
        let detail: GmailMessageDetail = serde_json::from_str(&response_text).map_err(|e| {
            tracing::error!("Failed to parse message detail JSON: {}\nResponse body: {}", e, response_text);
            GmailError::ParseError(e.to_string())
        })?;
        let get_header = |name: &str| {
            detail
                .payload
                .headers
                .iter()
                .find(|h| h.name.eq_ignore_ascii_case(name))
                .map(|h| h.value.clone())
        };

        let from_header = get_header("from");
        let from_email = from_header.as_ref().and_then(|from| {
            if let Some(start) = from.find('<') {
                if let Some(end) = from.find('>') {
                    return Some(from[start + 1..end].to_string());
                }
            }
            None
        });

        let body = extract_email_body(&detail.payload);
        let is_read = !detail.label_ids.contains(&"UNREAD".to_string());

        detailed_messages.push(GmailMessage {
            id: detail.id,
            thread_id: detail.thread_id,
            subject: get_header("subject"),
            from: from_header,
            from_email,
            date: Some(detail.internal_date),
            snippet: detail.snippet,
            body,
            is_read,
        });
    }

    tracing::info!("Fetched {} detailed messages", detailed_messages.len());
    
    // Debug print each message
    for (index, message) in detailed_messages.iter().enumerate() {
        tracing::debug!(
            "Message {}: ID: {}, Subject: {:?}, From: {:?}, Date: {:?}, Is Read: {}, Snippet length: {:#?}, Body length: {:#?}",
            index + 1,
            message.id,
            message.subject,
            message.from,
            message.date,
            message.is_read,
            message.snippet,
            message.body
        );
    }
    
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
                .delete_gmail_connection(user_id)
                .map_err(|e| GmailError::TokenError(e.to_string()))?;
            Err(GmailError::InvalidRefreshToken)
        }
    }
}
