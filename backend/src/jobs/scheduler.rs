use tokio_cron_scheduler::{JobScheduler, Job};
use axum::{
    http::StatusCode,
};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{info, error};
use crate::AppState;

//use crate::handlers::gmail;

use openai_api_rs::v1::common::GPT4_O;



use std::env;

use crate::handlers::imap_handlers;
use crate::api::twilio_utils;
use crate::handlers::google_tasks::{self, Task};

pub async fn start_scheduler(state: Arc<AppState>) {
    let sched = JobScheduler::new().await.expect("Failed to create scheduler");

    // Function to run continuous sync for a user
    async fn run_sync_task(state: Arc<AppState>, user_id: i32) {
        tracing::info!("Starting sync task for user {}", user_id);
        
        loop {
            match crate::utils::matrix_auth::get_client(user_id, &state.user_repository, false).await {
                Ok(client) => {
                    match client.sync_once(matrix_sdk::config::SyncSettings::default()).await {
                        Ok(_) => {
                            tracing::debug!("Sync completed successfully for user {}", user_id);
                        },
                        Err(e) => {
                            tracing::error!("Sync failed for user {}: {}", user_id, e);
                        }
                    }
                },
                Err(e) => {
                    tracing::error!("Failed to get Matrix client for user {}: {}", user_id, e);
                }
            }
            
            // Check if user still has active bridge before continuing
            match state.user_repository.get_whatsapp_bridge(user_id) {
                Ok(Some(_)) => {
                    sleep(Duration::from_secs(30)).await; // Wait before next sync
                    continue;
                },
                _ => {
                    tracing::info!("WhatsApp bridge no longer active for user {}, stopping sync task", user_id);
                    break;
                }
            }
        }
        
        // Remove task handle when finished
        let mut sync_tasks = state.matrix_sync_tasks.lock().await;
        sync_tasks.remove(&user_id);
    }

    // Function to run continuous invitation check for a user
    async fn run_invitation_task(state: Arc<AppState>, user_id: i32) {
        tracing::info!("Starting invitation task for user {}", user_id);
        
        loop {
            match crate::utils::matrix_auth::get_client(user_id, &state.user_repository, true).await {
                Ok(client) => {
                    match crate::utils::matrix_auth::join_invited_rooms(&client).await {
                        Ok(count) => {
                            if count > 0 {
                                tracing::info!("Joined {} rooms for user {}", count, user_id);
                            }
                        },
                        Err(e) => {
                            tracing::error!("Failed to join rooms for user {}: {}", user_id, e);
                        }
                    }
                },
                Err(e) => {
                    tracing::error!("Failed to get Matrix client for user {}: {}", user_id, e);
                }
            }
            
            // Check if user still has active bridge before continuing
            match state.user_repository.get_whatsapp_bridge(user_id) {
                Ok(Some(_)) => {
                    sleep(Duration::from_secs(60)).await; // Check invitations every minute
                    continue;
                },
                _ => {
                    tracing::info!("WhatsApp bridge no longer active for user {}, stopping invitation task", user_id);
                    break;
                }
            }
        }
        
        // Remove task handle when finished
        let mut invitation_tasks = state.matrix_invitation_tasks.lock().await;
        invitation_tasks.remove(&user_id);
    }

    // Start WhatsApp bridge maintenance for existing users
    let state_clone = Arc::clone(&state);
    tokio::spawn(async move {
        match state_clone.user_repository.get_users_with_matrix_bridge_connections() {
            Ok(user_ids) => {
                for user_id in user_ids {
                    if let Ok(Some(_)) = state_clone.user_repository.get_whatsapp_bridge(user_id) {
                        // Start sync task if not already running
                        let mut sync_tasks = state_clone.matrix_sync_tasks.lock().await;
                        if !sync_tasks.contains_key(&user_id) {
                            let state_for_sync = Arc::clone(&state_clone);
                            let sync_handle = tokio::spawn(async move {
                                run_sync_task(state_for_sync, user_id).await;
                            });
                            sync_tasks.insert(user_id, sync_handle);
                        }
                        
                        // Start invitation task if not already running
                        let mut invitation_tasks = state_clone.matrix_invitation_tasks.lock().await;
                        if !invitation_tasks.contains_key(&user_id) {
                            let state_for_invitation = Arc::clone(&state_clone);
                            let invitation_handle = tokio::spawn(async move {
                                run_invitation_task(state_for_invitation, user_id).await;
                            });
                            invitation_tasks.insert(user_id, invitation_handle);
                        }
                    }
                }
            },
            Err(e) => {
                tracing::error!("Failed to get users with Matrix bridge connections: {}", e);
            }
        }
    });

    // Create a job that runs every 5 minutes to check and restart tasks if needed
    let state_clone = Arc::clone(&state);
    let whatsapp_maintenance_job = Job::new_async("0 */5 * * * *", move |_, _| {
        let state = state_clone.clone();
        Box::pin(async move {
            tracing::info!("Running WhatsApp bridge maintenance check...");
            
            // Clean up completed or failed tasks
            let mut sync_tasks = state.matrix_sync_tasks.lock().await;
            let mut invitation_tasks = state.matrix_invitation_tasks.lock().await;
            
            sync_tasks.retain(|&user_id, handle| {
                if handle.is_finished() {
                    tracing::info!("Removing completed sync task for user {}", user_id);
                    false
                } else {
                    true
                }
            });
            
            invitation_tasks.retain(|&user_id, handle| {
                if handle.is_finished() {
                    tracing::info!("Removing completed invitation task for user {}", user_id);
                    false
                } else {
                    true
                }
            });
            
            // Start tasks for users who need them
            if let Ok(user_ids) = state.user_repository.get_users_with_matrix_bridge_connections() {
                for user_id in user_ids {
                    if let Ok(Some(_)) = state.user_repository.get_whatsapp_bridge(user_id) {
                        // Start sync task if not running
                        if !sync_tasks.contains_key(&user_id) {
                            let state_for_sync = Arc::clone(&state);
                            let sync_handle = tokio::spawn(async move {
                                run_sync_task(state_for_sync, user_id).await;
                            });
                            sync_tasks.insert(user_id, sync_handle);
                            tracing::info!("Started new sync task for user {}", user_id);
                        }
                        
                        // Start invitation task if not running
                        if !invitation_tasks.contains_key(&user_id) {
                            let state_for_invitation = Arc::clone(&state);
                            let invitation_handle = tokio::spawn(async move {
                                run_invitation_task(state_for_invitation, user_id).await;
                            });
                            invitation_tasks.insert(user_id, invitation_handle);
                            tracing::info!("Started new invitation task for user {}", user_id);
                        }
                    }

                }
            }
        })
    }).expect("Failed to create WhatsApp maintenance job");

    sched.add(whatsapp_maintenance_job).await.expect("Failed to add WhatsApp maintenance job to scheduler");

    // Create a job that runs every minute to check for new messages across services
    let state_clone = Arc::clone(&state);
    let message_monitor_job = Job::new_async("0 * * * * *", move |_, _| {
        let state = state_clone.clone();
        Box::pin(async move {
            info!("Running scheduled message check across services...");
            
            // Get all users with valid message monitor subscription
            let users_with_subscription = match state.user_repository.get_all_users() {
                Ok(users) => {
                    let mut subscribed_users = Vec::new();
                    for user in users {
                        match state.user_repository.has_valid_subscription_tier_with_messages(user.id, "tier 1") {
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
                    if imap_users.contains(&user.id) && user.imap_proactive {
                        info!("Checking IMAP messages for user {}", user.id);
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
                                        &final_notification
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
                match state.user_repository.has_valid_subscription_tier_with_messages(user.id, "tier 1") {
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
                                            println!("Recharging the user back up");
                                            use axum::extract::{State, Path};
                                            let state_clone = Arc::clone(&state);
                                            tokio::spawn(async move {
                                                let _ = crate::handlers::stripe_handlers::automatic_charge(
                                                    State(state_clone),
                                                    Path(log.user_id),
                                                ).await;
                                                println!("Recharged the user successfully back up!");
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

    // Create a job that runs every minute to check for due tasks
    let state_clone = Arc::clone(&state);
    let task_monitor_job = Job::new_async("0 * * * * *", move |_, _| {
        let state = state_clone.clone();
        Box::pin(async move {
            info!("Running scheduled task check...");
            
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
                match state.user_repository.has_valid_subscription_tier_with_messages(user.id, "tier 1") {
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
       
    // Start the scheduler
    sched.start().await.expect("Failed to start scheduler");

    // TODO we should add another scheduled call that just checks if there are items that are 'done' or not found in the elevenlabs
    // but are still 'ongoing' in our db. we don't want to be accidentally charging users.
    // and if that happens make error visible

}




