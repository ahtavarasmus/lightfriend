use crate::models::user_models::WaitingCheck;
use crate::AppState;
use std::sync::Arc;
use openai_api_rs::v1::{
    chat_completion,
    types,
};
use chrono::Timelike;
use crate::tool_call_utils::utils::create_openai_client;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc, Duration};

#[derive(Debug, Serialize, Deserialize)]
pub struct DigestData {
    pub messages: Vec<MessageInfo>,
    pub calendar_events: Vec<CalendarEvent>,
    pub time_period_hours: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct MessageInfo {
    sender: String,
    content: String,
    timestamp: DateTime<Utc>,
    platform: String, // e.g., "email", "whatsapp", etc.
}

#[derive(Debug, Serialize, Deserialize)]
struct CalendarEvent {
    title: String,
    start_time: DateTime<Utc>,
    end_time: DateTime<Utc>,
    description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MatchResponse {
    waiting_check_id: Option<i32>,
    is_critical: bool,
    what_to_inform: String,
    first_message: String,
}

pub async fn check_message_importance(
    message: &str,
    waiting_checks: Vec<WaitingCheck>,
) -> Result<(Option<i32>, bool, String, String), Box<dyn std::error::Error>> {
    let client = create_openai_client()?;

    let waiting_checks_str = waiting_checks
        .iter()
        .map(|check| {
            format!(
                "ID: {}, Content to watch for: {}",
                check.id.unwrap_or(-1),
                check.content,
            )
        })
        .collect::<Vec<String>>()
        .join("\n");

    let messages = vec![
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::system,
            content: chat_completion::Content::Text(
                "You are an AI that analyzes messages to determine if they match any waiting checks or if they are otherwise critical and require immediate attention. A message is considered critical ONLY if it absolutely cannot wait to be mentioned in the next scheduled notification summary - it must be something that requires truly immediate attention like emergencies, extremely time-sensitive matters, or critical updates that would be problematic if delayed until the next summary. Most normal updates, even if important, should wait for the scheduled summary unless they are genuinely urgent and time-critical. When reporting critical messages:\n1. Provide an extremely concise SMS-friendly message (ideally under 160 characters) that clearly states what requires immediate attention\n2. Generate a brief, attention-grabbing first message (under 100 characters) that would be suitable as an AI voice assistant's opening line in a phone call about this update. This first message should give a quick context about the type of urgent matter without all the details.".to_string(),
            ),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::user,
            content: chat_completion::Content::Text(format!(
                "Analyze this message:\n\n{}\n\nAgainst these waiting checks:\n\n{}\n\nDetermine:\n1. If it matches any waiting check (return the ID if it does)\n2. If the message is otherwise critical and requires immediate attention",
                message, waiting_checks_str
            )),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
    ];

    let mut properties = std::collections::HashMap::new();
    properties.insert(
        "waiting_check_id".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Number),
            description: Some("The ID of the matched waiting check, if any. None if no match found.".to_string()),
            ..Default::default()
        }),
    );
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
            description: Some("Concise SMS-friendly message (under 160 chars) about what is it that requires immediate attention".to_string()),
            ..Default::default()
        }),
    );
    properties.insert(
        "first_message".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Brief, attention-grabbing first message (under 100 chars) suitable as an AI voice assistant's opening line in a phone call. Should give quick context about the type of urgent matter.".to_string()),
            ..Default::default()
        }),
    );

    let tools = vec![chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("analyze_message"),
            description: Some(String::from(
                "Analyzes if a message matches any waiting checks or is critical"
            )),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(vec![
                    String::from("is_critical"),
                    String::from("what_to_inform"),
                    String::from("first_message"),
                ]),
            },
        },
    }];

    let request = chat_completion::ChatCompletionRequest::new(
        "openai/gpt-4o-mini".to_string(),
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
                        match serde_json::from_str::<MatchResponse>(args) {
                            Ok(response) => {
                                tracing::debug!(
                                    "Message analysis result: check_id={:?}, critical={}, message={}, first_message={}",
                                    response.waiting_check_id,
                                    response.is_critical,
                                    response.what_to_inform,
                                    response.first_message
                                );
                                Ok((response.waiting_check_id, response.is_critical, response.what_to_inform, response.first_message))
                            }
                            Err(e) => {
                                tracing::error!("Failed to parse message analysis response: {}", e);
                                Ok((None, false, "".to_string(), "".to_string()))
                            }
                        }
                    } else {
                        tracing::error!("No arguments found in tool call");
                        Ok((None, false, "".to_string(), "".to_string()))
                    }
                } else {
                    tracing::error!("No tool calls found");
                    Ok((None, false, "".to_string(), "".to_string()))
                }
            } else {
                tracing::error!("No tool calls section in response");
                Ok((None, false, "".to_string(), "".to_string()))
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

pub async fn check_morning_digest(state: &Arc<AppState>, user_id: i32) -> Result<(), Box<dyn std::error::Error>> {
    // Get the user's digest settings and timezone
    let (morning_digest, day_digest, evening_digest) = state.user_core.get_digests(user_id)?;
    let user_settings = state.user_core.get_user_settings(user_id)?;
    
    // If morning digest is enabled (Some value) and we have a timezone, check the time
    if let (Some(digest_hour_str), Some(timezone)) = (morning_digest.clone(), user_settings.timezone) {
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
            let calendar_events = if state.user_repository.has_active_google_calendar(user_id)? {
                match crate::handlers::google_calendar::handle_calendar_fetching(state.as_ref(), user_id, &start_time, &end_time).await {
                    Ok(axum::Json(value)) => {
                        if let Some(events) = value.get("events").and_then(|e| e.as_array()) {
                            events.iter().filter_map(|event| {
                                let summary = event.get("summary")?.as_str()?.to_string();
                                let start = event.get("start")?.as_str()?.parse().ok()?;
                                let end = event.get("end")?.as_str()?.parse().ok()?;
                                Some(CalendarEvent {
                                    title: summary,
                                    start_time: start,
                                    end_time: end,
                                    description: None,
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
            let end_timestamp = now.timestamp();
            
            // Check if user has IMAP credentials before fetching emails
            let mut messages = if state.user_repository.get_imap_credentials(user_id)?.is_some() {
                // Fetch and filter emails
                match crate::handlers::imap_handlers::fetch_emails_imap(state, user_id, false, Some(50), false).await {
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
                                timestamp: email.date.unwrap_or_else(|| Utc::now()),
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
                tracing::debug!("Skipping email fetch - user {} has no IMAP credentials configured", user_id);
                Vec::new()
            };

            // Log the number of filtered email messages
            tracing::debug!(
                "Filtered {} email messages from the last {} hours for digest",
                messages.len(),
                hours_since_prev
            );

            // Check if user has WhatsApp enabled before fetching messages
            if let Some(bridge) = state.user_repository.get_whatsapp_bridge(user_id)? {
                // Fetch WhatsApp messages
                match crate::utils::whatsapp_utils::fetch_whatsapp_messages(state, user_id, start_timestamp, end_timestamp).await {
                Ok(whatsapp_messages) => {
                    // Convert WhatsAppMessage to MessageInfo and add to messages
                    let whatsapp_infos: Vec<MessageInfo> = whatsapp_messages.into_iter()
                        .map(|msg| MessageInfo {
                            sender: format!("{} ({})", msg.sender_display_name, msg.room_name),
                            content: msg.content,
                            timestamp: DateTime::from_timestamp(msg.timestamp, 0)
                                .unwrap_or_else(|| Utc::now()),
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
                    messages.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
                }
                    Err(e) => {
                        tracing::error!("Failed to fetch WhatsApp messages for digest: {}", e);
                    }
                }
            }

            // Log total number of messages
            tracing::debug!(
                "Total {} messages collected for digest",
                messages.len()
            );

            // Prepare digest data
            let digest_data = DigestData {
                messages,
                calendar_events,
                time_period_hours: hours_to_next,
            };

            // Generate the digest
            let digest_message = match generate_digest(digest_data).await {
                Ok(digest) => format!("Good morning! {}",digest),
                Err(_) => format!(
                    "Good morning! Here's your morning digest covering the last {} hours. Next digest in {} hours.",
                    hours_since_prev, hours_to_next
                ),
            };
                
            tracing::info!("Sending morning digest for user {} at {}:00 in timezone {}", 
                user_id, digest_hour, timezone);
                
            send_notification(
                state,
                user_id,
                &digest_message,
                "morning_digest".to_string(),
                Some("Good morning! Want to hear your morning digest?".to_string()),
            ).await;
        }
    }
    
    Ok(())
}

pub async fn check_day_digest(state: &Arc<AppState>, user_id: i32) -> Result<(), Box<dyn std::error::Error>> {
    // Get the user's digest settings and timezone
    let (morning_digest, day_digest, evening_digest) = state.user_core.get_digests(user_id)?;
    let user_settings = state.user_core.get_user_settings(user_id)?;
    
    // If day digest is enabled (Some value) and we have a timezone, check the time
    if let (Some(digest_hour_str), Some(timezone)) = (day_digest.clone(), user_settings.timezone) {
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
            let calendar_events = if state.user_repository.has_active_google_calendar(user_id)? {
                match crate::handlers::google_calendar::handle_calendar_fetching(state.as_ref(), user_id, &start_time, &end_time).await {
                    Ok(axum::Json(value)) => {
                        if let Some(events) = value.get("events").and_then(|e| e.as_array()) {
                            events.iter().filter_map(|event| {
                                let summary = event.get("summary")?.as_str()?.to_string();
                                let start = event.get("start")?.as_str()?.parse().ok()?;
                                let end = event.get("end")?.as_str()?.parse().ok()?;
                                Some(CalendarEvent {
                                    title: summary,
                                    start_time: start,
                                    end_time: end,
                                    description: None,
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
            let end_timestamp = now.timestamp();
            
            // Check if user has IMAP credentials before fetching emails
            let mut messages = if state.user_repository.get_imap_credentials(user_id)?.is_some() {
                // Fetch and filter emails
                match crate::handlers::imap_handlers::fetch_emails_imap(state, user_id, false, Some(50), false).await {
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
                                timestamp: email.date.unwrap_or_else(|| Utc::now()),
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
                tracing::debug!("Skipping email fetch - user {} has no IMAP credentials configured", user_id);
                Vec::new()
            };

            // Log the number of filtered email messages
            tracing::debug!(
                "Filtered {} email messages from the last {} hours for digest",
                messages.len(),
                hours_since_prev
            );

            // Fetch WhatsApp messages
            if let Some(bridge) = state.user_repository.get_whatsapp_bridge(user_id)? {
                match crate::utils::whatsapp_utils::fetch_whatsapp_messages(state, user_id, start_timestamp, end_timestamp).await {
                    Ok(whatsapp_messages) => {
                        // Convert WhatsAppMessage to MessageInfo and add to messages
                        let whatsapp_infos: Vec<MessageInfo> = whatsapp_messages.into_iter()
                            .map(|msg| MessageInfo {
                                sender: format!("{} ({})", msg.sender_display_name, msg.room_name),
                                content: msg.content,
                                timestamp: DateTime::from_timestamp(msg.timestamp, 0)
                                    .unwrap_or_else(|| Utc::now()),
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
                        messages.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
                    }
                    Err(e) => {
                        tracing::error!("Failed to fetch WhatsApp messages for digest: {}", e);
                    }
                }
            }

            // Log total number of messages
            tracing::debug!(
                "Total {} messages collected for digest",
                messages.len()
            );

            // Prepare digest data
            let digest_data = DigestData {
                messages,
                calendar_events,
                time_period_hours: hours_to_next,
            };

            // Generate the digest
            let digest_message = match generate_digest(digest_data).await {
                Ok(digest) => format!("Hello! {}",digest),
                Err(_) => format!(
                    "Hello! Here's your daily digest covering the last {} hours. Next digest in {} hours.",
                    hours_since_prev, hours_to_next
                ),
            };
                
            tracing::info!("Sending day digest for user {} at {}:00 in timezone {}", 
                user_id, digest_hour, timezone);
                
            send_notification(
                state,
                user_id,
                &digest_message,
                "day_digest".to_string(),
                Some("Hello! Want to hear your daily digest?".to_string()),
            ).await;
        }
    }
    
    Ok(())
}

pub async fn check_evening_digest(state: &Arc<AppState>, user_id: i32) -> Result<(), Box<dyn std::error::Error>> {
    // Get the user's digest settings and timezone
    let (morning_digest, day_digest, evening_digest) = state.user_core.get_digests(user_id)?;
    let user_settings = state.user_core.get_user_settings(user_id)?;
    
    // If morning digest is enabled (Some value) and we have a timezone, check the time
    if let (Some(digest_hour_str), Some(timezone)) = (evening_digest.clone(), user_settings.timezone) {
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

            // Format start time (now) and end time (now + hours_to_next) in RFC3339
            let start_time = now.with_timezone(&Utc).to_rfc3339();

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
            let calendar_events = if state.user_repository.has_active_google_calendar(user_id)? {
                match crate::handlers::google_calendar::handle_calendar_fetching(state.as_ref(), user_id, &start_time, &end_time).await {
                    Ok(axum::Json(value)) => {
                        if let Some(events) = value.get("events").and_then(|e| e.as_array()) {
                            events.iter().filter_map(|event| {
                                let summary = event.get("summary")?.as_str()?.to_string();
                                let start = event.get("start")?.as_str()?.parse().ok()?;
                                let end = event.get("end")?.as_str()?.parse().ok()?;
                                Some(CalendarEvent {
                                    title: summary,
                                    start_time: start,
                                    end_time: end,
                                    description: None,
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
            let end_timestamp = now.timestamp();
            
            // Check if user has IMAP credentials before fetching emails
            let mut messages = if state.user_repository.get_imap_credentials(user_id)?.is_some() {
                // Fetch and filter emails
                match crate::handlers::imap_handlers::fetch_emails_imap(state, user_id, false, Some(50), false).await {
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
                                timestamp: email.date.unwrap_or_else(|| Utc::now()),
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
                tracing::debug!("Skipping email fetch - user {} has no IMAP credentials configured", user_id);
                Vec::new()
            };

            // Log the number of filtered email messages
            tracing::debug!(
                "Filtered {} email messages from the last {} hours for digest",
                messages.len(),
                hours_since_prev
            );

            // Fetch WhatsApp messages
            if let Some(bridge) = state.user_repository.get_whatsapp_bridge(user_id)? {
                match crate::utils::whatsapp_utils::fetch_whatsapp_messages(state, user_id, start_timestamp, end_timestamp).await {
                    Ok(whatsapp_messages) => {
                        // Convert WhatsAppMessage to MessageInfo and add to messages
                        let whatsapp_infos: Vec<MessageInfo> = whatsapp_messages.into_iter()
                            .map(|msg| MessageInfo {
                                sender: format!("{} ({})", msg.sender_display_name, msg.room_name),
                                content: msg.content,
                                timestamp: DateTime::from_timestamp(msg.timestamp, 0)
                                    .unwrap_or_else(|| Utc::now()),
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
                        messages.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
                    }
                    Err(e) => {
                        tracing::error!("Failed to fetch WhatsApp messages for digest: {}", e);
                    }
                }
            }

            // Log total number of messages
            tracing::debug!(
                "Total {} messages collected for digest",
                messages.len()
            );

            // Prepare digest data
            let digest_data = DigestData {
                messages,
                calendar_events,
                time_period_hours: hours_to_next,
            };

            // Generate the digest
            let digest_message = match generate_digest(digest_data).await {
                Ok(digest) => format!("Good evening! {}",digest),
                Err(_) => format!(
                    "Hello! Here's your evening digest covering the last {} hours. Next digest in {} hours.",
                    hours_since_prev, hours_to_next
                ),
            };
                
            tracing::info!("Sending evening digest for user {} at {}:00 in timezone {}", 
                user_id, digest_hour, timezone);
                
            send_notification(
                state,
                user_id,
                &digest_message,
                "evening_digest".to_string(),
                Some("Good evening! Want to hear your evening digest?".to_string()),
            ).await;
        }
    }
    
    Ok(())
}

pub async fn generate_digest(data: DigestData) -> Result<String, Box<dyn std::error::Error>> {
    let client = create_openai_client()?;

    // Format messages for the prompt
    let messages_str = data.messages
        .iter()
        .map(|msg| {
            format!(
                "- [{}] {} on {}: {}",
                msg.platform.to_uppercase(),
                msg.sender,
                msg.timestamp.format("%H:%M"),
                msg.content
            )
        })
        .collect::<Vec<String>>()
        .join("\n");

    // Format calendar events for the prompt
    let events_str = data.calendar_events
        .iter()
        .map(|event| {
            format!(
                "- {} at {}-{}: {}",
                event.title,
                event.start_time.format("%H:%M"),
                event.end_time.format("%H:%M"),
                event.description.as_deref().unwrap_or("")
            )
        })
        .collect::<Vec<String>>()
        .join("\n");

    let messages = vec![
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::system,
            content: chat_completion::Content::Text(
                "You are an AI that creates concise, SMS-friendly digests (maximum 480 chars, but the shorter the better without losing any critical information) of messages and calendar events. Focus on what's most important and actionable. When mentioning messages, include the platform (EMAIL/WHATSAPP) if it adds important context. If there are critical messages that need immediate attention, highlight those first. For calendar events, prioritize upcoming events in the next few hours.".to_string(),
            ),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::user,
            content: chat_completion::Content::Text(format!(
                "Create a digest covering the last {} hours.\n\nMessages:\n{}\n\nUpcoming calendar events:\n{}",
                data.time_period_hours, messages_str, events_str
            )),
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
        "openai/gpt-4o-mini".to_string(),
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
                                Ok("Failed to generate digest.".to_string())
                            }
                        }
                    } else {
                        tracing::error!("No arguments found in tool call");
                        Ok("Failed to generate digest.".to_string())
                    }
                } else {
                    tracing::error!("No tool calls found");
                    Ok("Failed to generate digest.".to_string())
                }
            } else {
                tracing::error!("No tool calls section in response");
                Ok("Failed to generate digest.".to_string())
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
    user_id: i32,
    notification: &str,
    content_type: String,
    first_message: Option<String>,
) {
    // Get user info
    let user = match state.user_core.find_by_id(user_id) {
        Ok(Some(user)) => user,
        Ok(None) => {
            tracing::error!("User {} not found for notification", user_id);
            return;
        }
        Err(e) => {
            tracing::error!("Failed to get user {}: {}", user_id, e);
            return;
        }
    };

    // Get user settings (assuming state has a user_settings repository or similar)
    let user_settings = match state.user_core.get_user_settings(user_id) {
        Ok(settings) => settings,
        Err(e) => {
            tracing::error!("Failed to get settings for user {}: {}", user_id, e);
            return;
        }
    };

    // Get the user's preferred number or default
    let sender_number = match user.preferred_number.clone() {
        Some(number) => {
            tracing::info!("Using user's preferred number: {}", number);
            number
        }
        None => {
            let number = std::env::var("SHAZAM_PHONE_NUMBER").expect("SHAZAM_PHONE_NUMBER not set");
            tracing::info!("Using default SHAZAM_PHONE_NUMBER: {}", number);
            number
        }
    };

    // Get the conversation for the user
    let conversation = match state.user_conversations.get_conversation(&user, sender_number).await {
        Ok(conv) => conv,
        Err(e) => {
            tracing::error!("Failed to ensure conversation exists: {}", e);
            return;
        }
    };

    // Check user's notification preference from settings
    let notification_type = user_settings.notification_type.as_deref().unwrap_or("sms");
    match notification_type {
        "call" => {
            // For calls, we need a brief intro and detailed message

            // Create dynamic variables (optional, can be customized based on needs)
            let mut dynamic_vars = std::collections::HashMap::new();

            match crate::api::elevenlabs::make_notification_call(
                &state.clone(),
                user.phone_number.clone(),
                user.preferred_number
                    .unwrap_or_else(|| std::env::var("SHAZAM_PHONE_NUMBER").expect("SHAZAM_PHONE_NUMBER not set")),
                content_type, // Notification type
                first_message.unwrap_or("Hello, I have a critical notification to tell you about".to_string()),
                notification.clone().to_string(),
                user.id.to_string(),
                user_settings.timezone,
            ).await {
                Ok(mut response) => {
                    // Add dynamic variables to the client data
                    if let Some(client_data) = response.get_mut("client_data") {
                        if let Some(obj) = client_data.as_object_mut() {
                            obj.extend(dynamic_vars.into_iter().map(|(k, v)| (k, serde_json::Value::String(v))));
                        }
                    }
                    tracing::debug!("Successfully initiated call notification for user {}", user.id);
                }
                Err((_, json_err)) => {
                    tracing::error!("Failed to initiate call notification: {:?}", json_err);
                    println!("Failed to send call notification for user {}", user_id);
                }
            }
        }
        _ => {
            // Default to SMS notification
            match crate::api::twilio_utils::send_conversation_message(
                &conversation.conversation_sid,
                &conversation.twilio_number,
                &notification,
                true,
                None,
                &user,
            ).await {
                Ok(_) => {
                    tracing::info!("Successfully sent notification to user {}", user_id);
                    println!("SMS notification sent successfully for user {}", user_id);
                }
                Err(e) => {
                    tracing::error!("Failed to send notification: {}", e);
                    println!("Failed to send SMS notification for user {}", user_id);
                }
            }
        }
    }
}
