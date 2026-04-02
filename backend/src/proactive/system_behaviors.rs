use std::collections::HashMap;
use std::sync::Arc;

use chrono::{TimeZone, Utc};
use tracing::info;

use crate::context::ContextBuilder;
use crate::proactive::signal_extraction::MessageSignals;
use crate::proactive::utils::send_notification;
use crate::repositories::user_core::UserCoreOps;
use crate::repositories::user_repository::LogUsageParams;
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
    let message_id = entity_snapshot.get("message_id").and_then(|v| v.as_i64());

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

    // Build context early so we have the user's timezone for sender signals
    let ctx = ContextBuilder::for_user(state, user_id)
        .with_user_context()
        .build()
        .await?;

    let tz_offset = ctx
        .timezone
        .as_ref()
        .map(|t| t.fixed_offset)
        .unwrap_or_else(|| chrono::FixedOffset::east_opt(0).unwrap());
    let tz_offset_secs = tz_offset.local_minus_utc();

    // Compute sender signals from message history (includes temporal anomaly detection)
    let signals = state.ontology_repository.compute_sender_signals(
        user_id,
        room_id,
        sender_name,
        now,
        tz_offset_secs,
        person_id,
    );
    let sender_context = signals.format_for_prompt(sender_name);

    // Detect if user is likely sleeping based on their activity patterns
    let sleep_context = state
        .ontology_repository
        .compute_user_sleep_context(user_id, now, tz_offset_secs)
        .unwrap_or_default();

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
                    "{} also messaged you on {} in the last hour (unanswered).",
                    sender_name,
                    platforms.join(", ")
                )
            }
            _ => String::new(),
        }
    } else {
        String::new()
    };

    // Fetch recent conversation history for this room (last 10 messages)
    let recent_messages = state
        .ontology_repository
        .get_messages_for_room(user_id, room_id, 10)
        .unwrap_or_default();

    // Step 1: Extract message-level signals
    let msg_signals = MessageSignals::extract(content, &recent_messages, sender_name, now);
    let content_signals_ctx = msg_signals.format_for_prompt();

    // Get room seen timestamp for marking messages as seen/unseen
    let seen_ts = get_room_seen_ts(state, user_id, room_id, platform).await;

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

    // Detect if user has an unanswered question in this thread
    let user_waiting = detect_user_waiting(&recent_messages, sender_name);

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

    let tracking_enabled = settings.auto_track_items_system;

    // Step 3: Build structured signal report (no tracked items - those are Pass 2 only)
    let signal_report = build_signal_report(&SignalReportInput {
        sender_name,
        sender_context: &sender_context,
        cross_platform_ctx: &cross_platform_ctx,
        sleep_context: &sleep_context,
        content_signals: &content_signals_ctx,
        user_waiting: &user_waiting,
    });

    // Step 4: Multi-dimensional classification prompt
    let system_prompt = format!(
        "You are evaluating whether an incoming message requires the user's immediate attention.\n\
        The user has muted all phone notifications and relies on you to catch time-critical messages. \
        If you miss something important, they won't see it for hours.\n\
        \n\
        Current time: {}\n\
        \n\
        {}\n\
        \n\
        The last message in the conversation is being evaluated. Each message is marked [seen] or \
        [unseen] - unseen messages have not been read by the user yet. Use the conversation history, \
        timestamps, and seen status to understand context and urgency. Compare mentioned times against \
        the current time.\n\
        \n\
        Classify the urgency level:\n\
        - critical: immediate danger, medical emergency, security breach\n\
        - high: 2-hour delay would cause real consequences (missed meeting, financial loss, time-sensitive decision)\n\
        - medium: important but can wait a few hours (friend asking to meet later today, non-urgent work question)\n\
        - low: routine updates, casual conversation\n\
        - none: spam, automated messages, irrelevant\n\
        \n\
        Set should_notify=true only for critical or high urgency.\n\
        Use the signal report to calibrate - sender relationship, timing patterns, and content signals all matter.\n\
        \n\
        If should_notify=false, set notification_message to empty string.\n\
        If should_notify=true, write a concise notification (max 480 chars, second person).{}",
        now_formatted, signal_report,
        if tracking_enabled {
            "\n\n\
            COMMITMENT TRACKING:\n\
            Also detect if this message contains a concrete commitment or obligation that the user \
            could forget and would benefit from being reminded about. Examples: paying a bill, booking \
            something, confirming attendance, sending a document, following up by a date.\n\
            - Set contains_commitment=true only for specific, actionable obligations with a timeframe.\n\
            - Do NOT track vague intentions (\"we should hang out\") or past-tense actions already done.\n\
            - Detect commitments both TO the user (\"I'll send you the invoice\") and BY the user (\"I'll call you back\").\n\
            - If a tracked item already exists for this commitment (see signal report), set existing_event_id instead of creating a duplicate.\n\
            - If the message updates a tracked item's deadline or status, set existing_event_id with the new details."
        } else {
            ""
        }
    );

    let user_msg = conversation.clone();

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

    // Step 4: Multi-dimensional tool schema
    let mut properties = HashMap::new();
    properties.insert(
        "urgency".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Urgency level: critical, high, medium, low, or none".to_string()),
            enum_values: Some(vec![
                "critical".to_string(),
                "high".to_string(),
                "medium".to_string(),
                "low".to_string(),
                "none".to_string(),
            ]),
            ..Default::default()
        }),
    );
    properties.insert(
        "category".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "Message category: emergency, financial, health, relationship, work, logistics, social, or spam"
                    .to_string(),
            ),
            enum_values: Some(vec![
                "emergency".to_string(),
                "financial".to_string(),
                "health".to_string(),
                "relationship".to_string(),
                "work".to_string(),
                "logistics".to_string(),
                "social".to_string(),
                "spam".to_string(),
            ]),
            ..Default::default()
        }),
    );
    properties.insert(
        "should_notify".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Boolean),
            description: Some("true only for critical or high urgency messages".to_string()),
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
    properties.insert(
        "summary".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "One-line summary of the message for digest delivery (max 100 chars). Always fill this."
                    .to_string(),
            ),
            ..Default::default()
        }),
    );

    // Pass 1 only classifies commitment type - extraction happens in Pass 2
    if tracking_enabled {
        properties.insert(
            "message_type".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some(
                    "Commitment type: commitment_to_user (someone promised user something), \
                     commitment_by_user (user promised to do something), \
                     deadline_update (changes a tracked item's deadline), \
                     completion_signal (indicates a tracked item was completed), \
                     none (no commitment detected)"
                        .to_string(),
                ),
                enum_values: Some(vec![
                    "commitment_to_user".to_string(),
                    "commitment_by_user".to_string(),
                    "deadline_update".to_string(),
                    "completion_signal".to_string(),
                    "none".to_string(),
                ]),
                ..Default::default()
            }),
        );
    }

    let tool = chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: "system_behavior_result".to_string(),
            description: Some(
                "Return message importance classification with urgency, category, and notification decision"
                    .to_string(),
            ),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(vec![
                    "urgency".to_string(),
                    "category".to_string(),
                    "should_notify".to_string(),
                    "notification_message".to_string(),
                    "summary".to_string(),
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

                let urgency = parsed
                    .get("urgency")
                    .and_then(|v| v.as_str())
                    .unwrap_or("none");
                let category = parsed
                    .get("category")
                    .and_then(|v| v.as_str())
                    .unwrap_or("social");
                let should_notify = parsed
                    .get("should_notify")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let summary = parsed.get("summary").and_then(|v| v.as_str()).unwrap_or("");

                info!(
                    "System behavior classification for user {}: urgency={}, category={}, notify={}",
                    user_id, urgency, category, should_notify
                );

                // Store classification + full prompt/result on the message
                if let Some(mid) = message_id {
                    if let Err(e) = state.ontology_repository.update_message_classification(
                        mid,
                        urgency,
                        category,
                        if summary.is_empty() {
                            None
                        } else {
                            Some(summary)
                        },
                        Some(&signal_report),
                        Some(args),
                    ) {
                        tracing::warn!("Failed to store classification for message {}: {}", mid, e);
                    }
                }

                // Pass 2: Commitment extraction (separate focused LLM call)
                if tracking_enabled {
                    let message_type = parsed
                        .get("message_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("none");

                    if message_type != "none" {
                        let auto_confirm = settings.auto_confirm_tracked_items;
                        // Fetch active events only when Pass 2 needs them
                        let active_events = state
                            .ontology_repository
                            .get_active_and_proposed_events(user_id)
                            .unwrap_or_default();
                        run_commitment_pass2(
                            state,
                            &ctx,
                            user_id,
                            message_id,
                            message_type,
                            &conversation,
                            sender_name,
                            &active_events,
                            &now_formatted,
                            now,
                            auto_confirm,
                        )
                        .await;
                    }
                }

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

                        // Use urgency to determine notification method
                        let content_type = match urgency {
                            "critical" => "critical".to_string(),
                            _ => "system_important".to_string(),
                        };

                        send_notification(state, user_id, notification_message, content_type, None)
                            .await;
                    }
                } else {
                    let _ = state.user_repository.log_usage(LogUsageParams {
                        user_id,
                        sid: None,
                        activity_type: "system_screened".to_string(),
                        credits: None,
                        time_consumed: None,
                        success: Some(true),
                        reason: Some(format!("{} on {} - not urgent", sender_name, platform)),
                        status: None,
                        recharge_threshold_timestamp: None,
                        zero_credits_timestamp: None,
                    });
                }

                return Ok(());
            }
        }
    }

    Ok(())
}

struct SignalReportInput<'a> {
    sender_name: &'a str,
    sender_context: &'a str,
    cross_platform_ctx: &'a str,
    sleep_context: &'a str,
    content_signals: &'a str,
    user_waiting: &'a str,
}

/// Build a structured signal report combining all intelligence sources.
fn build_signal_report(input: &SignalReportInput) -> String {
    let mut sections = Vec::new();

    sections.push(format!("SIGNAL REPORT\n\nSender: {}", input.sender_name));
    sections.push(format!("Relationship: {}", input.sender_context));

    if !input.cross_platform_ctx.is_empty() {
        sections.push(format!("Cross-platform: {}", input.cross_platform_ctx));
    }

    if !input.sleep_context.is_empty() {
        sections.push(format!("User state: {}", input.sleep_context));
    }

    sections.push(format!("Content signals: {}", input.content_signals));

    if !input.user_waiting.is_empty() {
        sections.push(format!("Thread context: {}", input.user_waiting));
    }

    sections.join("\n")
}

/// Detect if the user sent a message that hasn't been replied to yet.
fn detect_user_waiting(
    recent_messages: &[crate::models::ontology_models::OntMessage],
    sender_name: &str,
) -> String {
    if recent_messages.is_empty() {
        return String::new();
    }

    // Messages are DESC - find if user sent something and sender hasn't replied until now
    // The current message (index 0) is the sender's new message, so skip it
    let mut found_user_msg = false;
    let mut user_asked_question = false;

    for msg in recent_messages.iter().skip(1) {
        if msg.sender_name == "You" {
            found_user_msg = true;
            if msg.content.contains('?') {
                user_asked_question = true;
            }
            break;
        }
        if msg.sender_name == sender_name {
            // Sender already sent another message before this - user wasn't waiting
            break;
        }
    }

    if user_asked_question {
        "This is a response to a question you asked - likely important to you.".to_string()
    } else if found_user_msg {
        "This is a response to your earlier message in this conversation.".to_string()
    } else {
        String::new()
    }
}

/// Pass 2: Focused commitment extraction with a separate LLM call.
/// Only runs when Pass 1 detected a commitment-related message_type.
#[allow(clippy::too_many_arguments)]
async fn run_commitment_pass2(
    state: &Arc<AppState>,
    ctx: &crate::context::AgentContext,
    user_id: i32,
    message_id: Option<i64>,
    message_type: &str,
    conversation: &str,
    sender_name: &str,
    active_events: &[crate::models::ontology_models::OntEvent],
    now_formatted: &str,
    now: i32,
    auto_confirm: bool,
) {
    // For completion signals, handle directly without Pass 2 LLM call
    if message_type == "completion_signal" && !active_events.is_empty() {
        // Simple heuristic: find the most recently discussed event
        // The chat LLM handles explicit "I paid invoice #42" via update_event tool
        // This catches implicit completions like "done" or "sent it"
        return;
    }

    // Build candidate list for dedup, sorted by text similarity to the message
    let conv_lower = conversation.to_lowercase();
    let mut scored: Vec<(f64, &crate::models::ontology_models::OntEvent)> = active_events
        .iter()
        .map(|e| {
            let score = strsim::jaro_winkler(&conv_lower, &e.description.to_lowercase());
            (score, e)
        })
        .collect();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let candidates: Vec<String> = scored
        .iter()
        .map(|(_, e)| format!("[id={}] {}", e.id, e.description))
        .collect();
    let candidates_text = if candidates.is_empty() {
        "No existing tracked items.".to_string()
    } else {
        format!("Existing tracked items:\n{}", candidates.join("\n"))
    };

    let system_prompt = format!(
        "Extract the commitment or obligation from this message.\n\
        Current time: {}\n\
        Sender: {}\n\
        Message type: {}\n\
        \n\
        {}\n\
        \n\
        Rules:\n\
        - description: short actionable text (e.g. \"Pay electricity bill\", \"Send project files to Jake\")\n\
        - deadline: RFC 3339 datetime if mentioned or implied. Null if no deadline.\n\
        - who: \"sender\" if someone promised the user something, \"user\" if user committed to do something\n\
        - confidence: \"high\" if explicit commitment with clear action, \"medium\" if implied, \"low\" if ambiguous\n\
        - existing_match_id: set to the ID if this updates an existing tracked item, null if new",
        now_formatted, sender_name, message_type, candidates_text
    );

    let mut properties = HashMap::new();
    properties.insert(
        "description".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Short description of the obligation".to_string()),
            ..Default::default()
        }),
    );
    properties.insert(
        "deadline".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "RFC 3339 datetime for the deadline (e.g. 2026-04-04T17:00:00). Null if no deadline."
                    .to_string(),
            ),
            ..Default::default()
        }),
    );
    properties.insert(
        "who".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Who made the commitment".to_string()),
            enum_values: Some(vec!["sender".to_string(), "user".to_string()]),
            ..Default::default()
        }),
    );
    properties.insert(
        "confidence".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Confidence level".to_string()),
            enum_values: Some(vec![
                "high".to_string(),
                "medium".to_string(),
                "low".to_string(),
            ]),
            ..Default::default()
        }),
    );
    properties.insert(
        "existing_match_id".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Number),
            description: Some(
                "ID of existing tracked item this updates. Null if new commitment.".to_string(),
            ),
            ..Default::default()
        }),
    );

    let tool = chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: "commitment_extraction".to_string(),
            description: Some("Extract structured commitment details".to_string()),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(vec![
                    "description".to_string(),
                    "who".to_string(),
                    "confidence".to_string(),
                ]),
            },
        },
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
            content: chat_completion::Content::Text(conversation.to_string()),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
    ];

    let request = chat_completion::ChatCompletionRequest::new(ctx.model.clone(), messages)
        .tools(vec![tool])
        .tool_choice(chat_completion::ToolChoiceType::Required)
        .temperature(0.0);

    let result = match ctx.client.chat_completion(request).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Pass 2 commitment extraction failed: {}", e);
            return;
        }
    };

    crate::ai_config::log_llm_usage(
        &state.llm_usage_repository,
        user_id,
        match ctx.provider {
            crate::AiProvider::Tinfoil => "tinfoil",
            crate::AiProvider::OpenRouter => "openrouter",
        },
        &ctx.model,
        "commitment_extraction",
        &result,
    );

    let choice = match result.choices.first() {
        Some(c) => c,
        None => return,
    };

    if let Some(ref tool_calls) = choice.message.tool_calls {
        for tc in tool_calls {
            if tc.function.name.as_deref() != Some("commitment_extraction") {
                continue;
            }
            let args = tc.function.arguments.as_deref().unwrap_or("{}");
            let parsed: serde_json::Value = match serde_json::from_str(args) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let description = parsed
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let deadline_str = parsed.get("deadline").and_then(|v| v.as_str());
            let existing_match_id = parsed
                .get("existing_match_id")
                .and_then(|v| v.as_i64())
                .map(|v| v as i32);

            if description.is_empty() {
                continue;
            }

            // Parse RFC 3339 deadline
            let deadline_ts = deadline_str.and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(s)
                    .ok()
                    .map(|dt| dt.timestamp() as i32)
            });
            let due_at = deadline_ts;
            let remind_at = deadline_ts.map(|d| (d - 86400).max(now)); // remind 1 day before, but not in the past

            if let Some(event_id) = existing_match_id {
                // Update existing event
                let old_event = state.ontology_repository.get_event(user_id, event_id).ok();

                if let Err(e) = state.ontology_repository.update_event(
                    user_id,
                    event_id,
                    Some(&format!("Update: {}", description)),
                    None,
                    remind_at,
                    due_at,
                ) {
                    tracing::warn!("Failed to update tracked event {}: {}", event_id, e);
                } else {
                    // Link update message to event
                    if let Some(mid) = message_id {
                        let _ = state.ontology_repository.create_link(
                            user_id,
                            "Event",
                            event_id,
                            "Message",
                            mid as i32,
                            "update_message",
                            None,
                        );
                    }
                    // Notify if deadline changed
                    if let Some(old) = old_event {
                        let old_due = old.due_at.unwrap_or(0);
                        if let Some(new_due) = due_at {
                            if old_due != new_due && old_due != 0 {
                                let change_msg = format!(
                                    "Tracked item updated: \"{}\"\nDeadline changed based on new message from {}.",
                                    old.description, sender_name
                                );
                                crate::proactive::utils::send_notification(
                                    state,
                                    user_id,
                                    &change_msg,
                                    "tracked_item_update".to_string(),
                                    None,
                                )
                                .await;
                            }
                        }
                    }
                }
            } else {
                // Create new event
                let status = if auto_confirm { "active" } else { "proposed" };
                let new_event = crate::models::ontology_models::NewOntEvent {
                    user_id,
                    description: description.clone(),
                    remind_at,
                    due_at,
                    status: status.to_string(),
                    created_at: now,
                    updated_at: now,
                };
                match state.ontology_repository.create_event(&new_event) {
                    Ok(created) => {
                        if let Some(mid) = message_id {
                            let _ = state.ontology_repository.create_link(
                                user_id,
                                "Event",
                                created.id,
                                "Message",
                                mid as i32,
                                "source_message",
                                None,
                            );
                        }
                        info!(
                            "Auto-created tracked event {} ({}) for user {}: {}",
                            created.id, status, user_id, created.description
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Failed to create tracked event for user {}: {}",
                            user_id,
                            e
                        );
                    }
                }
            }
        }
    }
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
