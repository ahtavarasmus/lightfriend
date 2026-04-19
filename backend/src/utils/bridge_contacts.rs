// Shared helper for fetching bridge contacts directly from the bridge's
// database, bypassing Matrix room enumeration.
//
// This is the public surface that handlers should call instead of touching
// the WhatsAppBridgeRepository directly. It owns the chain of:
//   user_id -> Matrix client -> Matrix user_id -> WhatsApp JID -> contacts
// and degrades gracefully if any step is missing or fails.

use crate::AppState;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ContactOption {
    /// Best display name for the contact (full_name > push_name > business_name > phone).
    pub name: String,
    /// Phone number extracted from the JID, if individual.
    pub phone: Option<String>,
    /// Raw WhatsApp JID for downstream use (e.g. message routing).
    pub jid: String,
    /// True if the contact came from the user's phone book.
    pub is_phone_contact: bool,
}

/// Fetch all WhatsApp contacts for a Lightfriend user.
///
/// Returns an empty Vec (logging a warning) if any of the following are true:
/// - WHATSAPP_BRIDGE_DATABASE_URL is unset (repository is None)
/// - the user has no Matrix client cached
/// - the Matrix client has no user_id
/// - the user is not currently logged into the WhatsApp bridge
/// - any database query fails
///
/// This function never panics and never returns an error - failures are
/// logged but the caller always gets a Vec it can iterate over.
pub async fn get_whatsapp_contacts(state: &Arc<AppState>, user_id: i32) -> Vec<ContactOption> {
    let Some(repo) = state.whatsapp_bridge_repository.as_ref() else {
        tracing::warn!(
            "get_whatsapp_contacts: user {} - bridge repository not configured (WHATSAPP_BRIDGE_DATABASE_URL unset or pool failed)",
            user_id
        );
        return Vec::new();
    };

    // Resolve the Matrix user ID for this Lightfriend user.
    let matrix_user_id = {
        let Some(cell) = state.matrix_users.get(&user_id).map(|e| e.value().clone()) else {
            tracing::info!(
                "get_whatsapp_contacts: user {} - no matrix client cached",
                user_id
            );
            return Vec::new();
        };
        let slot = cell.lock().await;
        let Some(us) = slot.as_ref() else {
            tracing::info!(
                "get_whatsapp_contacts: user {} - matrix cell empty",
                user_id
            );
            return Vec::new();
        };
        let Some(uid) = us.client.user_id() else {
            tracing::info!(
                "get_whatsapp_contacts: user {} - matrix client has no user_id",
                user_id
            );
            return Vec::new();
        };
        uid.to_string()
    };

    // Run the blocking DB calls on a worker thread so we don't block the runtime.
    let repo = Arc::clone(repo);
    let matrix_user_id_for_task = matrix_user_id.clone();
    let result = tokio::task::spawn_blocking(move || {
        repo.get_contacts_for_matrix_user(&matrix_user_id_for_task)
    })
    .await;

    let contacts = match result {
        Ok(Ok(c)) => c,
        Ok(Err(e)) => {
            tracing::warn!(
                "get_whatsapp_contacts: user {} matrix={} db query failed: {}",
                user_id,
                matrix_user_id,
                e
            );
            return Vec::new();
        }
        Err(e) => {
            tracing::warn!(
                "get_whatsapp_contacts: user {} matrix={} blocking task panicked: {}",
                user_id,
                matrix_user_id,
                e
            );
            return Vec::new();
        }
    };

    tracing::info!(
        "get_whatsapp_contacts: user {} matrix={} -> {} contacts from bridge DB",
        user_id,
        matrix_user_id,
        contacts.len()
    );

    contacts
        .into_iter()
        .map(|c| ContactOption {
            name: c.display_name(),
            phone: c.phone.clone(),
            jid: c.jid.clone(),
            is_phone_contact: c.is_phone_contact(),
        })
        .collect()
}
