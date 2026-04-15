use std::collections::HashMap;
use std::sync::Arc;

use chrono::{TimeZone, Utc};
use tracing::info;

use crate::context::ContextBuilder;
use crate::models::ontology_models::OntMessage;
use crate::pg_models::PgMessageHistory;
use crate::proactive::signal_extraction::MessageSignals;
use crate::proactive::utils::send_notification;
use crate::repositories::ontology_repository::RecentHighSenderMessagesQuery;
use crate::repositories::user_core::UserCoreOps;
use crate::repositories::user_repository::LogUsageParams;
use crate::AppState;
use openai_api_rs::v1::{chat_completion, types};

const COOLDOWN_SECS: i32 = 3600; // 1 hour per room
const NOTIFICATION_CONTEXT_WINDOW_SECS: i32 = 3 * 3600;

/// Daily token budget per user for proactive AI processing (system_important, rules, etc.)
/// 5M tokens/month target (~$7.50/user/month at $1.50/M input tokens).
/// 5M / 30 days = ~166K tokens/day. Normal users use 5-20K/day.
const DAILY_TOKEN_BUDGET_PER_USER: i64 = 166_000;

/// Strip URLs and trailing connector phrases from a notification string.
/// SMS chars are expensive and tracking links balloon length, so we drop
/// any whitespace-delimited token starting with http:// or https:// even
/// if the LLM ignored the prompt.
fn strip_urls(s: &str) -> String {
    let cleaned: String = s
        .split_whitespace()
        .filter(|tok| {
            let lower = tok.to_lowercase();
            !(lower.starts_with("http://") || lower.starts_with("https://"))
        })
        .collect::<Vec<_>>()
        .join(" ");
    let trimmed = cleaned
        .trim_end_matches(|c: char| c.is_whitespace() || c == ',' || c == ':' || c == '.')
        .trim_end();
    let lower = trimmed.to_lowercase();
    let connectors = [
        " recharge now at",
        " sign in at",
        " log in at",
        " click here",
        " click here:",
        " visit",
        " here",
        " click",
        " via",
        " at",
    ];
    let mut out = trimmed.to_string();
    for c in &connectors {
        if lower.ends_with(c) {
            let cut = out.len() - c.len();
            out.truncate(cut);
            out = out.trim_end().to_string();
            break;
        }
    }
    out
}

/// Classify urgency of an incoming message and notify user if needed.
/// Runs after a delay, only for unseen messages.
pub async fn run_urgency_classification(
    state: &Arc<AppState>,
    user_id: i32,
    entity_snapshot: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let settings = state.user_core.get_user_settings(user_id)?;
    if !settings.system_important_notify {
        return Ok(());
    }

    if exceeds_daily_token_budget(state, user_id, "urgency_classification") {
        return Ok(());
    }

    let sender_name = entity_snapshot
        .get("sender_name")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown");
    let sender_key = entity_snapshot.get("sender_key").and_then(|v| v.as_str());
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

    // Compute user baseline response time (90-day window, geometric mean)
    let baseline = state
        .ontology_repository
        .compute_user_baseline(user_id, now);

    // Compute sender signals with Bayesian response time blended against baseline
    let signals = state.ontology_repository.compute_sender_signals(
        user_id,
        room_id,
        sender_name,
        now,
        tz_offset_secs,
        person_id,
        &baseline,
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

    let recent_high_alerts = state
        .ontology_repository
        .get_recent_high_messages_by_sender(RecentHighSenderMessagesQuery {
            user_id,
            platform,
            sender_name,
            sender_key,
            since_ts: now - NOTIFICATION_CONTEXT_WINDOW_SECS,
            exclude_message_id: message_id,
            limit: 5,
        })
        .unwrap_or_default();
    let recent_lightfriend_replies = state
        .user_repository
        .get_conversation_history(user_id, 3, false)
        .unwrap_or_default();
    let recent_notification_context = build_recent_notification_context(
        &recent_high_alerts,
        &recent_lightfriend_replies,
        now,
        tz_offset,
    );

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

                // Determine seen status: seen_at is set by read receipts and user replies
                let is_seen = if m.sender_name == "You" || m.seen_at.is_some() {
                    true
                } else if let Some(you_idx) = last_you_idx {
                    i <= you_idx
                } else {
                    false
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

    // Build structured signal report
    let signal_report = build_signal_report(&SignalReportInput {
        sender_name,
        sender_context: &sender_context,
        cross_platform_ctx: &cross_platform_ctx,
        sleep_context: &sleep_context,
        content_signals: &content_signals_ctx,
        user_waiting: &user_waiting,
        recent_notification_context: &recent_notification_context,
    });

    // Urgency classification prompt - provides rich context, lets the AI decide
    let system_prompt = format!(
        "You are a message triage system. The user has muted all phone notifications and relies \
        on you to catch messages that need their attention. If you miss something important, \
        they won't see it for hours.\n\
        \n\
        Current time: {}\n\
        \n\
        {}\n\
        \n\
        The last message in the conversation is being evaluated. Each message is marked [seen] or \
        [unseen]. Use the full conversation history, timestamps, and seen status to understand context.\n\
        \n\
        Avoid notification spam. If the user was already alerted about the same sender and same \
        apparent incident/session recently, classify follow-up confirmations, receipts, and status \
        updates as medium or low unless the new message adds materially new risk or needs a new \
        immediate action. Different amounts or repeated receipts from the same automated sender can \
        still be the same session. If the user recently replied to Lightfriend with something like \
        \"it was me\", \"on it\", \"handled\", or similar, treat same-sender follow-ups as already \
        acknowledged unless this is clearly a new situation.\n\
        \n\
        Classify the urgency:\n\
        - high: delay would cause real consequences\n\
        - medium: important but can wait hours\n\
        - low: routine, casual, or spam",
        now_formatted, signal_report
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
            content: chat_completion::Content::Text(conversation.clone()),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
    ];

    let mut properties = HashMap::new();
    properties.insert(
        "urgency".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Urgency level".to_string()),
            enum_values: Some(vec![
                "high".to_string(),
                "medium".to_string(),
                "low".to_string(),
            ]),
            ..Default::default()
        }),
    );
    properties.insert(
        "category".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Message category".to_string()),
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
        "summary".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "Concise summary of the message (under 160 chars, no URLs). Don't restate sender name. \
                 Used for SMS notification if urgent, digest teaser otherwise."
                    .to_string(),
            ),
            ..Default::default()
        }),
    );

    let tool = chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: "urgency_result".to_string(),
            description: Some("Return urgency classification".to_string()),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(vec![
                    "urgency".to_string(),
                    "category".to_string(),
                    "summary".to_string(),
                ]),
            },
        },
    };

    let classification_model = state
        .ai_config
        .model(ctx.provider, crate::ModelPurpose::Voice)
        .to_string();

    let request =
        chat_completion::ChatCompletionRequest::new(classification_model.clone(), messages)
            .tools(vec![tool])
            .tool_choice(chat_completion::ToolChoiceType::Required)
            .temperature(0.0);

    let result = state
        .ai_config
        .chat_completion(ctx.provider, &request)
        .await
        .map_err(|e| format!("Urgency classification LLM call failed: {}", e))?;

    crate::ai_config::log_llm_usage(
        &state.llm_usage_repository,
        user_id,
        match ctx.provider {
            crate::AiProvider::Tinfoil => "tinfoil",
            crate::AiProvider::OpenRouter => "openrouter",
        },
        &classification_model,
        "urgency_classification",
        &result,
    );

    let choice = result.choices.first().ok_or("No choices in LLM response")?;

    if let Some(ref tool_calls) = choice.message.tool_calls {
        for tc in tool_calls {
            if tc.function.name.as_deref() != Some("urgency_result") {
                continue;
            }
            let args = tc.function.arguments.as_deref().unwrap_or("{}");
            let parsed: serde_json::Value = serde_json::from_str(args)
                .map_err(|e| format!("Failed to parse urgency_result: {}", e))?;

            let urgency = parsed
                .get("urgency")
                .and_then(|v| v.as_str())
                .unwrap_or("low");
            let category = parsed
                .get("category")
                .and_then(|v| v.as_str())
                .unwrap_or("social");
            let summary = parsed.get("summary").and_then(|v| v.as_str()).unwrap_or("");

            // high = notify, unless it's our own outgoing message
            let should_notify = urgency == "high" && sender_name != "You";

            info!(
                "Urgency classification for user {}: urgency={}, category={}, notify={}",
                user_id, urgency, category, should_notify
            );

            // Store classification on the message
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

            if should_notify {
                let cleaned = strip_urls(summary);
                let notification_message = cleaned.trim();

                if !notification_message.is_empty() {
                    state
                        .system_notify_cooldowns
                        .insert((user_id, room_id.to_string()), now);

                    // Route: known contact + not email = call + SMS, otherwise just SMS
                    let content_type = if person_id.is_some() && platform != "email" {
                        "system_important_call".to_string()
                    } else {
                        "system_important".to_string()
                    };

                    send_notification(state, user_id, notification_message, content_type, None)
                        .await;
                }
            } else if sender_name != "You" {
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

    Ok(())
}

/// Check if user has exceeded daily token budget. Returns true if exceeded.
fn exceeds_daily_token_budget(state: &Arc<AppState>, user_id: i32, label: &str) -> bool {
    let day_start = {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;
        (now / 86400) * 86400
    };
    let used_today = state
        .llm_usage_repository
        .get_user_tokens_since(user_id, day_start)
        .unwrap_or(0);
    if used_today >= DAILY_TOKEN_BUDGET_PER_USER {
        tracing::warn!(
            "User {} exceeded daily token budget ({}/{}), skipping {}",
            user_id,
            used_today,
            DAILY_TOKEN_BUDGET_PER_USER,
            label
        );
        return true;
    }
    false
}

struct SignalReportInput<'a> {
    sender_name: &'a str,
    sender_context: &'a str,
    cross_platform_ctx: &'a str,
    sleep_context: &'a str,
    content_signals: &'a str,
    user_waiting: &'a str,
    recent_notification_context: &'a str,
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

    if !input.recent_notification_context.is_empty() {
        sections.push(input.recent_notification_context.to_string());
    }

    sections.join("\n")
}

fn build_recent_notification_context(
    recent_high_alerts: &[OntMessage],
    recent_lightfriend_replies: &[PgMessageHistory],
    now: i32,
    tz_offset: chrono::FixedOffset,
) -> String {
    let mut sections = Vec::new();

    if !recent_high_alerts.is_empty() {
        let lines: Vec<String> = recent_high_alerts
            .iter()
            .map(|m| {
                let ago = format_age(now - m.created_at);
                let summary = m
                    .summary
                    .as_deref()
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or(&m.content);
                format!(
                    "- {} ago: urgency={}, category={}, summary={}",
                    ago,
                    m.urgency.as_deref().unwrap_or("high"),
                    m.category.as_deref().unwrap_or("unknown"),
                    concise_for_prompt(summary, 180)
                )
            })
            .collect();
        sections.push(format!(
            "Recent same-sender alerts likely already sent to the user:\n{}",
            lines.join("\n")
        ));
    }

    let cutoff = now - NOTIFICATION_CONTEXT_WINDOW_SECS;
    let reply_lines: Vec<String> = recent_lightfriend_replies
        .iter()
        .filter(|m| m.created_at >= cutoff)
        .filter(|m| m.role == "user" || m.role == "assistant")
        .take(8)
        .map(|m| {
            let ts = Utc
                .timestamp_opt(m.created_at as i64, 0)
                .single()
                .map(|dt| {
                    dt.with_timezone(&tz_offset)
                        .format("%b %d %H:%M")
                        .to_string()
                })
                .unwrap_or_else(|| "??:??".to_string());
            format!(
                "- [{}] {}: {}",
                ts,
                m.role,
                concise_for_prompt(&m.encrypted_content, 160)
            )
        })
        .collect();

    if !reply_lines.is_empty() {
        sections.push(format!(
            "Recent Lightfriend SMS/chat context:\n{}",
            reply_lines.join("\n")
        ));
    }

    sections.join("\n")
}

fn format_age(seconds: i32) -> String {
    if seconds < 90 {
        "just now".to_string()
    } else if seconds < 3600 {
        format!("{} min", seconds / 60)
    } else {
        let hours = seconds / 3600;
        let mins = (seconds % 3600) / 60;
        if mins == 0 {
            format!("{}h", hours)
        } else {
            format!("{}h {}m", hours, mins)
        }
    }
}

fn concise_for_prompt(s: &str, max_chars: usize) -> String {
    let collapsed = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= max_chars {
        return collapsed;
    }

    let mut out = collapsed
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>();
    out.push_str("...");
    out
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

/// Detect and extract commitments from an incoming message.
/// Runs immediately on every message (no delay, no seen-check).
/// Independent of urgency classification.
pub async fn run_commitment_detection(
    state: &Arc<AppState>,
    user_id: i32,
    entity_snapshot: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let settings = state.user_core.get_user_settings(user_id)?;
    if !settings.auto_track_items_system {
        return Ok(());
    }

    if exceeds_daily_token_budget(state, user_id, "commitment_detection") {
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
    let room_id = entity_snapshot
        .get("room_id")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let message_id = entity_snapshot.get("message_id").and_then(|v| v.as_i64());

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i32;

    let ctx = ContextBuilder::for_user(state, user_id).build().await?;

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

    // Build conversation context from the chatroom
    let recent_messages = state
        .ontology_repository
        .get_messages_for_room(user_id, room_id, 10)
        .unwrap_or_default();

    let conversation = if recent_messages.len() > 1 {
        let mut chronological: Vec<_> = recent_messages.iter().collect();
        chronological.reverse();
        let lines: Vec<String> = chronological
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let ts = fmt_ts(m.created_at);
                let eval_marker = if i == chronological.len() - 1 {
                    " <-- evaluate this"
                } else {
                    ""
                };
                format!("[{}] {}: {}{}", ts, m.sender_name, m.content, eval_marker)
            })
            .collect();
        format!(
            "Conversation on {} (latest message is being evaluated):\n{}",
            platform,
            lines.join("\n")
        )
    } else {
        format!(
            "[{}] Message from {} on {}:\n{}",
            fmt_ts(now),
            sender_name,
            platform,
            entity_snapshot
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
        )
    };

    // Fetch existing events for dedup, sorted by similarity to conversation
    let active_events = state
        .ontology_repository
        .get_active_and_proposed_events(user_id)
        .unwrap_or_default();

    let conv_lower = conversation.to_lowercase();
    let mut scored: Vec<(f64, &crate::models::ontology_models::OntEvent)> = active_events
        .iter()
        .map(|e| {
            let score = strsim::jaro_winkler(&conv_lower, &e.description.to_lowercase());
            (score, e)
        })
        .collect();
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let events_context = if active_events.is_empty() {
        "No existing tracked items.".to_string()
    } else {
        let items: Vec<String> = scored
            .iter()
            .map(|(_, e)| format!("[id={}] {}", e.id, e.description))
            .collect();
        format!("Existing tracked items:\n{}", items.join("\n"))
    };

    // Single LLM call: detect + extract in one shot
    let system_prompt = format!(
        "You are analyzing a message for commitments or obligations.\n\
        Current time: {}\n\
        Sender: {}\n\
        \n\
        {}\n\
        \n\
        Detect if the latest message contains a concrete commitment, obligation, or completion signal.\n\
        - Commitments TO the user: someone promised to do something (send invoice, call back, deliver)\n\
        - Commitments BY the user: user promised to do something (pay, book, confirm, follow up)\n\
        - Completion signals: past-tense indication that a tracked item is done\n\
        - Deadline updates: changes to an existing tracked item's timeline\n\
        \n\
        Only track specific, actionable obligations the user could forget. \
        Not vague intentions or past-tense actions already completed.",
        now_formatted, sender_name, events_context
    );

    let mut properties = HashMap::new();
    properties.insert(
        "has_commitment".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Boolean),
            description: Some(
                "Whether a commitment, obligation, or completion signal was detected".to_string(),
            ),
            ..Default::default()
        }),
    );
    properties.insert(
        "commitment_type".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Type of commitment detected".to_string()),
            enum_values: Some(vec![
                "commitment_to_user".to_string(),
                "commitment_by_user".to_string(),
                "completion_signal".to_string(),
                "deadline_update".to_string(),
            ]),
            ..Default::default()
        }),
    );
    properties.insert(
        "description".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Short actionable description (e.g. 'Pay electricity bill', 'Send project files to Jake')".to_string()),
            ..Default::default()
        }),
    );
    properties.insert(
        "deadline".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "RFC 3339 datetime for the deadline if mentioned or implied. Null if none."
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
            description: Some(
                "How confident: high = explicit commitment, medium = implied, low = ambiguous"
                    .to_string(),
            ),
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
                "ID of existing tracked item this updates or completes. Null if new.".to_string(),
            ),
            ..Default::default()
        }),
    );

    let tool = chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: "commitment_result".to_string(),
            description: Some("Return commitment detection and extraction result".to_string()),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(vec!["has_commitment".to_string()]),
            },
        },
    };

    let classification_model = state
        .ai_config
        .model(ctx.provider, crate::ModelPurpose::Voice)
        .to_string();

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
            content: chat_completion::Content::Text(conversation),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
    ];

    let request =
        chat_completion::ChatCompletionRequest::new(classification_model.clone(), messages)
            .tools(vec![tool])
            .tool_choice(chat_completion::ToolChoiceType::Required)
            .temperature(0.0);

    let result = state
        .ai_config
        .chat_completion(ctx.provider, &request)
        .await
        .map_err(|e| format!("Commitment detection LLM call failed: {}", e))?;

    crate::ai_config::log_llm_usage(
        &state.llm_usage_repository,
        user_id,
        match ctx.provider {
            crate::AiProvider::Tinfoil => "tinfoil",
            crate::AiProvider::OpenRouter => "openrouter",
        },
        &classification_model,
        "commitment_detection",
        &result,
    );

    let choice = match result.choices.first() {
        Some(c) => c,
        None => return Ok(()),
    };

    if let Some(ref tool_calls) = choice.message.tool_calls {
        for tc in tool_calls {
            if tc.function.name.as_deref() != Some("commitment_result") {
                continue;
            }
            let args = tc.function.arguments.as_deref().unwrap_or("{}");
            let parsed: serde_json::Value = match serde_json::from_str(args) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let has_commitment = parsed
                .get("has_commitment")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if !has_commitment {
                return Ok(());
            }

            let commitment_type = parsed
                .get("commitment_type")
                .and_then(|v| v.as_str())
                .unwrap_or("commitment_by_user");
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

            if description.is_empty() && commitment_type != "completion_signal" {
                continue;
            }

            let deadline_ts = deadline_str.and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(s)
                    .ok()
                    .map(|dt| dt.timestamp() as i32)
            });
            let due_at = deadline_ts;
            let remind_at = deadline_ts.map(|d| (d - 86400).max(now));

            let valid_ids: std::collections::HashSet<i32> =
                active_events.iter().map(|e| e.id).collect();

            match commitment_type {
                "completion_signal" => {
                    // Mark matching event as completed
                    if let Some(event_id) = existing_match_id {
                        if valid_ids.contains(&event_id) {
                            if let Err(e) = state.ontology_repository.update_event_status(
                                user_id,
                                event_id,
                                "completed",
                            ) {
                                tracing::warn!(
                                    "Failed to mark event {} completed: {}",
                                    event_id,
                                    e
                                );
                            } else {
                                if let Some(mid) = message_id {
                                    let _ = state.ontology_repository.create_link(
                                        user_id,
                                        "Event",
                                        event_id,
                                        "Message",
                                        mid as i32,
                                        "resolve_message",
                                        None,
                                    );
                                }
                                let desc = active_events
                                    .iter()
                                    .find(|e| e.id == event_id)
                                    .map(|e| e.description.as_str())
                                    .unwrap_or("unknown");
                                info!(
                                    "Auto-completed event {} for user {}: {}",
                                    event_id, user_id, desc
                                );
                            }
                        }
                    }
                }
                "deadline_update" => {
                    if let Some(event_id) = existing_match_id {
                        if valid_ids.contains(&event_id) {
                            let old_event =
                                state.ontology_repository.get_event(user_id, event_id).ok();
                            let update_desc = if description.is_empty() {
                                None
                            } else {
                                Some(format!("Update: {}", description))
                            };
                            if let Err(e) = state.ontology_repository.update_event(
                                user_id,
                                event_id,
                                update_desc.as_deref(),
                                None,
                                remind_at,
                                due_at,
                            ) {
                                tracing::warn!("Failed to update event {}: {}", event_id, e);
                            } else {
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
                                            send_notification(
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
                        }
                    }
                }
                _ => {
                    // commitment_to_user or commitment_by_user
                    if let Some(event_id) = existing_match_id {
                        // Update existing event
                        if valid_ids.contains(&event_id) {
                            if let Err(e) = state.ontology_repository.update_event(
                                user_id,
                                event_id,
                                Some(&format!("Update: {}", description)),
                                None,
                                remind_at,
                                due_at,
                            ) {
                                tracing::warn!("Failed to update event {}: {}", event_id, e);
                            } else if let Some(mid) = message_id {
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
                        }
                    } else {
                        // Create new event (always auto-confirm)
                        let new_event = crate::models::ontology_models::NewOntEvent {
                            user_id,
                            description: description.clone(),
                            remind_at,
                            due_at,
                            status: "active".to_string(),
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
                                    "Auto-created event {} for user {}: {}",
                                    created.id, user_id, created.description
                                );
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to create event for user {}: {}",
                                    user_id,
                                    e
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

/// Check if the user's outgoing message indicates a tracked event is already done.
/// Only past-tense completions count ("done", "sent it", "paid it") - not promises or acknowledgements.
pub async fn check_outgoing_event_resolution(
    state: &Arc<AppState>,
    user_id: i32,
    room_id: &str,
    content: &str,
    message_id: i64,
) {
    // Check setting
    let settings = match state.user_core.get_user_settings(user_id) {
        Ok(s) => s,
        Err(_) => return,
    };
    if !settings.auto_track_items_system {
        return;
    }

    // Get active events - return early if none (no LLM cost)
    let active_events = match state
        .ontology_repository
        .get_active_and_proposed_events(user_id)
    {
        Ok(events) if !events.is_empty() => events,
        _ => return,
    };

    // Fetch conversation context for disambiguation
    let recent_messages = state
        .ontology_repository
        .get_messages_for_room(user_id, room_id, 10)
        .unwrap_or_default();

    let conversation_lines: Vec<String> = recent_messages
        .iter()
        .rev()
        .map(|m| format!("{}: {}", m.sender_name, m.content))
        .collect();
    let conversation_text = conversation_lines.join("\n");

    // Build event list
    let event_list: Vec<String> = active_events
        .iter()
        .map(|e| format!("[id={}] {}", e.id, e.description))
        .collect();
    let events_text = event_list.join("\n");

    // Build AgentContext
    let ctx = match ContextBuilder::for_user(state, user_id).build().await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(
                "Failed to build context for outgoing event resolution: {}",
                e
            );
            return;
        }
    };

    let system_prompt = format!(
        "You are checking if the user's outgoing message indicates a tracked event is ALREADY DONE.\n\
        \n\
        Tracked events:\n{}\n\
        \n\
        Recent conversation:\n{}\n\
        \n\
        The user just sent: \"{}\"\n\
        \n\
        Rules - be STRICT:\n\
        - Only mark an event completed if the user's message clearly indicates the action is DONE/FINISHED/PAST TENSE.\n\
        - \"Done, sent the report\" -> complete\n\
        - \"Paid it\" -> complete\n\
        - \"I'll do it later\" -> NOT complete (promise, not done)\n\
        - \"I'll be there at 5\" -> NOT complete (acknowledgement)\n\
        - \"ok\" -> NOT complete (just engaging)\n\
        - \"sounds good\" -> NOT complete (just acknowledging)\n\
        - When in doubt, do NOT mark as completed.\n\
        \n\
        Return the IDs of events that are now completed. Return empty array if none.",
        events_text, conversation_text, content
    );

    // Build tool
    let mut properties = HashMap::new();
    properties.insert(
        "completed_ids".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Array),
            description: Some(
                "IDs of tracked events that are now completed. Empty array if none.".to_string(),
            ),
            items: Some(Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::Number),
                ..Default::default()
            })),
            ..Default::default()
        }),
    );

    let tool = chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: "event_resolution".to_string(),
            description: Some("Report which tracked events are completed".to_string()),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(vec!["completed_ids".to_string()]),
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
            content: chat_completion::Content::Text(content.to_string()),
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
            tracing::warn!("Outgoing event resolution LLM call failed: {}", e);
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
        "outgoing_event_resolution",
        &result,
    );

    let choice = match result.choices.first() {
        Some(c) => c,
        None => return,
    };

    if let Some(ref tool_calls) = choice.message.tool_calls {
        for tc in tool_calls {
            if tc.function.name.as_deref() != Some("event_resolution") {
                continue;
            }
            let args = tc.function.arguments.as_deref().unwrap_or("{}");
            let parsed: serde_json::Value = match serde_json::from_str(args) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let completed_ids: Vec<i32> = parsed
                .get("completed_ids")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_f64().map(|n| n as i32))
                        .collect()
                })
                .unwrap_or_default();

            let valid_ids: std::collections::HashSet<i32> =
                active_events.iter().map(|e| e.id).collect();

            for event_id in completed_ids {
                if !valid_ids.contains(&event_id) {
                    continue;
                }
                if let Err(e) =
                    state
                        .ontology_repository
                        .update_event_status(user_id, event_id, "completed")
                {
                    tracing::warn!("Failed to mark event {} completed: {}", event_id, e);
                    continue;
                }
                let _ = state.ontology_repository.create_link(
                    user_id,
                    "Event",
                    event_id,
                    "Message",
                    message_id as i32,
                    "resolve_message",
                    None,
                );
                let desc = active_events
                    .iter()
                    .find(|e| e.id == event_id)
                    .map(|e| e.description.as_str())
                    .unwrap_or("unknown");
                info!(
                    "Auto-completed event {} for user {} via outgoing message: {}",
                    event_id, user_id, desc
                );
            }
        }
    }
}
