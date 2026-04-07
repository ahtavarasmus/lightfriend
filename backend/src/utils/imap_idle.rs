//! Per-IMAP-connection IDLE task loop.
//!
//! Each entry in `AppState::imap_idle_tasks` is a `tokio::task::JoinHandle`
//! keyed by `imap_connection.id`. The task opens an IMAP session, selects
//! INBOX, enters `IDLE`, and waits for the server to push notifications.
//! On every wake event we call
//! `imap_handlers::process_new_emails` to fetch + insert + mark.
//!
//! Design notes:
//! - Keyed by `imap_connection.id`, not `user_id`, so users with multiple
//!   email accounts each get their own task.
//! - The polling cron in `scheduler.rs` stays as a safety net for servers
//!   that don't advertise the `IDLE` capability or during task-down windows.
//! - 3-strike rule for auth failures: after 3 consecutive
//!   `CredentialsError`s the task flips `imap_connection.status` to
//!   `"auth_failed"` and exits + emits an admin alert.
//! - Reconnects use exponential backoff 1s -> 300s, reset on each
//!   successful login.
//! - IDLE wait timeout is 28 minutes (RFC 2177 says to re-IDLE every ~29
//!   min — most servers evict longer idles).

use std::sync::Arc;
use std::time::Duration;

use async_imap::extensions::idle::IdleResponse;
use tokio::task::JoinHandle;

use crate::admin_alert;
use crate::handlers::imap_handlers::{open_imap_session, process_new_emails, ImapError};
use crate::AppState;

const INITIAL_BACKOFF_SECS: u64 = 1;
const MAX_BACKOFF_SECS: u64 = 300;
const IDLE_TIMEOUT_SECS: u64 = 28 * 60;
const MAX_CONSECUTIVE_AUTH_FAILURES: u32 = 3;

/// Spawn a per-connection IDLE task if one isn't already running.
///
/// Safe to call multiple times for the same id — the second call is a
/// no-op if the existing task is still alive. Call from:
///   * `main.rs` startup (via `initialize_all_idle_tasks`)
///   * `imap_login` handler right after `set_imap_credentials` returns the id
///   * the scheduler's dead-task restart sweep
pub fn spawn_idle_task_for_connection(state: Arc<AppState>, imap_connection_id: i32) {
    // If an existing task is still alive, do nothing.
    if let Some(existing) = state.imap_idle_tasks.get(&imap_connection_id) {
        if !existing.is_finished() {
            tracing::debug!(
                "IDLE task for connection {} already running; skipping spawn",
                imap_connection_id
            );
            return;
        }
    }
    // Replace any stale finished handle.
    state.imap_idle_tasks.remove(&imap_connection_id);

    let state_for_task = state.clone();
    let handle: JoinHandle<()> = tokio::spawn(async move {
        run_idle_loop(state_for_task, imap_connection_id).await;
    });

    state.imap_idle_tasks.insert(imap_connection_id, handle);
    tracing::info!("Spawned IDLE task for connection {}", imap_connection_id);
}

/// Abort a running IDLE task and remove it from the registry. Called from
/// `delete_imap_connection` before the DB row is deleted.
pub fn abort_idle_task(state: &AppState, imap_connection_id: i32) {
    if let Some((_, handle)) = state.imap_idle_tasks.remove(&imap_connection_id) {
        handle.abort();
        tracing::info!("Aborted IDLE task for connection {}", imap_connection_id);
    }
}

/// Startup sweep: spawn one IDLE task for every `active` imap_connection
/// row. Staggers task spawns by 100ms to avoid a thundering herd against
/// Gmail's login rate limits.
pub async fn initialize_all_idle_tasks(state: Arc<AppState>) {
    let connections = match state.user_repository.get_all_active_imap_connections() {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!(
                "Failed to load IMAP connections for IDLE init: {} — skipping",
                e
            );
            return;
        }
    };

    tracing::info!(
        "Initializing IMAP IDLE tasks for {} connections",
        connections.len()
    );

    for (conn_id, _user_id) in connections {
        spawn_idle_task_for_connection(state.clone(), conn_id);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Outer reconnect loop: refresh creds, open a session, run the inner
/// IDLE loop, and reconnect with exponential backoff on any error.
async fn run_idle_loop(state: Arc<AppState>, imap_connection_id: i32) {
    let mut backoff_secs: u64 = INITIAL_BACKOFF_SECS;
    let mut consecutive_auth_failures: u32 = 0;

    loop {
        // Sleep *before* each attempt (on first iteration, 1s). Keeps the
        // control-flow simple and gives Postgres a moment on cold start.
        tokio::time::sleep(Duration::from_secs(backoff_secs)).await;

        // Look up live credentials each time so password rotations are
        // picked up without restarting the process.
        let (user_id, info, status) = match state
            .user_repository
            .get_imap_connection_by_id(imap_connection_id)
        {
            Ok(Some(row)) => row,
            Ok(None) => {
                tracing::info!(
                    "IDLE task: connection {} no longer exists, exiting",
                    imap_connection_id
                );
                state.imap_idle_tasks.remove(&imap_connection_id);
                return;
            }
            Err(e) => {
                tracing::warn!(
                    "IDLE task: failed to load connection {}: {}; will retry",
                    imap_connection_id,
                    e
                );
                backoff_secs = (backoff_secs * 2).min(MAX_BACKOFF_SECS);
                continue;
            }
        };

        if status != "active" {
            tracing::info!(
                "IDLE task: connection {} status='{}' (not active), exiting",
                imap_connection_id,
                status
            );
            state.imap_idle_tasks.remove(&imap_connection_id);
            return;
        }

        let server = info
            .imap_server
            .clone()
            .unwrap_or_else(|| "imap.gmail.com".to_string());
        let port = info.imap_port.unwrap_or(993) as u16;

        // Open the session (TCP + TLS + LOGIN).
        let mut session = match open_imap_session(&server, port, &info.email, &info.password).await
        {
            Ok(s) => s,
            Err(ImapError::CredentialsError(msg)) => {
                consecutive_auth_failures += 1;
                tracing::warn!(
                    "IDLE task: auth failure {}/{} for connection {}: {}",
                    consecutive_auth_failures,
                    MAX_CONSECUTIVE_AUTH_FAILURES,
                    imap_connection_id,
                    msg
                );
                if consecutive_auth_failures >= MAX_CONSECUTIVE_AUTH_FAILURES {
                    if let Err(e) = state
                        .user_repository
                        .set_imap_connection_status(imap_connection_id, "auth_failed")
                    {
                        tracing::error!(
                            "Failed to mark connection {} as auth_failed: {}",
                            imap_connection_id,
                            e
                        );
                    }
                    admin_alert!(
                        state,
                        Warning,
                        "IMAP IDLE auth failed after 3 attempts",
                        user_id = user_id,
                        connection_id = imap_connection_id,
                        email = info.email
                    );
                    state.imap_idle_tasks.remove(&imap_connection_id);
                    return;
                }
                backoff_secs = (backoff_secs * 2).min(MAX_BACKOFF_SECS);
                continue;
            }
            Err(e) => {
                tracing::warn!(
                    "IDLE task: failed to open session for connection {}: {:?}; will retry",
                    imap_connection_id,
                    e
                );
                backoff_secs = (backoff_secs * 2).min(MAX_BACKOFF_SECS);
                continue;
            }
        };

        // Reset auth-failure counter on successful login. `backoff_secs`
        // gets reset at the bottom of the loop after inner_idle_loop
        // returns (reconnect-from-healthy case).
        consecutive_auth_failures = 0;

        // Check for IDLE capability before committing to the IDLE path.
        let idle_supported = match session.capabilities().await {
            Ok(caps) => caps.has_str("IDLE"),
            Err(e) => {
                tracing::warn!(
                    "IDLE task: failed to read CAPABILITIES for connection {}: {}; reconnecting",
                    imap_connection_id,
                    e
                );
                let _ = session.logout().await;
                backoff_secs = 2;
                continue;
            }
        };

        if !idle_supported {
            tracing::warn!(
                "IDLE task: server does not advertise IDLE for connection {} ({}); exiting, cron will handle this user",
                imap_connection_id,
                info.email
            );
            let _ = session.logout().await;
            state.imap_idle_tasks.remove(&imap_connection_id);
            return;
        }

        // Initial resync: fetch anything that arrived while the task was down.
        let since_uid = state
            .user_repository
            .get_max_processed_uid(user_id, imap_connection_id)
            .unwrap_or(None);

        match process_new_emails(&state, user_id, imap_connection_id, &mut session, since_uid).await
        {
            Ok(n) if n > 0 => {
                tracing::info!(
                    "IDLE initial resync for connection {} processed {} new email(s)",
                    imap_connection_id,
                    n
                );
            }
            Ok(_) => {}
            Err(e) => {
                tracing::warn!(
                    "IDLE initial resync failed for connection {}: {:?}; reconnecting",
                    imap_connection_id,
                    e
                );
                let _ = session.logout().await;
                backoff_secs = 2;
                continue;
            }
        }

        tracing::info!("IDLE established for connection {}", imap_connection_id);

        // Inner IDLE loop. On any break, we fall back to the outer loop
        // which will reconnect.
        let returned_session = inner_idle_loop(&state, user_id, imap_connection_id, session).await;

        // If the inner loop handed a session back, try to log out cleanly
        // before reconnecting.
        if let Some(mut s) = returned_session {
            if let Err(e) = s.logout().await {
                tracing::debug!(
                    "IDLE task: logout failed for connection {}: {}",
                    imap_connection_id,
                    e
                );
            }
        }

        // Reconnect from a healthy state: short backoff, not the
        // exponentially-climbing one used after errors.
        backoff_secs = INITIAL_BACKOFF_SECS;
    }
}

/// Inner loop: keep re-entering IDLE until something forces a reconnect.
/// Returns the session back on clean exit (timeout, manual interrupt) so
/// the outer loop can log out.
async fn inner_idle_loop(
    state: &Arc<AppState>,
    user_id: i32,
    imap_connection_id: i32,
    mut session: crate::handlers::imap_handlers::ImapSession,
) -> Option<crate::handlers::imap_handlers::ImapSession> {
    loop {
        let mut handle = session.idle();
        if let Err(e) = handle.init().await {
            tracing::warn!(
                "IDLE task: init() failed for connection {}: {}; reconnecting",
                imap_connection_id,
                e
            );
            // Can't reuse the handle after failure; the outer loop will
            // rebuild a fresh session.
            return None;
        }

        let (fut, _stop_source) = handle.wait_with_timeout(Duration::from_secs(IDLE_TIMEOUT_SECS));
        let resp = fut.await;

        // Whether wait succeeded or failed, we must call done() to get
        // the session back before issuing any other IMAP command.
        let next_session = match handle.done().await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(
                    "IDLE task: done() failed for connection {}: {}; reconnecting",
                    imap_connection_id,
                    e
                );
                return None;
            }
        };
        session = next_session;

        match resp {
            Ok(IdleResponse::NewData(_)) => {
                // Server pushed something — fetch and insert.
                let since_uid = state
                    .user_repository
                    .get_max_processed_uid(user_id, imap_connection_id)
                    .unwrap_or(None);

                match process_new_emails(
                    state,
                    user_id,
                    imap_connection_id,
                    &mut session,
                    since_uid,
                )
                .await
                {
                    Ok(n) if n > 0 => {
                        tracing::info!(
                            "IDLE wake: processed {} new email(s) for connection {}",
                            n,
                            imap_connection_id
                        );
                    }
                    Ok(_) => {
                        tracing::debug!(
                            "IDLE wake for connection {} but no new mail (already processed)",
                            imap_connection_id
                        );
                    }
                    Err(e) => {
                        tracing::warn!(
                            "IDLE wake: process_new_emails failed for connection {}: {:?}; reconnecting",
                            imap_connection_id,
                            e
                        );
                        return Some(session);
                    }
                }
            }
            Ok(IdleResponse::Timeout) => {
                tracing::debug!(
                    "IDLE timeout (28m) for connection {}, refreshing",
                    imap_connection_id
                );
                // Fall through to the top of the loop to re-enter IDLE.
            }
            Ok(IdleResponse::ManualInterrupt) => {
                tracing::debug!(
                    "IDLE manual interrupt for connection {}",
                    imap_connection_id
                );
                return Some(session);
            }
            Err(e) => {
                tracing::warn!(
                    "IDLE wait error for connection {}: {}; reconnecting",
                    imap_connection_id,
                    e
                );
                return Some(session);
            }
        }
    }
}
