use crate::AppState;
use std::sync::Arc;
use serde::Deserialize;
use axum::Json;

pub fn get_send_whatsapp_message_tool() -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;

    let mut whatsapp_send_properties = HashMap::new();
    whatsapp_send_properties.insert(
        "chat_name".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The chat name or room name to send the message to. Doesn't have to be exact since fuzzy search is used.".to_string()),
            ..Default::default()
        }),
    );
    whatsapp_send_properties.insert(
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
            name: String::from("send_whatsapp_message"),
            description: Some(String::from("Sends a WhatsApp message to a specific chat. This tool will first make a confirmation message for the user, which they can then confirm or not. The chat_name will be used to fuzzy search for the actual chat name and then confirmed with the user.")),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(whatsapp_send_properties),
                required: Some(vec![String::from("chat_name"), String::from("message")]),
            },
        },
    }
}
pub fn get_fetch_whatsapp_messages_tool() -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;

    let mut whatsapp_messages_properties = HashMap::new();
    whatsapp_messages_properties.insert(
        "start".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Start time in RFC3339 format in UTC (e.g., '2024-03-16T00:00:00Z')".to_string()),
            ..Default::default()
        }),
    );
    whatsapp_messages_properties.insert(
        "end".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("End time in RFC3339 format in UTC (e.g., '2024-03-16T00:00:00Z')".to_string()),
            ..Default::default()
        }),
    );

    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("fetch_whatsapp_messages"),
            description: Some(String::from("Fetches recent WhatsApp messages. Use this when user asks about their WhatsApp messages or conversations.")),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(whatsapp_messages_properties),
                required: Some(vec![String::from("start"), String::from("end")]),
            },
        },
    }
}

pub fn get_fetch_whatsapp_room_messages_tool() -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;

    let mut whatsapp_room_messages_properties = HashMap::new();
    whatsapp_room_messages_properties.insert(
        "chat_name".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The name of the WhatsApp chat/room to fetch messages from".to_string()),
            ..Default::default()
        }),
    );
    whatsapp_room_messages_properties.insert(
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
            name: String::from("fetch_whatsapp_room_messages"),
            description: Some(String::from("Fetches messages from a specific WhatsApp chat/room. Use this when user asks about messages from a specific WhatsApp contact or group. You should return the latest messages that person or chat has sent to the user.")),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(whatsapp_room_messages_properties),
                required: Some(vec![String::from("chat_name")]),
            },
        },
    }
}

pub fn get_search_whatsapp_rooms_tool() -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;

    let mut whatsapp_search_properties = HashMap::new();
    whatsapp_search_properties.insert(
        "search_term".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Search term to find WhatsApp rooms/contacts".to_string()),
            ..Default::default()
        }),
    );

    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("search_whatsapp_rooms"),
            description: Some(String::from("Searches for WhatsApp rooms/contacts by name.")),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(whatsapp_search_properties),
                required: Some(vec![String::from("search_term")]),
            },
        },
    }
}




#[derive(Deserialize)]
pub struct WhatsAppSendArgs {
    pub chat_name: String,
    pub message: String,
}

#[derive(Deserialize)]
pub struct WhatsAppSearchArgs {
    pub search_term: String,
}

#[derive(Deserialize)]
pub struct WhatsAppRoomArgs {
    pub chat_name: String,
    pub limit: Option<u64>,
}

#[derive(Deserialize)]
pub struct WhatsAppTimeFrame {
    pub start: String,
    pub end: String,
}

pub async fn handle_send_whatsapp_message(
    state: &Arc<AppState>,
    user_id: i32,
    conversation_sid: &str,
    twilio_number: &String,
    args: &str,
    user: &crate::models::user_models::User,
    image_url: Option<&str>
) -> Result<(axum::http::StatusCode, [(axum::http::HeaderName, &'static str); 1], axum::Json<crate::api::twilio_sms::TwilioResponse>), Box<dyn std::error::Error>> {
    let args: WhatsAppSendArgs = serde_json::from_str(args)?;

    tracing::info!("IN HANDLE_SEND_WHATSAPP_MESSAGE");

    // First search for the chat room
    match crate::utils::whatsapp_utils::search_whatsapp_rooms(
        &state,
        user_id,
        &args.chat_name,
    ).await {
        Ok(rooms) => {
            if rooms.is_empty() {
                let error_msg = format!("No WhatsApp contacts found matching '{}'.", args.chat_name);
                if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                    conversation_sid,
                    twilio_number,
                    &error_msg,
                    true,
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
            let exact_name = best_match.display_name.trim_end_matches(" (WA)").to_string();


            // Set the temporary variable for WhatsApp message
            if let Err(e) = state.user_core.set_temp_variable(
                user_id,
                Some("whatsapp"),
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
                    conversation_sid,
                    twilio_number,
                    "Failed to prepare WhatsApp message sending. (not charged, contact rasmus@ahtava.com)",
                    true,
                    None,
                    user,
                ).await {
                    tracing::error!("Failed to send error message: {}", e);
                }
                return Ok((
                    axum::http::StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    axum::Json(crate::api::twilio_sms::TwilioResponse {
                        message: "Failed to prepare WhatsApp message sending".to_string(),
                    })
                ));
            }

            tracing::info!("Successfully set temporary variable for WhatsApp message");


            // Format the confirmation message with the found contact name and image if present
            let confirmation_msg = if image_url.is_some() {
                format!(
                    "Send WhatsApp to '{}' with the above image and a caption '{}' (yes-> send, no -> discard) (free reply)",
                    exact_name, args.message
                )
            } else {
                format!(
                    "Send WhatsApp to '{}' with content: '{}' (yes-> send, no -> discard) (free reply)",
                    exact_name, args.message
                )
            };

            // Send the confirmation message
            match crate::api::twilio_utils::send_conversation_message(
                conversation_sid,
                twilio_number,
                &confirmation_msg,
                true, // Don't redact since we need to extract info from this message later
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
                            message: "WhatsApp message confirmation sent".to_string(),
                        })
                    ))
                }
                Err(e) => {
                    eprintln!("Failed to send confirmation message: {}", e);
                    Ok((
                        axum::http::StatusCode::OK,
                        [(axum::http::header::CONTENT_TYPE, "application/json")],
                        axum::Json(crate::api::twilio_sms::TwilioResponse {
                            message: "Failed to send WhatsApp confirmation".to_string(),
                        })
                    ))
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to search WhatsApp rooms: {}", e);
            let error_msg = "Failed to find WhatsApp contact. Please make sure you're connected to WhatsApp bridge.";
            if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                conversation_sid,
                twilio_number,
                error_msg,
                true,
                None,
                user,
            ).await {
                eprintln!("Failed to send error message: {}", e);
            }
            Ok((
                axum::http::StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                axum::Json(crate::api::twilio_sms::TwilioResponse {
                    message: error_msg.to_string(),
                })
            ))
        }
    }
}

pub async fn handle_search_whatsapp_rooms(
    state: &Arc<AppState>,
    user_id: i32,
    args: &str,
) -> String {
    let args: WhatsAppSearchArgs = match serde_json::from_str(args) {
        Ok(args) => args,
        Err(e) => {
            eprintln!("Failed to parse WhatsApp search arguments: {}", e);
            return "Failed to parse search request.".to_string();
        }
    };

    match crate::utils::whatsapp_utils::search_whatsapp_rooms(
        &state,
        user_id,
        &args.search_term,
    ).await {
        Ok(rooms) => {
            if rooms.is_empty() {
                format!("No WhatsApp contacts found matching '{}'.", args.search_term)
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
            eprintln!("Failed to search WhatsApp rooms: {}", e);
            "Failed to search WhatsApp contacts. Please make sure you're connected to WhatsApp bridge.".to_string()
        }
    }
}

pub async fn handle_fetch_whatsapp_room_messages(
    state: &Arc<AppState>,
    user_id: i32,
    args: &str,
) -> String {
    let args: WhatsAppRoomArgs = match serde_json::from_str(args) {
        Ok(args) => args,
        Err(e) => {
            eprintln!("Failed to parse WhatsApp room arguments: {}", e);
            return "Failed to parse room message request.".to_string();
        }
    };

    match crate::utils::whatsapp_utils::fetch_whatsapp_room_messages(
        &state,
        user_id,
        &args.chat_name,
        args.limit,
    ).await {
        Ok((messages, room_name)) => {
            if messages.is_empty() {
                format!("No messages found in chat '{}'.", room_name.trim_end_matches(" (WA)"))
            } else {
                let mut response = format!("Messages from '{}':\n\n", room_name.trim_end_matches(" (WA)"));
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
                            msg.sender_display_name,
                            msg.formatted_timestamp,
                            content
                        ));
                    } else {
                        response.push_str(&format!("\n\n{}. {} at {}:\n{}", 
                            i + 1, 
                            msg.sender_display_name,
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
            eprintln!("Failed to fetch WhatsApp room messages: {}", e);
            format!("Failed to fetch messages from '{}'. Please make sure you're connected to WhatsApp bridge and the chat exists.", args.chat_name)
        }
    }
}

pub async fn handle_fetch_whatsapp_messages(
    state: &Arc<AppState>,
    user_id: i32,
    args: &str,
) -> String {
    let time_frame: WhatsAppTimeFrame = match serde_json::from_str(args) {
        Ok(tf) => tf,
        Err(e) => {
            eprintln!("Failed to parse WhatsApp time frame: {}", e);
            return "Failed to parse time frame for WhatsApp messages.".to_string();
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

    let end_time = match chrono::DateTime::parse_from_rfc3339(&time_frame.end) {
        Ok(dt) => dt.timestamp(),
        Err(e) => {
            eprintln!("Failed to parse end time: {}", e);
            return "Invalid end time format. Please use RFC3339 format.".to_string();
        }
    };

    match crate::utils::whatsapp_utils::fetch_whatsapp_messages(
        &state,
        user_id,
        start_time,
        end_time,
    ).await {
        Ok(messages) => {
            if messages.is_empty() {
                "No WhatsApp messages found for this time period.".to_string()
            } else {
                let mut response = String::new();
                for (i, msg) in messages.iter().take(15).enumerate() {
                    let content = if msg.content.len() > 100 {
                        format!("{}...", &msg.content[..97])
                    } else {
                        msg.content.clone()
                    };
                    
                    if i == 0 {
                        response.push_str(&format!("{}. {} in {} at {}:\n{}", 
                            i + 1, 
                            msg.sender_display_name,
                            msg.room_name,
                            msg.formatted_timestamp,
                            content
                        ));
                    } else {
                        response.push_str(&format!("\n\n{}. {} in {} at {}:\n{}", 
                            i + 1, 
                            msg.sender_display_name,
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
            eprintln!("Failed to fetch WhatsApp messages: {}", e);
            "Failed to fetch WhatsApp messages. Please make sure you're connected to WhatsApp bridge.".to_string()
        }
    }
}

