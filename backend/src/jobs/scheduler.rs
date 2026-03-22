use crate::AppState;
use crate::UserCoreOps;

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
                            .timeout(std::time::Duration::from_secs(30));

                        let handle = tokio::spawn(async move {
                            let mut backoff_secs: u64 = 1;
                            const MAX_BACKOFF_SECS: u64 = 300;

                            loop {
                                match client.sync(sync_settings.clone()).await {
                                    Ok(_) => {
                                        tracing::debug!(
                                            "Sync completed normally for user {}",
                                            user_id
                                        );
                                        backoff_secs = 1; // Reset on success
                                        tokio::time::sleep(tokio::time::Duration::from_secs(1))
                                            .await;
                                    }
                                    Err(e) => {
                                        error!(
                                            "Matrix sync error for user {} (retry in {}s): {}",
                                            user_id, backoff_secs, e
                                        );
                                        tokio::time::sleep(tokio::time::Duration::from_secs(
                                            backoff_secs,
                                        ))
                                        .await;
                                        backoff_secs = (backoff_secs * 2).min(MAX_BACKOFF_SECS);
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
    bridge: &crate::pg_models::PgBridge,
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

/// Migrate existing digest settings to items (one-time migration).
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

            // Process each user with auto-features (autopilot/byot)
            let tier2_users = state.user_core.get_users_by_tier("tier 2").unwrap_or_default();
            for user in tier2_users.into_iter().filter(|u| {
                crate::utils::plan_features::has_auto_features(u.plan_type.as_deref())
            }) {

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

                                    for email in &sorted_emails {
                                        // Find matching ontology Person for this email sender
                                        let mut matched_person_id: Option<i32> = None;
                                        if let Ok(persons) = state.ontology_repository.get_persons_with_channels(user.id, 500, 0) {
                                            let from_lower = email.from_email.as_deref().unwrap_or("").to_lowercase();
                                            let from_name_lower = email.from.as_deref().unwrap_or("").to_lowercase();
                                            for pwc in &persons {
                                                for channel in &pwc.channels {
                                                    if channel.platform == "email" {
                                                        if let Some(ref handle) = channel.handle {
                                                            let handle_lower = handle.to_lowercase();
                                                            if from_lower.contains(&handle_lower) || from_name_lower.contains(&handle_lower) {
                                                                matched_person_id = Some(pwc.person.id);
                                                                break;
                                                            }
                                                        }
                                                    }
                                                }
                                                if matched_person_id.is_some() { break; }
                                            }
                                        }

                                        // Store email as ont_message + emit ontology change for rule matching
                                        let now = std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .unwrap_or_default()
                                            .as_secs() as i32;
                                        let email_uid = email.id.clone();
                                        let sender_name = email.from.as_deref().unwrap_or("Unknown").to_string();
                                        let content = format!(
                                            "{}\n{}",
                                            email.subject.as_deref().unwrap_or(""),
                                            email.body.as_deref().unwrap_or("").chars().take(500).collect::<String>()
                                        );
                                        let msg = crate::models::ontology_models::NewOntMessage {
                                            user_id: user.id,
                                            room_id: format!("email_{}", email_uid),
                                            platform: "email".to_string(),
                                            sender_name: sender_name.clone(),
                                            content: content.clone(),
                                            person_id: matched_person_id,
                                            created_at: now,
                                            pinned: false,
                                            status: None,
                                            review_after: None,
                                        };
                                        let state_emit = state.clone();
                                        let uid = user.id;
                                        tokio::spawn(async move {
                                            match state_emit.ontology_repository.insert_message(&msg) {
                                                Ok(created) => {
                                                    let snapshot = serde_json::json!({
                                                        "message_id": created.id,
                                                        "platform": "email",
                                                        "sender_name": msg.sender_name,
                                                        "content": msg.content,
                                                        "room_id": msg.room_id,
                                                    });
                                                    crate::proactive::rules::emit_ontology_change(
                                                        &state_emit,
                                                        uid,
                                                        "Message",
                                                        created.id as i32,
                                                        "created",
                                                        snapshot,
                                                    )
                                                    .await;
                                                }
                                                Err(e) => {
                                                    tracing::warn!("Failed to store email message: {}", e);
                                                }
                                            }
                                        });

                                        // Rules handle all notification logic via ontology change events
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

    // Cleanup job - runs daily at 3am UTC to remove old logs and expire stale records
    let state_clone = Arc::clone(&state);
    let task_cleanup_job = Job::new_async("0 0 3 * * *", move |_, _| {
        let state = state_clone.clone();
        Box::pin(async move {
            debug!("Running daily cleanup...");

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

            // Expire stale "ongoing" call records (no webhook received after 1 hour)
            let call_cutoff = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i32
                - 3600;
            match state
                .user_repository
                .expire_stale_ongoing_calls(call_cutoff)
            {
                Ok(count) => {
                    if count > 0 {
                        debug!("Expired {} stale ongoing call records", count);
                    }
                }
                Err(e) => error!("Failed to expire stale ongoing calls: {}", e),
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

    // Daily at 3 AM: purge ont_messages older than 14 days
    let state_clone = Arc::clone(&state);
    let message_purge_job = Job::new_async("0 0 3 * * *", move |_, _| {
        let state = state_clone.clone();
        Box::pin(async move {
            let max_age_secs = 14 * 24 * 3600; // 14 days
            match state.ontology_repository.purge_old_messages(max_age_secs) {
                Ok(count) if count > 0 => {
                    tracing::info!("Purged {} old ont_messages (>14 days)", count);
                }
                Err(e) => {
                    error!("Failed to purge old ont_messages: {}", e);
                }
                _ => {}
            }

            // Purge completed/expired rules older than 7 days
            let rule_max_age = 7 * 24 * 3600;
            match state.ontology_repository.purge_old_rules(rule_max_age) {
                Ok(count) if count > 0 => {
                    tracing::info!("Purged {} old completed/expired rules (>7 days)", count);
                }
                Err(e) => {
                    error!("Failed to purge old rules: {}", e);
                }
                _ => {}
            }
        })
    })
    .expect("Failed to create message purge job");

    sched
        .add(message_purge_job)
        .await
        .expect("Failed to add message purge job to scheduler");

    // Every minute: fire schedule-based ont_rules that are due
    let state_clone = Arc::clone(&state);
    let rule_schedule_job = Job::new_async("0 * * * * *", move |_, _| {
        let state = state_clone.clone();
        Box::pin(async move {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i32;

            let due_rules = match state.ontology_repository.get_due_schedule_rules(now) {
                Ok(rules) => rules,
                Err(e) => {
                    error!("Failed to load due schedule rules: {}", e);
                    return;
                }
            };

            for rule in due_rules {
                let state = state.clone();
                let rule_clone = rule.clone();

                // Update next_fire_at BEFORE spawn to prevent double-fire
                let trigger: crate::proactive::rules::TriggerConfig =
                    serde_json::from_str(&rule_clone.trigger_config).unwrap_or_default();
                if trigger.schedule.as_deref() == Some("recurring") {
                    if let Some(ref pattern) = trigger.pattern {
                        let user_tz = state
                            .user_core
                            .get_user_info(rule_clone.user_id)
                            .ok()
                            .and_then(|info| info.timezone)
                            .unwrap_or_else(|| "UTC".to_string());
                        if let Some(next) =
                            crate::proactive::rules::compute_next_fire_at(pattern, &user_tz)
                        {
                            let _ = state
                                .ontology_repository
                                .update_rule_next_fire_at(rule_clone.id, next);
                        }
                    }
                }
                if trigger.schedule.as_deref() == Some("once") || trigger.fire_once {
                    let _ = state
                        .ontology_repository
                        .update_rule_next_fire_at(rule_clone.id, i32::MAX);
                }

                tokio::spawn(async move {
                    let ctx = format!(
                        "Schedule trigger fired at {}",
                        chrono::Utc::now().to_rfc3339()
                    );
                    crate::proactive::rules::evaluate_and_execute(&state, &rule_clone, &ctx, None)
                        .await;
                });
            }

            // Also expire old rules
            let _ = state.ontology_repository.expire_old_rules(now);
        })
    })
    .expect("Failed to create rule schedule job");

    sched
        .add(rule_schedule_job)
        .await
        .expect("Failed to add rule schedule job to scheduler");

    // Start the scheduler
    sched.start().await.expect("Failed to start scheduler");

    // TODO we should add another scheduled call that just checks if there are items that are 'done' or not found in the elevenlabs
    // but are still 'ongoing' in our db. we don't want to be accidentally charging users.
    // and if that happens make error visible
}
