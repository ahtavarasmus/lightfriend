use tokio_cron_scheduler::{JobScheduler, Job};
use std::sync::Arc;
use diesel::prelude::*;
use tracing::{info, error};
use crate::AppState;
use crate::api::elevenlabs::ElevenLabsResponse;
use crate::schema::users;
use crate::schema::subscriptions;
use std::error::Error;

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
    
    // Create a job that runs every 30 seconds to sync paddle subscriptions
    let state_clone = Arc::clone(&state);
    let sync_job = Job::new_async("*/30 * * * * *", move |_, _| {
        let state = state_clone.clone();
        Box::pin(async move {
            match sync_all_active_subscriptions(&state).await {
                Ok(_) => info!("Successfully synced all active subscriptions with Paddle"),
                Err(e) => error!("Error in subscription sync task: {}", e),
            }
        })
    }).expect("Failed to create subscription sync job");

    sched.add(sync_job).await.expect("Failed to add subscription sync job to scheduler");
    
    // Start the scheduler
    sched.start().await.expect("Failed to start scheduler");
}

// Function to sync all active subscriptions with Paddle
async fn sync_all_active_subscriptions(state: &AppState) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting to sync all active subscriptions with Paddle");
    
    let conn = &mut state.db_pool.get()?;
    
    // First query to get all active subscriptions with their user_id
    let active_subscription_users = subscriptions::table
        .filter(subscriptions::status.eq("active"))
        .filter(
            subscriptions::is_scheduled_to_cancel
                .eq(false) // Explicitly false
                .or(subscriptions::is_scheduled_to_cancel.is_null()) // Or NULL
        )
        .select((subscriptions::paddle_subscription_id, subscriptions::user_id))
        .load::<(String, i32)>(conn)?;
    
    // Create a vector to store subscription_id and iq pairs
    let mut active_subscriptions = Vec::new();
    
    // Second query to get the iq for each user
    for (subscription_id, user_id) in active_subscription_users {
        let user_iq = users::table
            .filter(users::id.eq(user_id))
            .select(users::iq)
            .first::<i32>(conn)?;
        
        active_subscriptions.push((subscription_id, user_iq));
    }
    
    info!("Found {} active subscriptions to sync", active_subscriptions.len());
    
    for (subscription_id, iq_quantity) in active_subscriptions {
        // TODO if user has set charge_when_under field and the iq is under that, charge it back to charge_back_to
        /*
        if iq_quantity < 0 {
            let iq_q = iq_quantity.abs() / 3;
            println!("iq_q: {}",iq_q);
            
            match crate::api::paddle_utils::sync_paddle_subscription_items(&subscription_id, iq_q).await {
                Ok(_) => info!("Successfully synced subscription {} with IQ quantity {}", subscription_id, iq_quantity),
                Err(e) => error!("Failed to sync subscription {}: {}", subscription_id, e),
            }
        }
        */
    }
    
    info!("Completed syncing all active subscriptions");
    Ok(())
}

async fn check_and_update_database(state: &AppState) -> Result<(), Box<dyn std::error::Error>> {

    let conn = &mut state.db_pool.get()?;

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
                                                info!("Call status == done => DECREASE IQ + DELETE");
                                                // Decrease user's IQ based on call duration
                                                let iq_used = conversation_details.metadata.call_duration_secs;
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
                                                    iq_used,
                                                    success, 
                                                    summary,
                                                ) {
                                                    error!("Failed to log usage: {}", e);
                                                }

                                                // Then decrease the IQ
                                                if let Err(e) = state.user_repository.decrease_iq(user_id, iq_used) {
                                                    error!("Failed to decrease user IQ: {}", e);
                                                } else {
                                                    info!("Successfully decreased IQ for user {} by {} seconds", user_id, iq_used);
                                                                                                       
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

