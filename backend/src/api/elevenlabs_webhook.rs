use axum::{
    Json,
    extract::State,
    response::Response,
    http::{StatusCode, Request, HeaderMap},
    body::{Body, to_bytes}
};
use axum::middleware;
use std::sync::Arc;
use crate::AppState;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};


// ========================================
// Structs for post_call_transcription webhook
// ========================================

#[derive(Debug, Deserialize, Serialize)]
pub struct WebhookPayload {
    #[serde(rename = "type")]
    pub type_field: String,  // "type" is a reserved keyword in Rust, so we rename it
    pub event_timestamp: u64,
    pub data: WebhookData,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct WebhookData {
    pub conversation_id: String,
    pub status: String,
    pub metadata: Metadata,
    pub analysis: Analysis,
    pub conversation_initiation_client_data: ConversationInitiationClientDataWebhook,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Metadata {
    pub call_duration_secs: i32,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Analysis {
    pub call_successful: String,
    pub transcript_summary: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ConversationInitiationClientDataWebhook {
    pub dynamic_variables: DynamicVariablesWebhook,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct DynamicVariablesWebhook {
    #[serde(deserialize_with = "deserialize_user_id")]
    pub user_id: Option<String>,  // Using Option since it might not always be present
}

// ========================================
// Structs for call_initiation_failure webhook
// ========================================

#[derive(Debug, Deserialize, Serialize)]
pub struct CallInitiationFailurePayload {
    #[serde(rename = "type")]
    pub type_field: String,  // "call_initiation_failure"
    pub event_timestamp: u64,
    pub data: CallInitiationFailureData,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CallInitiationFailureData {
    pub failure_reason: String,  // "busy", "no-answer", "unknown"
    #[serde(default)]
    pub conversation_initiation_client_data: Option<ConversationInitiationClientDataWebhook>,
}

// Add this function at the module level (outside of any struct)
fn deserialize_user_id<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    
    // This will accept either a number or a string
    let value = serde_json::Value::deserialize(deserializer)?;
    
    match value {
        serde_json::Value::String(s) => Ok(Some(s)),
        serde_json::Value::Number(n) => Ok(Some(n.to_string())),
        serde_json::Value::Null => Ok(None),
        _ => Err(D::Error::custom("user_id must be a string or number")),
    }
}


use tracing::error;

use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

// Add these to your existing imports
type HmacSha256 = Hmac<Sha256>;

// Middleware for HMAC validation
pub async fn validate_elevenlabs_hmac(
    headers: HeaderMap,
    request: Request<Body>,
    next: middleware::Next,
) -> Result<Response, StatusCode> {
    tracing::info!("\n=== Starting ElevenLabs HMAC Validation ===");


    // Get the webhook secret from environment
    let secret = match std::env::var("ELEVENLABS_WEBHOOK_SECRET") {
        Ok(key) => {
            tracing::info!("✅ Successfully retrieved ELEVENLABS_WEBHOOK_SECRET");
            key
        },
        Err(e) => {
            tracing::info!("❌ Failed to get ELEVENLABS_WEBHOOK_SECRET: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Get the signature header
    let signature_header = match headers.get("ElevenLabs-Signature") {
        Some(header) => header,
        None => {
            tracing::info!("❌ No ElevenLabs-Signature header found");
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    let signature_str = match signature_header.to_str() {
        Ok(s) => s,
        Err(e) => {
            tracing::info!("❌ Error converting signature header to string: {}", e);
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    // Parse the signature header (t=timestamp,v0=hash)
    let parts: Vec<&str> = signature_str.split(',').collect();
    let timestamp = parts.iter()
        .find(|&&part| part.starts_with("t="))
        .and_then(|part| part.strip_prefix("t="))
        .ok_or(StatusCode::UNAUTHORIZED)?;
    
    let signature = parts.iter()
        .find(|&&part| part.starts_with("v0="))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // Validate timestamp (within 30 minutes)
    let timestamp_num: u64 = timestamp.parse().map_err(|_| StatusCode::UNAUTHORIZED)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let tolerance = 30 * 60 * 1000; // 30 minutes in milliseconds
    
    if now - (timestamp_num * 1000) > tolerance {
        tracing::info!("❌ Request timestamp expired");
        return Err(StatusCode::FORBIDDEN);
    }

    // Get request body for HMAC validation
    let (parts, body) = request.into_parts();
    let body_bytes = to_bytes(body, 1024 * 1024).await  // 1MB limit
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    
    // Construct the message to verify (timestamp.request_body)
    let message = format!("{}.{}", timestamp, String::from_utf8_lossy(&body_bytes));

    // Calculate HMAC
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    mac.update(message.as_bytes());
    let result = mac.finalize();
    let calculated_hash = hex::encode(result.into_bytes());
    let expected_hash = format!("v0={}", calculated_hash);

    // Verify signature
    if signature != &expected_hash {
        tracing::info!("❌ HMAC signature validation failed");
        return Err(StatusCode::UNAUTHORIZED);
    }

    tracing::info!("✅ HMAC validation successful");
    
    // Reconstruct request and pass to next handler
    let request = Request::from_parts(parts, Body::from(body_bytes));
    Ok(next.run(request).await)
}



pub async fn elevenlabs_webhook(
    State(state): State<Arc<AppState>>,
    request: axum::extract::Json<serde_json::Value>,
) -> Result<Json<Value>, (StatusCode, Json<serde_json::Value>)> {
    // Log the raw payload first
    tracing::info!("Received raw webhook payload: {}", request.0);

    // First, extract the webhook type to determine how to handle it
    let webhook_type = request.0.get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("unknown");

    tracing::info!("Webhook type: {}", webhook_type);

    match webhook_type {
        "post_call_transcription" => {
            // Call was answered - handle normally and charge for call
            handle_post_call_transcription(state, request.0).await
        },
        "call_initiation_failure" => {
            // Call was NOT answered (busy, no-answer, declined)
            // Don't charge for call, just mark usage as failed
            handle_call_initiation_failure(state, request.0).await
        },
        _ => {
            tracing::warn!("Unknown webhook type: {}", webhook_type);
            // Return OK to prevent ElevenLabs from retrying
            Ok(Json(json!({
                "status": "received",
                "message": "Unknown webhook type, ignored"
            })))
        }
    }
}

/// Handle post_call_transcription webhook - call was answered
async fn handle_post_call_transcription(
    state: Arc<AppState>,
    payload_value: serde_json::Value,
) -> Result<Json<Value>, (StatusCode, Json<serde_json::Value>)> {
    // Parse the payload
    let payload: WebhookPayload = match serde_json::from_value(payload_value) {
        Ok(payload) => payload,
        Err(e) => {
            tracing::error!("Failed to parse post_call_transcription payload: {}", e);
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "error": "Invalid payload format",
                    "details": e.to_string()
                }))
            ));
        }
    };

    let conversation_id = payload.data.conversation_id;
    let call_duration_secs = payload.data.metadata.call_duration_secs;
    let call_successful = payload.data.analysis.call_successful;
    let call_summary = payload.data.analysis.transcript_summary;
    let user_id: Option<String> = payload.data.conversation_initiation_client_data.dynamic_variables.user_id;

    // Get user_id from dynamic variables
    let user_id_str = match user_id {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing user_id in dynamic variables"
                }))
            ));
        }
    };

    // Convert String to i32
    let user_id: i32 = match user_id_str.parse() {
        Ok(id) => id,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Invalid user_id format, must be an integer"
                }))
            ));
        }
    };

    // Fetch user from user_repository
    let user = match state.user_core.find_by_id(user_id) {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "User not found"
                }))
            ));
        }
        Err(e) => {
            error!("Error fetching user: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to fetch user"
                }))
            ));
        }
    };

    match state.user_repository.get_ongoing_usage(user_id) {
        Ok(Some(usage)) => {
            // Handle the ongoing usage log - CHARGE for call (it was answered)
            if let Err(e) = crate::utils::usage::deduct_user_credits(&state, user.id, "voice", Some(call_duration_secs)) {
                eprintln!("Failed to deduct user credits: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "error": "Failed to deduct user credits"
                    }))
                ));
            }

            let success = match call_successful.as_str() {
                "success" => true,
                "failure" => false,
                _ => false,
            };

            // Update the usage log with final values
            if let Err(e) = state.user_repository.update_usage_log_fields(
                user_id,
                &usage.sid.unwrap_or_default(),
                "done",
                success,
                &call_summary,
                Some(call_duration_secs),
            ) {
                error!("Failed to update usage log: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "error": "Failed to update usage log"
                    }))
                ));
            }

            let now_dt = chrono::Utc::now();
            let dur_secs = call_duration_secs as u64;
            let start_dt = now_dt - std::time::Duration::from_secs(dur_secs);

            let start_rfc3339 = start_dt.to_rfc3339();
            let start_epoch = start_dt.timestamp() as i32;
            let end_epoch = now_dt.timestamp() as i32;

            let call_start = crate::models::user_models::NewMessageHistory {
                user_id,
                conversation_id: conversation_id.clone(),
                role: "system".into(),
                encrypted_content: format!("[CALL_START] {}", start_rfc3339),
                tool_name: None,
                tool_call_id: None,
                tool_calls_json: None,
                created_at: start_epoch,
            };

            let call_end = crate::models::user_models::NewMessageHistory {
                user_id,
                conversation_id: conversation_id.clone(),
                role: "system".into(),
                encrypted_content: format!("[CALL_SUMMARY] {}", call_summary),
                tool_name: None,
                tool_call_id: None,
                tool_calls_json: None,
                created_at: end_epoch,
            };

            if let Err(e) = state.user_repository.create_message_history(&call_start) {
                error!("Failed to create message history: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "error": "Failed to create message history"
                    }))
                ));
            }

            if let Err(e) = state.user_repository.create_message_history(&call_end) {
                error!("Failed to create message history: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "error": "Failed to create message history"
                    }))
                ));
            }
        },
        Ok(None) => {
            error!("No ongoing usage found for user {}", user_id);
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "No ongoing usage found"
                }))
            ));
        },
        Err(e) => {
            error!("Failed to get ongoing usage: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to get ongoing usage"
                }))
            ));
        }
    };

    Ok(Json(json!({
        "status": "received"
    })))
}

/// Handle call_initiation_failure webhook - call was NOT answered (busy, no-answer, declined)
/// For "Call + SMS" notifications, we don't charge for the call portion
async fn handle_call_initiation_failure(
    state: Arc<AppState>,
    payload_value: serde_json::Value,
) -> Result<Json<Value>, (StatusCode, Json<serde_json::Value>)> {
    // Parse the payload
    let payload: CallInitiationFailurePayload = match serde_json::from_value(payload_value) {
        Ok(payload) => payload,
        Err(e) => {
            tracing::error!("Failed to parse call_initiation_failure payload: {}", e);
            // Return OK anyway to prevent retries - we'll log the error
            return Ok(Json(json!({
                "status": "received",
                "warning": "Failed to parse failure payload"
            })));
        }
    };

    let failure_reason = &payload.data.failure_reason;
    tracing::info!("Call initiation failed with reason: {}", failure_reason);

    // Try to get user_id from dynamic variables
    let user_id_opt = payload.data.conversation_initiation_client_data
        .as_ref()
        .and_then(|cicd| cicd.dynamic_variables.user_id.clone());

    let user_id_str = match user_id_opt {
        Some(id) => id,
        None => {
            tracing::warn!("No user_id in call_initiation_failure webhook, cannot update usage");
            return Ok(Json(json!({
                "status": "received",
                "message": "Call initiation failure logged (no user_id)"
            })));
        }
    };

    let user_id: i32 = match user_id_str.parse() {
        Ok(id) => id,
        Err(_) => {
            tracing::warn!("Invalid user_id format in call_initiation_failure webhook");
            return Ok(Json(json!({
                "status": "received",
                "message": "Call initiation failure logged (invalid user_id)"
            })));
        }
    };

    // Update usage log to mark call as not answered (no charge for call)
    match state.user_repository.get_ongoing_usage(user_id) {
        Ok(Some(usage)) => {
            // DON'T deduct credits for the call - it wasn't answered
            // Just mark the usage log as completed with failure
            if let Err(e) = state.user_repository.update_usage_log_fields(
                user_id,
                &usage.sid.unwrap_or_default(),
                "not_answered",  // Special status for unanswered calls
                false,           // success = false
                &format!("Call not answered: {}", failure_reason),
                Some(0),         // 0 duration since call wasn't connected
            ) {
                error!("Failed to update usage log for unanswered call: {}", e);
            }

            tracing::info!(
                "Call initiation failure for user {}: {} - NOT charging for call",
                user_id,
                failure_reason
            );
        },
        Ok(None) => {
            tracing::warn!("No ongoing usage found for user {} in call_initiation_failure", user_id);
        },
        Err(e) => {
            error!("Failed to get ongoing usage for call_initiation_failure: {}", e);
        }
    };

    Ok(Json(json!({
        "status": "received",
        "message": format!("Call initiation failure handled: {}", failure_reason)
    })))
}

