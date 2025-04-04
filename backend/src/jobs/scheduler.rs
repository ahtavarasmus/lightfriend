use tokio_cron_scheduler::{JobScheduler, Job};
use axum::{
    http::StatusCode,
};
use std::sync::Arc;
use tracing::{info, error};
use crate::AppState;
use crate::handlers::gmail;

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
            info!("Running scheduled message check across services...");
            
            // Get all users with valid message monitor subscription
            let users_with_subscription = match state.user_repository.get_all_users() {
                Ok(users) => {
                    let mut subscribed_users = Vec::new();
                    for user in users {
                        match state.user_repository.has_valid_subscription_tier_with_messages(user.id, "message_monitor") {
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
                info!("Processing services for subscribed user {}", user.id);

                // Check IMAP service
                if let Ok(imap_users) = state.user_repository.get_active_imap_connection_users() {
                    if imap_users.contains(&user.id) {
                        info!("Checking IMAP messages for user {}", user.id);
                        match imap_handlers::fetch_emails_imap(&state, user.id, true, Some(10), true).await {
                            Ok(emails) => {
                                info!("Successfully fetched {} new IMAP emails for user {}", emails.len(), user.id);
                                
                                if !emails.is_empty() {
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

                                    // Create notification message
                                    let notification = format!(
                                        "You have {} new email(s):\n{}",
                                        emails.len(),
                                        emails.iter()
                                            .take(3) // Limit to first 3 emails
                                            .map(|email| format!(
                                                "From: {}\nSubject: {}\n",
                                                email.from.as_deref().unwrap_or("Unknown"),
                                                email.subject.as_deref().unwrap_or("No subject")
                                            ))
                                            .collect::<Vec<_>>()
                                            .join("\n")
                                    );

                                    // Send SMS notification
                                    match twilio_utils::send_conversation_message(
                                        &conversation.conversation_sid,
                                        &conversation.twilio_number,
                                        &notification
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
                match state.user_repository.has_valid_subscription_tier_with_messages(user.id, "message_monitor") {
                    Ok(false) => {
                        info!("Skipping remaining services for user {} due to no messages left", user.id);
                        continue;
                    },
                    Err(e) => {
                        error!("Failed to check messages left for user {}: {}", user.id, e);
                        continue;
                    },
                    Ok(true) => {
                        info!("Continuing to process other services for user {}", user.id);
                    }
                }

                // Add more services here following the same pattern
            }

        })
    }).expect("Failed to create message monitor job");

    sched.add(message_monitor_job).await.expect("Failed to add message monitor job to scheduler");

    // Create a job that runs every 5 seconds to handle ongoing usage logs
    let state_clone = Arc::clone(&state);
    let usage_monitor_job = Job::new_async("*/5 * * * * *", move |_, _| {
        let state = state_clone.clone();
        Box::pin(async move {
            let api_key = env::var("ELEVENLABS_API_KEY").expect("ELEVENLABS_API_KEY must be set");
            let client = reqwest::Client::new();

            match state.user_repository.get_all_ongoing_usage() {
                Ok(ongoing_logs) => {
                    for log in ongoing_logs {
                        info!("Processing usage log for user {} with conversation_id {:?}", log.user_id, log.conversation_id);
                        let conversation_id = match log.conversation_id {
                            Some(id) => id,
                            None => continue,
                        };

                        // Check conversation status from ElevenLabs
                        let status_url = format!(
                            "https://api.elevenlabs.io/v1/convai/conversations/{}",
                            conversation_id
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
                            info!("Checking recharge threshold for user {}", log.user_id);
                            let current_timestamp = chrono::Utc::now().timestamp() as i32;
                            info!("current: {}", current_timestamp);
                            info!("threshold: {}", threshold_timestamp);
                            info!("current_timestamp >= threshold_timestamp: {}", current_timestamp >= threshold_timestamp);
                            if current_timestamp >= threshold_timestamp {
                                info!("current over threshold");
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
                                        info!("User {} does not have auto top-up enabled", log.user_id);
                                    }
                                    Err(e) => error!("Failed to check auto top-up status: {}", e),
                                }
                            }
                        }

                        // Handle zero credits timestamp
                        if let Some(zero_timestamp) = log.zero_credits_timestamp {
                            info!("Checking zero credits timestamp for user {}", log.user_id);
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
                                    &conversation_id,
                                    "done",
                                    credits_used,
                                    true,
                                    &format!("Call ended due to zero credits. Duration: {}s", call_duration)
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
                                        conversation_id
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
    
    // Start the scheduler
    sched.start().await.expect("Failed to start scheduler");

    // TODO we should add another scheduled call that just checks if there are items that are 'done' or not found in the elevenlabs
    // but are still 'ongoing' in our db. we don't want to be accidentally charging users.
    // and if that happens make error visible

}




