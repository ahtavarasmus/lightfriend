use anyhow::{anyhow, Result};
use reqwest::Client;

use crate::config::Config;

pub async fn send(http: &Client, cfg: &Config, body: &str) -> Result<()> {
    let url = format!(
        "https://api.twilio.com/2010-04-01/Accounts/{}/Messages.json",
        cfg.twilio_account_sid
    );

    let resp = http
        .post(&url)
        .basic_auth(&cfg.twilio_account_sid, Some(&cfg.twilio_auth_token))
        .form(&[
            ("From", cfg.twilio_from_number.as_str()),
            ("To", cfg.admin_phone_number.as_str()),
            ("Body", body),
        ])
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        return Err(anyhow!("twilio returned {}: {}", status, text));
    }
    Ok(())
}
