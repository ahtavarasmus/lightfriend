use anyhow::{anyhow, Result};
use matrix_sdk::Client as MatrixClient;
use std::path::Path;
use std::sync::Arc;
use tokio::fs;
use tokio::time::{sleep, Duration};

use crate::utils::matrix_auth;
use crate::AppState;

/// Helper function to detect the one-time key conflict error from Matrix SDK
pub fn is_one_time_key_conflict(error: &anyhow::Error) -> bool {
    if let Some(http_err) = error.downcast_ref::<matrix_sdk::HttpError>() {
        let error_str = http_err.to_string();
        return error_str.contains("One time key") && error_str.contains("already exists");
    }
    false
}

/// Helper function to get the store path for a Matrix user
pub fn get_store_path(username: &str) -> Result<String> {
    let persistent_store_path = std::env::var("MATRIX_HOMESERVER_PERSISTENT_STORE_PATH")
        .map_err(|_| anyhow!("MATRIX_HOMESERVER_PERSISTENT_STORE_PATH not set"))?;
    Ok(format!("{}/{}", persistent_store_path, username))
}

/// Generic wrapper function with retry logic for bridge connections.
///
/// This function wraps any bridge-specific connection function and provides:
/// - Automatic retry on one-time key conflict errors
/// - Client store cleanup and reinitialization between retries
/// - Configurable retry count and delay
///
/// # Arguments
/// * `client` - Mutable reference to the Matrix client (will be reinitialized on retry)
/// * `bridge_name` - Name of the bridge for logging (e.g., "Signal", "Telegram")
/// * `user_id` - User ID for client reinitialization
/// * `state` - App state for accessing repositories
/// * `connect_fn` - The bridge-specific connection function to call
pub async fn connect_bridge_with_retry<F, Fut, R>(
    client: &mut Arc<MatrixClient>,
    bridge_name: &str,
    user_id: i32,
    state: &Arc<AppState>,
    connect_fn: F,
) -> Result<R>
where
    F: Fn(&Arc<MatrixClient>) -> Fut,
    Fut: std::future::Future<Output = Result<R>>,
{
    const MAX_RETRIES: u32 = 3;
    const RETRY_DELAY: Duration = Duration::from_secs(2);

    let username = client
        .user_id()
        .ok_or_else(|| anyhow!("User ID not available"))?
        .localpart()
        .to_string();

    for retry_count in 0..MAX_RETRIES {
        match connect_fn(client).await {
            Ok(result) => return Ok(result),
            Err(e) => {
                if retry_count < MAX_RETRIES - 1 && is_one_time_key_conflict(&e) {
                    tracing::warn!(
                        "One-time key conflict detected for {} user {} (attempt {}/{}), resetting client store",
                        bridge_name,
                        user_id,
                        retry_count + 1,
                        MAX_RETRIES
                    );

                    // Clear the store
                    let store_path = get_store_path(&username)?;
                    if Path::new(&store_path).exists() {
                        fs::remove_dir_all(&store_path).await?;
                        sleep(Duration::from_millis(500)).await;
                        fs::create_dir_all(&store_path).await?;
                        tracing::info!("Cleared store directory: {}", store_path);
                    }

                    // Add delay before retry
                    sleep(RETRY_DELAY).await;

                    // Reinitialize client
                    match matrix_auth::get_client(user_id, state).await {
                        Ok(new_client) => {
                            *client = new_client.into();
                            tracing::info!("Client reinitialized, retrying {} operation", bridge_name);
                            continue;
                        }
                        Err(init_err) => {
                            tracing::error!("Failed to reinitialize client: {}", init_err);
                            return Err(init_err);
                        }
                    }
                } else {
                    if is_one_time_key_conflict(&e) {
                        return Err(anyhow!(
                            "Failed after {} attempts to resolve one-time key conflict for {}: {}",
                            MAX_RETRIES,
                            bridge_name,
                            e
                        ));
                    } else {
                        return Err(e);
                    }
                }
            }
        }
    }

    Err(anyhow!("Unexpected: exhausted retry loop for {} without result", bridge_name))
}
