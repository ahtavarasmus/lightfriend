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
use std::collections::HashMap;



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


use std::error::Error;
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
    
    // Try to parse the payload
    let payload: WebhookPayload = match serde_json::from_value(request.0) {
        Ok(payload) => payload,
        Err(e) => {
            tracing::error!("Failed to parse webhook payload: {}", e);
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "error": "Invalid payload format",
                    "details": e.to_string()
                }))
            ));
        }
    };

    tracing::info!("Successfully parsed webhook payload: {:?}", payload);
    println!("Type: {}", payload.type_field);
    let conversation_id = payload.data.conversation_id;
    println!("Conversation ID: {}", conversation_id);
    let call_status = payload.data.status;
    println!("Status: {}", call_status);
    let call_duration_secs = payload.data.metadata.call_duration_secs;
    println!("Call Duration (secs): {}", call_duration_secs);
    let call_successful = payload.data.analysis.call_successful;
    println!("Call Successful: {}", call_successful);
    let call_summary = payload.data.analysis.transcript_summary;
    println!("Transcript Summary: {}", call_summary);
    let user_id: Option<String> = payload.data.conversation_initiation_client_data.dynamic_variables.user_id;
    println!("User ID: {:?}", user_id);
    // Your webhook processing logic here

    // Get user_id from query params
    let user_id_str = match user_id {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing user_id query parameter"
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
    let user = match state.user_repository.find_by_id(user_id) {
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
            // Handle the ongoing usage log
            let voice_second_cost = std::env::var("VOICE_SECOND_COST")
                .expect("VOICE_SECOND_COST not set")
                .parse::<f32>()
                .unwrap_or(0.0033);
            let credits_used = call_duration_secs as f32 * voice_second_cost;
            let success = match call_successful.as_str() {
                "success" => true,
                "failure" => false,
                _ => false,
            };

            // Update the usage log with final values
            if let Err(e) = state.user_repository.update_usage_log_fields(
                user_id,
                &usage.conversation_id.unwrap_or_default(),
                "done",
                credits_used,
                success,
                &call_summary
            ) {
                error!("Failed to update usage log: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "error": "Failed to update usage log"
                    }))
                ));
            }

            // Decrease user's credits
            if let Err(e) = state.user_repository.decrease_credits(user_id, credits_used) {
                error!("Failed to decrease credits: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "error": "Failed to decrease credits"
                    }))
                ));
            }
            println!("decreased user's credits by {}", credits_used);

            match state.user_repository.is_credits_under_threshold(user_id) {
                Ok(is_under) => {
                    if is_under {
                        tracing::info!("User {} credits is under threshold, attempting automatic charge", user_id);
                        if user.charge_when_under {
                            use axum::extract::{State, Path};
                            let state_clone = Arc::clone(&state);
                            tokio::spawn(async move {
                                let _ = crate::handlers::stripe_handlers::automatic_charge(
                                    State(state_clone),
                                    Path(user.id),
                                ).await;
                                tracing::info!("Recharged the user successfully back up!");
                            });                                                                            
                            tracing::info!("recharged the user successfully back up!");
                        }
                    }
                },
                Err(e) => error!("Failed to check if user credits is under threshold: {}", e),
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

