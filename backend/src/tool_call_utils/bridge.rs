use crate::AppState;
use std::sync::Arc;
use serde::Deserialize;
use axum::Json;

pub fn get_search_bridge_rooms_tool(
    service_type: &str,
) -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;

    let mut bridge_search_properties = HashMap::new();
    bridge_search_properties.insert(
        "search_term".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(format!("Search term to find {} rooms/contacts", capitalize(service_type))),
            ..Default::default()
        }),
    );

    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from(format!("search_{}_rooms", service_type)),
            description: Some(String::from(format!("Searches for {} rooms/contacts by name.", capitalize(service_type)))),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(bridge_search_properties),
                required: Some(vec![String::from("search_term")]),
            },
        },
    }
}

pub fn get_fetch_bridge_room_messages_tool(
    service_type: &str,
) -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;

    let mut bridge_room_messages_properties = HashMap::new();
    bridge_room_messages_properties.insert(
        "chat_name".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(format!("The name of the {} chat/room to fetch messages from", capitalize(service_type))),
            ..Default::default()
        }),
    );
    bridge_room_messages_properties.insert(
        "limit".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Number),
            description: Some("Optional: Maximum number of messages to fetch (default: 20)".to_string()),
            ..Default::default()
        }),
    );


    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from(format!("fetch_{}_room_messages", service_type).as_str()),
            description: Some(String::from(format!("Fetches messages from a specific {} chat/room. Use this when user asks about messages from a specific {} contact or group. You should return the latest messages that person or chat has sent to the user.", capitalize(service_type), capitalize(service_type)).as_str())),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(bridge_room_messages_properties),
                required: Some(vec![String::from("chat_name")]),
            },
        },
    }
}

pub fn get_fetch_bridge_messages_tool(
    service_type: &str,
) -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;

    let mut bridge_messages_properties = HashMap::new();
    bridge_messages_properties.insert(
        "start".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Start time in RFC3339 format in UTC (e.g., '2024-03-16T00:00:00Z')".to_string()),
            ..Default::default()
        }),
    );

    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from(format!("fetch_{}_messages", service_type).as_str()),
            description: Some(String::from(format!("Fetches recent {} messages. Use this when user asks about their {} messages or conversations.", capitalize(service_type), capitalize(service_type)).as_str())),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(bridge_messages_properties),
                required: Some(vec![String::from("start")]),
            },
        },
    }
}

pub fn get_send_bridge_message_tool(
    service_type: &str,
) -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;

    let mut bridge_send_properties = HashMap::new();
    bridge_send_properties.insert(
        "chat_name".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The chat name or room name to send the message to. Doesn't have to be exact since fuzzy search is used.".to_string()),
            ..Default::default()
        }),
    );
    bridge_send_properties.insert(
        "message".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The message content to send".to_string()),
            ..Default::default()
        }),
    );

    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from(format!("send_{}_message", service_type).as_str()),
            description: Some(String::from(format!("Sends a {} message to a specific chat. This tool will first make a confirmation message for the user, which they can then confirm or not. The chat_name will be used to fuzzy search for the actual chat name and then confirmed with the user.", capitalize(service_type)).as_str())),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(bridge_send_properties),
                required: Some(vec![String::from("chat_name"), String::from("message")]),
            },
        },
    }
}

#[derive(Deserialize)]
pub struct BridgeSendArgs {
    pub chat_name: String,
    pub message: String,
}

#[derive(Deserialize)]
pub struct BridgeSearchArgs {
    pub search_term: String,
}

#[derive(Deserialize)]
pub struct BridgeRoomArgs {
    pub chat_name: String,
    pub limit: Option<u64>,
}

#[derive(Deserialize)]
pub struct BridgeTimeFrame {
    pub start: String,
}

use crate::utils::bridge::capitalize;


pub async fn handle_send_bridge_message(
    state: &Arc<AppState>,
    service_type: &str,
    user_id: i32,
    args: &str,
    user: &crate::models::user_models::User,
    image_url: Option<&str>
) -> Result<(axum::http::StatusCode, [(axum::http::HeaderName, &'static str); 1], axum::Json<crate::api::twilio_sms::TwilioResponse>), Box<dyn std::error::Error>> {
    let args: BridgeSendArgs = serde_json::from_str(args)?;
    
    // Get user settings to check confirmation preference
    let user_settings = state.user_core.get_user_settings(user_id)?;

    // First search for the chat room
    let rooms = match crate::utils::bridge::search_bridge_rooms(
        service_type,
        &state,
        user_id,
        &args.chat_name,
    ).await {
        Ok(rooms) => rooms,
        Err(e) => {
            let error_msg = format!("Failed to find contact. Please make sure you're connected to {} bridge.", capitalize(service_type));
            if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                &state,
                error_msg.as_str(),
                None,
                user,
            ).await {
                eprintln!("Failed to send error message: {}", e);
            }
            return Ok((
                axum::http::StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                axum::Json(crate::api::twilio_sms::TwilioResponse {
                    message: error_msg.to_string(),
                })
            ));
        }
    };

    if rooms.is_empty() {
        let error_msg = format!("No {} contacts found matching '{}'.", capitalize(service_type), args.chat_name.as_str());
        if let Err(e) = crate::api::twilio_utils::send_conversation_message(
            &state,
            &error_msg,
            None,
            user,
        ).await {
            eprintln!("Failed to send error message: {}", e);
        }
        return Ok((
            axum::http::StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            axum::Json(crate::api::twilio_sms::TwilioResponse {
                message: error_msg,
            })
        ));
    }

    // Get the best match (first result)
    let best_match = &rooms[0];
    let exact_name = best_match.display_name.trim_end_matches(" (WA)").to_string().trim_end_matches(" (Telegram)").to_string();

    println!("confirmation: {}", user_settings.require_confirmation);
    // If confirmation is not required, send the message directly
    if !user_settings.require_confirmation {
        let message = args.message;
        match crate::utils::bridge::send_bridge_message(
            service_type,
            &state,
            user_id,
            &exact_name,
            &message,
            image_url.map(|url| url.to_string()),
        ).await {
            Ok(_) => {
                let success_msg = format!("{} message: '{}' sent to '{}'", capitalize(service_type), message ,exact_name);
                if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                    &state,
                    &success_msg,
                    None,
                    user,
                ).await {
                    eprintln!("Failed to send success message: {}", e);
                }
                return Ok((
                    axum::http::StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    axum::Json(crate::api::twilio_sms::TwilioResponse {
                        message: success_msg,
                    })
                ));
            }
            Err(e) => {
                let error_msg = format!("Failed to send {} message: {}", capitalize(service_type), e);
                if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                    &state,
                    &error_msg,
                    None,
                    user,
                ).await {
                    eprintln!("Failed to send error message: {}", e);
                }
                return Ok((
                    axum::http::StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    axum::Json(crate::api::twilio_sms::TwilioResponse {
                        message: error_msg,
                    })
                ));
            }
        }
    }

    // If confirmation is required, continue
    // Set the temporary variable for WhatsApp message
    if let Err(e) = state.user_core.set_temp_variable(
        user_id,
        Some(service_type),
        Some(&exact_name),
        None,
        Some(&args.message),
        None,
        None,
        None,
        image_url,
    ) {
        tracing::error!("Failed to set temporary variable: {}", e);
        if let Err(e) = crate::api::twilio_utils::send_conversation_message(
            &state,
            format!("Failed to prepare {} message sending. (contact rasmus@ahtava.com)", capitalize(service_type)).as_str(),
            None,
            user,
        ).await {
            tracing::error!("Failed to send error message: {}", e);
        }
        return Ok((
            axum::http::StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            axum::Json(crate::api::twilio_sms::TwilioResponse {
                message: "Failed to prepare message sending".to_string(),
            })
        ));
    }

    tracing::info!("Successfully set temporary variable");

    // Format the confirmation message with the found contact name and image if present
    let confirmation_msg = if image_url.is_some() {
        format!(
            "Send {} to '{}' with the above image and a caption '{}' (reply 'Yes' to confirm, otherwise it will be discarded)",
            capitalize(service_type), exact_name, args.message
        )
    } else {
        format!(
            "Send {} to '{}' with content: '{}' (reply 'Yes' to confirm, otherwise it will be discarded)",
            capitalize(service_type), exact_name, args.message
        )
    };

    // Send the confirmation message
    match crate::api::twilio_utils::send_conversation_message(
        &state,
        &confirmation_msg,
        None,
        user,
    ).await {
        Ok(_) => {
            // Deduct credits for the confirmation message
            if let Err(e) = crate::utils::usage::deduct_user_credits(&state, user_id, "message", None) {
                tracing::error!("Failed to deduct user credits: {}", e);
            }
            Ok((
                axum::http::StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                axum::Json(crate::api::twilio_sms::TwilioResponse {
                    message: "Message confirmation sent".to_string(),
                })
            ))
        }
        Err(e) => {
            eprintln!("Failed to send confirmation message: {}", e);
            Ok((
                axum::http::StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                axum::Json(crate::api::twilio_sms::TwilioResponse {
                    message: "Failed to send message confirmation".to_string(),
                })
            ))
        }
    }
}

pub async fn handle_search_bridge_rooms(
    state: &Arc<AppState>,
    service_type: &str,
    user_id: i32,
    args: &str,
) -> String {
    let args: BridgeSearchArgs = match serde_json::from_str(args) {
        Ok(args) => args,
        Err(e) => {
            eprintln!("Failed to parse message search arguments: {}", e);
            return "Failed to parse search request.".to_string();
        }
    };

    match crate::utils::bridge::search_bridge_rooms(
        service_type,
        &state,
        user_id,
        &args.search_term,
    ).await {
        Ok(rooms) => {
            if rooms.is_empty() {
                format!("No {} contacts found matching '{}'.", capitalize(service_type), args.search_term)
            } else {
                let mut response = String::new();
                for (i, room) in rooms.iter().take(5).enumerate() {
                    if i == 0 {
                        response.push_str(&format!("{}. {} (last active: {})", 
                            i + 1,
                            room.display_name.trim_end_matches(" (WA)"),
                            room.last_activity_formatted
                        ));
                    } else {
                        response.push_str(&format!("\n{}. {} (last active: {})", 
                            i + 1,
                            room.display_name.trim_end_matches(" (WA)"),
                            room.last_activity_formatted
                        ));
                    }
                }
                
                if rooms.len() > 5 {
                    response.push_str(&format!("\n\n(+ {} more contacts)", rooms.len() - 5));
                }
                
                response
            }
        }
        Err(e) => {
            eprintln!("Failed to search rooms: {}", e);
            format!("Failed to search contacts. Please make sure you're connected to {} bridge.", capitalize(service_type))
        }
    }
}

pub async fn handle_fetch_bridge_room_messages(
    state: &Arc<AppState>,
    service_type: &str,
    user_id: i32,
    args: &str,
) -> String {
    let args: BridgeRoomArgs = match serde_json::from_str(args) {
        Ok(args) => args,
        Err(e) => {
            eprintln!("Failed to parse room arguments: {}", e);
            return "Failed to parse room message request.".to_string();
        }
    };

    match crate::utils::bridge::fetch_bridge_room_messages(
        service_type,
        &state,
        user_id,
        &args.chat_name,
        args.limit,
    ).await {
        Ok((messages, room_name)) => {
            if messages.is_empty() {
                format!("No messages found in chat '{}'.", room_name.trim_end_matches(" (WA)").trim_end_matches(" (Telegram)"))
            } else {
                let mut response = format!("Messages from '{}':\n\n", room_name.trim_end_matches(" (WA)").trim_end_matches(" (Telegram)"));
                for (i, msg) in messages.iter().take(10).enumerate() {
                    let content = if msg.content.chars().count() > 100 {
                        let truncated: String = msg.content.chars().take(97).collect();
                        format!("{}...", truncated)
                    } else {
                        msg.content.clone()
                    };
                    
                    if i == 0 {
                        response.push_str(&format!("{}. {} at {}:\n{}", 
                            i + 1, 
                            msg.room_name,
                            msg.formatted_timestamp,
                            content
                        ));
                    } else {
                        response.push_str(&format!("\n\n{}. {} at {}:\n{}", 
                            i + 1, 
                            msg.room_name,
                            msg.formatted_timestamp,
                            content
                        ));
                    }
                }
                
                if messages.len() > 10 {
                    response.push_str(&format!("\n\n(+ {} more messages)", messages.len() - 10));
                }
                
                response
            }
        }
        Err(e) => {
            eprintln!("Failed to fetch room messages: {}", e);
            format!("Failed to fetch messages from '{}'. Please make sure you're connected to {} bridge and the chat exists.", args.chat_name, capitalize(service_type))
        }
    }
}

pub async fn handle_fetch_bridge_messages(
    state: &Arc<AppState>,
    service_type: &str,
    user_id: i32,
    args: &str,
) -> String {
    let time_frame: BridgeTimeFrame = match serde_json::from_str(args) {
        Ok(tf) => tf,
        Err(e) => {
            eprintln!("Failed to parse time frame: {}", e);
            return format!("Failed to parse time frame for {} messages.", capitalize(service_type));
        }
    };

    // Parse the RFC3339 timestamps into Unix timestamps
    let start_time = match chrono::DateTime::parse_from_rfc3339(&time_frame.start) {
        Ok(dt) => dt.timestamp(),
        Err(e) => {
            eprintln!("Failed to parse start time: {}", e);
            return "Invalid start time format. Please use RFC3339 format.".to_string();
        }
    };

    match crate::utils::bridge::fetch_bridge_messages(
        service_type,
        &state,
        user_id,
        start_time,
        false,
    ).await {
        Ok(messages) => {
            if messages.is_empty() {
                format!("No {} messages found for this time period.", capitalize(service_type))
            } else {
                let mut response = String::new();
                for (i, msg) in messages.iter().take(15).enumerate() {
                    let content = if msg.content.len() > 100 {
                        format!("{}...", &msg.content[..97])
                    } else {
                        msg.content.clone()
                    };
                    
                    if i == 0 {
                        response.push_str(&format!("{}. {} at {}:\n{}", 
                            i + 1, 
                            msg.room_name,
                            msg.formatted_timestamp,
                            content
                        ));
                    } else {
                        response.push_str(&format!("\n\n{}. {} at {}:\n{}", 
                            i + 1, 
                            msg.room_name,
                            msg.formatted_timestamp,
                            content
                        ));
                    }
                }
                
                if messages.len() > 15 {
                    response.push_str(&format!("\n\n(+ {} more messages)", messages.len() - 15));
                }
                
                response
            }
        }
        Err(e) => {
            eprintln!("Failed to fetch messages: {}", e);
            format!("Failed to fetch messages. Please make sure you're connected to {} bridge.", capitalize(service_type))
        }
    }
}
