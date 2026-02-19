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
    status: Option<String>,
    created_at: i32,
    is_permanent: Option<i32>,
    recurrence_rule: Option<String>,
    recurrence_time: Option<String>,
    sources: Option<String>,
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
        "Attempting to delete item {} for user {}",
        task_id,
        auth_user.user_id
    );

    match state
        .item_repository
        .delete_item(task_id, auth_user.user_id)
    {
        Ok(true) => {
            tracing::debug!(
                "Successfully deleted item {} for user {}",
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
            tracing::error!("Failed to delete item {}: {}", task_id, e);
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
    tracing::debug!("Fetching items for user {}", auth_user.user_id);

    let items = state
        .item_repository
        .get_items(auth_user.user_id)
        .map_err(|e| {
            tracing::error!(
                "Failed to fetch items for user {}: {}",
                auth_user.user_id,
                e
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?;

    let response: Vec<TaskResponse> = items
        .into_iter()
        .map(|item| {
            let trigger = if item.monitor {
                "recurring_email".to_string()
            } else if let Some(nca) = item.next_check_at {
                format!("once_{}", nca)
            } else {
                "once_0".to_string()
            };
            TaskResponse {
                id: item.id,
                user_id: item.user_id,
                trigger,
                condition: None,
                action: item.summary.clone(),
                status: Some("active".to_string()),
                created_at: item.created_at,
                is_permanent: if item.monitor { Some(1) } else { Some(0) },
                recurrence_rule: None,
                recurrence_time: None,
                sources: None,
            }
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

/// Get a single item by ID - used for auto-showing newly created items
pub async fn get_task(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(task_id): Path<i32>,
) -> Result<Json<SingleTaskResponse>, (StatusCode, Json<serde_json::Value>)> {
    let item = state
        .item_repository
        .get_item(task_id, auth_user.user_id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Task not found"})),
            )
        })?;

    // Determine trigger type and timestamp from item fields
    let (trigger_type, trigger_timestamp) = if item.monitor {
        ("recurring_email".to_string(), 0)
    } else if let Some(nca) = item.next_check_at {
        ("once".to_string(), nca)
    } else {
        ("once".to_string(), 0)
    };

    // Get user timezone for formatting
    let user_info = state.user_core.get_user_info(auth_user.user_id).ok();
    let tz_str = user_info
        .and_then(|u| u.timezone)
        .unwrap_or_else(|| "UTC".to_string());
    let tz: chrono_tz::Tz = tz_str.parse().unwrap_or(chrono_tz::UTC);

    let now_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    use crate::handlers::dashboard_handlers::{
        format_date_display, format_relative_days, format_time_display,
    };

    let (time_display, date_display, relative_display) = if item.monitor {
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

    Ok(Json(SingleTaskResponse {
        id: item.id.unwrap_or(0),
        trigger_timestamp,
        trigger_type,
        time_display,
        description: item.summary,
        date_display,
        relative_display,
        condition: None,
        sources: None,
        sources_display: None,
    }))
}

/// Create a new scheduled/recurring item from the frontend
pub async fn create_task_item(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<CreateTaskRequest>,
) -> Result<Json<TaskResponse>, (StatusCode, Json<serde_json::Value>)> {
    use chrono::TimeZone;

    let user_id = auth_user.user_id;
    tracing::debug!("Creating item for user {}: {:?}", user_id, request.action);

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

    // Build natural language summary
    let rule = request.recurrence_rule.as_deref().unwrap_or("daily");
    let time = request.recurrence_time.as_deref().unwrap_or("08:00");
    let is_monitor = request.condition.is_some();

    let summary = if is_monitor {
        format!(
            "Monitor: {}. Alert when match arrives.",
            request
                .condition
                .as_deref()
                .unwrap_or("any matching content")
        )
    } else if request.action == "generate_digest" {
        let sources = request
            .sources
            .as_deref()
            .unwrap_or("email,whatsapp,telegram,signal,calendar");
        format!(
            "Daily digest. Sources: {}. Repeats {} at {}.",
            sources, rule, time
        )
    } else {
        format!(
            "Reminder: {}. Repeats {} at {}.",
            request.action, rule, time
        )
    };

    // Encode notification_type into summary if it's "call" or "call_sms"
    let noti_type = request.notification_type.as_deref().unwrap_or("sms");
    let summary = if noti_type == "call" || noti_type == "call_sms" {
        format!("{} [VIA CALL]", summary)
    } else {
        summary
    };

    let new_item = crate::models::user_models::NewItem {
        user_id,
        summary: summary.clone(),
        monitor: is_monitor,
        due_at: None,
        next_check_at: Some(trigger_ts),
        priority: 0,
        source_id: None,
        created_at: current_ts,
    };

    let item_id = state.item_repository.create_item(&new_item).map_err(|e| {
        tracing::error!("Failed to create item for user {}: {}", user_id, e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to create item: {}", e)})),
        )
    })?;

    // Fetch the created item
    let created_item = state
        .item_repository
        .get_item(item_id, user_id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Item not found after creation"})),
            )
        })?;

    Ok(Json(TaskResponse {
        id: created_item.id,
        user_id: created_item.user_id,
        trigger: format!("once_{}", trigger_ts),
        condition: if is_monitor {
            request.condition.clone()
        } else {
            None
        },
        action: request.action,
        status: Some("active".to_string()),
        created_at: created_item.created_at,
        is_permanent: Some(1),
        recurrence_rule: Some(rule.to_string()),
        recurrence_time: Some(time.to_string()),
        sources: request.sources,
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

/// AI response structure for item editing
#[derive(Deserialize)]
struct AiItemEditResult {
    new_trigger_time: Option<String>,
    new_summary: Option<String>,
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
        "edit_task_with_ai called: item_id={}, user_id={}, instruction={}",
        task_id,
        auth_user.user_id,
        request.instruction
    );

    // Get the item and verify ownership
    let item = state
        .item_repository
        .get_item(task_id, auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Failed to get item: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Item not found"})),
            )
        })?;

    // Build context for LLM client and timezone
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

    // Format current scheduled time
    let current_time = item
        .next_check_at
        .and_then(|nca| chrono::DateTime::from_timestamp(nca as i64, 0))
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "Not scheduled".to_string());

    let item_type = if item.monitor {
        "monitor (watches incoming data)"
    } else {
        "scheduled item"
    };

    let system_prompt = format!(
        r#"You are editing an item tracked by an AI assistant.

CURRENT ITEM:
- Type: {}
- Summary: {}
- Scheduled time: {} (timezone: {})

USER'S EDIT REQUEST: "{}"

Return ONLY valid JSON (no markdown, no code blocks):
{{
  "new_trigger_time": "YYYY-MM-DDTHH:MM:SS" or null,
  "new_summary": "updated summary text" or null,
  "explanation": "Brief explanation"
}}

RULES:
- new_trigger_time: Only set if user wants to change the scheduled time
- new_summary: Only set if user wants to change what the item does/tracks
- explanation: Always provide a brief explanation of the change
- Return null for unchanged fields

TIME RULES:
- Calculate actual datetime for "tomorrow", "9am", "in 2 hours", etc.
- Timezone: {}
- Current time: {}
- Return null if no time change"#,
        item_type,
        item.summary,
        current_time,
        user_tz_str,
        request.instruction,
        user_tz_str,
        formatted_now
    );

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

    let response = ctx.client.chat_completion(ai_request).await.map_err(|e| {
        tracing::error!("AI request failed: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("AI request failed: {}", e)})),
        )
    })?;

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

    tracing::debug!("AI response for item edit: {}", ai_response);

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

    let edit_result: AiItemEditResult = serde_json::from_str(cleaned_response).map_err(|e| {
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

    let mut changes_made = false;

    // Update trigger time if specified
    if let Some(new_time_str) = &edit_result.new_trigger_time {
        let tz: chrono_tz::Tz = user_tz_str.parse().unwrap_or(chrono_tz::UTC);

        if let Ok(naive_dt) =
            chrono::NaiveDateTime::parse_from_str(new_time_str, "%Y-%m-%dT%H:%M:%S")
        {
            use chrono::TimeZone;
            if let Some(local_dt) = tz.from_local_datetime(&naive_dt).single() {
                let new_ts = local_dt.timestamp() as i32;
                state
                    .item_repository
                    .update_next_check_at(task_id, Some(new_ts))
                    .map_err(|e| {
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({"error": format!("Failed to update time: {}", e)})),
                        )
                    })?;
                changes_made = true;
            }
        }
    }

    // Update summary if specified
    if let Some(new_summary) = &edit_result.new_summary {
        state
            .item_repository
            .update_summary(task_id, new_summary)
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to update summary: {}", e)})),
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
