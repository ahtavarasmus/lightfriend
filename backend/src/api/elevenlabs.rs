use axum::{
    Json,
    extract::State,
    response::Response,
    http::{StatusCode, Request, HeaderMap},
    body::{Body, to_bytes}
};
use tracing::error;
use std::error::Error;
use axum::middleware;
use std::sync::Arc;
use crate::AppState;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use reqwest::Error as ReqwestError;



#[derive(Debug, Deserialize)]
pub struct LocationCallPayload {
    location: String,
    units: String,
}

#[derive(Debug, Deserialize)]
pub struct MessageCallPayload {
    message: String,
}

#[derive(Debug, Deserialize)]
pub struct AssistantPayload {
    agent_id: String,
    call_sid: String,
    called_number: String,
    caller_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct ConversationInitiationClientData {
    r#type: String,
    conversation_config_override: ConversationConfig,
    dynamic_variables: HashMap<String, Value>,
}

#[derive(Serialize, Deserialize)]
pub struct ConversationConfig {
    agent: AgentConfig,
}

#[derive(Serialize, Deserialize)]
pub struct AgentConfig {
    first_message: String,
}




pub async fn validate_elevenlabs_secret(
    headers: HeaderMap,
    request: Request<Body>,
    next: middleware::Next,
) -> Result<Response, StatusCode> {
    println!("\n=== Starting Elevenlabs Secret Validation ===");
    
    let secret_key = match std::env::var("ELEVENLABS_SERVER_URL_SECRET") {
        Ok(key) => {
            println!("✅ Successfully retrieved ELEVENLABS_SERVER_URL_SECRET");
            key
        },
        Err(e) => {
            println!("❌ Failed to get ELEVENLABS_SERVER_URL_SECRET: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    match headers.get("x-elevenlabs-secret") {
        Some(header_value) => {
            println!("🔍 Found x-elevenlabs-secret header");
            match header_value.to_str() {
                Ok(value) => {
                    if value == secret_key {
                        println!("✅ Secret validation successful");
                        Ok(next.run(request).await)
                    } else {
                        println!("❌ Invalid secret provided");
                        Err(StatusCode::UNAUTHORIZED)
                    }
                },
                Err(e) => {
                    println!("❌ Error converting header to string: {}", e);
                    Err(StatusCode::UNAUTHORIZED)
                }
            }
        },
        None => {
            println!("❌ No x-elevenlabs-secret header found");
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}


pub async fn fetch_assistant(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AssistantPayload>,
) -> Result<Json<ConversationInitiationClientData>, (StatusCode, Json<serde_json::Value>)> {
    println!("Received assistant request:");
    let agent_id = payload.agent_id;
    println!("Agent ID: {}", agent_id);
    let call_sid = payload.call_sid;
    println!("Call SID: {}", call_sid);
    let called_number = payload.called_number;
    println!("Called Number: {}", called_number);
    let caller_number = payload.caller_id;
    println!("Caller Number: {}", caller_number);

    let mut dynamic_variables = HashMap::new();
    let mut conversation_config_override = ConversationConfig {
        agent: AgentConfig {
            first_message: "Hello {{name}}!".to_string(),
        },
    };


    match state.user_repository.find_by_phone_number(&caller_number) {
        Ok(Some(user)) => {
            println!("Found user: {}, {}", user.email, user.phone_number);
            
            if user.credits <= 0.00 {

                match state.user_repository.is_credits_under_threshold(user.id) {
                    Ok(is_under) => {
                        if is_under {
                            println!("User {} credits is under threshold, attempting automatic charge", user.id);
                            // Get user information
                            if user.charge_when_under {
                                use axum::extract::{State, Path};
                                let state_clone = Arc::clone(&state);
                                tokio::spawn(async move {
                                    let _ = crate::handlers::stripe_handlers::automatic_charge(
                                        State(state_clone),
                                        Path(user.id),
                                    ).await;
                                    println!("Recharged the user successfully back up!");
                                });
                                println!("recharged the user successfully back up!");
                            }
                        }
                    },
                    Err(e) => eprintln!("Failed to check if user credits is under threshold: {}", e),
                }

                return Err((
                    StatusCode::FORBIDDEN,
                    Json(json!({
                        "error": "Insufficient credits balance",
                        "message": "Please add more credits to your account to continue on lightfriend website"
                    }))
                ));
            }

            // If user is not verified, verify them
            if !user.verified {
                if let Err(e) = state.user_repository.verify_user(user.id) {
                    println!("Error verifying user: {}", e);
                    // Continue even if verification fails
                } else {
                    conversation_config_override = ConversationConfig {
                        agent: AgentConfig {
                            first_message: "Welcome! Your number is now verified. You can add some information about yourself in the profile page. Anyways, how can I help?".to_string(),
                        },
                    };
                }
            }

            let nickname = match user.nickname {
                Some(nickname) => nickname,
                None => "".to_string()
            };
            let user_info = match user.info {
                Some(info) => info,
                None => "".to_string()
            };

            dynamic_variables.insert("name".to_string(), json!(nickname));
            dynamic_variables.insert("user_info".to_string(), json!(user_info));
            dynamic_variables.insert("user_id".to_string(), json!(user.id));

            let charge_back_threshold= std::env::var("CHARGE_BACK_THRESHOLD")
                .expect("CHARGE_BACK_THRESHOLD not set")
                .parse::<f32>()
                .unwrap_or(2.00);
            let voice_second_cost = std::env::var("VOICE_SECOND_COST")
                .expect("VOICE_SECOND_COST not set")
                .parse::<f32>()
                .unwrap_or(0.0033);

            let user_current_credits_to_threshold = user.credits - charge_back_threshold;
            let seconds_to_threshold = (user_current_credits_to_threshold / voice_second_cost) as i32;
            println!("SEconds_to_threshold: {}", seconds_to_threshold);
            // following just so it doesn't go negative although i don't think it matters
            let recharge_threshold_timestamp: i32 = (chrono::Utc::now().timestamp() as i32) + seconds_to_threshold;

            let seconds_to_zero_credits= (user.credits / voice_second_cost) as i32;
            println!("Seconds to zero credits: {}", seconds_to_zero_credits);
            let zero_credits_timestamp: i32 = (chrono::Utc::now().timestamp() as i32) + seconds_to_zero_credits as i32;

            // log usage and start call
            if let Err(e) = state.user_repository.log_usage(
                user.id,
                "call",
                None,
                None,
                None,
                Some(call_sid),
                Some("ongoing".to_string()),
                Some(recharge_threshold_timestamp),
                Some(zero_credits_timestamp),
            ) {
                eprintln!("Failed to log call usage: {}", e);
                // Continue execution even if logging fails
            }

        },
        Ok(None) => {
            println!("No user found for number: {}", caller_number);
            dynamic_variables.insert("name".to_string(), json!(""));
            dynamic_variables.insert("user_info".to_string(), json!("new user"));
        },
        Err(e) => {
            println!("Error looking up user: {}", e);
            dynamic_variables.insert("name".to_string(), json!("Guest"));
            dynamic_variables.insert("user_info".to_string(), json!({
                "error": "Database error"
            }));
        }
    }

    dynamic_variables.insert("now".to_string(), json!(format!("{}", chrono::Utc::now())));

    let payload = ConversationInitiationClientData {
        r#type: "conversation_initiation_client_data".to_string(),
        conversation_config_override,
        dynamic_variables,
    };

    Ok(Json(payload))
}


pub async fn handle_send_sms_tool_call(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
    Json(payload): Json<MessageCallPayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    println!("Received SMS send request with message: {}", payload.message);
    
    // Get user_id from query params
    let user_id_str = match params.get("user_id") {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing user_id query parameter"
                }))
            ));
        }
    };

    // Convert String to i32
    let user_id: i32 = match user_id_str.parse() {
        Ok(id) => id,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Invalid user_id format, must be an integer"
                }))
            ));
        }
    };

    // Fetch user from user_repository
    let user = match state.user_repository.find_by_id(user_id) {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": "User not found"
                }))
            ));
        }
        Err(e) => {
            error!("Error fetching user: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to fetch user"
                }))
            ));
        }
    };


    // Get conversation for the user
    let conversation = match state.user_conversations.get_conversation(&user, user.preferred_number.clone().unwrap()).await {
        Ok(conv) => conv,
        Err(e) => {
            error!("Failed to get conversation: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to get or create conversation"
                }))
            ));
        }
    };

    // Send the message using Twilio
    match crate::api::twilio_utils::send_conversation_message(
        &conversation.conversation_sid,
        &conversation.twilio_number,
        &payload.message
    ).await {
        Ok(message_sid) => {
            println!("Successfully sent SMS with SID: {}", message_sid);
            Ok(Json(json!({
                "status": "success",
                "message_sid": message_sid,
                "conversation_sid": conversation.conversation_sid
            })))
        }
        Err(e) => {
            error!("Failed to send SMS: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": "Failed to send message",
                    "details": e.to_string()
                }))
            ))
        }
    }
}


pub async fn handle_shazam_tool_call(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(params): axum::extract::Query<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    println!("Received shazam request with message");
    
    // Get user_id from query params
    let user_id_str = match params.get("user_id") {
        Some(id) => id,
        None => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Missing user_id query parameter"
                }))
            ));
        }
    };

    // Convert String to i32
    let user_id: i32 = match user_id_str.parse() {
        Ok(id) => id,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Invalid user_id format, must be an integer"
                }))
            ));
        }
    };

    // Spawn a new thread to handle the Shazam call
    let state_clone = Arc::clone(&state);
    let user_id_string = user_id.to_string();
    
    tokio::spawn(async move {
        crate::api::shazam_call::start_call_for_user(
            axum::extract::Path(user_id_string),
            axum::extract::State(state_clone),
        ).await;
    });

    Ok(Json(json!({
        "status": "success",
        "message": "Shazam call initiated",
        "user_id": user_id
    })))
}

pub async fn handle_perplexity_tool_call(
    State(_state): State<Arc<AppState>>,
    Json(payload): Json<MessageCallPayload>,
) -> Json<serde_json::Value> {
    println!("Received message: {}", payload.message);
    

    let system_prompt = "You are assisting an AI voice calling service. The questions you receive are from voice conversations where users are seeking information or help. Please note: 1. Provide clear, conversational responses that can be easily read aloud 2. Avoid using any markdown, HTML, or other markup languages 3. Keep responses concise but informative 4. Use natural language sentence structure 5. When listing multiple points, use simple numbering (1, 2, 3) or natural language transitions (First... Second... Finally...) 6. Focus on the most relevant information that addresses the user's immediate needs 7. If specific numbers, dates, or proper names are important, spell them out clearly 8. Format numerical data in a way that's easy to read aloud (e.g., twenty-five percent instead of 25%) Your responses will be incorporated into a voice conversation, so clarity and natural flow are essential.";
    
    match ask_perplexity(&payload.message, system_prompt).await {
        Ok(response) => {
            println!("Perplexity response: {}", response);
            Json(json!({
                "response": response
            }))
        },
        Err(e) => {
            error!("Error getting response from Perplexity: {}", e);
            Json(json!({
                "error": "Failed to get response from AI"
            }))
        }
    }
}


pub async fn handle_weather_tool_call(
    State(_state): State<Arc<AppState>>,
    Json(payload): Json<LocationCallPayload>,
) -> Json<serde_json::Value> {
    println!("Received weather request for location: {}, using: {}", payload.location, payload.units);
    
    match get_weather(&payload.location, &payload.units).await {
        Ok(weather_info) => {
            println!("Weather response: {}", weather_info);
            Json(json!({
                "response": weather_info
            }))
        },
        Err(e) => {
            error!("Error getting weather information: {}", e);
            Json(json!({
                "error": "Failed to get weather information",
                "details": e.to_string()
            }))
        }
    }
}

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
