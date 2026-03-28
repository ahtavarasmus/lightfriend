use std::collections::HashMap;
use std::sync::Arc;

use chrono::{TimeZone, Utc};

use crate::context::ContextBuilder;
use crate::proactive::utils::send_notification;
use crate::repositories::user_core::UserCoreOps;
use crate::AppState;
use openai_api_rs::v1::{chat_completion, types};

const COOLDOWN_SECS: i32 = 1800; // 30 minutes per room

pub async fn run_system_behaviors(
    state: &Arc<AppState>,
    user_id: i32,
    entity_snapshot: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let settings = state.user_core.get_user_settings(user_id)?;
    if !settings.system_important_notify {
        return Ok(());
    }

    let sender_name = entity_snapshot
        .get("sender_name")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown");
    let platform = entity_snapshot
        .get("platform")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let content = entity_snapshot
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let room_id = entity_snapshot
        .get("room_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let person_id = entity_snapshot
        .get("person_id")
        .and_then(|v| v.as_i64())
        .map(|v| v as i32);

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i32;

    // Per-room cooldown: skip if we already notified for this room recently
    let cooldown_key = (user_id, room_id.to_string());
    if let Some(last_notify) = state.system_notify_cooldowns.get(&cooldown_key) {
        if now - *last_notify < COOLDOWN_SECS {
            return Ok(());
        }
    }

    // Compute sender signals from message history
    let signals =
        state
            .ontology_repository
            .compute_sender_signals(user_id, room_id, sender_name, now);
    let sender_context = signals.format_for_prompt(sender_name);

    // Cross-platform escalation: check if this person also messaged on other platforms recently
    let cross_platform_ctx = if let Some(pid) = person_id {
        let one_hour_ago = now - 3600;
        match state.ontology_repository.get_cross_platform_messages(
            user_id,
            pid,
            platform,
            one_hour_ago,
        ) {
            Ok(msgs) if !msgs.is_empty() => {
                let platforms: Vec<_> = msgs
                    .iter()
                    .map(|m| m.platform.as_str())
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect();
                format!(
                    "{} also messaged you on {} in the last hour.",
                    sender_name,
                    platforms.join(", ")
                )
            }
            _ => String::new(),
        }
    } else {
        String::new()
    };

    // Build context
    let ctx = ContextBuilder::for_user(state, user_id)
        .with_user_context()
        .build()
        .await?;

    // Fetch recent conversation history for this room (last 10 messages)
    let recent_messages = state
        .ontology_repository
        .get_messages_for_room(user_id, room_id, 10)
        .unwrap_or_default();

    // Get room seen timestamp for marking messages as seen/unseen
    let seen_ts = get_room_seen_ts(state, user_id, room_id, platform).await;

    // Format conversation with timestamps in user's timezone
    let tz_offset = ctx
        .timezone
        .as_ref()
        .map(|t| t.fixed_offset)
        .unwrap_or_else(|| chrono::FixedOffset::east_opt(0).unwrap());

    let fmt_ts = |unix: i32| -> String {
        Utc.timestamp_opt(unix as i64, 0)
            .single()
            .map(|dt| {
                dt.with_timezone(&tz_offset)
                    .format("%b %d %H:%M")
                    .to_string()
            })
            .unwrap_or_else(|| "??:??".to_string())
    };

    let now_formatted = fmt_ts(now);

    let conversation = if recent_messages.len() > 1 {
        // Messages come DESC, reverse for chronological order
        let mut chronological: Vec<_> = recent_messages.iter().collect();
        chronological.reverse();

        // Find the last "You" message index - everything at or before it is seen
        let last_you_idx = chronological.iter().rposition(|m| m.sender_name == "You");

        let lines: Vec<String> = chronological
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let ts = fmt_ts(m.created_at);

                // Determine seen status
                let is_seen = if m.sender_name == "You" {
                    true
                } else if let Some(you_idx) = last_you_idx {
                    i <= you_idx
                } else {
                    // No "You" messages - use bridge read receipt
                    seen_ts
                        .map(|st| (m.created_at as i64) <= st)
                        .unwrap_or(false)
                };

                let seen_marker = if is_seen { "seen" } else { "unseen" };
                let eval_marker = if i == chronological.len() - 1 {
                    " <-- evaluate this"
                } else {
                    ""
                };
                format!(
                    "[{}] [{}] {}: {}{}",
                    ts, seen_marker, m.sender_name, m.content, eval_marker
                )
            })
            .collect();

        format!(
            "Conversation on {} (latest message is being evaluated):\n{}",
            platform,
            lines.join("\n")
        )
    } else {
        format!(
            "[{}] [unseen] Message from {} on {}:\n{}",
            fmt_ts(now),
            sender_name,
            platform,
            content
        )
    };

    // Build full sender context with cross-platform signal
    let full_sender_ctx = if cross_platform_ctx.is_empty() {
        sender_context
    } else {
        format!("{}\n{}", sender_context, cross_platform_ctx)
    };

    let system_prompt = format!(
        "You are evaluating whether an incoming message requires the user's immediate attention.\n\
        The user has muted all phone notifications and relies on you to catch time-critical messages. \
        If you miss something important, they won't see it for hours.\n\
        \n\
        Current time: {}\n\
        \n\
        Sender context:\n\
        {}\n\
        \n\
        The last message in the conversation is being evaluated. Each message is marked [seen] or \
        [unseen] - unseen messages have not been read by the user yet. Use the conversation history, \
        timestamps, and seen status to understand context and urgency. Compare mentioned times against \
        the current time.\n\
        \n\
        should_notify=true only if a 2-hour delay would cause real consequences for the user. \
        Use the sender context to calibrate - who this person is to the user matters.\n\
        \n\
        If should_notify=false, set notification_message to empty string.\n\
        If should_notify=true, write a concise notification (max 480 chars, second person).",
        now_formatted, full_sender_ctx
    );

    let user_msg = conversation;

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
            content: chat_completion::Content::Text(user_msg),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
    ];

    let mut properties = HashMap::new();
    properties.insert(
        "should_notify".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Boolean),
            description: Some(
                "true if this message is important enough to notify the user immediately"
                    .to_string(),
            ),
            ..Default::default()
        }),
    );
    properties.insert(
        "notification_message".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "Concise notification text (max 480 chars, second person). Empty string if should_notify=false."
                    .to_string(),
            ),
            ..Default::default()
        }),
    );

    let tool = chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: "system_behavior_result".to_string(),
            description: Some("Return importance evaluation result".to_string()),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(vec![
                    "should_notify".to_string(),
                    "notification_message".to_string(),
                ]),
            },
        },
    };

    let request = chat_completion::ChatCompletionRequest::new(ctx.model.clone(), messages)
        .tools(vec![tool])
        .tool_choice(chat_completion::ToolChoiceType::Required)
        .temperature(0.0);

    let result = ctx
        .client
        .chat_completion(request)
        .await
        .map_err(|e| format!("System behavior LLM call failed: {}", e))?;

    crate::ai_config::log_llm_usage(
        &state.llm_usage_repository,
        user_id,
        match ctx.provider {
            crate::AiProvider::Tinfoil => "tinfoil",
            crate::AiProvider::OpenRouter => "openrouter",
        },
        &ctx.model,
        "system_important",
        &result,
    );

    let choice = result.choices.first().ok_or("No choices in LLM response")?;

    if let Some(ref tool_calls) = choice.message.tool_calls {
        for tc in tool_calls {
            let fn_name = tc.function.name.as_deref().unwrap_or("");
            if fn_name == "system_behavior_result" {
                let args = tc.function.arguments.as_deref().unwrap_or("{}");
                let parsed: serde_json::Value = serde_json::from_str(args)
                    .map_err(|e| format!("Failed to parse system_behavior_result: {}", e))?;

                let should_notify = parsed
                    .get("should_notify")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if should_notify {
                    let notification_message = parsed
                        .get("notification_message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    if !notification_message.is_empty() {
                        // Record cooldown before sending
                        state
                            .system_notify_cooldowns
                            .insert((user_id, room_id.to_string()), now);

                        send_notification(
                            state,
                            user_id,
                            notification_message,
                            "system_important".to_string(),
                            None,
                        )
                        .await;
                    }
                }

                return Ok(());
            }
        }
    }

    Ok(())
}

/// Get the room's seen-up-to timestamp from bridge read receipts.
/// Returns None for email or if the lookup fails.
async fn get_room_seen_ts(
    state: &Arc<AppState>,
    user_id: i32,
    room_id: &str,
    platform: &str,
) -> Option<i64> {
    if platform == "email" {
        return None;
    }
    let client = crate::utils::matrix_auth::get_cached_client(user_id, state)
        .await
        .ok()?;
    let matrix_room_id = matrix_sdk::ruma::OwnedRoomId::try_from(room_id).ok()?;
    let room = client.get_room(&matrix_room_id)?;
    crate::utils::bridge::get_room_seen_timestamp(&room, &client).await
}
