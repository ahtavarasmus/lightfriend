use tokio_cron_scheduler::{JobScheduler, Job};
use std::sync::Arc;
use tracing::{info, error};
use crate::AppState;
use crate::api::elevenlabs::ElevenLabsResponse;

use std::env;

pub async fn start_scheduler(state: Arc<AppState>) {
    let sched = JobScheduler::new().await.expect("Failed to create scheduler");
    
    // Create a job that runs every 30 seconds to check and update database
    let state_clone = Arc::clone(&state);
    let job = Job::new_async("*/30 * * * * *", move |_, _| {
        let state = state_clone.clone();
        Box::pin(async move {
            match check_and_update_database(&state).await {
                Ok(_) => info!("Successfully ran scheduled task for database updates"),
                Err(e) => error!("Error in scheduled database update task: {}", e),
            }
        })
    }).expect("Failed to create job");

    sched.add(job).await.expect("Failed to add job to scheduler");
    
    // Start the scheduler
    sched.start().await.expect("Failed to start scheduler");
}


async fn check_and_update_database(state: &Arc<AppState>) -> Result<(), Box<dyn std::error::Error>> {


    let api_key = env::var("ELEVENLABS_API_KEY")
        .expect("ELEVENLABS_API_KEY must be set");
    
    let client = reqwest::Client::new();

    // First, fetch all conversations from ElevenLabs
    let conversations_url = "https://api.elevenlabs.io/v1/convai/conversations";
    
    info!("Fetching all conversations from ElevenLabs API");
    let conversations_response = client
        .get(conversations_url)
        .header("xi-api-key", &api_key)
        .send()
        .await?;

    let conversations_text = conversations_response.text().await?;
    info!("Received conversations response: {}", conversations_text);

    // Parse the response with the correct structure
    let response: serde_json::Value = serde_json::from_str(&conversations_text)?;
    
    // Get the conversations array
    let conversations = response["conversations"]
        .as_array()
        .unwrap_or(&Vec::new())
        .to_owned();

    // Now process each conversation
    for conversation in conversations {
        println!("Starting to go through conversations");
        let conversation_id = conversation["conversation_id"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        // Make API request for the conversation
        let url = format!(
            "https://api.elevenlabs.io/v1/convai/conversations/{}", 
            conversation_id
        );

        info!("Fetching status from ElevenLabs API for conversation: {}", conversation_id);
        match client
            .get(&url)
            .header("xi-api-key", &api_key)
            .send()
            .await
        {
            Ok(response) => {
                match response.text().await {
                    Ok(text) => {
                        info!("Received conversation details: {}", text);
                        match serde_json::from_str::<ElevenLabsResponse>(&text) {
                            Ok(conversation_details) => {

                                // Handle conversations based on user_id presence and status
                                match conversation_details.conversation_initiation_client_data.dynamic_variables.user_id {
                                    Some(user_id_str) => {
                                        if let Ok(user_id) = user_id_str.parse::<i32>() {
                                            // Only process if the call status is 'done'
                                            if conversation_details.status == "done" {
                                                info!("Call status == done => DECREASE CREDITS + DELETE");
                                                // Decrease user's credits based on call duration
                                                
                                                let voice_second_cost = std::env::var("VOICE_SECOND_COST")
                                                    .expect("VOICE_SECOND_COST not set")
                                                    .parse::<f32>()
                                                    .unwrap_or(0.0033);
                                                let credits_used = conversation_details.metadata.call_duration_secs as f32 * voice_second_cost;
                                                let success = match conversation_details.analysis {
                                                    Some(ref analysis) => match analysis.call_successful.as_str() {
                                                        "success" => true,
                                                        "failure" => false,
                                                        _ => false,
                                                    },
                                                    None => false,
                                                };
                                                let summary = conversation_details.analysis
                                                    .as_ref()
                                                    .map(|analysis| analysis.transcript_summary.to_string())
                                                    .or(Some(String::from("")));
                                                
                                                // First log the usage
                                                if let Err(e) = state.user_repository.log_usage(
                                                    user_id,
                                                    "call",
                                                    credits_used,
                                                    success, 
                                                    summary,
                                                ) {
                                                    error!("Failed to log usage: {}", e);
                                                }

                                                // Then decrease the credits 
                                                if let Err(e) = state.user_repository.decrease_credits(user_id, credits_used) {
                                                    error!("Failed to decrease user credits: {}", e);
                                                } else { 
                                                    info!("Successfully decreased credits for user {} by {}", user_id, credits_used);

                                                    match state.user_repository.is_credits_under_threshold(user_id) {
                                                        Ok(is_under) => {
                                                            if is_under {
                                                                info!("User {} credits is under threshold, attempting automatic charge", user_id);
                                                                // Get user information
                                                                match state.user_repository.find_by_id(user_id) {
                                                                    Ok(Some(user)) => {
                                                                        if user.charge_when_under {
                                                                            use axum::extract::{State, Path};
                                                                            let state_clone = Arc::clone(&state);
                                                                            tokio::spawn(async move {
                                                                                let _ = crate::handlers::stripe_handlers::automatic_charge(
                                                                                    State(state_clone),
                                                                                    Path(user.id),
                                                                                ).await;
                                                                                info!("Recharged the user successfully back up!");
                                                                            });                                                                            
                                                                            info!("recharged the user successfully back up!");
                                                                        }
                                                                    },
                                                                    Ok(None) => error!("User {} not found for automatic charge", user_id),
                                                                    Err(e) => error!("Failed to get user for automatic charge: {}", e),
                                                                }
                                                            }
                                                        },
                                                        Err(e) => error!("Failed to check if user credits is under threshold: {}", e),
                                                    }
                                                                                                       
                                                    // Delete the conversation from ElevenLabs
                                                    let delete_url = format!(
                                                        "https://api.elevenlabs.io/v1/convai/conversations/{}", 
                                                        conversation_id
                                                    );
                                                    
                                                    match client
                                                        .delete(&delete_url)
                                                        .header("xi-api-key", &api_key)
                                                        .send()
                                                        .await
                                                    {
                                                        Ok(_) => info!("Successfully deleted conversation {} from ElevenLabs", conversation_id),
                                                        Err(e) => error!("Failed to delete conversation from ElevenLabs: {}", e),
                                                    }
                                                }
                                            }
                                        } else {
                                            info!("Invalid user_id format found, deleting conversation {}", conversation_id);
                                            // Delete invalid conversation
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
                                                error!("Failed to delete invalid conversation: {}", e);
                                            }
                                        }
                                    }
                                    None => {
                                        info!("No user_id found, deleting conversation {}", conversation_id);
                                        // Delete conversation without user_id
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
                                            error!("Failed to delete conversation without user_id: {}", e);
                                        }

                                    }

                                }
                            }
                            Err(e) => error!("Failed to parse conversation details: {}", e),
                        }
                    }
                    Err(e) => error!("Failed to get response text: {}", e),
                }
            }
            Err(e) => {
                error!("Failed to fetch conversation details: {}", e);
            }
        }
    }

    Ok(())
}

