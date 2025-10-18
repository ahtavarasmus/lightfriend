use crate::models::user_models::WaitingCheck;
use crate::AppState;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use openai_api_rs::v1::{
    chat_completion,
    types,
    common::GPT4_O,
};
use chrono::Timelike;
use crate::tool_call_utils::utils::create_openai_client;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, Duration};


/// Definition of a **critical** message: something that will cause human‑safety risk,
/// major financial/data loss, legal breach, or production outage if it waits >2 h.
/// The model must default to *non‑critical* when uncertain.

/// Prompt for matching incoming messages against the user’s *waiting checks*.
/// A waiting check represents something the user explicitly asked to be notified
/// about (e.g. \"Tell me when the shipment arrives\").
const WAITING_CHECK_PROMPT: &str = r#"You are an AI that determines whether an incoming message *definitively* satisfies **one** of the outstanding waiting checks listed below. Each waiting check's 'Content' describes the condition the message must meet.
    **Match rules**
    • Interpret the waiting check 'Content' as the user's condition or instruction for matching.
    • If the content is descriptive or instructional (e.g., a sentence >5 words), use semantic reasoning (synonyms, paraphrases, context) to evaluate fulfillment. Translate non-English text internally.
    • If the content is short (≤5 words, e.g., keywords), require the message to contain *all* those words (case-insensitive, but exact matches preferred; stems/synonyms only if explicitly related).
    • A match must be *unambiguous*: the message clearly fulfills the condition. Ambiguous, partial, or sender-only matches DO NOT count.
    • Do not match based solely on sender or metadata unless explicitly stated in the content.
    • If multiple checks could match, choose the single *best* match (highest confidence). Return `null` if none match.

    **Edge cases**:
    • If the check mentions a sender (e.g., 'from Rasmus'), require the message metadata to match exactly.
    • For conditions like 'related to [topic]', use broad semantic similarity but ensure at least 70% conceptual overlap.
    • Ignore irrelevant message parts; focus only on fulfilling the core condition.

    If a match is found you MUST additionally craft two short notifications:
    1. `sms_message` (≤160 chars) – a concise SMS describing the event.
    2. `first_message` (≤100 chars) – an attention-grabbing first sentence a voice assistant would speak on a call.

    Return JSON with:
    • `waiting_check_id` – integer ID of the matched check, or null
    • `sms_message` – String (required when matched, else empty string). Ensure `sms_message` is neutral and factual, e.g., 'Matched waiting check: Update from Rasmus on phone received.'
    • `first_message` – String (required when matched, else empty string). `first_message` should be urgent and spoken-friendly, e.g., 'Hey, you have an update from Rasmus about the phone!'
    • `match_explanation` – ≤120 chars explaining why it matched (or empty when null)
"#;

const CRITICAL_PROMPT: &str = r#"You are an AI that decides whether an incoming user message is **critical** — i.e. it must be surfaced within **two hours** and cannot wait for the next scheduled summary.
A message is **critical** if delaying action beyond 2 h risks:
• Direct harm to people
• Severe data loss or major financial loss
• Production system outage or security breach
• Hard legal/compliance deadline expiring in ≤ 2 h
• The sender explicitly says it must be handled immediately (e.g. “ASAP”, “emergency”, “right now”) or gives a ≤ 2 h deadline.
• Time-sensitive personal or social requests/opportunities with an implied or stated window of ≤2 hours (e.g., invitations for immediate events like lunch, or quick decisions needed right now).
Everything else — vague urgency, routine updates, or unclear requests — is **NOT** critical.
If unsure, choose **not critical**.
---
### Process
1. Detect the message language; translate internally to English before reasoning.
2. Identify any explicit or implied time windows (e.g., "now," "soon," "today at noon," or contexts like being at a location requiring immediate input).
3. Apply the criteria **strictly**.
4. Produce JSON with these fields (do **not** add others):
| Field | Required? | Max chars | Content rules |
|-------|-----------|-----------|---------------|
| `is_critical` | always | — | Boolean. |
| `what_to_inform` | *only when* `is_critical==true` | 160 | **One SMS sentence** that:<br> • Briefly summarizes the core problem/ask (who/what/when).<br> • States the single most urgent next action the recipient must take within 2 h. Remember to include the sender or the Chat the message is from. |
| `first_message` | *only when* `is_critical==true` | 100 | **Voice-assistant opener** that grabs attention and repeats the required action in imperative form. |
If `is_critical` is false, leave the other two fields empty strings.
---
#### Examples
**Incoming:**
“I'm at the store should I buy eggs or do we have some still?”
**Output:**
{
  "is_critical": true,
  "what_to_inform": "Rasmus is asking on WhatsApp if he needs to buy eggs as well",
  "first_message": "Hey, Rasmus needs more information about the shopping list"
}

**Incoming**:
"Hey, want to grab lunch? I'm free until 1 PM."
**Output**:
{
  "is_critical": true,
  "what_to_inform": "Alex is inviting you to lunch on WhatsApp",
  "first_message": "Alex is asking if you want to crab lunch!"
}

**Incoming**:
"Weekly team update: Project is on track."
**Output**:
{
  "is_critical": false,
  "what_to_inform": "",
  "first_message": ""
}
"#;


#[derive(Debug, Serialize, Deserialize)]
// or #[serde_with::skip_serializing_none]  <-- whole-struct shortcut
pub struct WaitingCheckMatchResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiting_check_id: Option<i32>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sms_message: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_message: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub match_explanation: Option<String>,
}

/// Determine whether `message` satisfies **one** of the supplied `waiting_checks`.
/// Returns `(waiting_check_id, sms_message, first_message)`.
pub async fn check_waiting_check_match(
    state: &Arc<AppState>,
    message: &str,
    waiting_checks: &Vec<WaitingCheck>,
) -> Result<(Option<i32>, Option<String>, Option<String>), Box<dyn std::error::Error>> {
    let client = create_openai_client(&state)?;

    let waiting_checks_str = waiting_checks
        .iter()
        .map(|check| format!("ID: {}, Content: {}", check.id.unwrap_or(-1), check.content))
        .collect::<Vec<_>>()
        .join("\n");

    let messages = vec![
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::system,
            content: chat_completion::Content::Text(WAITING_CHECK_PROMPT.to_string()),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::user,
            content: chat_completion::Content::Text(format!(
                "Incoming message:\n\n{}\n\nWaiting checks:\n\n{}\n\nReturn the best match or null.",
                message, waiting_checks_str
            )),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
    ];

    // JSON schema -----------------------------------------------------------
    let mut properties = std::collections::HashMap::new();
    properties.insert(
        "waiting_check_id".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Number),
            description: Some("ID of the matched waiting check, or null".to_string()),
            ..Default::default()
        }),
    );
    properties.insert(
        "sms_message".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Concise SMS (≤160 chars) when matched, else empty".to_string()),
            ..Default::default()
        }),
    );
    properties.insert(
        "first_message".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Voice‑assistant opening line (≤100 chars) when matched, else empty".to_string()),
            ..Default::default()
        }),
    );

    let tools = vec![chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: "analyze_waiting_check_match".to_string(),
            description: Some("Determines whether the message matches a waiting check and drafts notifications".to_string()),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(vec!["waiting_check_id".to_string()]),
            },
        },
    }];

    let request = chat_completion::ChatCompletionRequest::new(
        GPT4_O.to_string(),
        messages)
        .tools(tools)
        .tool_choice(chat_completion::ToolChoiceType::Required)
        .temperature(0.0)
        .max_tokens(200);

    let result = client.chat_completion(request).await?;
    let tool_call = result.choices[0]
        .message
        .tool_calls
        .as_ref()
        .and_then(|tc| tc.first())
        .ok_or("No tool call in waiting‑check response")?;

    let args = tool_call
        .function
        .arguments
        .as_ref()
        .ok_or("No arguments in waiting‑check tool call")?;

    let response: WaitingCheckMatchResponse = serde_json::from_str(args)?;

    if let Some(explanation) = &response.match_explanation {
        tracing::debug!("Waiting‑check match explanation: {}", explanation);
    }

    Ok((response.waiting_check_id, response.sms_message, response.first_message))
}

#[derive(Debug, Serialize)]
pub struct DigestData {
    pub messages: Vec<MessageInfo>,
    pub calendar_events: Vec<CalendarEvent>,
    pub time_period_hours: u32,
}

#[derive(Debug, Serialize)]
pub struct MessageInfo {
    pub sender: String,
    pub content: String,
    pub timestamp_rfc: String,
    pub platform: String, // e.g., "email", "whatsapp", "telegram", "signal" etc.
}

#[derive(Debug, Serialize)]
pub struct CalendarEvent {
    pub title: String,
    pub start_time_rfc: String,
    pub duration_minutes: i64,
}


#[derive(Debug, Serialize, Deserialize)]
pub struct MatchResponse {
    pub is_critical: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub what_to_inform: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_message: Option<String>,
}

/// Checks whether a single message is critical.
/// Returns `(is_critical, what_to_inform, first_message)`.
pub async fn check_message_importance(
    state: &Arc<AppState>,
    user_id: i32,
    message: &str,
    service: &str,
    chat_name: &str,
    raw_content: &str,
) -> Result<(bool, Option<String>, Option<String>), Box<dyn std::error::Error>> {
    // Special case for WhatsApp incoming calls
    if raw_content.contains("Incoming call") || raw_content.contains("Missed call") {
        let call_notify = state.user_core.get_call_notify(user_id).unwrap_or(true);
        if call_notify {
            // Trim for SMS
            let what_to_inform= format!("You have an incoming {} call from {}", service, chat_name);
            let first_message = format!("Hello, you have an incoming WhatsApp call from {}.", chat_name);
            return Ok((true, Some(what_to_inform), Some(first_message)));
        } else {
            return Ok((false, None, None));
        }
    }
    // Build the chat payload ----------------------------------------------
    let client = create_openai_client(&state)?;
    let messages = vec![
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::system,
            content: chat_completion::Content::Text(CRITICAL_PROMPT.to_string()),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::user,
            content: chat_completion::Content::Text(format!(
                "Analyze this message and decide if it is critical:\n\n{}",
                message
            )),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
    ];
    // JSON schema for the structured output -------------------------------
    let mut properties = std::collections::HashMap::new();
    properties.insert(
        "is_critical".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Boolean),
            description: Some("Whether the message is critical and requires immediate attention".to_string()),
            ..Default::default()
        }),
    );
    properties.insert(
        "what_to_inform".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Concise SMS (≤160 chars) to send if the message is critical".to_string()),
            ..Default::default()
        }),
    );
    properties.insert(
        "first_message".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Brief voice‑assistant opening line (≤100 chars) if critical".to_string()),
            ..Default::default()
        }),
    );
    let tools = vec![chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: "analyze_message".to_string(),
            description: Some("Analyzes if a message is critical".to_string()),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(vec!["is_critical".to_string()]),
            },
        },
    }];
    let request = chat_completion::ChatCompletionRequest::new(
        GPT4_O.to_string(),
        messages)
        .tools(tools)
        .tool_choice(chat_completion::ToolChoiceType::Required)
        // Lower temperature for more deterministic classification
        .temperature(0.2)
        .max_tokens(200);
    // ---------------------------------------------------------------------
    match client.chat_completion(request).await {
        Ok(result) => {
            if let Some(tool_calls) = result.choices[0].message.tool_calls.as_ref() {
                if let Some(first_call) = tool_calls.first() {
                    if let Some(args) = &first_call.function.arguments {
                        match serde_json::from_str::<MatchResponse>(args) {
                            Ok(response) => {
                                tracing::debug!(target: "critical_check", ?response, "Message analysis result");
                                Ok((response.is_critical, response.what_to_inform, response.first_message))
                            }
                            Err(e) => {
                                tracing::error!("Failed to parse message analysis response: {}", e);
                                Ok((false, None, None))
                            }
                        }
                    } else {
                        tracing::error!("No arguments found in tool call");
                        Ok((false, None, None))
                    }
                } else {
                    tracing::error!("No tool calls found");
                    Ok((false, None, None))
                }
            } else {
                tracing::error!("No tool calls section in response");
                Ok((false, None, None))
            }
        }
        Err(e) => {
            tracing::error!("Failed to get message analysis: {}", e);
            Err(e.into())
        }
    }
}


// Helper function to calculate hours until a target hour
fn hours_until(current_hour: u32, target_hour: u32) -> u32 {
    if current_hour <= target_hour {
        target_hour - current_hour
    } else {
        24 - (current_hour - target_hour)
    }
}

// Helper function to calculate hours since a previous hour
fn hours_since(current_hour: u32, previous_hour: u32) -> u32 {
    if current_hour >= previous_hour {
        current_hour - previous_hour
    } else {
        current_hour + (24 - previous_hour)
    }
}

pub async fn check_morning_digest(state: &Arc<AppState>) -> Result<(), Box<dyn std::error::Error>> {
    // Get the user's digest settings and timezone
    let (morning_digest, day_digest, evening_digest) = state.user_core.get_digests()?;
    let user_info = state.user_core.get_user_info()?;
    
    // If morning digest is enabled (Some value) and we have a timezone, check the time
    if let (Some(digest_hour_str), Some(timezone)) = (morning_digest.clone(), user_info.timezone) {
        // Parse the timezone
        let tz: chrono_tz::Tz = timezone.parse()
            .map_err(|e| format!("Invalid timezone: {}", e))?;
            
        // Get current time in user's timezone
        let now = chrono::Utc::now().with_timezone(&tz);
        
        // Parse the digest hour (expected format: "HH:00" like "00:00", "23:00")
        let digest_hour: u32 = digest_hour_str
            .split(':')
            .next()
            .ok_or("Invalid time format")?
            .parse()
            .map_err(|e| format!("Invalid hour in digest time: {}", e))?;

        // Validate hour is between 0-23
        if digest_hour > 23 {
            tracing::error!("Invalid hour value (must be 0-23): {}", digest_hour);
            return Ok(());
        }
            
        // Compare current hour with digest hour
        if now.hour() == digest_hour {
            // Calculate hours until next digest
            let hours_to_next = match (day_digest.as_ref(), evening_digest.as_ref()) {
                (Some(day), _) => {
                    let day_hour: u32 = day.split(':').next().unwrap_or("12").parse().unwrap_or(12);
                    hours_until(digest_hour, day_hour)
                },
                (None, Some(evening)) => {
                    let evening_hour: u32 = evening.split(':').next().unwrap_or("18").parse().unwrap_or(18);
                    hours_until(digest_hour, evening_hour)
                },
                (None, None) => {
                    // If no other digests, calculate hours until midnight
                    hours_until(digest_hour, 0)
                }
            };

            // Calculate hours since previous digest
            let hours_since_prev = match evening_digest.as_ref() {
                Some(evening) => {
                    let evening_hour: u32 = evening.split(':').next().unwrap_or("18").parse().unwrap_or(18);
                    hours_since(digest_hour, evening_hour)
                },
                None => {
                    // If no evening digest, calculate hours since midnight
                    hours_since(digest_hour, 0)
                }
            };

            // Format start time (now) and end time (now + hours_to_next) in RFC3339
            let start_time = now.with_timezone(&Utc).to_rfc3339();
            let end_time = (now + Duration::hours(hours_to_next as i64)).with_timezone(&Utc).to_rfc3339();

            // Check if user has active Google Calendar before fetching events
            let calendar_events = if state.user_repository.has_active_google_calendar()? {
                match crate::handlers::google_calendar::handle_calendar_fetching(state.as_ref(), &start_time, &end_time).await {
                    Ok(axum::Json(value)) => {
                        if let Some(events) = value.get("events").and_then(|e| e.as_array()) {
                            events.iter().filter_map(|event| {
                                let summary = event.get("summary")?.as_str()?.to_string();
                                let start = event.get("start")?.as_str()?.parse().ok()?;
                                let duration_minutes = event.get("duration_minutes")?.as_str()?.parse().ok()?;
                                Some(CalendarEvent {
                                    title: summary,
                                    start_time_rfc: start,
                                    duration_minutes,
                                })
                            }).collect()
                        } else {
                            Vec::new()
                        }
                    },
                    Err(_) => Vec::new(),
                }
            } else {
                Vec::new()
            };

            // Calculate the time range for message fetching
            let now = Utc::now();
            let cutoff_time = now - Duration::hours(hours_since_prev as i64);
            let start_timestamp = cutoff_time.timestamp();
            
            // Check if user has IMAP credentials before fetching emails
            let mut messages = if state.user_repository.get_imap_credentials()?.is_some() {
                // Fetch and filter emails
                match crate::handlers::imap_handlers::fetch_emails_imap(state, false, Some(50), false, true).await {
                    Ok(emails) => {
                        emails.into_iter()
                            .filter(|email| {
                                // Filter emails based on timestamp
                                if let Some(date) = email.date {
                                    date >= cutoff_time
                                } else {
                                    false // Exclude emails without a timestamp
                                }
                            })
                            .map(|email| MessageInfo {
                                sender: email.from.unwrap_or_else(|| "Unknown sender".to_string()),
                                content: email.snippet.unwrap_or_else(|| "No content".to_string()),
                                timestamp_rfc: email.date_formatted.unwrap_or_else(|| "No Timestamp".to_string()),
                                platform: "email".to_string(),
                            })
                            .collect::<Vec<MessageInfo>>()
                    },
                    Err(e) => {
                        tracing::error!("Failed to fetch emails for digest: {:#?}", e);
                        Vec::new()
                    }
                }
            } else {
                tracing::debug!("Skipping email fetch - user has no IMAP credentials configured");
                Vec::new()
            };

            // Log the number of filtered email messages
            tracing::debug!(
                "Filtered {} email messages from the last {} hours for digest",
                messages.len(),
                hours_since_prev
            );

            // Fetch WhatsApp messages
            if let Some(bridge) = state.user_repository.get_bridge("whatsapp")? {
                match crate::utils::bridge::fetch_bridge_messages("whatsapp", state, start_timestamp, true).await {
                    Ok(whatsapp_messages) => {
                        // Convert WhatsAppMessage to MessageInfo and add to messages
                        let whatsapp_infos: Vec<MessageInfo> = whatsapp_messages.into_iter()
                            .map(|msg| MessageInfo {
                                sender: msg.room_name,
                                content: msg.content,
                                timestamp_rfc: msg.formatted_timestamp,
                                platform: "whatsapp".to_string(),
                            })
                            .collect();
                        
                        tracing::debug!(
                            "Fetched {} WhatsApp messages from the last {} hours for digest",
                            whatsapp_infos.len(),
                            hours_since_prev
                        );

                        // Extend messages with WhatsApp messages
                        messages.extend(whatsapp_infos);

                        // Sort all messages by timestamp (most recent first)
                        messages.sort_by(|a, b| b.timestamp_rfc.cmp(&a.timestamp_rfc));
                    }
                    Err(e) => {
                        tracing::error!("Failed to fetch WhatsApp messages for digest: {}", e);
                    }
                }
            }

            // Fetch Telegram messages
            if let Some(bridge) = state.user_repository.get_bridge("telegram")? {
                match crate::utils::bridge::fetch_bridge_messages("telegram", state, start_timestamp, true).await {
                    Ok(telegram_messages) => {
                        // Convert TelegramMessage to MessageInfo and add to messages
                        let telegram_infos: Vec<MessageInfo> = telegram_messages.into_iter()
                            .map(|msg| MessageInfo {
                                sender: msg.room_name,
                                content: msg.content,
                                timestamp_rfc: msg.formatted_timestamp,
                                platform: "telegram".to_string(),
                            })
                            .collect();
                        
                        tracing::debug!(
                            "Fetched {} Telegram messages from the last {} hours for digest",
                            telegram_infos.len(),
                            hours_since_prev
                        );

                        // Extend messages with Telegram messages
                        messages.extend(telegram_infos);

                        // Sort all messages by timestamp (most recent first)
                        messages.sort_by(|a, b| b.timestamp_rfc.cmp(&a.timestamp_rfc));
                    }
                    Err(e) => {
                        tracing::error!("Failed to fetch Telegram messages for digest: {}", e);
                    }
                }
            }

            // Fetch Signal messages
            if let Some(bridge) = state.user_repository.get_bridge("signal")? {
                match crate::utils::bridge::fetch_bridge_messages("signal", state, start_timestamp, true).await {
                    Ok(signal_messages) => {
                        // Convert Signal Message to MessageInfo and add to messages
                        let signal_infos: Vec<MessageInfo> = signal_messages.into_iter()
                            .map(|msg| MessageInfo {
                                sender: msg.room_name,
                                content: msg.content,
                                timestamp_rfc: msg.formatted_timestamp,
                                platform: "signal".to_string(),
                            })
                            .collect();
                        
                        tracing::debug!(
                            "Fetched {} Signal messages from the last {} hours for digest",
                            signal_infos.len(),
                            hours_since_prev
                        );

                        // Extend messages with Signal messages
                        messages.extend(signal_infos);

                        // Sort all messages by timestamp (most recent first)
                        messages.sort_by(|a, b| b.timestamp_rfc.cmp(&a.timestamp_rfc));
                    }
                    Err(e) => {
                        tracing::error!("Failed to fetch Signal messages for digest: {}", e);
                    }
                }
            }

            // Log total number of messages
            tracing::debug!(
                "Total {} messages collected for digest",
                messages.len()
            );

            // return if no new nothing
            if messages.is_empty() && calendar_events.is_empty() {
                return Ok(());
            }

            // Fetch priority senders for each platform and build a lookup map
            let mut priority_map: HashMap<String, HashSet<String>> = HashMap::new();
            for platform in ["email", "whatsapp", "telegram", "signal"] {
                let priors = state.user_repository.get_priority_senders(platform).unwrap_or(Vec::new());
                let set: HashSet<String> = priors.into_iter().map(|p| p.sender).collect();
                if !set.is_empty() {
                    tracing::debug!("Loaded {} priority senders for {}", set.len(), platform);
                }
                priority_map.insert(platform.to_string(), set);
            }

            messages.sort_by(|a, b| {
                let plat_cmp = a.platform.cmp(&b.platform);
                if plat_cmp == std::cmp::Ordering::Equal {
                    let a_pri = priority_map.get(&a.platform).map_or(false, |set| set.contains(&a.sender));
                    let b_pri = priority_map.get(&b.platform).map_or(false, |set| set.contains(&b.sender));
                    b_pri.cmp(&a_pri).then_with(|| b.timestamp_rfc.cmp(&a.timestamp_rfc))
                } else {
                    plat_cmp
                }
            });



            // Prepare digest data
            let digest_data = DigestData {
                messages,
                calendar_events,
                time_period_hours: hours_to_next,
            };

            // Generate the digest
            let digest_message = match generate_digest(&state, digest_data, priority_map).await {
                Ok(digest) => format!("Good morning! {}",digest),
                Err(_) => format!(
                    "Good morning! Here's your morning digest covering the last {} hours. Next digest in {} hours.",
                    hours_since_prev, hours_to_next
                ),
            };
                
            tracing::info!("Sending morning digest for user at {}:00 in timezone {}", 
                digest_hour, timezone);
                
            send_notification(
                state,
                &digest_message,
                "morning_digest".to_string(),
                Some("Good morning! Want to hear your morning digest?".to_string()),
            ).await;
        }
    }
    
    Ok(())
}

pub async fn check_day_digest(state: &Arc<AppState>) -> Result<(), Box<dyn std::error::Error>> {
    // Get the user's digest settings and timezone
    let (morning_digest, day_digest, evening_digest) = state.user_core.get_digests()?;
    let user_info = state.user_core.get_user_info()?;
    
    // If day digest is enabled (Some value) and we have a timezone, check the time
    if let (Some(digest_hour_str), Some(timezone)) = (day_digest.clone(), user_info.timezone) {
        // Parse the timezone
        let tz: chrono_tz::Tz = timezone.parse()
            .map_err(|e| format!("Invalid timezone: {}", e))?;
            
        // Get current time in user's timezone
        let now = chrono::Utc::now().with_timezone(&tz);
        
        // Parse the digest hour (expected format: "HH:00" like "00:00", "23:00")
        let digest_hour: u32 = digest_hour_str
            .split(':')
            .next()
            .ok_or("Invalid time format")?
            .parse()
            .map_err(|e| format!("Invalid hour in digest time: {}", e))?;

        // Validate hour is between 0-23
        if digest_hour > 23 {
            tracing::error!("Invalid hour value (must be 0-23): {}", digest_hour);
            return Ok(());
        }
            
        // Compare current hour with digest hour
        if now.hour() == digest_hour {
            // Calculate hours until next digest
            let hours_to_next = match evening_digest.as_ref() {
                Some(evening) => {
                    let evening_hour: u32 = evening.split(':').next().unwrap_or("0").parse().unwrap_or(0);
                    hours_until(digest_hour, evening_hour)
                },
                None => {
                    // If no other digests, calculate hours until evening
                    hours_until(digest_hour, 0)
                }
            };

            // Calculate hours since previous digest
            let hours_since_prev = match morning_digest.as_ref() {
                Some(morning) => {
                    let morning_hour: u32 = morning.split(':').next().unwrap_or("6").parse().unwrap_or(6);
                    hours_since(digest_hour, morning_hour)
                },
                None => {
                    // If no morning digest, calculate hours since 6 o'clock 
                    hours_since(digest_hour, 6)
                }
            };

            // Format start time (now) and end time (now + hours_to_next) in RFC3339
            let start_time = now.with_timezone(&Utc).to_rfc3339();
            let end_time = (now + Duration::hours(hours_to_next as i64)).with_timezone(&Utc).to_rfc3339();

            // Fetch calendar events for the period
            let calendar_events = if state.user_repository.has_active_google_calendar()? {
                match crate::handlers::google_calendar::handle_calendar_fetching(state.as_ref(), &start_time, &end_time).await {
                    Ok(axum::Json(value)) => {
                        if let Some(events) = value.get("events").and_then(|e| e.as_array()) {
                            events.iter().filter_map(|event| {
                                let summary = event.get("summary")?.as_str()?.to_string();
                                let start = event.get("start")?.as_str()?.parse().ok()?;
                                let duration_minutes = event.get("duration_minutes")?.as_str()?.parse().ok()?;
                                Some(CalendarEvent {
                                    title: summary,
                                    start_time_rfc: start,
                                    duration_minutes,
                                })
                            }).collect()
                        } else {
                            Vec::new()
                        }
                    },
                    Err(_) => Vec::new(),
                }
            } else {
                Vec::new()
            };

            // Calculate the time range for message fetching
            let now = Utc::now();
            let cutoff_time = now - Duration::hours(hours_since_prev as i64);
            let start_timestamp = cutoff_time.timestamp();
            
            // Check if user has IMAP credentials before fetching emails
            let mut messages = if state.user_repository.get_imap_credentials()?.is_some() {
                // Fetch and filter emails
                match crate::handlers::imap_handlers::fetch_emails_imap(state, false, Some(50), false, true).await {
                    Ok(emails) => {
                        emails.into_iter()
                            .filter(|email| {
                                // Filter emails based on timestamp
                                if let Some(date) = email.date {
                                    date >= cutoff_time
                                } else {
                                    false // Exclude emails without a timestamp
                                }
                            })
                            .map(|email| MessageInfo {
                                sender: email.from.unwrap_or_else(|| "Unknown sender".to_string()),
                                content: email.snippet.unwrap_or_else(|| "No content".to_string()),
                                timestamp_rfc: email.date_formatted.unwrap_or_else(|| "No Timestamp".to_string()),
                                platform: "email".to_string(),
                            })
                            .collect::<Vec<MessageInfo>>()
                    },
                    Err(e) => {
                        tracing::error!("Failed to fetch emails for digest: {:#?}", e);
                        Vec::new()
                    }
                }
            } else {
                tracing::debug!("Skipping email fetch - user has no IMAP credentials configured");
                Vec::new()
            };

            // Log the number of filtered email messages
            tracing::debug!(
                "Filtered {} email messages from the last {} hours for digest",
                messages.len(),
                hours_since_prev
            );

            // Fetch WhatsApp messages
            if let Some(bridge) = state.user_repository.get_bridge("whatsapp")? {
                match crate::utils::bridge::fetch_bridge_messages("whatsapp", state, start_timestamp, true).await {
                    Ok(whatsapp_messages) => {
                        // Convert WhatsAppMessage to MessageInfo and add to messages
                        let whatsapp_infos: Vec<MessageInfo> = whatsapp_messages.into_iter()
                            .map(|msg| MessageInfo {
                                sender: msg.room_name,
                                content: msg.content,
                                timestamp_rfc: msg.formatted_timestamp,
                                platform: "whatsapp".to_string(),
                            })
                            .collect();
                        
                        tracing::debug!(
                            "Fetched {} WhatsApp messages from the last {} hours for digest",
                            whatsapp_infos.len(),
                            hours_since_prev
                        );

                        // Extend messages with WhatsApp messages
                        messages.extend(whatsapp_infos);

                        // Sort all messages by timestamp (most recent first)
                        messages.sort_by(|a, b| b.timestamp_rfc.cmp(&a.timestamp_rfc));
                    }
                    Err(e) => {
                        tracing::error!("Failed to fetch WhatsApp messages for digest: {}", e);
                    }
                }
            }

            // Fetch Telegram messages
            if let Some(bridge) = state.user_repository.get_bridge("telegram")? {
                match crate::utils::bridge::fetch_bridge_messages("telegram", state, start_timestamp, true).await {
                    Ok(telegram_messages) => {
                        // Convert TelegramMessage to MessageInfo and add to messages
                        let telegram_infos: Vec<MessageInfo> = telegram_messages.into_iter()
                            .map(|msg| MessageInfo {
                                sender: msg.room_name,
                                content: msg.content,
                                timestamp_rfc: msg.formatted_timestamp,
                                platform: "telegram".to_string(),
                            })
                            .collect();
                        
                        tracing::debug!(
                            "Fetched {} Telegram messages from the last {} hours for digest",
                            telegram_infos.len(),
                            hours_since_prev
                        );
                        // Extend messages with Telegram messages
                        messages.extend(telegram_infos);

                        // Sort all messages by timestamp (most recent first)
                        messages.sort_by(|a, b| b.timestamp_rfc.cmp(&a.timestamp_rfc));
                    }
                    Err(e) => {
                        tracing::error!("Failed to fetch Telegram messages for digest: {}", e);
                    }
                }
            }

            // Fetch Signal messages
            if let Some(bridge) = state.user_repository.get_bridge("signal")? {
                match crate::utils::bridge::fetch_bridge_messages("signal", state, start_timestamp, true).await {
                    Ok(signal_messages) => {
                        // Convert Signal Message to MessageInfo and add to messages
                        let signal_infos: Vec<MessageInfo> = signal_messages.into_iter()
                            .map(|msg| MessageInfo {
                                sender: msg.room_name,
                                content: msg.content,
                                timestamp_rfc: msg.formatted_timestamp,
                                platform: "signal".to_string(),
                            })
                            .collect();
                        
                        tracing::debug!(
                            "Fetched {} Signal messages from the last {} hours for digest",
                            signal_infos.len(),
                            hours_since_prev
                        );

                        // Extend messages with Signal messages
                        messages.extend(signal_infos);

                        // Sort all messages by timestamp (most recent first)
                        messages.sort_by(|a, b| b.timestamp_rfc.cmp(&a.timestamp_rfc));
                    }
                    Err(e) => {
                        tracing::error!("Failed to fetch Signal messages for digest: {}", e);
                    }
                }
            }

            // Log total number of messages
            tracing::debug!(
                "Total {} messages collected for digest",
                messages.len()
            );

            // return if no new nothing
            if messages.is_empty() && calendar_events.is_empty() {
                return Ok(());
            }

            // Fetch priority senders for each platform and build a lookup map
            let mut priority_map: HashMap<String, HashSet<String>> = HashMap::new();
            for platform in ["email", "whatsapp", "telegram", "signal"] {
                let priors = state.user_repository.get_priority_senders(platform).unwrap_or(Vec::new());
                let set: HashSet<String> = priors.into_iter().map(|p| p.sender).collect();
                if !set.is_empty() {
                    tracing::debug!("Loaded {} priority senders for {}", set.len(), platform);
                }
                priority_map.insert(platform.to_string(), set);
            }

            messages.sort_by(|a, b| {
                let plat_cmp = a.platform.cmp(&b.platform);
                if plat_cmp == std::cmp::Ordering::Equal {
                    let a_pri = priority_map.get(&a.platform).map_or(false, |set| set.contains(&a.sender));
                    let b_pri = priority_map.get(&b.platform).map_or(false, |set| set.contains(&b.sender));
                    b_pri.cmp(&a_pri).then_with(|| b.timestamp_rfc.cmp(&a.timestamp_rfc))
                } else {
                    plat_cmp
                }
            });


            // Prepare digest data
            let digest_data = DigestData {
                messages,
                calendar_events,
                time_period_hours: hours_to_next,
            };

            // Generate the digest
            let digest_message = match generate_digest(&state, digest_data, priority_map).await {
                Ok(digest) => format!("Hello! {}",digest),
                Err(_) => format!(
                    "Hello! Here's your daily digest covering the last {} hours. Next digest in {} hours.",
                    hours_since_prev, hours_to_next
                ),
            };
                
            tracing::info!("Sending day digest for user at {}:00 in timezone {}", 
                digest_hour, timezone);
                
            send_notification(
                state,
                &digest_message,
                "day_digest".to_string(),
                Some("Hello! Want to hear your daily digest?".to_string()),
            ).await;
        }
    }
    
    Ok(())
}

pub async fn check_evening_digest(state: &Arc<AppState>) -> Result<(), Box<dyn std::error::Error>> {
    // Get the user's digest settings and timezone
    let (morning_digest, day_digest, evening_digest) = state.user_core.get_digests()?;
    let user_info = state.user_core.get_user_info()?;
    
    // If morning digest is enabled (Some value) and we have a timezone, check the time
    if let (Some(digest_hour_str), Some(timezone)) = (evening_digest.clone(), user_info.timezone) {
        // Parse the timezone
        let tz: chrono_tz::Tz = timezone.parse()
            .map_err(|e| format!("Invalid timezone: {}", e))?;
            
        // Get current time in user's timezone
        let now = chrono::Utc::now().with_timezone(&tz);
        
        // Parse the digest hour (expected format: "HH:00" like "00:00", "23:00")
        let digest_hour: u32 = digest_hour_str
            .split(':')
            .next()
            .ok_or("Invalid time format")?
            .parse()
            .map_err(|e| format!("Invalid hour in digest time: {}", e))?;

        // Validate hour is between 0-23
        if digest_hour > 23 {
            tracing::error!("Invalid hour value (must be 0-23): {}", digest_hour);
            return Ok(());
        }
            
        // Compare current hour with digest hour
        if now.hour() == digest_hour {
            // Calculate hours until next digest
            let hours_to_next = match morning_digest.as_ref() {
                Some(morning) => {
                    let morning_hour: u32 = morning.split(':').next().unwrap_or("8").parse().unwrap_or(8);
                    hours_until(digest_hour, morning_hour)
                },
                None => {
                    // If no other digests, calculate hours until morning
                    hours_until(digest_hour, 8)
                }
            };

            // Calculate hours since previous digest
            let hours_since_prev = match day_digest.as_ref() {
                Some(day) => {
                    let day_hour: u32 = day.split(':').next().unwrap_or("12").parse().unwrap_or(12);
                    hours_since(digest_hour, day_hour)
                },
                None => {
                    // If no morning digest, calculate hours since 6 o'clock 
                    hours_since(digest_hour, 12)
                }
            };


            let tz: chrono_tz::Tz = timezone.parse()
                .map_err(|e| format!("Invalid timezone: {}", e))?;

            // Format start time (now) and end time (now + hours_to_next) in RFC3339
            let start_time = now.with_timezone(&tz).to_rfc3339();


            // Calculate end of tomorrow
            let tomorrow_end = now.date_naive().succ_opt() // Get tomorrow's date
                .unwrap_or(now.date_naive()) // Fallback to today if overflow
                .and_hms_opt(23, 59, 59) // Set to end of day
                .unwrap_or(now.naive_local()) // Fallback to now if invalid time
                .and_local_timezone(tz)
                .earliest() // Get the earliest possible time if ambiguous
                .unwrap_or(now); // Fallback to now if conversion fails
            
            let end_time = tomorrow_end.with_timezone(&Utc).to_rfc3339();

            // Check if user has active Google Calendar before fetching events
            let calendar_events = if state.user_repository.has_active_google_calendar()? {
                match crate::handlers::google_calendar::handle_calendar_fetching(state.as_ref(), &start_time, &end_time).await {
                    Ok(axum::Json(value)) => {
                        if let Some(events) = value.get("events").and_then(|e| e.as_array()) {
                            events.iter().filter_map(|event| {
                                let summary = event.get("summary")?.as_str()?.to_string();
                                let start = event.get("start")?.as_str()?.parse().ok()?;
                                let duration_minutes = event.get("duration_minutes")?.as_str()?.parse().ok()?;
                                Some(CalendarEvent {
                                    title: summary,
                                    start_time_rfc: start,
                                    duration_minutes,
                                })
                            }).collect()
                        } else {
                            Vec::new()
                        }
                    },
                    Err(_) => Vec::new(),
                }
            } else {
                Vec::new()
            };

            // Calculate the time range for message fetching
            let now = Utc::now();
            let cutoff_time = now - Duration::hours(hours_since_prev as i64);
            let start_timestamp = cutoff_time.timestamp();
            
            // Check if user has IMAP credentials before fetching emails
            let mut messages = if state.user_repository.get_imap_credentials()?.is_some() {
                // Fetch and filter emails
                match crate::handlers::imap_handlers::fetch_emails_imap(state, false, Some(50), false, true).await {
                    Ok(emails) => {
                        emails.into_iter()
                            .filter(|email| {
                                // Filter emails based on timestamp
                                if let Some(date) = email.date {
                                    date >= cutoff_time
                                } else {
                                    false // Exclude emails without a timestamp
                                }
                            })
                            .map(|email| MessageInfo {
                                sender: email.from.unwrap_or_else(|| "Unknown sender".to_string()),
                                content: email.snippet.unwrap_or_else(|| "No content".to_string()),
                                timestamp_rfc: email.date_formatted.unwrap_or_else(|| "No Timestamp".to_string()),
                                platform: "email".to_string(),
                            })
                            .collect::<Vec<MessageInfo>>()
                    },
                    Err(e) => {
                        tracing::error!("Failed to fetch emails for digest: {:#?}", e);
                        Vec::new()
                    }
                }
            } else {
                tracing::debug!("Skipping email fetch - user has no IMAP credentials configured");
                Vec::new()
            };

            // Log the number of filtered email messages
            tracing::debug!(
                "Filtered {} email messages from the last {} hours for digest",
                messages.len(),
                hours_since_prev
            );

            // Fetch WhatsApp messages
            if let Some(bridge) = state.user_repository.get_bridge("whatsapp")? {
                match crate::utils::bridge::fetch_bridge_messages("whatsapp", state, start_timestamp, true).await {
                    Ok(whatsapp_messages) => {
                        // Convert WhatsAppMessage to MessageInfo and add to messages
                        let whatsapp_infos: Vec<MessageInfo> = whatsapp_messages.into_iter()
                            .map(|msg| MessageInfo {
                                sender: msg.room_name,
                                content: msg.content,
                                timestamp_rfc: msg.formatted_timestamp,
                                platform: "whatsapp".to_string(),
                            })
                            .collect();
                        
                        tracing::debug!(
                            "Fetched {} WhatsApp messages from the last {} hours for digest",
                            whatsapp_infos.len(),
                            hours_since_prev
                        );

                        // Extend messages with WhatsApp messages
                        messages.extend(whatsapp_infos);

                        // Sort all messages by timestamp (most recent first)
                        messages.sort_by(|a, b| b.timestamp_rfc.cmp(&a.timestamp_rfc));
                    }
                    Err(e) => {
                        tracing::error!("Failed to fetch WhatsApp messages for digest: {}", e);
                    }
                }
            }

            // Fetch Telegram messages
            if let Some(bridge) = state.user_repository.get_bridge("telegram")? {
                match crate::utils::bridge::fetch_bridge_messages("telegram", state, start_timestamp, true).await {
                    Ok(telegram_messages) => {
                        // Convert Telegram to MessageInfo and add to messages
                        let telegram_infos: Vec<MessageInfo> = telegram_messages.into_iter()
                            .map(|msg| MessageInfo {
                                sender: msg.room_name,
                                content: msg.content,
                                timestamp_rfc: msg.formatted_timestamp,
                                platform: "telegram".to_string(),
                            })
                            .collect();
                        
                        tracing::debug!(
                            "Fetched {} Telegram messages from the last {} hours for digest",
                            telegram_infos.len(),
                            hours_since_prev
                        );

                        // Extend messages with Telegram messages
                        messages.extend(telegram_infos);

                        // Sort all messages by timestamp (most recent first)
                        messages.sort_by(|a, b| b.timestamp_rfc.cmp(&a.timestamp_rfc));
                    }
                    Err(e) => {
                        tracing::error!("Failed to fetch Telegram messages for digest: {}", e);
                    }
                }
            }

            // Fetch Signal messages
            if let Some(bridge) = state.user_repository.get_bridge("signal")? {
                match crate::utils::bridge::fetch_bridge_messages("signal", state, start_timestamp, true).await {
                    Ok(signal_messages) => {
                        // Convert Signal Message to MessageInfo and add to messages
                        let signal_infos: Vec<MessageInfo> = signal_messages.into_iter()
                            .map(|msg| MessageInfo {
                                sender: msg.room_name,
                                content: msg.content,
                                timestamp_rfc: msg.formatted_timestamp,
                                platform: "signal".to_string(),
                            })
                            .collect();
                        
                        tracing::debug!(
                            "Fetched {} Signal messages from the last {} hours for digest",
                            signal_infos.len(),
                            hours_since_prev
                        );

                        // Extend messages with Signal messages
                        messages.extend(signal_infos);

                        // Sort all messages by timestamp (most recent first)
                        messages.sort_by(|a, b| b.timestamp_rfc.cmp(&a.timestamp_rfc));
                    }
                    Err(e) => {
                        tracing::error!("Failed to fetch Signal messages for digest: {}", e);
                    }
                }
            }

            // Log total number of messages
            tracing::debug!(
                "Total {} messages collected for digest",
                messages.len()
            );

            // return if no new nothing
            if messages.is_empty() && calendar_events.is_empty() {
                return Ok(());
            }

                        // Fetch priority senders for each platform and build a lookup map
            let mut priority_map: HashMap<String, HashSet<String>> = HashMap::new();
            for platform in ["email", "whatsapp", "telegram", "signal"] {
                let priors = state.user_repository.get_priority_senders(platform).unwrap_or(Vec::new());
                let set: HashSet<String> = priors.into_iter().map(|p| p.sender).collect();
                if !set.is_empty() {
                    tracing::debug!("Loaded {} priority senders for {}", set.len(), platform);
                }
                priority_map.insert(platform.to_string(), set);
            }

            messages.sort_by(|a, b| {
                let plat_cmp = a.platform.cmp(&b.platform);
                if plat_cmp == std::cmp::Ordering::Equal {
                    let a_pri = priority_map.get(&a.platform).map_or(false, |set| set.contains(&a.sender));
                    let b_pri = priority_map.get(&b.platform).map_or(false, |set| set.contains(&b.sender));
                    b_pri.cmp(&a_pri).then_with(|| b.timestamp_rfc.cmp(&a.timestamp_rfc))
                } else {
                    plat_cmp
                }
            });


            // Prepare digest data
            let digest_data = DigestData {
                messages,
                calendar_events,
                time_period_hours: hours_to_next,
            };

            // Generate the digest
            let digest_message = match generate_digest(&state, digest_data, priority_map).await {
                Ok(digest) => format!("Good evening! {}",digest),
                Err(_) => format!(
                    "Hello! Here's your evening digest covering the last {} hours. Next digest in {} hours.",
                    hours_since_prev, hours_to_next
                ),
            };
                
            tracing::info!("Sending evening digest for user at {}:00 in timezone {}", 
                digest_hour, timezone);
                
            send_notification(
                state,
                &digest_message,
                "evening_digest".to_string(),
                Some("Good evening! Want to hear your evening digest?".to_string()),
            ).await;
        }
    }
    
    Ok(())
}

const DIGEST_PROMPT: &str = r#"You are an AI called lightfriend that creates concise SMS digests of messages and calendar events. Your goal is to help users stay on top of unread messages and upcoming calendar events without needing to open their apps. Group items by platform (e.g., WHATSAPP:, EMAIL:, CALENDAR:), starting each group on a new line. Within each group, provide clear teasers for critical or prioritized items (e.g., sender, topic hint, timestamp in parentheses), separating them with commas or '+' for brevity. Summarize less urgent or grouped items at the end of the group with '+' (e.g., '+ other routine items from xai, claude, ..'). Adjust detail based on overall content: if low volume or mostly low-criticality, expand critical items with fuller, detailed teasers (e.g., key excerpts or actions) to avoid follow-ups. For high volume or non-critical items, use minimal teasers. Highlight critical/actionable items with more specific hints to reduce follow-ups, but avoid full content. Cover all items concisely without omissions.
Rules
• Absolute length limit: 480 characters.
• Do NOT use markdown (no *, **, _, links, or backticks).
• Do NOT use emojis or emoticons.
• Plain text only.
• Start each platform group on a new line, followed by ': ' and the teasers/summaries.
• Messages marked with [PRIORITY] are from user-defined priority senders. Always put them first in their platform group, highlight them with more detailed teasers (e.g., key excerpts, actions, or urgency hints), and treat them as critical/actionable to minimize user follow-ups.
• Put critical or prioritized items first within each group.
• Include timestamps in parentheses (e.g., '(yesterday 8pm)') for relevance.
• For calendar, include events in the next 24 hours with start time and brief hint.
• Tease naturally, e.g., 'Mom suggested dinner in family chat (yesterday 8pm)'.
Return JSON with a single field:
• `digest` – the plain-text SMS message, with newlines separating groups.
"#;
pub async fn generate_digest(
    state: &Arc<AppState>,
    data: DigestData,
    priority_map: HashMap<String, HashSet<String>>,
) -> Result<String, Box<dyn std::error::Error>> {
    let client = create_openai_client(&state)?;
    // Format messages for the prompt
    let messages_str = data.messages
        .iter()
        .map(|msg| {
            let priority_tag = if priority_map.get(&msg.platform).map_or(false, |set| set.contains(&msg.sender)) {
                " [PRIORITY]".to_string()
            } else {
                String::new()
            };
            format!(
                "- [{}] {} on {}: {}{}",
                msg.platform.to_uppercase(),
                msg.sender,
                msg.timestamp_rfc,
                msg.content,
                priority_tag,
            )
        })
        .collect::<Vec<String>>()
        .join("\n");
    // Format calendar events for the prompt
    let events_str = data.calendar_events
        .iter()
        .map(|event| {
            format!(
                "- {} at {} lasting {} minutes",
                event.title,
                event.start_time_rfc,
                event.duration_minutes,
            )
        })
        .collect::<Vec<String>>()
        .join("\n");
    // Conditionally include calendar section only if there are events
    let user_content = if data.calendar_events.is_empty() {
        format!(
            "Create a digest covering the last {} hours.\n\nMessages:\n{}",
            data.time_period_hours, messages_str
        )
    } else {
        format!(
            "Create a digest covering the last {} hours.\n\nMessages:\n{}\n\nUpcoming calendar events:\n{}",
            data.time_period_hours, messages_str, events_str
        )
    };
    let messages = vec![
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::system,
            content: chat_completion::Content::Text(DIGEST_PROMPT.to_string()),
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
    let mut properties = std::collections::HashMap::new();
    properties.insert(
        "digest".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The SMS-friendly digest message".to_string()),
            ..Default::default()
        }),
    );
    let tools = vec![chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("create_digest"),
            description: Some(String::from(
                "Creates a concise digest of messages and calendar events"
            )),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(vec![
                    String::from("digest"),
                ]),
            },
        },
    }];
    let request = chat_completion::ChatCompletionRequest::new(
        GPT4_O.to_string(),
        messages,
    )
    .tools(tools)
    .tool_choice(chat_completion::ToolChoiceType::Required)
    .max_tokens(200);
    match client.chat_completion(request).await {
        Ok(result) => {
            if let Some(tool_calls) = result.choices[0].message.tool_calls.as_ref() {
                if let Some(first_call) = tool_calls.first() {
                    if let Some(args) = &first_call.function.arguments {
                        #[derive(Debug, Deserialize)]
                        struct DigestResponse {
                            digest: String,
                        }
                        match serde_json::from_str::<DigestResponse>(args) {
                            Ok(response) => {
                                tracing::debug!(
                                    "Generated digest: {}",
                                    response.digest
                                );
                                Ok(response.digest)
                            }
                            Err(e) => {
                                tracing::error!("Failed to parse digest response: {}", e);
                                Ok("Failed to generate digest(parse error).".to_string())
                            }
                        }
                    } else {
                        tracing::error!("No arguments found in tool call");
                        Ok("Failed to generate digest(arguments missing).".to_string())
                    }
                } else {
                    tracing::error!("No tool calls found");
                    Ok("Failed to generate digest(no first tool call).".to_string())
                }
            } else {
                tracing::error!("No tool calls section in response");
                Ok("Failed to generate digest(no tool calls).".to_string())
            }
        }
        Err(e) => {
            tracing::error!("Failed to generate digest: {}", e);
            Err(e.into())
        }
    }
}

pub async fn send_notification(
    state: &Arc<AppState>,
    notification: &str,
    content_type: String,
    first_message: Option<String>,
) {
    // Get current timestamp for message history
    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;
    // Get user info
    let user = match state.user_core.get_user() {
        Ok(Some(user)) => user,
        Ok(None) => {
            tracing::error!("User not found for notification");
            return;
        }
        Err(e) => {
            tracing::error!("Failed to get user: {}", e);
            return;
        }
    };

    // Get user settings (assuming state has a user_settings repository or similar)
    let user_settings = match state.user_core.get_user_settings() {
        Ok(settings) => settings,
        Err(e) => {
            tracing::error!("Failed to get settings for user: {}", e);
            return;
        }
    };

    let user_info = match state.user_core.get_user_info() {
        Ok(info) => info,
        Err(e) => {
            tracing::error!("Failed to get info for user: {}", e);
            return;
        }
    };

    // Check user's notification preference from settings
    let notification_type = if content_type.contains("critical") {
        user_settings.critical_enabled.as_deref().unwrap_or("sms")
    } else if content_type.contains("_call") {
        "call"
    } else if content_type.contains("_sms") {
        "sms"
    } else {
        user_settings.notification_type.as_deref().unwrap_or("sms")
    };


    match notification_type {
        "call" => {

            // Create dynamic variables (optional, can be customized based on needs)
            let dynamic_vars = std::collections::HashMap::new();

            match crate::api::elevenlabs::make_notification_call(
                &state.clone(),
                content_type.clone(), // Notification type
                first_message.clone().unwrap_or("Hello, I have a critical notification to tell you about".to_string()),
                notification.to_string(),
                user_info.timezone,
            ).await {
                Ok(mut response) => {
                    // Add dynamic variables to the client data
                    if let Some(client_data) = response.get_mut("client_data") {
                        if let Some(obj) = client_data.as_object_mut() {
                            obj.extend(dynamic_vars.into_iter().map(|(k, v)| (k, serde_json::Value::String(v))));
                        }
                    }
                    tracing::debug!("Successfully initiated call notification for user {}", user.id);
                    
                    // Store notification in message history
                    let assistant_notification = crate::models::user_models::NewMessageHistory {
                        user_id: user.id,
                        role: "assistant".to_string(),
                        encrypted_content: notification.to_string(),
                        tool_name: None,
                        tool_call_id: None,
                        tool_calls_json: None,
                        created_at: current_time,
                        conversation_id: "".to_string(),
                    };

                    if let Err(e) = state.user_repository.create_message_history(&assistant_notification) {
                        tracing::error!("Failed to store notification in history: {}", e);
                    }

                }
                Err((_, json_err)) => {
                    tracing::error!("Failed to initiate call notification: {:?}", json_err);
                    println!("Failed to send call notification for user");
                }
            }
        }
        _ => {

            match crate::api::twilio_utils::send_conversation_message(
                &state,
                &notification,
                None,
                &user,
            ).await {
                Ok(response_sid) => {
                    tracing::info!("Successfully sent notification to user");
                    println!("SMS notification sent successfully for user");
                    
                    // Store notification in message history
                    let assistant_notification = crate::models::user_models::NewMessageHistory {
                        user_id: user.id,
                        role: "assistant".to_string(),
                        encrypted_content: notification.to_string(),
                        tool_name: None,
                        tool_call_id: None,
                        tool_calls_json: None,
                        created_at: current_time,
                        conversation_id: "".to_string(),
                    };

                    // Store message in history
                    if let Err(e) = state.user_repository.create_message_history(&assistant_notification) {
                        tracing::error!("Failed to store notification in history: {}", e);
                    }
                    
                }
                Err(e) => {
                    tracing::error!("Failed to send notification: {}", e);
                    println!("Failed to send SMS notification for user");
                }
            }
        }
    }
}
