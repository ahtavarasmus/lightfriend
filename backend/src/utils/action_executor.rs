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

use crate::tool_call_utils::utils::create_openai_client_for_user;
use crate::AppState;
use crate::ModelPurpose;
use openai_api_rs::v1::chat_completion;
use serde::Deserialize;
use std::sync::Arc;

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

/// Fetch data from configured sources
///
/// Sources can be: email, whatsapp, telegram, signal, calendar
/// Returns formatted string with all source data
pub async fn fetch_sources(
    state: &Arc<AppState>,
    user_id: i32,
    sources: &str,
    lookback_hours: i32,
) -> Result<String, String> {
    let mut context_parts = Vec::new();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let since = now - (lookback_hours as i64 * 3600);

    for source in sources.split(',').map(|s| s.trim().to_lowercase()) {
        match source.as_str() {
            "email" => {
                match crate::handlers::imap_handlers::fetch_emails_imap(
                    state,
                    user_id,
                    true,     // preview_only
                    Some(20), // limit
                    false,    // unprocessed
                    true,     // unread_only - consistent with bridge messages for digests
                )
                .await
                {
                    Ok(emails) => {
                        // Filter by timestamp (date is DateTime<Utc>)
                        let recent: Vec<_> = emails
                            .into_iter()
                            .filter(|e| e.date.map(|d| d.timestamp() > since).unwrap_or(false))
                            .collect();
                        if !recent.is_empty() {
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
                            context_parts.push(format!(
                                "EMAIL ({} messages):\n{}",
                                recent.len(),
                                email_str
                            ));
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to fetch emails for user {}: {:?}", user_id, e);
                        context_parts.push("EMAIL: (fetch failed)".to_string());
                    }
                }
            }
            "whatsapp" | "telegram" | "signal" => {
                match crate::utils::bridge::fetch_bridge_messages(
                    &source, state, user_id, since, true, // unread_only
                )
                .await
                {
                    Ok(messages) => {
                        if !messages.is_empty() {
                            let msg_str = messages
                                .iter()
                                .take(15)
                                .map(|m| {
                                    format!(
                                        "- {} in {}: {} ({})",
                                        m.sender_display_name,
                                        m.room_name,
                                        m.content,
                                        m.formatted_timestamp
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join("\n");
                            context_parts.push(format!(
                                "{} ({} messages):\n{}",
                                source.to_uppercase(),
                                messages.len(),
                                msg_str
                            ));
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to fetch {} messages for user {}: {:?}",
                            source,
                            user_id,
                            e
                        );
                        context_parts.push(format!("{}: (fetch failed)", source.to_uppercase()));
                    }
                }
            }
            "calendar" => {
                // Calendar fetches next 24 hours, not affected by lookback
                let calendar_result =
                    crate::tool_call_utils::calendar::handle_fetch_calendar_events(
                        state, user_id, "{}",
                    )
                    .await;
                if !calendar_result.contains("No events")
                    && !calendar_result.contains("not connected")
                {
                    context_parts.push(format!("CALENDAR:\n{}", calendar_result));
                }
            }
            _ => {
                tracing::warn!("Unknown source type: {}", source);
            }
        }
    }

    if context_parts.is_empty() {
        Ok(String::new())
    } else {
        Ok(context_parts.join("\n\n---\n\n"))
    }
}

/// Parse action string into tool name and optional parameter
/// Examples:
/// - "generate_digest" -> ("generate_digest", None)
/// - "control_tesla(climate_on)" -> ("control_tesla", Some("climate_on"))
fn parse_action(action: &str) -> (String, Option<String>) {
    let action = action.trim();
    if action.is_empty() {
        return (String::new(), None);
    }

    if let Some(idx) = action.find('(') {
        if action.ends_with(')') {
            let tool = action[..idx].to_string();
            let param = action[idx + 1..action.len() - 1].to_string();
            return (tool, Some(param));
        }
    }
    (action.to_string(), None)
}

/// Execute a single tool call directly (without AI)
async fn execute_direct_tool(
    state: &Arc<AppState>,
    user_id: i32,
    tool_name: &str,
    param: Option<&str>,
    source_data: &str,
) -> Result<String, String> {
    match tool_name {
        "generate_digest" => {
            crate::tool_call_utils::management::handle_generate_digest(state, user_id, source_data)
                .await
                .map_err(|e| e.to_string())
        }
        "control_tesla" => {
            let command = param.unwrap_or("status");
            let args = serde_json::json!({ "command": command }).to_string();
            Ok(crate::tool_call_utils::tesla::handle_tesla_command(
                state, user_id, &args, true, // silent mode
            )
            .await)
        }
        "get_weather" => {
            let location = param.unwrap_or("current location");
            crate::utils::tool_exec::get_weather(state, location, "metric", "current", user_id)
                .await
                .map_err(|e| e.to_string())
        }
        "fetch_calendar_events" => Ok(
            crate::tool_call_utils::calendar::handle_fetch_calendar_events(state, user_id, "{}")
                .await,
        ),
        "" => {
            // Empty action - just return source data summary
            if source_data.is_empty() {
                Ok("No source data available.".to_string())
            } else {
                Ok(source_data.to_string())
            }
        }
        _ => Err(format!("Unknown action tool: {}", tool_name)),
    }
}

/// Generate a notification message based on task results
async fn generate_notification_message(
    state: &Arc<AppState>,
    user_id: i32,
    source_data: &str,
    condition: Option<&str>,
    action_result: &str,
) -> Result<String, String> {
    // If action result looks like a digest, use it directly
    if action_result.len() > 50
        && (action_result.contains("WhatsApp:")
            || action_result.contains("Email:")
            || action_result.contains("Telegram:"))
    {
        return Ok(action_result.to_string());
    }

    let (client, provider) = create_openai_client_for_user(state, user_id)
        .map_err(|e| format!("Failed to create AI client: {}", e))?;

    let system_prompt = r#"You are generating a brief SMS notification (max 160 chars) for the user.
Based on the task results, create a concise, friendly message.
- Be specific about what happened
- Include key details (who, what)
- Plain text only, no markdown or emojis
- Return ONLY the notification text, nothing else"#;

    let user_content = format!(
        "Task completed. Generate notification:\n\nAction result: {}\n\nSource data summary: {}\n\nCondition matched: {}",
        action_result,
        if source_data.len() > 500 { &source_data[..500] } else { source_data },
        condition.unwrap_or("none")
    );

    let messages = vec![
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::system,
            content: chat_completion::Content::Text(system_prompt.to_string()),
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

    let model = state
        .ai_config
        .model(provider, ModelPurpose::Default)
        .to_string();

    let request = chat_completion::ChatCompletionRequest::new(model, messages).max_tokens(100);

    match client.chat_completion(request).await {
        Ok(result) => Ok(result
            .choices
            .first()
            .and_then(|c| c.message.content.clone())
            .unwrap_or_else(|| action_result.to_string())),
        Err(e) => {
            tracing::warn!("Failed to generate notification: {}", e);
            Ok(action_result.to_string())
        }
    }
}

/// Evaluate if condition matches source data
async fn evaluate_condition(
    state: &Arc<AppState>,
    user_id: i32,
    condition: &str,
    source_data: &str,
) -> Result<bool, String> {
    if source_data.is_empty() {
        return Ok(false);
    }

    let (client, provider) = create_openai_client_for_user(state, user_id)
        .map_err(|e| format!("Failed to create AI client: {}", e))?;

    let system_prompt = r#"You are evaluating if source data matches a condition.
Return JSON with:
- "matches": true/false
- "reason": brief explanation (max 50 chars)

Be strict - only return true if there's a clear match."#;

    let user_content = format!(
        "Condition to check: \"{}\"\n\nSource data:\n{}",
        condition, source_data
    );

    let messages = vec![
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::system,
            content: chat_completion::Content::Text(system_prompt.to_string()),
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

    let model = state
        .ai_config
        .model(provider, ModelPurpose::Default)
        .to_string();

    let request = chat_completion::ChatCompletionRequest::new(model, messages).max_tokens(100);

    match client.chat_completion(request).await {
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
pub async fn execute_action_spec(
    state: &Arc<AppState>,
    user_id: i32,
    action_spec: &str,
    notification_type: &str,
    _trigger_context: Option<&str>, // Reserved for future use with recurring tasks
    sources: Option<&str>,
    condition: Option<&str>,
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
                    tracing::warn!("Failed to evaluate condition: {}", e);
                    // Continue anyway on evaluation error
                }
            }
        }
    }

    // Step 3: Parse and execute action (continue even on failure)
    let (tool_name, param) = parse_action(action_spec);
    let mut action_failed = false;
    let action_result =
        match execute_direct_tool(state, user_id, &tool_name, param.as_deref(), &source_data).await
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

    // Step 4: Generate notification message (with fallbacks)
    let notification_message = if tool_name == "generate_digest" && !action_failed {
        // Digest already formatted, use directly
        action_result.clone()
    } else {
        match generate_notification_message(state, user_id, &source_data, condition, &action_result)
            .await
        {
            Ok(msg) => msg,
            Err(e) => {
                tracing::warn!("Failed to generate notification message: {}", e);
                // Fallback: use source data summary or action result
                if !source_data.is_empty() && source_data.len() > 20 {
                    let preview_len = source_data.len().min(150);
                    format!("Digest: {}", &source_data[..preview_len])
                } else {
                    action_result.clone()
                }
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
