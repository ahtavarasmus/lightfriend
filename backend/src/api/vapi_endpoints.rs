
use axum::{
    Json,
    extract::State,
    response::Response,
    http::StatusCode
};
use std::future::Future;
use std::sync::Arc;
use crate::AppState;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::error::Error;
use tracing::{error, info};


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
                    let nickname = user.nickname.unwrap_or_else(|| user.username.clone());
                    println!("User nickname: {}", nickname);
                    let response = json!({
                        "messageResponse": {
                            "assistantId": &std::env::var("ASSISTANT_ID").expect("ASSISTANT_ID must be set"),
                            "assistantOverrides": {
                                "firstMessage": format!("Hello {}!", nickname),
                                "variableValues": {
                                    "name": nickname
                                }
                            }
                        }
                    });
                    println!("Returning response: {:#?}", response);
                    Json(response)
                } else {
                    println!("Verifying user: {}", phone_number);
                    
                    match state.user_repository.verify_user(user.id) {
                        Ok(_) => {
                            println!("User verified successfully");
                            let nickname = user.nickname.unwrap_or_else(|| user.username.clone());
                            // TODO: make assistant explain the service
                            // cap the call length to users credits and make the assistant say that
                            let response = json!({
                                "messageResponse": {
                                    "assistantId": &std::env::var("ASSISTANT_ID").expect("ASSISTANT_ID must be set"),
                                    "assistantOverrides": {
                                        "firstMessage": format!("Welcome {}! Your account has been verified! Anyways, how can I help?", nickname),
                                        "variableValues": {
                                            "name": nickname
                                        }
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
                                    "assistantId": &std::env::var("ASSISTANT_ID").expect("ASSISTANT_ID must be set"),
                                    "assistantOverrides": {
                                        "firstMessage": "Sorry, there was an error verifying your account.",
                                    }
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
                let resp = json!({
                    "messageResponse": {
                        "error": "No user found with the phone number",
                    }
                });
                println!("Returning response: {:#?}", resp);
                Json(resp)
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
        "model": "llama-3.1-sonar-small-128k-online",
        "messages": [
            {
                "role": "system",
                "content": "Be precise and concise."
            },
            {
                "role": "user",
                "content": message
            }
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
