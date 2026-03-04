use crate::AppState;
use crate::UserCoreOps;

use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{debug, error};

use crate::handlers::imap_handlers;

// ---------------------------------------------------------------------------
// Migration summary helpers (pure functions, testable from integration tests)
// ---------------------------------------------------------------------------

/// Map comma-separated legacy source names to the fetch tag format.
/// "whatsapp", "telegram", "signal" all map to "chat" (deduped).
/// "email" stays "email".
/// "items" is always appended.
pub fn map_sources_to_fetch(sources: &str) -> String {
    let mut fetch = Vec::new();
    let mut has_chat = false;
    for src in sources.split(',').map(|s| s.trim().to_lowercase()) {
        match src.as_str() {
            "whatsapp" | "telegram" | "signal" => {
                if !has_chat {
                    fetch.push("chat".to_string());
                    has_chat = true;
                }
            }
            "email" => {
                if !fetch.contains(&src) {
                    fetch.push(src);
                }
            }
            other if !other.is_empty() => {
                if !fetch.contains(&other.to_string()) {
                    fetch.push(other.to_string());
                }
            }
            _ => {}
        }
    }
    if !fetch.contains(&"items".to_string()) {
        fetch.push("items".to_string());
    }
    fetch.join(",")
}

/// Map a weekday number (0=Sun or 1=Mon depending on legacy format) to a name.
/// The legacy format uses 0=Sunday, 1=Monday, ..., 6=Saturday.
pub fn weekday_number_to_name(n: u32) -> Option<&'static str> {
    match n {
        0 => Some("Sunday"),
        1 => Some("Monday"),
        2 => Some("Tuesday"),
        3 => Some("Wednesday"),
        4 => Some("Thursday"),
        5 => Some("Friday"),
        6 => Some("Saturday"),
        _ => None,
    }
}

/// Build a tagged summary for a digest migration item.
/// Returns (summary_string, priority).
pub fn build_digest_migration_summary(
    time: &str,
    notification_type: Option<&str>,
    sources: Option<&str>,
) -> (String, i32) {
    let hour_min = normalize_time(time);
    let (notify_tag, priority) = match notification_type {
        Some("call") => ("call", 2),
        _ => ("sms", 1),
    };
    let fetch = match sources {
        Some(s) => map_sources_to_fetch(s),
        None => "email,chat,items".to_string(),
    };
    let tags = format!(
        "[type:recurring] [notify:{}] [repeat:daily {}] [fetch:{}]",
        notify_tag, hour_min, fetch
    );
    let description = "Summarize recent emails, messages, and tracked items for the user.";
    (format!("{}\n{}", tags, description), priority)
}

/// Build a tagged summary for a digest task (from tasks table).
/// Returns (summary, priority).
pub fn build_digest_task_summary(
    notification_type: Option<&str>,
    recurrence_time: Option<&str>,
    sources: Option<&str>,
) -> (String, i32) {
    let time = recurrence_time.unwrap_or("08:00");
    build_digest_migration_summary(time, notification_type, sources)
}

/// Build a tagged summary for a tracking task (from tasks table).
/// Returns (summary, priority).
pub fn build_tracking_task_summary(
    trigger: &str,
    condition: Option<&str>,
    notification_type: Option<&str>,
) -> (String, i32) {
    let platform = if trigger.starts_with("recurring_email") {
        "email"
    } else {
        "chat"
    };
    let fetch = if platform == "email" { "email" } else { "chat" };
    let (notify_tag, priority) = match notification_type {
        Some("call") => ("call", 2),
        _ => ("sms", 1),
    };
    let condition_text = condition.unwrap_or("any matching content");
    let tags = format!(
        "[type:tracking] [notify:{}] [fetch:{}] [platform:{}] [sender:any] [topic:{}]",
        notify_tag, fetch, platform, condition_text
    );
    (format!("{}\n{}", tags, condition_text), priority)
}

/// Build a tagged summary for a recurring (non-digest) task.
/// Returns a Vec of (summary, priority) - multiple items if weekly with multiple days.
pub fn build_recurring_task_summary(
    action: &str,
    recurrence_rule: Option<&str>,
    recurrence_time: Option<&str>,
    notification_type: Option<&str>,
) -> Vec<(String, i32)> {
    let rule = recurrence_rule.unwrap_or("daily");
    let time = normalize_time(recurrence_time.unwrap_or("08:00"));
    let (notify_tag, priority) = match notification_type {
        Some("call") => ("call", 2),
        _ => ("sms", 1),
    };

    if rule == "daily" {
        let tags = format!(
            "[type:recurring] [notify:{}] [repeat:daily {}]",
            notify_tag, time
        );
        return vec![(format!("{}\n{}", tags, action), priority)];
    }

    if rule.starts_with("weekly:") {
        let days_str = rule.trim_start_matches("weekly:");
        let day_nums: Vec<u32> = days_str
            .split(',')
            .filter_map(|d| d.trim().parse().ok())
            .collect();

        // Check for weekdays (Mon-Fri = 1,2,3,4,5)
        if day_nums.len() == 5 && (1..=5).all(|d| day_nums.contains(&d)) {
            let tags = format!(
                "[type:recurring] [notify:{}] [repeat:weekdays {}]",
                notify_tag, time
            );
            return vec![(format!("{}\n{}", tags, action), priority)];
        }

        // Otherwise create one item per day
        return day_nums
            .iter()
            .filter_map(|&d| {
                weekday_number_to_name(d).map(|name| {
                    let tags = format!(
                        "[type:recurring] [notify:{}] [repeat:weekly {} {}]",
                        notify_tag, name, time
                    );
                    (format!("{}\n{}", tags, action), priority)
                })
            })
            .collect();
    }

    // Fallback: treat as daily
    let tags = format!(
        "[type:recurring] [notify:{}] [repeat:daily {}]",
        notify_tag, time
    );
    vec![(format!("{}\n{}", tags, action), priority)]
}

/// Build a tagged summary for a one-shot reminder.
/// Returns (summary, priority).
pub fn build_oneshot_task_summary(action: &str, notification_type: Option<&str>) -> (String, i32) {
    let (notify_tag, priority) = match notification_type {
        Some("call") => ("call", 2),
        _ => ("sms", 1),
    };
    let tags = format!("[type:oneshot] [notify:{}]", notify_tag);
    (format!("{}\n{}", tags, action), priority)
}

/// Build a tagged summary for a quiet mode task.
/// Returns (summary, priority).
pub fn build_quiet_mode_summary() -> (String, i32) {
    let tags = "[type:oneshot] [notify:silent]";
    let description = "Quiet mode - suppress notifications until end time.";
    (format!("{}\n{}", tags, description), 0)
}

/// Normalize a time string to "HH:MM" format.
/// Handles inputs like "9:00" -> "09:00", "14:30" -> "14:30".
fn normalize_time(time: &str) -> String {
    let parts: Vec<&str> = time.split(':').collect();
    if parts.len() >= 2 {
        let hour: u32 = parts[0].parse().unwrap_or(8);
        let minute: u32 = parts[1].parse().unwrap_or(0);
        format!("{:02}:{:02}", hour, minute)
    } else {
        "08:00".to_string()
    }
}

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

                // Record disconnection as an item
                let current_time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i32;

                let bridge_name = match bridge.bridge_type.as_str() {
                    "whatsapp" => "WhatsApp",
                    "telegram" => "Telegram",
                    "signal" => "Signal",
                    other => other,
                };

                let new_item = crate::pg_models::NewPgItem {
                    user_id,
                    summary: format!("System: {} bridge disconnected.", bridge_name),
                    due_at: None,
                    priority: 1,
                    source_id: None,
                    created_at: current_time,
                };

                if let Err(e) = state.item_repository.create_item(&new_item) {
                    error!(
                        "Failed to create item for bridge disconnection for user {}: {}",
                        user_id, e
                    );
                }

                // Also record in legacy table for backward compatibility during migration
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

                                    let contact_profiles = match state.user_repository.get_contact_profiles(user.id) {
                                        Ok(profiles) => profiles,
                                        Err(e) => {
                                            tracing::error!("Failed to get contact profiles for user {}: {}", user.id, e);
                                            Vec::new()
                                        }
                                    };
                                    // Check if user has auto_create_items enabled
                                    let auto_create_items = state.user_core.get_auto_create_items(user.id).unwrap_or(false);

                                    // Mark emails as processed and format them for importance checking
                                    let mut emails_content = String::from("New emails:\n");
                                    for email in &sorted_emails {
                                        // Auto-detect trackable items (invoices, shipments, deadlines)
                                        // Runs for every email - spawned in background to avoid blocking
                                        if auto_create_items {
                                            let state_clone = state.clone();
                                            let user_id = user.id;
                                            let email_uid = email.id.clone();
                                            let trackable_content = format!(
                                                "From: {}\nSubject: {}\nDate: {}\nBody: {}",
                                                email.from.as_deref().unwrap_or("Unknown"),
                                                email.subject.as_deref().unwrap_or("No subject"),
                                                email.date_formatted.as_deref().unwrap_or("Unknown date"),
                                                email.body.as_deref().unwrap_or("No content")
                                            );
                                            tokio::spawn(async move {
                                                if let Err(e) = crate::proactive::utils::check_trackable_items(
                                                    &state_clone,
                                                    user_id,
                                                    &email_uid,
                                                    &trackable_content,
                                                ).await {
                                                    tracing::debug!("Trackable item check failed for email {}: {}", email_uid, e);
                                                }
                                            });
                                        }

                                        // Check if sender matches contact profiles with "all" notification mode
                                        if let Some(matched_profile) = contact_profiles.iter().filter(|p| p.notification_mode == "all").find(|profile| {
                                            if let Some(ref emails) = profile.email_addresses {
                                                let from_lower = email.from_email.as_deref().unwrap_or("").to_lowercase();
                                                let from_name_lower = email.from.as_deref().unwrap_or("").to_lowercase();
                                                emails.split(',').any(|addr| {
                                                    let addr_lower = addr.trim().to_lowercase();
                                                    from_lower.contains(&addr_lower) || from_name_lower.contains(&addr_lower)
                                                })
                                            } else {
                                                false
                                            }
                                        }) {
                                            tracing::info!("Fast check: Contact profile matched for user {}", user.id);

                                            // Determine suffix based on notification_type
                                            let suffix = match matched_profile.notification_type.as_str() {
                                                "call" => "_call",
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
                                            "Platform: email\nFrom: {}\nChat: inbox\nSubject: {}\nContent: {}",
                                            email.from.as_deref().unwrap_or("Unknown"),
                                            email.subject.as_deref().unwrap_or("No subject"),
                                            email.body.as_deref().unwrap_or("No content")
                                        );

                                        // Check tracking items with email fetch for email matches
                                        let tracking_items = state.item_repository.get_tracking_items(user.id).unwrap_or_default();
                                        let now_ts = std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .unwrap()
                                            .as_secs() as i32;
                                        let email_tracking: Vec<_> = tracking_items.into_iter().filter(|item| {
                                            let tags = crate::proactive::utils::parse_summary_tags(&item.summary);
                                            tags.fetch.contains(&"email".to_string())
                                                && item.due_at.is_some_and(|d| d > now_ts)
                                        }).collect();
                                        if !email_tracking.is_empty() {
                                            // Extract data from Result immediately to drop non-Send Box<dyn Error>
                                            let maybe_match: Option<crate::pg_models::PgItem> =
                                                crate::proactive::utils::check_item_monitor_match(
                                                    &state,
                                                    user.id,
                                                    &email_content,
                                                    &email_tracking,
                                                ).await.ok().flatten().and_then(|resp| {
                                                    let item_id = resp.task_id.unwrap_or(0);
                                                    email_tracking.iter().find(|i| i.id == item_id).cloned()
                                                });
                                            if let Some(matched_item) = maybe_match {
                                                let item_id = matched_item.id;
                                                let priority = matched_item.priority;
                                                let result: Result<crate::proactive::utils::TriggeredItemResult, String> =
                                                    crate::proactive::utils::process_triggered_item(
                                                        &state, user.id, &matched_item, Some(&email_content),
                                                    ).await.map_err(|e| e.to_string());
                                                match result {
                                                    Ok(response) => {
                                                        crate::proactive::utils::handle_triggered_item_result(
                                                            &state, user.id, item_id, priority, &response,
                                                        ).await;
                                                    }
                                                    Err(e) => {
                                                        tracing::error!("Failed to process tracking match for item {}: {}", item_id, e);
                                                        crate::proactive::utils::send_notification(
                                                            &state, user.id, &matched_item.summary,
                                                            "item_sms".to_string(),
                                                            Some("Hey, you have a notification!".to_string()),
                                                        ).await;
                                                    }
                                                }
                                                continue;
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
                                    match crate::proactive::utils::check_message_importance(&state, user.id, &emails_content, "", "", "", None, "").await {
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

                        // Auto-resolve email tracking items for emails the user has read
                        {
                            let state_clone = state.clone();
                            let user_id = user.id;
                            tokio::spawn(async move {
                                if let Err(e) = crate::proactive::utils::resolve_read_email_items(
                                    &state_clone, user_id
                                ).await {
                                    tracing::debug!("Email tracking item resolve failed for user {}: {}", user_id, e);
                                }
                            });
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

    // Triggered items - runs every minute to process items whose due_at has passed
    let state_clone = Arc::clone(&state);
    let triggered_items_job = Job::new_async("0 */1 * * * *", move |_, _| {
        let state = state_clone.clone();
        Box::pin(async move {
            debug!("Checking for triggered items...");
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i32;

            match state.item_repository.get_triggered_items(now) {
                Ok(items) => {
                    debug!("Found {} triggered items", items.len());
                    for item in items {
                        let item_id = item.id;
                        let user_id = item.user_id;
                        let state = state.clone();
                        let item_clone = item.clone();

                        // Mark as running BEFORE spawning to prevent duplicate execution
                        // on the next cron tick (set due_at far in the future temporarily)
                        let _ = state
                            .item_repository
                            .update_due_at(item_id, Some(now + 86400));

                        tokio::spawn(async move {
                            debug!(
                                "Processing triggered item {} for user {}: {}",
                                item_id, user_id, item_clone.summary
                            );

                            // Unified path for all items: time fired, no matched message
                            let summary = item_clone.summary.clone();
                            let priority = item_clone.priority;

                            let result = crate::proactive::utils::process_triggered_item(
                                &state,
                                user_id,
                                &item_clone,
                                None,
                            )
                            .await
                            .map_err(|e| e.to_string());

                            match result {
                                Ok(response) => {
                                    crate::proactive::utils::handle_triggered_item_result(
                                        &state, user_id, item_id, priority, &response,
                                    )
                                    .await;
                                }
                                Err(e) => {
                                    error!("Failed to process triggered item {}: {}", item_id, e);
                                    // Fallback: send summary as SMS, delete item
                                    crate::proactive::utils::send_notification(
                                        &state,
                                        user_id,
                                        &summary,
                                        "item_sms".to_string(),
                                        Some("Hey, you have a reminder!".to_string()),
                                    )
                                    .await;
                                    let _ = state.item_repository.delete_item(item_id, user_id);
                                }
                            }
                        });
                    }
                }
                Err(e) => error!("Failed to get triggered items: {}", e),
            }
        })
    })
    .expect("Failed to create triggered items job");

    sched
        .add(triggered_items_job)
        .await
        .expect("Failed to add triggered items job to scheduler");

    // Tracking interval job - runs every hour to process tracking items with internet/weather/items fetch
    let state_clone = Arc::clone(&state);
    let tracking_interval_job = Job::new_async("0 0 */1 * * *", move |_, _| {
        let state = state_clone.clone();
        Box::pin(async move {
            debug!("Running hourly tracking interval check...");
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i32;

            // Get all users with auto features
            let users = match state.user_core.get_all_users() {
                Ok(users) => users,
                Err(e) => {
                    error!("Failed to get users for tracking interval: {}", e);
                    return;
                }
            };

            for user in &users {
                let user_plan = state.user_repository.get_plan_type(user.id).unwrap_or(None);
                if !crate::utils::plan_features::has_auto_features(user_plan.as_deref()) {
                    continue;
                }

                let tracking_items = state
                    .item_repository
                    .get_tracking_items(user.id)
                    .unwrap_or_default();
                // Filter to items with non-email/chat fetch sources that haven't expired,
                // OR email/chat-only items within 2 days of their deadline (pre-deadline check)
                let interval_items: Vec<_> = tracking_items
                    .into_iter()
                    .filter(|item| {
                        let tags = crate::proactive::utils::parse_summary_tags(&item.summary);
                        let has_interval_fetch = tags
                            .fetch
                            .iter()
                            .any(|f| matches!(f.as_str(), "internet" | "weather" | "items"));
                        let only_realtime = tags
                            .fetch
                            .iter()
                            .all(|f| matches!(f.as_str(), "email" | "chat"));
                        let near_deadline =
                            item.due_at.is_some_and(|d| d > now && d - now <= 2 * 86400);

                        // Include: interval fetch items (not expired), OR email/chat items near deadline
                        (has_interval_fetch
                            && !only_realtime
                            && item.due_at.is_some_and(|d| d > now))
                            || (only_realtime && near_deadline)
                    })
                    .collect();

                for item in interval_items {
                    let item_id = item.id;
                    let user_id = item.user_id;
                    let priority = item.priority;
                    let state = state.clone();

                    tokio::spawn(async move {
                        let result = crate::proactive::utils::process_triggered_item(
                            &state, user_id, &item, None,
                        )
                        .await
                        .map_err(|e| e.to_string());

                        match result {
                            Ok(response) => {
                                crate::proactive::utils::handle_triggered_item_result(
                                    &state, user_id, item_id, priority, &response,
                                )
                                .await;
                            }
                            Err(e) => {
                                tracing::error!(
                                    "Failed to process tracking interval item {}: {}",
                                    item_id,
                                    e
                                );
                            }
                        }
                    });
                }
            }
        })
    })
    .expect("Failed to create tracking interval job");

    sched
        .add(tracking_interval_job)
        .await
        .expect("Failed to add tracking interval job to scheduler");

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

            // Clean up old items (30 days past due_at)
            let item_cutoff = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i32
                - (30 * 24 * 60 * 60); // 30 days ago

            match state.item_repository.delete_old_items(item_cutoff) {
                Ok(count) => {
                    if count > 0 {
                        debug!("Cleaned up {} old items", count);
                    }
                }
                Err(e) => error!("Failed to cleanup old items: {}", e),
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

            // Auto-expire tracking items with due_at >7 days past
            let cleanup_now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i32;

            match state
                .item_repository
                .delete_expired_tracking_items(cleanup_now)
            {
                Ok(count) => {
                    if count > 0 {
                        debug!("Auto-expired {} stale tracking items", count);
                    }
                }
                Err(e) => error!("Failed to auto-expire tracking items: {}", e),
            }

            // Expire stale "ongoing" call records (no webhook received after 1 hour)
            let call_cutoff = cleanup_now - 3600;
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

    // Start the scheduler
    sched.start().await.expect("Failed to start scheduler");

    // TODO we should add another scheduled call that just checks if there are items that are 'done' or not found in the elevenlabs
    // but are still 'ongoing' in our db. we don't want to be accidentally charging users.
    // and if that happens make error visible
}
