use reqwest::Client;
use std::env;
use std::error::Error;
use std::sync::Arc;
use crate::AppState;
use crate::api::twilio_utils::send_conversation_message;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::api::vapi_endpoints;
use axum::{
    extract::Form,
    response::IntoResponse,
    extract::State,
    http::StatusCode,
};
use chrono::{DateTime, Utc};

use serde_json::json;
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

#[derive(Deserialize)]
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

pub async fn send_sms(to: &str, body: &str) -> Result<(), Box<dyn Error>> {
    let account_sid = env::var("TWILIO_ACCOUNT_SID")?;
    let auth_token = env::var("TWILIO_AUTH_TOKEN")?;
    let from = env::var("FIN_PHONE")?;

    let client = Client::new();
    let response = client
        .post(format!(
            "https://api.twilio.com/2010-04-01/Accounts/{}/Messages.json",
            account_sid
        ))
        .basic_auth(&account_sid, Some(&auth_token))
        .form(&[
            ("To", to),
            ("From", &from),
            ("Body", body),
        ])
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("Failed to send SMS: {}", response.status()).into());
    }

    Ok(())
}


pub async fn handle_incoming_sms(
    State(state): State<Arc<AppState>>,
    Form(payload): Form<TwilioWebhookPayload>,
) -> impl IntoResponse {
    println!("Received SMS from: {} to: {}", payload.from, payload.to);
    println!("Message: {}", payload.body);

    let result = async {
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


        // Check if user has enough IQ points
        if user.iq < 60 {
            return (
                StatusCode::BAD_REQUEST,
                axum::Json(TwilioResponse {
                    message: "Insufficient IQ points to send message. Please add more credits to continue.".to_string(),
                })
            );
        }

        // Deduct 60 IQ points for the message
        if let Err(e) = state.user_repository.update_user_iq(user.id, user.iq - 60) {
            eprintln!("Failed to update user IQ: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(TwilioResponse {
                    message: "Failed to process IQ points".to_string(),
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
        // Start with the system message
        let mut chat_messages: Vec<ChatMessage> = vec![ChatMessage {
            role: "system".to_string(),
            content: format!("You are a friendly and helpful AI assistant named lightfriend. The current date is {}. You must provide extremely concise responses (max 400 characters) while being accurate and helpful. Be direct and natural in your answers. Since users are using SMS, keep responses clear and brief. Avoid suggesting actions requiring smartphones or internet. Please note: 1. Provide clear, conversational responses that can be easily read from a small screen 2. Avoid using any markdown, HTML, or other markup languages. Use simple language and focus on the most important information first. This is what the user wants to you to know: {}. When you use tools make sure to add relevant info about the user to the tool messages so they can act accordingly.", Utc::now().format("%Y-%m-%d"), user_info),
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
        let mut final_response = match result.choices[0].finish_reason {
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

                let mut perplexity_answer = String::new();
                for tool_call in tool_calls {
                    println!("Processing tool call: {:?}", tool_call);
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
                                perplexity_answer = answer;
                                break; // Use the first successful perplexity answer
                            }
                            Err(e) => {
                                eprintln!("Failed to get perplexity answer: {}", e);
                                continue;
                            }
                        };
                    }
                }

                // Make a second call to Openrouter with the perplexity answer
                if !perplexity_answer.is_empty() {
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
                            follow_up_messages.push(chat_completion::ChatCompletionMessage {
                                role: chat_completion::MessageRole::tool,
                                content: chat_completion::Content::Text(perplexity_answer.clone()),
                                name: None,
                                tool_calls: None,
                                tool_call_id: Some(tool_call.id.clone()),
                            });
                        }
                    }

                    println!("Making follow-up request to model with Perplexity answer");
                    let follow_up_req = chat_completion::ChatCompletionRequest::new(
                        GPT4_O.to_string(),
                        follow_up_messages,
                    )
                    .max_tokens(250); // Consistent token limit for follow-up messages
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
                            format!("Based on my research: {}", perplexity_answer)
                        }
                    }
                } else {
                    "I apologize, but I couldn't find the information you requested.".to_string()
                }
            }
            Some(chat_completion::FinishReason::length) => {
                "I apologize, but my response was too long. Could you please ask your question in a more specific way?".to_string()
            }
            Some(chat_completion::FinishReason::content_filter) => {
                "I apologize, but I cannot provide an answer to that question due to content restrictions.".to_string()
            }
            Some(chat_completion::FinishReason::null) => {
                "I apologize, but something went wrong while processing your request.".to_string()
            }
        };

        if user.iq - 60 > 60 && user.iq - 60 < 120 {
            final_response = format!("{}\n\n(enough IQ left for 1 free message)", final_response);
        }

        // Send the final response to the conversation
        match send_conversation_message(&conversation.conversation_sid, &conversation.twilio_number,&final_response).await {
            Ok(_) => (
                StatusCode::OK,
                axum::Json(TwilioResponse {
                    message: "Message sent successfully".to_string(),
                })
            ),
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
    }.await;

    result
}

