use std::sync::Arc;
use axum::{
    Json,
    extract::State,
    http::StatusCode,
};
use serde_json::json;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct BroadcastMessageRequest {
    message: String,
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
        std::env::var("NLD_PHONE").expect("NLD_PHONE must be set in environment"),
        std::env::var("CHZ_PHONE").expect("CHZ_PHONE must be set in environment"),
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


