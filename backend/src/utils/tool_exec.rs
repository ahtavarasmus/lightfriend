use crate::AppState;
use std::sync::Arc;
use std::error::Error;

use crate::tool_call_utils::utils::create_openai_client;
use openai_api_rs::v1::chat_completion::{self, ChatCompletionMessage, MessageRole, Content};

pub async fn get_weather(
    state: &Arc<AppState>,
    location: &str, 
    units: &str
) -> Result<String, Box<dyn Error>> {
    
    let client = reqwest::Client::new();
    // Get API keys from environment or user settings
    let is_self_hosted = std::env::var("ENVIRONMENT") == Ok("self_hosted".to_string());
    
    let (geoapify_key, pirate_weather_key) = if is_self_hosted {
        // Get keys from user settings for self-hosted environment
        match state.user_core.get_settings_for_tier3() {
            Ok((_, _, _, _, Some(geoapify), Some(pirate))) => {
                tracing::info!("✅ Successfully retrieved weather API keys from user settings");
                (geoapify, pirate)
            },
            _ => {
                tracing::error!("❌ Failed to get weather API keys from user settings");
                return Err("Failed to get weather API keys from user settings".into());
            }
        }
    } else {
        // Get keys from environment variables
        (
            std::env::var("GEOAPIFY_API_KEY").expect("GEOAPIFY_API_KEY must be set"),
            std::env::var("PIRATE_WEATHER_API_KEY").expect("PIRATE_WEATHER_API_KEY must be set")
        )
    };
    
    // First, get coordinates using Geoapify
    let geocoding_url = format!(
        "https://api.geoapify.com/v1/geocode/search?text={}&format=json&apiKey={}",
        urlencoding::encode(location),
        geoapify_key
    );

    let geocoding_response: serde_json::Value = client
        .get(&geocoding_url)
        .send()
        .await?
        .json()
        .await?;

    let results = geocoding_response["results"].as_array()
        .ok_or("No results found")?;

    if results.is_empty() {
        return Err("Location not found".into());
    }

    let result = &results[0];
    let lat = result["lat"].as_f64()
        .ok_or("Latitude not found")?;
    let lon = result["lon"].as_f64()
        .ok_or("Longitude not found")?;
    let location_name = result["formatted"].as_str()
        .unwrap_or(location);

    println!("Found coordinates for {}: lat={}, lon={}", location_name, lat, lon);

    // Get weather data using Pirate Weather
    let unit_system = match units {
        "imperial" => "us",
        _ => "si"
    };

    let weather_url = format!(
        "https://api.pirateweather.net/forecast/{}/{},{}?units={}&exclude=minutely,daily,alerts",
        pirate_weather_key,
        lat,
        lon,
        unit_system
    );

    let weather_data: serde_json::Value = client
        .get(&weather_url)
        .send()
        .await?
        .json()
        .await?;

    let current = weather_data["currently"].as_object()
        .ok_or("No current weather data")?;

    let temp = current["temperature"].as_f64().unwrap_or(0.0);
    let humidity = current["humidity"].as_f64().unwrap_or(0.0) * 100.0; // Convert from 0-1 to percentage
    let wind_speed = current["windSpeed"].as_f64().unwrap_or(0.0);
    let description = current["summary"].as_str().unwrap_or("unknown weather");

    let (temp_unit, speed_unit) = match units {
        "imperial" => ("Fahrenheit", "miles per hour"),
        _ => ("Celsius", "meters per second")
    };

    println!("{:#?}", weather_data);
    // Process hourly forecast
    let mut hourly_forecast = String::new();
    if let Some(hourly) = weather_data["hourly"]["data"].as_array() {
        // Get next 6 hours
        for (i, hour) in hourly.iter().take(6).enumerate() {
            if let (Some(temp), Some(precip_prob)) = (
                hour["temperature"].as_f64(),
                hour["precipProbability"].as_f64()
            ) {
                if i == 0 {
                    hourly_forecast.push_str("\n\nHourly forecast:");
                }
                let time = hour["time"].as_i64().unwrap_or(0);
                let datetime = chrono::DateTime::from_timestamp(time, 0)
                    .map(|dt| dt.format("%H:%M").to_string())
                    .unwrap_or_else(|| "unknown time".to_string());
                
                hourly_forecast.push_str(&format!(
                    "\n{}: {} degrees {} with {}% chance of precipitation",
                    datetime,
                    temp.round(),
                    temp_unit,
                    (precip_prob * 100.0).round()
                ));
            }
        }
    }

    let response = format!(
        "The weather in {} is {} with a temperature of {} degrees {}. \
        The humidity is {}% and wind speed is {} {}.{}",
        location_name,
        description.to_lowercase(),
        temp.round(),
        temp_unit,
        humidity.round(),
        wind_speed.round(),
        speed_unit,
        hourly_forecast
    );

    Ok(response)
}


pub async fn ask_perplexity(
    state: &Arc<AppState>,
    message: &str, 
    system_prompt: &str
) -> Result<String, Box<dyn Error>> {

    println!("message for perplexity: {}", message);
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
        "perplexity/sonar-pro".to_string(),
        messages,
    );

    let response = client.chat_completion(request).await?;
    
    let content = response.choices[0].message.content.clone().unwrap_or_default();
    println!("content: {}", content);

    Ok(content)
}
