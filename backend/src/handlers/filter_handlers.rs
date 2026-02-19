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
}

#[derive(Deserialize)]
pub struct SetPermanenceRequest {
    pub is_permanent: bool,
    pub recurrence_rule: Option<String>, // "daily", "weekly:1,3,5", "monthly:15"
    pub recurrence_time: Option<String>, // "09:00" (HH:MM)
}

#[derive(Deserialize)]
pub struct CreateTaskRequest {
    pub action: String,                    // "generate_digest" or other action
    pub recurrence_rule: Option<String>,   // "daily", "weekly:1,3,5"
    pub recurrence_time: Option<String>,   // "08:00" (HH:MM in user timezone)
    pub sources: Option<String>,           // "email,whatsapp,telegram,signal,calendar"
    pub notification_type: Option<String>, // "sms" or "call"
    pub condition: Option<String>,         // Optional condition to check
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
        })
        .collect();

    Ok(Json(response))
}

/// Response for single task with display formatting
#[derive(Serialize)]
pub struct SingleTaskResponse {
    pub id: i32,
    pub trigger_timestamp: i32,
    pub trigger_type: String,
    pub time_display: String,
    pub description: String,
    pub date_display: String,
    pub relative_display: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sources: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sources_display: Option<String>,
}

/// Get a single task by ID - used for auto-showing newly created tasks
pub async fn get_task(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(task_id): Path<i32>,
) -> Result<Json<SingleTaskResponse>, (StatusCode, Json<serde_json::Value>)> {
    let tasks = state
        .user_repository
        .get_user_tasks(auth_user.user_id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?;

    let task = tasks
        .into_iter()
        .find(|t| t.id == Some(task_id))
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Task not found"})),
            )
        })?;

    // Determine trigger type and timestamp
    let (trigger_type, trigger_timestamp) = if let Some(ts_str) = task.trigger.strip_prefix("once_")
    {
        ("once".to_string(), ts_str.parse::<i32>().unwrap_or(0))
    } else if task.trigger == "recurring_email" {
        ("recurring_email".to_string(), 0)
    } else if task.trigger == "recurring_messaging" {
        ("recurring_messaging".to_string(), 0)
    } else {
        (task.trigger.clone(), 0)
    };

    // Get user timezone for formatting
    let user_info = state.user_core.get_user_info(auth_user.user_id).ok();
    let tz_str = user_info
        .and_then(|u| u.timezone)
        .unwrap_or_else(|| "UTC".to_string());
    let tz: chrono_tz::Tz = tz_str.parse().unwrap_or(chrono_tz::UTC);

    // Get current timestamp for relative display
    let now_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    // Use the formatting functions from dashboard_handlers
    use crate::handlers::dashboard_handlers::{
        format_action_description, format_date_display, format_relative_days, format_time_display,
    };

    // For recurring tasks, show "Ongoing" instead of a timestamp-based time
    let (time_display, date_display, relative_display) =
        if trigger_type == "recurring_email" || trigger_type == "recurring_messaging" {
            (
                "Ongoing".to_string(),
                String::new(),
                "Always active".to_string(),
            )
        } else {
            (
                format_time_display(trigger_timestamp, &tz),
                format_date_display(trigger_timestamp, &tz),
                format_relative_days(trigger_timestamp, now_ts, &tz),
            )
        };

    // Format the action as a human-readable description
    let description = format_action_description(&task.action);

    // Extract condition - filter out JSON objects (those are action data, not display conditions)
    let condition = task.condition.as_ref().and_then(|c| {
        let trimmed = c.trim();
        if trimmed.starts_with('{') || trimmed.is_empty() {
            None
        } else {
            Some(c.clone())
        }
    });

    let sources = task.sources.clone();
    let sources_display = sources.as_ref().map(|s| {
        crate::handlers::dashboard_handlers::format_sources_display(s, &state, auth_user.user_id)
    });

    Ok(Json(SingleTaskResponse {
        id: task.id.unwrap_or(0),
        trigger_timestamp,
        trigger_type,
        time_display,
        description,
        date_display,
        relative_display,
        condition,
        sources,
        sources_display,
    }))
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

    // Calculate next occurrence - handle DST transitions gracefully
    let mut next_time = now_local
        .date_naive()
        .and_hms_opt(hour, minute, 0)
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Invalid time specified"})),
            )
        })?;
    let check_time = chrono::NaiveTime::from_hms_opt(hour, minute, 0).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid time specified"})),
        )
    })?;
    if now_local.time() >= check_time {
        // Already past this time today, schedule for tomorrow
        next_time += chrono::Duration::days(1);
    }
    let next_dt = tz.from_local_datetime(&next_time).single().ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid time for timezone (DST transition?)"})),
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
        end_time: None,
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
    }))
}

// Priority Senders handlers
pub async fn create_priority_sender(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<PrioritySenderRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::debug!(
        "Attempting to create priority sender for user {} with type: {}",
        auth_user.user_id,
        request.service_type
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
            tracing::debug!(
                "Successfully created priority sender {} for user {}",
                request.sender,
                auth_user.user_id
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
    tracing::debug!(
        "Attempting to delete priority sender {} for user {}",
        sender,
        auth_user.user_id
    );

    match state
        .user_repository
        .delete_priority_sender(auth_user.user_id, &service_type, &sender)
    {
        Ok(_) => {
            tracing::debug!(
                "Successfully deleted priority sender {} for user {}",
                sender,
                auth_user.user_id
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
    tracing::debug!("Fetching priority senders for user {}", auth_user.user_id);
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
    tracing::debug!(
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
            tracing::debug!(
                "Successfully created keyword {} for user {}",
                request.keyword,
                auth_user.user_id
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
    tracing::debug!(
        "Attempting to delete keyword {} for user {}",
        keyword,
        auth_user.user_id
    );

    match state
        .user_repository
        .delete_keyword(auth_user.user_id, &service_type, &keyword)
    {
        Ok(_) => {
            tracing::debug!(
                "Successfully deleted keyword {} for user {}",
                keyword,
                auth_user.user_id
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

// Task edit with AI
#[derive(Deserialize)]
pub struct TaskEditRequest {
    pub instruction: String,
}

#[derive(Serialize)]
pub struct TaskEditResponse {
    pub message: String,
    pub success: bool,
}

/// AI response structure for task editing
#[derive(Deserialize)]
struct AiTaskEditResult {
    new_trigger_time: Option<String>,
    new_action: Option<serde_json::Value>,
    new_condition: Option<String>,
    new_sources: Option<String>,
    explanation: String,
}

pub async fn edit_task_with_ai(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(task_id): Path<i32>,
    Json(request): Json<TaskEditRequest>,
) -> Result<Json<TaskEditResponse>, (StatusCode, Json<serde_json::Value>)> {
    use openai_api_rs::v1::chat_completion::{
        ChatCompletionMessage, ChatCompletionRequest, Content, MessageRole,
    };

    tracing::info!(
        "edit_task_with_ai called: task_id={}, user_id={}, instruction={}",
        task_id,
        auth_user.user_id,
        request.instruction
    );

    // Get the task and verify ownership
    let tasks = state
        .user_repository
        .get_user_tasks(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Failed to get user tasks: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?;

    tracing::info!("Found {} tasks for user", tasks.len());

    let task = tasks
        .into_iter()
        .find(|t| t.id == Some(task_id))
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Task not found"})),
            )
        })?;

    // Build context for LLM client and timezone (respects user's LLM provider preference)
    let ctx = crate::context::ContextBuilder::for_user(&state, auth_user.user_id)
        .with_user_context()
        .build()
        .await
        .map_err(|e| {
            tracing::error!("Failed to build context: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "AI service unavailable"})),
            )
        })?;

    let user_tz_str = ctx
        .timezone
        .as_ref()
        .map(|tz| tz.tz_str.clone())
        .unwrap_or_else(|| "UTC".to_string());
    let formatted_now = ctx
        .timezone
        .as_ref()
        .map(|tz| tz.formatted_now.clone())
        .unwrap_or_else(|| chrono::Utc::now().format("%Y-%m-%d %H:%M UTC").to_string());

    // Parse trigger to get time display
    let trigger_ts = task
        .trigger
        .strip_prefix("once_")
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);
    let current_time = chrono::DateTime::from_timestamp(trigger_ts, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "Unknown".to_string());

    // Build AI prompt - show action, condition, and sources separately
    let action_display = if !task.action.trim().is_empty() {
        task.action.trim().to_string()
    } else {
        "(empty)".to_string()
    };
    let condition_display = task
        .condition
        .as_ref()
        .filter(|c| !c.trim().is_empty() && !c.trim().starts_with('{'))
        .cloned()
        .unwrap_or_else(|| "(none)".to_string());
    let sources_display = task
        .sources
        .as_ref()
        .filter(|s| !s.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| "(none)".to_string());

    // Get available tools dynamically
    let tools_prompt = crate::tool_call_utils::utils::get_runtime_tools_prompt();

    let system_prompt = format!(
        r#"You are editing a scheduled task that will be executed by an AI assistant.

CURRENT TASK:
- Scheduled time: {} (timezone: {})
- Action: {}
- Condition: {}
- Sources: {}

USER'S EDIT REQUEST: "{}"

{}
IMPORTANT RULES:
1. If the task is ONLY to remind/notify the user (no other action), set new_action to: {{"tool":"send_reminder","params":{{"message":"Your message"}}}}
2. For tasks with actual actions (Tesla, email, etc.), the user is automatically notified when the task completes
3. Use exact tool names from the list above
4. send_reminder is NOT a standalone tool - it is only valid as new_action.tool

Return ONLY valid JSON (no markdown, no code blocks):
{{
  "new_trigger_time": "YYYY-MM-DDTHH:MM:SS" or null,
  "new_action": {{"tool": "tool_name", "params": {{...}}}} or null,
  "new_condition": "natural language condition text" or null,
  "new_sources": "comma-separated source types" or null,
  "explanation": "Brief explanation"
}}

FIELD SEPARATION (CRITICAL):
- new_action: ONLY change the action (what to do). Do NOT change this when user only asks to change the condition.
- new_condition: ONLY change the condition (what to check). Do NOT change this when user only asks to change the action.
- new_sources: ONLY change the data sources to fetch (e.g., "weather", "email", "calendar"). Set this when the condition depends on external data.
- When the user asks to change the condition, ONLY return new_condition (and new_sources if needed). Do NOT change the action.
- When the user asks to change the action, ONLY return new_action. Do NOT change the condition.

SOURCE TYPES:
- "weather" - fetch weather data (for temperature/rain conditions)
- "email" - fetch recent emails
- "calendar" - fetch calendar events
- "whatsapp" - fetch WhatsApp messages
- "telegram" - fetch Telegram messages
- "signal" - fetch Signal messages
- Multiple sources: "weather,email"

SOURCE EDITING RULES:
- To REMOVE a source: return new_sources with the FULL remaining list (excluding the removed source)
  Example: Current sources = "email,whatsapp,telegram,signal,calendar"
  User: "remove calendar" -> new_sources = "email,whatsapp,telegram,signal"
- To ADD a source: return new_sources with the full list INCLUDING the new source
  Example: Current sources = "email,weather"
  User: "add calendar" -> new_sources = "email,weather,calendar"
- To REPLACE all sources: return the new complete list
- NEVER return null for new_sources when the user asks to add or remove a source

ACTION FORMAT (CRITICAL):
- new_action MUST be a JSON object with "tool" and optional "params" keys
- For reminders: {{"tool":"send_reminder","params":{{"message":"Call mom"}}}}
- For Tesla: {{"tool":"control_tesla","params":{{"command":"climate_on"}}}}
- For weather: {{"tool":"get_weather","params":{{"location":"New York"}}}}

CRITICAL - REMINDER vs ACTION:
- "remind me to X" = {{"tool":"send_reminder","params":{{"message":"X"}}}}
- "do X" / "turn on X" = {{"tool":"control_tesla","params":{{"command":"climate_on"}}}}

LANGUAGE: Never use 'you' or 'your' - use descriptive third-person text.

CORRECT EXAMPLES:
- User: "change to 11pm" -> new_trigger_time with 11pm, everything else null
- User: "change the condition to if it's snowing" -> new_condition = "if it's snowing", new_sources = "weather", new_action = null
- User: "make it a reminder about the package" -> new_action = {{"tool":"send_reminder","params":{{"message":"Package reminder"}}}}, new_condition = null
- User: "add weather check" -> new_sources = "weather", everything else null
- User: "remove calendar from the sources" (current sources: "email,whatsapp,calendar") -> new_sources = "email,whatsapp", everything else null
- User: "don't check email" (current sources: "email,weather") -> new_sources = "weather", everything else null

WRONG:
- new_action = "send_reminder(Call you)" - WRONG! Must be JSON object, never a string
- Changing new_action when user only asked to change the condition
- Returning null for all fields when user clearly wants a change

TIME RULES:
- Calculate actual datetime for "tomorrow", "9am", "in 2 hours", etc.
- Timezone: {}
- Current time: {}
- Return null if no time change

CHANGE RULES:
- Return null for unchanged fields
- Only set fields that the user explicitly asked to change"#,
        current_time,
        user_tz_str,
        action_display,
        condition_display,
        sources_display,
        request.instruction,
        tools_prompt,
        user_tz_str,
        formatted_now
    );

    // Call AI to interpret the edit instruction
    tracing::info!("Calling AI to interpret edit instruction");

    let messages = vec![ChatCompletionMessage {
        role: MessageRole::user,
        content: Content::Text(system_prompt),
        name: None,
        tool_calls: None,
        tool_call_id: None,
    }];

    let ai_request = ChatCompletionRequest::new(ctx.model.clone(), messages)
        .max_tokens(500)
        .temperature(0.1);

    tracing::info!("Sending request to AI...");
    let response = ctx.client.chat_completion(ai_request).await.map_err(|e| {
        tracing::error!("AI request failed: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("AI request failed: {}", e)})),
        )
    })?;
    tracing::info!("AI response received");

    let ai_response = response
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "No response from AI"})),
            )
        })?;

    tracing::debug!("AI response for task edit: {}", ai_response);

    // Clean up AI response - remove markdown code blocks if present
    let cleaned_response = ai_response
        .trim()
        .strip_prefix("```json")
        .or_else(|| ai_response.trim().strip_prefix("```"))
        .unwrap_or(&ai_response)
        .trim()
        .strip_suffix("```")
        .unwrap_or(&ai_response)
        .trim();

    // Parse the AI response
    let edit_result: AiTaskEditResult = serde_json::from_str(cleaned_response).map_err(|e| {
        tracing::error!(
            "Failed to parse AI response: {} - Response was: {}",
            e,
            cleaned_response
        );
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to understand edit instruction. Please try rephrasing."})),
        )
    })?;

    // Apply the changes
    let mut changes_made = false;

    // Update trigger time if specified
    if let Some(new_time_str) = &edit_result.new_trigger_time {
        let tz: chrono_tz::Tz = user_tz_str.parse().unwrap_or(chrono_tz::UTC);

        // Parse the datetime string
        if let Ok(naive_dt) =
            chrono::NaiveDateTime::parse_from_str(new_time_str, "%Y-%m-%dT%H:%M:%S")
        {
            use chrono::TimeZone;
            if let Some(local_dt) = tz.from_local_datetime(&naive_dt).single() {
                let new_ts = local_dt.timestamp() as i32;
                let new_trigger = format!("once_{}", new_ts);
                state
                    .user_repository
                    .reschedule_task(task_id, &new_trigger)
                    .map_err(|e| {
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({"error": format!("Failed to update task time: {}", e)})),
                        )
                    })?;
                changes_made = true;
            }
        }
    }

    // Update action if specified - serialize the JSON Value to a string for storage
    if let Some(new_action) = &edit_result.new_action {
        let action_str = serde_json::to_string(new_action).map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to serialize action: {}", e)})),
            )
        })?;
        state
            .user_repository
            .update_task_action(auth_user.user_id, task_id, &action_str)
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to update task action: {}", e)})),
                )
            })?;
        changes_made = true;
    }

    // Update condition if specified
    if let Some(new_condition) = &edit_result.new_condition {
        state
            .user_repository
            .update_task_condition_only(auth_user.user_id, task_id, Some(new_condition))
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to update task condition: {}", e)})),
                )
            })?;
        changes_made = true;
    }

    // Update sources if specified
    if let Some(new_sources) = &edit_result.new_sources {
        state
            .user_repository
            .update_task_sources(auth_user.user_id, task_id, Some(new_sources))
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to update task sources: {}", e)})),
                )
            })?;
        changes_made = true;
    }

    if changes_made {
        Ok(Json(TaskEditResponse {
            message: edit_result.explanation,
            success: true,
        }))
    } else {
        Ok(Json(TaskEditResponse {
            message: "No changes were made. Please clarify what you'd like to change.".to_string(),
            success: false,
        }))
    }
}
