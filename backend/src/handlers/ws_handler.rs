use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    http::StatusCode,
    response::IntoResponse,
};
use futures::{SinkExt, StreamExt};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::handlers::auth_dtos::Claims;
use crate::handlers::auth_middleware::AuthUser;
use crate::AppState;
use crate::UserCoreOps;

#[derive(Deserialize, Default)]
pub struct WsQueryParams {
    pub token: Option<String>,
}

/// WebSocket upgrade handler.
/// Auth via cookie-based JWT (AuthUser extractor) or `?token=` query param for mobile clients.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
    Query(params): Query<WsQueryParams>,
    auth_user: Option<AuthUser>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Try cookie/Bearer auth first (AuthUser extractor), then query param token
    let user_id = if let Some(auth) = auth_user {
        auth.user_id
    } else if let Some(token) = params.token {
        validate_ws_token(&token, &state)?
    } else {
        return Err((
            StatusCode::UNAUTHORIZED,
            "No authorization token provided".to_string(),
        ));
    };

    tracing::info!("WebSocket upgrade requested for user {}", user_id);
    Ok(ws.on_upgrade(move |socket| handle_ws(socket, state, user_id)))
}

fn validate_ws_token(token: &str, state: &Arc<AppState>) -> Result<i32, (StatusCode, String)> {
    let claims = decode::<Claims>(
        token,
        &DecodingKey::from_secret(
            std::env::var("JWT_SECRET_KEY")
                .expect("JWT_SECRET_KEY must be set")
                .as_bytes(),
        ),
        &Validation::new(Algorithm::HS256),
    )
    .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid token".to_string()))?
    .claims;

    // Verify user exists
    state
        .user_core
        .find_by_id(claims.sub)
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Database error".to_string(),
            )
        })?
        .ok_or((StatusCode::UNAUTHORIZED, "User not found".to_string()))?;

    Ok(claims.sub)
}

async fn handle_ws(socket: WebSocket, state: Arc<AppState>, user_id: i32) {
    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Subscribe to notification broadcast for this user
    let mut notification_rx = {
        let entry = state
            .ws_notification_senders
            .entry(user_id)
            .or_insert_with(|| broadcast::channel(64).0);
        entry.subscribe()
    };

    tracing::info!("WebSocket connected for user {}", user_id);

    // Task 1: Forward broadcast notifications to this WebSocket client
    let mut send_task = tokio::spawn(async move {
        loop {
            match notification_rx.recv().await {
                Ok(msg) => {
                    if ws_sender.send(Message::Text(msg.into())).await.is_err() {
                        break; // Client disconnected
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("WebSocket for user {} lagged by {} messages", user_id, n);
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    // Task 2: Handle incoming messages from the client
    let state_clone = state.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            match msg {
                Message::Text(text) => {
                    handle_client_message(&state_clone, user_id, &text).await;
                }
                Message::Close(_) => break,
                _ => {} // Ping/pong handled by axum automatically
            }
        }
    });

    // Wait for either task to finish (= disconnect)
    tokio::select! {
        _ = &mut send_task => { recv_task.abort(); }
        _ = &mut recv_task => { send_task.abort(); }
    }

    tracing::info!("WebSocket disconnected for user {}", user_id);
}

async fn handle_client_message(state: &Arc<AppState>, user_id: i32, text: &str) {
    let parsed: serde_json::Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => {
            send_to_user(
                state,
                user_id,
                &json!({"type": "chat_error", "error": "Invalid JSON"}),
            );
            return;
        }
    };

    match parsed["type"].as_str() {
        Some("ping") => {
            send_to_user(state, user_id, &json!({"type": "pong"}));
        }
        Some("chat") => {
            let message = parsed["message"].as_str().unwrap_or("").to_string();
            if message.trim().is_empty() {
                send_to_user(
                    state,
                    user_id,
                    &json!({"type": "chat_error", "error": "Empty message"}),
                );
                return;
            }

            match crate::handlers::profile_handlers::process_web_chat(state, user_id, message).await
            {
                Ok(response) => {
                    send_to_user(
                        state,
                        user_id,
                        &json!({
                            "type": "chat_response",
                            "message": response.message,
                            "credits_charged": response.credits_charged,
                            "media": response.media,
                            "created_task_id": response.created_task_id,
                        }),
                    );
                }
                Err(error_msg) => {
                    send_to_user(
                        state,
                        user_id,
                        &json!({"type": "chat_error", "error": error_msg}),
                    );
                }
            }
        }
        _ => {
            send_to_user(
                state,
                user_id,
                &json!({"type": "chat_error", "error": "Unknown message type"}),
            );
        }
    }
}

/// Send a JSON message to all connected WebSocket clients for a user.
/// Best-effort: silently ignores errors (no receivers = user offline).
pub fn send_to_user(state: &Arc<AppState>, user_id: i32, msg: &serde_json::Value) {
    if let Some(sender) = state.ws_notification_senders.get(&user_id) {
        let _ = sender.send(msg.to_string());
    }
}
