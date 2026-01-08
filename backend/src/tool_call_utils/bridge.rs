use crate::AppState;
use std::sync::Arc;
use serde::Deserialize;
use axum::Json;

pub fn get_search_chat_contacts_tool() -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;
    let mut properties = HashMap::new();
    properties.insert(
        "platform".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The platform to fetch messages from. Must be either 'telegram', 'whatsapp' or 'signal'.".to_string()),
            enum_values: Some(vec!["telegram".to_string(), "whatsapp".to_string(), "signal".to_string()]),
            ..Default::default()
        }),
    );
    properties.insert(
        "search_term".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The search term (e.g., name or keyword) to check for matching contacts, rooms, groups, or channels on the specified platform.".to_string()),
            ..Default::default()
        }),
    );
    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("search_chat_contacts"),
            description: Some(String::from(
                "Searches to check if specific contacts, rooms, groups, or channels exist on the specified platform by name or keyword. \
                Use this only when the user asks to search, find, or check for the existence of contacts/people/groups/channels on Telegram, WhatsApp or Signal. \
                Do not use this for searching messages within a chat; use the separate message search tool for that."
            )),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(vec![String::from("platform"), String::from("search_term")]),
            },
        },
    }
}

pub fn get_fetch_chat_messages_tool() -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;
    let mut properties = HashMap::new();
    properties.insert(
        "platform".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Optional: The platform to fetch messages from ('telegram', 'whatsapp' or 'signal'). If omitted and a contact profile nickname is used, will automatically search all platforms linked to that profile.".to_string()),
            enum_values: Some(vec!["telegram".to_string(), "whatsapp".to_string(), "signal".to_string()]),
            ..Default::default()
        }),
    );
    properties.insert(
        "chat_name".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The name of a specific contact or group (e.g., 'John Doe', 'Mom', 'Family Group'). Can be a contact profile nickname.".to_string()),
            ..Default::default()
        }),
    );
    properties.insert(
        "limit".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Number),
            description: Some("Optional: Maximum number of messages to fetch (default: 20).".to_string()),
            ..Default::default()
        }),
    );
    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("fetch_chat_messages"),
            description: Some(String::from(
                "Fetches messages from a specific contact or group. \
                Use this when the user asks about messages from a specific person (e.g., 'has Mom messaged me?', 'messages from John'). \
                Platform is optional—if omitted and the contact has a profile, will search all their linked platforms and return from the most recently active one. \
                Do not use if no specific chat is mentioned—use fetch_recent_messages instead."
            )),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(vec![String::from("chat_name")]),
            },
        },
    }
}

pub fn get_fetch_recent_messages_tool() -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;
    let mut properties = HashMap::new();
    properties.insert(
        "platform".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The platform to fetch recent messages from. Must be either 'telegram', 'whatsapp' or 'signal'.".to_string()),
            enum_values: Some(vec!["telegram".to_string(), "whatsapp".to_string(), "signal".to_string()]),
            ..Default::default()
        }),
    );
    properties.insert(
        "start".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Start time in RFC3339 format in UTC (e.g., '2024-03-16T00:00:00Z'). Default to 24 hours before now if unspecified.".to_string()),
            ..Default::default()
        }),
    );
    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("fetch_recent_messages"),
            description: Some(String::from(
                "Fetches recent messages across ALL chats on Telegram, WhatsApp or Signal from the given start time. \
                Use this when the user asks about recent messages without naming a specific chat (e.g., 'fetch telegram messages'). \
                Do not use if a particular contact or group is specified—use fetch_chat_messages instead."
            )),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(vec![String::from("platform"), String::from("start")]),
            },
        },
    }
}

pub fn get_send_chat_message_tool() -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;
    let mut properties = HashMap::new();
    properties.insert(
        "platform".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The platform to fetch recent messages from. Must be either 'telegram', 'whatsapp' or 'signal'.".to_string()),
            enum_values: Some(vec!["telegram".to_string(), "whatsapp".to_string(), "signal".to_string()]),
            ..Default::default()
        }),
    );
    properties.insert(
        "chat_name".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The chat name or room name to send the message to. Doesn't have to be exact since fuzzy search is used.".to_string()),
            ..Default::default()
        }),
    );
    properties.insert(
        "message".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The message content to send.".to_string()),
            ..Default::default()
        }),
    );
    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("send_chat_message"),
            description: 
                Some(String::from(
                    "Sends a message to a specific chat on the specified platform. \
                    Use this when the user asks to send a message to a contact or group on Telegram, WhatsApp or Signal. \
                    This tool will fuzzy search for the chat_name, add the message to the sending queue and unless user replies cancel the message will be sent after 60 seconds.
                    Only use this tool if the user has explicitly mentioned the message content or it is obviously clear what content they want to send; otherwise, ask the user to specify the message content, recipient and platform before calling the tool."
                )),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(vec![String::from("platform"), String::from("chat_name"), String::from("message")]),
            },
        },
    }
}

use axum::http::{StatusCode, HeaderName};
use crate::models::user_models::User;
use crate::api::twilio_sms::TwilioResponse;

#[derive(Deserialize)]
struct SendChatMessageArgs {
    platform: String,
    chat_name: String,
    message: String,
}
pub async fn handle_send_chat_message(
    state: &Arc<AppState>,
    user_id: i32,
    args: &str,
    user: &User,
    image_url: Option<&str>
) -> Result<(StatusCode, [(HeaderName, &'static str); 1], Json<TwilioResponse>), Box<dyn std::error::Error>> {
    let args: SendChatMessageArgs = serde_json::from_str(args)?;
    let capitalized_platform = args.platform.chars().next().map(|c| c.to_uppercase().collect::<String>()).unwrap_or_default() + &args.platform[1..];
    let bridge = state.user_repository.get_bridge(user_id, &args.platform)?;
    if bridge.map(|b| b.status != "connected").unwrap_or(true) {
        let error_msg = format!("Failed to find contact. Please make sure you're connected to {} bridge.", capitalized_platform);
        if let Err(e) = crate::api::twilio_utils::send_conversation_message(
            state,
            error_msg.as_str(),
            None,
            user,
        ).await {
            eprintln!("Failed to send error message: {}", e);
        }
        return Ok((
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            Json(TwilioResponse {
                message: error_msg.to_string(),
            })
        ));
    }
    let client = crate::utils::matrix_auth::get_cached_client(user_id, state).await?;
    let rooms = match crate::utils::bridge::get_service_rooms(&client, &args.platform).await {
        Ok(rooms) => rooms,
        Err(e) => {
            let error_msg = format!("Failed to fetch {} rooms: {}", capitalized_platform, e);
            if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                state,
                &error_msg,
                None,
                user,
            ).await {
                eprintln!("Failed to send error message: {}", e);
            }
            return Ok((
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                Json(TwilioResponse {
                    message: error_msg,
                })
            ));
        }
    };

    // First, try to resolve chat_name via contact profile nickname
    let profiles = state.user_repository.get_contact_profiles(user_id).unwrap_or_default();
    let profile_chat = profiles.iter().find_map(|p| {
        let nickname_lower = p.nickname.to_lowercase();
        let chat_name_lower = args.chat_name.to_lowercase();
        if nickname_lower.contains(&chat_name_lower) || chat_name_lower.contains(&nickname_lower) {
            match args.platform.as_str() {
                "whatsapp" => p.whatsapp_chat.clone(),
                "telegram" => p.telegram_chat.clone(),
                "signal" => p.signal_chat.clone(),
                _ => None
            }
        } else {
            None
        }
    });

    // Use profile's chat name if found, otherwise fall back to user's input
    let search_term = profile_chat.unwrap_or_else(|| args.chat_name.clone());
    let best_match = crate::utils::bridge::search_best_match(&rooms, &search_term);
    let best_match = match best_match {
        Some(room) => room,
        None => {
            let error_msg = format!("No {} contacts found matching '{}'.", capitalized_platform, args.chat_name.as_str());
            if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                state,
                &error_msg,
                None,
                user,
            ).await {
                eprintln!("Failed to send error message: {}", e);
            }
            return Ok((
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                Json(TwilioResponse {
                    message: error_msg,
                })
            ));
        }
    };
    // Get the best match
    let exact_name = crate::utils::bridge::remove_bridge_suffix(&best_match.display_name);
    tracing::info!("Message will be sent to {}", exact_name);
    // Format the queued message with the found contact name and image if present
    let queued_msg = if image_url.is_some() {
        format!(
            "Will send {} to '{}' with image and caption '{}' in 60s. Reply 'C' to discard.",
            capitalized_platform, exact_name, args.message
        )
    } else {
        format!(
            "Will send {} to '{}' with content '{}' in 60s. Reply 'C' to discard.",
            capitalized_platform, exact_name, args.message
        )
    };
    // Send the queued message
    match crate::api::twilio_utils::send_conversation_message(
        state,
        &queued_msg,
        None,
        user,
    ).await {
        Ok(_) => {
            // Deduct credits for the queued message
            if let Err(e) = crate::utils::usage::deduct_user_credits(state, user_id, "message", None) {
                tracing::error!("Failed to deduct user credits: {}", e);
            }
        }
        Err(e) => {
            eprintln!("Failed to send queued message: {}", e);
            return Ok((
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                Json(TwilioResponse {
                    message: "Failed to send message queue notification".to_string(),
                })
            ));
        }
    }
    // Create cancellation channel
    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
    // Spawn the delayed send task after sending the message
    let cloned_state = state.clone();
    let cloned_user_id = user_id;
    let cloned_user = user.clone();
    let cloned_capitalized_platform = capitalized_platform.clone();
    let cloned_platform = args.platform.clone();
    let cloned_exact_name = exact_name.clone();
    let cloned_message = args.message.clone();
    let cloned_image_url = image_url.map(|s| s.to_string());
    tokio::spawn(async move {
        let reason = tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => "timeout",
            _ = cancel_rx => "cancel",
        };
        if reason == "timeout" {
            // Proceed with send using captured variables
            println!("sending message now");
            if let Err(e) = crate::utils::bridge::send_bridge_message(
                &cloned_platform,
                &cloned_state,
                cloned_user_id,
                &cloned_exact_name,
                &cloned_message,
                cloned_image_url,
            ).await {
                let error_msg = format!("Failed to send {} message: {}", cloned_capitalized_platform, e);
                if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                    &cloned_state,
                    &error_msg,
                    None,
                    &cloned_user,
                ).await {
                    eprintln!("Failed to send error message: {}", e);
                }
            }
        }
        // Remove from map
        let mut senders = cloned_state.pending_message_senders.lock().await;
        senders.remove(&cloned_user_id);
    });
    // Store the cancel sender in the map
    {
        let mut senders = state.pending_message_senders.lock().await;
        senders.insert(user_id, cancel_tx);
    }
    Ok((
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        Json(TwilioResponse {
            message: "Message queued".to_string(),
        })
    ))
}

#[derive(Deserialize)]
struct SearchChatContactsArgs {
    platform: String,
    search_term: String,
}

pub async fn handle_search_chat_contacts(
    state: &Arc<AppState>,
    user_id: i32,
    args: &str,
) -> String {
    let args: SearchChatContactsArgs = match serde_json::from_str(args) {
        Ok(args) => args,
        Err(e) => {
            eprintln!("Failed to parse search arguments: {}", e);
            return "Failed to parse search request.".to_string();
        }
    };

    // First, try to resolve search_term via contact profile nickname
    let profiles = state.user_repository.get_contact_profiles(user_id).unwrap_or_default();
    let profile_chat = profiles.iter().find_map(|p| {
        let nickname_lower = p.nickname.to_lowercase();
        let search_lower = args.search_term.to_lowercase();
        if nickname_lower.contains(&search_lower) || search_lower.contains(&nickname_lower) {
            match args.platform.as_str() {
                "whatsapp" => p.whatsapp_chat.clone(),
                "telegram" => p.telegram_chat.clone(),
                "signal" => p.signal_chat.clone(),
                _ => None
            }
        } else {
            None
        }
    });

    // Use profile's chat name if found, otherwise fall back to user's input
    let search_term = profile_chat.unwrap_or_else(|| args.search_term.clone());

    match crate::utils::bridge::search_bridge_rooms(
        &args.platform,
        state,
        user_id,
        &search_term,
    ).await {
        Ok(rooms) => {
            if rooms.is_empty() {
                let capitalized_platform = args.platform.chars().next().map(|c| c.to_uppercase().collect::<String>()).unwrap_or_default() + &args.platform[1..];
                format!("No {} contacts found matching '{}'.", capitalized_platform, args.search_term)
            } else {
                let mut response = String::new();
                for (i, room) in rooms.iter().take(5).enumerate() {
                    if i == 0 {
                        response.push_str(&format!("{}. {} (last active: {})",
                            i + 1,
                            room.display_name.trim_end_matches(" (WA)").trim_end_matches(" (Telegram)"),
                            room.last_activity_formatted
                        ));
                    } else {
                        response.push_str(&format!("\n{}. {} (last active: {})",
                            i + 1,
                            room.display_name.trim_end_matches(" (WA)").trim_end_matches(" (Telegram)"),
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
            e.to_string()
        }
    }
}

#[derive(Deserialize)]
struct FetchChatMessagesArgs {
    platform: Option<String>,
    chat_name: String,
    limit: Option<u64>,
}

pub async fn handle_fetch_chat_messages(
    state: &Arc<AppState>,
    user_id: i32,
    args: &str,
) -> String {
    let args: FetchChatMessagesArgs = match serde_json::from_str(args) {
        Ok(args) => args,
        Err(e) => {
            eprintln!("Failed to parse chat messages arguments: {}", e);
            return "Failed to parse chat messages request.".to_string();
        }
    };

    // First, try to find a matching contact profile by nickname
    let profiles = state.user_repository.get_contact_profiles(user_id).unwrap_or_default();
    let matching_profile = profiles.iter().find(|p| {
        let nickname_lower = p.nickname.to_lowercase();
        let chat_name_lower = args.chat_name.to_lowercase();
        nickname_lower.contains(&chat_name_lower) || chat_name_lower.contains(&nickname_lower)
    });

    // Determine platform and chat_name to use
    let (platform, chat_name) = if let Some(platform) = &args.platform {
        // Platform was specified - use existing behavior
        let chat = matching_profile.and_then(|p| {
            match platform.as_str() {
                "whatsapp" => p.whatsapp_chat.clone(),
                "telegram" => p.telegram_chat.clone(),
                "signal" => p.signal_chat.clone(),
                _ => None
            }
        }).unwrap_or_else(|| args.chat_name.clone());
        (platform.clone(), chat)
    } else if let Some(profile) = matching_profile {
        // No platform specified - find the most recently active platform from profile
        let mut best_platform: Option<(String, String, i64)> = None; // (platform, chat_name, last_activity)

        // Check each linked platform for activity
        for (plat, chat_opt) in [
            ("whatsapp", &profile.whatsapp_chat),
            ("telegram", &profile.telegram_chat),
            ("signal", &profile.signal_chat),
        ] {
            if let Some(chat) = chat_opt {
                // Check if bridge is connected and get room activity
                if let Ok(Some(_)) = state.user_repository.get_bridge(user_id, plat) {
                    if let Ok(client) = crate::utils::matrix_auth::get_cached_client(user_id, state).await {
                        if let Ok(rooms) = crate::utils::bridge::get_service_rooms(&client, plat).await {
                            // Find the room matching this chat
                            let chat_lower = chat.to_lowercase();
                            if let Some(room) = rooms.iter().find(|r| {
                                let name_lower = r.display_name.to_lowercase();
                                name_lower.contains(&chat_lower) || chat_lower.contains(&name_lower)
                            }) {
                                if best_platform.is_none() || room.last_activity > best_platform.as_ref().unwrap().2 {
                                    best_platform = Some((plat.to_string(), chat.clone(), room.last_activity));
                                }
                            }
                        }
                    }
                }
            }
        }

        match best_platform {
            Some((plat, chat, _)) => (plat, chat),
            None => {
                return format!("No connected platforms found for '{}'. Make sure they have a linked WhatsApp, Telegram, or Signal chat in their contact profile.", args.chat_name);
            }
        }
    } else {
        // No platform and no matching profile - we need a platform
        return format!("Please specify a platform (whatsapp, telegram, or signal) for '{}', or create a contact profile for them.", args.chat_name);
    };

    match crate::utils::bridge::fetch_bridge_room_messages(
        &platform,
        state,
        user_id,
        &chat_name,
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
            eprintln!("Failed to fetch chat messages: {}", e);
            e.to_string()
        }
    }
}

use chrono::DateTime;

#[derive(Deserialize)]
struct FetchRecentMessagesArgs {
    platform: String,
    start: String,
}

pub async fn handle_fetch_recent_messages(
    state: &Arc<AppState>,
    user_id: i32,
    args: &str,
) -> String {
    let args: FetchRecentMessagesArgs = match serde_json::from_str(args) {
        Ok(args) => args,
        Err(e) => {
            eprintln!("Failed to parse recent messages arguments: {}", e);
            return "Failed to parse recent messages request.".to_string();
        }
    };
    let capitalized_platform = args.platform.chars().next().map(|c| c.to_uppercase().collect::<String>()).unwrap_or_default() + &args.platform[1..];
    // Parse the RFC3339 timestamps into Unix timestamps
    let start_time = match DateTime::parse_from_rfc3339(&args.start) {
        Ok(dt) => dt.timestamp(),
        Err(e) => {
            eprintln!("Failed to parse start time: {}", e);
            return "Invalid start time format. Please use RFC3339 format.".to_string();
        }
    };
    match crate::utils::bridge::fetch_bridge_messages(
        &args.platform,
        state,
        user_id,
        start_time,
        false,
    ).await {
        Ok(messages) => {
            if messages.is_empty() {
                format!("No {} messages found for this time period.", capitalized_platform)
            } else {
                let mut response = String::new();
                for (i, msg) in messages.iter().take(15).enumerate() {
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
               
                if messages.len() > 15 {
                    response.push_str(&format!("\n\n(+ {} more messages)", messages.len() - 15));
                }
               
                response
            }
        }
        Err(e) => {
            eprintln!("Failed to fetch messages: {}", e);
            e.to_string()
        }
    }
}
