//! Resend Contacts API - syncs user emails to Resend for disaster recovery.
//! If we lose all data, we can fetch the email list from Resend to rebuild users.
//! Uses the HTTP API directly (resend-rs 0.9 doesn't support contacts/segments).

use reqwest::Client;
use serde::{Deserialize, Serialize};

const RESEND_API_BASE: &str = "https://api.resend.com";

#[derive(Serialize)]
struct CreateContactRequest {
    email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    first_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_name: Option<String>,
    unsubscribed: bool,
}

#[derive(Deserialize, Debug)]
struct ContactResponse {
    #[allow(dead_code)]
    id: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct Contact {
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub unsubscribed: bool,
}

#[derive(Deserialize, Debug)]
struct ListContactsResponse {
    data: Vec<Contact>,
    has_more: bool,
}

fn get_api_key() -> Option<String> {
    std::env::var("RESEND_API_KEY").ok()
}

/// Add or update a contact in Resend. Idempotent - safe to call multiple times.
/// Fails silently if RESEND_API_KEY is not set (non-critical feature).
pub async fn sync_contact(email: &str) {
    let api_key = match get_api_key() {
        Some(k) => k,
        None => return,
    };

    let client = Client::new();
    let body = CreateContactRequest {
        email: email.to_string(),
        first_name: None,
        last_name: None,
        unsubscribed: false,
    };

    match client
        .post(format!("{}/contacts", RESEND_API_BASE))
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&body)
        .send()
        .await
    {
        Ok(resp) => {
            if resp.status().is_success() {
                tracing::debug!("Synced contact to Resend: {}", email);
            } else {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                // 409 = already exists, which is fine
                if status.as_u16() == 409 {
                    tracing::debug!("Contact already exists in Resend: {}", email);
                } else {
                    tracing::warn!("Failed to sync contact to Resend ({}): {}", status, body);
                }
            }
        }
        Err(e) => {
            tracing::warn!("Failed to reach Resend contacts API: {}", e);
        }
    }
}

/// Fetch all contacts from Resend. Used for disaster recovery to rebuild user list.
pub async fn list_all_contacts() -> Result<Vec<Contact>, String> {
    let api_key = get_api_key().ok_or("RESEND_API_KEY not set")?;
    let client = Client::new();
    let mut all_contacts = Vec::new();
    let mut after: Option<String> = None;

    loop {
        let mut url = format!("{}/contacts?limit=100", RESEND_API_BASE);
        if let Some(ref cursor) = after {
            url.push_str(&format!("&after={}", cursor));
        }

        let resp = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await
            .map_err(|e| format!("Failed to reach Resend: {}", e))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Resend API error: {}", body));
        }

        let list: ListContactsResponse = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse Resend response: {}", e))?;

        let has_more = list.has_more;
        if let Some(last) = list.data.last() {
            after = Some(last.email.clone());
        }
        all_contacts.extend(list.data);

        if !has_more {
            break;
        }
    }

    Ok(all_contacts)
}
