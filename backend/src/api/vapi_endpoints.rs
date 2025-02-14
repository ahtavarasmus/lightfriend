
use axum::{
    Json,
    extract::State,
    response::Response,
    http::{StatusCode, Request, HeaderMap},
    body::Body
};
use axum::middleware;
use std::future::Future;
use std::sync::Arc;
use crate::AppState;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::error::Error;
use tracing::{error, info};

pub async fn validate_vapi_secret(
    headers: HeaderMap,
    request: Request<Body>,
    next: middleware::Next<Body>,
) -> Result<Response, StatusCode> {
    println!("\n=== Starting VAPI Secret Validation ===");
    
    let secret_key = match std::env::var("VAPI_SERVER_URL_SECRET") {
        Ok(key) => {
            println!("✅ Successfully retrieved VAPI_SERVER_URL_SECRET");
            key
        },
        Err(e) => {
            println!("❌ Failed to get VAPI_SERVER_URL_SECRET: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    match headers.get("x-vapi-secret") {
        Some(header_value) => {
            println!("🔍 Found x-vapi-secret header");
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
            println!("❌ No x-vapi-secret header found");
            Err(StatusCode::UNAUTHORIZED)
        }
    }
}

pub async fn vapi_server(
    Json(payload): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    tracing::info!("Debug Payload Received: {:#?}", payload);
    println!("Payload: {:#?}",payload);
    Json(payload)
}

use crate::api::vapi_dtos::{MessageResponse, ServerResponse};

pub async fn handle_phone_call_event_print(Json(payload): Json<serde_json::Value>) -> Json<serde_json::Value> {
    match serde_json::from_value::<MessageResponse>(payload) {
        Ok(event) => {
            let phone_number = event.get_phone_number();
            let request_type = event.get_request_type();
            
            println!("Received call from: {:?}", phone_number);
            println!("Request type: {:?}", request_type);

            if let Some(tool_calls) = event.get_tool_calls() {
                println!("Tool Calls:");
                for tool_call in tool_calls {
                    println!("  - Tool Call ID: {}", tool_call.id);
                    println!("    Type: {}", tool_call.r#type);
                    println!("    Function: {}", tool_call.function.name);
                    println!("    Arguments: {:?}", tool_call.function.arguments);
                }
            }
            
            Json(json!({
                "status": "success",
                "message": "Phone number and request type extracted successfully",
                "phone_number": phone_number,
                "request_type": request_type
            }))
        }
        Err(e) => {
            eprintln!("Error parsing payload: {}", e);
            Json(json!({
                "status": "error",
                "message": "Invalid payload format",
                "error": format!("{:#?}", e)
            }))
        }
    }
}
   
pub async fn handle_tool_calls(event: &MessageResponse, state: &Arc<AppState>) -> Json<serde_json::Value> {
    println!("\n=== Starting handle_tool_calls ===");
    println!("📥 Incoming Event: {:#?}", event);

    
    let mut results = Vec::new();
    
    if let Some(tool_calls) = event.get_tool_calls() {
        println!("\n🔧 Found {} tool calls to process", tool_calls.len());
        println!("📋 Tool calls details: {:#?}", tool_calls);
        
        for (index, tool_call) in tool_calls.iter().enumerate() {
            println!("\n🔄 Processing tool call #{}", index + 1);
            println!("🆔 Tool Call ID: {}", tool_call.id);
            println!("📝 Function Name: {}", tool_call.function.name);
            println!("⚙️ Arguments: {:#?}", tool_call.function.arguments);
        }
         
        for tool_call in tool_calls {
            println!("Processing tool call: {:#?}", tool_call);
            let mut tool_result = json!({
                "name": tool_call.function.name,
                "toolCallId": tool_call.id,
                "result": null,
                "error": null
            });

            match tool_call.function.name.as_str() {
                "AskPerplexity" => {
                    println!("\n🤖 Handling AskPerplexity function");
                    if let Some(arguments) = tool_call.function.arguments.as_object() {
                        
                        let message = arguments.get("message").and_then(|v| v.as_str()).unwrap_or("");
                        println!("📤 Message to Perplexity: {}", message);
                        println!("🔄 Making API request to Perplexity...");
                        match ask_perplexity(message).await {
                            Ok(result) => {
                                println!("✅ Perplexity API call successful!");
                                println!("📥 Response received: {}", result);
                                tool_result["result"] = json!(result);
                            },
                            Err(e) => {
                                let error_msg = format!("❌ Error making Perplexity request: {}", e);
                                eprintln!("🚨 {}", error_msg);
                                tool_result["error"] = json!(error_msg);
                            }
                        }
                    } else {
                        tool_result["error"] = json!("Invalid arguments format");
                    }
                },
                "system-command" => {
                    println!("\n⚙️ Handling system-command function");
                    println!("❌ System commands are not implemented");
                    tool_result["error"] = json!("System command not implemented");
                },
                _ => {
                    let error_msg = format!("❌ Unknown function type: {}", tool_call.function.name);
                    println!("🚨 {}", error_msg);
                    tool_result["error"] = json!(error_msg);
                }
            }

            println!("\n📊 Tool result: {:#?}", tool_result);
            results.push(tool_result);
        }
        
        println!("\n✅ All tool calls processed successfully");
        println!("📤 Returning results: {:#?}", results);
        Json(json!({
            "results": results,
            "error": null
        }))
    } else {
        println!("\n⚠️ No tool calls found in event");
        Json(json!({
            "results": [],
            "error": "No tool calls found"
        }))
    }
}

pub async fn handle_assistant_request(event: &MessageResponse, state: &Arc<AppState>) -> Json<serde_json::Value> {
    println!("Entering handle_assistant_request");
    println!("Event: {:#?}", event);
    
    if let Some(phone_number) = event.get_phone_number() {
        println!("Found phone number: {}", phone_number);
        
        match state.user_repository.find_by_phone_number(&phone_number) {
            Ok(Some(user)) => {
                println!("User found for phone number: {}", phone_number);
                
                if user.verified {
                    let nickname = user.nickname.unwrap_or_else(|| "".to_string());
                    println!("User nickname: {}", nickname);
                    
                    if user.iq <= 0 {
                        let response = json!({
                            "messageResponse": {
                                "assistantId": &std::env::var("ASSISTANT_ID").expect("ASSISTANT_ID must be set"),
                                "assistantOverrides": {
                                    "firstMessage": "Hey, I don't have enough IQ to talk, you can give me more by visiting lightfriend website.",
                                    "variableValues": {
                                        "name": nickname
                                    },
                                    "maxDurationSeconds": 10,
                                }
                            }
                        });
                        println!("Returning response: {:#?}", response);
                        Json(response)
                    } else {
                        let response = json!({
                            "messageResponse": {
                                "assistantId": &std::env::var("ASSISTANT_ID").expect("ASSISTANT_ID must be set"),
                                "assistantOverrides": {
                                    "variableValues": {
                                        "name": nickname,
                                        "user_info": user.info,
                                    },
                                    "maxDurationSeconds": user.iq,
                                }
                            }
                        });
                    println!("Returning response: {:#?}", response);
                    Json(response)
                                        }
                } else {
                    println!("Verifying user: {}", phone_number);
                    
                    match state.user_repository.verify_user(user.id) {
                        Ok(_) => {
                            println!("User verified successfully");
                            let nickname = user.nickname.unwrap_or_else(|| "".to_string());
                            // TODO: make assistant explain the service
                            // cap the call length to users credits and make the assistant say that
                            let response = json!({
                                "messageResponse": {
                                    "assistantId": &std::env::var("ASSISTANT_ID").expect("ASSISTANT_ID must be set"),
                                    "assistantOverrides": {
                                        "firstMessage": format!("Welcome {}! Your account has been verified! Anyways, how can I help?", nickname),
                                        "variableValues": {
                                            "name": nickname,
                                            "user_info": user.info,
                                        },
                                        "maxDurationSeconds": std::cmp::max(user.iq, 10),
                                    }
                                }
                            });
                            println!("Returning response: {:#?}", response);
                            Json(response)
                        },
                        Err(e) => {
                            println!("Error verifying user: {}", e);
                            let resp = json!({
                                "messageResponse": {
                                    "error": "Sorry there was an error verifying your account"
                                }
                            });
                            println!("Returning error response: {:#?}", resp);
                            Json(resp)
                        }
                    }
                }
            },
            Ok(None) => {
                println!("No user found for phone number: {}", phone_number);
                let response = json!({
                    "messageResponse": {
                        "assistantId": &std::env::var("ASSISTANT_ID").expect("ASSISTANT_ID must be set"),
                        "assistantOverrides": {
                            "firstMessage": "Hey, I didn't find your phone number. Make sure you have added plus infront of it in the lightfriend settings.",
                            "maxDurationSeconds": 10,
                        }
                    }
                });
                println!("Returning response: {:#?}", response);
                Json(response)
            },
            Err(e) => {
                println!("Database error while finding user: {}", e);
                let resp = json!({
                    "messageResponse": {
                        "error": "Database error when fetching user",
                    }
                });
                println!("Returning response: {:#?}", resp);
                Json(resp)
            }
        }
    } else {
        println!("No phone number found in event");
        let resp = json!({
            "error": "Internal error, no phone number was included in the payload",
        });
        println!("Returning response: {:#?}", resp);
        Json(resp)
    }
}


pub async fn handle_end_of_call_report(
    event: &MessageResponse,
    state: &Arc<AppState>
) -> Json<serde_json::Value> {
    println!("\n=== Processing End of Call Report ===");
    
    let phone_number = event.get_phone_number().unwrap_or_default();
    println!("📱 Phone Number: {}", phone_number);
    
    if let Some(analysis) = &event.message.analysis {
        println!("📊 Call Analysis:");
        println!("Success Evaluation: {}", analysis.success_evaluation);
        println!("Summary: {}", analysis.summary);
    }

    if let Some(duration) = event.message.duration_seconds {
        println!("⏱️ Call Duration: {:.2} seconds", duration);
        
        // Update user's remaining credits based on call duration
        if let Ok(Some(mut user)) = state.user_repository.find_by_phone_number(&phone_number) {
            let new_iq = user.iq.saturating_sub(duration as i32);
            
            match state.user_repository.update_user_iq(user.id, new_iq) {
                Ok(_) => {
                    println!("✅ Updated user IQ to: {}", new_iq);
                },
                Err(e) => {
                    error!("Failed to update user IQ: {}", e);
                    return Json(json!({
                        "status": "error",
                        "message": "Failed to update user credits",
                        "error": e.to_string()
                    }));
                }
            }
        }
    }

    Json(json!({
        "status": "success",
        "message": "End of call report processed successfully"
    }))
}



pub async fn handle_status_update(event: &MessageResponse) -> ServerResponse {

    println!("Processing status update");

    // Add your status update handling logic here
    
    ServerResponse {
        status: "success".to_string(),
        message: "Status update received".to_string(),
        data: None,
    }
}

pub async fn handle_phone_call_event(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    println!("Received payload: {:#?}", payload);
    match serde_json::from_value::<MessageResponse>(payload) {
        Ok(event) => {
            let request_type = event.get_request_type();
            
            println!("Received call from: {:?}", event.get_phone_number());
            println!("Request type: {:?}", request_type);

            match request_type.as_str() {
                "tool-calls" => {
                    println!("Handling the tool-calls");
                    handle_tool_calls(&event, &state).await
                },
                "assistant-request" => {
                    println!("Calling handle_assistant_request for assistant request");
                    handle_assistant_request(&event, &state).await
                },
                "end-of-call-report" => {
                    println!("Handling end of call report");
                    handle_end_of_call_report(&event, &state).await
                },
                //"status-update" => {
                 //   handle_status_update(&event).await
                //},
                _ => Json(json!({
                    "status": "error",
                    "message": "Unknown request type",
                    "data": null
                }))
            }
        }
        Err(e) => {
            eprintln!("Error parsing payload: {}", e);
            Json(json!({
                "status": "error",
                "message": "Error parsing payload",
                "data": null
            }))

        }
    }
}




pub async fn ask_perplexity(message: &str) -> Result<String, reqwest::Error> {
    let api_key = std::env::var("PERPLEXITY_API_KEY").expect("PERPLEXITY_API_KEY must be set");
    let client = reqwest::Client::new();
    
    let payload = json!({
        "model": "sonar",
        "messages": [
                {
                    "role": "system",
                    "content": "You are assisting an AI voice calling service. The questions you receive are from voice conversations where users are seeking information or help. Please note: 1. Provide clear, conversational responses that can be easily read aloud 2. Avoid using any markdown, HTML, or other markup languages 3. Keep responses concise but informative 4. Use natural language sentence structure 5. When listing multiple points, use simple numbering (1, 2, 3) or natural language transitions (First... Second... Finally...) 6. Focus on the most relevant information that addresses the user's immediate needs 7. If specific numbers, dates, or proper names are important, spell them out clearly 8. Format numerical data in a way that's easy to read aloud (e.g., twenty-five percent instead of 25%) Your responses will be incorporated into a voice conversation, so clarity and natural flow are essential."
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

    let result = response.text().await?;
    println!("{}", result);
    Ok(result)
}
