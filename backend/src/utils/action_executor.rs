//! Action Executor for Task Runtime
//!
//! This module handles executing task actions at runtime.
//! It's called by the scheduler when scheduled tasks are due, or when recurring
//! task conditions are matched.
//!
//! Flow:
//! 1. Fetch sources (if configured) - emails, messages, calendar
//! 2. If condition set, evaluate condition against source data
//! 3. Execute action tool (if any) - e.g., generate_digest, control_tesla
//! 4. Generate notification message using AI
//! 5. Send notification via SMS/call

use crate::context::ContextBuilder;
use crate::models::user_models::ContactProfile;
use crate::AppState;
use crate::UserCoreOps;
use openai_api_rs::v1::chat_completion;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// A structured representation of a task action.
/// Stored as JSON in the DB `action` column.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StructuredAction {
    pub tool: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl StructuredAction {
    /// Convert to old-style action string for display formatting compatibility.
    /// e.g. StructuredAction { tool: "send_reminder", params: {"message": "Call mom"} }
    ///   -> "send_reminder(Call mom)"
    pub fn to_action_string(&self) -> String {
        match &self.params {
            Some(params) if params.is_object() => {
                // Extract the "primary" param value for display
                let obj = params.as_object().unwrap();
                let display_val = match self.tool.as_str() {
                    "send_reminder" => obj.get("message").and_then(|v| v.as_str()),
                    "control_tesla" => obj.get("command").and_then(|v| v.as_str()),
                    "get_weather" => obj.get("location").and_then(|v| v.as_str()),
                    _ => obj.values().next().and_then(|v| v.as_str()),
                };
                match display_val {
                    Some(val) => format!("{}({})", self.tool, val),
                    None => self.tool.clone(),
                }
            }
            _ => self.tool.clone(),
        }
    }
}

/// Context about who triggered a task, used to determine sender trust.
pub enum SenderContext<'a> {
    /// Time-triggered task with no external sender - always trusted.
    TimeBased,
    /// Triggered by an incoming email.
    Email {
        from_email: &'a str,
        from_display: &'a str,
    },
    /// Triggered by an incoming messaging platform message.
    Messaging {
        service: &'a str,
        room_id: &'a str,
        room_name: &'a str,
    },
}

/// Check if the sender is trusted by matching against the user's contact profiles.
///
/// - Time-based tasks are always trusted (no external sender).
/// - Email senders are trusted if their address matches a contact profile's email_addresses.
/// - Messaging senders are trusted if the room_id or chat name matches a contact profile.
pub fn is_sender_trusted(state: &Arc<AppState>, user_id: i32, sender: &SenderContext) -> bool {
    match sender {
        SenderContext::TimeBased => true,
        SenderContext::Email {
            from_email,
            from_display,
        } => {
            let profiles = state
                .user_repository
                .get_contact_profiles(user_id)
                .unwrap_or_default();
            let from_email_lower = from_email.to_lowercase();
            let from_display_lower = from_display.to_lowercase();
            match_email_sender(&profiles, &from_email_lower, &from_display_lower)
        }
        SenderContext::Messaging {
            service,
            room_id,
            room_name,
        } => {
            let profiles = state
                .user_repository
                .get_contact_profiles(user_id)
                .unwrap_or_default();
            let chat_name = crate::utils::bridge::remove_bridge_suffix(room_name);
            match_messaging_sender(&profiles, service, room_id, &chat_name)
        }
    }
}

/// Check if an email sender matches any contact profile's email addresses.
fn match_email_sender(
    profiles: &[ContactProfile],
    from_email_lower: &str,
    from_display_lower: &str,
) -> bool {
    profiles.iter().any(|p| {
        if let Some(ref emails) = p.email_addresses {
            emails.split(',').any(|e| {
                let e = e.trim().to_lowercase();
                !e.is_empty() && (from_email_lower.contains(&e) || from_display_lower.contains(&e))
            })
        } else {
            false
        }
    })
}

/// Check if a messaging sender matches any contact profile by room_id or chat name.
fn match_messaging_sender(
    profiles: &[ContactProfile],
    service: &str,
    room_id: &str,
    chat_name: &str,
) -> bool {
    let chat_lower = chat_name.to_lowercase();
    profiles.iter().any(|p| {
        // Try room_id match first (stable identifier)
        let profile_room_id = match service {
            "whatsapp" => p.whatsapp_room_id.as_deref(),
            "telegram" => p.telegram_room_id.as_deref(),
            "signal" => p.signal_room_id.as_deref(),
            _ => None,
        };
        if profile_room_id == Some(room_id) {
            return true;
        }
        // Fall back to display name matching
        match service {
            "whatsapp" => p.whatsapp_chat.as_ref(),
            "telegram" => p.telegram_chat.as_ref(),
            "signal" => p.signal_chat.as_ref(),
            _ => None,
        }
        .map(|c| {
            let c_lower = crate::utils::bridge::remove_bridge_suffix(c).to_lowercase();
            chat_lower.contains(&c_lower) || c_lower.contains(&chat_lower)
        })
        .unwrap_or(false)
    })
}

/// Result of executing a task's action
pub enum ActionResult {
    Success { message: String },
    Failed { error: String },
    Skipped { reason: String }, // Condition not met
}

/// Calculate how many hours to look back for source data.
///
/// Returns hours since the last completed task of the same action type,
/// capped at 24 hours. If no previous task found, defaults to 24 hours.
fn calculate_lookback_hours(state: &Arc<AppState>, user_id: i32, action: &str) -> i32 {
    const MAX_LOOKBACK_HOURS: i32 = 24;

    // Get the last completed task time for this user and action
    match state
        .user_repository
        .get_last_completed_task_time(user_id, action)
    {
        Ok(Some(completed_at)) => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i32;

            let hours_since = (now - completed_at) / 3600;

            // Cap at max lookback
            hours_since.clamp(1, MAX_LOOKBACK_HOURS)
        }
        Ok(None) | Err(_) => {
            // No previous task or error - use default
            MAX_LOOKBACK_HOURS
        }
    }
}

/// Fetch data from configured sources (concurrently)
///
/// Sources can be: email, whatsapp, telegram, signal, calendar
/// Returns formatted string with all source data
pub async fn fetch_sources(
    state: &Arc<AppState>,
    user_id: i32,
    sources: &str,
    lookback_hours: i32,
) -> Result<String, String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let since = now - (lookback_hours as i64 * 3600);

    let source_list: Vec<String> = sources
        .split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect();

    let futures: Vec<_> = source_list
        .iter()
        .map(|source| {
            let state = state.clone();
            let source = source.clone();
            async move {
                match source.as_str() {
                    "email" => fetch_email_source(&state, user_id, since).await,
                    "whatsapp" | "telegram" | "signal" => {
                        fetch_bridge_source(&state, user_id, &source, since).await
                    }
                    "calendar" => fetch_calendar_source(&state, user_id).await,
                    "weather" => fetch_weather_source(&state, user_id).await,
                    _ => {
                        tracing::warn!("Unknown source type: {}", source);
                        None
                    }
                }
            }
        })
        .collect();

    let results = futures::future::join_all(futures).await;
    let context_parts: Vec<String> = results.into_iter().flatten().collect();

    if context_parts.is_empty() {
        Ok(String::new())
    } else {
        Ok(context_parts.join("\n\n---\n\n"))
    }
}

async fn fetch_email_source(state: &Arc<AppState>, user_id: i32, since: i64) -> Option<String> {
    match crate::handlers::imap_handlers::fetch_emails_imap(
        state,
        user_id,
        true,     // preview_only
        Some(20), // limit
        false,    // unprocessed
        true,     // unread_only
    )
    .await
    {
        Ok(emails) => {
            let recent: Vec<_> = emails
                .into_iter()
                .filter(|e| e.date.map(|d| d.timestamp() > since).unwrap_or(false))
                .collect();
            if recent.is_empty() {
                None
            } else {
                let email_str = recent
                    .iter()
                    .map(|e| {
                        format!(
                            "- From: {}, Subject: {}, Date: {}",
                            e.from.as_deref().unwrap_or("unknown"),
                            e.subject.as_deref().unwrap_or("(no subject)"),
                            e.date_formatted.as_deref().unwrap_or("unknown")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                Some(format!("EMAIL ({} messages):\n{}", recent.len(), email_str))
            }
        }
        Err(e) => {
            tracing::warn!("Failed to fetch emails for user {}: {:?}", user_id, e);
            Some("EMAIL: (fetch failed)".to_string())
        }
    }
}

async fn fetch_bridge_source(
    state: &Arc<AppState>,
    user_id: i32,
    source: &str,
    since: i64,
) -> Option<String> {
    match crate::utils::bridge::fetch_bridge_messages(
        source, state, user_id, since, true, // unread_only
    )
    .await
    {
        Ok(messages) => {
            if messages.is_empty() {
                None
            } else {
                let msg_str = messages
                    .iter()
                    .take(15)
                    .map(|m| {
                        format!(
                            "- {} in {}: {} ({})",
                            m.sender_display_name, m.room_name, m.content, m.formatted_timestamp
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                Some(format!(
                    "{} ({} messages):\n{}",
                    source.to_uppercase(),
                    messages.len(),
                    msg_str
                ))
            }
        }
        Err(e) => {
            tracing::warn!(
                "Failed to fetch {} messages for user {}: {:?}",
                source,
                user_id,
                e
            );
            Some(format!("{}: (fetch failed)", source.to_uppercase()))
        }
    }
}

async fn fetch_calendar_source(state: &Arc<AppState>, user_id: i32) -> Option<String> {
    let calendar_result =
        crate::tool_call_utils::calendar::handle_fetch_calendar_events(state, user_id, "{}").await;
    if calendar_result.contains("No events") || calendar_result.contains("not connected") {
        None
    } else {
        Some(format!("CALENDAR:\n{}", calendar_result))
    }
}

async fn fetch_weather_source(state: &Arc<AppState>, user_id: i32) -> Option<String> {
    let user_info = state.user_core.get_user_info(user_id).ok();
    let location = user_info
        .and_then(|u| u.location)
        .unwrap_or_else(|| "current location".to_string());

    match crate::utils::tool_exec::get_weather(state, &location, "metric", "current", user_id).await
    {
        Ok(weather_data) => Some(format!("WEATHER:\n{}", weather_data)),
        Err(e) => {
            tracing::warn!("Failed to fetch weather: {:?}", e);
            Some("WEATHER: (fetch failed)".to_string())
        }
    }
}

/// Parse an action string into a StructuredAction.
/// Handles both new JSON format and old `tool(param)` format for backward compatibility.
///
/// New format: `{"tool":"send_reminder","params":{"message":"Call mom"}}`
/// Old format: `send_reminder(Call mom)` or `generate_digest`
pub fn parse_action_structured(action: &str) -> StructuredAction {
    let action = action.trim();
    if action.is_empty() {
        return StructuredAction {
            tool: String::new(),
            params: None,
        };
    }

    // Try JSON parse first (new format)
    if let Ok(structured) = serde_json::from_str::<StructuredAction>(action) {
        return structured;
    }

    // Fall back to old tool(param) format
    if let Some(idx) = action.find('(') {
        if action.ends_with(')') {
            let tool = action[..idx].to_string();
            let param = action[idx + 1..action.len() - 1].to_string();
            // Map old param to the correct JSON shape per tool
            let params = match tool.as_str() {
                "send_reminder" => serde_json::json!({ "message": param }),
                "control_tesla" => serde_json::json!({ "command": param }),
                "get_weather" => serde_json::json!({ "location": param }),
                "send_chat_message" => {
                    let parts: Vec<&str> = param.splitn(3, ',').map(|s| s.trim()).collect();
                    if parts.len() >= 3 {
                        serde_json::json!({
                            "platform": parts[0],
                            "contact": parts[1],
                            "message": parts[2]
                        })
                    } else {
                        serde_json::json!({ "raw": param })
                    }
                }
                _ => serde_json::json!({ "raw": param }),
            };
            return StructuredAction {
                tool,
                params: Some(params),
            };
        }
    }

    // Simple tool name with no params
    StructuredAction {
        tool: action.to_string(),
        params: None,
    }
}

/// Execute a single tool call directly (without AI) from a StructuredAction.
///
/// For API-calling tools (tesla, weather, email, chat), actually performs the action.
/// For non-API tools (reminder, digest) or unknown/natural-language actions,
/// passes data through to the unified notification step.
async fn execute_direct_tool(
    state: &Arc<AppState>,
    user_id: i32,
    action: &StructuredAction,
    source_data: &str,
    sender_trusted: bool,
) -> Result<String, String> {
    // Block restricted tools for untrusted senders (no matching contact profile).
    // Restriction is declared per-tool via ToolHandler::is_restricted() in the registry.
    if !sender_trusted && state.tool_registry.is_restricted(&action.tool) {
        return Err(format!(
            "Action '{}' blocked: sender not in contact profiles",
            action.tool
        ));
    }

    let params = action.params.as_ref();

    match action.tool.as_str() {
        "control_tesla" => {
            let command = params
                .and_then(|p| p.get("command"))
                .and_then(|v| v.as_str())
                .unwrap_or("status");
            let args = serde_json::json!({ "command": command }).to_string();
            Ok(crate::tool_call_utils::tesla::handle_tesla_command(
                state, user_id, &args, true, // silent mode
            )
            .await)
        }
        "get_weather" => {
            let location = params
                .and_then(|p| p.get("location"))
                .and_then(|v| v.as_str())
                .unwrap_or("current location");
            crate::utils::tool_exec::get_weather(state, location, "metric", "current", user_id)
                .await
                .map_err(|e| e.to_string())
        }
        "fetch_calendar_events" => Ok(
            crate::tool_call_utils::calendar::handle_fetch_calendar_events(state, user_id, "{}")
                .await,
        ),
        "send_email" => {
            let user = state
                .user_core
                .find_by_id(user_id)
                .map_err(|e| e.to_string())?
                .ok_or("User not found")?;
            let args_str = serde_json::to_string(params.unwrap_or(&serde_json::json!({}))).unwrap();
            match crate::tool_call_utils::email::handle_send_email(state, user_id, &args_str, &user)
                .await
            {
                Ok((_, _, axum::Json(resp))) => Ok(resp.message),
                Err(e) => Err(e.to_string()),
            }
        }
        "send_chat_message" => {
            let user = state
                .user_core
                .find_by_id(user_id)
                .map_err(|e| e.to_string())?
                .ok_or("User not found")?;
            let args_str = serde_json::to_string(params.unwrap_or(&serde_json::json!({}))).unwrap();
            match crate::tool_call_utils::bridge::handle_send_chat_message(
                state, user_id, &args_str, &user, None,
            )
            .await
            {
                Ok((_, _, axum::Json(resp))) => Ok(resp.message),
                Err(e) => Err(e.to_string()),
            }
        }
        "send_reminder" => {
            let message = params
                .and_then(|p| p.get("message"))
                .and_then(|v| v.as_str())
                .unwrap_or("Reminder");
            Ok(message.to_string())
        }
        _ => {
            // No tool to execute, natural language action, or generate_digest.
            // Pass source data through to the unified notification step.
            if source_data.is_empty() {
                Ok("No source data available.".to_string())
            } else {
                Ok(source_data.to_string())
            }
        }
    }
}

/// Generate a unified notification message for any task type.
///
/// This single function handles all notification formatting:
/// - Digest actions: produces a detailed summary grouped by platform
/// - API actions (tesla, weather): briefly describes what happened
/// - Reminders: states the reminder clearly
/// - Uses user's timezone for relative timestamps
async fn generate_task_notification(
    state: &Arc<AppState>,
    user_id: i32,
    action: &StructuredAction,
    source_data: &str,
    action_result: &str,
    condition: Option<&str>,
) -> Result<String, String> {
    // If action result already looks like a formatted digest, use it directly
    if action_result.len() > 50
        && (action_result.contains("WhatsApp:")
            || action_result.contains("Email:")
            || action_result.contains("Telegram:"))
    {
        return Ok(action_result.to_string());
    }

    let ctx = ContextBuilder::for_user(state, user_id)
        .with_user_context()
        .build()
        .await
        .map_err(|e| format!("Failed to build context: {}", e))?;

    let tz_str = ctx
        .timezone
        .as_ref()
        .map(|tz| tz.tz_str.as_str())
        .unwrap_or("UTC");
    let formatted_now = ctx
        .timezone
        .as_ref()
        .map(|tz| tz.formatted_now.as_str())
        .unwrap_or("unknown");

    // Build the action description
    let action_desc = if action.tool.is_empty() {
        String::new()
    } else {
        let params_str = action
            .params
            .as_ref()
            .map(|p| format!(" {}", p))
            .unwrap_or_default();
        format!("Task action: {}{}\n", action.tool, params_str)
    };

    // Build source/result sections - only include if meaningful
    let source_section = if !source_data.is_empty() {
        format!("Source data:\n{}\n\n", source_data)
    } else {
        String::new()
    };

    let result_section = if action_result != source_data && !action_result.is_empty() {
        format!("Action result:\n{}\n\n", action_result)
    } else {
        String::new()
    };

    let condition_section = match condition {
        Some(c) if !c.is_empty() => format!("Condition matched: {}\n", c),
        _ => String::new(),
    };

    let system_prompt = format!(
        r#"You are generating an SMS notification for a scheduled task.

{}User timezone: {}
Current time: {}
{}
Generate a concise, informative SMS notification.
- Plain text only, no markdown or emojis
- Maximum 480 characters
- If the action involves summarizing/digesting data, provide a detailed summary grouped by platform
- If the action is a completed task (tesla, weather), briefly describe what happened
- If the action is a reminder, state the reminder clearly
- Use relative timestamps in the user's timezone (today 3pm, yesterday 8am)
- Put important items first
- Return ONLY the notification text, nothing else"#,
        action_desc, tz_str, formatted_now, condition_section
    );

    let user_content = format!("{}{}", source_section, result_section);
    let user_content = if user_content.trim().is_empty() {
        format!("Action result: {}", action_result)
    } else {
        user_content
    };

    let messages = vec![
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::system,
            content: chat_completion::Content::Text(system_prompt),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::user,
            content: chat_completion::Content::Text(user_content),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
    ];

    let request = chat_completion::ChatCompletionRequest::new(ctx.model.clone(), messages);

    match ctx.client.chat_completion(request).await {
        Ok(result) => Ok(result
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .unwrap_or_else(|| action_result.to_string())),
        Err(e) => {
            tracing::warn!("Failed to generate task notification: {}", e);
            Ok(action_result.to_string())
        }
    }
}

/// Evaluate if condition matches source data.
///
/// Binary gate: does the source data satisfy the given condition?
/// Uses ContextBuilder for user's LLM provider preference and timezone context.
async fn evaluate_condition(
    state: &Arc<AppState>,
    user_id: i32,
    condition: &str,
    source_data: &str,
) -> Result<bool, String> {
    if source_data.is_empty() {
        return Ok(false);
    }

    let ctx = ContextBuilder::for_user(state, user_id)
        .with_user_context()
        .build()
        .await
        .map_err(|e| format!("Failed to build context: {}", e))?;

    let tz_str = ctx
        .timezone
        .as_ref()
        .map(|tz| tz.tz_str.as_str())
        .unwrap_or("UTC");
    let formatted_now = ctx
        .timezone
        .as_ref()
        .map(|tz| tz.formatted_now.as_str())
        .unwrap_or("unknown");

    let system_prompt = format!(
        r#"You are evaluating if source data matches a condition.
User timezone: {}
Current time: {}

Return JSON with:
- "matches": true/false
- "reason": brief explanation (max 50 chars)

Be strict - only return true if there's a clear match."#,
        tz_str, formatted_now
    );

    let user_content = format!(
        "Condition to check: \"{}\"\n\nSource data:\n{}",
        condition, source_data
    );

    let messages = vec![
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::system,
            content: chat_completion::Content::Text(system_prompt),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::user,
            content: chat_completion::Content::Text(user_content),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
    ];

    let request = chat_completion::ChatCompletionRequest::new(ctx.model.clone(), messages);

    match ctx.client.chat_completion(request).await {
        Ok(result) => {
            let content = result
                .choices
                .first()
                .and_then(|c| c.message.content.clone())
                .unwrap_or_default();

            #[derive(Deserialize)]
            struct ConditionResult {
                matches: bool,
            }

            // Try to parse JSON from the response
            if let Ok(parsed) = serde_json::from_str::<ConditionResult>(&content) {
                return Ok(parsed.matches);
            }

            // Fallback: check if response contains "true" or positive indicators
            Ok(content.to_lowercase().contains("\"matches\": true")
                || content.to_lowercase().contains("\"matches\":true"))
        }
        Err(e) => {
            tracing::warn!("Failed to evaluate condition: {}", e);
            Ok(false) // Default to not matching on error
        }
    }
}

/// Execute a task's action with optional sources.
///
/// This function:
/// 1. Fetches sources if configured
/// 2. Evaluates condition if set
/// 3. Executes the action tool
/// 4. Generates notification message
/// 5. Sends notification
///
/// Arguments:
/// - `action_spec` - Tool call like "generate_digest" or "control_tesla(climate_on)" or empty
/// - `notification_type` - "sms" or "call"
/// - `trigger_context` - Optional context about what triggered the task
/// - `sources` - Optional comma-separated sources like "email,whatsapp"
/// - `condition` - Optional condition to evaluate against source data
///
/// Lookback hours are calculated automatically based on the last completed task
/// of the same action type for this user, capped at 24 hours.
#[allow(clippy::too_many_arguments)]
pub async fn execute_action_spec(
    state: &Arc<AppState>,
    user_id: i32,
    action_spec: &str,
    notification_type: &str,
    _trigger_context: Option<&str>, // Reserved for future use with recurring tasks
    sources: Option<&str>,
    condition: Option<&str>,
    sender_trusted: bool,
) -> ActionResult {
    tracing::debug!(
        "Executing task for user {}: action={}, sources={:?}, condition={:?}",
        user_id,
        action_spec,
        sources,
        condition
    );

    // Step 1: Fetch sources if configured
    let source_data = if let Some(src) = sources {
        if !src.is_empty() {
            // Calculate lookback dynamically: time since last completed task of same action, capped at 24h
            let lookback = calculate_lookback_hours(state, user_id, action_spec);
            tracing::debug!(
                "Using lookback of {} hours for user {} action {}",
                lookback,
                user_id,
                action_spec
            );
            match fetch_sources(state, user_id, src, lookback).await {
                Ok(data) => {
                    // Check if all sources failed (data only contains failure placeholders)
                    let all_failed = !data.is_empty()
                        && data
                            .lines()
                            .filter(|l| !l.is_empty() && !l.starts_with("---"))
                            .all(|l| l.contains("(fetch failed)"));

                    if all_failed {
                        tracing::error!("All source fetches failed for user {} task", user_id);
                        if let Err(e) = state.admin_alert_repository.create_alert(
                            "task_sources_failed",
                            "warning",
                            &format!("All sources ({}) failed for user {}", src, user_id),
                            "action_executor",
                            "execute_action_spec",
                        ) {
                            tracing::error!("Failed to create admin alert: {}", e);
                        }
                    }
                    data
                }
                Err(e) => {
                    tracing::warn!("Failed to fetch sources: {}", e);
                    String::new()
                }
            }
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    // Step 2: Evaluate condition if set
    if let Some(cond) = condition {
        if !cond.is_empty() {
            match evaluate_condition(state, user_id, cond, &source_data).await {
                Ok(matches) => {
                    if !matches {
                        tracing::debug!("Condition not met for user {}: {}", user_id, cond);
                        return ActionResult::Skipped {
                            reason: format!("Condition not met: {}", cond),
                        };
                    }
                    tracing::debug!("Condition matched for user {}: {}", user_id, cond);
                }
                Err(e) => {
                    tracing::warn!("Failed to evaluate condition, skipping action: {}", e);
                    return ActionResult::Skipped {
                        reason: format!("Condition evaluation failed: {}", e),
                    };
                }
            }
        }
    }

    // Step 3: Parse and execute action (continue even on failure)
    let structured = parse_action_structured(action_spec);
    let mut action_failed = false;
    let action_result = match execute_direct_tool(
        state,
        user_id,
        &structured,
        &source_data,
        sender_trusted,
    )
    .await
    {
        Ok(result) => result,
        Err(e) => {
            tracing::error!("Action execution failed for user {}: {}", user_id, e);
            action_failed = true;

            // Create admin alert (no sensitive content)
            if let Err(alert_err) = state.admin_alert_repository.create_alert(
                "task_action_failed",
                "warning",
                &format!(
                    "Action '{}' failed for user {}: {}",
                    action_spec, user_id, e
                ),
                "action_executor",
                "execute_action_spec",
            ) {
                tracing::error!("Failed to create admin alert: {}", alert_err);
            }

            // Continue with error message - we'll still try to notify user
            format!("Task action failed: {}", e)
        }
    };

    // Step 4: Generate notification message (unified for all action types)
    let notification_message = match generate_task_notification(
        state,
        user_id,
        &structured,
        &source_data,
        &action_result,
        condition,
    )
    .await
    {
        Ok(msg) => msg,
        Err(e) => {
            tracing::warn!("Failed to generate task notification: {}", e);
            // Fallback: use source data summary or action result
            if !source_data.is_empty() && source_data.len() > 20 {
                let preview_len = source_data.len().min(150);
                format!("Update: {}", &source_data[..preview_len])
            } else {
                action_result.clone()
            }
        }
    };

    // Step 5: Always send notification - user should know their task ran
    let noti_type = format!("task_{}", notification_type);
    let first_message = if notification_type == "call" {
        Some("Hey, here's an update from your scheduled task.".to_string())
    } else {
        None
    };

    crate::proactive::utils::send_notification(
        state,
        user_id,
        &notification_message,
        noti_type,
        first_message,
    )
    .await;

    tracing::info!(
        "Task execution completed for user {}: {}",
        user_id,
        if notification_message.len() > 100 {
            format!("{}...", &notification_message[..100])
        } else {
            notification_message.clone()
        }
    );

    if action_failed {
        ActionResult::Failed {
            error: action_result,
        }
    } else {
        ActionResult::Success {
            message: notification_message,
        }
    }
}

// Tests are in tests/action_executor_test.rs
