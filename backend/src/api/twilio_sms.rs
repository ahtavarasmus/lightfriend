use reqwest::Client;
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
};
use chrono::Utc;

use crate::api::vapi_endpoints::ask_perplexity;

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

#[derive(Serialize)]
struct TwilioResponse {
    #[serde(rename = "Message")]
    message: String,
}

pub async fn send_shazam_answer_to_user(
    state: Arc<crate::shazam_call::ShazamState>,
    user_id: i32,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
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
    let conversation = state
        .user_conversations
        .get_conversation(&user, sender_number.to_string())
        .await?;
    tracing::info!("Retrieved conversation with SID: {}", conversation.conversation_sid);

    tracing::info!("Sending message to conversation {}", conversation.conversation_sid);
    crate::api::twilio_utils::send_conversation_message(
        &conversation.conversation_sid,
        &sender_number,
        message,
    )
    .await
    .map_err(|e| {
        eprintln!("Failed to send message to {} (conversation {}): {}", user.phone_number, conversation.conversation_sid, e);
        e
    })?;

    tracing::info!("Successfully sent Shazam answer to user {} at {}", user_id, user.phone_number);
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
) -> impl IntoResponse {
    println!("Received SMS from: {} to: {}", payload.from, payload.to);
    println!("Message: {}", payload.body);

    // Spawn a background task to handle the processing
    tokio::spawn(async move {
        let result = process_sms(state.clone(), payload.clone()).await;
        
        if result.0 != StatusCode::OK {
            eprintln!("Background SMS processing failed" );
        }
    });

    // Immediately return a success response to Twilio
    (
        StatusCode::OK,
        axum::Json(TwilioResponse {
            message: "Message received, processing in progress".to_string(),
        })
    )
}

async fn process_sms(state: Arc<AppState>, payload: TwilioWebhookPayload) -> (StatusCode, axum::Json<TwilioResponse>) {
    let user = match state.user_repository.find_by_phone_number(&payload.from) {
        Ok(Some(user)) => user,
        Ok(None) => return (
            StatusCode::NOT_FOUND,
            axum::Json(TwilioResponse {
                message: "User not found".to_string(),
            })
        ),
        Err(e) => {
            eprintln!("Database error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
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
    println!("{:#?}",messages);

    let user_info = match user.info {
        Some(info) => info,
        None => "".to_string()
    };
    // Start with the system message
    let mut chat_messages: Vec<ChatMessage> = vec![ChatMessage {
        role: "system".to_string(),
        content: format!("You are a friendly and helpful AI assistant named lightfriend. The current date is {}. You must provide extremely concise responses (max 400 characters) while being accurate and helpful. Be direct and natural in your answers. Since users are using SMS, keep responses clear and brief. Avoid suggesting actions requiring smartphones or internet. Do not ask for confirmation to use tools. If there is even slightest hint that they could be helpful, use them immediately. Please note: 1. Provide clear, conversational responses that can be easily read from a small screen 2. Avoid using any markdown, HTML, or other markup languages. Use simple language and focus on the most important information first. This is what the user wants to you to know: {}. When you use tools make sure to add relevant info about the user to the tool call so they can act accordingly.", Utc::now().format("%Y-%m-%d"), user_info),
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

    // Print formatted messages for debugging
    for msg in &chat_messages {
        println!("Formatted message - Role: {}, Content: {}", msg.role, msg.content);
    }

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



    // Define tools
    let tools = vec![chat_completion::Tool {
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
            println!("Direct response from model: {}", resp);
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
                    println!("Raw arguments: {}", arguments);
                    let c: PerplexityQuestion = match serde_json::from_str(arguments) {
                        Ok(q) => q,
                        Err(e) => {
                            eprintln!("Failed to parse perplexity question: {}", e);
                            continue;
                        }
                    };
                    let query = format!("User info: {}. Query: {}", user_info, c.query);
                    println!("question for perplexity: {}", query);
                    println!("Calling Perplexity API with query: {}", query);

                    let sys_prompt = format!("You are assisting an AI text messaging service. The questions you receive are from text messaging conversations where users are seeking information or help. Please note: 1. Provide clear, conversational responses that can be easily read from a small screen 2. Avoid using any markdown, HTML, or other markup languages 3. Keep responses concise but informative 4. When listing multiple points, use simple numbering (1, 2, 3) 5. Focus on the most relevant information that addresses the user's immediate needs. This is what you should know about the user who this information is going to in their own words: {}", user_info);
                    match ask_perplexity(&query, &sys_prompt).await {
                        Ok(answer) => {
                            println!("Successfully received Perplexity answer");
                            println!("Perplexity response: {}", answer);
                            tool_answers.insert(tool_call_id, answer);
                        }
                        Err(e) => {
                            eprintln!("Failed to get perplexity answer: {}", e);
                            continue;
                        }
                    };
                } else if name == "get_weather" {
                    println!("Executing get_weather tool call");
                    println!("Raw arguments: {}", arguments);
                    let c: WeatherQuestion = match serde_json::from_str(arguments) {
                        Ok(q) => q,
                        Err(e) => {
                            eprintln!("Failed to parse calendar question: {}", e);
                            continue;
                        }
                    };
                    let location= c.location;
                    let units= c.units;
                    println!("location for weather: {}", location);
                    println!("units for weather: {}", units);

                    match crate::api::elevenlabs::get_weather(&location, &units).await {
                        Ok(answer) => {
                            println!("Successfully received weather answer");
                            println!("Weather response: {}", answer);
                            tool_answers.insert(tool_call_id, answer);
                        }
                        Err(e) => {
                            eprintln!("Failed to get weather answer: {}", e);
                            continue;
                        }
                    };
                } else if name == "use_shazam" {
                    println!("Executing use_shazam tool call");
                    let user_id = user.id;
                    let state_clone = state.clone();
                    tokio::spawn(async move {
                        crate::api::shazam_call::start_call_for_user(
                            axum::extract::Path(user_id.to_string()),
                            axum::extract::State(state_clone),
                        ).await;
                    });
                    tool_answers.insert(tool_call_id, "Shazam initiated. Lightfriend should be calling you now. Song name will be texted to you. Say this in the final response.".to_string());

                } else if name == "calendar" {
                    println!("Executing calendar tool call");
                    println!("Raw arguments: {}", arguments);
                    let c: CalendarTimeFrame = match serde_json::from_str(arguments) {
                        Ok(q) => q,
                        Err(e) => {
                            eprintln!("Failed to parse calendar question: {}", e);
                            continue;
                        }
                    };
                    // Parse the start and end times into DateTime<Utc>
                    let start_time = match chrono::DateTime::parse_from_rfc3339(&c.start) {
                        Ok(dt) => dt.with_timezone(&chrono::Utc),
                        Err(e) => {
                            eprintln!("Failed to parse start time: {}", e);
                            continue;
                        }
                    };
                    
                    let end_time = match chrono::DateTime::parse_from_rfc3339(&c.end) {
                        Ok(dt) => dt.with_timezone(&chrono::Utc),
                        Err(e) => {
                            eprintln!("Failed to parse end time: {}", e);
                            continue;
                        }
                    };

                    let timeframe = crate::handlers::google_calendar::TimeframeQuery {
                        start: start_time,
                        end: end_time,
                    };
                    println!("starting time: {}", start_time);
                    println!("endint time: {}", end_time);

                    match crate::handlers::google_calendar::fetch_calendar_events(&state, user.id, timeframe).await {
                        Ok(events) => {
                            println!("Successfully fetched {} calendar events", events.len());
                            
                            // Format events into a readable response
                            let mut formatted_response = String::new();
                            
                            if events.is_empty() {
                                formatted_response = "No events found for this time period.".to_string();
                            } else {
                                for (i, event) in events.iter().enumerate() {
                                    let summary = event.summary.as_deref().unwrap_or("Untitled Event");
                                    
                                    let start_time = event.start.date_time
                                        .map(|dt| dt.format("%I:%M %p").to_string())
                                        .or_else(|| event.start.date.as_ref().map(|d| "All day".to_string()))
                                        .unwrap_or_else(|| "Unknown time".to_string());
                                        
                                    let start_date = event.start.date_time
                                        .map(|dt| dt.format("%b %d").to_string())
                                        .or_else(|| event.start.date.as_ref().map(|d| d.to_string()))
                                        .unwrap_or_else(|| "Unknown date".to_string());

                                    if i == 0 {
                                        formatted_response.push_str(&format!("{}. {} on {} at {}", i + 1, summary, start_date, start_time));
                                    } else {
                                        formatted_response.push_str(&format!(", {}. {} on {} at {}", i + 1, summary, start_date, start_time));
                                    }
                                }
                            }
                            
                            tool_answers.insert(tool_call_id, formatted_response);
                        }
                        Err(e) => {
                            let error_message = match e {
                                crate::handlers::google_calendar::CalendarError::NoConnection => 
                                    "You need to connect your Google Calendar first. Visit the website to connect.",
                                crate::handlers::google_calendar::CalendarError::TokenError(_) => 
                                    "Your calendar connection needs to be renewed. Please reconnect on the website.",
                                _ => "Failed to fetch calendar events. Please try again later.",
                            };
                            tool_answers.insert(tool_call_id, error_message.to_string());
                            eprintln!("Failed to fetch calendar events: {:?}", e);
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
                    println!("Final response: {}", response);
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
                let message_credits_cost = std::env::var("MESSAGE_COST")
                    .expect("MESSAGE_COST not set")
                    .parse::<f32>()
                    .unwrap_or(0.20);

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

                let message_credits_cost = std::env::var("MESSAGE_COST")
                    .expect("MESSAGE_COST not set")
                    .parse::<f32>()
                    .unwrap_or(0.20);

                // Log the SMS usage
                if let Err(e) = state.user_repository.log_usage(
                    user.id,
                    "sms",
                    Some(message_credits_cost),  // credits points used
                    Some(true), // Success
                    None,
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

