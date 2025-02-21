use tokio_cron_scheduler::{JobScheduler, Job};
use std::sync::Arc;
use diesel::prelude::*;
use tracing::{info, error};
use crate::AppState;
use crate::models::user_models::{Call, ElevenLabsResponse};
use crate::schema::users;

use crate::schema::calls;
use std::env;

pub async fn start_scheduler(state: Arc<AppState>) {
    let sched = JobScheduler::new().await.expect("Failed to create scheduler");
    
    // Create a job that runs every minute
    let state_clone = Arc::clone(&state);
    let job = Job::new_async("1/1 * * * * *", move |_, _| {
        let state = state_clone.clone();
        Box::pin(async move {
            match check_and_update_database(&state).await {
                Ok(_) => info!("Successfully ran scheduled task"),
                Err(e) => error!("Error in scheduled task: {}", e),
            }
        })
    }).expect("Failed to create job");

    sched.add(job).await.expect("Failed to add job to scheduler");
    
    // Start the scheduler
    sched.start().await.expect("Failed to start scheduler");
}

async fn check_and_update_database(state: &AppState) -> Result<(), Box<dyn std::error::Error>> {

    let conn = &mut state.db_pool.get()?;
    
    // Fetch all calls with 'processing' status
    let processing_calls = calls::table
        .filter(calls::status.eq("processing"))
        .load::<Call>(conn)?;

    let api_key = env::var("ELEVENLABS_API_KEY")
        .expect("ELEVENLABS_API_KEY must be set");
    
    let client = reqwest::Client::new();

    info!("Found {} calls in processing status", processing_calls.len());

    for call in processing_calls {
        info!("Processing call id: {}, conversation_id: {}", call.id, call.conversation_id);
        
        // Make API request for each processing call
        let url = format!(
            "https://api.elevenlabs.io/v1/convai/conversations/{}", 
            call.conversation_id
        );

        info!("Fetching status from ElevenLabs API for conversation: {}", call.conversation_id);
        match client
            .get(&url)
            .header("xi-api-key", &api_key)
            .send()
            .await
        {
            Ok(response) => {
                info!("Received response from ElevenLabs API for call: {}", call.id);
                let status = response.status();
                let response_text = response.text().await?;
                
                if !status.is_success() {
                    error!(
                        "Error response from ElevenLabs API for call {}: Status: {}, Body: {}", 
                        call.id, 
                        status,
                        response_text
                    );
                    
                    // Delete the failed call from the database
                    match diesel::delete(calls::table.find(call.id)).execute(conn) {
                        Ok(_) => {
                            info!("Successfully deleted failed call {} from database", call.id);
                        }
                        Err(e) => {
                            error!("Failed to delete call {} from database: {}", call.id, e);
                        }
                    }
                    continue;
                }
                
                info!("Received successful response: {}", response_text);
                match serde_json::from_str::<ElevenLabsResponse>(&response_text) {
                    Ok(call_data) => {
                        info!("Parsed response for call {}: status={}, duration={}s", 
                            call.id, call_data.status, call_data.call_duration_secs);
                        // Check if status changed from processing to done
                        if call.status == "processing" && call_data.status == "done" {
                            // Update user's IQ
                            if let Err(e) = state.user_repository.decrease_iq(
                                call.user_id,
                                call_data.call_duration_secs
                            ) {
                                error!("Failed to update IQ for user {}: {}", call.user_id, e);
                            } else {
                                info!("Successfully decreased IQ for user {} by {} seconds", 
                                    call.user_id, call_data.call_duration_secs);
                            }
                        }

                        // Update the call using UserCalls repository
                        if let Err(e) = state.user_calls.update_call(
                            call.id,
                            call_data.status.clone(),
                            call_data.call_duration_secs,
                        ) {
                            error!("Failed to update call {}: {}", call.id, e);
                        } else {
                            info!("Updated call {} status from {} to {}", 
                                call.id, call.status, call_data.status);
                        }
                    }
                    Err(e) => {
                        error!("Failed to parse response for call {}: {}", call.id, e);
                    }
                }

            }
            Err(e) => {
                error!("Failed to fetch status for call {}: {}", call.id, e);
            }
        }
    }

    Ok(())
}

