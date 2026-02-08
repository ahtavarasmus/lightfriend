use crate::AppState;
use crate::UserCoreOps;
use chrono::TimeZone;
use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{debug, error};

use crate::handlers::imap_handlers;

async fn initialize_matrix_clients(state: Arc<AppState>) {
    tracing::debug!("Starting Matrix client initialization...");

    // Get all users with active WhatsApp connection
    match state
        .user_repository
        .get_users_with_matrix_bridge_connections()
    {
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
                        use matrix_sdk::room::Room;
                        use matrix_sdk::ruma::events::room::message::OriginalSyncRoomMessageEvent;

                        let state_for_handler = Arc::clone(&state);
                        client.add_event_handler(
                            move |ev: OriginalSyncRoomMessageEvent, room: Room, client| {
                                let state = Arc::clone(&state_for_handler);
                                async move {
                                    tracing::debug!(
                                        "📨 Received message in room {}: {:?}",
                                        room.room_id(),
                                        ev
                                    );
                                    crate::utils::bridge::handle_bridge_message(
                                        ev, room, client, state,
                                    )
                                    .await;
                                }
                            },
                        );

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
                                        tracing::debug!(
                                            "Sync completed normally for user {}",
                                            user_id
                                        );
                                        tokio::time::sleep(tokio::time::Duration::from_secs(1))
                                            .await;
                                    }
                                    Err(e) => {
                                        error!("Matrix sync error for user {}: {}", user_id, e);
                                        tokio::time::sleep(tokio::time::Duration::from_secs(30))
                                            .await;
                                    }
                                }
                            }
                        });

                        sync_tasks.insert(user_id, handle);
                    }
                    Err(e) => {
                        error!("Failed to create Matrix client for user {}: {}", user_id, e);
                    }
                }
            }
        }
        Err(e) => error!("Failed to get active WhatsApp users: {}", e),
    }
}

/// Checks all bridges for all users and deletes any that are unhealthy
pub async fn check_all_bridges_health(
    state: &Arc<AppState>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    debug!("Starting bridge health check for all users...");

    let users_with_bridges = state.user_repository.get_users_with_active_bridges()?;
    debug!(
        "Found {} users with active bridges",
        users_with_bridges.len()
    );

    for (user_id, bridges) in users_with_bridges {
        for bridge in bridges {
            if !is_bridge_healthy(state, user_id, &bridge).await {
                tracing::info!(
                    "Bridge {} for user {} is unhealthy, deleting",
                    bridge.bridge_type,
                    user_id
                );

                // Record disconnection event for digest notification
                let current_time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i32;
                if let Err(e) = state.user_repository.record_bridge_disconnection(
                    user_id,
                    &bridge.bridge_type,
                    current_time,
                ) {
                    error!(
                        "Failed to record disconnection event for user {}: {}",
                        user_id, e
                    );
                }

                if let Err(e) = state
                    .user_repository
                    .delete_bridge(user_id, &bridge.bridge_type)
                {
                    error!(
                        "Failed to delete unhealthy bridge {} for user {}: {}",
                        bridge.bridge_type, user_id, e
                    );
                }
            }
        }

        // Clean up Matrix client if no bridges left
        match state.user_repository.has_active_bridges(user_id) {
            Ok(false) => {
                cleanup_matrix_client(state, user_id).await;
            }
            Err(e) => {
                error!("Failed to check active bridges for user {}: {}", user_id, e);
            }
            _ => {}
        }
    }

    debug!("Bridge health check completed");
    Ok(())
}

async fn is_bridge_healthy(
    state: &Arc<AppState>,
    user_id: i32,
    bridge: &crate::models::user_models::Bridge,
) -> bool {
    // Try to get Matrix client and fetch rooms for this bridge type
    // Note: Empty rooms is OK (user might not have any chats yet)
    // We only consider it unhealthy if we get an actual error
    match crate::utils::matrix_auth::get_cached_client(user_id, state).await {
        Ok(client) => {
            match crate::utils::bridge::get_service_rooms(&client, &bridge.bridge_type).await {
                Ok(_) => true, // Successfully fetched rooms (even if empty) = healthy
                Err(e) => {
                    tracing::warn!(
                        "Failed to fetch {} rooms for user {}: {}",
                        bridge.bridge_type,
                        user_id,
                        e
                    );
                    false
                }
            }
        }
        Err(e) => {
            tracing::warn!("Failed to get Matrix client for user {}: {}", user_id, e);
            false
        }
    }
}

async fn cleanup_matrix_client(state: &Arc<AppState>, user_id: i32) {
    let mut matrix_clients = state.matrix_clients.lock().await;
    let mut sync_tasks = state.matrix_sync_tasks.lock().await;
    if let Some(task) = sync_tasks.remove(&user_id) {
        task.abort();
        debug!("Aborted sync task for user {} during cleanup", user_id);
    }
    if matrix_clients.remove(&user_id).is_some() {
        debug!("Removed Matrix client for user {} during cleanup", user_id);
    }
}

/// Migrate existing digest settings to tasks (one-time migration)
/// This converts morning_digest, day_digest, evening_digest from user_settings
/// into recurring tasks with sources.
async fn migrate_digests_to_tasks(state: &Arc<AppState>) {
    tracing::info!("Starting digest migration to tasks...");

    let users = match state.user_core.get_all_users() {
        Ok(u) => u,
        Err(e) => {
            tracing::error!("Failed to get users for digest migration: {}", e);
            return;
        }
    };

    let mut migrated_count = 0;

    for user in users {
        let user_id = user.id;

        // Get digest settings
        let (morning, day, evening) = match state.user_core.get_digests(user_id) {
            Ok(d) => d,
            Err(_) => continue, // Skip users without settings
        };

        // Skip if no digests configured
        if morning.is_none() && day.is_none() && evening.is_none() {
            continue;
        }

        // Get user timezone for trigger calculation
        let user_tz = state
            .user_core
            .get_user_info(user_id)
            .ok()
            .and_then(|info| info.timezone)
            .unwrap_or_else(|| "UTC".to_string());

        let tz: chrono_tz::Tz = match user_tz.parse() {
            Ok(t) => t,
            Err(_) => chrono_tz::UTC,
        };

        let now = chrono::Utc::now();
        let now_local = now.with_timezone(&tz);
        let current_ts = now.timestamp() as i32;

        // Helper to create digest task - returns Option to handle DST/invalid time gracefully
        let create_digest_task = |digest_time: &str,
                                  _digest_name: &str|
         -> Option<crate::models::user_models::NewTask> {
            // Parse hour from "HH:00" format
            let hour: u32 = digest_time
                .split(':')
                .next()
                .and_then(|h| h.parse().ok())
                .unwrap_or(8);

            // Calculate next occurrence - use ? to handle DST transitions gracefully
            let mut next_time = now_local.date_naive().and_hms_opt(hour, 0, 0)?;
            let check_time = chrono::NaiveTime::from_hms_opt(hour, 0, 0)?;
            if now_local.time() >= check_time {
                // Already past this time today, schedule for tomorrow
                next_time += chrono::Duration::days(1);
            }
            let next_dt = tz.from_local_datetime(&next_time).single()?;
            let trigger_ts = next_dt.timestamp() as i32;

            Some(crate::models::user_models::NewTask {
                user_id,
                trigger: format!("once_{}", trigger_ts),
                condition: None,
                action: "generate_digest".to_string(),
                notification_type: Some("sms".to_string()),
                status: "active".to_string(),
                created_at: current_ts,
                is_permanent: Some(1), // Permanent recurring task
                recurrence_rule: Some("daily".to_string()),
                recurrence_time: Some(digest_time.to_string()), // "HH:00" format
                sources: Some("email,whatsapp,telegram,signal,calendar".to_string()),
                end_time: None,
            })
        };

        // Idempotency check: skip if user already has digest tasks
        let existing_tasks = state
            .user_repository
            .get_user_tasks(user_id)
            .unwrap_or_default();
        let has_digest_tasks = existing_tasks
            .iter()
            .any(|t| t.action == "generate_digest" && t.is_permanent == Some(1));

        if has_digest_tasks {
            tracing::debug!(
                "User {} already has digest tasks, skipping migration",
                user_id
            );
            continue;
        }

        // Create tasks for each enabled digest
        let mut created_tasks = Vec::new();
        let mut failed_tasks = Vec::new();

        if let Some(ref time) = morning {
            if let Some(task) = create_digest_task(time, "morning") {
                match state.user_repository.create_task(&task) {
                    Ok(_) => created_tasks.push("morning"),
                    Err(e) => {
                        tracing::error!(
                            "Failed to create morning digest task for user {}: {}",
                            user_id,
                            e
                        );
                        failed_tasks.push("morning");
                    }
                }
            } else {
                tracing::warn!("Invalid time format for morning digest: {}", time);
            }
        }

        if let Some(ref time) = day {
            if let Some(task) = create_digest_task(time, "day") {
                match state.user_repository.create_task(&task) {
                    Ok(_) => created_tasks.push("day"),
                    Err(e) => {
                        tracing::error!(
                            "Failed to create day digest task for user {}: {}",
                            user_id,
                            e
                        );
                        failed_tasks.push("day");
                    }
                }
            } else {
                tracing::warn!("Invalid time format for day digest: {}", time);
            }
        }

        if let Some(ref time) = evening {
            if let Some(task) = create_digest_task(time, "evening") {
                match state.user_repository.create_task(&task) {
                    Ok(_) => created_tasks.push("evening"),
                    Err(e) => {
                        tracing::error!(
                            "Failed to create evening digest task for user {}: {}",
                            user_id,
                            e
                        );
                        failed_tasks.push("evening");
                    }
                }
            } else {
                tracing::warn!("Invalid time format for evening digest: {}", time);
            }
        }

        // Only clear old settings if we created at least one task AND had no failures
        if !created_tasks.is_empty() && failed_tasks.is_empty() {
            if let Err(e) = state.user_core.update_digests(user_id, None, None, None) {
                tracing::warn!(
                    "Failed to clear digest settings for user {}: {}",
                    user_id,
                    e
                );
            } else {
                migrated_count += 1;
                tracing::info!(
                    "Migrated {} digest(s) to tasks for user {}: {:?}",
                    created_tasks.len(),
                    user_id,
                    created_tasks
                );
            }
        } else if !failed_tasks.is_empty() {
            tracing::warn!(
                "Skipping digest settings clear for user {} due to failures: {:?}",
                user_id,
                failed_tasks
            );
        }
    }

    tracing::info!(
        "Digest migration complete: {} users migrated",
        migrated_count
    );
}

/// Initialize the smartphone-free days metric if it doesn't exist.
/// This runs on startup to ensure the metric is available immediately.
async fn initialize_smartphone_free_days_metric(state: Arc<AppState>) {
    // Check if metric already exists
    match state.metrics_repository.get_metric("smartphone_free_days") {
        Ok(Some(_)) => {
            tracing::debug!("smartphone_free_days metric already exists, skipping initialization");
        }
        Ok(None) => {
            tracing::info!("smartphone_free_days metric not found, calculating initial value...");
            match crate::services::metrics_service::calculate_smartphone_free_days().await {
                Ok(days) => {
                    match state
                        .metrics_repository
                        .upsert_metric("smartphone_free_days", &days.to_string())
                    {
                        Ok(()) => {
                            tracing::info!(
                                "Initialized smartphone_free_days metric to {} days",
                                days
                            );
                        }
                        Err(e) => {
                            tracing::error!(
                                "Failed to save initial smartphone_free_days metric: {}",
                                e
                            );
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to calculate initial smartphone-free days (will retry in cron job): {}",
                        e
                    );
                }
            }
        }
        Err(e) => {
            tracing::warn!(
                "Failed to check for existing smartphone_free_days metric: {}",
                e
            );
        }
    }
}

pub async fn start_scheduler(state: Arc<AppState>) {
    // One-time migration: convert digest settings to tasks
    migrate_digests_to_tasks(&state).await;

    // Initialize matrix clients and sync tasks once on startup
    tracing::debug!("Initializing Matrix clients and sync tasks...");
    initialize_matrix_clients(Arc::clone(&state)).await;

    // Initialize smartphone-free days metric if it doesn't exist
    initialize_smartphone_free_days_metric(Arc::clone(&state)).await;

    let sched = JobScheduler::new()
        .await
        .expect("Failed to create scheduler");

    // Create a job that runs every 10 minutes to check for new IMAP messages
    let state_clone = Arc::clone(&state);
    let message_monitor_job = Job::new_async("0 */10 * * * *", move |_, _| {
    //let message_monitor_job = Job::new_async("*/30 * * * * *", move |_, _| {
        let state = state_clone.clone();
        Box::pin(async move {

            // Process each subscribed user
            for user in state.user_core.get_users_by_tier("tier 2").unwrap_or_default(){

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
                                        let a_date = a.date.unwrap_or_else(chrono::Utc::now);
                                        let b_date = b.date.unwrap_or_else(chrono::Utc::now);
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
                                        // Check if sender matches priority senders and send the noti anyways about it
                                        if let Some(matched_sender) = priority_senders.iter().filter(|p_send| p_send.noti_mode == "all").find(|priority_sender| {
                                            let priority_lower = priority_sender.sender.to_lowercase();
                                            // Check 'from' (display name)
                                            let from_matches = email.from.as_deref().unwrap_or("Unknown").to_lowercase().contains(&priority_lower);
                                            // Also check 'from_email' (actual email address)
                                            let from_email_matches = email.from_email.as_deref().unwrap_or("Unknown").to_lowercase().contains(&priority_lower);
                                            from_matches || from_email_matches
                                        }) {
                                            tracing::info!("Fast check: Priority sender matched for user {}", user.id);

                                            // Determine suffix based on noti_type
                                            let suffix = match matched_sender.noti_type.as_deref() {
                                                Some("call") => "_call",
                                                Some("call_sms") => "_call_sms",
                                                _ => "_sms",
                                            };
                                            let notification_type = format!("email_priority{}", suffix);

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
                                                    notification_type,
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

                                        // Check recurring_email tasks if they exist
                                        let email_tasks = match state.user_repository.get_recurring_tasks_for_user(user.id, "recurring_email") {
                                            Ok(tasks) => tasks,
                                            Err(e) => {
                                                tracing::error!("Failed to get recurring email tasks for user {}: {}", user.id, e);
                                                Vec::new()
                                            }
                                        };
                                        if !email_tasks.is_empty() {
                                            // Check if any tasks match the message
                                            if let Ok((Some(task_id), _message, _first_message)) = crate::proactive::utils::check_task_condition_match(
                                                &state,
                                                user.id,
                                                &email_content,
                                                &email_tasks,
                                            ).await {
                                                // Find the matched task to get its action and notification_type
                                                let matched_task = email_tasks.iter().find(|t| t.id == Some(task_id)).cloned();
                                                if let Some(task) = matched_task {
                                                        let notification_type = task.notification_type.clone().unwrap_or_else(|| "sms".to_string());
                                                        let action_spec = task.action.clone();

                                                        // Complete or reschedule the task (for permanent recurring tasks)
                                                        let user_tz = state.user_core.get_user_info(user.id)
                                                            .ok()
                                                            .and_then(|info| info.timezone)
                                                            .unwrap_or_else(|| "UTC".to_string());
                                                        match state.user_repository.complete_or_reschedule_task(&task, &user_tz) {
                                                            Ok(rescheduled) => {
                                                                if rescheduled {
                                                                    tracing::debug!("Rescheduled permanent task {}", task_id);
                                                                }
                                                            }
                                                            Err(e) => tracing::error!("Failed to complete task {}: {}", task_id, e),
                                                        }

                                                        // Execute the action_spec through AI + tools, passing the email context
                                                        let state_clone = state.clone();
                                                        let user_id = user.id;
                                                        let trigger_context = email_content.clone();
                                                        tokio::spawn(async move {
                                                            tracing::debug!("Executing email task {} for user {}: {}", task_id, user_id, action_spec);
                                                            match crate::utils::action_executor::execute_action_spec(
                                                                &state_clone,
                                                                user_id,
                                                                &action_spec,
                                                                &notification_type,
                                                                Some(&trigger_context),
                                                                None, // No sources for recurring email tasks
                                                                None, // Condition already matched
                                                            ).await {
                                                                crate::utils::action_executor::ActionResult::Success { message } => {
                                                                    tracing::debug!("Email task {} completed successfully: {}", task_id, message);
                                                                }
                                                                crate::utils::action_executor::ActionResult::Skipped { reason } => {
                                                                    tracing::debug!("Email task {} skipped: {}", task_id, reason);
                                                                }
                                                                crate::utils::action_executor::ActionResult::Failed { error } => {
                                                                    tracing::error!("Email task {} failed: {}", task_id, error);
                                                                }
                                                            }
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
                                    match crate::proactive::utils::check_message_importance(&state, user.id, &emails_content, "", "", "").await {
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

    sched
        .add(message_monitor_job)
        .await
        .expect("Failed to add message monitor job to scheduler");

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
                        if let Ok(Some(tier)) = state.user_repository.get_subscription_tier(user.id)
                        {
                            if !state
                                .user_core
                                .get_proactive_agent_on(user.id)
                                .unwrap_or(true)
                            {
                                tracing::debug!(
                                    "User {} does not have monitoring enabled",
                                    user.id
                                );
                                continue;
                            }
                            if tier == "tier 2" {
                                debug!(
                                    "Checking morning digest for user {} with tier 2 subscription",
                                    user.id
                                );
                                if let Err(e) =
                                    crate::proactive::utils::check_morning_digest(&state, user.id)
                                        .await
                                {
                                    error!(
                                        "Failed to check morning digest for user {}: {}",
                                        user.id, e
                                    );
                                }
                                if let Err(e) =
                                    crate::proactive::utils::check_day_digest(&state, user.id).await
                                {
                                    error!(
                                        "Failed to check day digest for user {}: {}",
                                        user.id, e
                                    );
                                }
                                if let Err(e) =
                                    crate::proactive::utils::check_evening_digest(&state, user.id)
                                        .await
                                {
                                    error!(
                                        "Failed to check evening digest for user {}: {}",
                                        user.id, e
                                    );
                                }
                            }
                        }
                    }
                }
                Err(e) => error!("Failed to fetch users for morning digest check: {}", e),
            }
        })
    })
    .expect("Failed to create digest check job");

    sched
        .add(digest_check_job)
        .await
        .expect("Failed to add digest check job to scheduler");

    // Create a job that runs every 5 minutes to check for upcoming calendar events
    let state_clone = Arc::clone(&state);
    let calendar_notification_job = Job::new_async("0 */5 * * * *", move |_, _| {
        // Run every 5 minutes
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
            let cleanup_threshold =
                (chrono::Utc::now() - chrono::Duration::hours(24)).timestamp() as i32;
            for attempt in 1..=3 {
                match state
                    .user_repository
                    .cleanup_old_calendar_notifications(cleanup_threshold)
                {
                    Ok(_) => break,
                    Err(e) => {
                        error!(
                            "Attempt {} to clean up old calendar notifications failed: {}",
                            attempt, e
                        );
                        if attempt < 3 {
                            tokio::time::sleep(tokio::time::Duration::from_millis(
                                100 * attempt as u64,
                            ))
                            .await;
                        }
                    }
                }
            }

            // Get all users with valid Google Calendar connection and subscription
            let users = match state.user_core.get_all_users() {
                Ok(users) => users
                    .into_iter()
                    .filter(|user| {
                        // Check subscription and calendar status
                        matches!(
                            state
                                .user_repository
                                .has_valid_subscription_tier(user.id, "tier 2"),
                            Ok(true)
                        ) && matches!(
                            state.user_repository.has_active_google_calendar(user.id),
                            Ok(true)
                        ) && matches!(state.user_core.get_proactive_agent_on(user.id), Ok(true))
                    })
                    .collect::<Vec<_>>(),
                Err(e) => {
                    error!("Failed to fetch users: {}", e);
                    return;
                }
            };
            let now = chrono::Utc::now();
            let window_end = now + chrono::Duration::minutes(30);

            debug!(
                "🗓️ Calendar check: Starting check for {} users at {}",
                users.len(),
                now.format("%Y-%m-%d %H:%M:%S UTC")
            );

            // Process users with rate limiting
            for (index, user) in users.iter().enumerate() {
                // Add delay between users to avoid rate limiting
                if index > 0 {
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                }

                debug!(
                    "🗓️ Calendar check: Processing user {} ({}/{})",
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
                    },
                )
                .await
                {
                    Ok(events) => {
                        debug!(
                            "🗓️ Calendar check: Found {} events for user {}",
                            events.len(),
                            user.id
                        );
                        for event in events {
                            if let (Some(reminders), Some(_start_time)) =
                                (&event.reminders, event.start.date_time)
                            {
                                for reminder in &reminders.overrides {
                                    let reminder_key = format!("{}_{}", event.id, reminder.minutes);

                                    // Check if notification was already sent
                                    if state
                                        .user_repository
                                        .check_calendar_notification_exists(user.id, &reminder_key)
                                        .unwrap_or(true)
                                    {
                                        continue;
                                    }

                                    // Record notification before sending
                                    let new_notification =
                                        crate::models::user_models::NewCalendarNotification {
                                            user_id: user.id,
                                            event_id: reminder_key.clone(),
                                            notification_time: now.timestamp() as i32,
                                        };

                                    if let Err(e) = state
                                        .user_repository
                                        .create_calendar_notification(&new_notification)
                                    {
                                        error!("Failed to record calendar notification: {}", e);
                                        continue;
                                    }

                                    let event_summary = event
                                        .summary
                                        .clone()
                                        .unwrap_or_else(|| "Untitled Event".to_string());
                                    let notification = format!(
                                        "Calendar: {} in {} mins",
                                        event_summary, reminder.minutes
                                    );

                                    let state_clone = state.clone();
                                    let first_message = format!(
                                        "Hello, you have a calendar event starting in {}.",
                                        reminder.minutes
                                    );
                                    let user_id = user.id;
                                    tokio::spawn(async move {
                                        crate::proactive::utils::send_notification(
                                            &state_clone,
                                            user_id,
                                            &notification,
                                            "calendar_notification".to_string(),
                                            Some(first_message),
                                        )
                                        .await;
                                    });
                                }
                            }
                        }
                    }
                    Err(e) => error!(
                        "Failed to fetch calendar events for user {}: {}",
                        user.id, e
                    ),
                }
            }
        })
    })
    .expect("Failed to create calendar notification job");

    sched
        .add(calendar_notification_job)
        .await
        .expect("Failed to add calendar notification job to scheduler");

    // Bridge health check - runs daily at midnight UTC
    let state_clone = Arc::clone(&state);
    let bridge_health_job = Job::new_async("0 0 0 * * *", move |_, _| {
        let state = state_clone.clone();
        Box::pin(async move {
            debug!("Running daily bridge health check...");
            if let Err(e) = check_all_bridges_health(&state).await {
                error!("Bridge health check failed: {}", e);
            }
        })
    })
    .expect("Failed to create bridge health check job");

    sched
        .add(bridge_health_job)
        .await
        .expect("Failed to add bridge health check job to scheduler");

    // Time-triggered tasks - runs every 5 minutes to execute due "once_*" tasks
    let state_clone = Arc::clone(&state);
    let once_tasks_job = Job::new_async("0 */5 * * * *", move |_, _| {
        let state = state_clone.clone();
        Box::pin(async move {
            debug!("Checking for due scheduled tasks...");
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i32;

            match state.user_repository.get_due_once_tasks(now) {
                Ok(tasks) => {
                    debug!("Found {} due scheduled tasks", tasks.len());
                    for task in tasks {
                        // Skip quiet_mode tasks - they're not executable actions.
                        // Auto-complete expired ones.
                        if crate::handlers::dashboard_handlers::is_quiet_mode_task(&task.action) {
                            if let Some(end_ts) = task.end_time {
                                if end_ts <= now {
                                    let _ = state
                                        .user_repository
                                        .update_task_status(task.id.unwrap_or(0), "completed");
                                }
                            }
                            continue;
                        }

                        let state = state.clone();
                        let task_clone = task.clone();
                        let task_id = task.id.unwrap_or(0);
                        let user_id = task.user_id;
                        let action = task.action.clone();
                        let notification_type = task
                            .notification_type
                            .clone()
                            .unwrap_or_else(|| "sms".to_string());

                        // Clone task fields for use in async block
                        let sources = task.sources.clone();
                        let condition = task.condition.clone();

                        tokio::spawn(async move {
                            debug!(
                                "Executing scheduled task {} for user {}: {}",
                                task_id, user_id, action
                            );

                            // Execute the action with sources, condition, and auto-notification
                            match crate::utils::action_executor::execute_action_spec(
                                &state,
                                user_id,
                                &action,
                                &notification_type,
                                None, // No trigger context for time-based tasks
                                sources.as_deref(),
                                condition.as_deref(),
                            )
                            .await
                            {
                                crate::utils::action_executor::ActionResult::Success {
                                    message,
                                } => {
                                    debug!("Task {} completed successfully: {}", task_id, message);
                                }
                                crate::utils::action_executor::ActionResult::Skipped { reason } => {
                                    debug!("Task {} skipped: {}", task_id, reason);
                                }
                                crate::utils::action_executor::ActionResult::Failed { error } => {
                                    error!("Task {} failed: {}", task_id, error);
                                    // Optionally notify user of failure
                                    let noti_type = format!("task_failed_{}", notification_type);
                                    crate::proactive::utils::send_notification(
                                        &state,
                                        user_id,
                                        &format!("Your scheduled task failed: {}", error),
                                        noti_type,
                                        Some(
                                            "Sorry, your scheduled task encountered an error."
                                                .to_string(),
                                        ),
                                    )
                                    .await;
                                }
                            }

                            // Complete or reschedule the task (for permanent recurring tasks)
                            let user_tz = state
                                .user_core
                                .get_user_info(user_id)
                                .ok()
                                .and_then(|info| info.timezone)
                                .unwrap_or_else(|| "UTC".to_string());
                            match state
                                .user_repository
                                .complete_or_reschedule_task(&task_clone, &user_tz)
                            {
                                Ok(rescheduled) => {
                                    if rescheduled {
                                        debug!("Rescheduled permanent task {}", task_id);
                                    } else {
                                        debug!("Task {} marked as completed", task_id);
                                    }
                                }
                                Err(e) => error!("Failed to complete task {}: {}", task_id, e),
                            }
                        });
                    }
                }
                Err(e) => error!("Failed to get due scheduled tasks: {}", e),
            }
        })
    })
    .expect("Failed to create once tasks job");

    sched
        .add(once_tasks_job)
        .await
        .expect("Failed to add once tasks job to scheduler");

    // Admin alert cleanup - runs daily at 2am UTC to remove alerts older than 30 days
    let state_clone = Arc::clone(&state);
    let alert_cleanup_job = Job::new_async("0 0 2 * * *", move |_, _| {
        let state = state_clone.clone();
        Box::pin(async move {
            debug!("Running admin alert cleanup...");

            // Clean up alerts older than 30 days
            let cutoff = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i32
                - (30 * 24 * 60 * 60); // 30 days ago

            match state.admin_alert_repository.delete_old_alerts(cutoff) {
                Ok(count) => debug!("Cleaned up {} old admin alerts", count),
                Err(e) => error!("Failed to cleanup old admin alerts: {}", e),
            }
        })
    })
    .expect("Failed to create alert cleanup job");

    sched
        .add(alert_cleanup_job)
        .await
        .expect("Failed to add alert cleanup job to scheduler");

    // Task cleanup - runs daily at 3am UTC to remove old completed/cancelled tasks
    let state_clone = Arc::clone(&state);
    let task_cleanup_job = Job::new_async("0 0 3 * * *", move |_, _| {
        let state = state_clone.clone();
        Box::pin(async move {
            debug!("Running daily cleanup...");

            // Clean up old tasks (7 days)
            let task_cutoff = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i32
                - (7 * 24 * 60 * 60); // 7 days ago

            match state.user_repository.delete_old_tasks(task_cutoff) {
                Ok(count) => debug!("Cleaned up {} old tasks", count),
                Err(e) => error!("Failed to cleanup old tasks: {}", e),
            }

            // Clean up old message status logs (30 days)
            let message_log_cutoff = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i32
                - (30 * 24 * 60 * 60); // 30 days ago

            match state
                .user_repository
                .delete_old_message_status_logs(message_log_cutoff)
            {
                Ok(count) => debug!("Cleaned up {} old message status logs", count),
                Err(e) => error!("Failed to cleanup old message status logs: {}", e),
            }
        })
    })
    .expect("Failed to create task cleanup job");

    sched
        .add(task_cleanup_job)
        .await
        .expect("Failed to add task cleanup job to scheduler");

    // Smartphone-free days metric update - runs daily at 4am UTC
    let state_clone = Arc::clone(&state);
    let metrics_update_job = Job::new_async("0 0 4 * * *", move |_, _| {
        let state = state_clone.clone();
        Box::pin(async move {
            debug!("Running smartphone-free days metric update...");

            match crate::services::metrics_service::calculate_smartphone_free_days().await {
                Ok(days) => {
                    match state
                        .metrics_repository
                        .upsert_metric("smartphone_free_days", &days.to_string())
                    {
                        Ok(()) => {
                            tracing::info!("Updated smartphone_free_days metric to {} days", days);
                        }
                        Err(e) => {
                            error!("Failed to save smartphone_free_days metric: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to calculate smartphone-free days: {}", e);
                }
            }
        })
    })
    .expect("Failed to create metrics update job");

    sched
        .add(metrics_update_job)
        .await
        .expect("Failed to add metrics update job to scheduler");

    // Start the scheduler
    sched.start().await.expect("Failed to start scheduler");

    // TODO we should add another scheduled call that just checks if there are items that are 'done' or not found in the elevenlabs
    // but are still 'ongoing' in our db. we don't want to be accidentally charging users.
    // and if that happens make error visible
}
