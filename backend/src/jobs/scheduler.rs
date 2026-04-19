use crate::AppState;
use crate::UserCoreOps;

use chrono::Offset;
use futures_util::TryStreamExt;
use std::sync::Arc;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{debug, error};

use crate::handlers::imap_handlers;

/// Concurrent cold-builds allowed at once during a reconciler tick.
///
/// Each cold build makes ~2-3 HTTP calls to Tuwunel (whoami + first sync,
/// plus one extra sync_once for E2EE users). A semaphore of size N caps
/// in-flight builds to N regardless of user count - 229 users, 2000
/// users, or 50000 users all put the same instantaneous load on Tuwunel.
///
/// N=5 is the conservative phase-1 starting point. Raise after measuring
/// Tuwunel's p99 sync latency and 4xx/5xx rate under load. Do NOT raise
/// without measuring; guessing is what got us into thundering-herd
/// problems before.
const RECONCILE_CONCURRENCY: usize = 5;

/// Max age of a sync heartbeat (`UserMatrixState::last_sync_at`) before
/// the reconciler treats the sync loop as zombied and forces a rebuild.
/// Mirrors `matrix_auth::SYNC_STALE_AFTER`; kept here as a separate
/// constant only to make the reconciler's loop condition self-documenting.
/// The reconciler's own tick interval should be <= this.
const RECONCILE_TICK_INTERVAL_SECS: u64 = 60;

/// Idempotent reconciler for Matrix client lifecycle. Called once at boot
/// (in a background task) and then on a 60s cron. Does three things under
/// a single top-level mutex so ticks can't overlap and stampede Tuwunel:
///
/// 1. Tear down any cell whose user no longer has a connected bridge.
/// 2. Walk the DB-truth list of active-bridge users, ordered by most-recent
///    bridge activity first (so warm users come up before cold ones).
/// 3. For each, acquire a permit from `RECONCILE_CONCURRENCY` and call
///    `ensure_matrix_user_running`. Live users fast-path under the
///    per-user mutex; dead/zombie/missing users cold-rebuild.
///
/// Non-reentrant via `state.matrix_reconcile_lock.try_lock()`. If a
/// previous tick is still running (e.g. first tick on 5000 users at
/// N=5 takes ~30 min), the new tick no-ops and lets the old one finish.
///
/// Per-user serialization still comes from the per-user cell mutex
/// inside `ensure_matrix_user_running`: a handler call and a reconciler
/// call for the same user can race, but the per-user mutex makes them
/// sequential, and the heartbeat fast-path means whichever goes second
/// sees a live client and no-ops. Safe without further coordination.
pub async fn reconcile_matrix_users(state: Arc<AppState>) {
    let _guard = match state.matrix_reconcile_lock.try_lock() {
        Ok(g) => g,
        Err(_) => {
            tracing::debug!("Matrix reconciler tick skipped - previous tick still running");
            return;
        }
    };

    let active_users = match state.user_repository.get_active_bridge_users_prioritized() {
        Ok(u) => u,
        Err(e) => {
            error!("Reconciler: failed to fetch active bridge users: {}", e);
            return;
        }
    };
    let active_set: std::collections::HashSet<i32> = active_users.iter().copied().collect();

    // 1. Tear down stale cells (users with no remaining connected bridges).
    //    We use `stop_matrix_user_if_no_bridges` which does its own
    //    has-bridges check as a double-safety against a racing
    //    create-bridge that slipped in after our DB snapshot.
    let stale_users: Vec<i32> = state
        .matrix_users
        .iter()
        .filter(|e| !active_set.contains(e.key()))
        .map(|e| *e.key())
        .collect();
    for uid in stale_users {
        if let Err(e) = crate::utils::matrix_auth::stop_matrix_user_if_no_bridges(uid, &state).await
        {
            tracing::warn!("Reconciler: stop failed for user {}: {}", uid, e);
        }
    }

    // 2. Ensure each active user with bounded concurrency. spawn + owned
    //    permit pattern lets us run N builds concurrently while the outer
    //    loop itself doesn't block on any of them.
    let total = active_users.len();
    tracing::info!(
        "Reconciler: ensuring {} active-bridge users (concurrency={})",
        total,
        RECONCILE_CONCURRENCY
    );

    let semaphore = Arc::new(tokio::sync::Semaphore::new(RECONCILE_CONCURRENCY));
    let mut handles = Vec::with_capacity(total);
    for user_id in active_users {
        let permit = match Arc::clone(&semaphore).acquire_owned().await {
            Ok(p) => p,
            Err(_) => {
                tracing::error!("Reconciler: semaphore closed unexpectedly");
                break;
            }
        };
        let state = Arc::clone(&state);
        handles.push(tokio::spawn(async move {
            let result =
                crate::utils::matrix_auth::ensure_matrix_user_running(user_id, &state).await;
            if let Err(e) = result {
                tracing::warn!(
                    "Reconciler: ensure_matrix_user_running failed for user {}: {}",
                    user_id,
                    e
                );
            }
            drop(permit);
        }));
    }

    // Wait for the whole batch. If a build hangs forever (shouldn't -
    // build_matrix_client has network timeouts), the reconcile lock keeps
    // the next tick from overlapping.
    for h in handles {
        let _ = h.await;
    }

    tracing::info!("Reconciler: tick complete ({} users processed)", total);
}

/// Checks all bridges for all users and deletes any that are confirmed
/// disconnected. Inconclusive probes (bot slow, unknown status, transient
/// matrix error) are left alone - we'd rather wait one more tick than
/// tear down a working bridge on a bad signal.
///
/// Verified disconnect signals per bridge:
/// - whatsapp / signal (bridgev2): `list-logins` returns a row with status
///   `BAD_CREDENTIALS` and no `CONNECTED` row. This is emitted when the
///   upstream service invalidates the session - e.g. user unlinks the
///   lightfriend device from their WhatsApp mobile app's Linked Devices.
///   Verified on mautrix-whatsapp v26.04 on 2026-04-19.
/// - telegram (v0.15.3): matrix-side room enumeration error. Older bridge
///   with no list-logins equivalent.
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
            match probe_bridge_login_health(state, user_id, &bridge).await {
                Some(true) => {
                    tracing::debug!(
                        "Bridge health: {} for user {} healthy",
                        bridge.bridge_type,
                        user_id
                    );
                }
                Some(false) => {
                    tracing::info!(
                        "Bridge health: {} for user {} confirmed disconnected, deleting",
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
                            "Failed to delete disconnected bridge {} for user {}: {}",
                            bridge.bridge_type, user_id, e
                        );
                    }
                }
                None => {
                    // Inconclusive: probe error, bot timed out, unknown
                    // status token. Leave bridge in place, try again next
                    // tick. We never delete on None - that would thrash
                    // on any transient matrix flake.
                    tracing::debug!(
                        "Bridge health: {} for user {} inconclusive, leaving in place",
                        bridge.bridge_type,
                        user_id
                    );
                }
            }
        }

        // Clean up Matrix client if no bridges left (no-op otherwise).
        let _ = crate::utils::matrix_auth::stop_matrix_user_if_no_bridges(user_id, state).await;
    }

    debug!("Bridge health check completed");
    Ok(())
}

/// Probe the bridge bot and classify the live login health.
///
/// For bridgev2 bridges (whatsapp, signal) the signal we rely on is the
/// `list-logins` response status token:
///
/// - `CONNECTED` -> healthy
/// - `BAD_CREDENTIALS` -> confirmed disconnect (upstream service revoked
///   the session, e.g. user unlinked from phone side)
/// - `Empty`/`Unknown` -> DO NOT mark unhealthy. The bridge bot might be
///   slow, or the bridge version might emit something we don't recognise;
///   either way we'd rather keep the bridge and retry next tick than
///   wrongly tear down a working connection.
///
/// For telegram (still on the older mautrix-telegram v0.15.3, pre-bridgev2)
/// we keep the previous behaviour: check Matrix client + room enumeration.
/// There's no list-logins command on that bridge.
///
/// Return value:
///
/// - `Some(true)` = confirmed healthy. Leave bridge alone.
/// - `Some(false)` = confirmed disconnect. Caller should delete bridge.
/// - `None` = inconclusive (probe error, empty, unknown status). Caller
///   should log and leave bridge in place until the next tick.
async fn probe_bridge_login_health(
    state: &Arc<AppState>,
    user_id: i32,
    bridge: &crate::pg_models::PgBridge,
) -> Option<bool> {
    use crate::utils::bridge_responses::{classify_bridgev2_list_logins, BridgeLoginHealth};
    use matrix_sdk::ruma::{OwnedRoomId, OwnedUserId};

    let client = match crate::utils::matrix_auth::get_cached_client(user_id, state).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(
                "Bridge health: failed to get Matrix client for user {}: {}",
                user_id,
                e
            );
            return None;
        }
    };

    // Telegram: older python bridge, no bridgev2 contract. Fall back to the
    // old "can we reach rooms" check. Not a perfect disconnect signal but
    // better than nothing for telegram users.
    if bridge.bridge_type == "telegram" {
        return match crate::utils::bridge::get_service_rooms(&client, &bridge.bridge_type).await {
            Ok(_) => Some(true),
            Err(e) => {
                tracing::warn!(
                    "Bridge health: telegram room fetch failed for user {}: {}",
                    user_id,
                    e
                );
                None
            }
        };
    }

    // Bridgev2 path: need the management room to talk to the bot. We stored
    // it on the bridge record when the connection completed.
    let (room_id, bot_user_id, cmd_prefix) = match bridge.bridge_type.as_str() {
        "whatsapp" => (bridge.room_id.as_deref(), "whatsappbot", "!wa"),
        "signal" => (bridge.room_id.as_deref(), "signalbot", "!signal"),
        other => {
            tracing::warn!(
                "Bridge health: unknown bridge_type {:?} for user {} - skipping",
                other,
                user_id
            );
            return None;
        }
    };

    let Some(room_id_str) = room_id else {
        tracing::warn!(
            "Bridge health: {} bridge for user {} has no room_id - cannot probe",
            bridge.bridge_type,
            user_id
        );
        return None;
    };

    // Resolve matrix bot user id from the bridge bot localpart + homeserver
    // domain. The homeserver is derived from the user's Matrix user_id.
    let Some(matrix_user_id) = client.user_id() else {
        tracing::warn!(
            "Bridge health: client for user {} has no matrix user_id",
            user_id
        );
        return None;
    };
    let domain = matrix_user_id.server_name().as_str();
    let bot_full = format!("@{}:{}", bot_user_id, domain);
    let bot_owned = match OwnedUserId::try_from(bot_full.as_str()) {
        Ok(b) => b,
        Err(e) => {
            tracing::warn!("Bridge health: invalid bot user id {}: {}", bot_full, e);
            return None;
        }
    };

    let room_owned = match OwnedRoomId::try_from(room_id_str) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Bridge health: invalid room id {}: {}", room_id_str, e);
            return None;
        }
    };

    let room = match client.get_room(&room_owned) {
        Some(r) => r,
        None => {
            tracing::warn!(
                "Bridge health: management room {} not in client state for user {}",
                room_id_str,
                user_id
            );
            return None;
        }
    };

    let list_cmd = format!("{} list-logins", cmd_prefix);
    let responses = match crate::utils::bridge::probe_bridge_room(
        &client,
        &room,
        &bot_owned,
        &list_cmd,
        std::time::Duration::from_secs(8),
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(
                "Bridge health: {} list-logins probe failed for user {}: {}",
                bridge.bridge_type,
                user_id,
                e
            );
            return None;
        }
    };

    let combined = responses.join("\n");
    let health = classify_bridgev2_list_logins(&combined);
    tracing::info!(
        "Bridge health: user {} {} -> {:?} (body: {:?})",
        user_id,
        bridge.bridge_type,
        health,
        combined
    );

    match health {
        BridgeLoginHealth::Connected => Some(true),
        BridgeLoginHealth::BadCredentials => Some(false),
        BridgeLoginHealth::Empty | BridgeLoginHealth::Unknown => None,
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
    // Matrix clients are brought up by the reconciler. Kick the first
    // tick in a background task so we don't block boot on warming N users
    // at a time through the semaphore (could be many minutes at scale).
    // Handlers that need a client for a user the first tick hasn't
    // reached yet will call `ensure_matrix_user_running` directly, which
    // cold-builds on demand under the per-user mutex - the reconciler is
    // the backstop, not the only path.
    tracing::info!("Spawning initial Matrix reconciler tick (background)");
    let state_for_first_tick = Arc::clone(&state);
    tokio::spawn(async move {
        reconcile_matrix_users(state_for_first_tick).await;
    });

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
                                            processed_emails.sort_by_key(|b| std::cmp::Reverse(b.processed_at));

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

                                    // Cache persons once per batch (free perf win).
                                    let persons = state
                                        .ontology_repository
                                        .get_persons_with_channels(user.id, 500, 0)
                                        .unwrap_or_default();

                                    for email in &sorted_emails {
                                        let uid_str = email.id.clone();
                                        match imap_handlers::insert_email_into_ontology(
                                            &state,
                                            user.id,
                                            email,
                                            &persons,
                                        )
                                        .await
                                        {
                                            Ok(_) => {
                                                // Mark as processed ONLY after successful ontology
                                                // insertion. A failed insert stays unprocessed so
                                                // the next run retries it. This fixes the old
                                                // mark-before-insert ordering bug.
                                                if let Err(e) = state.user_repository.mark_email_as_processed(
                                                    user.id,
                                                    &uid_str,
                                                    None,
                                                ) {
                                                    error!(
                                                        "Failed to mark email {} as processed for user {}: {}",
                                                        uid_str, user.id, e
                                                    );
                                                }
                                            }
                                            Err(e) => {
                                                tracing::warn!(
                                                    "Failed to insert email {} into ontology for user {} (will retry): {}",
                                                    uid_str, user.id, e
                                                );
                                            }
                                        }
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

    // Bridge health check - runs every 15 minutes at :07, :22, :37, :52.
    //
    // Cadence rationale: the WhatsApp bridge (mautrix-whatsapp v26.04) does
    // NOT emit any passive push when a user unlinks lightfriend from their
    // phone's Linked Devices - the session just goes BAD_CREDENTIALS
    // silently. Empirically verified 2026-04-19. The only detection signal
    // is our own `!wa list-logins` probe, so health-check frequency is the
    // upper bound on "how long can a user's bridge look connected but
    // actually be dead". Daily was way too slow; 15 min gives us prompt
    // detection without hammering the bot.
    //
    // Offset the minutes (:07, :22, :37, :52) to avoid overlapping with
    // the top-of-hour IMAP monitor (":00, :10, :20...") and the
    // every-minute reconciler tick (":00"). Keeps Tuwunel RPS smooth.
    let state_clone = Arc::clone(&state);
    let bridge_health_job = Job::new_async("0 7,22,37,52 * * * *", move |_, _| {
        let state = state_clone.clone();
        Box::pin(async move {
            debug!("Running bridge health check...");
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

    // Matrix reconciler - runs every 60s. Sole lifecycle driver for
    // Matrix clients: brings up missing ones, rebuilds dead/zombied ones
    // (via `last_sync_at` staleness check), tears down users whose
    // bridges are all gone. Also cleans up stale "connecting" bridges
    // in the same tick since they share the "once a minute, housekeep"
    // cadence.
    //
    // Non-reentrant: ticks that fire while a previous tick is still
    // running (common on the very first post-boot tick if there are
    // many users to warm) no-op. Per-user mutex inside ensure_matrix_
    // user_running keeps handler calls and the reconciler from racing
    // on the same user.
    // "at second 0 of every minute" = every 60s. If you change the
    // cadence, update RECONCILE_TICK_INTERVAL_SECS (used only for docs
    // and the sync-heartbeat staleness threshold).
    debug_assert_eq!(
        RECONCILE_TICK_INTERVAL_SECS, 60,
        "cron expression below is hardcoded for 60s cadence"
    );
    let state_clone = Arc::clone(&state);
    let matrix_reconcile_job = Job::new_async("0 * * * * *", move |_, _| {
        let state = state_clone.clone();
        Box::pin(async move {
            reconcile_matrix_users(Arc::clone(&state)).await;
            cleanup_stale_connecting_bridges(&state);
        })
    })
    .expect("Failed to create matrix reconcile job");

    sched
        .add(matrix_reconcile_job)
        .await
        .expect("Failed to add matrix reconcile job to scheduler");

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

    // Every 10 minutes: sync email read receipts from IMAP \Seen flags
    let state_clone = Arc::clone(&state);
    let email_read_sync_job = Job::new_async("30 */10 * * * *", move |_, _| {
        let state = state_clone.clone();
        Box::pin(async move {
            sync_email_read_receipts(&state).await;
        })
    })
    .expect("Failed to create email read receipt sync job");

    sched
        .add(email_read_sync_job)
        .await
        .expect("Failed to add email read receipt sync job to scheduler");

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
        let total_min = h * 60 + m;
        let snapped = (((total_min + 5) / 10) * 10) % 1440;
        let snapped = snapped as u16;
        if !slots.contains(&snapped) {
            slots.push(snapped);
        }
    }
    if slots.is_empty() {
        return Err("at least one time required".to_string());
    }
    if slots.len() > 10 {
        return Err(format!(
            "too many digest times: {} (max 10 per day)",
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
    slots.contains(&current_slot)
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

/// How many FYI items to show individually before collapsing to counts.
const FYI_CAP: usize = 5;

/// Render FYI section with per-item summaries, similar to high-signal sections
/// but with a higher cap since these are shorter teasers.
fn render_fyi_inline(messages: &[crate::models::ontology_models::OntMessage]) -> Option<String> {
    if messages.is_empty() {
        return None;
    }
    let total = messages.len();
    let shown: Vec<String> = messages
        .iter()
        .take(FYI_CAP)
        .map(format_digest_message)
        .collect();
    let mut block = format!("FYI:\n{}", shown.join("\n"));
    if total > FYI_CAP {
        block.push_str(&format!("\n+ {} more", total - FYI_CAP));
    }
    Some(block)
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

    // -------- Section 4: FYI (medium + low urgency) --------
    // We include "low" here too because otherwise users whose emails all get
    // classified at the non-urgent end (which is most normal traffic: bills,
    // newsletters, routine updates) would never see anything in a digest and
    // think the feature is broken. Spam is still filtered by category below.
    let mut fyi_msgs = state
        .ontology_repository
        .get_pending_messages_by_urgency(user_id, &["medium", "low"], since_window, 30)
        .unwrap_or_default();

    // -------- Filter: drop messages the user has already seen --------
    // seen_at is populated by read receipt events from bridges and user replies.
    let drop_seen = |msgs: &mut Vec<crate::models::ontology_models::OntMessage>| {
        msgs.retain(|m| m.seen_at.is_none());
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

    // Header wording: disambiguate "msgs" (unread/pending messages from
    // the last 24h) from "events today" (ontology events with a due_at
    // timestamp inside the user's local day). Previously the header
    // said just "1 today" which users read as "1 new thing today" and
    // got confused about what was being counted.
    let total_messages = critical_msgs.len() + important_msgs.len() + fyi_msgs.len();
    let total_today = today_events.len();
    let header = match (total_messages, total_today) {
        (0, t) => format!("{} event{} today", t, if t == 1 { "" } else { "s" }),
        (m, 0) => format!("{} msg{}", m, if m == 1 { "" } else { "s" }),
        (m, t) => format!(
            "{} msg{}, {} event{} today",
            m,
            if m == 1 { "" } else { "s" },
            t,
            if t == 1 { "" } else { "s" }
        ),
    };

    let digest_text = format!("{}\n\n{}", header, digest_parts.join("\n\n"));
    Some((digest_text, message_ids))
}

/// Deliver smart digests: check which users have pending unhandled messages
/// and deliver a sectioned recap at the user's local digest window.
async fn deliver_smart_digests(state: &Arc<AppState>) {
    // Iterate EVERY user with digest_enabled=true, not just users with
    // pending high/medium messages. The old `get_users_with_pending_digests`
    // gate silently dropped users whose emails all classified as
    // urgency='low' or 'none' — they would never hit their scheduled
    // digest time even though new emails were flowing in. The entry gate
    // must be "is the user opted in", not "does the user have high-urgency
    // traffic right now".
    //
    // `build_digest_for_user` already returns `None` for a truly empty
    // digest, so this change doesn't produce noise for users with no
    // activity — it just gives users with any scanned activity a real
    // shot at receiving their scheduled digest.
    let all_users = match state.user_core.get_all_users() {
        Ok(u) => u,
        Err(e) => {
            error!("Failed to get all users for digest delivery: {}", e);
            return;
        }
    };
    let users: Vec<i32> = all_users.into_iter().map(|u| u.id).collect();

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
        // Determine if we're in a delivery window:
        // - Manual mode: exact 10-min slot match against user's configured times
        // - Auto mode: 3 fixed slots at wake, wake+5h, wake+10h
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
            // Auto mode: 3 fixed slots relative to predicted wake hour.
            // Morning (wake), midday (wake+5h), evening (wake+10h).
            let predicted = state
                .ontology_repository
                .compute_user_wake_hour(user_id, tz_offset_secs);
            let wake_hour = predicted.unwrap_or(8);
            let slots: Vec<u16> = [0, 5, 10]
                .iter()
                .map(|offset| (((wake_hour + offset) % 24) * 60) as u16)
                .collect();
            should_deliver_now(&slots, current_minute_of_day)
        };

        // Cooldown: don't send more than one digest per N seconds.
        // Manual mode uses 9 min — just under the 10-min cron cadence — so consecutive
        // 10-min slots like 08:50, 09:00 can both fire while still preventing
        // double-fires from cron drift within the same slot.
        // Auto mode uses 9 min too since it now uses fixed slots like manual.
        let cooldown_secs = 540; // 9 min for both modes
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

/// Sync email read receipts: check IMAP \Seen flags for recent unseen email
/// ont_messages and mark them as seen if the user read them in their mail client.
async fn sync_email_read_receipts(state: &Arc<AppState>) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i32;
    let since = now - 48 * 3600; // check emails from last 48 hours

    let all_users = match state.user_core.get_all_users() {
        Ok(u) => u,
        Err(e) => {
            error!("Failed to get users for email read receipt sync: {}", e);
            return;
        }
    };

    for user in all_users {
        let user_id = user.id;

        let unseen = match state
            .ontology_repository
            .get_unseen_email_messages(user_id, since)
        {
            Ok(msgs) => msgs,
            Err(_) => continue,
        };
        if unseen.is_empty() {
            continue;
        }

        // Get all active IMAP connections for this user
        let creds = match state.user_repository.get_all_imap_credentials(user_id) {
            Ok(c) if !c.is_empty() => c,
            _ => continue,
        };

        // Extract UIDs from room_ids (format: "email_{uid}")
        let uid_msg_pairs: Vec<(String, i64)> = unseen
            .iter()
            .filter_map(|m| {
                m.room_id
                    .strip_prefix("email_")
                    .map(|uid| (uid.to_string(), m.id))
            })
            .collect();

        if uid_msg_pairs.is_empty() {
            continue;
        }

        // Wall-clock timeout per user: 60s total for all IMAP connections.
        // Prevents a single slow server from blocking the entire sync.
        let per_user = sync_email_receipts_for_user(state, user_id, &creds, &uid_msg_pairs, now);
        if tokio::time::timeout(std::time::Duration::from_secs(60), per_user)
            .await
            .is_err()
        {
            tracing::warn!(
                "Email read receipt sync timed out for user {} after 60s",
                user_id
            );
        }
    }
}

async fn sync_email_receipts_for_user(
    state: &Arc<AppState>,
    user_id: i32,
    creds: &[crate::repositories::user_repository::ImapConnectionInfo],
    uid_msg_pairs: &[(String, i64)],
    now: i32,
) {
    for cred in creds {
        let server = match &cred.imap_server {
            Some(s) => s.clone(),
            None => continue,
        };
        let port = cred.imap_port.unwrap_or(993) as u16;

        let session_result = tokio::time::timeout(
            std::time::Duration::from_secs(15),
            imap_handlers::open_imap_session(&server, port, &cred.email, &cred.password),
        )
        .await;

        let mut session = match session_result {
            Ok(Ok(s)) => s,
            _ => continue,
        };

        if session.select("INBOX").await.is_err() {
            let _ = session.logout().await;
            continue;
        }

        let uid_set: String = uid_msg_pairs
            .iter()
            .map(|(uid, _)| uid.as_str())
            .collect::<Vec<_>>()
            .join(",");

        {
            let fetch_result = tokio::time::timeout(
                std::time::Duration::from_secs(15),
                session.uid_fetch(&uid_set, "FLAGS"),
            )
            .await;

            if let Ok(Ok(mut stream)) = fetch_result {
                while let Ok(Some(msg)) = stream.try_next().await {
                    if let Some(uid) = msg.uid {
                        let uid_str = uid.to_string();
                        if msg.flags().any(|f| f == async_imap::types::Flag::Seen) {
                            if let Some((_, msg_id)) =
                                uid_msg_pairs.iter().find(|(u, _)| u == &uid_str)
                            {
                                let _ = state.ontology_repository.mark_message_seen(*msg_id, now);
                            }
                        }
                    }
                }
            }
        } // drop stream before logout

        let _ = session.logout().await;

        debug!(
            "Synced email read receipts for user {} via {}",
            user_id, cred.email
        );
    }
}
