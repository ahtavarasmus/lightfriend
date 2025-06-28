use crate::AppState;
use std::sync::Arc;
use tracing::{info, error};
use std::env;
use openai_api_rs;
use serde_json;
use reqwest;
use serde::Deserialize;

use crate::handlers::imap_handlers::ImapEmailPreview;

use reqwest::multipart;

#[derive(Debug, Deserialize)]
struct TwilioMediaResponse {
    sid: String,
    links: TwilioMediaLinks,
}

#[derive(Debug, Deserialize)]
struct TwilioMediaLinks {
    #[serde(default)]
    content_direct_temporary: Option<String>,
    content: String,
}

pub async fn upload_media_to_twilio(
    content_type: String,
    data: Vec<u8>,
    filename: String,
    service_sid: String,
) -> Result<String, Box<dyn std::error::Error>> {
    let twilio_account_sid = env::var("TWILIO_ACCOUNT_SID")
        .map_err(|_| "TWILIO_ACCOUNT_SID not set")?;
    let twilio_auth_token = env::var("TWILIO_AUTH_TOKEN")
        .map_err(|_| "TWILIO_AUTH_TOKEN not set")?;
    
    let client = reqwest::Client::new();
    
    let url = format!(
        "https://mcs.us1.twilio.com/v1/Services/{}/Media",
        service_sid
    );

    tracing::info!("Uploading media to Twilio. Content-Type: {}, Filename: {}", content_type, filename);

    // Create multipart form data
    let part = multipart::Part::bytes(data)
        .file_name(filename.clone())
        .mime_str(&content_type)?;
    
    let form = multipart::Form::new()
        .part("file", part);
    
    let response = client
        .post(&url)
        .basic_auth(&twilio_account_sid, Some(&twilio_auth_token))
        .multipart(form)
        .send()
        .await?;
    
    let status = response.status();
    let headers = response.headers().clone();
    
    tracing::debug!("Twilio response status: {}", status);
    tracing::debug!("Twilio response headers: {:?}", headers);
    
    if !status.is_success() {
        let error_text = response.text().await?;
        tracing::error!("Twilio upload failed with status {} and error: {}", status, error_text);
        return Err(format!("Failed to upload media: {} - {}", status, error_text).into());
    }
    
    let response_text = response.text().await?;
    tracing::debug!("Twilio response body: {}", response_text);
    
    match serde_json::from_str::<TwilioMediaResponse>(&response_text) {
        Ok(media_response) => {
            tracing::info!("Successfully uploaded media to Twilio");
            match media_response.links.content_direct_temporary {
                Some(url) => Ok(url),
                None => Ok(format!( // ← new fallback
                    "https://mcs.us1.twilio.com{}",
                    media_response.links.content
                )),
            }
        },
        Err(e) => {
            tracing::error!("Failed to parse Twilio response: {}", e);
            tracing::error!("Raw response: {}", response_text);
            Err(format!("Failed to parse Twilio response: {} - Raw response: {}", e, response_text).into())
        }
    }
}



