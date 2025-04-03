use reqwest::Client;
use crate::handlers::imap_handlers::ImapError;
use std::env;
use std::error::Error;
use std::sync::Arc;
use crate::AppState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use axum::{
    extract::Form,
    response::IntoResponse,
    extract::State,
    http::StatusCode,
    Json,
};
use chrono::Utc;


use openai_api_rs::v1::{
    chat_completion,
    types,
    api::OpenAIClient,
    common::GPT4_O,
};


#[derive(Clone, Debug, Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct TwilioMessageResponse {
    sid: String,
    conversation_sid: String,
    body: String,
    author: String,
}

#[derive(Debug, Deserialize)]
struct TwilioMessagesResponse {
    messages: Vec<TwilioMessageResponse>,
}

async fn fetch_conversation_messages(conversation_sid: &str) -> Result<Vec<TwilioMessageResponse>, Box<dyn Error>> {
    let account_sid = env::var("TWILIO_ACCOUNT_SID")?;
    let auth_token = env::var("TWILIO_AUTH_TOKEN")?;

    let client = Client::new();
    let url = format!(
        "https://conversations.twilio.com/v1/Conversations/{}/Messages",
        conversation_sid
    );

    let response = client
        .get(&url)
        .basic_auth(&account_sid, Some(&auth_token))
        .query(&[("Order", "desc"), ("PageSize", "15")])
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("Failed to fetch messages: {}", response.status()).into());
    }

    let messages_response: TwilioMessagesResponse = response.json().await?;
    Ok(messages_response.messages)
}

#[derive(Deserialize, Clone)]
pub struct TwilioWebhookPayload {
    #[serde(rename = "From")]
    from: String,
    #[serde(rename = "To")]
    to: String,
    #[serde(rename = "Body")]
    body: String,
}

#[derive(Serialize, Debug)]
pub struct TwilioResponse {
    #[serde(rename = "Message")]
    pub message: String,
}

pub async fn send_shazam_answer_to_user(
    state: Arc<crate::shazam_call::ShazamState>,
    user_id: i32,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // Add check for user's subscription status
    tracing::info!("Checking subscription status for user {}", user_id);
    
    tracing::info!("Starting send_shazam_answer_to_user for user_id: {}", user_id);
    tracing::info!("Message to send: {}", message);

    let user = match state.user_repository.find_by_id(user_id) {
        Ok(Some(user)) => {
            tracing::info!("Found user with phone number: {}", user.phone_number);
            user
        },
        Ok(None) => {
            tracing::info!("User not found with id: {}", user_id);
            return Err("User not found".into());
        },
        Err(e) => {
            eprintln!("Database error while finding user {}: {}", user_id, e);
            return Err(Box::new(e));
        },
    };
    let message_credits_cost = if user.phone_number.starts_with("+1") {
        std::env::var("MESSAGE_COST_US")
            .unwrap_or_else(|_| std::env::var("MESSAGE_COST").expect("MESSAGE_COST not set"))
            .parse::<f32>()
            .unwrap_or(0.10)
    } else {
        std::env::var("MESSAGE_COST")
            .expect("MESSAGE_COST not set")
            .parse::<f32>()
            .unwrap_or(0.15)
    };

    tracing::info!("Determining sender number for user {}", user_id);
    let sender_number = match user.preferred_number.clone() {
        Some(number) => {
            tracing::info!("Using user's preferred number: {}", number);
            number
        },
        None => {
            let number = std::env::var("SHAZAM_PHONE_NUMBER").expect("SHAZAM_PHONE_NUMBER not set");
            tracing::info!("Using default SHAZAM_PHONE_NUMBER: {}", number);
            number
        },
    };

    tracing::info!("Getting conversation for user {} with sender number {}", user_id, sender_number);
    // First check if there are any existing conversations
    let conversation = state
        .user_conversations
        .get_conversation(&user, sender_number.to_string())
        .await?;

    // Verify the conversation is still active by fetching participants
    let participants = crate::api::twilio_utils::fetch_conversation_participants(&conversation.conversation_sid).await?;
    
    if participants.is_empty() {
        tracing::error!("No active participants found in conversation {}", conversation.conversation_sid);
        return Err("No active participants in conversation".into());
    }

    // Check if the user's number is still active in the conversation
    let user_participant = participants.iter().find(|p| {
        p.messaging_binding.as_ref()
            .and_then(|b| b.address.as_ref())
            .map_or(false, |addr| addr == &user.phone_number)
    });

    if user_participant.is_none() {
        tracing::error!("User {} is no longer active in conversation {}", user.phone_number, conversation.conversation_sid);
        return Err("User is no longer active in conversation".into());
    }
    tracing::info!("Retrieved conversation with SID: {}", conversation.conversation_sid);

    tracing::info!("Sending message to conversation {}", conversation.conversation_sid);
    crate::api::twilio_utils::send_conversation_message(
        &conversation.conversation_sid,
        &conversation.twilio_number,
        message,
    )
    .await
    .map_err(|e| {
        eprintln!("Failed to send message to {} (conversation {}): {}", user.phone_number, conversation.conversation_sid, e);
        e
    })?;

    // Add a small delay to ensure message ordering
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    tracing::info!("Successfully sent Shazam answer to user {} at {} using conversation {} and number {}", 
        user_id, 
        user.phone_number, 
        conversation.conversation_sid,
        conversation.twilio_number
    );

    // Deduct credits for the message
    if let Err(e) = state.user_repository
        .update_user_credits(user.id, user.credits - message_credits_cost) {
        eprintln!("Failed to update user credits after Shazam message: {}", e);
        return Err("Failed to process credits points".into());
    }

    // Log the SMS usage
    if let Err(e) = state.user_repository.log_usage(
        user.id,
        "sms",
        Some(message_credits_cost),
        Some(true),
        Some("shazam".to_string()),
        None,
        None,
        None,
        None,
    ) {
        eprintln!("Failed to log Shazam SMS usage: {}", e);
        // Continue execution even if logging fails
    }

    /* TODO have to make this whole charging thing in a separate function at some point
    // Check if credits are under threshold and handle automatic charging
    match state.user_repository.is_credits_under_threshold(user.id) {
        Ok(is_under) => {
            if is_under && user.charge_when_under {
                println!("User {} credits is under threshold after Shazam message, attempting automatic charge", user.id);
                use axum::extract::{State, Path};
                let state_clone = Arc::clone(&state);
                tokio::spawn(async move {
                    let _ = crate::handlers::stripe_handlers::automatic_charge(
                        State(state_clone),
                        Path(user.id),
                    ).await;
                });
                println!("Initiated automatic recharge for user after Shazam message");
            }
        },
        Err(e) => eprintln!("Failed to check if user credits is under threshold after Shazam message: {}", e),
    }
    */

    Ok(())
}



pub async fn send_conversation_outmessage(
    conversation_sid: &str,
    from_number: &str,
    body: &str
) -> Result<(), Box<dyn Error>> {
    let account_sid = env::var("TWILIO_ACCOUNT_SID")?;
    let auth_token = env::var("TWILIO_AUTH_TOKEN")?;

    let client = Client::new();
    let url = format!(
        "https://conversations.twilio.com/v1/Conversations/{}/Messages",
        conversation_sid
    );

    let response = client
        .post(&url)
        .basic_auth(&account_sid, Some(&auth_token))
        .form(&[
            ("Body", body),
            ("Author", from_number),
        ])
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("Failed to send conversation message: {}", response.status()).into());
    }

    Ok(())
}




pub async fn handle_incoming_sms(
    State(state): State<Arc<AppState>>,
    Form(payload): Form<TwilioWebhookPayload>,
) -> (StatusCode, [(axum::http::HeaderName, &'static str); 1], axum::Json<TwilioResponse>) {
    println!("Received SMS from: {} to: {}", payload.from, payload.to);

    // Check for Shazam shortcut ('S' or 's')
    if payload.body.trim() == "S" || payload.body.trim() == "s" {
        println!("Shazam shortcut detected");
    let user = match state.user_repository.find_by_phone_number(&payload.from) {
        Ok(Some(user)) => {
            user
        },
            Ok(None) => return (
                StatusCode::NOT_FOUND,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                axum::Json(TwilioResponse {
                    message: "User not found".to_string(),
                })
            ),
            Err(e) => {
                eprintln!("Database error: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    axum::Json(TwilioResponse {
                        message: "Database error".to_string(),
                    })
                );
            }
        };

        // Check if user has enough credits
        let message_credits_cost = std::env::var("MESSAGE_COST")
            .expect("MESSAGE_COST not set")
            .parse::<f32>()
            .unwrap_or(0.20);

        if user.credits < message_credits_cost {
            return (
                StatusCode::BAD_REQUEST,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                axum::Json(TwilioResponse {
                    message: "Insufficient credits points to use Shazam.".to_string(),
                })
            );
        }

        // Check credits threshold and handle automatic charging
        match state.user_repository.is_credits_under_threshold(user.id) {
            Ok(is_under) => {
                if is_under && user.charge_when_under {
                    println!("User {} credits is under threshold, attempting automatic charge", user.id);
                    use axum::extract::{State, Path};
                    let state_clone = Arc::clone(&state);
                    tokio::spawn(async move {
                        let _ = crate::handlers::stripe_handlers::automatic_charge(
                            State(state_clone),
                            Path(user.id),
                        ).await;
                    });
                    println!("Initiated automatic recharge for user");
                }
            },
            Err(e) => eprintln!("Failed to check if user credits is under threshold: {}", e),
        }

        let user_id = user.id;
        let state_clone = state.clone();
        tokio::spawn(async move {
            crate::api::shazam_call::start_call_for_user(
                axum::extract::Path(user_id.to_string()),
                axum::extract::State(state_clone),
            ).await;
        });

        return (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            axum::Json(TwilioResponse {
                message: "Shazam call initiated".to_string(),
            })
        );
    }

    // Check for STOP command
    if payload.body.trim().to_uppercase() == "STOP" {
        if let Ok(Some(user)) = state.user_repository.find_by_phone_number(&payload.from) {
            if let Err(e) = state.user_repository.update_notify(user.id, false) {
                eprintln!("Failed to update notify status: {}", e);
            } else {
                return (
                    StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    axum::Json(TwilioResponse {
                        message: "You have been unsubscribed from notifications.".to_string(),
                    })
                );
            }
        }
    }

    // Spawn a background task to handle the processing
    tokio::spawn(async move {
        let result = process_sms(state.clone(), payload.clone()).await;
        
        if result.0 != StatusCode::OK {
            eprintln!("Background SMS processing failed with status: {:?}", result.0);
            eprintln!("Error response: {:?}", result.1);
        }
    });

    // Immediately return a success response to Twilio
    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        axum::Json(TwilioResponse {
            message: "Message received, processing in progress".to_string(),
        })
    )
}

async fn process_sms(state: Arc<AppState>, payload: TwilioWebhookPayload) -> (StatusCode, axum::Json<TwilioResponse>) {

    let user = match state.user_repository.find_by_phone_number(&payload.from) {
        Ok(Some(user)) => {
            tracing::info!("Found user with ID: {} for phone number: {}", user.id, payload.from);
            user
        },
        Ok(None) => {
            tracing::error!("No user found for phone number: {}", payload.from);
            return (
                StatusCode::NOT_FOUND,
                axum::Json(TwilioResponse {
                    message: "User not found".to_string(),
                })
            );
        },
        Err(e) => {
            tracing::error!("Database error while finding user for phone number {}: {}", payload.from, e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(TwilioResponse {
                    message: "Database error".to_string(),
                })
            );
        }
    };
    let auth_user = crate::handlers::auth_middleware::AuthUser {
        user_id: user.id, 
        is_admin: false,
    };

    // Check if user has enough credits
    let message_credits_cost = std::env::var("MESSAGE_COST")
        .expect("MESSAGE_COST not set")
        .parse::<f32>()
        .unwrap_or(0.20);

    if user.credits < message_credits_cost {
        return (
            StatusCode::BAD_REQUEST,
            axum::Json(TwilioResponse {
                message: "Insufficient credits points to send message.".to_string(),
            })
        );
    }

    let conversation = match state.user_conversations.get_conversation(&user, payload.to).await {
        Ok(conv) => conv,
        Err(e) => {
            eprintln!("Failed to ensure conversation exists: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(TwilioResponse {
                    message: "Failed to create conversation".to_string(),
                })
            );
        }
    };

    // Fetch conversation messages
    let messages = match fetch_conversation_messages(&conversation.conversation_sid).await {
        Ok(msgs) => msgs,
        Err(e) => {
            eprintln!("Failed to fetch conversation messages: {}", e);
            Vec::new()
        }
    };

    let user_info = match user.info {
        Some(info) => info,
        None => "".to_string()
    };

    // Get timezone from user info or default to UTC
    let timezone_str = match user.timezone {
        Some(ref tz) => tz.as_str(),
        None => "UTC",
    };

    // Get timezone offset using jiff
    let (hours, minutes) = match crate::api::elevenlabs::get_offset_with_jiff(timezone_str) {
        Ok((h, m)) => (h, m),
        Err(_) => {
            println!("Failed to get timezone offset for {}, defaulting to UTC", timezone_str);
            (0, 0) // UTC default
        }
    };

    // Format offset string (e.g., "+02:00" or "-05:30")
    let offset = format!("{}{:02}:{:02}", 
        if hours >= 0 { "+" } else { "-" },
        hours.abs(),
        minutes.abs()
    );

    // Start with the system message
    let mut chat_messages: Vec<ChatMessage> = vec![ChatMessage {
        role: "system".to_string(),
        content: format!("You are a friendly and helpful AI assistant named lightfriend. The current date is {}. You must provide extremely concise responses (max 400 characters) while being accurate and helpful. Be direct and natural in your answers. Since users are using SMS, keep responses clear and brief. Avoid suggesting actions requiring smartphones or internet. Do not ask for confirmation to use tools. If there is even slightest hint that they could be helpful, use them immediately. Please note: 1. Provide clear, conversational responses that can be easily read from a small screen 2. Avoid using any markdown, HTML, or other markup languages. Use simple language and focus on the most important information first. This is what the user wants to you to know: {}. The user's timezone is {} with offset {}. When using tools that require time information (like calendar or email): 1. Always use RFC3339/ISO8601 format (e.g. '2024-03-23T14:30:00Z') 2. If no specific time is mentioned, use the current time for start and 24 hours ahead for end 3. Always consider the user's timezone when interpreting time-related requests 4. For 'today' queries, use 00:00 of current day as start and 23:59 as end in user's timezone 5. For 'tomorrow' queries, use 00:00 to 23:59 of next day in user's timezone. 6. Unless user references the previous query's results, you should always use the tools to fetch the latest information before answering the question. When you use tools make sure to add relevant info about the user to the tool call so they can act accordingly.", Utc::now().format("%Y-%m-%d"), user_info, timezone_str, offset),
    }];
    
    // Process the message body to remove "forget" if it exists at the start
    let processed_body = if payload.body.to_lowercase().starts_with("forget") {
        payload.body.trim_start_matches(|c: char| c.is_alphabetic()).trim().to_string()
    } else {
        payload.body.clone()
    };

    // Only include conversation history if message starts with "forget"
    if !payload.body.to_lowercase().starts_with("forget") {
        let mut history: Vec<ChatMessage> = messages.into_iter().map(|msg| {
            ChatMessage {
                role: if msg.author == "lightfriend" { "assistant" } else { "user" }.to_string(),
                content: msg.body,
            }
        }).collect();
        history.reverse();
        
        // Combine system message with conversation history
        chat_messages.extend(history);
    }

    // Add the current message with processed body
    chat_messages.push(ChatMessage {
        role: "user".to_string(),
        content: processed_body,
    });


    let mut plex_properties = HashMap::new();
    plex_properties.insert(
        "query".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The question or topic to get information about".to_string()),
            ..Default::default()
        }),
    );

    let mut weather_properties = HashMap::new();
    weather_properties.insert(
        "location".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Location of the place where we want to search the weather.".to_string()),
            ..Default::default()
        }),
    );
    weather_properties.insert(
        "units".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Units that the weather should be returned as. Should be either 'metric' or 'imperial'".to_string()),
            ..Default::default()
        }),
    );


    let mut shazam_properties = HashMap::new();
    shazam_properties.insert(
        "param".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("can be anything, won't get used anyways".to_string()),
            ..Default::default()
        }),
    );

    let mut calendar_properties = HashMap::new();
    calendar_properties.insert(
        "start".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Start time from which we start fetching the events. Should be in format: '2024-03-16T00:00:00Z'".to_string()),
            ..Default::default()
        }),
    );
    calendar_properties.insert(
        "end".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("End time for which we end fetching the events from. Should be in format: '2024-03-16T00:00:00Z'".to_string()),
            ..Default::default()
        }),
    );

    let mut imap_properties = HashMap::new();
    imap_properties.insert(
        "param".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Can be anything, will fetch last 5 emails regardless".to_string()),
            ..Default::default()
        }),
    );

    let mut specific_email_properties = HashMap::new();
    specific_email_properties.insert(
        "query".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The search query to find a specific email".to_string()),
            ..Default::default()
        }),
    );


    // Define tools
    let tools = vec![
        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: String::from("fetch_imap_emails"),
                description: Some(String::from("Fetches the last 5 emails using IMAP. Use this when user asks about their recent emails or wants to check their inbox.")),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(imap_properties),
                    required: None,
                },
            },
        },
        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: String::from("fetch_specific_email"),
                description: Some(String::from("Search and fetch a specific email based on a query. Use this when user asks about a specific email or wants to find an email about a particular topic. You must ALWAYS respond with the whole message body or summary of the body if too long. Never reply with just the subject line!")),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(specific_email_properties),
                    required: Some(vec![String::from("query")]),
                },
            },
        },
        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: String::from("ask_perplexity"),
                description: Some(String::from("Get factual or timely information about any topic")),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(plex_properties),
                    required: Some(vec![String::from("query")]),
                },
            },
        },
        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: String::from("get_weather"),
                description: Some(String::from("Used to get the current weather if user asks for it. If user doesn't give a specific location you should assume they are at home(Do NOT put location as 'home' though, you have to find it from user's info section above along with more information about the user.")),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(weather_properties),
                    required: Some(vec![String::from("location"), String::from("units")]),
                },
            },
        },
        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: String::from("use_shazam"),
                description: Some(String::from("Shazam tool identifies the song and the artist from audio clip. This tool gives the user a call which when answered can listen to the song audio and sends the song name to user as sms. This returns a shazam listener for the user. If user asks to use shazam, identify a song or ask about it, it means you have to call this tool.")),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(shazam_properties),
                    required: None,
                },
            },
        },
        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: String::from("calendar"),
                description: Some(String::from("Calendar tool fetches the user's calendar events for the specific time frame. If the user doesn't give the specific time frame assume for today and tomorrow to be the time range.")),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(calendar_properties),
                    required: Some(vec![String::from("start"), String::from("end")]),
                },
            },
        },

    ];

    let api_key = match env::var("OPENROUTER_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            eprintln!("OPENROUTER_API_KEY not set");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(TwilioResponse {
                    message: "Server configuration error".to_string(),
                })
            );
        }
    };

    let client = match OpenAIClient::builder()
        .with_endpoint("https://openrouter.ai/api/v1")
        .with_api_key(api_key)
        .build() {
            Ok(client) => client,
            Err(e) => {
                eprintln!("Failed to build OpenAI client: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    axum::Json(TwilioResponse {
                        message: "Failed to initialize AI service".to_string(),
                    })
                );
            }
        };

    println!("built client");

    // Convert ChatMessage vec into ChatCompletionMessage vec
    let completion_messages: Vec<chat_completion::ChatCompletionMessage> = chat_messages
        .into_iter()
        .map(|msg| chat_completion::ChatCompletionMessage {
            role: match msg.role.as_str() {
                "user" => chat_completion::MessageRole::user,
                "assistant" => chat_completion::MessageRole::assistant,
                "system" => chat_completion::MessageRole::system,
                _ => chat_completion::MessageRole::user, // default to user if unknown
            },
            content: chat_completion::Content::Text(msg.content),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        })
        .collect();

    let req = chat_completion::ChatCompletionRequest::new(
        GPT4_O.to_string(),
        completion_messages.clone(),
    )
    .tools(tools)
    .tool_choice(chat_completion::ToolChoiceType::Auto)
    .max_tokens(250); // This will result in responses around 400-450 characters

    println!("built request");

    let result = match client.chat_completion(req).await {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Failed to get chat completion: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(TwilioResponse {
                    message: "Failed to process your request".to_string(),
                })
            );
        }
    };

    println!("built completion");

    println!("Processing model response with finish reason: {:?}", result.choices[0].finish_reason);
    let mut fail = false;
    let mut tool_answers: HashMap<String, String> = HashMap::new(); // tool_call id and answer
    let final_response = match result.choices[0].finish_reason {
        None | Some(chat_completion::FinishReason::stop) => {
            println!("Model provided direct response (no tool calls needed)");
            // Direct response from the model
            let resp = result.choices[0].message.content.clone().unwrap_or_default();
            resp
        }
        Some(chat_completion::FinishReason::tool_calls) => {
            println!("Model requested tool calls - beginning tool execution phase");
            #[derive(Deserialize, Serialize)]
            struct PerplexityQuestion {
                query: String,
            }
            #[derive(Deserialize, Serialize)]
            struct WeatherQuestion {
                location: String,
                units: String,
            }
            #[derive(Deserialize, Serialize)]
            struct CalendarTimeFrame {
                start: String,
                end: String,
            }
            
            let tool_calls = match result.choices[0].message.tool_calls.as_ref() {
                Some(calls) => {
                    println!("Found {} tool call(s) in response", calls.len());
                    calls
                },
                None => {
                    eprintln!("No tool calls found in response despite tool_calls finish reason");
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        axum::Json(TwilioResponse {
                            message: "Failed to process your request".to_string(),
                        })
                    );
                }
            };

            for tool_call in tool_calls {
                let tool_call_id = tool_call.id.clone();
                println!("Processing tool call: {:?} with id: {:?}", tool_call, tool_call_id);
                let name = match &tool_call.function.name {
                    Some(n) => {
                        println!("Tool call function name: {}", n);
                        n
                    },
                    None => {
                        println!("Tool call missing function name, skipping");
                        continue;
                    },
                };
                let arguments = match &tool_call.function.arguments {
                    Some(args) => args,
                    None => continue,
                };
                if name == "ask_perplexity" {
                    println!("Executing ask_perplexity tool call");
                    let c: PerplexityQuestion = match serde_json::from_str(arguments) {
                        Ok(q) => q,
                        Err(e) => {
                            eprintln!("Failed to parse perplexity question: {}", e);
                            continue;
                        }
                    };
                    let query = format!("User info: {}. Query: {}", user_info, c.query);

                    let sys_prompt = format!("You are assisting an AI text messaging service. The questions you receive are from text messaging conversations where users are seeking information or help. Please note: 1. Provide clear, conversational responses that can be easily read from a small screen 2. Avoid using any markdown, HTML, or other markup languages 3. Keep responses concise but informative 4. When listing multiple points, use simple numbering (1, 2, 3) 5. Focus on the most relevant information that addresses the user's immediate needs. This is what you should know about the user who this information is going to in their own words: {}", user_info);
                    match crate::utils::tool_exec::ask_perplexity(&query, &sys_prompt).await {
                        Ok(answer) => {
                            println!("Successfully received Perplexity answer");
                            tool_answers.insert(tool_call_id, answer);
                        }
                        Err(e) => {
                            eprintln!("Failed to get perplexity answer: {}", e);
                            continue;
                        }
                    };
                } else if name == "get_weather" {
                    println!("Executing get_weather tool call");
                    let c: WeatherQuestion = match serde_json::from_str(arguments) {
                        Ok(q) => q,
                        Err(e) => {
                            eprintln!("Failed to parse calendar question: {}", e);
                            continue;
                        }
                    };
                    let location= c.location;
                    let units= c.units;

                    match crate::utils::tool_exec::get_weather(&location, &units).await {
                        Ok(answer) => {
                            println!("Successfully received weather answer");
                            tool_answers.insert(tool_call_id, answer);
                        }
                        Err(e) => {
                            eprintln!("Failed to get weather answer: {}", e);
                            continue;
                        }
                    };
                } else if name == "use_shazam" {
                    println!("Executing use_shazam tool call");
                    // Check if credits are under threshold and handle automatic charging
                    // this is because if user only uses shazam they won't be recharged since the 
                    // shazam message sending function does not do this yet(TODO)
                    match state.user_repository.is_credits_under_threshold(user.id) {
                        Ok(is_under) => {
                            if is_under && user.charge_when_under {
                                println!("User {} credits is under threshold after Shazam message, attempting automatic charge", user.id);
                                use axum::extract::{State, Path};
                                let state_clone = Arc::clone(&state);
                                tokio::spawn(async move {
                                    let _ = crate::handlers::stripe_handlers::automatic_charge(
                                        State(state_clone),
                                        Path(user.id),
                                    ).await;
                                });
                                println!("Initiated automatic recharge for user after Shazam message");
                            }
                        },
                        Err(e) => eprintln!("Failed to check if user credits is under threshold after Shazam message: {}", e),
                    }
                    let user_id = user.id;
                    let state_clone = state.clone();
                    tokio::spawn(async move {
                        crate::api::shazam_call::start_call_for_user(
                            axum::extract::Path(user_id.to_string()),
                            axum::extract::State(state_clone),
                        ).await;
                    });
                    // Return early without sending any SMS response
                    return (
                        StatusCode::OK,
                        axum::Json(TwilioResponse {
                            message: "Shazam call initiated".to_string(),
                        })
                    );
                } else if name == "fetch_imap_emails" {
                    println!("Executing fetch_imap_emails tool call");
                    let queryObj = crate::handlers::imap_handlers::FetchEmailsQuery { limit: None };
                    match crate::handlers::imap_handlers::fetch_full_imap_emails(
                        axum::extract::State(state.clone()),
                        auth_user,
                        axum::extract::Query(queryObj),
                    ).await {
                        Ok(Json(response)) => {
                            if let Some(emails) = response.get("emails") {
                                if let Some(emails_array) = emails.as_array() {
                                    let mut response = String::new();
                                    for (i, email) in emails_array.iter().rev().take(5).rev().enumerate() {
                                        let subject = email.get("subject").and_then(|s| s.as_str()).unwrap_or("No subject");
                                        let from = email.get("from").and_then(|f| f.as_str()).unwrap_or("Unknown sender");
                                        let date = email.get("date").and_then(|d| d.as_str())
                                            .and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok())
                                            .map(|d| d.format("%m/%d").to_string())
                                            .unwrap_or_else(|| "No date".to_string());
                                        let body = email.get("body").and_then(|b| b.as_str()).unwrap_or("");
                                        let snippet = if body.len() > 100 {
                                            format!("{}...", body.chars().take(100).collect::<String>())
                                        } else {
                                            body.to_string()
                                        };
                                        
                                        if i == 0 {
                                            response.push_str(&format!("{}. {} from {} ({}):\n{}", i + 1, subject, from, date, snippet));
                                        } else {
                                            response.push_str(&format!("\n\n{}. {} from {} ({}):\n{}", i + 1, subject, from, date, snippet));
                                        }
                                    }
                                    
                                    if emails_array.len() > 5 {
                                        response.push_str(&format!("\n\n(+ {} more emails)", emails_array.len() - 5));
                                    }
                                    
                                    if emails_array.is_empty() {
                                        response = "No recent emails found.".to_string();
                                    }
                                    
                                    tool_answers.insert(tool_call_id, response);
                                } else {
                                    tool_answers.insert(tool_call_id, "Failed to parse emails.".to_string());
                                }
                            } else {
                                tool_answers.insert(tool_call_id, "No emails found.".to_string());
                            }

                        }
                        Err((status, Json(error))) => {
                            let error_message = match status {
                                StatusCode::BAD_REQUEST => "No IMAP connection found. Please check your email settings.",
                                StatusCode::UNAUTHORIZED => "Your email credentials need to be updated.",
                                _ => "Failed to fetch emails. Please try again later.",
                            };
                            tool_answers.insert(tool_call_id, error_message.to_string());
                            eprintln!("Failed to fetch IMAP emails: {:?}", error);
                        }
                    }
                } else if name == "fetch_specific_email" {
                    println!("Executing fetch_specific_email tool call");
                    #[derive(Deserialize)]
                    struct EmailQuery {
                        query: String,
                    }
                    
                    let query: EmailQuery = match serde_json::from_str(arguments) {
                        Ok(q) => q,
                        Err(e) => {
                            eprintln!("Failed to parse email query: {}", e);
                            continue;
                        }
                    };

                    // First fetch previews using IMAP
                    match crate::handlers::imap_handlers::fetch_emails_imap(&state, user.id, true, None, false).await {
                        Ok(previews) => {
                            if previews.is_empty() {
                                tool_answers.insert(tool_call_id, "No emails found.".to_string());
                                continue;
                            }

                            // Create a system message for email matching
                            let sys_message = chat_completion::ChatCompletionMessage {
                                role: chat_completion::MessageRole::system,
                                content: chat_completion::Content::Text(
                                    "You are an email matcher. Given a list of email previews and a search query, \
                                    find the most relevant email. The emails are sorted from newest to oldest. \
                                    When multiple emails match the query similarly well, prefer the newest one by date and time. \
                                    Return ONLY the ID of the most relevant email, or 'NONE' if no email matches well. \
                                    Consider subject, sender, date, and snippet in your matching, with a bias towards \
                                    more recent emails.".to_string()
                                ),
                                name: None,
                                tool_calls: None,
                                tool_call_id: None,
                            };

                            // Create the content for matching, ensuring newest emails are considered first
                            let emails_json = serde_json::json!({
                                "query": query.query,
                                "emails": previews.iter().rev().map(|p| {  // Reverse to prioritize newer emails
                                    serde_json::json!({
                                        "id": p.id,
                                        "subject": p.subject,
                                        "from": p.from,
                                        "date": p.date.map(|d| d.to_rfc3339()),
                                        "snippet": p.snippet,
                                        "body": p.body
                                    })
                                }).collect::<Vec<_>>()
                            });

                            let user_message = chat_completion::ChatCompletionMessage {
                                role: chat_completion::MessageRole::user,
                                content: chat_completion::Content::Text(emails_json.to_string()),
                                name: None,
                                tool_calls: None,
                                tool_call_id: None,
                            };

                            let matching_req = chat_completion::ChatCompletionRequest::new(
                                GPT4_O.to_string(),
                                vec![sys_message, user_message],
                            ).max_tokens(50);

                            match client.chat_completion(matching_req).await {
                                Ok(matching_result) => {
                                    let email_id = matching_result.choices[0].message.content.clone().unwrap_or_default();
                                    if email_id == "NONE" {
                                        tool_answers.insert(tool_call_id, "Could not find a matching email.".to_string());
                                        continue;
                                    }

                                    // Fetch the specific email using IMAP
                                    match crate::handlers::imap_handlers::fetch_single_email_imap(
                                        &state,
                                        user.id,
                                        email_id.trim()
                                    ).await {
                                        Ok(email) => {
                                            let formatted_response = format!(
                                                "Found email: From: {}, Subject: {}, Date: {}\n\n{}",
                                                email.from.unwrap_or_else(|| "Unknown".to_string()),
                                                email.subject.unwrap_or_else(|| "No subject".to_string()),
                                                email.date.map_or("No date".to_string(), |d| d.to_rfc3339()),
                                                email.body.unwrap_or_else(|| "No content".to_string())
                                            );
                                            tool_answers.insert(tool_call_id, formatted_response);
                                        }
                                        Err(e) => {
                                            let error_message = match e {
                                                ImapError::NoConnection => "No IMAP connection found. Please check your email settings.",
                                                ImapError::CredentialsError(_) => "Your email credentials need to be updated.",
                                                ImapError::ConnectionError(msg) | ImapError::FetchError(msg) | ImapError::ParseError(msg) => {
                                                    eprintln!("Failed to fetch specific email: {}", msg);
                                                    "Failed to fetch the email. Please try again later."
                                                }
                                            };
                                            tool_answers.insert(tool_call_id, error_message.to_string());
                                        }
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Failed to match email: {}", e);
                                    tool_answers.insert(tool_call_id, "Failed to find matching email.".to_string());
                                }
                            }
                        }
                        Err(e) => {
                            let error_message = match e {
                                ImapError::NoConnection => "No IMAP connection found. Please check your email settings.",
                                ImapError::CredentialsError(_) => "Your email credentials need to be updated.",
                                ImapError::ConnectionError(msg) | ImapError::FetchError(msg) | ImapError::ParseError(msg) => {
                                    eprintln!("Failed to fetch email previews: {}", msg);
                                    "Failed to fetch emails. Please try again later."
                                }
                            };
                            tool_answers.insert(tool_call_id, error_message.to_string());
                        }
                    }
                } else if name == "calendar" {
                    println!("Executing calendar tool call");
                    let c: CalendarTimeFrame = match serde_json::from_str(arguments) {
                        Ok(q) => q,
                        Err(e) => {
                            eprintln!("Failed to parse calendar question: {}", e);
                            continue;
                        }
                    };

                    match crate::handlers::google_calendar::handle_calendar_fetching(&state, user.id, &c.start, &c.end).await {
                        Ok(Json(response)) => {
                            if let Some(events) = response.get("events") {
                                tool_answers.insert(tool_call_id, format!("Here are your calendar events: {}", events.to_string()));
                            } else {
                                tool_answers.insert(tool_call_id, "No events found for this time period.".to_string());
                            }

                        }
                        Err((status, json_error)) => {
                            let error_message = match status {
                                StatusCode::BAD_REQUEST => "No active Google Calendar connection found. Visit the website to connect.",
                                StatusCode::UNAUTHORIZED => "Your calendar connection needs to be renewed. Please reconnect on the website.",
                                _ => "Failed to fetch calendar events. Please try again later.",
                            };
                            tool_answers.insert(tool_call_id, error_message.to_string());
                            eprintln!("Failed to fetch calendar events: {:?}", json_error);
                        }
                    }
                }
            }


            let mut follow_up_messages = completion_messages.clone();
            // Add the assistant's message with tool calls
            follow_up_messages.push(chat_completion::ChatCompletionMessage {
                role: chat_completion::MessageRole::assistant,
                content: chat_completion::Content::Text(result.choices[0].message.content.clone().unwrap_or_default()),
                name: None,
                tool_calls: result.choices[0].message.tool_calls.clone(),
                tool_call_id: None,
            });
            // Add the tool response
            if let Some(tool_calls) = &result.choices[0].message.tool_calls {
                for tool_call in tool_calls {
                    let tool_answer = match tool_answers.get(&tool_call.id) {
                        Some(ans) => ans.clone(),
                        None => "".to_string(),
                    };
                    follow_up_messages.push(chat_completion::ChatCompletionMessage {
                        role: chat_completion::MessageRole::tool,
                        content: chat_completion::Content::Text(tool_answer),
                        name: None,
                        tool_calls: None,
                        tool_call_id: Some(tool_call.id.clone()),
                    });
                }
            }

            println!("Making follow-up request to model with tool call answers");
            let follow_up_req = chat_completion::ChatCompletionRequest::new(
                GPT4_O.to_string(),
                follow_up_messages,
            )
            .max_tokens(100); // Consistent token limit for follow-up messages
            println!("Follow-up request created");

            match client.chat_completion(follow_up_req).await {
                Ok(follow_up_result) => {
                    println!("Received follow-up response from model");
                    let response = follow_up_result.choices[0].message.content.clone().unwrap_or_default();
                    response
                }
                Err(e) => {
                    eprintln!("Failed to get follow-up completion: {}", e);
                    fail = true;
                    tool_answers.values().next()
                        .map(|ans| format!("Based on my research: {} (you were not charged for this message)", ans.chars().take(370).collect::<String>()))
                        .unwrap_or_else(|| "I apologize, but I encountered an error processing your request. (you were not charged for this message)".to_string())
                }
            }
        }
        Some(chat_completion::FinishReason::length) => {
            fail = true;
            "I apologize, but my response was too long. Could you please ask your question in a more specific way? (you were not charged for this message)".to_string()
        }
        Some(chat_completion::FinishReason::content_filter) => {
            fail = true;
            "I apologize, but I cannot provide an answer to that question due to content restrictions. (you were not charged for this message)".to_string()
        }
        Some(chat_completion::FinishReason::null) => {
            fail = true;
            "I apologize, but something went wrong while processing your request. (you were not charged for this message)".to_string()
        }
    };

    // Send the final response to the conversation
    match crate::api::twilio_utils::send_conversation_message(&conversation.conversation_sid, &conversation.twilio_number,&final_response).await {
        Ok(_) => {
            if !fail {
                // Deduct credits for the message
                let message_credits_cost = if user.phone_number.starts_with("+1") {
                    std::env::var("MESSAGE_COST_US")
                        .unwrap_or_else(|_| std::env::var("MESSAGE_COST").expect("MESSAGE_COST not set"))
                        .parse::<f32>()
                        .unwrap_or(0.10)
                } else {
                    std::env::var("MESSAGE_COST")
                        .expect("MESSAGE_COST not set")
                        .parse::<f32>()
                        .unwrap_or(0.15)
                };                

                if let Err(e) = state.user_repository
                    .update_user_credits(user.id, user.credits - message_credits_cost) {

                    eprintln!("Failed to update user credits: {}", e);
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        axum::Json(TwilioResponse {
                            message: "Failed to process credits points".to_string(),
                        })
                    );
                }

                // Log the SMS usage
                if let Err(e) = state.user_repository.log_usage(
                    user.id,
                    "sms",
                    Some(message_credits_cost),  // credits points used
                    Some(true), // Success
                    Some("normal sms response".to_string()),
                    None,
                    None,
                    None,
                    None,
                ) {
                    eprintln!("Failed to log SMS usage: {}", e);
                    // Continue execution even if logging fails
                }
                        
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

            }
            (
                StatusCode::OK,
                axum::Json(TwilioResponse {
                    message: "Message sent successfully".to_string(),
                })
            )
        }
        Err(e) => {
            eprintln!("Failed to send conversation message: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(TwilioResponse {
                    message: "Failed to send message".to_string(),
                })
            )
        }
    }
}

