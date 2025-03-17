
use std::error::Error;
use serde_json::{json, Value};

pub async fn get_weather(location: &str, units: &str) -> Result<String, Box<dyn Error>> {

    let client = reqwest::Client::new();
    
    // First, get coordinates using Open-Meteo Geocoding API
    let geocoding_url = format!(
        "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&language=en&format=json",
        urlencoding::encode(location)
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
    let lat = result["latitude"].as_f64()
        .ok_or("Latitude not found")?;
    let lon = result["longitude"].as_f64()
        .ok_or("Longitude not found")?;
    let location_name = result["name"].as_str()
        .unwrap_or(location);

    println!("Found coordinates for {}: lat={}, lon={}", location_name, lat, lon);

    // Get weather data using coordinates
    let temperature_unit = match units {
        "imperial" => "fahrenheit",
        _ => "celsius"
    };

    let wind_speed_unit = match units {
        "imperial" => "mph",
        _ => "ms"
    };

    let weather_url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current=temperature_2m,relative_humidity_2m,wind_speed_10m,weather_code&temperature_unit={}&wind_speed_unit={}",
        lat,
        lon,
        temperature_unit,
        wind_speed_unit
    );

    let weather_data: serde_json::Value = client
        .get(&weather_url)
        .send()
        .await?
        .json()
        .await?;

    let current = weather_data["current"].as_object()
        .ok_or("No current weather data")?;

    let temp = current["temperature_2m"].as_f64().unwrap_or(0.0);
    let humidity = current["relative_humidity_2m"].as_f64().unwrap_or(0.0);
    let wind_speed = current["wind_speed_10m"].as_f64().unwrap_or(0.0);
    let weather_code = current["weather_code"].as_i64().unwrap_or(0);

    // Convert WMO weather code to description
    let description = match weather_code {
        0 => "clear sky",
        1..=3 => "partly cloudy",
        45..=48 => "foggy",
        51..=57 => "drizzling",
        61..=65 => "raining",
        71..=77 => "snowing",
        80..=82 => "rain showers",
        85..=86 => "snow showers",
        95 => "thunderstorm",
        96..=99 => "thunderstorm with hail",
        _ => "unknown weather"
    };


    let (temp_unit, speed_unit) = match units {
        "imperial" => ("Fahrenheit", "miles per hour"),
        _ => ("Celsius", "meters per second")
    };

    let response = format!(
        "The weather in {} is {} with a temperature of {} degrees {}. \
        The humidity is {}% and wind speed is {} {}.",
        location_name,
        description,
        temp.round(),
        temp_unit,
        humidity.round(),
        wind_speed.round(),
        speed_unit
    );

    Ok(response)
}


pub async fn ask_perplexity(message: &str, system_prompt: &str) -> Result<String, Box<dyn Error>> {
    let api_key = std::env::var("PERPLEXITY_API_KEY").expect("PERPLEXITY_API_KEY must be set");
    let client = reqwest::Client::new();
    
    let payload = json!({
        "model": "sonar-pro",
        "messages": [
                {
                    "role": "system",
                    "content": system_prompt, 
                },
                {
                    "role": "user",
                    "content": message
                },
        ]
    });

    let response = client
        .post("https://api.perplexity.ai/chat/completions")
        .header("accept", "application/json")
        .header("content-type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&payload)
        .send()
        .await?;

    let response_text = response.text().await?;
    println!("Raw response: {}", response_text);
    
    // Parse the JSON response
    let response_json: Value = serde_json::from_str(&response_text)?;
    
    // Extract the assistant's message content
    let content = response_json
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_str())
        .ok_or("Failed to extract message content")?;

    println!("Extracted content: {}", content);
    Ok(content.to_string())
}
