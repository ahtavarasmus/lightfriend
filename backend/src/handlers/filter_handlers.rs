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
        NewKeyword, NewImportancePriority,
    },
    handlers::auth_middleware::AuthUser,
};

// Request DTOs
#[derive(Deserialize, Serialize)]
pub struct WaitingCheckRequest {
    waiting_type: String,
    content: String,
    due_date: i32,
    remove_when_found: bool,
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
    due_date: i32,
    remove_when_found: bool,
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
        due_date: request.due_date,
        content: request.content,
        remove_when_found: request.remove_when_found,
        service_type: request.service_type,
    };

    match state.user_repository.create_waiting_check(&new_check) {
        Ok(_) => {
            println!("Successfully created waiting check {} for user {}", request.waiting_type, auth_user.user_id);
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
    auth_user: AuthUser,
    Path(service_type): Path<String>,
) -> Result<Json<Vec<WaitingCheckResponse>>, (StatusCode, Json<serde_json::Value>)> {
    println!("Fetching waiting checks for user {} and service {}", auth_user.user_id, service_type);

    let checks = state.user_repository.get_waiting_checks(auth_user.user_id, &service_type)
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
        due_date: check.due_date,
        remove_when_found: check.remove_when_found,
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

pub async fn get_priority_senders(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(service_type): Path<String>,
) -> Result<Json<Vec<PrioritySenderResponse>>, (StatusCode, Json<serde_json::Value>)> {
    println!("Fetching priority senders for user {} and service {}", auth_user.user_id, service_type);

    let senders = state.user_repository.get_priority_senders(auth_user.user_id, &service_type)
        .map_err(|e| {
            tracing::error!("Failed to fetch priority senders for user {}: {}", auth_user.user_id, e);
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

    Ok(Json(response))
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

pub async fn get_keywords(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(service_type): Path<String>,
) -> Result<Json<Vec<String>>, (StatusCode, Json<serde_json::Value>)> {
    println!("Fetching keywords for user {} and service {}", auth_user.user_id, service_type);

    let keywords = state.user_repository.get_keywords(auth_user.user_id, &service_type)
        .map_err(|e| {
            tracing::error!("Failed to fetch keywords for user {}: {}", auth_user.user_id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            )
        })?;

    let keyword_strings: Vec<String> = keywords.into_iter().map(|keyword| keyword.keyword).collect();
    println!("Successfully fetched keywords");

    Ok(Json(keyword_strings))
}

// Importance Priorities handlers
pub async fn create_importance_priority(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<ImportancePriorityRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    println!("Attempting to create importance priority for user {}", auth_user.user_id);

    let new_priority = NewImportancePriority {
        user_id: auth_user.user_id,
        threshold: request.threshold,
        service_type: request.service_type,
    };

    match state.user_repository.create_importance_priority(&new_priority) {
        Ok(_) => {
            println!("Successfully created importance priority for user {}", auth_user.user_id);
            Ok(Json(json!({"message": "Importance priority created successfully"})))
        },
        Err(DieselError::RollbackTransaction) => Err((
            StatusCode::CONFLICT,
            Json(json!({"error": "Importance priority already exists"}))
        )),
        Err(e) => {
            tracing::error!("Failed to create importance priority for user {}: {}", auth_user.user_id, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))
        },
    }
}

pub async fn delete_importance_priority(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(service_type): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    println!("Attempting to delete importance priority {} for user {}", service_type, auth_user.user_id);

    match state.user_repository.delete_importance_priority(auth_user.user_id, &service_type) {
        Ok(_) => {
            println!("Successfully deleted importance priority {} for user {}", service_type, auth_user.user_id);
            Ok(Json(json!({"message": "Importance priority deleted successfully"})))
        },
        Err(DieselError::NotFound) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Importance priority not found"}))
        )),
        Err(e) => {
            tracing::error!("Failed to delete importance priority {}: {}", service_type, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))
        },
    }
}
pub async fn get_connected_services(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<Vec<ConnectedService>>, (StatusCode, Json<serde_json::Value>)> {
    let mut services = Vec::new();

    // Check Google Calendar
    if let Ok(true) = state.user_repository.has_active_google_calendar(auth_user.user_id) {
        services.push(ConnectedService {
            service_type: "calendar".to_string(),
            identifier: "(google)".to_string(),  // Using access token email as identifier
        });
    }

    // Check Email
    if let Ok(Some((email, _, _, _))) = state.user_repository.get_imap_credentials(auth_user.user_id) {
        services.push(ConnectedService {
            service_type: "imap".to_string(),
            identifier: email,
        });
    }

    // Check WhatsApp
    if let Ok(Some(_)) = state.user_repository.get_active_whatsapp_connection(auth_user.user_id) {
        services.push(ConnectedService {
            service_type: "whatsapp".to_string(),
            identifier: "".to_string(),  // Using bridge ID as identifier
        });
    }

    Ok(Json(services))
}

// Generic filter toggle handlers
pub async fn toggle_keywords(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(service_type): Path<String>,
    Json(request): Json<FilterToggleRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    println!("Toggling {} keywords filter for user {} to {}", service_type, auth_user.user_id, request.active);

    let result = match service_type.as_str() {
        "whatsapp" => state.user_repository.update_whatsapp_keywords_active(auth_user.user_id, request.active),
        "imap" => state.user_repository.update_email_keywords_active(auth_user.user_id, request.active),
        _ => return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid service type"}))
        )),
    };

    let keywords = state.user_repository.get_whatsapp_filter_settings(auth_user.user_id).unwrap();
    println!("whatsapp keywords state after toggling: {}", keywords.0);

    let email_keywords = state.user_repository.get_email_filter_settings(auth_user.user_id).unwrap();
    println!("email keywords state after toggling: {}", email_keywords.0);



    match result {
        Ok(_) => {
            println!("Successfully updated {} keywords filter for user {}", service_type, auth_user.user_id);
            Ok(Json(json!({"message": format!("{} keywords filter updated successfully", service_type)})))
        },
        Err(e) => {
            tracing::error!("Failed to update {} keywords filter for user {}: {}", service_type, auth_user.user_id, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))
        },
    }
}

pub async fn toggle_priority_senders(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(service_type): Path<String>,
    Json(request): Json<FilterToggleRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    println!("Toggling {} priority senders filter for user {} to {}", service_type, auth_user.user_id, request.active);

    let result = match service_type.as_str() {
        "whatsapp" => state.user_repository.update_whatsapp_priority_senders_active(auth_user.user_id, request.active),
        "imap" => state.user_repository.update_email_priority_senders_active(auth_user.user_id, request.active),
        _ => return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid service type"}))
        )),
    };

    match result {
        Ok(_) => {
            println!("Successfully updated {} priority senders filter for user {}", service_type, auth_user.user_id);
            Ok(Json(json!({"message": format!("{} priority senders filter updated successfully", service_type)})))
        },
        Err(e) => {
            tracing::error!("Failed to update {} priority senders filter for user {}: {}", service_type, auth_user.user_id, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))
        },
    }
}

pub async fn toggle_waiting_checks(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(service_type): Path<String>,
    Json(request): Json<FilterToggleRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    println!("Toggling {} waiting checks filter for user {} to {}", service_type, auth_user.user_id, request.active);

    let result = match service_type.as_str() {
        "whatsapp" => state.user_repository.update_whatsapp_waiting_checks_active(auth_user.user_id, request.active),
        "imap" => state.user_repository.update_email_waiting_checks_active(auth_user.user_id, request.active),
        _ => return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid service type"}))
        )),
    };

    match result {
        Ok(_) => {
            println!("Successfully updated {} waiting checks filter for user {}", service_type, auth_user.user_id);
            Ok(Json(json!({"message": format!("{} waiting checks filter updated successfully", service_type)})))
        },
        Err(e) => {
            tracing::error!("Failed to update {} waiting checks filter for user {}: {}", service_type, auth_user.user_id, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))
        },
    }
}

pub async fn toggle_general_importance(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(service_type): Path<String>,
    Json(request): Json<FilterToggleRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    println!("Toggling {} general importance filter for user {} to {}", service_type, auth_user.user_id, request.active);

    let result = match service_type.as_str() {
        "whatsapp" => state.user_repository.update_whatsapp_general_importance_active(auth_user.user_id, request.active),
        "imap" => state.user_repository.update_email_general_importance_active(auth_user.user_id, request.active),
        _ => return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid service type"}))
        )),
    };

    match result {
        Ok(_) => {
            println!("Successfully updated {} general importance filter for user {}", service_type, auth_user.user_id);
            Ok(Json(json!({"message": format!("{} general importance filter updated successfully", service_type)})))
        },
        Err(e) => {
            tracing::error!("Failed to update {} general importance filter for user {}: {}", service_type, auth_user.user_id, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))
        },
    }
}


// Get filter keywods active state
pub async fn get_keywords_state(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    println!("Fetching keywords filter state for user {}", auth_user.user_id);

    match state.user_repository.get_whatsapp_filter_settings(auth_user.user_id) {
        Ok(whatsapp_keywords_active) => {
            println!("whatsapp_keywords_active: {:#?}", whatsapp_keywords_active);
            Ok(Json(json!({
                "keywords_active": whatsapp_keywords_active,
            })))
        },
        Err(e) => {
            tracing::error!("Failed to fetch WhatsApp filter settings for user {}: {}", auth_user.user_id, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))
        },
    }
}


// Get filter settings handlers
pub async fn get_whatsapp_filter_settings(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    println!("Fetching WhatsApp filter settings for user {}", auth_user.user_id);

    match state.user_repository.get_whatsapp_filter_settings(auth_user.user_id) {
        Ok((whatsapp_keywords_active, whatsapp_priority_senders_active, whatsapp_waiting_checks_active, whatsapp_general_importance_active)) => {
            println!("whatsapp_keywords_active in get_whatsapp_filter_settings: {}", whatsapp_keywords_active);
            println!("whatsapp_priority_senders_active in get_whatsapp_filter_settings: {}", whatsapp_priority_senders_active);
            println!("whatsapp_waiting_checks_active in get_whatsapp_filter_settings: {}", whatsapp_waiting_checks_active);
            println!("whatsapp_general_importance_active in get_whatsapp_filter_settings: {}", whatsapp_general_importance_active);
            Ok(Json(json!({
                "keywords_active": whatsapp_keywords_active,
                "priority_senders_active": whatsapp_priority_senders_active,
                "waiting_checks_active": whatsapp_waiting_checks_active,
                "general_importance_active": whatsapp_general_importance_active
            })))
        },
        Err(e) => {
            tracing::error!("Failed to fetch WhatsApp filter settings for user {}: {}", auth_user.user_id, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))
        },
    }
}

pub async fn get_email_filter_settings(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    println!("Fetching email filter settings for user {}", auth_user.user_id);

    match state.user_repository.get_email_filter_settings(auth_user.user_id) {
        Ok((email_keywords_active, email_priority_senders_active, email_waiting_checks_active, email_general_importance_active)) => {
            Ok(Json(json!({
                "keywords_active": email_keywords_active,
                "priority_senders_active": email_priority_senders_active,
                "waiting_checks_active": email_waiting_checks_active,
                "general_importance_active": email_general_importance_active
            })))
        },
        Err(e) => {
            tracing::error!("Failed to fetch email filter settings for user {}: {}", auth_user.user_id, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            ))
        },
    }
}

pub async fn get_importance_priority(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(service_type): Path<String>,
) -> Result<Json<ImportancePriorityResponse>, (StatusCode, Json<serde_json::Value>)> {
    println!("Fetching importance priority for user {}", auth_user.user_id);

    let priority = state.user_repository.get_importance_priority(auth_user.user_id, service_type.as_str())
        .map_err(|e| {
            tracing::error!("Failed to fetch importance priority for user {}: {}", auth_user.user_id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)}))
            )
        })?;

    match priority {
        Some(priority) => Ok(Json(ImportancePriorityResponse {
            user_id: priority.user_id,
            threshold: priority.threshold,
            service_type: priority.service_type,
        })),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Importance priority not found"}))
        ))
    }
}
