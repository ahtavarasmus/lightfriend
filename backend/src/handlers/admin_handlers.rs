use std::sync::Arc;
use axum::{
    Json,
    extract::State,
    http::StatusCode,
};
use serde_json::json;
use serde::{Deserialize, Serialize};
use resend_rs::{Resend, Result as ResendResult};
use resend_rs::types::CreateEmailBaseOptions;

#[derive(Deserialize)]
pub struct TestSmsRequest {
    pub message: String,
    pub user_id: i32,
}

#[derive(Deserialize)]
pub struct BroadcastMessageRequest {
    pub message: String,
}

#[derive(Deserialize)]
pub struct EmailBroadcastRequest {
    pub subject: String,
    pub message: String,
}

#[derive(Serialize)]
pub struct UsageLogResponse {
    id: i32,
    sid: Option<String>,
    activity_type: String,
    credits: Option<f32>,
    timestamp: i32,
    time_consumed: Option<i32>,
    success: Option<bool>,
    reason: Option<String>,
    status: Option<String>,
    recharge_threshold_timestamp: Option<i32>,
    zero_credits_timestamp: Option<i32>,
}

use crate::AppState;


pub async fn verify_user(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(user_id): axum::extract::Path<i32>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {

    // Verify the user
    state.user_repository.verify_user(user_id).map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": format!("Database error: {}", e)}))
    ))?;

    Ok(Json(json!({
        "message": "User verified successfully"
    })))
}



pub async fn update_preferred_number_admin(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(user_id): axum::extract::Path<i32>,
    Json(preferred_number): Json<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {

    // Get allowed numbers from environment
    let allowed_numbers = vec![
        std::env::var("USA_PHONE").expect("USA_PHONE must be set in environment"),
        std::env::var("FIN_PHONE").expect("FIN_PHONE must be set in environment"),
        std::env::var("AUS_PHONE").expect("AUS_PHONE must be set in environment"),
        std::env::var("GB_PHONE").expect("GB_PHONE must be set in environment"),
    ];

    // Validate that the preferred number is in the allowed list
    if !allowed_numbers.contains(&preferred_number) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid preferred number"}))
        ));
    }

    // Update the user's preferred number
    state.user_repository.update_preferred_number(user_id, &preferred_number).map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": format!("Database error: {}", e)}))
    ))?;

    Ok(Json(json!({
        "message": "Preferred number updated successfully"
    })))
}


pub async fn broadcast_email(
    State(state): State<Arc<AppState>>,
    Json(request): Json<EmailBroadcastRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let users = state.user_repository.get_all_users().map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": format!("Database error: {}", e)}))
    ))?;

    let resend_api_key = std::env::var("RESEND_EMAIL_API_KEY")
        .map_err(|_| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "RESEND_EMAIL_API_KEY not set"}))
        ))?;

    let resend = Resend::new(&resend_api_key);
    let from = "info@updates.lightfriend.ai";
    let reply_to = "rasmus@ahtava.com";

    let mut success_count = 0;
    let mut failed_count = 0;

    for user in users {
        if !user.notify {
            continue;
        }

        // Skip users with invalid or empty email addresses
        if user.email.is_empty() || !user.email.contains('@') || !user.email.contains('.') {
            tracing::warn!("Skipping invalid email address: {}", user.email);
            continue;
        }

        let to = vec![user.email.as_str()];
        let email = CreateEmailBaseOptions::new(from, to, &request.subject)
            .with_text(&format!(
                "{}",
                request.message
            ))
            .with_reply(reply_to); 

        match resend.emails.send(email).await {
            Ok(_) => {
                success_count += 1;
                tracing::info!("Successfully sent email to {}", user.email);
            }
            Err(e) => {
                failed_count += 1;
                tracing::error!("Failed to send email to {}: {}", user.email, e);
            }
        }

        // Add a small delay to avoid hitting rate limits
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    Ok(Json(json!({
        "message": "Email broadcast completed",
        "stats": {
            "success": success_count,
            "failed": failed_count
        }
    })))
}

pub async fn broadcast_message(
    State(state): State<Arc<AppState>>,
    Json(request): Json<BroadcastMessageRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {

    let users = state.user_repository.get_all_users().map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": format!("Database error: {}", e)}))
    ))?;

    tokio::spawn(async move {
        process_broadcast_messages(state.clone(), users, request.message.clone()).await;
    });
    // Immediately return a success response
    Ok(Json(json!({
        "message": "Broadcast request received, processing in progress",
        "status": "ok"
    })))

}


#[derive(Debug)]
enum BroadcastError {
    ConversationError(String),
    MessageSendError(String),
}

impl std::fmt::Display for BroadcastError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BroadcastError::ConversationError(msg) => write!(f, "Conversation error: {}", msg),
            BroadcastError::MessageSendError(msg) => write!(f, "Message send error: {}", msg),
        }
    }
}

impl std::error::Error for BroadcastError {}

async fn process_broadcast_messages(
    state: Arc<AppState>,
    users: Vec<crate::models::user_models::User>,
    message: String,
) {
    let mut success_count = 0;
    let mut failed_count = 0;

    for user in users {
        let sender_number = match user.preferred_number.clone() {
            Some(number) => number,
            None => {
                eprintln!("No preferred number for user: {}", user.phone_number);
                failed_count += 1;
                continue;
            }
        };
        if !user.notify {
            continue;
        }

        // Skip users who have a valid email address
        if !user.email.is_empty() && user.email.contains('@') && user.email.contains('.') {
            tracing::info!("Skipping SMS for user with valid email: {}", user.email);
            continue;
        }

        let conversation_result = state
            .user_conversations
            .get_conversation(&user, sender_number.to_string())
            .await
            .map_err(|e| BroadcastError::ConversationError(e.to_string()));

        match conversation_result {
            Ok(conversation) => {
                let message_with_stop = format!("{}\n\nTo stop receiving updates about new features, reply \"STOP\".", message);
                match crate::api::twilio_utils::send_conversation_message(
                    &conversation.conversation_sid,
                    &sender_number,
                    &message_with_stop,
                    false,
                )
                .await
                .map_err(|e| BroadcastError::MessageSendError(e.to_string()))
                {
                    Ok(_) => {
                        success_count += 1;
                        println!("Successfully sent message to {}", user.phone_number);
                    }
                    Err(e) => {
                        eprintln!("Failed to send message to {}: {}", user.phone_number, e);
                        failed_count += 1;
                    }
                }
            }
            Err(e) => {

                eprintln!(
                    "Failed to get/create conversation for {}: {}",
                    user.phone_number,
                    e
                );
                failed_count += 1;
            }
        }
    }

    println!(
        "Broadcast completed: {} successful, {} failed",
        success_count, failed_count
    );
}



pub async fn update_user_messages(
    State(state): State<Arc<AppState>>,
    axum::extract::Path((user_id, amount)): axum::extract::Path<(i32, i32)>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Get current user
    let user = state.user_repository.find_by_id(user_id)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)}))
        ))?
        .ok_or_else(|| (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found"}))
        ))?;

    // Calculate new messages count, ensuring it doesn't go below 0
    let new_msgs = (user.msgs_left as i32 + amount).max(0);

    // Update messages count
    state.user_repository.update_proactive_messages_left(user_id, new_msgs)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to update messages: {}", e)}))
        ))?;

    println!("successfully updated messages for user");
    Ok(Json(json!({
        "message": "Messages updated successfully",
        "new_count": new_msgs
    })))
}

pub async fn update_subscription_tier(
    State(state): State<Arc<AppState>>,
    axum::extract::Path((user_id, tier)): axum::extract::Path<(i32, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let tier = if tier == "tier 0" { None } else { Some(tier.as_str()) };
    
    // Update the subscription tier
    state.user_repository.set_subscription_tier(user_id, tier).map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": format!("Database error: {}", e)}))
    ))?;
    tracing::info!("subscription tier set successfully");

    Ok(Json(json!({
        "message": "Subscription tier updated successfully"
    })))
}

pub async fn get_usage_logs(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<UsageLogResponse>>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("getting usage logs");
    // Get all usage logs from the database
    let logs = state.user_repository.get_all_usage_logs()
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)}))
        ))?;

    // Transform the logs into the response format
    let response_logs: Vec<UsageLogResponse> = logs.into_iter()
        .map(|log| {

            UsageLogResponse {
                id: log.id.unwrap_or(0),

                sid: log.sid,
                activity_type: log.activity_type,
                credits: log.credits,
                timestamp: log.created_at,
                time_consumed: log.time_consumed,
                success: log.success,
                reason: log.reason,
                status: log.status,
                recharge_threshold_timestamp: log.recharge_threshold_timestamp,
                zero_credits_timestamp: log.zero_credits_timestamp,
            }
        })
        .collect();

    tracing::info!("returning response_logs");
    Ok(Json(response_logs))
}

pub async fn test_sms(
    State(state): State<Arc<AppState>>,
    Json(request): Json<TestSmsRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Get the user for the test
    let user = state.user_repository.find_by_id(request.user_id)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)}))
        ))?
        .ok_or_else(|| (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found"}))
        ))?;

    // Create a mock Twilio payload
    let mock_payload = crate::api::twilio_sms::TwilioWebhookPayload {
        from: user.phone_number.clone(),
        to: user.preferred_number.unwrap_or_else(|| "+0987654321".to_string()),
        body: request.message,
    };

    // Process the SMS using the existing handler with test mode
    let (status, _, response) = crate::api::twilio_sms::process_sms(
        &state,
        mock_payload,
        true, // Set test mode to true
    ).await;

    if status == StatusCode::OK {
        Ok(Json(json!({
            "message": response.message,
            "status": "success"
        })))
    } else {
        Err((
            status,
            Json(json!({
                "error": "Failed to process test message",
                "details": response.message
            }))
        ))
    }
}

pub async fn set_preferred_number_default(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(user_id): axum::extract::Path<i32>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    

    // Get the user's phone number
    let user = state.user_repository.find_by_id(user_id)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)}))
        ))?
        .ok_or_else(|| (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found"}))
        ))?;

    // Set the preferred number
    match state.user_repository.set_preferred_number_to_default(user_id, &user.phone_number) {
        Ok(preferred_number) => Ok(Json(json!({
            "message": "Preferred number set successfully",
            "preferred_number": preferred_number
        }))),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to set preferred number: {}", e)}))
        )),
    }
}


