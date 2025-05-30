use tokio_cron_scheduler::{JobScheduler, Job};
use std::sync::Arc;
use tracing::{info, error};
use crate::AppState;

//use crate::handlers::gmail;




use std::env;

use crate::handlers::imap_handlers;
use crate::api::twilio_utils;

pub async fn start_scheduler(state: Arc<AppState>) {
    let sched = JobScheduler::new().await.expect("Failed to create scheduler");

    // Create a job that runs every minute to check for new messages across services
    let state_clone = Arc::clone(&state);
    let message_monitor_job = Job::new_async("0 * * * * *", move |_, _| {
        let state = state_clone.clone();
        Box::pin(async move {
            
            // Get all users with valid message monitor subscription
            let users_with_subscription = match state.user_repository.get_all_users() {
                Ok(users) => {
                    let mut subscribed_users = Vec::new();
                    for user in users {
                        match state.user_repository.has_valid_subscription_tier_with_messages(user.id, "tier 2") {
                            Ok(true) => {
                                subscribed_users.push(user);
                            },
                            Ok(false) => {
                                info!("User {} does not have valid subscription or messages left for message monitoring", user.id);
                            },
                            Err(e) => {
                                error!("Failed to check subscription status for user {}: {:?}", user.id, e);
                            }
                        }
                    }
                    subscribed_users
                },
                Err(e) => {
                    error!("Failed to fetch users: {}", e);
                    Vec::new()
                }
            };

            // Process each subscribed user
            for user in users_with_subscription {

                // Check IMAP service
                if let Ok(imap_users) = state.user_repository.get_active_imap_connection_users() {
                    // Get both the proactive status and last activation timestamp
                    let (is_proactive, last_activated) = match state.user_repository.get_imap_proactive_status(user.id) {
                        Ok((enabled, timestamp)) => (enabled, timestamp),
                        Err(e) => {
                            error!("Failed to check IMAP proactive status for user {}: {}", user.id, e);
                            continue;
                        }
                    };

                    if imap_users.contains(&user.id) && is_proactive {
                        info!("Checking IMAP messages for user {} (activated since timestamp {})", user.id, last_activated);
                        match imap_handlers::fetch_emails_imap(&state, user.id, true, Some(10), true).await {
                            Ok(emails) => {
                                
                                match state.user_repository.get_processed_emails(user.id) {
                                    Ok(mut processed_emails) => {
                                        // Define constants
                                        let fetch_window = 10;  // Number of emails your scheduler fetches
                                        let cleanup_threshold = 100;  // Only cleanup when we have significantly more than fetch window

                                        if processed_emails.len() > cleanup_threshold {
                                            // Sort by processed_at timestamp (newest first)
                                            processed_emails.sort_by(|a, b| b.processed_at.cmp(&a.processed_at));
                                            
                                            // Keep at least fetch_window emails plus some buffer
                                            let keep_count = fetch_window * 2;  // Keep 20 emails (double the fetch window)
                                            
                                            // Get emails to delete (older than our keep_count)
                                            let emails_to_delete: Vec<_> = processed_emails
                                                .iter()
                                                .skip(keep_count)
                                                .collect();

                                            // Delete old processed emails
                                            for email in emails_to_delete {
                                                if let Err(e) = state.user_repository.delete_processed_email(user.id, &email.email_uid) {
                                                    error!("Failed to delete old processed email {}: {}", email.email_uid, e);
                                                } else {
                                                    info!("Deleted old processed email {} for user {}", email.email_uid, user.id);
                                                }
                                            }

                                            // Update the original collection
                                            processed_emails.truncate(keep_count);

                                            // Also clean up old email judgments
                                            if let Err(e) = state.user_repository.delete_old_email_judgments(user.id) {
                                                error!("Failed to delete old email judgments for user {}: {}", user.id, e);
                                            } else {
                                                info!("Successfully cleaned up old email judgments for user {}", user.id);
                                            }
                                        }
                                    }
                                    Err(e) => error!("Failed to fetch processed emails for garbage collection: {}", e),
                                }
                                
                                if !emails.is_empty() {
                                    // Get user's waiting checks, priority senders, and keywords
                                    let waiting_checks = match state.user_repository.get_waiting_checks(user.id, "imap") {
                                        Ok(checks) => checks,
                                        Err(e) => {
                                            error!("Failed to get waiting checks for user {}: {}", user.id, e);
                                            Vec::new()
                                        }
                                    };

                                    let priority_senders = match state.user_repository.get_priority_senders(user.id, "imap") {
                                        Ok(senders) => senders,
                                        Err(e) => {
                                            error!("Failed to get priority senders for user {}: {}", user.id, e);
                                            Vec::new()
                                        }
                                    };

                                    let keywords = match state.user_repository.get_keywords(user.id, "imap") {
                                        Ok(kw) => kw,
                                        Err(e) => {
                                            error!("Failed to get keywords for user {}: {}", user.id, e);
                                            Vec::new()
                                        }
                                    };

                                    let importance_priority = match state.user_repository.get_importance_priority(user.id, "imap") {
                                        Ok(Some(priority)) => priority.threshold,
                                        Ok(None) => 7, // Default threshold
                                        Err(e) => {
                                            error!("Failed to get importance priority for user {}: {}", user.id, e);
                                            7 // Default threshold on error
                                        }
                                    };

                                    // Prepare the system message for email evaluation
                                    info!("Building system message with following parameters:");
                                    info!("Importance threshold: {}", importance_priority);
                                    
                                    // Get user's custom general checks prompt or use default
                                    let general_checks_prompt = match state.user_repository.get_imap_general_checks(user.id) {
                                        Ok(prompt) => {
                                            info!("Using custom general checks prompt for user {}", user.id);
                                            prompt
                                        },
                                        Err(e) => {
                                            error!("Failed to get general checks prompt for user {}: {}", user.id, e);
                                            continue;
                                        }
                                    };

                                    let waiting_checks_formatted = waiting_checks.iter()
                                        .map(|wc| format!("{{id: {}, content: '{}'}}", wc.id.unwrap_or(-1), wc.content))
                                        .collect::<Vec<_>>()
                                        .join(", ");

                                    let system_message = format!(
                                        "You are an intelligent email filter designed to determine if an email is important enough to notify the user via SMS. \
                                        Your evaluation process has two main parts:\n\n\
                                        PART 1 - SPECIFIC FILTERS CHECK:\n\
                                        First, check if the email matches any user-defined 'waiting checks', priority senders, or keywords. These are absolute filters \
                                        that should trigger a notification if matched:\n\
                                        - Waiting Checks: {}\n\
                                        - Priority Senders: {}\n\
                                        - Keywords: {}\n\n\
                                        PART 2 - GENERAL IMPORTANCE ANALYSIS:\n\
                                        If no specific filters are matched, evaluate the email's importance using these general criteria:\n\
                                        {}\n\n\
                                        Based on all checks, assign an importance score from 0 (not important) to 10 (extremely important). \
                                        If the score meets or exceeds the user's threshold ({}), recommend sending an SMS notification.\n\n\
                                        When a waiting check matches, you MUST include its ID in the matched_waiting_check field.\n\n\
                                        Return a JSON object with the following structure:\n\
                                        {{\n\
                                            'should_notify': true/false,\n\
                                            'reason': 'explanation',\n\
                                            'score': number (if applicable),\n\
                                            'matched_waiting_check': number (the ID of the matched waiting check, if any)\n\
                                        }}",
                                        waiting_checks_formatted,
                                        priority_senders.iter().map(|ps| ps.sender.clone()).collect::<Vec<_>>().join(", "),
                                        keywords.iter().map(|k| k.keyword.clone()).collect::<Vec<_>>().join(", "),
                                        general_checks_prompt,
                                        importance_priority
                                    );

                                    let api_key = match env::var("OPENROUTER_API_KEY") {
                                        Ok(key) => {
                                            info!("Successfully retrieved OpenRouter API key");
                                            key
                                        },
                                        Err(e) => {
                                            error!("Failed to get OPENROUTER_API_KEY: {}", e);
                                            continue;
                                        }
                                    };
                                    let client = match openai_api_rs::v1::api::OpenAIClient::builder()
                                        .with_endpoint("https://openrouter.ai/api/v1")
                                        .with_api_key(api_key)
                                        .build() {
                                            Ok(client) => {
                                                info!("Successfully built OpenAI client");
                                                client
                                            },
                                            Err(e) => {
                                                error!("Failed to build OpenAI client: {}", e);
                                                continue;
                                            }
                                        };

                                    let mut important_emails = Vec::new();

                                    for email in &emails {
                                        let email_content = format!(
                                            "From: {}\nSubject: {}\nBody: {}",
                                            email.from_email.as_deref().unwrap_or("Unknown"),
                                            email.subject.as_deref().unwrap_or("No subject"),
                                            email.body.as_deref().unwrap_or("No content")
                                        );

                                    // Define the tool for email evaluation
                                    let mut email_eval_properties = std::collections::HashMap::new();
                                    email_eval_properties.insert(
                                        "should_notify".to_string(),
                                        Box::new(openai_api_rs::v1::types::JSONSchemaDefine {
                                            schema_type: Some(openai_api_rs::v1::types::JSONSchemaType::Boolean),
                                            description: Some("Whether the user should be notified about this email".to_string()),
                                            ..Default::default()
                                        }),
                                    );
                                    email_eval_properties.insert(
                                        "reason".to_string(),
                                        Box::new(openai_api_rs::v1::types::JSONSchemaDefine {
                                            schema_type: Some(openai_api_rs::v1::types::JSONSchemaType::String),
                                            description: Some("Explanation for why the user should or should not be notified".to_string()),
                                            ..Default::default()
                                        }),
                                    );
                                    email_eval_properties.insert(
                                        "score".to_string(),
                                        Box::new(openai_api_rs::v1::types::JSONSchemaDefine {
                                            schema_type: Some(openai_api_rs::v1::types::JSONSchemaType::Number),
                                            description: Some("Importance score from 0 to 10".to_string()),
                                            ..Default::default()
                                        }),
                                    );
                                    email_eval_properties.insert(
                                        "matched_waiting_check".to_string(),
                                        Box::new(openai_api_rs::v1::types::JSONSchemaDefine {
                                            schema_type: Some(openai_api_rs::v1::types::JSONSchemaType::Number),
                                            description: Some("The ID of the waiting check that was matched, if any. Must be the exact ID from the waiting checks list.".to_string()),
                                            ..Default::default()
                                        }),
                                    );

                                    let tools = vec![
                                        openai_api_rs::v1::chat_completion::Tool {
                                            r#type: openai_api_rs::v1::chat_completion::ToolType::Function,
                                            function: openai_api_rs::v1::types::Function {
                                                name: String::from("evaluate_email"),
                                                description: Some(String::from("Evaluate email importance and determine if notification is needed")),
                                                parameters: openai_api_rs::v1::types::FunctionParameters {
                                                    schema_type: openai_api_rs::v1::types::JSONSchemaType::Object,
                                                    properties: Some(email_eval_properties),
                                                    required: Some(vec![
                                                        String::from("should_notify"),
                                                        String::from("reason"),
                                                        String::from("score"),
                                                    ]),
                                                },
                                            },
                                        },
                                    ];

                                    let messages = vec![
                                        openai_api_rs::v1::chat_completion::ChatCompletionMessage {
                                            role: openai_api_rs::v1::chat_completion::MessageRole::system,
                                            content: openai_api_rs::v1::chat_completion::Content::Text(system_message.clone()),
                                            name: None,
                                            tool_calls: None,
                                            tool_call_id: None,
                                        },
                                        openai_api_rs::v1::chat_completion::ChatCompletionMessage {
                                            role: openai_api_rs::v1::chat_completion::MessageRole::user,
                                            content: openai_api_rs::v1::chat_completion::Content::Text(email_content.clone()),
                                            name: None,
                                            tool_calls: None,
                                            tool_call_id: None,
                                        },
                                    ];

                                    let req = openai_api_rs::v1::chat_completion::ChatCompletionRequest::new(
                                        "meta-llama/llama-4-scout".to_string(),
                                        messages,
                                    )
                                    .tools(tools)
                                    .tool_choice(openai_api_rs::v1::chat_completion::ToolChoiceType::Required);

                                        
                                    match client.chat_completion(req.clone()).await {
                                        Ok(response) => {
                                            info!("Received LLM response: {:?}", response);
                                            if let Some(tool_calls) = response.choices[0].message.tool_calls.as_ref() {
                                                for tool_call in tool_calls {
                                                    if let Some(name) = &tool_call.function.name {
                                                        if name == "evaluate_email" {
                                                            if let Some(arguments) = &tool_call.function.arguments {
                                                                info!("Processing tool call arguments");
                                                                match serde_json::from_str::<serde_json::Value>(arguments) {
                                                    Ok(evaluation) => {
                                                        info!("Parsed evaluation");
                                                        
                                                        // Create email judgment log regardless of notification decision
                                                        let current_time = std::time::SystemTime::now()
                                                            .duration_since(std::time::UNIX_EPOCH)
                                                            .unwrap()
                                                            .as_secs() as i32;

                                                        let email_timestamp = email.date
                                                                .map(|dt| dt.timestamp() as i32)
                                                                .unwrap_or(current_time);

                                                        // Skip emails that are older than the last activation time
                                                        if email_timestamp <= last_activated {
                                                            info!("Skipping email with timestamp {} as it's older than last activation time {}", email_timestamp, last_activated);
                                                            continue;
                                                        }

                                                        let new_judgment = crate::models::user_models::NewEmailJudgment {
                                                            user_id: user.id,
                                                            email_timestamp,
                                                            processed_at: current_time,
                                                            should_notify: evaluation["should_notify"].as_bool().unwrap_or(false),
                                                            score: evaluation["score"].as_i64().unwrap_or(0) as i32,
                                                            reason: evaluation["reason"].as_str().unwrap_or("No reason provided").to_string(),
                                                        };

                                                        if let Err(e) = state.user_repository.create_email_judgment(&new_judgment) {
                                                            error!("Failed to create email judgment log: {}", e);
                                                        } else {
                                                            info!("Successfully created email judgment log for email");
                                                        }

                                                        if evaluation["should_notify"].as_bool().unwrap_or(false) {
                                                            info!("Email marked as important, adding to notification list");
                                                            important_emails.push(email);
                                                            
                                                            // Check if notification was due to a waiting check
                                                            if let Some(matched_check_id) = evaluation["matched_waiting_check"].as_i64() {
                                                                                info!("Matched waiting check ID: {}", matched_check_id);
                                                                                // Find the matching waiting check
                                                                                if let Some(check) = waiting_checks.iter().find(|wc| wc.id == Some(matched_check_id as i32)) {
                                                                                    if check.remove_when_found {
                                                                                        info!("Removing waiting check with ID {}", matched_check_id);
                                                                                        if let Err(e) = state.user_repository.delete_waiting_check(
                                                                                            user.id,
                                                                                            "imap",
                                                                                            &check.content
                                                                                        ) {
                                                                                            error!("Failed to delete waiting check: {}", e);
                                                                                        }
                                                                                    }
                                                                                }
                                                                            }
                                                                        } else {
                                                                            info!("Email not marked as important, skipping");
                                                                        }
                                                                    }
                                                                    Err(e) => {
                                                                        error!("Failed to parse tool call arguments: {}", e);
                                                                        error!("Raw arguments that failed to parse: {}", arguments);
                                                                    }
                                                                }
                                                            }
                                                        }

                                                    }

                                                }
                                            } else {
                                                error!("No tool calls in LLM response");
                                            }
                                        }
                                        Err(e) => {
                                            error!("Failed to get LLM response: {}", e);
                                            error!("Request details: {:?}", req);
                                        }
                                    }
                                }

                                if important_emails.is_empty() {
                                    info!("No important emails found for user {}", user.id);
                                    continue;
                                }

                                    // Check if we should process this notification based on messages left
                                    let should_continue = match state.user_repository.decrease_messages_left(user.id) {
                                        Ok(msgs_left) => {
                                            info!("User {} has {} messages left after decrease", user.id, msgs_left);
                                            msgs_left > 0
                                        },
                                        Err(e) => {
                                            error!("Failed to decrease messages left for user {}: {}", user.id, e);
                                            false
                                        }
                                    };

                                    if !should_continue {
                                        info!("Skipping further processing for user {} due to no messages left", user.id);
                                        continue;
                                    }
                                    let sender_number = match user.preferred_number.clone() {
                                        Some(number) => {
                                            tracing::info!("Using user's preferred number: {}", number);
                                            number
                                        },
                                        None => {
                                            let number = std::env::var("SHAZAM_PHONE_NUMBER").expect("SHAZAM_PHONE_NUMBER not set");
                                            tracing::info!("Using default SHAZAM_PHONE_NUMBER: {}", number);
                                            number
                                        },
                                    };

                                    // Get the conversation for the user
                                    let conversation = match state.user_conversations.get_conversation(&user, sender_number).await {
                                        Ok(conv) => conv,
                                        Err(e) => {
                                            eprintln!("Failed to ensure conversation exists: {}", e);
                                            continue;
                                        }
                                    };

                                    // Format the email data for LLM processing
                                    let email_summaries = important_emails.iter()
                                        .take(3) // Still limit to 3 emails
                                        .map(|email| format!(
                                            "From: {} \nSubject: {}\nBody: {}",
                                            email.from_email.as_deref().unwrap_or("unknown@email.com"),
                                            email.subject.as_deref().unwrap_or("No subject"),
                                            email.body.as_deref().unwrap_or("No body")
                                        ))
                                        .collect::<Vec<_>>()
                                        .join("\n\n---\n\n");

                                    // Define the system message for notification formatting
                                    let format_system_message = "You are an AI assistant that creates concise, natural-sounding SMS notifications about important emails. \
                                        Your message should be clear, informative while keeping the length appropriate for SMS. \
                                        Include the relevant details from each email. \
                                        Focus on what makes these emails important and state the information, never reason about the content. Also mention that the message(s) was from email. You are the users assistant which provides this important information.";

                                    // Define the tool for notification formatting
                                    let mut format_properties = std::collections::HashMap::new();
                                    format_properties.insert(
                                        "notification_text".to_string(),
                                        Box::new(openai_api_rs::v1::types::JSONSchemaDefine {
                                            schema_type: Some(openai_api_rs::v1::types::JSONSchemaType::String),
                                            description: Some("The formatted notification message".to_string()),
                                            ..Default::default()
                                        }),
                                    );

                                    let format_tools = vec![
                                        openai_api_rs::v1::chat_completion::Tool {
                                            r#type: openai_api_rs::v1::chat_completion::ToolType::Function,
                                            function: openai_api_rs::v1::types::Function {
                                                name: String::from("format_notification"),
                                                description: Some(String::from("Format the email summaries into a natural notification message")),
                                                parameters: openai_api_rs::v1::types::FunctionParameters {
                                                    schema_type: openai_api_rs::v1::types::JSONSchemaType::Object,
                                                    properties: Some(format_properties),
                                                    required: Some(vec![String::from("notification_text")]),
                                                },
                                            },
                                        },
                                    ];

                                    let format_messages = vec![
                                        openai_api_rs::v1::chat_completion::ChatCompletionMessage {
                                            role: openai_api_rs::v1::chat_completion::MessageRole::system,
                                            content: openai_api_rs::v1::chat_completion::Content::Text(format_system_message.to_string()),
                                            name: None,
                                            tool_calls: None,
                                            tool_call_id: None,
                                        },
                                        openai_api_rs::v1::chat_completion::ChatCompletionMessage {
                                            role: openai_api_rs::v1::chat_completion::MessageRole::user,
                                            content: openai_api_rs::v1::chat_completion::Content::Text(format!(
                                                "Please format a notification for these {} important emails:\n\n{}",
                                                important_emails.len(),
                                                email_summaries
                                            )),
                                            name: None,
                                            tool_calls: None,
                                            tool_call_id: None,
                                        },
                                    ];

                                    let format_req = openai_api_rs::v1::chat_completion::ChatCompletionRequest::new(
                                        "meta-llama/llama-4-scout".to_string(),
                                        format_messages,
                                    )
                                    .tools(format_tools)
                                    .tool_choice(openai_api_rs::v1::chat_completion::ToolChoiceType::Required);

                                    // Get the formatted notification from LLM
                                    let notification = match client.chat_completion(format_req).await {
                                        Ok(response) => {
                                            if let Some(tool_calls) = response.choices[0].message.tool_calls.as_ref() {
                                                if let Some(tool_call) = tool_calls.first() {
                                                    if let Some(arguments) = &tool_call.function.arguments {
                                                        match serde_json::from_str::<serde_json::Value>(arguments) {
                                                            Ok(formatted) => {
                                                                formatted["notification_text"]
                                                                    .as_str()
                                                                    .unwrap_or("You have new important emails to check.")
                                                                    .to_string()
                                                            }
                                                            Err(e) => {
                                                                error!("Failed to parse notification format response: {}", e);
                                                                format!("You have {} important new emails to check.", important_emails.len())
                                                            }
                                                        }
                                                    } else {
                                                        format!("You have {} important new emails to check.", important_emails.len())
                                                    }
                                                } else {
                                                    format!("You have {} important new emails to check.", important_emails.len())
                                                }
                                            } else {
                                                format!("You have {} important new emails to check.", important_emails.len())
                                            }
                                        }
                                        Err(e) => {
                                            error!("Failed to get notification format from LLM: {}", e);
                                            format!("You have {} important new emails to check.", important_emails.len())
                                        }
                                    };

                                    // Check if this is the final message
                                    let is_final_message = user.msgs_left <= 1;

                                    // Append final message notice if needed
                                    let final_notification = if is_final_message {
                                        format!("{}\n\nNote: This is your final proactive message for this month. Your message quota will reset at the start of next month.", notification)
                                    } else {
                                        notification
                                    };

                                    // Send SMS notification
                                    match twilio_utils::send_conversation_message(
                                        &conversation.conversation_sid,
                                        &conversation.twilio_number,
                                        &final_notification,
                                        true,
                                    ).await {
                                        Ok(_) => info!("Successfully sent email notification to user {}", user.id),
                                        Err(e) => error!("Failed to send email notification: {}", e),
                                    }
                                }
                            },
                            Err(e) => {
                                error!("Failed to fetch IMAP emails for user {}: Error: {:?}", user.id, e);
                            }
                        }
                    }
                }

                // Check if we should continue processing other services
                match state.user_repository.has_valid_subscription_tier_with_messages(user.id, "tier 2") {
                    Ok(false) => {
                        continue;
                    },
                    Err(e) => {
                        error!("Failed to check messages left for user {}: {}", user.id, e);
                        continue;
                    },
                    Ok(true) => {
                    }
                }

                // Add more services here following the same pattern
            }

        })
    }).expect("Failed to create message monitor job");

    sched.add(message_monitor_job).await.expect("Failed to add message monitor job to scheduler");

    // Create a job that runs every minute to handle ongoing usage logs
    let state_clone = Arc::clone(&state);
    let usage_monitor_job = Job::new_async("0 * * * * *", move |_, _| {
        let state = state_clone.clone();
        Box::pin(async move {
            let api_key = env::var("ELEVENLABS_API_KEY").expect("ELEVENLABS_API_KEY must be set");
            let client = reqwest::Client::new();

            match state.user_repository.get_all_ongoing_usage() {
                Ok(ongoing_logs) => {
                    for log in ongoing_logs {
                        let sid= match log.sid {
                            Some(id) => id,
                            None => continue,
                        };

                        // Check conversation status from ElevenLabs
                        let status_url = format!(
                            "https://api.elevenlabs.io/v1/convai/conversations/{}",
                            sid 
                        );

                        let conversation_data = match client
                            .get(&status_url)
                            .header("xi-api-key", &api_key)
                            .send()
                            .await
                        {
                            Ok(response) => {
                                match response.json::<serde_json::Value>().await {
                                    Ok(data) => data,
                                    Err(e) => {
                                        error!("Failed to parse conversation response: {}", e);
                                        continue;
                                    }
                                }
                            },
                            Err(e) => {
                                error!("Failed to fetch conversation status: {}", e);
                                continue;
                            }
                        };

                        // Handle recharge threshold timestamp
                        if let Some(threshold_timestamp) = log.recharge_threshold_timestamp {
                            let current_timestamp = chrono::Utc::now().timestamp() as i32;
                            if current_timestamp >= threshold_timestamp {
                                match state.user_repository.has_auto_topup_enabled(log.user_id) {
                                    Ok(true) => {
                                        info!("has auto top up");
                                        info!("conversation_data status: {}",conversation_data["status"]);
                                        info!("conversation_data : {}",conversation_data);
                                        // Verify call is still active
                                        if conversation_data["status"] == "processing" {
                                            tracing::debug!("Recharging the user back up");
                                            use axum::extract::{State, Path};
                                            let state_clone = Arc::clone(&state);
                                            tokio::spawn(async move {
                                                let _ = crate::handlers::stripe_handlers::automatic_charge(
                                                    State(state_clone),
                                                    Path(log.user_id),
                                                ).await;
                                                tracing::debug!("Recharged the user successfully back up!");
                                            });
                                        }
                                    }
                                    Ok(false) => {
                                    }
                                    Err(e) => error!("Failed to check auto top-up status: {}", e),
                                }
                            }
                        }

                        // Handle zero credits timestamp
                        if let Some(zero_timestamp) = log.zero_credits_timestamp {
                            let current_timestamp = chrono::Utc::now().timestamp() as i32;
                            if current_timestamp >= zero_timestamp {
                                // Get final status and delete conversation
                                let call_duration = conversation_data["metadata"]["call_duration_secs"].as_f64().unwrap_or(0.0) as f32;
                                let voice_second_cost = env::var("VOICE_SECOND_COST")
                                    .expect("VOICE_SECOND_COST not set")
                                    .parse::<f32>()
                                    .unwrap_or(0.0033);
                                let credits_used = call_duration * voice_second_cost;

                                // Update usage log with final status
                                if let Err(e) = state.user_repository.update_usage_log_fields(
                                    log.user_id,
                                    &sid,
                                    "done",
                                    true,
                                    &format!("Call ended due to zero credits. Duration: {}s", call_duration),
                                    None,
                                ) {
                                    error!("Failed to update usage log fields: {}", e);
                                }

                                // Decrease user's credits
                                if let Err(e) = state.user_repository.decrease_credits(log.user_id, credits_used) {
                                    error!("Failed to decrease user credits: {}", e);
                                }

                                if conversation_data["status"] == "processing" {
                                    info!("deleting conversation");
                                    // Delete conversation
                                    let delete_url = format!(
                                        "https://api.elevenlabs.io/v1/convai/conversations/{}", 
                                        sid 
                                    );
                                    
                                    if let Err(e) = client
                                        .delete(&delete_url)
                                        .header("xi-api-key", &api_key)
                                        .send()
                                        .await
                                    {
                                        error!("Failed to delete conversation: {}", e);
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => error!("Failed to fetch ongoing usage logs: {}", e),
            }
        })
    }).expect("Failed to create usage monitor job");

    sched.add(usage_monitor_job).await.expect("Failed to add usage monitor job to scheduler");

    /*
    // Create a job that runs every minute to check for due tasks
    let state_clone = Arc::clone(&state);
    let task_monitor_job = Job::new_async("0 * * * * *", move |_, _| {
        let state = state_clone.clone();
        Box::pin(async move {
            
            // Get all users with valid Google Tasks connection
            let users = match state.user_repository.get_all_users() {
                Ok(users) => users,
                Err(e) => {
                    error!("Failed to fetch users: {}", e);
                    return;
                }
            };

            for user in users {
                // Check if user has messages left and valid subscription
                match state.user_repository.has_valid_subscription_tier_with_messages(user.id, "tier 2") {
                    Ok(true) => (),
                    Ok(false) => {
                        info!("User {} does not have valid subscription or messages left", user.id);
                        continue;
                    },
                    Err(e) => {
                        error!("Failed to check subscription status for user {}: {}", user.id, e);
                        continue;
                    }
                }

                // Check if user has enabled proactive calendar notifications
                match state.user_repository.get_proactive_calendar(user.id) {
                    Ok(true) => (),
                    Ok(false) => {
                        info!("User {} has not enabled proactive calendar notifications", user.id);
                        continue;
                    },
                    Err(e) => {
                        error!("Failed to check proactive calendar setting for user {}: {}", user.id, e);
                        continue;
                    }
                }

                // Check if user has active Google Tasks connection
                match state.user_repository.has_active_google_tasks(user.id) {
                    Ok(true) => (),
                    Ok(false) => continue,
                    Err(e) => {
                        error!("Failed to check Google Tasks status for user {}: {}", user.id, e);
                        continue;
                    }
                }

                // Fetch user's tasks
                let tasks = match google_tasks::get_tasks(&state, user.id).await {
                    Ok(response) => {
                        match response.0.get("tasks") {
                            Some(tasks) => {
                                match serde_json::from_value::<Vec<Task>>(tasks.clone()) {
                                    Ok(tasks) => tasks,
                                    Err(e) => {
                                        error!("Failed to parse tasks for user {}: {}", user.id, e);
                                        continue;
                                    }
                                }
                            },
                            None => {
                                error!("No tasks field in response for user {}", user.id);
                                continue;
                            }
                        }
                    },
                    Err(e) => {
                        error!("Failed to fetch tasks for user {}: {:?}", user.id, e);
                        continue;
                    }
                };

                let now = chrono::Utc::now();
                let mut due_tasks = Vec::new();

                // Check each task
                for task in tasks {
                    // Skip if task is already completed
                    if task.status != "needsAction" {
                        continue;
                    }

                    // Check if task has a due date and is due
                    if let Some(ref due_str) = task.due_time {
                        // Parse the due date string into a timestamp
                        if let Ok(due_time) = chrono::DateTime::parse_from_rfc3339(&due_str) {
                            let due_timestamp = due_time.timestamp();
                            let current_timestamp = now.timestamp();
                            let thirty_days_ago_timestamp = (now - chrono::Duration::days(30)).timestamp();
                            
                            // Compare timestamps directly
                            if due_timestamp <= current_timestamp && due_timestamp > thirty_days_ago_timestamp {
                                // Check if we've already notified about this task
                                match state.user_repository.get_task_notification(user.id, &task.id) {
                                    Ok(Some(_)) => continue, // Already notified
                                    Ok(None) => due_tasks.push(task),
                                    Err(e) => {
                                        error!("Failed to check task notification status: {}", e);
                                        continue;
                                    }
                                }
                            }
                        } else {
                            error!("Failed to parse due date '{}' for task {}", due_str, task.id);
                            continue;
                        }
                    }
                }

                if due_tasks.is_empty() {
                    continue;
                }

                // Check if we should process this notification based on messages left
                let should_continue = match state.user_repository.decrease_messages_left(user.id) {
                    Ok(msgs_left) => {
                        info!("User {} has {} messages left after decrease", user.id, msgs_left);
                        msgs_left > 0
                    },
                    Err(e) => {
                        error!("Failed to decrease messages left for user {}: {}", user.id, e);
                        false
                    }
                };

                if !should_continue {
                    info!("Skipping further processing for user {} due to no messages left", user.id);
                    continue;
                }

                // Get the user's preferred number or default
                let sender_number = match user.preferred_number.clone() {
                    Some(number) => {
                        info!("Using user's preferred number: {}", number);
                        number
                    },
                    None => {
                        let number = std::env::var("SHAZAM_PHONE_NUMBER").expect("SHAZAM_PHONE_NUMBER not set");
                        info!("Using default SHAZAM_PHONE_NUMBER: {}", number);
                        number
                    },
                };

                // Get the conversation for the user
                let conversation = match state.user_conversations.get_conversation(&user, sender_number).await {
                    Ok(conv) => conv,
                    Err(e) => {
                        error!("Failed to ensure conversation exists: {}", e);
                        continue;
                    }
                };

                // Format notification message
                let tasks_text = due_tasks.iter()
                    .map(|task| {
                        let desc = task.notes.as_deref().unwrap_or("").to_string();
                        if desc.is_empty() {
                            task.title.clone()
                        } else {
                            format!("{}: {}", task.title, desc)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                let notification = format!(
                    "You have {} overdue Google Tasks:\n\n{}",
                    due_tasks.len(),
                    tasks_text
                );

                // Send notification
                match twilio_utils::send_conversation_message(
                    &conversation.conversation_sid,
                    &conversation.twilio_number,
                    &notification
                ).await {
                    Ok(_) => {
                        info!("Successfully sent task notification to user {}", user.id);
                        
                        // Record notifications
                        let current_time = chrono::Utc::now().timestamp() as i32;
                        for task in due_tasks {
                            if let Err(e) = state.user_repository.create_task_notification(user.id, &task.id, current_time) {
                                error!("Failed to record task notification: {}", e);
                            }
                        }
                    },
                    Err(e) => error!("Failed to send task notification: {}", e),
                }
            }
        })
    }).expect("Failed to create task monitor job");

    sched.add(task_monitor_job).await.expect("Failed to add task monitor job to scheduler");
    */

    // Create a job that runs daily to clean up old task notifications
    let state_clone = Arc::clone(&state);
    let task_cleanup_job = Job::new_async("0 0 0 * * *", move |_, _| {  // Runs at midnight every day
        let state = state_clone.clone();
        Box::pin(async move {
            info!("Running task notification cleanup...");
            
            // Calculate timestamp for 30 days ago
            let thirty_days_ago = (chrono::Utc::now() - chrono::Duration::days(30)).timestamp() as i32;
            
            // Delete notifications for tasks that were due more than 30 days ago
            match state.user_repository.delete_old_task_notifications(thirty_days_ago) {
                Ok(count) => info!("Cleaned up {} old task notifications", count),
                Err(e) => error!("Failed to clean up old task notifications: {}", e),
            }
        })
    }).expect("Failed to create task cleanup job");

    sched.add(task_cleanup_job).await.expect("Failed to add task cleanup job to scheduler");

    // Create a job that runs every minute to check for upcoming calendar events
    let state_clone = Arc::clone(&state);
    let calendar_notification_job = Job::new_async("0 * * * * *", move |_, _| {
        let state = state_clone.clone();
        Box::pin(async move {
            
            // Clean up old notifications (older than 30 minutes)
            let cleanup_threshold = (chrono::Utc::now() - chrono::Duration::minutes(30)).timestamp() as i32;
            if let Err(e) = state.user_repository.cleanup_old_calendar_notifications(cleanup_threshold) {
                error!("Failed to clean up old calendar notifications: {}", e);
            }

            // Get all users with valid Google Calendar connection
            let users = match state.user_repository.get_all_users() {
                Ok(users) => users,
                Err(e) => {
                    error!("Failed to fetch users: {}", e);
                    return;
                }
            };

            for user in users {
                // Check if user has messages left and valid subscription
                match state.user_repository.has_valid_subscription_tier_with_messages(user.id, "tier 2") {
                    Ok(true) => (),
                    Ok(false) => {
                        tracing::debug!("User {} does not have valid subscription or messages left", user.id);
                        continue;
                    },
                    Err(e) => {
                        error!("Failed to check subscription status for user {}: {}", user.id, e);
                        continue;
                    }
                }

                // Check if user has enabled proactive calendar notifications and get last activation time
                let (is_enabled, last_activated) = match state.user_repository.get_proactive_calendar_status(user.id) {
                    Ok((enabled, timestamp)) => (enabled, timestamp),
                    Err(e) => {
                        error!("Failed to check proactive calendar setting for user {}: {}", user.id, e);
                        continue;
                    }
                };

                if !is_enabled {
                    tracing::debug!("User {} has not enabled proactive calendar notifications", user.id);
                    continue;
                }

                // Check if user has active Google Calendar connection
                match state.user_repository.has_active_google_calendar(user.id) {
                    Ok(true) => (),
                    Ok(false) => continue,
                    Err(e) => {
                        error!("Failed to check Google Calendar status for user {}: {}", user.id, e);
                        continue;
                    }
                }

                // Calculate time window for events (±5 minutes from now for more precise checking)
                let now = chrono::Utc::now();
                let window_start = now - chrono::Duration::minutes(5);
                let window_end = now + chrono::Duration::minutes(5);

                // Fetch events in the time window
                match crate::handlers::google_calendar::fetch_calendar_events(
                    &state,
                    user.id,
                    crate::handlers::google_calendar::TimeframeQuery {
                        start: window_start,
                        end: window_end,
                    }
                ).await {
                    Ok(events) => {
                        for event in events {
                            if let Some(reminders) = &event.reminders {
                                // Process each reminder override
                                for reminder in &reminders.overrides {
                                    if let Some(start_time) = event.start.date_time {
                                        let reminder_time = start_time - chrono::Duration::minutes(reminder.minutes as i64);
                                        let current_time = chrono::Utc::now();
                                        
                                        // Check if the reminder time has passed
                                        // Only process events that occurred after the last activation time
                                        if current_time >= reminder_time && start_time.timestamp() as i32 > last_activated {
                                            // Clone all necessary data before the async block
                                            let event_id = event.id.clone();
                                            let minutes = reminder.minutes;
                                            let reminder_time_key = format!("{}_{}", event_id, minutes);
                                            let notification_exists = state.user_repository
                                                .check_calendar_notification_exists(user.id, &reminder_time_key)
                                                .unwrap_or(true); // Skip if error occurs

                                            if !notification_exists {
                                                // Clone event details we need for the notification
                                                let event_summary = event.summary.clone().unwrap_or_else(|| "Untitled Event".to_string());
                                                let event_description = event.description.clone();
                                                
                                                // Get the user's preferred number or default
                                                let sender_number = match user.preferred_number.clone() {
                                                    Some(number) => {
                                                        info!("Using user's preferred number: {}", number);
                                                        number
                                                    },
                                                    None => {
                                                        let number = std::env::var("SHAZAM_PHONE_NUMBER").expect("SHAZAM_PHONE_NUMBER not set");
                                                        info!("Using default SHAZAM_PHONE_NUMBER: {}", number);
                                                        number
                                                    },
                                                };

                                                // Check if we should process this notification based on messages left
                                                let should_continue = match state.user_repository.decrease_messages_left(user.id) {
                                                    Ok(msgs_left) => {
                                                        info!("User {} has {} messages left after decrease", user.id, msgs_left);
                                                        msgs_left > 0
                                                    },
                                                    Err(e) => {
                                                        error!("Failed to decrease messages left for user {}: {}", user.id, e);
                                                        false
                                                    }
                                                };

                                                if !should_continue {
                                                    info!("Skipping calendar notification for user {} due to no messages left", user.id);
                                                    continue;
                                                }

                                                // Get the conversation for the user
                                                if let Ok(conversation) = state.user_conversations.get_conversation(&user, sender_number).await {
                                                    // Check if this is the final message
                                                    let is_final_message = user.msgs_left <= 1;

                                                    // Format notification message
                                                    let notification = format!(
                                                        "Reminder: Your event '{}' starts in {} minutes{}",
                                                        event_summary,
                                                        minutes,
                                                        event_description.map_or("".to_string(), |desc| format!("\nDetails: {}", desc))
                                                    );

                                                    // Append final message notice if needed
                                                    let final_notification = if is_final_message {
                                                        format!("{}\n\nNote: This is your final proactive message for this month. Your message quota will reset at the start of next month.", notification)
                                                    } else {
                                                        notification
                                                    };

                                                    // Clone necessary values for the new thread
                                                    let conversation_sid = conversation.conversation_sid.clone();
                                                    let twilio_number = conversation.twilio_number.clone();
                                                    let notification_clone = final_notification.clone();
                                                    let state_clone = Arc::clone(&state);
                                                    let user_id = user.id;
                                                    let reminder_time_key = reminder_time_key.clone();
                                                    let current_timestamp = current_time.timestamp() as i32;

                                                    // Spawn a new thread for sending the notification
                                                    tokio::spawn(async move {
                                                        match crate::api::twilio_utils::send_conversation_message(
                                                            &conversation_sid,
                                                            &twilio_number,
                                                            &notification_clone,
                                                            true,
                                                        ).await {
                                                            Ok(_) => {
                                                                info!("Successfully sent calendar reminder to user {}", user_id);
                                                                
                                                                // Record the notification
                                                                let new_notification = crate::models::user_models::NewCalendarNotification {
                                                                    user_id,
                                                                    event_id: reminder_time_key,
                                                                    notification_time: current_timestamp,
                                                                };
                                                                
                                                                if let Err(e) = state_clone.user_repository.create_calendar_notification(&new_notification) {
                                                                    error!("Failed to record calendar notification: {}", e);
                                                                }
                                                            },
                                                            Err(e) => error!("Failed to send calendar reminder: {}", e),
                                                        }
                                                    });
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    },
                    Err(e) => error!("Failed to fetch calendar events for user {}: {}", user.id, e),
                }
            }
        })
    }).expect("Failed to create calendar notification job");

    sched.add(calendar_notification_job).await.expect("Failed to add calendar notification job to scheduler");

    // Create a job that runs daily to manage Matrix sync tasks
    let state_clone = Arc::clone(&state);
    let matrix_sync_job = Job::new_async("0 * * * * *", move |_, _| {  // Runs at midnight every day
        let state = state_clone.clone();
        Box::pin(async move {
            tracing::debug!("Managing Matrix sync tasks...");
            
            // Get all users with active WhatsApp connection
            match state.user_repository.get_users_with_matrix_bridge_connections() {
                Ok(users) => {
                    let mut sync_tasks = state.matrix_sync_tasks.lock().await;
                    
                    // Remove any sync tasks for users who are no longer active
                    sync_tasks.retain(|user_id, task| {
                        if !users.contains(user_id) {
                            tracing::debug!("Removing sync task for inactive user {}", user_id);
                            task.abort();
                            false
                        } else {
                            true
                        }
                    });

                        // Start sync tasks for new active users
                        for user_id in users {
                            if !sync_tasks.contains_key(&user_id) {
                                tracing::debug!("Starting new sync task for user {}", user_id);
                                
                                // Create a new sync task
                                let state_clone = Arc::clone(&state);
                                let handle = tokio::spawn(async move {
                                    match crate::utils::matrix_auth::get_client(user_id, &state_clone.user_repository, false).await {
                                        Ok(client) => {
                                            info!("Starting Matrix sync for user {}", user_id);
                                            
                                            // Always add WhatsApp message handler - it will check proactive status internally
                                            info!("Adding WhatsApp message handler for user {}", user_id);
                                            use matrix_sdk::ruma::events::room::message::OriginalSyncRoomMessageEvent;
                                            use matrix_sdk::room::Room;
                                            use matrix_sdk::Client as MatrixClient;
                                            let state_for_handler = Arc::clone(&state_clone);
                                            client.add_event_handler(move |ev: OriginalSyncRoomMessageEvent, room: Room, client: MatrixClient| {
                                                let state = Arc::clone(&state_for_handler);
                                                async move {
                                                    crate::utils::whatsapp_utils::handle_whatsapp_message(ev, room, client, state).await;
                                                }
                                            });

                                            if let Err(e) = client.sync(matrix_sdk::config::SyncSettings::default()).await {
                                                error!("Matrix sync error for user {}: {}", user_id, e);
                                            }
                                        },
                                        Err(e) => {
                                            error!("Failed to get Matrix client for user {}: {}", user_id, e);
                                        }

                                    }
                                });
                                
                                sync_tasks.insert(user_id, handle);
                            }
                        }
                },
                Err(e) => error!("Failed to get active WhatsApp users: {}", e),
            }
        })
    }).expect("Failed to create matrix sync job");

    sched.add(matrix_sync_job).await.expect("Failed to add matrix sync job to scheduler");

    /*
    // Create a job that runs daily to manage Matrix invitation tasks
    let state_clone = Arc::clone(&state);
    let matrix_invitation_job = Job::new_async("0 * * * * *", move |_, _| {  // runs every fifteen minutes 
        let state = state_clone.clone();
        Box::pin(async move {
            tracing::debug!("Managing Matrix invitation tasks...");
            
            // Get all users with active WhatsApp connection
            match state.user_repository.get_users_with_matrix_bridge_connections() {
                Ok(users) => {
                    let mut invitation_tasks = state.matrix_invitation_tasks.lock().await;
                    
                    // Remove any invitation tasks for users who are no longer active
                    invitation_tasks.retain(|user_id, task| {
                        if !users.contains(user_id) {
                            tracing::debug!("Removing invitation task for inactive user {}", user_id);
                            task.abort();
                            false
                        } else {
                            true
                        }
                    });

                    // Start invitation tasks for new active users
                    for user_id in users {
                        if !invitation_tasks.contains_key(&user_id) {
                            tracing::debug!("Starting new invitation task for user {}", user_id);
                            
                            // Create a new invitation task
                            let state_clone = Arc::clone(&state);
                            let handle = tokio::spawn(async move {
                                match crate::utils::matrix_auth::get_client(user_id, &state_clone.user_repository, true).await {
                                    Ok(client) => {
                                        tracing::debug!("Starting Matrix invitation acceptance for user {}", user_id);
                                        // Run the invitation acceptance loop for 15 minutes
                                        if let Err(e) = crate::handlers::whatsapp_auth::accept_room_invitations(client, Duration::from_secs(900)).await {
                                            tracing::error!("Error in accept_room_invitations: {}", e);
                                        }
                                    },
                                    Err(e) => {
                                        error!("Failed to get Matrix client for user {}: {}", user_id, e);
                                    }
                                }
                            });
                            
                            invitation_tasks.insert(user_id, handle);
                        }
                    }
                },
                Err(e) => error!("Failed to get active WhatsApp users: {}", e),
            }
        })
    }).expect("Failed to create matrix invitation job");

    sched.add(matrix_invitation_job).await.expect("Failed to add matrix invitation job to scheduler");
    */
       
    // Start the scheduler
    sched.start().await.expect("Failed to start scheduler");

    // TODO we should add another scheduled call that just checks if there are items that are 'done' or not found in the elevenlabs
    // but are still 'ongoing' in our db. we don't want to be accidentally charging users.
    // and if that happens make error visible

}


