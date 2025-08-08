use std::sync::Arc;
use axum::{
    extract::{State, Path},
    Json,
    http::StatusCode,
};
use diesel::result::Error as DieselError;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{
    AppState,
    models::user_models::{
        NewWaitingCheck, NewPrioritySender,
        NewKeyword
    },
    handlers::auth_middleware::AuthUser,
};

// Request DTOs
#[derive(Deserialize, Serialize)]
pub struct WaitingCheckRequest {
    content: String,
    service_type: String, // imap, whatsapp, etc.
}

#[derive(Deserialize)]
pub struct PrioritySenderRequest {
    sender: String,
    service_type: String, // imap, whatsapp, etc.
}

#[derive(Deserialize)]
pub struct KeywordRequest {
    keyword: String,
    service_type: String, // imap, whatsapp, etc.
}

#[derive(Deserialize)]
pub struct ImportancePriorityRequest {
    threshold: i32,
    service_type: String, // imap, whatsapp, etc.
}

#[derive(Deserialize)]
pub struct FilterToggleRequest {
    active: bool,
}

// Response DTOs
#[derive(Serialize)]
pub struct ConnectedService {
    service_type: String,
    identifier: String,  // email address or calendar name
}

#[derive(Serialize)]
pub struct WaitingCheckResponse {
    user_id: i32,
    content: String,
    service_type: String,
}

#[derive(Serialize)]
pub struct PrioritySenderResponse {
    user_id: i32,
    sender: String,
    service_type: String,
}

#[derive(Serialize)]
pub struct KeywordResponse {
    user_id: i32,
    keyword: String,
    service_type: String,
}

#[derive(Serialize)]
pub struct ImportancePriorityResponse {
    user_id: i32,
    threshold: i32,
    service_type: String,
}


// Waiting Checks handlers
pub async fn create_waiting_check(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<WaitingCheckRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    println!("Attempting to create waiting check for user {} with type: {}", auth_user.user_id, request.service_type);

    let new_check = NewWaitingCheck {
        user_id: auth_user.user_id,
        content: request.content,
        service_type: request.service_type,
    };

    match state.user_repository.create_waiting_check(&new_check) {
        Ok(_) => {
            println!("Successfully created waiting check for user {}", auth_user.user_id);
            Ok(Json(json!({"message": "Waiting check created successfully"})))
        },
        Err(DieselError::RollbackTransaction) => Err((
            StatusCode::CONFLICT,
            Json(json!({"error": "Waiting check already exists"}))
        )),
        Err(e) => {
            tracing::error!("Failed to create waiting check for user {}: {}", auth_user.user_id, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))
        },
    }
}

pub async fn delete_waiting_check(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path((service_type, content)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    println!("Attempting to delete waiting check {} for user {}", service_type, auth_user.user_id);

    match state.user_repository.delete_waiting_check(auth_user.user_id, &service_type, &content) {
        Ok(_) => {
            println!("Successfully deleted waiting check {} for user {}", service_type, auth_user.user_id);
            Ok(Json(json!({"message": "Waiting check deleted successfully"})))
        },
        Err(DieselError::NotFound) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Waiting check not found"}))
        )),
        Err(e) => {
            tracing::error!("Failed to delete waiting check {}: {}", service_type, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))
        },
    }
}

pub async fn get_waiting_checks(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser
) -> Result<Json<Vec<WaitingCheckResponse>>, (StatusCode, Json<serde_json::Value>)> {
    println!("Fetching waiting checks for user {}", auth_user.user_id);

    let checks = state.user_repository.get_waiting_checks_all(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Failed to fetch waiting checks for user {}: {}", auth_user.user_id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            )
        })?;

    let response: Vec<WaitingCheckResponse> = checks.into_iter().map(|check| WaitingCheckResponse {
        user_id: check.user_id,
        content: check.content,
        service_type: check.service_type,
    }).collect();

    Ok(Json(response))
}

// Priority Senders handlers
pub async fn create_priority_sender(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<PrioritySenderRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    println!("Attempting to create priority sender for user {} with type: {}", auth_user.user_id, request.service_type);

    let new_sender = NewPrioritySender {
        user_id: auth_user.user_id,
        sender: request.sender.clone(),
        service_type: request.service_type,
    };

    match state.user_repository.create_priority_sender(&new_sender) {
        Ok(_) => {
            println!("Successfully created priority sender {} for user {}", request.sender, auth_user.user_id);
            Ok(Json(json!({"message": "Priority sender created successfully"})))
        },
        Err(DieselError::RollbackTransaction) => Err((
            StatusCode::CONFLICT,
            Json(json!({"error": "Priority sender already exists"}))
        )),
        Err(e) => {
            tracing::error!("Failed to create priority sender for user {}: {}", auth_user.user_id, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))
        },
    }
}

pub async fn delete_priority_sender(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path((service_type, sender)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    println!("Attempting to delete priority sender {} for user {}", sender, auth_user.user_id);

    match state.user_repository.delete_priority_sender(auth_user.user_id, &service_type, &sender) {
        Ok(_) => {
            println!("Successfully deleted priority sender {} for user {}", sender, auth_user.user_id);
            Ok(Json(json!({"message": "Priority sender deleted successfully"})))
        },
        Err(DieselError::NotFound) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Priority sender not found"}))
        )),
        Err(e) => {
            tracing::error!("Failed to delete priority sender {}: {}", sender, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))
        },
    }
}

#[derive(Serialize, Deserialize)]
pub struct PriorityNotificationInfo {
    pub average_per_day: f32,
    pub estimated_monthly_price: f32,
}


pub async fn get_priority_senders(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    println!("Fetching priority senders for user {}", auth_user.user_id);
    let senders = state.user_repository.get_priority_senders_all(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Failed to fetch priority senders for user {}: {}", auth_user.user_id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            )
        })?;
    let info = state.user_core.get_priority_notification_info(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Failed to fetch priority info for user {}: {}", auth_user.user_id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            )
        })?;
    let response: Vec<PrioritySenderResponse> = senders.into_iter().map(|sender| PrioritySenderResponse {
        user_id: sender.user_id,
        sender: sender.sender,
        service_type: sender.service_type,
    }).collect();
    let full_response = json!({
        "contacts": response,
        "average_per_day": info.average_per_day,
        "estimated_monthly_price": info.estimated_monthly_price
    });
    Ok(Json(full_response))
}


// Keywords handlers
pub async fn create_keyword(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<KeywordRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    println!("Attempting to create keyword for user {}", auth_user.user_id);

    // First check if the keyword already exists
    let existing_keywords = state.user_repository.get_keywords(auth_user.user_id, &request.service_type)
        .map_err(|e| {
            tracing::error!("Failed to fetch keywords for user {}: {}", auth_user.user_id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            )
        })?;

    // Check if keyword already exists (case-insensitive)
    if existing_keywords.iter().any(|k| k.keyword.to_lowercase() == request.keyword.to_lowercase()) {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({"error": "Keyword already exists"}))
        ));
    }

    let new_keyword = NewKeyword {
        user_id: auth_user.user_id,
        keyword: request.keyword.clone(),
        service_type: request.service_type,
    };

    match state.user_repository.create_keyword(&new_keyword) {
        Ok(_) => {
            println!("Successfully created keyword {} for user {}", request.keyword, auth_user.user_id);
            Ok(Json(json!({"message": "Keyword created successfully"})))
        },

        Err(e) => {
            tracing::error!("Failed to create keyword for user {}: {}", auth_user.user_id, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))
        },
    }
}

pub async fn delete_keyword(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path((service_type, keyword)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    println!("Attempting to delete keyword {} for user {}", keyword, auth_user.user_id);

    match state.user_repository.delete_keyword(auth_user.user_id, &service_type, &keyword) {
        Ok(_) => {
            println!("Successfully deleted keyword {} for user {}", keyword, auth_user.user_id);
            Ok(Json(json!({"message": "Keyword deleted successfully"})))
        },
        Err(DieselError::NotFound) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Keyword not found"}))
        )),
        Err(e) => {
            tracing::error!("Failed to delete keyword {}: {}", keyword, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))
        },
    }
}
