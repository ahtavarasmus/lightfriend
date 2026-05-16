use anyhow::{Context, Result};
use std::time::Duration;

#[derive(Debug)]
pub struct Config {
    pub bind_addr: String,
    pub bearer_token: String,

    pub twilio_account_sid: String,
    pub twilio_auth_token: String,
    pub twilio_from_number: String,
    pub admin_phone_number: String,

    pub resend_api_key: Option<String>,
    pub resend_from_email: String,
    pub admin_email: String,

    pub dedup_ttl_critical: Duration,
    pub dedup_ttl_error: Duration,
    pub dedup_ttl_warning: Duration,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            bind_addr: std::env::var("BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:8088".to_string()),
            bearer_token: required("NOTIFY_BEARER_TOKEN")?,

            twilio_account_sid: required("TWILIO_ACCOUNT_SID")?,
            twilio_auth_token: required("TWILIO_AUTH_TOKEN")?,
            twilio_from_number: required("TWILIO_FROM_NUMBER")?,
            admin_phone_number: required("ADMIN_PHONE_NUMBER")?,

            resend_api_key: std::env::var("RESEND_API_KEY").ok(),
            resend_from_email: std::env::var("RESEND_FROM_EMAIL")
                .unwrap_or_else(|_| "notify@lightfriend.ai".to_string()),
            admin_email: std::env::var("ADMIN_EMAIL")
                .unwrap_or_else(|_| "rasmus@lightfriend.ai".to_string()),

            dedup_ttl_critical: Duration::from_secs(
                std::env::var("DEDUP_TTL_CRITICAL_SECS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(3600),
            ),
            dedup_ttl_error: Duration::from_secs(
                std::env::var("DEDUP_TTL_ERROR_SECS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(6 * 3600),
            ),
            dedup_ttl_warning: Duration::from_secs(
                std::env::var("DEDUP_TTL_WARNING_SECS")
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(24 * 3600),
            ),
        })
    }
}

fn required(key: &str) -> Result<String> {
    std::env::var(key).with_context(|| format!("env var {} is required", key))
}
