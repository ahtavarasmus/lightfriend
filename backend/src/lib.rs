
use rand::Rng;
use chrono::{Duration, Utc};
use reqwest::Client;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use serde_json::json;

pub struct TwilioConfig {
    pub account_sid: String,
    pub auth_token: String,
    pub from_number: String,
}

impl TwilioConfig {
    pub fn new() -> Self {
        Self {
            account_sid: std::env::var("TWILIO_ACCOUNT_SID")
                .expect("TWILIO_ACCOUNT_SID must be set"),
            auth_token: std::env::var("TWILIO_AUTH_TOKEN")
                .expect("TWILIO_AUTH_TOKEN must be set"),
            from_number: std::env::var("TWILIO_FROM_NUMBER")
                .expect("TWILIO_FROM_NUMBER must be set"),
        }
    }
}

pub fn generate_otp() -> String {
    let mut rng = rand::thread_rng();
    format!("{:06}", rng.gen_range(0..999999))
}

pub async fn send_otp(config: &TwilioConfig, to_number: &str, otp: &str) -> Result<(), String> {
    let client = Client::new();
    let message = format!("Your verification code is: {}. Valid for 10 minutes.", otp);
    
    // Create basic auth header
    let auth = format!("{}:{}", config.account_sid, config.auth_token);
    let encoded_auth = BASE64.encode(auth.as_bytes());
    
    // Prepare the request URL
    let url = format!(
        "https://api.twilio.com/2010-04-01/Accounts/{}/Messages.json",
        config.account_sid
    );

    // Create form data
    let form = json!({
        "From": config.from_number,
        "To": to_number,
        "Body": message,
    });

    // Send the request
    let response = client
        .post(&url)
        .header("Authorization", format!("Basic {}", encoded_auth))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&form)
        .send()
        .await
        .map_err(|e| format!("Failed to send request: {}", e))?;

    // Check if the request was successful
    if !response.status().is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("Twilio API error: {}", error_text));
    }

    Ok(())
}
