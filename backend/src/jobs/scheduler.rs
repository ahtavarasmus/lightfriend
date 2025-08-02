use tokio_cron_scheduler::{JobScheduler, Job};
use std::sync::Arc;
use tracing::{debug, error};
use crate::AppState;

use crate::handlers::imap_handlers::ImapEmailPreview;

use std::env;

use crate::handlers::imap_handlers;
use crate::api::twilio_utils;
use reqwest::StatusCode;

async fn initialize_matrix_clients(state: Arc<AppState>) {
    tracing::debug!("Starting Matrix client initialization...");
    
    // Get all users with active WhatsApp connection
    match state.user_repository.get_users_with_matrix_bridge_connections() {
        Ok(users) => {
            let mut matrix_clients = state.matrix_clients.lock().await;
            let mut sync_tasks = state.matrix_sync_tasks.lock().await;
            
            // Remove any existing clients and sync tasks
            for (_, task) in sync_tasks.drain() {
                task.abort();
            }
            matrix_clients.clear();

            // Setup clients and sync tasks for active users
            for user_id in users {
                tracing::debug!("Setting up new Matrix client for user {}", user_id);
                
                // Create and initialize client
                match crate::utils::matrix_auth::get_client(user_id, &state).await {
                    Ok(client) => {
                        // Add event handlers before storing/cloning the client
                        use matrix_sdk::ruma::events::room::message::OriginalSyncRoomMessageEvent;
                        use matrix_sdk::room::Room;
                        
                        let state_for_handler = Arc::clone(&state);
                        client.add_event_handler(move |ev: OriginalSyncRoomMessageEvent, room: Room, client| {
                            let state = Arc::clone(&state_for_handler);
                            async move {
                                tracing::debug!("📨 Received message in room {}: {:?}", room.room_id(), ev);
                                crate::utils::bridge::handle_bridge_message(ev, room, client, state).await;
                            }
                        });

                        // Store the client
                        let client = Arc::new(client);
                        matrix_clients.insert(user_id, client.clone());

                        // Create sync task
                        let sync_settings = matrix_sdk::config::SyncSettings::default()
                            .timeout(std::time::Duration::from_secs(30))
                            .full_state(true);

                        let handle = tokio::spawn(async move {
                            loop {
                                match client.sync(sync_settings.clone()).await {
                                    Ok(_) => {
                                        tracing::debug!("Sync completed normally for user {}", user_id);
                                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                                    },
                                    Err(e) => {
                                        error!("Matrix sync error for user {}: {}", user_id, e);
                                        tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
                                    }
                                }
                            }
                        });

                        sync_tasks.insert(user_id, handle);
                    },
                    Err(e) => {
                        error!("Failed to create Matrix client for user {}: {}", user_id, e);
                    }
                }
            }
        },
        Err(e) => error!("Failed to get active WhatsApp users: {}", e),
    }
}

pub async fn start_scheduler(state: Arc<AppState>) {
    // Initialize matrix clients and sync tasks once on startup
    tracing::debug!("Initializing Matrix clients and sync tasks...");
    initialize_matrix_clients(Arc::clone(&state)).await;

    let sched = JobScheduler::new().await.expect("Failed to create scheduler");

    // Create a job that runs every 10 minutes to check for new IMAP messages
    let state_clone = Arc::clone(&state);
    let message_monitor_job = Job::new_async("0 */10 * * * *", move |_, _| {
        let state = state_clone.clone();
        Box::pin(async move {
            
            // Get all users with valid message monitor subscription
            let users_with_subscription = match state.user_core.get_all_users() {
                Ok(users) => {
                    let mut subscribed_users = Vec::new();
                    for user in users {
                        match state.user_repository.has_valid_subscription_tier(user.id, "tier 2") {
                            Ok(true) => {
                                if !state.user_core.get_proactive_agent_on(user.id).unwrap_or(true) {
                                    tracing::debug!("User {} does not have monitoring enabled", user.id);
                                    continue;
                                }
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
                    if imap_users.contains(&user.id) {
                        match imap_handlers::fetch_emails_imap(&state, user.id, true, Some(10), true, true).await {
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

                                    let priority_senders = match state.user_repository.get_priority_senders(user.id, "imap") {
                                        Ok(senders) => senders,
                                        Err(e) => {
                                            tracing::error!("Failed to get priority senders for user {}: {}", user.id, e);
                                            Vec::new()
                                        }
                                    };

                                    // Mark emails as processed and format them for importance checking
                                    let mut emails_content = String::from("New emails:\n");
                                    for email in &sorted_emails {

                                        // Check if this email was already processed
                                        if let Ok(already_processed) = state.user_repository.is_email_processed(user.id, &email.id) {
                                            if already_processed {
                                                debug!("Skipping already processed email {}", email.id);
                                                continue;
                                            }
                                        }
                                        
                                        if let Err(e) = state.user_repository.mark_email_as_processed(user.id, &email.id) {
                                            error!("Failed to mark email {} as processed: {}", email.id, e);
                                            continue;
                                        }

                                        // Check if sender matches priority senders and send the noti anyways about it
                                        if priority_senders.iter().any(|priority_sender| {
                                            email.from.as_deref().unwrap_or("Uknown").to_lowercase().contains(&priority_sender.sender.to_lowercase())
                                        }) {
                                            tracing::info!("Fast check: Priority sender matched for user {}", user.id);
                                            
                                            // Format the notification message with sender and content
                                            let message = format!(
                                                "Email from: {}\nSubject: {}\nContent: {}",
                                                email.from.as_deref().unwrap_or("Unknown"),
                                                email.subject.as_deref().unwrap_or("No subject"),
                                                email.body.as_deref().unwrap_or("No content").chars().take(200).collect::<String>()
                                            );
                                            let first_message = format!("Hello, you have a critical email from {} with subject: {}", 
                                                        email.from.as_deref().unwrap_or("Unknown"),
                                                        email.subject.as_deref().unwrap_or("No subject")
                                                    );
                                            
                                            // Spawn a new task for sending notification
                                            let state_clone = state.clone();
                                            tokio::spawn(async move {
                                                crate::proactive::utils::send_notification(
                                                    &state_clone,
                                                    user.id,
                                                    &message,
                                                    "email_priority".to_string(),
                                                    Some(first_message),
                                                ).await;
                                            });
                                            continue;
                                        }
                                        // Format email content for checking
                                        let email_content = format!(
                                            "From: {}\nSubject: {}\nDate: {}\nBody: {}\n---\n",
                                            email.from.as_deref().unwrap_or("Unknown"),
                                            email.subject.as_deref().unwrap_or("No subject"),
                                            email.date_formatted.as_deref().unwrap_or("Unknown date"),
                                            email.body.as_deref().unwrap_or("No content")
                                        );

                                        // Check waiting checks first if they exist
                                        let waiting_checks = match state.user_repository.get_waiting_checks(user.id, "email") {
                                            Ok(checks) => checks,
                                            Err(e) => {
                                                tracing::error!("Failed to get waiting checks for user {}: {}", user.id, e);
                                                Vec::new()
                                            }
                                        };

                                        if !waiting_checks.is_empty() {
                                            // Check if any waiting checks match the message
                                            if let Ok((check_id_option, message, first_message)) = crate::proactive::utils::check_waiting_check_match(
                                                &state,
                                                &email_content,
                                                &waiting_checks,
                                            ).await {
                                                if let Some(check_id) = check_id_option {
                                                    let message = message.unwrap_or("Waiting check matched in Email, but failed to get content".to_string());
                                                    let first_message = first_message.unwrap_or("Hey, I found a match for one of your waiting checks in Email.".to_string());
                                                    
                                                    // Delete the matched waiting check
                                                    if let Err(e) = state.user_repository.delete_waiting_check_by_id(user.id, check_id) {
                                                        tracing::error!("Failed to delete waiting check {}: {}", check_id, e);
                                                    }
                                                    
                                                    // Send notification
                                                    let state_clone = state.clone();
                                                    let user_id = user.id;
                                                    tokio::spawn(async move {
                                                        crate::proactive::utils::send_notification(
                                                            &state_clone,
                                                            user_id,
                                                            &message,
                                                            "email_waiting_check".to_string(),
                                                            Some(first_message),
                                                        ).await;
                                                    });
                                                    continue;
                                                }
                                            }
                                        }


                                        // Add email to content string for importance checking
                                        emails_content.push_str(&email_content);
                                    }


                                    // Check message importance based on waiting checks and criticality
                                    let user_settings = match state.user_core.get_user_settings(user.id) {
                                        Ok(settings) => settings,
                                        Err(e) => {
                                            tracing::error!("Failed to get user settings: {}", e);
                                            return;
                                        }
                                    };

                                    if user_settings.critical_enabled.is_none() {
                                        tracing::debug!("Critical message checking disabled for user {}", user.id);
                                        return;
                                    }

                                    // Check message importance based on criticality
                                    match crate::proactive::utils::check_message_importance(&state, &emails_content).await {
                                        Ok((is_critical, message, first_message)) => {
                                            if is_critical {
                                                let message = message.unwrap_or("Critical email found, check email to see it (failed to fetch actual content, pls report)".to_string());
                                                let first_message = first_message.unwrap_or("Hey, I found some critical email you should know.".to_string());
                                                tracing::info!(
                                                    "Email critical check passed for user {}: {}",
                                                    user.id, message
                                                );
                                                                
                                                // Spawn a new task for sending critical message notification
                                                let state_clone = state.clone();
                                                let message_clone= message.clone();
                                                tokio::spawn(async move {
                                                    crate::proactive::utils::send_notification(
                                                        &state_clone,
                                                        user.id,
                                                        &message_clone,
                                                        "email_critical".to_string(),
                                                        Some(first_message),
                                                    ).await;
                                                });
                                            } else {
                                                tracing::debug!(
                                                    "Email not considered important for user {}: {}",
                                                    user.id, message.unwrap_or("failed to get the email content".to_string())
                                                );

                                            }
                                        }
                                        Err(e) => {
                                            tracing::error!("Failed to check email importance: {}", e);
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
            }

        })
    }).expect("Failed to create message monitor job");

    sched.add(message_monitor_job).await.expect("Failed to add message monitor job to scheduler");

    /*

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
                                match state.user_core.has_auto_topup_enabled(log.user_id) {
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

    // Create a job that runs every hour to check morning digests
    let state_clone = Arc::clone(&state);
    let digest_check_job = Job::new_async("0 0 * * * *", move |_, _| {
        let state = state_clone.clone();
        Box::pin(async move {
            debug!("Running hourly morning digest check...");
            
            // Get all users with tier 2 subscription
            match state.user_core.get_all_users() {
                Ok(users) => {
                    for user in users {
                        // Check if user has a tier 2 subscription
                        if let Ok(Some(tier)) = state.user_repository.get_subscription_tier(user.id) {

                            if !state.user_core.get_proactive_agent_on(user.id).unwrap_or(true) {
                                tracing::debug!("User {} does not have monitoring enabled", user.id);
                                continue;
                            }
                            if tier == "tier 2" {
                                debug!("Checking morning digest for user {} with tier 2 subscription", user.id);
                                if let Err(e) = crate::proactive::utils::check_morning_digest(&state, user.id).await {
                                    error!("Failed to check morning digest for user {}: {}", user.id, e);
                                }
                                if let Err(e) = crate::proactive::utils::check_day_digest(&state, user.id).await {
                                    error!("Failed to check day digest for user {}: {}", user.id, e);
                                }
                                if let Err(e) = crate::proactive::utils::check_evening_digest(&state, user.id).await {
                                    error!("Failed to check evening digest for user {}: {}", user.id, e);
                                }
                            }
                        }
                    }
                }
                Err(e) => error!("Failed to fetch users for morning digest check: {}", e),
            }
        })
    }).expect("Failed to create digest check job");

    sched.add(digest_check_job).await.expect("Failed to add digest check job to scheduler");

    // Create a job that runs every 5 minutes to check for upcoming calendar events
    let state_clone = Arc::clone(&state);
    let calendar_notification_job = Job::new_async("0 */5 * * * *", move |_, _| {  // Run every 5 minutes
        let state = state_clone.clone();
        Box::pin(async move {
            // Use a mutex to ensure only one instance runs at a time
            let calendar_mutex = tokio::sync::Mutex::new(());
            let _lock = calendar_mutex.try_lock();
            if _lock.is_err() {
                debug!("Calendar check already in progress, skipping this run");
                return;
            }

            // Clean up old notifications (older than 24 hours) with retry logic
            let cleanup_threshold = (chrono::Utc::now() - chrono::Duration::hours(24)).timestamp() as i32;
            for attempt in 1..=3 {
                match state.user_repository.cleanup_old_calendar_notifications(cleanup_threshold) {
                    Ok(_) => break,
                    Err(e) => {
                        error!("Attempt {} to clean up old calendar notifications failed: {}", attempt, e);
                        if attempt < 3 {
                            tokio::time::sleep(tokio::time::Duration::from_millis(100 * attempt as u64)).await;
                        }
                    }
                }
            }

            // Get all users with valid Google Calendar connection and subscription
            let users = match state.user_core.get_all_users() {
                Ok(users) => users.into_iter().filter(|user| {
                    // Check subscription and calendar status
                    matches!(state.user_repository.has_valid_subscription_tier(user.id, "tier 2"), Ok(true)) &&
                    matches!(state.user_repository.has_active_google_calendar(user.id), Ok(true)) &&
                    matches!(state.user_core.get_proactive_agent_on(user.id), Ok(true))
                }).collect::<Vec<_>>(),
                Err(e) => {
                    error!("Failed to fetch users: {}", e);
                    return;
                }
            };

            let now = chrono::Utc::now();
            let window_end = now + chrono::Duration::minutes(30);

            debug!("🗓️ Calendar check: Starting check for {} users at {}", 
                users.len(),
                now.format("%Y-%m-%d %H:%M:%S UTC")
            );

            // Process users with rate limiting
            for (index, user) in users.iter().enumerate() {
                // Add delay between users to avoid rate limiting
                if index > 0 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                }

                debug!("🗓️ Calendar check: Processing user {} ({}/{})", 
                    user.id, 
                    index + 1, 
                    users.len()
                );

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

                                    let event_summary = event.summary.clone().unwrap_or_else(|| "Untitled Event".to_string());
                                    let notification = format!("Calendar: {} in {} mins", event_summary, reminder.minutes);

                                    let state_clone = state.clone();
                                    let first_message = format!("Hello, you have a calendar event starting in {}.", reminder.minutes);
                                    let user_id = user.id.clone();
                                    tokio::spawn(async move {
                                        crate::proactive::utils::send_notification(
                                            &state_clone,
                                            user_id,
                                            &notification,
                                            "calendar_notification".to_string(),
                                            Some(first_message),
                                        ).await;
                                    });
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

    // Create a job that runs daily to check self-hosted instance status
    let state_clone = Arc::clone(&state);
    let self_host_check_job = Job::new_async("0 0 0 * * *", move |_, _| {  // Run at midnight every day
        let state = state_clone.clone();
        Box::pin(async move {
            debug!("Running self-hosted instance status check...");
            
            // Get user with self-hosted subscription
            match state.user_core.get_all_users() {
                Ok(users) => {
                    for user in users {
                        if let Ok(Some(tier)) = state.user_repository.get_subscription_tier(user.id) {
                            if tier == "self_hosted" {
                                // Get server instance ID
                                if let Ok(settings) = state.user_core.get_user_settings(user.id) {
                                    if let Some(instance_id) = settings.server_instance_id {
                                        // Get current timestamp
                                        let current_timestamp = chrono::Utc::now().timestamp() as i32;
                                        
                                        // Check if we've already pinged today
                                        if let Some(last_ping) = settings.server_instance_last_ping_timestamp {
                                            let last_ping_date = chrono::NaiveDateTime::from_timestamp_opt(last_ping as i64, 0)
                                                .map(|dt| dt.date());
                                            let current_date = chrono::NaiveDateTime::from_timestamp_opt(current_timestamp as i64, 0)
                                                .map(|dt| dt.date());
                                            
                                            if let (Some(last), Some(current)) = (last_ping_date, current_date) {
                                                if last == current {
                                                    debug!("Already pinged self-hosted instance today, skipping");
                                                    continue;
                                                }
                                            }
                                        }

                                        // Send ping request
                                        let client = reqwest::Client::new();
                                        let server_url = env::var("SERVER_URL").unwrap_or_else(|_| "https://lightfriend.ai".to_string());
                                        let ping_url = format!("{}/api/self-host-ping", server_url);
                                        
                                        match client.post(&ping_url)
                                            .json(&serde_json::json!({
                                                "instance_id": instance_id
                                            }))
                                            .send()
                                            .await 
                                        {
                                            Ok(response) => {
                                                if response.status() != StatusCode::OK {
                                                    error!("Self-hosted instance ping failed with status: {}", response.status());
                                                    // Update user's subscription tier to None
                                                    if let Err(e) = state.user_core.update_subscription_tier(user.id, Some("self_hosted_inactive")) {
                                                        error!("Failed to update subscription tier: {}", e);
                                                    }
                                                } else {
                                                    debug!("Self-hosted instance ping successful");
                                                    // Update last ping timestamp
                                                    if let Err(e) = state.user_core.update_server_instance_last_ping(user.id, current_timestamp) {
                                                        error!("Failed to update last ping timestamp: {}", e);
                                                    }
                                                }
                                            },
                                            Err(e) => {
                                                error!("Failed to send self-hosted instance ping: {}", e);
                                                // Update user's subscription tier to self_hosted_inactive
                                                if let Err(e) = state.user_repository.set_subscription_tier(user.id, Some("self_hosted_inactive")) {
                                                    error!("Failed to update subscription tier: {}", e);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                },
                Err(e) => error!("Failed to fetch users for self-hosted instance check: {}", e),
            }
        })
    }).expect("Failed to create self-host check job");

    sched.add(self_host_check_job).await.expect("Failed to add self-host check job to scheduler");

    // Start the scheduler
    sched.start().await.expect("Failed to start scheduler");

    // TODO we should add another scheduled call that just checks if there are items that are 'done' or not found in the elevenlabs
    // but are still 'ongoing' in our db. we don't want to be accidentally charging users.
    // and if that happens make error visible

}


