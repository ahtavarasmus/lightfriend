use crate::AppState;
use std::sync::Arc;
use std::error::Error;

use crate::tool_call_utils::utils::create_openai_client;
use openai_api_rs::v1::chat_completion::{self, ChatCompletionMessage, MessageRole, Content};

use serde_json::{json, Value};

pub async fn handle_firecrawl_search(
    query: String,
    limit: u32,
) -> Result<String, Box<dyn Error>> {
    let api_key = std::env::var("FIRECRAWL_API_KEY")
        .map_err(|_| "FIRECRAWL_API_KEY environment variable not set")?;

    let data = json!({
      "query": query,
      "limit": limit,
      "location": "",
      "tbs": "",
      "scrapeOptions": {
        "formats": [ "markdown" ]
      }
    });

    let client = reqwest::Client::new();
    let response = client
        .post("https://api.firecrawl.dev/v1/search")
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&data)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("Failed to search: HTTP {}", response.status()).into());
    }

    let text = response.text().await?;
    Ok(text)
}

pub async fn get_weather(
    state: &Arc<AppState>,
    location: &str, 
    units: &str,
    user_id: i32,
) -> Result<String, Box<dyn Error>> {
    
    // TODO make api call to lightfriend.ai

    /*let response = format!(
        "The weather in {} is {} with a temperature of {} degrees {}. \
        The humidity is {}% and wind speed is {} {}. \n{}",
        location_name,
        description.to_lowercase(),
        temp.round(),
        temp_unit,
        humidity.round(),
        wind_speed.round(),
        speed_unit,
        hourly_forecast
    );
    */

    Ok("".to_string())
}

pub async fn ask_perplexity(
    state: &Arc<AppState>,
    message: &str, 
    system_prompt: &str
) -> Result<String, Box<dyn Error>> {

    let client = create_openai_client(&state)?;

    let messages = vec![
        ChatCompletionMessage {
            role: MessageRole::system,
            content: Content::Text(system_prompt.to_string()),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
        ChatCompletionMessage {
            role: MessageRole::user,
            content: Content::Text(message.to_string()),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
    ];

    let request = chat_completion::ChatCompletionRequest::new(
        "perplexity/sonar-reasoning-pro".to_string(),
        messages,
    );

    let response = client.chat_completion(request).await?;
    
    let content = response.choices[0].message.content.clone().unwrap_or_default();

    Ok(content)
}

use std::collections::HashSet;
use reqwest;
use serde_json;
use urlencoding;

pub async fn get_nearby_towns(
    location: &str,
) -> Result<Vec<String>, Box<dyn Error>> {
    // TODO make api req to lightfriend.ai
   
    Ok(Vec::new())
}

pub async fn get_coordinates(
    client: &reqwest::Client,
    address: &str,
    api_key: &str,
) -> Result<(f64, f64, String), Box<dyn Error>> {
    // TODO api req to lightfriend.ai
    Ok((0.3, 0.2, "".to_string()))
}
