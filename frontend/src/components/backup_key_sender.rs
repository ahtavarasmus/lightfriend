//! Background component that periodically sends the session key to the backend.
//!
//! This component:
//! 1. Sends the session key immediately on mount
//! 2. Refreshes every 5 minutes to keep the backend's session key current
//!
//! The component renders nothing - it's purely for side effects.

use gloo_console::log;
use gloo_timers::callback::Interval;
use yew::prelude::*;

use crate::utils::api::Api;
use crate::utils::backup_crypto;

const REFRESH_INTERVAL_MS: u32 = 5 * 60 * 1000; // 5 minutes

#[function_component(BackupKeySender)]
pub fn backup_key_sender() -> Html {
    use_effect(|| {
        // Initial send on mount
        wasm_bindgen_futures::spawn_local(send_session_key());

        // Set up periodic refresh
        let interval = Interval::new(REFRESH_INTERVAL_MS, || {
            wasm_bindgen_futures::spawn_local(send_session_key());
        });

        // Cleanup: drop interval when component unmounts
        move || drop(interval)
    });

    // Render nothing - this is a side-effect-only component
    html! {}
}

async fn send_session_key() {
    match backup_crypto::get_session_key_from_storage().await {
        Ok(Some(session_key)) => {
            log!("Sending session key to backend");
            let result = Api::post("/api/backup/establish-key")
                .json(&serde_json::json!({ "session_key": session_key }))
                .ok()
                .map(|r| r.send());

            if let Some(future) = result {
                match future.await {
                    Ok(response) => {
                        if response.ok() {
                            log!("Session key sent successfully");
                        } else {
                            log!("Failed to send session key: server returned error");
                        }
                    }
                    Err(e) => {
                        log!(format!("Failed to send session key: {:?}", e));
                    }
                }
            }
        }
        Ok(None) => {
            // No backup initialized yet - this is normal if user hasn't logged in
            // with password-based backup enabled
            log!("No backup session key in storage");
        }
        Err(e) => {
            log!(format!("Error getting session key from storage: {}", e));
        }
    }
}
