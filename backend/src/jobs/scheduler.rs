use tokio_cron_scheduler::{JobScheduler, Job};
use std::sync::Arc;
use tracing::{debug, error};
use crate::AppState;

use crate::handlers::imap_handlers::ImapEmailPreview;

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
                                tracing::debug!("User {} does not have valid subscription or messages left for message monitoring", user.id);
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
                        debug!("Checking IMAP messages for user {} (activated since timestamp {})", user.id, last_activated);
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
                                                    debug!("Deleted old processed email {} for user {}", email.email_uid, user.id);
                                                }
                                            }

                                            // Update the original collection
                                            processed_emails.truncate(keep_count);

                                            // Also clean up old email judgments
                                            if let Err(e) = state.user_repository.delete_old_email_judgments(user.id) {
                                                error!("Failed to delete old email judgments for user {}: {}", user.id, e);
                                            } else {
                                                debug!("Successfully cleaned up old email judgments for user {}", user.id);
                                            }
                                        }
                                    }
                                    Err(e) => error!("Failed to fetch processed emails for garbage collection: {}", e),
                                }
                                
                                if !emails.is_empty() {
                                    // Sort emails by date in descending order (most recent first)
                                    let mut sorted_emails = emails;
                                    sorted_emails.sort_by(|a, b| {
                                        let a_date = a.date.unwrap_or_else(|| chrono::Utc::now());
                                        let b_date = b.date.unwrap_or_else(|| chrono::Utc::now());
                                        b_date.cmp(&a_date)
                                    });
                                    
                                    // Take only the most recent email
                                    let latest_email = sorted_emails.into_iter().next().unwrap();
                                    let emails = vec![latest_email];


                                    let importance_priority = match state.user_repository.get_importance_priority(user.id, "imap") {
                                        Ok(Some(priority)) => priority.threshold,
                                        Ok(None) => 7, // Default threshold
                                        Err(e) => {
                                            error!("Failed to get importance priority for user {}: {}", user.id, e);
                                            7 // Default threshold on error
                                        }
                                    };
                                    // Create OpenAI client
                                    let api_key = match env::var("OPENROUTER_API_KEY") {
                                        Ok(key) => key,
                                        Err(e) => {
                                            error!("Failed to get OPENROUTER_API_KEY: {}", e);
                                            continue;
                                        }
                                    };

                                    let client = match openai_api_rs::v1::api::OpenAIClient::builder()
                                        .with_endpoint("https://openrouter.ai/api/v1")
                                        .with_api_key(api_key)
                                        .build() {
                                        Ok(client) => client,
                                        Err(e) => {
                                            error!("Failed to build OpenAI client: {}", e);
                                            continue;
                                        }
                                    };

                                    let mut important_emails: Option<Vec<(ImapEmailPreview, serde_json::Value)>> = None;

                                    match crate::utils::imap_utils::judge_email_importance(&state, user.id, emails.clone(), importance_priority, last_activated).await {
                                        Ok(important_emails_with_evaluations) => {
                                            if important_emails_with_evaluations.is_empty() {
                                                debug!("No important emails found for user {}", user.id);
                                                continue;
                                            }

                                            // Store the important emails for later use
                                            important_emails = Some(important_emails_with_evaluations.clone());

                                            for (email, evaluation) in &important_emails_with_evaluations {
                                                debug!("Parsed evaluation");

                                                let current_time = std::time::SystemTime::now()
                                                    .duration_since(std::time::UNIX_EPOCH)
                                                    .unwrap()
                                                    .as_secs() as i32;

                                                let new_judgment = crate::models::user_models::NewEmailJudgment {
                                                    user_id: user.id,
                                                    email_timestamp: evaluation["email_timestamp"].as_i64().unwrap_or(0) as i32,
                                                    processed_at: current_time,
                                                    should_notify: evaluation["should_notify"].as_bool().unwrap_or(false),
                                                    score: evaluation["score"].as_i64().unwrap_or(0) as i32,
                                                    reason: evaluation["reason"].as_str().unwrap_or("No reason provided").to_string(),
                                                };

                                                if let Err(e) = state.user_repository.create_email_judgment(&new_judgment) {
                                                    error!("Failed to create email judgment log: {}", e);
                                                } else {
                                                    debug!("Successfully created email judgment log for email");
                                                }

                                                // Check if notification was due to a waiting check
                                                if let Some(matched_check_id) = evaluation["matched_waiting_check"].as_i64() {
                                                    debug!("Matched waiting check ID: {}", matched_check_id);
                                                    // Get waiting checks again to handle the removal
                                                    if let Ok(waiting_checks) = state.user_repository.get_waiting_checks(user.id, "imap") {
                                                        if let Some(check) = waiting_checks.iter().find(|wc| wc.id == Some(matched_check_id as i32)) {
                                                            if check.remove_when_found {
                                                                debug!("Removing waiting check with ID {}", matched_check_id);
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

                                                }

                                            }
                                        }
                                        Err(e) => {
                                            error!("Failed to judge email importance: {}", e);
                                            continue;
                                        }
                                    }

                                    // Check if we should process this notification based on messages left
                                    let should_continue = match state.user_repository.decrease_messages_left(user.id) {
                                        Ok(msgs_left) => {
                                            debug!("User {} has {} messages left after decrease", user.id, msgs_left);
                                            msgs_left > 0
                                        },
                                        Err(e) => {
                                            error!("Failed to decrease messages left for user {}: {}", user.id, e);
                                            false
                                        }
                                    };

                                    if !should_continue {
                                        debug!("Skipping further processing for user {} due to no messages left", user.id);
                                        continue;
                                    }
                                    // Get user settings first
                                    let user_settings = match state.user_repository.get_user_settings(user.id) {
                                        Ok(settings) => settings,
                                        Err(e) => {
                                            error!("Failed to get user settings: {}", e);
                                            continue;
                                        }
                                    };

                                    let sender_number = match user.preferred_number.clone() {
                                        Some(number) => {
                                            tracing::debug!("Using user's preferred number: {}", number);
                                            number
                                        },
                                        None => {
                                            let number = std::env::var("SHAZAM_PHONE_NUMBER").expect("SHAZAM_PHONE_NUMBER not set");
                                            tracing::debug!("Using default SHAZAM_PHONE_NUMBER: {}", number);
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
                                    let email_summaries = important_emails.as_ref().unwrap().iter()
                                        .take(3) // Still limit to 3 emails
                                        .map(|(email, _)| format!(
                                            "From: {} \nSubject: {}\nBody: {}",
                                            email.from_email.as_deref().unwrap_or("unknown@email.com"),
                                            email.subject.as_deref().unwrap_or("No subject"),
                                            email.body.as_deref().unwrap_or("No body")
                                        ))
                                        .collect::<Vec<_>>()
                                        .join("\n\n---\n\n");

                                    // Define the system message for notification formatting
                                    let format_system_message = "You are an AI assistant that creates very concise SMS notifications about important emails. \
                                        Create notifications that are STRICTLY under 120 characters and use ONLY ASCII characters (no emoji, no Unicode). \
                                        Focus only on the most critical information. \
                                        Always start with 'Email:' and be direct and brief. \
                                        Never use quotes or special characters.";

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
                                                description: Some(String::from("Format a brief ASCII-only notification under 120 chars")),
                                                parameters: openai_api_rs::v1::types::FunctionParameters {
                                                    schema_type: openai_api_rs::v1::types::JSONSchemaType::Object,
                                                    properties: Some(format_properties),
                                                    required: Some(vec![String::from("notification_text")]),
                                                },
                                            },
                                        },
                                    ];

                                    // Add a post-processing step to ensure notification length and character constraints
                                    let process_notification = |notification: String| -> String {
                                        // Remove any non-ASCII characters
                                        let ascii_only: String = notification.chars()
                                            .filter(|c| c.is_ascii())
                                            .collect();
                                        // Truncate to 120 chars if needed, ensuring we don't panic on unicode boundaries
                                        if ascii_only.len() > 120 {
                                            ascii_only.char_indices()
                                                .take_while(|(i, _)| *i < 120)
                                                .map(|(_, c)| c)
                                                .collect()
                                        } else {
                                            ascii_only
                                        }
                                    };

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
                                                important_emails.clone().unwrap().len(),
                                                email_summaries
                                            )),
                                            name: None,
                                            tool_calls: None,
                                            tool_call_id: None,
                                        },
                                    ];

                                    let format_req = openai_api_rs::v1::chat_completion::ChatCompletionRequest::new(
                                        "meta-llama/llama-4-maverick".to_string(),
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
                                                                process_notification(
                                                                    formatted["notification_text"]
                                                                        .as_str()
                                                                        .unwrap_or("Email: New important message received.")
                                                                        .to_string()
                                                                )
                                                            }
                                                            Err(e) => {
                                                                error!("Failed to parse notification format response: {}", e);
                                                                format!("Email: {} new important messages.", important_emails.unwrap().len())
                                                            }
                                                        }
                                                    } else {
                                                        format!("Email: {} new important messages.", important_emails.unwrap().len())
                                                    }
                                                } else {
                                                    format!("Email: {} new important messages.", important_emails.unwrap().len())
                                                }
                                            } else {
                                                format!("Email: {} new important messages.", important_emails.unwrap().len())
                                            }
                                        }
                                        Err(e) => {
                                            error!("Failed to get notification format from LLM: {}", e);
                                            format!("Email: {} new important messages.", important_emails.unwrap().len())
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

                                    // Check user's notification preference from settings
                                    let notification_type = user_settings.notification_type.as_deref().unwrap_or("sms");
                                    match notification_type {
                                        "call" => {
                                            // For calls, we need a brief intro and detailed message
                                            let notification_first_message = "Hello, I have an important email to tell you about.".to_string();
                                            
                                            // Extract the email ID from the latest email
                                            let email_id = emails.first().map(|email| email.id.clone()).unwrap_or_default();
                                            
                                            // Create dynamic variables with email ID
                                            let mut dynamic_vars = std::collections::HashMap::new();
                                            dynamic_vars.insert("email_id".to_string(), email_id.clone());
                                            
                                            match crate::api::elevenlabs::make_notification_call(
                                                &state.clone(),
                                                user.phone_number.clone(),
                                                user.preferred_number.unwrap_or_else(|| std::env::var("SHAZAM_PHONE_NUMBER").expect("SHAZAM_PHONE_NUMBER not set")),
                                                email_id,
                                                "email".to_string(),

                                                notification_first_message,
                                                final_notification.clone(),
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
                                                    debug!("Successfully initiated call notification for user {} with email ID", user.id);
                                                },
                                                Err((_, json_err)) => error!("Failed to initiate call notification: {:?}", json_err),
                                            }
                                        },


                                        _ => { // Default to SMS for "sms" or None
                                            match twilio_utils::send_conversation_message(
                                                &conversation.conversation_sid,
                                                &conversation.twilio_number,
                                                &final_notification,
                                                true,
                                                &user,
                                            ).await {
                                                Ok(_) => debug!("Successfully sent email notification to user {}", user.id),
                                                Err(e) => error!("Failed to send email notification: {}", e),
                                            }
                                        }
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
                let state_clone = Arc::clone(&state);
                match state_clone.user_repository.has_valid_subscription_tier_with_messages(user.id, "tier 2") {
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
                                        debug!("has auto top up");
                                        debug!("conversation_data status: {}",conversation_data["status"]);
                                        debug!("conversation_data : {}",conversation_data);
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
                                    debug!("deleting conversation");
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
                        debug!("User {} does not have valid subscription or messages left", user.id);
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
                        debug!("User {} has not enabled proactive calendar notifications", user.id);
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
                        debug!("User {} has {} messages left after decrease", user.id, msgs_left);
                        msgs_left > 0
                    },
                    Err(e) => {
                        error!("Failed to decrease messages left for user {}: {}", user.id, e);
                        false
                    }
                };

                if !should_continue {
                    debug!("Skipping further processing for user {} due to no messages left", user.id);
                    continue;
                }

                // Get the user's preferred number or default
                let sender_number = match user.preferred_number.clone() {
                    Some(number) => {
                        debug!("Using user's preferred number: {}", number);
                        number
                    },
                    None => {
                        let number = std::env::var("SHAZAM_PHONE_NUMBER").expect("SHAZAM_PHONE_NUMBER not set");
                        debug!("Using default SHAZAM_PHONE_NUMBER: {}", number);
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
                        debug!("Successfully sent task notification to user {}", user.id);
                        
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
            debug!("Running task notification cleanup...");
            
            // Calculate timestamp for 30 days ago
            let thirty_days_ago = (chrono::Utc::now() - chrono::Duration::days(30)).timestamp() as i32;
            
            // Delete notifications for tasks that were due more than 30 days ago
            match state.user_repository.delete_old_task_notifications(thirty_days_ago) {
                Ok(count) => debug!("Cleaned up {} old task notifications", count),
                Err(e) => error!("Failed to clean up old task notifications: {}", e),
            }
        })
    }).expect("Failed to create task cleanup job");

    sched.add(task_cleanup_job).await.expect("Failed to add task cleanup job to scheduler");

    // Create a job that runs at midnight UTC to reset credits_left for subscribers
    let state_clone = Arc::clone(&state);
    let credits_reset_job = Job::new_async("0 0 0 * * *", move |_, _| {
        let state = state_clone.clone();
        Box::pin(async move {
            debug!("Running daily credits reset for subscribers...");
            
            // Get all users
            match state.user_repository.get_all_users() {
                Ok(users) => {
                    for user in users {
                        // Check if user has a subscription
                        if let Ok(Some(tier)) = state.user_repository.get_subscription_tier(user.id) {
                            // Get user settings to check country
                            match state.user_repository.get_user_settings(user.id) {
                                Ok(settings) => {

                                    // Skip if sub_country is not set (used to block older subs)
                                    if settings.sub_country.is_none() {
                                        debug!("Skipping credits reset for user {} - no subscription country set", user.id);
                                        continue;
                                    }

                                    // Define credits based on country and tier
                                    let new_credits = match (settings.sub_country.as_deref(), tier.as_str()) {
                                        // Tier 1 (Basic Plan) credits
                                        (Some("US"), "tier 1") => Ok(10.0),  // United States: 10/day
                                        (Some("FI"), "tier 1") => Ok(4.0),   // Finland: 4/day
                                        (Some("UK"), "tier 1") => Ok(4.0),   // United Kingdom: 4/day
                                        (Some("AU"), "tier 1") => Ok(3.0),   // Australia: 3/day
                                        (Some("IL"), "tier 1") => Ok(2.0),   // Israel: 2/day
                                        
                                        // Tier 2 (Escape Plan) credits
                                        (Some("US"), "tier 2") => Ok(15.0),  // United States: 15/day
                                        (Some("FI"), "tier 2") => Ok(10.0),  // Finland: 10/day
                                        (Some("UK"), "tier 2") => Ok(10.0),  // United Kingdom: 10/day
                                        (Some("AU"), "tier 2") => Ok(6.0),   // Australia: 6/day
                                        (Some("IL"), "tier 2") => Ok(3.0),   // Israel: 3/day
                                        
                                        (Some(country), tier_type) => {
                                            error!("Invalid country/tier combination for user {}: country '{}', tier '{}'", user.id, country, tier_type);
                                            Err(format!("Unsupported country '{}' for tier '{}'", country, tier_type))
                                        },
                                        (None, tier_type) => {
                                            error!("Missing country for user {} with tier '{}'", user.id, tier_type);
                                            Err("Country not set for user".to_string())
                                        }
                                    };

                                    match new_credits {
                                        Ok(credits) => {
                                            // Reset credits_left for the user
                                            if let Err(e) = state.user_repository.update_sub_credits(user.id, credits) {
                                                error!("Failed to reset credits for user {}: {}", user.id, e);
                                            } else {
                                                debug!("Successfully reset credits_left to {} for user {} (tier: {}, country: {})", 
                                                    credits, 
                                                    user.id, 
                                                    tier,
                                                    settings.sub_country.as_deref().unwrap_or("unknown")
                                                );
                                            }
                                        },
                                        Err(e) => {
                                            error!("Failed to determine credits for user {}: {}", user.id, e);
                                            // Consider sending an alert or notification here for admin attention
                                        }
                                    }

                                }
                                Err(e) => error!("Failed to get settings for user {}: {}", user.id, e),
                            }
                        }
                    }
                }
                Err(e) => error!("Failed to fetch users for credits reset: {}", e),
            }
        })
    }).expect("Failed to create credits reset job");

    sched.add(credits_reset_job).await.expect("Failed to add credits reset job to scheduler");

                // Create a job that runs every minute to check for upcoming calendar events
                let state_clone = Arc::clone(&state);
                let calendar_notification_job = Job::new_async("0 * * * * *", move |_, _| {
                    let state = state_clone.clone();
                    Box::pin(async move {
                        // Ensure all error types implement Send + Sync
                        type BoxError = Box<dyn std::error::Error + Send + Sync>;
                        // Clean up old notifications (older than 24 hours)
                        let cleanup_threshold = (chrono::Utc::now() - chrono::Duration::hours(24)).timestamp() as i32;
                        if let Err(e) = state.user_repository.cleanup_old_calendar_notifications(cleanup_threshold) {
                            error!("Failed to clean up old calendar notifications: {}", e);
                        }

                        // Get all users with valid Google Calendar connection and subscription
                        let users = match state.user_repository.get_all_users() {
                            Ok(users) => users.into_iter().filter(|user| {
                                // Check subscription and calendar status
                                matches!(state.user_repository.has_valid_subscription_tier_with_messages(user.id, "tier 2"), Ok(true)) &&
                                matches!(state.user_repository.has_active_google_calendar(user.id), Ok(true)) &&
                                matches!(state.user_repository.get_proactive_calendar_status(user.id), Ok((true, _)))
                            }).collect::<Vec<_>>(),
                            Err(e) => {
                                error!("Failed to fetch users: {}", e);
                                return;
                            }
                        };

                        let now = chrono::Utc::now();
                        let window_end = now + chrono::Duration::minutes(30);

                        debug!("🗓️ Calendar check: Checking events between {} and {}", 
                            now.format("%Y-%m-%d %H:%M:%S UTC"),
                            window_end.format("%Y-%m-%d %H:%M:%S UTC")
                        );

                        for user in users {
                            debug!("🗓️ Calendar check: Fetching events for user {}", user.id);
                            // Get the last activation time
                            let last_activated = match state.user_repository.get_proactive_calendar_status(user.id) {
                                Ok((_, timestamp)) => timestamp,
                                Err(e) => {
                                    error!("Failed to get last activation time for user {}: {}", user.id, e);
                                    continue;
                                }
                            };

                            // Fetch upcoming events
                            match crate::handlers::google_calendar::fetch_calendar_events(
                                &state,
                                user.id,
                                crate::handlers::google_calendar::TimeframeQuery {
                                    start: now,
                                    end: window_end,
                                }
                            ).await {
                                Ok(events) => {
                                    debug!("🗓️ Calendar check: Found {} events for user {}", events.len(), user.id);
                                    for event in events {
                                        if let (Some(reminders), Some(start_time)) = (&event.reminders, event.start.date_time) {
                                            for reminder in &reminders.overrides {
                                                let reminder_time = start_time - chrono::Duration::minutes(reminder.minutes as i64);
                                                
                                                    // Only process if reminder should fire now or has just passed
                                                    if reminder_time <= now && start_time.timestamp() as i32 > last_activated {
                                                        tracing::debug!("🗓️ Calendar check: Processing reminder for event ({}min before)", 
                                                            reminder.minutes);
                                                    let reminder_key = format!("{}_{}", event.id, reminder.minutes);
                                                    
                                                    // Check if notification was already sent
                                                    if state.user_repository.check_calendar_notification_exists(user.id, &reminder_key).unwrap_or(true) {
                                                        continue;
                                                    }

                                                    // Record notification before sending
                                                    let new_notification = crate::models::user_models::NewCalendarNotification {
                                                        user_id: user.id,
                                                        event_id: reminder_key.clone(),
                                                        notification_time: now.timestamp() as i32,
                                                    };
                                                    
                                                    if let Err(e) = state.user_repository.create_calendar_notification(&new_notification) {
                                                        error!("Failed to record calendar notification: {}", e);
                                                        continue;
                                                    }

                                                    // Decrease message count
                                                    if !matches!(state.user_repository.decrease_messages_left(user.id), Ok(count) if count > 0) {
                                                        continue;
                                                    }

                                                    let event_summary = event.summary.clone().unwrap_or_else(|| "Untitled Event".to_string());
                                                    let notification = format!("Calendar: {} in {} mins", event_summary, reminder.minutes);
                                                    
                                                    let sender_number = user.preferred_number.clone()
                                                        .unwrap_or_else(|| std::env::var("SHAZAM_PHONE_NUMBER").expect("SHAZAM_PHONE_NUMBER not set"));

                                                    // Send notification based on user preference
                                                    // Get user settings
                                                    let user_settings = match state.user_repository.get_user_settings(user.id) {
                                                        Ok(settings) => settings,
                                                        Err(e) => {
                                                            error!("Failed to get user settings: {}", e);
                                                            continue;
                                                        }
                                                    };

                                                    debug!("🗓️ Calendar notification: Preparing to send notification for user {} via {}", 
                                                        user.id, 
                                                        user_settings.notification_type.as_deref().unwrap_or("sms"));

                                                    match user_settings.notification_type.as_deref().unwrap_or("sms") {
                                                        "call" => {
                                                            debug!("🗓️ Calendar notification: Initiating call notification process for user {}", user.id);
                                                            
                                                            // Clone necessary data for the new thread
                                                            let state_clone = Arc::clone(&state);
                                                            let user_clone = user.clone();
                                                            let sender_number_clone = sender_number.clone();
                                                            let notification_clone = notification.clone();
                                                            
                                                            // Convert error type to a Send + Sync error before spawning
                                                            debug!("🗓️ Calendar notification: Attempting to get conversation for call - user {}", user.id);
                                                            let conversation_result = state_clone.user_conversations
                                                                .get_conversation(&user_clone, sender_number_clone.clone()).await;

                                                            match conversation_result {
                                                                Ok(_conversation) => {
                                                                    debug!("🗓️ Calendar notification: Successfully got conversation for call - user {}", user.id);
                                                                    let intro = "Hello, I have a calendar event to tell you about.".to_string();
                                                                    
                                                                    // No need to convert error type, just handle the error directly
                                                                    tokio::spawn(async move {
                                                                        debug!("🗓️ Calendar notification: Initiating ElevenLabs call for user {}", user_clone.id);
                                                                        match crate::api::elevenlabs::make_notification_call(
                                                                            &state_clone,
                                                                            user_clone.phone_number.clone(),
                                                                            sender_number_clone,
                                                                            "1".to_string(),
                                                                            "calendar".to_string(),
                                                                            intro,
                                                                            notification_clone,
                                                                            user_clone.id.to_string(),
                                                                user_settings.timezone.clone(),
                                                                        ).await {
                                                                            Ok(_) => debug!("🗓️ Calendar notification: Successfully completed call notification for user {}", user_clone.id),
                                                                            Err((_, e)) => error!("🗓️ Calendar notification: Failed to make call notification for user {}: {:?}", user_clone.id, e),
                                                                        }
                                                                    });
                                                                }
                                                                Err(e) => error!("🗓️ Calendar notification: Failed to get conversation for call - user {}: {:?}", user.id, e),
                                                            }
                                                        },
                                                        _ => {
                                                            debug!("🗓️ Calendar notification: Initiating SMS notification process for user {}", user.id);
                                                            
                                                            // Clone necessary data for the new thread
                                                            let state_clone = Arc::clone(&state);
                                                            let user_clone = user.clone();
                                                            let sender_number_clone = sender_number;
                                                            let notification_clone = notification;
                                                            
                                                            // Get conversation before spawning the thread
                                                            debug!("🗓️ Calendar notification: Attempting to get conversation for SMS - user {}", user.id);
                                                            match state_clone.user_conversations.get_conversation(&user_clone, sender_number_clone.clone()).await {
                                                                Ok(conversation) => {
                                                                    debug!("🗓️ Calendar notification: Successfully got conversation for SMS - user {}", user.id);
                                                                    // Now spawn the thread with the conversation already retrieved
                                                                    tokio::spawn(async move {
                                                                        debug!("🗓️ Calendar notification: Sending SMS via Twilio for user {}", user_clone.id);
                                                                        match twilio_utils::send_conversation_message(
                                                                            &conversation.conversation_sid,
                                                                            &conversation.twilio_number,
                                                                            &notification_clone,
                                                                            false,
                                                                            &user_clone,
                                                                        ).await {
                                                                            Ok(_) => debug!("🗓️ Calendar notification: Successfully sent SMS notification to user {}", user_clone.id),
                                                                            Err(e) => error!("🗓️ Calendar notification: Failed to send SMS notification to user {}: {}", user_clone.id, e),
                                                                        }
                                                                    });
                                                                }
                                                                Err(e) => error!("🗓️ Calendar notification: Failed to get conversation for SMS - user {}: {:?}", user.id, e),
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
                                    // Try to get cached client first
                                    match crate::utils::matrix_auth::get_cached_client(
                                        user_id,
                                        &state_clone.user_repository,
                                        false,
                                        &state_clone.matrix_clients
                                    ).await {
                                        Ok(client) => {
                                            debug!("Starting Matrix sync for user {}", user_id);
                                            
                                            // Always add WhatsApp message handler - it will check proactive status internally
                                            println!("Adding WhatsApp message handler for user {}", user_id);
                                            use matrix_sdk::ruma::events::room::message::OriginalSyncRoomMessageEvent;
                                            use matrix_sdk::room::Room;
                                            use matrix_sdk::Client as MatrixClient;
                                            
                                            // First ensure the client is logged in and ready
                                            if let Err(e) = client.sync_once(matrix_sdk::config::SyncSettings::default()).await {
                                                error!("Initial sync failed for user {}: {}", user_id, e);
                                                return;
                                            }

                                            let state_for_handler = Arc::clone(&state_clone);
                                            client.add_event_handler(move |ev: OriginalSyncRoomMessageEvent, room: Room, client: MatrixClient| {
                                                let state = Arc::clone(&state_for_handler);
                                                async move {
                                                    tracing::debug!("📨 Received message in room {}: {:?}", room.room_id(), ev);
                                                    crate::utils::whatsapp_utils::handle_whatsapp_message(ev, room, client, state).await;
                                                }
                                            });

                                            // Configure sync settings with appropriate timeouts and full state
                                            let sync_settings = matrix_sdk::config::SyncSettings::default()
                                                .timeout(std::time::Duration::from_secs(30))
                                                .full_state(true);

                                            tracing::debug!("Starting continuous sync for user {}", user_id);
                                            match client.sync(sync_settings.clone()).await {
                                                Ok(_) => {
                                                    tracing::debug!("Sync completed normally for user {}", user_id);
                                                },
                                                Err(e) => {
                                                    error!("Matrix sync error for user {}: {}", user_id, e);
                                                    
                                                    // Try to recover the client
                                                    if let Ok(new_client) = crate::utils::matrix_auth::get_client(user_id, &state_clone.user_repository, true).await {
                                                        tracing::debug!("Successfully recovered client for user {}", user_id);
                                                        if let Err(e) = new_client.sync(sync_settings).await {
                                                            error!("Recovered client sync failed for user {}: {}", user_id, e);
                                                        }
                                                    }
                                                }
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


