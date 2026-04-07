use crate::AppState;
use crate::UserCoreOps;

use chrono::Offset;
use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{debug, error};

use crate::handlers::imap_handlers;

pub async fn initialize_matrix_clients(state: Arc<AppState>) {
    tracing::debug!("Starting Matrix client initialization...");

    // Get all users with active WhatsApp connection
    match state
        .user_repository
        .get_users_with_matrix_bridge_connections()
    {
        Ok(users) => {
            // Clear existing clients and sync tasks
            {
                let mut matrix_clients = state.matrix_clients.lock().await;
                let mut sync_tasks = state.matrix_sync_tasks.lock().await;
                for (_, task) in sync_tasks.drain() {
                    task.abort();
                }
                matrix_clients.clear();
            }
            // Lock is released here - get_client can safely lock it again

            // Setup clients and sync tasks for active users.
            // Stagger initialization to avoid overwhelming tuwunel with 229+ concurrent syncs
            let user_count = users.len();
            tracing::info!(
                "Initializing Matrix clients for {} users (staggered)",
                user_count
            );
            for (idx, user_id) in users.into_iter().enumerate() {
                tracing::debug!(
                    "Setting up Matrix client {}/{} for user {}",
                    idx + 1,
                    user_count,
                    user_id
                );

                match crate::utils::matrix_auth::get_cached_client(user_id, &state).await {
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

                        // Create sync task
                        let sync_settings = matrix_sdk::config::SyncSettings::default()
                            .timeout(std::time::Duration::from_secs(30));

                        let client_for_sync = client.clone();
                        let handle = tokio::spawn(async move {
                            let mut backoff_secs: u64 = 1;
                            const MAX_BACKOFF_SECS: u64 = 300;

                            loop {
                                match client_for_sync.sync(sync_settings.clone()).await {
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

                        {
                            let mut sync_tasks = state.matrix_sync_tasks.lock().await;
                            sync_tasks.insert(user_id, handle);
                        }

                        // Stagger sync starts: 100ms between each to avoid thundering herd on tuwunel
                        if idx < user_count - 1 {
                            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        }
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

/// Check if any Matrix sync tasks have died and restart them.
/// A dead sync task means messages stop flowing even though the bridge
/// DB record still shows "connected".
pub async fn check_and_restart_dead_sync_tasks(state: &Arc<AppState>) {
    let mut dead_user_ids = Vec::new();

    // Find dead sync tasks
    {
        let sync_tasks = state.matrix_sync_tasks.lock().await;
        for (&user_id, handle) in sync_tasks.iter() {
            if handle.is_finished() {
                tracing::warn!("Sync task for user {} is dead, will restart", user_id);
                dead_user_ids.push(user_id);
            }
        }
    }

    if dead_user_ids.is_empty() {
        return;
    }

    tracing::info!(
        "Restarting {} dead sync tasks: {:?}",
        dead_user_ids.len(),
        dead_user_ids
    );

    // Restart each dead sync task
    for user_id in dead_user_ids {
        // Remove the dead task handle
        {
            let mut sync_tasks = state.matrix_sync_tasks.lock().await;
            sync_tasks.remove(&user_id);
        }

        // Get existing cached client (should still be there)
        let client = match state.matrix_clients.lock().await.get(&user_id).cloned() {
            Some(c) => c,
            None => {
                // Client was also removed - try to recreate from scratch
                match crate::utils::matrix_auth::get_cached_client(user_id, state).await {
                    Ok(c) => {
                        // Register event handler on the new client
                        use matrix_sdk::room::Room;
                        use matrix_sdk::ruma::events::room::message::OriginalSyncRoomMessageEvent;
                        let state_for_handler = Arc::clone(state);
                        c.add_event_handler(
                            move |ev: OriginalSyncRoomMessageEvent, room: Room, client| {
                                let state = Arc::clone(&state_for_handler);
                                async move {
                                    crate::utils::bridge::handle_bridge_message(
                                        ev, room, client, state,
                                    )
                                    .await;
                                }
                            },
                        );
                        // Store in cache
                        state.matrix_clients.lock().await.insert(user_id, c.clone());
                        c
                    }
                    Err(e) => {
                        tracing::error!(
                            "Failed to recreate Matrix client for user {}: {}",
                            user_id,
                            e
                        );
                        continue;
                    }
                }
            }
        };

        // Start new sync task
        let sync_settings =
            matrix_sdk::config::SyncSettings::default().timeout(std::time::Duration::from_secs(30));
        let client_for_sync = client.clone();
        let handle = tokio::spawn(async move {
            let mut backoff_secs: u64 = 1;
            const MAX_BACKOFF_SECS: u64 = 300;
            loop {
                match client_for_sync.sync(sync_settings.clone()).await {
                    Ok(_) => {
                        backoff_secs = 1;
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    }
                    Err(e) => {
                        error!(
                            "Matrix sync error for user {} (retry in {}s): {}",
                            user_id, backoff_secs, e
                        );
                        tokio::time::sleep(tokio::time::Duration::from_secs(backoff_secs)).await;
                        backoff_secs = (backoff_secs * 2).min(MAX_BACKOFF_SECS);
                    }
                }
            }
        });

        {
            let mut sync_tasks = state.matrix_sync_tasks.lock().await;
            sync_tasks.insert(user_id, handle);
        }
        tracing::info!("Restarted sync task for user {}", user_id);
    }
}

/// Delete bridges stuck in "connecting" status for more than 10 minutes.
/// A bridge stuck in "connecting" blocks message processing for that service.
fn cleanup_stale_connecting_bridges(state: &Arc<AppState>) {
    use crate::pg_schema::bridges;
    use diesel::prelude::*;

    let mut conn = match state.user_repository.pool.get() {
        Ok(c) => c,
        Err(e) => {
            error!(
                "Failed to get DB connection for stale bridge cleanup: {}",
                e
            );
            return;
        }
    };

    let five_minutes_ago = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i32
        - 300;

    let stale: Vec<(i32, String)> = match bridges::table
        .filter(bridges::status.eq("connecting"))
        .filter(
            bridges::created_at
                .lt(five_minutes_ago)
                .or(bridges::created_at.is_null()),
        )
        .select((bridges::user_id, bridges::bridge_type))
        .load::<(i32, String)>(&mut conn)
    {
        Ok(rows) => rows,
        Err(e) => {
            error!("Failed to query stale connecting bridges: {}", e);
            return;
        }
    };

    for (user_id, bridge_type) in &stale {
        tracing::warn!(
            "Deleting stale 'connecting' bridge {} for user {} (older than 5 min)",
            bridge_type,
            user_id
        );
        if let Err(e) = state.user_repository.delete_bridge(*user_id, bridge_type) {
            error!(
                "Failed to delete stale connecting bridge {} for user {}: {}",
                bridge_type, user_id, e
            );
        }
    }

    if !stale.is_empty() {
        tracing::info!("Cleaned up {} stale connecting bridges", stale.len());
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
            // Hard cap on the entire job duration. If a slow IMAP server or DB
            // contention causes the loop to run long, we abort instead of holding
            // pool connections indefinitely and starving /api/health.
            // 5 minutes is well under the 10-minute cron interval so the next run
            // will start cleanly.
            let job_body = async {

            // Process each user with auto-features (autopilot/byot)
            let tier2_users = state.user_core.get_users_by_tier("tier 2").unwrap_or_default();
            for user in tier2_users.into_iter().filter(|u| {
                crate::utils::plan_features::has_auto_features(u.plan_type.as_deref())
            }) {

                // Check IMAP service
                // Wrapped in catch_unwind via spawned task to prevent imap crate
                // panics (e.g. invalid encoding) from killing the scheduler loop.
                if let Ok(imap_users) = state.user_repository.get_active_imap_connection_users() {
                    if imap_users.contains(&user.id) {
                        let fetch_result = {
                            let state_ref = state.clone();
                            let uid = user.id;
                            tokio::spawn(async move {
                                imap_handlers::fetch_emails_imap(&state_ref, uid, Some(10), true, true).await
                            }).await
                        };
                        let fetch_emails = match fetch_result {
                            Ok(inner) => inner,
                            Err(e) => {
                                error!("IMAP fetch task panicked for user {}: {}", user.id, e);
                                continue;
                            }
                        };
                        match fetch_emails {
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
                                        };
                                        let state_emit = state.clone();
                                        let uid = user.id;
                                        tokio::spawn(async move {
                                            match state_emit.ontology_repository.insert_message(&msg) {
                                                Ok(created) => {
                                                    let snapshot = serde_json::json!({
                                                        "message_id": created.id,
                                                        "platform": "email",
                                                        "sender": msg.sender_name,
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

            }; // end of job_body

            match tokio::time::timeout(std::time::Duration::from_secs(300), job_body).await {
                Ok(()) => {}
                Err(_) => {
                    error!(
                        "Message monitor job exceeded 5-minute timeout - aborting to free DB connections. Next run will retry."
                    );
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

    // Sync task health check - runs every 2 minutes to restart dead sync tasks
    // and clean up stale "connecting" bridges
    let state_clone = Arc::clone(&state);
    let sync_health_job = Job::new_async("0 */2 * * * *", move |_, _| {
        let state = state_clone.clone();
        Box::pin(async move {
            check_and_restart_dead_sync_tasks(&state).await;
            cleanup_stale_connecting_bridges(&state);
        })
    })
    .expect("Failed to create sync health check job");

    sched
        .add(sync_health_job)
        .await
        .expect("Failed to add sync health check job to scheduler");

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

            // Purge dismissed/expired/notified events older than 30 days
            let event_max_age = 30 * 24 * 3600;
            match state.ontology_repository.purge_old_events(event_max_age) {
                Ok(count) if count > 0 => {
                    tracing::info!("Purged {} old events (>30 days)", count);
                }
                Err(e) => {
                    error!("Failed to purge old events: {}", e);
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

    // Every minute: process event notifications and expirations
    let state_clone = Arc::clone(&state);
    let event_cron_job = Job::new_async("30 * * * * *", move |_, _| {
        let state = state_clone.clone();
        Box::pin(async move {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i32;

            // Notify events that are due
            let due_events = state
                .ontology_repository
                .get_events_due_for_notification(now)
                .unwrap_or_default();
            for event in &due_events {
                let message = format!("Event reminder: {}", event.description);
                crate::proactive::utils::send_notification(
                    &state,
                    event.user_id,
                    &message,
                    "event_notification".to_string(),
                    None,
                )
                .await;
                let _ = state.ontology_repository.update_event_status(
                    event.user_id,
                    event.id,
                    "notified",
                );
            }

            // Expire events past their due date
            let expired_events = state
                .ontology_repository
                .get_expired_events(now)
                .unwrap_or_default();
            for event in &expired_events {
                let _ = state.ontology_repository.update_event_status(
                    event.user_id,
                    event.id,
                    "expired",
                );
            }

            // Expire stale events with no due_at that haven't been updated in 14 days
            let stale_max_age = 14 * 24 * 3600;
            let stale_events = state
                .ontology_repository
                .get_stale_events(stale_max_age)
                .unwrap_or_default();
            for event in &stale_events {
                tracing::info!(
                    "Auto-expiring stale event {} for user {} (no due_at, inactive 14d)",
                    event.id,
                    event.user_id
                );
                let _ = state.ontology_repository.update_event_status(
                    event.user_id,
                    event.id,
                    "expired",
                );
            }
        })
    })
    .expect("Failed to create event cron job");

    sched
        .add(event_cron_job)
        .await
        .expect("Failed to add event cron job to scheduler");

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

    // Every 10 minutes: deliver smart digests to users at their predicted wake time
    // (10-min interval is the granularity for user-set custom digest times)
    let state_clone = Arc::clone(&state);
    let digest_job = Job::new_async("0 */10 * * * *", move |_, _| {
        let state = state_clone.clone();
        Box::pin(async move {
            deliver_smart_digests(&state).await;
        })
    })
    .expect("Failed to create digest delivery job");

    sched
        .add(digest_job)
        .await
        .expect("Failed to add digest delivery job to scheduler");

    // Start the scheduler
    sched.start().await.expect("Failed to start scheduler");
}

/// Parse and normalize a custom digest time string.
///
/// Accepts comma-separated `HH:MM` values (or bare `HH` which is treated as `HH:00`).
/// Each time is snapped to the nearest 10-minute boundary (matching the scheduler's
/// fire interval), deduplicated, and sorted.
///
/// Returns `(canonical_string, minutes_of_day)` where `canonical_string` is the
/// normalized form to store (e.g. `"08:50,09:00,11:00"`) and `minutes_of_day` is
/// the parsed slots in minutes-since-midnight.
pub fn parse_digest_times(input: &str) -> Result<(String, Vec<u16>), String> {
    let mut slots: Vec<u16> = Vec::new();
    for raw in input.split(',') {
        let token = raw.trim();
        if token.is_empty() {
            continue;
        }
        let (h_str, m_str) = match token.split_once(':') {
            Some((h, m)) => (h, m),
            None => (token, "0"),
        };
        let h: u32 = h_str
            .parse()
            .map_err(|_| format!("invalid hour in '{}'", token))?;
        let m: u32 = m_str
            .parse()
            .map_err(|_| format!("invalid minute in '{}'", token))?;
        if h >= 24 {
            return Err(format!("hour must be 0-23 in '{}'", token));
        }
        if m >= 60 {
            return Err(format!("minute must be 0-59 in '{}'", token));
        }
        // Snap to nearest 10-minute boundary, wrap 24:00 -> 00:00
        let total_min = (h * 60 + m) as u32;
        let snapped = (((total_min + 5) / 10) * 10) % 1440;
        let snapped = snapped as u16;
        if !slots.contains(&snapped) {
            slots.push(snapped);
        }
    }
    if slots.is_empty() {
        return Err("at least one time required".to_string());
    }
    if slots.len() > 4 {
        return Err(format!(
            "too many digest times: {} (max 4 per day)",
            slots.len()
        ));
    }
    slots.sort();
    let canonical = slots
        .iter()
        .map(|&m| format!("{:02}:{:02}", m / 60, m % 60))
        .collect::<Vec<_>>()
        .join(",");
    Ok((canonical, slots))
}

/// Pure helper used by `deliver_smart_digests` and unit tests:
/// returns true if any of the user's slots equals the snapped current minute-of-day.
pub fn should_deliver_now(slots: &[u16], local_minute_of_day: u16) -> bool {
    let current_slot = (local_minute_of_day / 10) * 10;
    slots.iter().any(|&s| s == current_slot)
}

/// Score a message's category for in-section ordering. Higher = more important.
/// Spam returns a very negative value so callers can drop those messages outright.
pub fn category_score(category: Option<&str>) -> i32 {
    match category {
        Some("emergency") => 100,
        Some("financial") => 60,
        Some("health") => 60,
        Some("work") => 30,
        Some("relationship") => 30,
        Some("logistics") => 15,
        Some("social") => 5,
        Some("spam") => -1000,
        _ => 10,
    }
}

/// Per-message teaser cap. We show just enough to grab interest — the user can
/// reply to ask for the full message via SMS.
const DIGEST_TEASER_CAP: usize = 50;

/// How many high-signal items to show individually before collapsing to counts.
const HIGH_SIGNAL_CAP: usize = 3;

/// How many sender names to list in collapsed FYI / New / Done sections.
const NAMES_LIST_CAP: usize = 5;

/// Strip a leading sender-name prefix from a summary, even when the LLM
/// re-states it despite the prompt. Handles common patterns like:
///   "Mom is feeling unwell"  → "feeling unwell"
///   "Mom: feeling unwell"    → "feeling unwell"
///   "Mom's project update"   → "project update"
///   "Mom asking about ..."   → "asking about ..."
/// Case-insensitive. Returns the original summary if the sender prefix isn't found.
pub fn strip_sender_prefix(summary: &str, sender: &str) -> String {
    let trimmed = summary.trim();
    let sender_trim = sender.trim();
    if sender_trim.is_empty() {
        return trimmed.to_string();
    }
    let lower_sum = trimmed.to_lowercase();
    let lower_send = sender_trim.to_lowercase();
    if !lower_sum.starts_with(&lower_send) {
        return trimmed.to_string();
    }
    // Slice off the sender's letters from the original (preserving case in the rest)
    let rest = &trimmed[sender_trim.len()..];
    // Strip a possessive 's first if present
    let rest = rest.strip_prefix("'s").unwrap_or(rest);
    // Then any leading punctuation/whitespace
    let rest =
        rest.trim_start_matches(|c: char| c == ':' || c == ',' || c == '-' || c.is_whitespace());
    // Finally strip a leading "is " / "was " / "has " connector if present
    let lower_rest = rest.to_lowercase();
    let rest = if let Some(stripped) = lower_rest.strip_prefix("is ") {
        &rest[rest.len() - stripped.len()..]
    } else if let Some(stripped) = lower_rest.strip_prefix("was ") {
        &rest[rest.len() - stripped.len()..]
    } else if let Some(stripped) = lower_rest.strip_prefix("has ") {
        &rest[rest.len() - stripped.len()..]
    } else {
        rest
    };
    let cleaned = rest.trim();
    if cleaned.is_empty() {
        // Stripping killed the whole summary — fall back to the original
        trimmed.to_string()
    } else {
        cleaned.to_string()
    }
}

/// Format a single high-signal message line: "- Sender: short teaser".
/// Drops platform name (redundant), strips sender re-statement from the
/// summary, and aggressively truncates since this is an interest hook, not
/// the full content.
fn format_digest_message(msg: &crate::models::ontology_models::OntMessage) -> String {
    let raw_summary = msg.summary.as_deref().unwrap_or(&msg.content);
    let cleaned = strip_sender_prefix(raw_summary, &msg.sender_name);
    let short: String = cleaned.chars().take(DIGEST_TEASER_CAP).collect();
    let trimmed = short.trim();
    if trimmed.is_empty() {
        format!("- {}", msg.sender_name)
    } else {
        format!("- {}: {}", msg.sender_name, trimmed)
    }
}

/// Render a high-signal section (Critical / Important): per-item lines with
/// short teasers, capped at HIGH_SIGNAL_CAP. Anything beyond is summarized
/// with "+N more".
fn render_high_signal_section(
    title: &str,
    messages: &[crate::models::ontology_models::OntMessage],
) -> Option<String> {
    if messages.is_empty() {
        return None;
    }
    let total = messages.len();
    let shown: Vec<String> = messages
        .iter()
        .take(HIGH_SIGNAL_CAP)
        .map(format_digest_message)
        .collect();
    let mut block = format!("{}:\n{}", title, shown.join("\n"));
    if total > HIGH_SIGNAL_CAP {
        block.push_str(&format!("\n+ {} more", total - HIGH_SIGNAL_CAP));
    }
    Some(block)
}

/// Render a low-signal section as a single inline sender list:
/// "FYI from Sarah, John, Newsletter (+2 more)".
/// Used for FYI to keep the digest from blowing up when there are many
/// medium-urgency messages.
fn render_fyi_inline(messages: &[crate::models::ontology_models::OntMessage]) -> Option<String> {
    if messages.is_empty() {
        return None;
    }
    let total = messages.len();
    // Dedupe sender names while preserving order
    let mut seen = std::collections::HashSet::new();
    let mut names: Vec<String> = Vec::new();
    for m in messages {
        if seen.insert(m.sender_name.clone()) {
            names.push(m.sender_name.clone());
            if names.len() >= NAMES_LIST_CAP {
                break;
            }
        }
    }
    let unique_total = messages
        .iter()
        .map(|m| &m.sender_name)
        .collect::<std::collections::HashSet<_>>()
        .len();
    let names_str = names.join(", ");
    let suffix = if unique_total > names.len() {
        format!(" +{} more", unique_total - names.len())
    } else {
        String::new()
    };
    let count_label = if total == 1 { "msg" } else { "msgs" };
    Some(format!(
        "FYI ({} {}): {}{}",
        total, count_label, names_str, suffix
    ))
}

/// Build a sectioned digest for a single user.
///
/// Pure-ish: makes DB queries and one round of async lookups for bridge read
/// receipts, but does not send anything or mutate state. Returns the rendered
/// digest text and the message IDs that should be marked delivered, or None
/// when there is nothing to send (after applying seen / spam filters).
///
/// Exposed for integration tests in `backend/tests/digest_layout_test.rs`.
pub async fn build_digest_for_user(
    state: &Arc<AppState>,
    user_id: i32,
    settings: &crate::models::user_models::UserSettings,
    now: i32,
    tz_offset_secs: i32,
) -> Option<(String, Vec<i64>)> {
    let local_ts = now as i64 + tz_offset_secs as i64;

    // Compute today's local-day window in UTC seconds. Used for "Today" events.
    let local_day_start_local = local_ts.div_euclid(86400) * 86400;
    let local_day_start = (local_day_start_local - tz_offset_secs as i64) as i32;
    let local_day_end = local_day_start.saturating_add(86400);

    // Window for catching unhandled messages: last 24 hours.
    let since_window: i32 = now.saturating_sub(86400);

    // -------- Section 1: Today's events (due_at within local day) --------
    let today_events = state
        .ontology_repository
        .get_events_due_on_local_day(user_id, local_day_start, local_day_end)
        .unwrap_or_default();

    // -------- Section 2/3: Critical + Important --------
    // Both are filtered out when the user has critical pushes ON, because in
    // that case the system already sent an SMS/call for both critical AND high
    // urgency messages — repeating them in the digest would be redundant.
    let critical_disabled = settings
        .critical_enabled
        .as_deref()
        .map(|v| v.eq_ignore_ascii_case("off"))
        .unwrap_or(false);

    let mut critical_msgs = if critical_disabled {
        state
            .ontology_repository
            .get_pending_messages_by_urgency(user_id, &["critical"], since_window, 20)
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let mut important_msgs = if critical_disabled {
        state
            .ontology_repository
            .get_pending_messages_by_urgency(user_id, &["high"], since_window, 20)
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    // -------- Section 4: FYI (medium urgency) --------
    let mut fyi_msgs = state
        .ontology_repository
        .get_pending_messages_by_urgency(user_id, &["medium"], since_window, 30)
        .unwrap_or_default();

    // -------- Filter: drop messages the user has already seen via bridge read receipts --------
    let mut seen_cache: std::collections::HashMap<String, Option<i64>> =
        std::collections::HashMap::new();
    let mut seen_lookup_keys: Vec<(String, String)> = Vec::new();
    for msg in critical_msgs
        .iter()
        .chain(important_msgs.iter())
        .chain(fyi_msgs.iter())
    {
        let key = format!("{}|{}", msg.room_id, msg.platform);
        if !seen_cache.contains_key(&key) {
            seen_cache.insert(key.clone(), None);
            seen_lookup_keys.push((msg.room_id.clone(), msg.platform.clone()));
        }
    }
    for (room_id, platform) in &seen_lookup_keys {
        let key = format!("{}|{}", room_id, platform);
        let ts =
            crate::proactive::system_behaviors::get_room_seen_ts(state, user_id, room_id, platform)
                .await;
        seen_cache.insert(key, ts);
    }

    let drop_seen = |msgs: &mut Vec<crate::models::ontology_models::OntMessage>| {
        msgs.retain(|m| {
            let key = format!("{}|{}", m.room_id, m.platform);
            match seen_cache.get(&key).copied().flatten() {
                Some(seen_ts) => (m.created_at as i64) > seen_ts,
                None => true,
            }
        });
    };
    drop_seen(&mut critical_msgs);
    drop_seen(&mut important_msgs);
    drop_seen(&mut fyi_msgs);

    // -------- Filter: drop spam --------
    let drop_spam = |msgs: &mut Vec<crate::models::ontology_models::OntMessage>| {
        msgs.retain(|m| m.category.as_deref() != Some("spam"));
    };
    drop_spam(&mut critical_msgs);
    drop_spam(&mut important_msgs);
    drop_spam(&mut fyi_msgs);

    // -------- Sort each section by category score, then recency --------
    let sort_section = |msgs: &mut Vec<crate::models::ontology_models::OntMessage>| {
        msgs.sort_by(|a, b| {
            let a_score = category_score(a.category.as_deref());
            let b_score = category_score(b.category.as_deref());
            b_score
                .cmp(&a_score)
                .then_with(|| b.created_at.cmp(&a.created_at))
        });
    };
    sort_section(&mut critical_msgs);
    sort_section(&mut important_msgs);
    sort_section(&mut fyi_msgs);

    // -------- Recently added / completed events --------
    let six_hours_ago = now - 6 * 3600;
    let recent_events = state
        .ontology_repository
        .get_recently_created_events(user_id, six_hours_ago)
        .unwrap_or_default();
    let completed_events = state
        .ontology_repository
        .get_recently_completed_events(user_id, six_hours_ago)
        .unwrap_or_default();

    // -------- Build digest text --------
    // Format philosophy: this is an interest grabber, not a full data dump.
    // Show counts + headlines so the user knows what's waiting; they can reply
    // via SMS to ask for the full content of any item.
    let mut digest_parts: Vec<String> = Vec::new();

    // Today's events: inline list with times. Cap at 5 (overflow → "+N more").
    if !today_events.is_empty() {
        let total_today = today_events.len();
        let parts: Vec<String> = today_events
            .iter()
            .take(5)
            .map(|event| {
                let time_str = if let Some(due) = event.due_at {
                    let local_due = due as i64 + tz_offset_secs as i64;
                    let h = ((local_due % 86400 + 86400) % 86400 / 3600) as i32;
                    let m = (((local_due % 86400 + 86400) % 86400) % 3600 / 60) as i32;
                    format!("{:02}:{:02} {}", h, m, event.description)
                } else {
                    event.description.clone()
                };
                time_str
            })
            .collect();
        let mut today_line = format!("Today: {}", parts.join(", "));
        if total_today > 5 {
            today_line.push_str(&format!(" +{} more", total_today - 5));
        }
        digest_parts.push(today_line);
    }

    // High-signal sections: per-item teasers
    if let Some(block) = render_high_signal_section("Critical", &critical_msgs) {
        digest_parts.push(block);
    }
    if let Some(block) = render_high_signal_section("Important", &important_msgs) {
        digest_parts.push(block);
    }

    // FYI: collapsed to a single inline sender list
    if let Some(line) = render_fyi_inline(&fyi_msgs) {
        digest_parts.push(line);
    }

    // Recently added tracked items: inline list, no per-item due dates (teaser)
    if !recent_events.is_empty() {
        let total = recent_events.len();
        let names: Vec<String> = recent_events
            .iter()
            .take(NAMES_LIST_CAP)
            .map(|e| e.description.clone())
            .collect();
        let mut line = format!("New: {}", names.join(", "));
        if total > NAMES_LIST_CAP {
            line.push_str(&format!(" +{} more", total - NAMES_LIST_CAP));
        }
        digest_parts.push(line);
    }

    // Just done: inline list
    if !completed_events.is_empty() {
        let total = completed_events.len();
        let names: Vec<String> = completed_events
            .iter()
            .take(NAMES_LIST_CAP)
            .map(|e| e.description.clone())
            .collect();
        let mut line = format!("Done: {}", names.join(", "));
        if total > NAMES_LIST_CAP {
            line.push_str(&format!(" +{} more", total - NAMES_LIST_CAP));
        }
        digest_parts.push(line);
    }

    if digest_parts.is_empty() {
        return None;
    }

    // Mark all displayed/counted messages as delivered (so they don't reappear
    // in the next digest). We mark the FULL list, not just the rendered top —
    // otherwise overflow items would loop.
    let mut message_ids: Vec<i64> = Vec::new();
    for m in critical_msgs.iter() {
        message_ids.push(m.id);
    }
    for m in important_msgs.iter() {
        message_ids.push(m.id);
    }
    for m in fyi_msgs.iter() {
        message_ids.push(m.id);
    }

    let total_messages = critical_msgs.len() + important_msgs.len() + fyi_msgs.len();
    let total_today = today_events.len();
    let header = match (total_messages, total_today) {
        (0, t) => format!("{} today", t),
        (m, 0) => format!("{} msg{}", m, if m == 1 { "" } else { "s" }),
        (m, t) => format!("{} msg{}, {} today", m, if m == 1 { "" } else { "s" }, t),
    };

    // CTA: nudge the user to reply for full content of any item.
    let footer = "Reply to dig in.";

    let digest_text = format!("{}\n\n{}\n\n{}", header, digest_parts.join("\n\n"), footer);
    Some((digest_text, message_ids))
}

/// Deliver smart digests: check which users have pending unhandled messages
/// and deliver a sectioned recap at the user's local digest window.
async fn deliver_smart_digests(state: &Arc<AppState>) {
    let users = match state.ontology_repository.get_users_with_pending_digests() {
        Ok(u) => u,
        Err(e) => {
            error!("Failed to get users with pending digests: {}", e);
            return;
        }
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i32;

    for user_id in users {
        // Check if user has digests enabled
        let settings = match state.user_core.get_user_settings(user_id) {
            Ok(s) => s,
            Err(_) => continue,
        };
        if !settings.digest_enabled {
            continue;
        }

        // Get user timezone
        let tz_offset_secs: i32 = match state.user_core.get_user_info(user_id) {
            Ok(info) => {
                let tz_name = info.timezone.as_deref().unwrap_or("UTC");
                tz_name
                    .parse::<chrono_tz::Tz>()
                    .ok()
                    .map(|tz| {
                        chrono::Utc::now()
                            .with_timezone(&tz)
                            .offset()
                            .fix()
                            .local_minus_utc()
                    })
                    .unwrap_or(0)
            }
            Err(_) => 0,
        };

        let local_ts = now as i64 + tz_offset_secs as i64;
        let current_minute_of_day = (((local_ts % 86400 + 86400) % 86400) / 60) as u16;
        // Snap current time to a 10-minute slot — matches the cron's */10 cadence
        let current_slot = (current_minute_of_day / 10) * 10;
        let current_hour = (current_minute_of_day / 60) as usize;

        // Determine if we're in a delivery window:
        // - Manual mode: exact 10-min slot match against user's configured times
        // - Auto mode: predicted wake hour (or default 8 AM), 2-hour fuzzy window
        let in_target_window = if let Some(ref time_str) = settings.digest_time {
            match parse_digest_times(time_str) {
                Ok((_, slots)) => should_deliver_now(&slots, current_minute_of_day),
                Err(e) => {
                    debug!(
                        "User {} has malformed digest_time '{}': {} — skipping",
                        user_id, time_str, e
                    );
                    false
                }
            }
        } else {
            let predicted = state
                .ontology_repository
                .compute_user_wake_hour(user_id, tz_offset_secs);
            let target_hours: Vec<usize> = predicted.into_iter().collect();
            let target_hours = if target_hours.is_empty() {
                vec![8] // default 8 AM
            } else {
                target_hours
            };
            target_hours
                .iter()
                .any(|&h| current_hour == h || current_hour == (h + 1) % 24)
        };

        // Cooldown: don't send more than one digest per N seconds.
        // Manual mode uses 9 min — just under the 10-min cron cadence — so consecutive
        // 10-min slots like 08:50, 09:00 can both fire while still preventing
        // double-fires from cron drift within the same slot.
        let cooldown_secs = if settings.digest_time.is_some() {
            540 // 9 min for manual
        } else {
            10800 // 3h for auto
        };
        if let Some(last_sent) = state.digest_cooldowns.get(&user_id) {
            if now - *last_sent < cooldown_secs {
                continue;
            }
        }

        // Build the digest payload (sections, filters, formatting).
        let (digest_text, message_ids) =
            match build_digest_for_user(state, user_id, &settings, now, tz_offset_secs).await {
                Some(d) => d,
                None => continue, // nothing to send (empty after filters)
            };

        // Window check: only fire if we're in the user's delivery window. Skip
        // the staleness fallback now that the digest is built — anything in the
        // sections is at most 24h old already.
        if !in_target_window {
            continue;
        }

        tracing::info!(
            "Built digest for user {} (msgs={}, slot={:02}:{:02}):\n{}",
            user_id,
            message_ids.len(),
            current_slot / 60,
            current_slot % 60,
            digest_text
        );

        crate::proactive::utils::send_notification(
            state,
            user_id,
            &digest_text,
            "digest".to_string(),
            None,
        )
        .await;

        // Record cooldown and mark as delivered
        state.digest_cooldowns.insert(user_id, now);
        if !message_ids.is_empty() {
            if let Err(e) = state
                .ontology_repository
                .mark_digest_delivered(&message_ids, now)
            {
                error!(
                    "Failed to mark digest messages as delivered for user {}: {}",
                    user_id, e
                );
            }
        }

        debug!(
            "Delivered digest to user {} (msgs={}, slot={:02}:{:02}, mode={})",
            user_id,
            message_ids.len(),
            current_slot / 60,
            current_slot % 60,
            if settings.digest_time.is_some() {
                "manual"
            } else {
                "auto"
            }
        );
    }
}
