use anyhow::{anyhow, Result};
use reqwest::Client;
use serde_json::json;

use crate::config::Config;

pub async fn send_digest(
    http: &Client,
    cfg: &Config,
    subject: &str,
    plain_body: &str,
    html_body: Option<&str>,
) -> Result<()> {
    let api_key = cfg
        .resend_api_key
        .as_ref()
        .ok_or_else(|| anyhow!("RESEND_API_KEY not configured; cannot send digest"))?;

    let mut payload = json!({
        "from": format!("Lightfriend Notify <{}>", cfg.resend_from_email),
        "to": [cfg.admin_email],
        "subject": subject,
        "text": plain_body,
    });
    if let Some(html) = html_body {
        payload["html"] = json!(html);
    }

    let resp = http
        .post("https://api.resend.com/emails")
        .bearer_auth(api_key)
        .json(&payload)
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(anyhow!("resend returned {}: {}", status, text));
    }
    Ok(())
}
