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

const COOLDOWN_SECS: i32 = 3600; // 1 hour per room

/// Daily token budget per user for proactive AI processing (system_important, rules, etc.)
/// 10M tokens/month target (~$15/user/month at $1.50/M input tokens).
/// 10M / 30 days = ~332K tokens/day. Normal users use 5-20K/day.
const DAILY_TOKEN_BUDGET_PER_USER: i64 = 332_000;

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
                let notified_marker = if m.sender_name != "You"
                    && m.urgency.as_deref() == Some("now")
                    && i != chronological.len() - 1
                {
                    " [notified]"
                } else {
                    ""
                };
                let eval_marker = if i == chronological.len() - 1 {
                    " <-- evaluate this"
                } else {
                    ""
                };
                format!(
                    "[{}] [{}]{} {}: {}{}",
                    ts, seen_marker, notified_marker, m.sender_name, m.content, eval_marker
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
        conversation_thread: &conversation,
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
        Decide whether this needs the user's attention immediately, or can wait for the next scheduled digest:\n\
        - now: genuine emergency or time-critical situation where a delay of hours would cause \
        irreversible harm (safety, health, locked out, stranded, critical deadline). The user gets an \
        immediate SMS. A friend casually asking for help, making plans, or requesting a favor is NOT \
        \"now\" even if they say \"today\" or \"tomorrow\".\n\
        - later: everything else — important-but-not-urgent, routine, casual, or spam. Bundled into \
        the next scheduled digest.",
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
            description: Some(
                "now = needs attention immediately (triggers SMS). later = can wait for the next digest."
                    .to_string(),
            ),
            enum_values: Some(vec!["now".to_string(), "later".to_string()]),
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
                "Summary starting with sender name, e.g. 'Mom: ...'. No URLs. \
                 Length depends on urgency: \
                 - now: up to 160 chars. This goes directly as an SMS to a user who muted all other notifications — include enough context to act (who, what, when, what's the ask). \
                 - later: 30-60 chars max. This becomes a teaser in a bundled digest alongside many others — just a slight hint of what's happened. The user can reply to ask for full context. \
                 \n\nUS A2P CARRIER COMPLIANCE — STRICT RULES TO AVOID PHISHING-FILTER BANS:\n\
                 1. NEVER start with a brand name that looks like impersonation. Bad: 'Google Security: new sign-in...'. 'PayPal: account access...'. 'Apple ID: ...'. Good: 'Your Gmail flagged a sign-in...'. 'PayPal alert in your inbox: ...'. The message is FROM Lightfriend ABOUT what a third party reported — never echo their alert headline.\n\
                 2. NEVER defang emails or URLs ('user[.]name', 'user(at)domain', 'domain[.]com'). Use real text or truncate ('user...@gmail.com'). Defanging is itself a phishing/evasion signal that filters score on.\n\
                 3. AVOID action-prompt language: 'verify now', 'click here', 'check activity', 'confirm immediately', 'act now', 'secure your account'. State facts only — let the user decide what to do. Bad: 'Check activity if this wasn't you'. Good: 'Sign-in was from an iPhone in California'.\n\
                 4. AVOID urgency keywords clustered together: 'verify' + 'suspended' + 'urgent' in the same SMS reads as spam to filters.\n\
                 5. Reframe security/financial alerts in your own conversational voice. Bad: 'PayPal Security: unauthorized charge of $500'. Good: 'PayPal flagged a $500 charge as unfamiliar — happened 10 min ago at a gas station.'\n\
                 6. For sender name prefix when it's a brand, prefer descriptors over brand-as-impersonator. Bad: 'Chase: balance low'. Good: 'Your Chase account balance dropped below $100'."
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
                .unwrap_or("later");
            let category = parsed
                .get("category")
                .and_then(|v| v.as_str())
                .unwrap_or("social");
            let summary = parsed.get("summary").and_then(|v| v.as_str()).unwrap_or("");

            // now = notify, unless it's our own outgoing message
            let should_notify = urgency == "now" && sender_name != "You";

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

                    let _ =
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
    conversation_thread: &'a str,
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

    if !input.conversation_thread.is_empty() {
        sections.push(input.conversation_thread.to_string());
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

/// Pass 1 of commitment detection. Cheap LLM filter that decides whether a
/// message is worth running the full extraction pass on. Uses the Voice model
/// (fast/cheap), tiny prompt, single boolean output. Multilingual by design -
/// the prompt describes categories abstractly rather than listing English
/// phrases, so it works on any-language conversations.
///
/// Bias: lean toward `true`. Pass 2 will reject false positives; false
/// negatives are lost forever.
///
/// Returns (relevant, raw_args_json) where raw_args_json is the LLM tool-call
/// arguments verbatim, suitable for persisting on the message for the activity
/// feed.
async fn commitment_gate(
    state: &Arc<AppState>,
    user_id: i32,
    ctx: &crate::context::AgentContext,
    content: &str,
    sender_name: &str,
    platform: &str,
) -> Result<(bool, String), Box<dyn std::error::Error + Send + Sync>> {
    let system_prompt = "You are a fast filter that decides whether a single chat message is worth analyzing for commitments, obligations, deadlines, or completion signals.\n\
        \n\
        A message is RELEVANT if it could contain any of (in any language):\n\
        - a promise or commitment to do something (\"I'll do X\", \"I will\", \"I promise\")\n\
        - a request that creates a future obligation (\"can you do X by Friday?\")\n\
        - a deadline, due date, or time reference tied to an action\n\
        - a completion signal (saying something is done, sent, paid, finished, completed)\n\
        - a follow-up commitment (\"will get back\", \"let you know\", \"I'll check\")\n\
        - a deadline change, postponement, or reschedule\n\
        - a short positive reply that could be confirming or completing a prior request (\"yes\", \"done\", \"ok\", \"sure\", \"thanks\")\n\
        \n\
        NOT RELEVANT: pure greetings, reactions to news/links, opinions, statements of fact, emotional reactions, jokes.\n\
        \n\
        Lean toward RELEVANT when uncertain. A more thorough second pass will verify and reject false positives.\n\
        Works in any language - never reject solely because the message is non-English.";

    let user_msg = format!("Message from {} on {}:\n{}", sender_name, platform, content);

    let mut properties = HashMap::new();
    properties.insert(
        "relevant".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Boolean),
            description: Some(
                "True if the message is worth analyzing for commitments/obligations. Lean toward true when unsure.".to_string(),
            ),
            ..Default::default()
        }),
    );

    let tool = chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: "gate_result".to_string(),
            description: Some(
                "Return whether the message is relevant for commitment tracking".to_string(),
            ),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(vec!["relevant".to_string()]),
            },
        },
    };

    let gate_model = state
        .ai_config
        .model(ctx.provider, crate::ModelPurpose::Voice)
        .to_string();

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
            content: chat_completion::Content::Text(user_msg),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
    ];

    let request = chat_completion::ChatCompletionRequest::new(gate_model.clone(), messages)
        .tools(vec![tool])
        .tool_choice(chat_completion::ToolChoiceType::Required)
        .temperature(0.0);

    let result = state
        .ai_config
        .chat_completion(ctx.provider, &request)
        .await
        .map_err(|e| format!("Commitment gate LLM call failed: {}", e))?;

    crate::ai_config::log_llm_usage(
        &state.llm_usage_repository,
        user_id,
        match ctx.provider {
            crate::AiProvider::Tinfoil => "tinfoil",
            crate::AiProvider::OpenRouter => "openrouter",
        },
        &gate_model,
        "commitment_gate",
        &result,
    );

    let choice = result
        .choices
        .first()
        .ok_or_else(|| "gate: no choices".to_string())?;
    let tool_calls = choice
        .message
        .tool_calls
        .as_ref()
        .ok_or_else(|| "gate: no tool_calls".to_string())?;
    let tc = tool_calls
        .first()
        .ok_or_else(|| "gate: empty tool_calls".to_string())?;
    let args = tc.function.arguments.as_deref().unwrap_or("{}");
    let parsed: serde_json::Value = serde_json::from_str(args)?;
    // Bias toward true: if the LLM returns garbage, fail open.
    let relevant = parsed
        .get("relevant")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    Ok((relevant, args.to_string()))
}

/// Detect and extract commitments from an incoming message.
/// Runs immediately on every message (no delay, no seen-check).
/// Independent of urgency classification.
pub async fn run_commitment_detection(
    state: &Arc<AppState>,
    user_id: i32,
    entity_snapshot: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
    let content = entity_snapshot
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let content_preview: String = content.chars().take(80).collect();

    let settings = state.user_core.get_user_settings(user_id)?;
    if !settings.auto_track_items_system {
        info!(
            "commitment_detection skip user={} reason=setting_off platform={} sender={}",
            user_id, platform, sender_name
        );
        return Ok(());
    }

    info!(
        "commitment_detection start user={} platform={} sender={} room={} msg_id={:?} preview={:?}",
        user_id, platform, sender_name, room_id, message_id, content_preview
    );

    if exceeds_daily_token_budget(state, user_id, "commitment_detection") {
        return Ok(());
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i32;

    let ctx = ContextBuilder::for_user(state, user_id).build().await?;

    // ---------------------------------------------------------------------
    // Pass 1: cheap LLM gate. Tight prompt, single bool output, Voice model.
    // Multilingual by design. Biased toward "relevant" - the full extraction
    // pass below will reject false positives. Skips empty content.
    // ---------------------------------------------------------------------
    if content.trim().is_empty() {
        info!(
            "commitment_detection user={} pass1=skip reason=empty_content",
            user_id
        );
        return Ok(());
    }
    let (gate_relevant, gate_raw) =
        match commitment_gate(state, user_id, &ctx, &content, sender_name, platform).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(
                    "commitment_gate failed user={}: {} (failing open)",
                    user_id,
                    e
                );
                (true, format!("{{\"error\":{:?}}}", e.to_string()))
            }
        };

    info!(
        "commitment_detection user={} pass1={} platform={} sender={}",
        user_id,
        if gate_relevant { "relevant" } else { "skip" },
        platform,
        sender_name
    );

    // Persist gate decision on the message immediately, so the activity feed
    // shows why a message was gated out even if Pass 2 never runs.
    if let Some(mid) = message_id {
        let envelope = serde_json::json!({
            "gate": serde_json::from_str::<serde_json::Value>(&gate_raw)
                .unwrap_or(serde_json::Value::String(gate_raw.clone())),
            "extraction": serde_json::Value::Null,
        });
        if let Err(e) = state.ontology_repository.update_message_commitment(
            mid,
            Some("<Pass 1 gate>"),
            Some(&envelope.to_string()),
        ) {
            tracing::warn!("Failed to persist gate result on message {}: {}", mid, e);
        }
    }

    if !gate_relevant {
        return Ok(());
    }

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

    // Pass 2 prompt: extraction-framed (Palantir-style). Asks the model to
    // produce a structured Obligation; if none is present, return
    // has_commitment=false. All other fields are required - the model MUST
    // fill them with valid values OR set has_commitment=false. This prevents
    // the silent-skip path where has_commitment=true but description is empty.
    let tz_label = ctx
        .timezone
        .as_ref()
        .map(|t| format!("{} {}", t.tz_str, t.offset_string))
        .unwrap_or_else(|| "UTC".to_string());
    let system_prompt = format!(
        "You extract structured Obligation entities from a chat message.\n\
        \n\
        Current time: {} ({})\n\
        Sender of the message under evaluation: {}\n\
        \"You\" in the conversation refers to the user this assistant works for.\n\
        \n\
        {}\n\
        \n\
        An Obligation is a specific, actionable thing the user could forget about. Categories:\n\
        - commitment_to_user: the sender (or another non-user party) promised to do something for the user (send invoice, deliver, call back).\n\
        - commitment_by_user: the user promised to do something (pay, book, confirm, follow up, buy).\n\
        - completion_signal: a past-tense indication that one of the existing tracked items is now done.\n\
        - deadline_update: a change to an existing tracked item's timeline.\n\
        \n\
        Decision rules:\n\
        1. If the latest message contains no concrete actionable obligation, return has_commitment=false and STOP. Leave description empty.\n\
        2. If it DOES, you MUST fill ALL of: commitment_type, description, who, confidence. description must be a short imperative phrase (\"Buy hat from seller\", \"Pay invoice\", \"Send project files\") - never empty when has_commitment=true.\n\
        3. who is strictly \"sender\" or \"user\". \"sender\" means the message sender. \"user\" means the user this assistant works for. Never free-form.\n\
        4. deadline must be RFC 3339 in the timezone shown above, or null.\n\
        5. existing_match_id is the integer id from the \"Existing tracked items\" list above, or null if this is a new obligation.\n\
        6. Works in any language. Conditional commitments (\"if X then I'll buy Friday\") still count.\n\
        \n\
        Bias: when in doubt about description specificity, write a best-guess imperative phrase rather than leaving it empty.",
        now_formatted, tz_label, sender_name, events_context
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
                // All structural fields are required - the model MUST fill
                // them with valid values OR set has_commitment=false. This
                // prevents the silent-skip path where has_commitment=true
                // but description is empty.
                required: Some(vec![
                    "has_commitment".to_string(),
                    "commitment_type".to_string(),
                    "description".to_string(),
                    "who".to_string(),
                    "confidence".to_string(),
                ]),
            },
        },
    };

    // Pass 2 uses the Default (stronger) model for structured extraction.
    // The Voice model breaks the schema on this task - returning freeform
    // text in enum fields, omitting required fields. Pass 1 already filtered
    // out small talk, so we only pay the bigger model on signal-bearing
    // messages.
    let classification_model = state
        .ai_config
        .model(ctx.provider, crate::ModelPurpose::Default)
        .to_string();

    let messages = vec![
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::system,
            content: chat_completion::Content::Text(system_prompt.clone()),
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

    // Build the gate JSON value once - reused across every envelope write.
    let gate_json = serde_json::from_str::<serde_json::Value>(&gate_raw)
        .unwrap_or(serde_json::Value::String(gate_raw.clone()));
    let combined_prompt = format!("<Pass 2 extraction>\n{}", system_prompt);

    // Helper: persist the full envelope at any decision point. Captures
    // gate + extraction + Pass 3 routing decision so the activity feed shows
    // the complete trace for every message, including ones we silently skipped.
    let persist_envelope = |extraction: serde_json::Value, routing: serde_json::Value| {
        if let Some(mid) = message_id {
            let envelope = serde_json::json!({
                "gate": gate_json.clone(),
                "extraction": extraction,
                "routing": routing,
            });
            if let Err(e) = state.ontology_repository.update_message_commitment(
                mid,
                Some(&combined_prompt),
                Some(&envelope.to_string()),
            ) {
                tracing::warn!(
                    "Failed to persist commitment result on message {}: {}",
                    mid,
                    e
                );
            }
        }
    };

    let choice = match result.choices.first() {
        Some(c) => c,
        None => {
            info!(
                "commitment_detection user={} llm returned no choices",
                user_id
            );
            persist_envelope(
                serde_json::Value::Null,
                serde_json::json!({"action": "skipped", "reason": "no_llm_choices"}),
            );
            return Ok(());
        }
    };

    let extraction_raw = if let Some(ref tcs) = choice.message.tool_calls {
        tcs.iter()
            .find(|tc| tc.function.name.as_deref() == Some("commitment_result"))
            .and_then(|tc| tc.function.arguments.clone())
    } else {
        choice.message.content.clone()
    };
    let extraction_json = extraction_raw
        .as_deref()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
        .unwrap_or(serde_json::Value::Null);

    let tool_calls = match choice.message.tool_calls.as_ref() {
        Some(tcs) if !tcs.is_empty() => tcs,
        _ => {
            info!(
                "commitment_detection user={} llm returned no tool_calls (text content present)",
                user_id
            );
            persist_envelope(
                extraction_json.clone(),
                serde_json::json!({"action": "skipped", "reason": "no_tool_calls"}),
            );
            return Ok(());
        }
    };

    let mut routing: serde_json::Value =
        serde_json::json!({"action": "skipped", "reason": "no_matching_tool_call"});

    for tc in tool_calls {
        if tc.function.name.as_deref() != Some("commitment_result") {
            continue;
        }
        let args = tc.function.arguments.as_deref().unwrap_or("{}");
        let parsed: serde_json::Value = match serde_json::from_str(args) {
            Ok(v) => v,
            Err(e) => {
                info!(
                    "commitment_detection user={} failed to parse tool args: {} (raw={})",
                    user_id, e, args
                );
                routing = serde_json::json!({
                    "action": "skipped",
                    "reason": "tool_args_unparseable",
                    "error": e.to_string(),
                });
                continue;
            }
        };

        let has_commitment = parsed
            .get("has_commitment")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if !has_commitment {
            info!(
                "commitment_detection user={} llm said no commitment (platform={} sender={} preview={:?})",
                user_id, platform, sender_name, content_preview
            );
            routing = serde_json::json!({"action": "skipped", "reason": "has_commitment_false"});
            break;
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
            info!(
                "commitment_detection user={} llm has_commitment=true but empty description (type={})",
                user_id, commitment_type
            );
            routing = serde_json::json!({
                "action": "skipped",
                "reason": "empty_description",
                "commitment_type": commitment_type,
            });
            continue;
        }

        info!(
            "commitment_detection user={} has_commitment=true type={} desc={:?} match_id={:?}",
            user_id, commitment_type, description, existing_match_id
        );

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
                routing = match existing_match_id {
                    Some(event_id) if valid_ids.contains(&event_id) => {
                        match state.ontology_repository.update_event_status(
                            user_id,
                            event_id,
                            "completed",
                        ) {
                            Ok(_) => {
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
                                serde_json::json!({
                                    "action": "completed",
                                    "event_id": event_id,
                                })
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to mark event {} completed: {}",
                                    event_id,
                                    e
                                );
                                serde_json::json!({
                                    "action": "error",
                                    "reason": "update_event_status_failed",
                                    "error": e.to_string(),
                                })
                            }
                        }
                    }
                    Some(_) => serde_json::json!({
                        "action": "skipped",
                        "reason": "existing_match_id_not_in_active_set",
                    }),
                    None => serde_json::json!({
                        "action": "skipped",
                        "reason": "completion_signal_without_match_id",
                    }),
                };
            }
            "deadline_update" => {
                routing = match existing_match_id {
                    Some(event_id) if valid_ids.contains(&event_id) => {
                        let old_event = state.ontology_repository.get_event(user_id, event_id).ok();
                        let update_desc = if description.is_empty() {
                            None
                        } else {
                            Some(format!("Update: {}", description))
                        };
                        match state.ontology_repository.update_event(
                            user_id,
                            event_id,
                            update_desc.as_deref(),
                            None,
                            remind_at,
                            due_at,
                        ) {
                            Ok(_) => {
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
                                if let Some(old) = old_event {
                                    let old_due = old.due_at.unwrap_or(0);
                                    if let Some(new_due) = due_at {
                                        if old_due != new_due && old_due != 0 {
                                            let change_msg = format!(
                                                "Tracked item updated: \"{}\"\nDeadline changed based on new message from {}.",
                                                old.description, sender_name
                                            );
                                            let _ = send_notification(
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
                                serde_json::json!({
                                    "action": "deadline_updated",
                                    "event_id": event_id,
                                    "new_due_at": due_at,
                                })
                            }
                            Err(e) => {
                                tracing::warn!("Failed to update event {}: {}", event_id, e);
                                serde_json::json!({
                                    "action": "error",
                                    "reason": "update_event_failed",
                                    "error": e.to_string(),
                                })
                            }
                        }
                    }
                    Some(_) => serde_json::json!({
                        "action": "skipped",
                        "reason": "existing_match_id_not_in_active_set",
                    }),
                    None => serde_json::json!({
                        "action": "skipped",
                        "reason": "deadline_update_without_match_id",
                    }),
                };
            }
            _ => {
                // commitment_to_user or commitment_by_user
                routing = if let Some(event_id) = existing_match_id {
                    if valid_ids.contains(&event_id) {
                        match state.ontology_repository.update_event(
                            user_id,
                            event_id,
                            Some(&format!("Update: {}", description)),
                            None,
                            remind_at,
                            due_at,
                        ) {
                            Ok(_) => {
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
                                serde_json::json!({
                                    "action": "updated",
                                    "event_id": event_id,
                                })
                            }
                            Err(e) => {
                                tracing::warn!("Failed to update event {}: {}", event_id, e);
                                serde_json::json!({
                                    "action": "error",
                                    "reason": "update_event_failed",
                                    "error": e.to_string(),
                                })
                            }
                        }
                    } else {
                        serde_json::json!({
                            "action": "skipped",
                            "reason": "existing_match_id_not_in_active_set",
                        })
                    }
                } else {
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
                            serde_json::json!({
                                "action": "created",
                                "event_id": created.id,
                                "status": "active",
                                "due_at": due_at,
                            })
                        }
                        Err(e) => {
                            tracing::warn!("Failed to create event for user {}: {}", user_id, e);
                            serde_json::json!({
                                "action": "error",
                                "reason": "create_event_failed",
                                "error": e.to_string(),
                            })
                        }
                    }
                };
            }
        }
        break;
    }

    persist_envelope(extraction_json, routing);
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
