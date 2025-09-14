use std::sync::Arc;
use axum::{
    Json,
    extract::{State, Multipart},
    http::StatusCode,
};
use serde_json::json;
use serde::{Deserialize, Serialize};
use resend_rs::Resend;
use resend_rs::types::CreateEmailBaseOptions;

use std::path::Path;
use tokio::fs;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct TestSmsRequest {
    pub message: String,
    pub user_id: i32,
}

#[derive(Serialize)]
pub struct TestSmsWithImageResponse {
    message: String,
    image_path: String,
}

#[derive(Deserialize)]
pub struct BroadcastMessageRequest {
    pub message: String,
}

#[derive(Deserialize, Clone)]
pub struct EmailBroadcastRequest {
    pub subject: String,
    pub message: String,
}

#[derive(Serialize)]
pub struct UsageLogResponse {
    id: i32,
    user_id: i32,
    activity_type: String,
    timestamp: i32,
    sid: Option<String>,
    status: Option<String>,
    success: Option<bool>,
    credits: Option<f32>,
    time_consumed: Option<i32>,
    reason: Option<String>,
    recharge_threshold_timestamp: Option<i32>,
    zero_credits_timestamp: Option<i32>,
}

use crate::AppState;


pub async fn verify_user(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(user_id): axum::extract::Path<i32>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {

    // Verify the user
    state.user_core.verify_user(user_id).map_err(|e| (
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
    state.user_core.update_preferred_number(user_id, &preferred_number).map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": format!("Database error: {}", e)}))
    ))?;

    Ok(Json(json!({
        "message": "Preferred number updated successfully"
    })))
}

#[derive(Debug, Deserialize)]
pub struct UnsubscribeParams {
    pub email: String,
}

use axum::extract::Query;
use axum::response::Html;

pub async fn unsubscribe(
    State(state): State<Arc<AppState>>,
    Query(params): Query<UnsubscribeParams>,
) -> Result<Html<String>, (StatusCode, String)> {
    tracing::info!("Unsubscribe request received for raw email param: {}", params.email);

    match state.user_core.find_by_email(&params.email) {
        Ok(Some(user)) => {
            tracing::info!("Found user {} for email: {}", user.id, params.email);
            match state.user_core.update_notify(user.id, false) {
                Ok(_) => {
                    tracing::info!("User {} unsubscribed from notifications", user.id);
                    Ok(Html("<h1>You have been unsubscribed!</h1>".to_string()))
                }
                Err(e) => {
                    tracing::error!("Failed to update notify for user {}: {}", user.id, e);
                    Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to unsubscribe. Sorry about this, send email to rasmus@ahtava.com".to_string(),
                    ))
                }
            }
        }
        Ok(None) => {
            tracing::warn!("No user found for email: {}", params.email);
            Err((
                StatusCode::BAD_REQUEST,
                "Invalid email.".to_string(),
            ))
        }
        Err(e) => {
            tracing::error!("Failed to find user by email {}: {}", params.email, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to process request.".to_string(),
            ))
        }
    }
}

fn wrap_text(text: &str, max_width: usize) -> String {
    let mut result = String::new();
    for line in text.lines() {
        if line.is_empty() {
            result.push('\n');
            continue;
        }
        let mut current_line = String::new();
        for word in line.split_whitespace() {
            let mut remaining_word = word.to_string();
            loop {
                let add_space = !current_line.is_empty();
                let space_len = if add_space { 1 } else { 0 };
                if current_line.len() + remaining_word.len() + space_len <= max_width {
                    if add_space {
                        current_line.push(' ');
                    }
                    current_line.push_str(&remaining_word);
                    break;
                } else {
                    if current_line.is_empty() {
                        // Break the long word
                        let chunk_len = max_width;
                        let (chunk, rest) = remaining_word.split_at(chunk_len);
                        current_line.push_str(chunk);
                        remaining_word = rest.to_string();
                    }
                    result.push_str(&current_line);
                    result.push('\n');
                    current_line = String::new();
                }
            }
        }
        if !current_line.is_empty() {
            result.push_str(&current_line);
        }
        result.push('\n');
    }
    // Remove any trailing newline if the original didn't end with one
    if !text.ends_with('\n') && result.ends_with('\n') {
        result.pop();
    }
    result
}

pub async fn broadcast_email(
    State(state): State<Arc<AppState>>,
    Json(request): Json<EmailBroadcastRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Validate input
    if request.subject.is_empty() || request.message.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Subject and message cannot be empty"}))
        ));
    }

    // Fetch users outside the spawn to avoid DB issues, then move into task
    let users = state.user_core.get_all_users().map_err(|e| {
        tracing::error!("Database error when fetching users: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)}))
        )
    })?;

    // Clone what we need for the task
    let state_clone = state.clone();
    let request_clone = request.clone();

    // Spawn the background task
    tokio::spawn(async move {
        let auth_user = crate::handlers::auth_middleware::AuthUser { user_id: 1, is_admin: false }; // Hardcode user_id to 1

        let mut success_count = 0;
        let mut failed_count = 0;
        let mut error_details = Vec::new();

        for user in users {
            let user_settings = match state_clone.user_core.get_user_settings(user.id) {
                Ok(settings) => settings,
                Err(e) => {
                    tracing::error!("Failed to get settings for user {}: {}", user.id, e);
                    failed_count += 1;
                    error_details.push(format!("Failed to get settings for {}: {}", user.email, e));
                    continue;
                }
            };

            if !user_settings.notify {
                tracing::info!("skipping user since they don't have notify on");
                continue;
            }

            // Skip users with invalid or empty email addresses
            if user.email.is_empty() || !user.email.contains('@') || !user.email.contains('.') {
                tracing::warn!("Skipping invalid email address: {}", user.email);
                continue;
            }

            // Prepare the unsubscribe link
            let encoded_email = urlencoding::encode(&user.email);
            let server_url = std::env::var("SERVER_URL").expect("SERVER_URL not set");
            let unsubscribe_link = format!("{}/api/unsubscribe?email={}", server_url, encoded_email);

            // Prepare plain text body with unsubscribe (link inline now)
            let plain_body = format!(
                "{}\n\nTo unsubscribe from these feature updates/fixes, click here: {}",
                request_clone.message, unsubscribe_link
            );
            let wrapped_body = wrap_text(&plain_body, 72);
            // Convert to CRLF line endings for email compliance
            let crlf_body = wrapped_body.replace("\n", "\r\n");

            // Prepare the email request for the send_email handler
            let email_request = crate::handlers::imap_handlers::SendEmailRequest {
                to: user.email.clone(),
                subject: request_clone.subject.clone(),
                body: crlf_body,
            };

            // Call the existing send_email handler
            match crate::handlers::imap_handlers::send_email(
                State(state_clone.clone()),
                auth_user.clone(),
                Json(email_request)
            ).await {
                Ok(_) => {
                    success_count += 1;
                    tracing::info!("Successfully sent email to {}", user.email);
                }
                Err((status, err)) => {
                    failed_count += 1;
                    let error_msg = format!("Failed to send to {}: {:?}", user.email, err);
                    tracing::error!("{}", error_msg);
                    error_details.push(error_msg);
                }
            }

            // Add a small delay to avoid hitting rate limits
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        // Log final stats since we can't return them
        tracing::info!(
            "Email broadcast completed: success={}, failed={}, errors={:?}",
            success_count,
            failed_count,
            error_details
        );
    });

    // Respond immediately
    Ok(Json(json!({
        "message": "Email broadcast queued and will process in the background"
    })))
}


pub async fn broadcast_message(
    State(state): State<Arc<AppState>>,
    Json(request): Json<BroadcastMessageRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {

    let users = state.user_core.get_all_users().map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": format!("Database error: {}", e)}))
    ))?;

    // Immediately return a success response
    Ok(Json(json!({
        "message": "Broadcast is currently disabled(create it for programmable messaging)",
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
        // Get user settings
        let user_settings = match state.user_core.get_user_settings(user.id) {
            Ok(settings) if !settings.notify => continue,
            Ok(_) => (), // Continue if notify is true or no settings exist
            Err(e) => {
                eprintln!("Failed to get user settings for {}: {}", user.email, e);
                continue;
            }
        };

    }
}



pub async fn update_discount_tier(
    State(state): State<Arc<AppState>>,
    axum::extract::Path((user_id, tier)): axum::extract::Path<(i32, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Convert empty string or "none" to None, otherwise Some(tier)
    let tier = match tier.to_lowercase().as_str() {
        "" | "none" | "null" => None,
        _ if ["msg", "voice", "full"].contains(&tier.as_str()) => Some(tier.as_str()),
        _ => return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid tier. Must be 'msg', 'voice', 'full', or 'none'"}))
        )),
    };

    // Update the discount tier
    state.user_core.update_discount_tier(user_id, tier).map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": format!("Database error: {}", e)}))
    ))?;

    Ok(Json(json!({
        "message": "Discount tier updated successfully",
        "tier": tier
    })))
}

pub async fn update_monthly_credits(
    State(state): State<Arc<AppState>>,
    axum::extract::Path((user_id, amount)): axum::extract::Path<(f32, f32)>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Get current user
    let user = state.user_core.find_by_id(user_id as i32)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)}))
        ))?
        .ok_or_else(|| (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found"}))
        ))?;

    // Calculate new credits count, ensuring it doesn't go below 0
    let new_credits = (user.credits_left + amount).max(0.0);

    // Update credits count
    state.user_repository.update_user_credits_left(user.id, new_credits)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to update monthly credits: {}", e)}))
        ))?;

    Ok(Json(json!({
        "message": "Monthly credits updated successfully",
        "new_count": new_credits
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
                user_id: log.user_id,
                activity_type: log.activity_type,
                timestamp: log.created_at,
                sid: log.sid,
                status: log.status,
                success: log.success,
                credits: log.credits,
                time_consumed: log.time_consumed,
                reason: log.reason,
                recharge_threshold_timestamp: log.recharge_threshold_timestamp,
                zero_credits_timestamp: log.zero_credits_timestamp,
            }
        })
        .collect();

    tracing::info!("returning response_logs");
    Ok(Json(response_logs))
}

pub async fn test_sms_with_image(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<TestSmsWithImageResponse>, (StatusCode, Json<serde_json::Value>)> {
    println!("test_sms_with_image");
    // Ensure uploads directory path is absolute
    // Create uploads directory if it doesn't exist
    let uploads_dir = Path::new("backend/uploads");
    if !uploads_dir.exists() {
        fs::create_dir_all(uploads_dir).await.map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to create uploads directory: {}", e)}))
        ))?;
    }

    let mut message = String::new();
    let mut image_data_url = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| (
        StatusCode::BAD_REQUEST,
        Json(json!({"error": format!("Failed to process form data: {}", e)}))
    ))? {
        let name = field.name().unwrap_or("").to_string();

        match name.as_str() {
            "message" => {
                println!("Processing message field");
                message = field.text().await.map_err(|e| (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": format!("Failed to read message: {}", e)}))
                ))?;
            }
            "image" => {
                let file_name = format!("{}.jpg", Uuid::new_v4());
                println!("Processing image: {}", file_name);
                let path = uploads_dir.join(&file_name);
                
                let data = field.bytes().await.map_err(|e| (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": format!("Failed to read image data: {}", e)}))
                ))?;

                // Save the file
                fs::write(&path, &data).await.map_err(|e| (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to save image: {}", e)}))
                ))?;

                // Convert to base64 data URL
                let base64 = base64::encode(&data);
                let mime_type = "image/jpeg"; // Assuming JPEG format
                let data_url = format!("data:{};base64,{}", mime_type, base64);
                
                // Store both the data URL and save the file path
                let absolute_path = path.canonicalize().map_err(|e| (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to get absolute path: {}", e)}))
                ))?.to_string_lossy().into_owned();

                image_data_url = Some((data_url, absolute_path.clone()));
                println!("Image saved at: {}", absolute_path);
            }
            _ => continue,
        }
    }

    // Create mock Twilio payload with image
    let mock_payload = crate::api::twilio_sms::TwilioWebhookPayload {
        from: "+358442105886".to_string(), // Default test number
        to: std::env::var("SHAZAM_PHONE_NUMBER").expect("SHAZAM_PHONE_NUMBER must be set"),
        body: message.clone(),
        num_media: image_data_url.as_ref().map(|_| "1".to_string()),
        media_url0: Some(image_data_url.as_ref().map(|(data_url, _)| data_url.clone()).unwrap_or_default()),
        media_content_type0: Some("image/jpeg".to_string()),
        message_sid: "".to_string(),
    };
    println!("mock_payload.num_media: {:#?}",mock_payload.num_media);
    // Process the SMS using the existing handler with test mode
    let (status, _, response) = crate::api::twilio_sms::process_sms(
        &state,
        mock_payload,
        true, // Set test mode to true
    ).await;

    if status == StatusCode::OK {
        Ok(Json(TestSmsWithImageResponse {
            message: response.message.clone(),
            image_path: image_data_url.map(|(_, path)| path).unwrap_or_default(),
        }))
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

pub async fn test_sms(
    State(state): State<Arc<AppState>>,
    Json(request): Json<TestSmsRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Get the user for the test
    let user = state.user_core.find_by_id(request.user_id)
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
        num_media: None,
        media_url0: None,
        media_content_type0: None,
        message_sid: "".to_string(),
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


