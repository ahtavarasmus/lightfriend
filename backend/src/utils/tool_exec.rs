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
    
    // Get user settings
    let settings = state.user_core.get_user_settings(user_id).map_err(|e| format!("Failed to get user settings: {}", e))?;
    let user_info= state.user_core.get_user_info(user_id).map_err(|e| format!("Failed to get user settings: {}", e))?;
    let user_timezone = user_info.timezone;
    
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

    // Get timezone: prefer user's if set, else location's from weather data
    let location_timezone = weather_data["timezone"].as_str().unwrap_or("UTC").to_string();
    let tz_str = user_timezone.unwrap_or(location_timezone);

    // Parse timezone using chrono_tz
    use chrono_tz::Tz;
    let tz: Tz = tz_str.parse::<Tz>().unwrap_or(chrono_tz::UTC);

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
                let dt_utc = chrono::DateTime::from_timestamp(time, 0)
                    .unwrap_or(chrono::Utc::now());
                let dt_local = dt_utc.with_timezone(&tz);
                let datetime = dt_local.format("%H:%M").to_string();
                
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

    Ok(response)
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
   
    let client = reqwest::Client::new();
    let geoapify_key = std::env::var("GEOAPIFY_API_KEY").expect("GEOAPIFY_API_KEY must be set");
   
    // Get coordinates using Geoapify Geocoding
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
   
    // Get nearby populated places (focus on suburb and neighbourhood for close places)
    let categories = "populated_place.suburb,populated_place.neighbourhood";
    let places_url = format!(
        "https://api.geoapify.com/v2/places?categories={}&filter=circle:{},{},8000&bias=proximity:{},{}&limit=50&apiKey={}",
        categories,
        lon,
        lat,
        lon,
        lat,
        geoapify_key
    );
    println!("Places API URL: {}", places_url);
    let response = client.get(&places_url).send().await?;
    println!("Places API status: {}", response.status());
    let places_response: serde_json::Value = response.json().await?;
   
    let features = places_response["features"].as_array()
        .ok_or("No features found")?;
   
    let mut nearby_places: Vec<(String, f64)> = Vec::new(); // (name, distance)
    let mut seen = HashSet::new();
   
    // Extract suburb part from location_name for accurate skipping (e.g., "Vuores" from full address)
    let input_suburb = location_name.split(',').next().unwrap_or(location).trim().to_lowercase();
   
    for feature in features {
        if let Some(properties) = feature["properties"].as_object() {
            let place_name = properties.get("name").and_then(|v| v.as_str()).map(|s| s.to_string());
           
            if let Some(name) = place_name {
                let lower_name = name.to_lowercase();
                if lower_name == input_suburb {
                    continue;
                }
                // Get distance if available, default to MAX if not
                let distance = properties.get("distance").and_then(|v| v.as_f64()).unwrap_or(f64::MAX);
                if seen.insert(name.clone()) {
                    nearby_places.push((name, distance));
                }
            }
        }
    }
   
    // Sort by distance ascending (closest first)
    nearby_places.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
   
    // Take top 20
    let top_places: Vec<(String, f64)> = nearby_places.into_iter().take(15).collect();
   
    println!("Found {} nearby places (top 20 by proximity ascending) for {}", top_places.len(), location_name);
    for (name, dist) in &top_places {
        println!("- {} (distance: {} meters)", name, dist.round() as i64); // Round distance to integer
    }
   
    // Return just names for the result
    let place_names: Vec<String> = top_places.into_iter().map(|(name, _)| name).collect();
   
    Ok(place_names)
}

pub async fn get_coordinates(
    client: &reqwest::Client,
    address: &str,
    api_key: &str,
) -> Result<(f64, f64, String), Box<dyn Error>> {
    let url = format!(
        "https://api.geoapify.com/v1/geocode/search?text={}&format=json&apiKey={}",
        urlencoding::encode(address),
        api_key
    );
    let response: serde_json::Value = client.get(&url).send().await?.json().await?;
    let results = response["results"].as_array().ok_or("No results found")?;
    if results.is_empty() {
        return Err("Location not found".into());
    }
    let result = &results[0];
    let lat = result["lat"].as_f64().ok_or("Latitude not found")?;
    let lon = result["lon"].as_f64().ok_or("Longitude not found")?;
    let formatted = result["formatted"]
        .as_str()
        .unwrap_or(address)
        .to_string();
    Ok((lat, lon, formatted))
}
