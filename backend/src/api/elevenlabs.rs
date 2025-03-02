use axum::{
    Json,
    extract::State,
    response::Response,
    http::{StatusCode, Request, HeaderMap},
    body::Body
};
use axum::middleware;
use std::sync::Arc;
use crate::AppState;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;


#[derive(Deserialize)]
pub struct ElevenLabsResponse {
    pub status: String,
    pub metadata: CallMetaData,
    pub analysis: Option<Analysis>,
    pub conversation_initiation_client_data: CallInitiationData,
}

#[derive(Deserialize)]
pub struct Analysis {
    pub call_successful: String, // success, failure, unknown
    pub transcript_summary: String,
}

#[derive(Deserialize)]
pub struct CallMetaData {
    pub call_duration_secs: i32,
}

#[derive(Deserialize)]
pub struct CallInitiationData {
    pub dynamic_variables: DynVariables,
}

#[derive(Deserialize)]
pub struct DynVariables {
    pub user_id: Option<String>,
}



#[derive(Debug, Deserialize)]
pub struct PhoneCallPayload {
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




use std::error::Error;
use tracing::error;


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




pub async fn handle_perplexity_tool_call(
    State(_state): State<Arc<AppState>>,
    Json(payload): Json<PhoneCallPayload>,
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
