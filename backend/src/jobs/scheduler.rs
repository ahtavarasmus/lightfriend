use tokio_cron_scheduler::{JobScheduler, Job};
use axum::{
    http::StatusCode,
};
use std::sync::Arc;
use tracing::{info, error};
use crate::AppState;
use crate::handlers::gmail;

use std::env;

pub async fn start_scheduler(state: Arc<AppState>) {
    let sched = JobScheduler::new().await.expect("Failed to create scheduler");

    // Create a job that runs every minute to fetch Gmail previews
    let state_clone = Arc::clone(&state);
    let gmail_monitor_job = Job::new_async("0 * * * * *", move |_, _| {
        let state = state_clone.clone();
        Box::pin(async move {
            info!("Running scheduled Gmail preview fetch job...");
            
            // Get users with active Gmail connections
            match state.user_repository.get_active_gmail_connection_users() {
                Ok(user_ids) => {
                    for user_id in user_ids {
                        info!("Fetching Gmail previews for user {}", user_id);
                        match crate::gmail::fetch_gmail_previews(&state, user_id, Some(10)).await {
                            Ok(messages) => {
                                info!("Successfully fetched {} Gmail messages for user {}", messages.len(), user_id);
                                
                                // Filter unread messages
                                let unread_messages: Vec<_> = messages.into_iter()
                                    .filter(|msg| !msg.is_read)
                                    .collect();
                                
                                if !unread_messages.is_empty() {
                                    info!("Found {} unread messages", unread_messages.len());
                                    let api_key = match env::var("OPENROUTER_API_KEY") {
                                    Ok(key) => key,
                                    Err(_) => {
                                        eprintln!("OPENROUTER_API_KEY not set");
                                        return (
                                            StatusCode::INTERNAL_SERVER_ERROR,
                                            axum::Json(TwilioResponse {
                                                message: "Server configuration error".to_string(),
                                            })
                                        );
                                    }
                                };

                                let client = match OpenAIClient::builder()
                                    .with_endpoint("https://openrouter.ai/api/v1")
                                    .with_api_key(api_key)
                                    .build() {
                                        Ok(client) => client,
                                        Err(e) => {
                                            eprintln!("Failed to build OpenAI client: {}", e);
                                            return (
                                                StatusCode::INTERNAL_SERVER_ERROR,
                                                axum::Json(TwilioResponse {
                                                    message: "Failed to initialize AI service".to_string(),
                                                })
                                            );
                                        }
                                    };

                                    // Filter important emails
                                    match gmail::filter_important_emails(unread_messages, &client).await {
                                        Ok(important_messages) => {
                                            for (message, reason) in important_messages {
                                                info!(
                                                    "Important unread email found - From: {:?}, Subject: {:?}, Reason: {}",
                                                    message.from, message.subject, reason
                                                );
                                            }
                                        }
                                        Err(e) => {
                                            error!("Failed to filter important emails: {:?}", e);
                                        }
                                    }
                                }
                            },
                            Err(e) => {
                                error!(
                                    "Failed to fetch Gmail previews for user {}: Error: {:?}",
                                    user_id, e
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to fetch active Gmail connections: {}", e);
                }
            }
        })
    }).expect("Failed to create Gmail monitor job");

    sched.add(gmail_monitor_job).await.expect("Failed to add Gmail monitor job to scheduler");
        

    // Create a job that runs every 5 seconds to handle ongoing usage logs
    let state_clone = Arc::clone(&state);
    let usage_monitor_job = Job::new_async("*/5 * * * * *", move |_, _| {
        let state = state_clone.clone();
        Box::pin(async move {
            info!("Running scheduled usage monitor job...");
            let api_key = env::var("ELEVENLABS_API_KEY").expect("ELEVENLABS_API_KEY must be set");
            let client = reqwest::Client::new();

            match state.user_repository.get_all_ongoing_usage() {
                Ok(ongoing_logs) => {
                    info!("Found {} ongoing usage logs to process", ongoing_logs.len());
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




