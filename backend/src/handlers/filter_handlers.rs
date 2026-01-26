use crate::UserCoreOps;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use diesel::result::Error as DieselError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

use crate::{
    handlers::auth_middleware::AuthUser,
    models::user_models::{NewKeyword, NewPrioritySender},
    AppState,
};

#[derive(Deserialize)]
pub struct PrioritySenderRequest {
    sender: String,
    service_type: String, // imap, whatsapp, etc.
    noti_type: Option<String>,
    noti_mode: String, // "all", "focus"
}

#[derive(Deserialize)]
pub struct KeywordRequest {
    keyword: String,
    service_type: String, // imap, whatsapp, etc.
}

// Response DTOs
#[derive(Serialize)]
pub struct TaskResponse {
    id: Option<i32>,
    user_id: i32,
    trigger: String,
    condition: Option<String>,
    action: String,
    notification_type: Option<String>,
    status: Option<String>,
    created_at: i32,
    is_permanent: Option<i32>,
    recurrence_rule: Option<String>,
    recurrence_time: Option<String>,
    sources: Option<String>,
    source_lookback_hours: Option<i32>,
}

#[derive(Deserialize)]
pub struct SetPermanenceRequest {
    pub is_permanent: bool,
    pub recurrence_rule: Option<String>, // "daily", "weekly:1,3,5", "monthly:15"
    pub recurrence_time: Option<String>, // "09:00" (HH:MM)
}

#[derive(Deserialize)]
pub struct CreateTaskRequest {
    pub action: String,                     // "generate_digest" or other action
    pub recurrence_rule: Option<String>,    // "daily", "weekly:1,3,5"
    pub recurrence_time: Option<String>,    // "08:00" (HH:MM in user timezone)
    pub sources: Option<String>,            // "email,whatsapp,telegram,signal,calendar"
    pub source_lookback_hours: Option<i32>, // Default 24
    pub notification_type: Option<String>,  // "sms" or "call"
    pub condition: Option<String>,          // Optional condition to check
}

#[derive(Serialize)]
pub struct PrioritySenderResponse {
    user_id: i32,
    sender: String,
    service_type: String,
    noti_type: Option<String>,
    noti_mode: String,
}

pub async fn cancel_task(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(task_id): Path<i32>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::debug!(
        "Attempting to cancel task {} for user {}",
        task_id,
        auth_user.user_id
    );

    match state
        .user_repository
        .cancel_task(auth_user.user_id, task_id)
    {
        Ok(true) => {
            tracing::debug!(
                "Successfully cancelled task {} for user {}",
                task_id,
                auth_user.user_id
            );
            Ok(Json(json!({"message": "Task cancelled successfully"})))
        }
        Ok(false) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Task not found or already completed"})),
        )),
        Err(e) => {
            tracing::error!("Failed to cancel task {}: {}", task_id, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            ))
        }
    }
}

pub async fn set_task_permanence(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(task_id): Path<i32>,
    Json(request): Json<SetPermanenceRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::debug!(
        "Setting permanence for task {} for user {}",
        task_id,
        auth_user.user_id
    );

    // Validate recurrence settings
    if request.is_permanent {
        if let Some(ref rule) = request.recurrence_rule {
            // Validate rule format
            if !rule.starts_with("daily")
                && !rule.starts_with("weekly:")
                && !rule.starts_with("monthly:")
            {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(
                        json!({"error": "Invalid recurrence rule. Use 'daily', 'weekly:1,2,3', or 'monthly:15'"}),
                    ),
                ));
            }
        }
        if let Some(ref time) = request.recurrence_time {
            // Validate time format (HH:MM)
            if time.len() != 5 || !time.contains(':') {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "Invalid time format. Use HH:MM (e.g., '09:00')"})),
                ));
            }
        }
    }

    match state.user_repository.update_task_permanence(
        auth_user.user_id,
        task_id,
        request.is_permanent,
        request.recurrence_rule,
        request.recurrence_time,
    ) {
        Ok(true) => {
            tracing::debug!(
                "Successfully updated permanence for task {} for user {}",
                task_id,
                auth_user.user_id
            );
            Ok(Json(
                json!({"message": "Task permanence updated successfully"}),
            ))
        }
        Ok(false) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Task not found"})),
        )),
        Err(e) => {
            tracing::error!("Failed to update task permanence {}: {}", task_id, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            ))
        }
    }
}

pub async fn get_tasks(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<Vec<TaskResponse>>, (StatusCode, Json<serde_json::Value>)> {
    tracing::debug!("Fetching tasks for user {}", auth_user.user_id);

    let tasks = state
        .user_repository
        .get_user_tasks(auth_user.user_id)
        .map_err(|e| {
            tracing::error!(
                "Failed to fetch tasks for user {}: {}",
                auth_user.user_id,
                e
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?;

    let response: Vec<TaskResponse> = tasks
        .into_iter()
        .map(|task| TaskResponse {
            id: task.id,
            user_id: task.user_id,
            trigger: task.trigger,
            condition: task.condition,
            action: task.action,
            notification_type: task.notification_type,
            status: task.status,
            created_at: task.created_at,
            is_permanent: task.is_permanent,
            recurrence_rule: task.recurrence_rule,
            recurrence_time: task.recurrence_time,
            sources: task.sources,
            source_lookback_hours: task.source_lookback_hours,
        })
        .collect();

    Ok(Json(response))
}

/// Create a new scheduled/recurring task from the frontend
pub async fn create_task(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<CreateTaskRequest>,
) -> Result<Json<TaskResponse>, (StatusCode, Json<serde_json::Value>)> {
    use chrono::TimeZone;

    let user_id = auth_user.user_id;
    tracing::debug!("Creating task for user {}: {:?}", user_id, request.action);

    // Get user timezone for trigger calculation
    let user_tz_str = state
        .user_core
        .get_user_info(user_id)
        .ok()
        .and_then(|info| info.timezone)
        .unwrap_or_else(|| "UTC".to_string());

    let tz: chrono_tz::Tz = user_tz_str.parse().unwrap_or(chrono_tz::UTC);

    // Calculate initial trigger timestamp
    let now = chrono::Utc::now();
    let now_local = now.with_timezone(&tz);
    let current_ts = now.timestamp() as i32;

    // Parse recurrence_time to get hour and minute
    let (hour, minute) = if let Some(ref time_str) = request.recurrence_time {
        let parts: Vec<&str> = time_str.split(':').collect();
        if parts.len() >= 2 {
            (
                parts[0].parse::<u32>().unwrap_or(8),
                parts[1].parse::<u32>().unwrap_or(0),
            )
        } else {
            (8, 0)
        }
    } else {
        (8, 0) // Default to 8:00 AM
    };

    // Calculate next occurrence
    let mut next_time = now_local.date_naive().and_hms_opt(hour, minute, 0).unwrap();
    if now_local.time() >= chrono::NaiveTime::from_hms_opt(hour, minute, 0).unwrap() {
        // Already past this time today, schedule for tomorrow
        next_time += chrono::Duration::days(1);
    }
    let next_dt = tz.from_local_datetime(&next_time).single().ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid time for timezone"})),
        )
    })?;
    let trigger_ts = next_dt.timestamp() as i32;

    let new_task = crate::models::user_models::NewTask {
        user_id,
        trigger: format!("once_{}", trigger_ts),
        condition: request.condition,
        action: request.action,
        notification_type: request.notification_type.or(Some("sms".to_string())),
        status: "active".to_string(),
        created_at: current_ts,
        is_permanent: Some(1), // Recurring tasks are permanent
        recurrence_rule: request.recurrence_rule,
        recurrence_time: request.recurrence_time,
        sources: request.sources,
        source_lookback_hours: request.source_lookback_hours.or(Some(24)),
    };

    state.user_repository.create_task(&new_task).map_err(|e| {
        tracing::error!("Failed to create task for user {}: {}", user_id, e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to create task: {}", e)})),
        )
    })?;

    // Return the created task (fetch it back to get the ID)
    let tasks = state.user_repository.get_user_tasks(user_id).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)})),
        )
    })?;

    // Find the most recently created task
    let created_task = tasks.into_iter().next().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Task not found after creation"})),
        )
    })?;

    Ok(Json(TaskResponse {
        id: created_task.id,
        user_id: created_task.user_id,
        trigger: created_task.trigger,
        condition: created_task.condition,
        action: created_task.action,
        notification_type: created_task.notification_type,
        status: created_task.status,
        created_at: created_task.created_at,
        is_permanent: created_task.is_permanent,
        recurrence_rule: created_task.recurrence_rule,
        recurrence_time: created_task.recurrence_time,
        sources: created_task.sources,
        source_lookback_hours: created_task.source_lookback_hours,
    }))
}

// Priority Senders handlers
pub async fn create_priority_sender(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<PrioritySenderRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    println!(
        "Attempting to create priority sender for user {} with type: {}",
        auth_user.user_id, request.service_type
    );

    let new_sender = NewPrioritySender {
        user_id: auth_user.user_id,
        sender: request.sender.clone(),
        service_type: request.service_type,
        noti_type: request.noti_type,
        noti_mode: request.noti_mode,
    };

    match state.user_repository.create_priority_sender(&new_sender) {
        Ok(_) => {
            println!(
                "Successfully created priority sender {} for user {}",
                request.sender, auth_user.user_id
            );
            Ok(Json(
                json!({"message": "Priority sender created successfully"}),
            ))
        }
        Err(DieselError::RollbackTransaction) => Err((
            StatusCode::CONFLICT,
            Json(json!({"error": "Priority sender already exists"})),
        )),
        Err(e) => {
            tracing::error!(
                "Failed to create priority sender for user {}: {}",
                auth_user.user_id,
                e
            );
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            ))
        }
    }
}

pub async fn delete_priority_sender(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path((service_type, sender)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    println!(
        "Attempting to delete priority sender {} for user {}",
        sender, auth_user.user_id
    );

    match state
        .user_repository
        .delete_priority_sender(auth_user.user_id, &service_type, &sender)
    {
        Ok(_) => {
            println!(
                "Successfully deleted priority sender {} for user {}",
                sender, auth_user.user_id
            );
            Ok(Json(
                json!({"message": "Priority sender deleted successfully"}),
            ))
        }
        Err(DieselError::NotFound) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Priority sender not found"})),
        )),
        Err(e) => {
            tracing::error!("Failed to delete priority sender {}: {}", sender, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            ))
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct PriorityNotificationInfo {
    pub average_per_day: f32,
    pub estimated_monthly_price: f32,
}

pub async fn get_priority_senders(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    println!("Fetching priority senders for user {}", auth_user.user_id);
    let senders = state
        .user_repository
        .get_priority_senders_all(auth_user.user_id)
        .map_err(|e| {
            tracing::error!(
                "Failed to fetch priority senders for user {}: {}",
                auth_user.user_id,
                e
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?;
    let info = state
        .user_core
        .get_priority_notification_info(auth_user.user_id)
        .map_err(|e| {
            tracing::error!(
                "Failed to fetch priority info for user {}: {}",
                auth_user.user_id,
                e
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?;
    let response: Vec<PrioritySenderResponse> = senders
        .into_iter()
        .map(|sender| PrioritySenderResponse {
            user_id: sender.user_id,
            sender: sender.sender,
            service_type: sender.service_type,
            noti_type: sender.noti_type,
            noti_mode: sender.noti_mode,
        })
        .collect();
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
    println!(
        "Attempting to create keyword for user {}",
        auth_user.user_id
    );

    // First check if the keyword already exists
    let existing_keywords = state
        .user_repository
        .get_keywords(auth_user.user_id, &request.service_type)
        .map_err(|e| {
            tracing::error!(
                "Failed to fetch keywords for user {}: {}",
                auth_user.user_id,
                e
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?;

    // Check if keyword already exists (case-insensitive)
    if existing_keywords
        .iter()
        .any(|k| k.keyword.to_lowercase() == request.keyword.to_lowercase())
    {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({"error": "Keyword already exists"})),
        ));
    }

    let new_keyword = NewKeyword {
        user_id: auth_user.user_id,
        keyword: request.keyword.clone(),
        service_type: request.service_type,
    };

    match state.user_repository.create_keyword(&new_keyword) {
        Ok(_) => {
            println!(
                "Successfully created keyword {} for user {}",
                request.keyword, auth_user.user_id
            );
            Ok(Json(json!({"message": "Keyword created successfully"})))
        }

        Err(e) => {
            tracing::error!(
                "Failed to create keyword for user {}: {}",
                auth_user.user_id,
                e
            );
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            ))
        }
    }
}

pub async fn delete_keyword(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path((service_type, keyword)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    println!(
        "Attempting to delete keyword {} for user {}",
        keyword, auth_user.user_id
    );

    match state
        .user_repository
        .delete_keyword(auth_user.user_id, &service_type, &keyword)
    {
        Ok(_) => {
            println!(
                "Successfully deleted keyword {} for user {}",
                keyword, auth_user.user_id
            );
            Ok(Json(json!({"message": "Keyword deleted successfully"})))
        }
        Err(DieselError::NotFound) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "Keyword not found"})),
        )),
        Err(e) => {
            tracing::error!("Failed to delete keyword {}: {}", keyword, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            ))
        }
    }
}
