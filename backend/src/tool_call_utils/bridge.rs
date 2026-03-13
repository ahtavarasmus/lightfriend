use crate::AppState;
use crate::UserCoreOps;
use axum::Json;
use serde::Deserialize;
use std::sync::Arc;

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
                "Searches for contacts, groups, or channels on a messaging platform by name.",
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
            description: Some("Optional: The platform to fetch messages from ('telegram', 'whatsapp' or 'signal'). If omitted, will automatically search all platforms linked to that contact.".to_string()),
            enum_values: Some(vec!["telegram".to_string(), "whatsapp".to_string(), "signal".to_string()]),
            ..Default::default()
        }),
    );
    properties.insert(
        "chat_name".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The name of a specific contact or group (e.g., 'John Doe', 'Mom', 'Family Group').".to_string()),
            ..Default::default()
        }),
    );
    properties.insert(
        "limit".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Number),
            description: Some(
                "Optional: Maximum number of messages to fetch (default: 20).".to_string(),
            ),
            ..Default::default()
        }),
    );
    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("fetch_chat_messages"),
            description: Some(String::from(
                "Fetches messages from a specific contact or group. If platform is omitted, searches all linked platforms."
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
            description: Some("Start time as 'YYYY-MM-DDTHH:MM' in the user's timezone (e.g., '2026-03-16T00:00'). Default to 24 hours before now if unspecified.".to_string()),
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
                    "Sends a message to a specific chat on the specified platform IMMEDIATELY. \
                    Use this when the user asks to send a message RIGHT NOW to a contact or group on Telegram, WhatsApp or Signal. \
                    IMPORTANT: If the user specifies a future time (e.g. 'at 5pm text...', 'in 2 hours send...'), do NOT call this tool - use create_item instead to schedule it. This tool executes immediately and cannot be scheduled. \
                    This tool will fuzzy search for the chat_name, add the message to the sending queue and unless user replies cancel the message will be sent after 60 seconds. \
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

use crate::api::twilio_sms::TwilioResponse;
use crate::models::user_models::User;
use axum::http::{HeaderName, StatusCode};

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
    image_url: Option<&str>,
) -> Result<
    (
        StatusCode,
        [(HeaderName, &'static str); 1],
        Json<TwilioResponse>,
    ),
    Box<dyn std::error::Error>,
> {
    let args: SendChatMessageArgs = serde_json::from_str(args)?;
    let capitalized_platform = args
        .platform
        .chars()
        .next()
        .map(|c| c.to_uppercase().collect::<String>())
        .unwrap_or_default()
        + &args.platform[1..];
    let bridge = state.user_repository.get_bridge(user_id, &args.platform)?;
    if bridge.map(|b| b.status != "connected").unwrap_or(true) {
        let error_msg = format!(
            "Failed to find contact. Please make sure you're connected to {} bridge.",
            capitalized_platform
        );
        if let Err(e) = state
            .twilio_message_service
            .send_sms(error_msg.as_str(), None, user)
            .await
        {
            eprintln!("Failed to send error message: {}", e);
        }
        return Ok((
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            Json(TwilioResponse {
                message: error_msg.to_string(),
                created_item_id: None,
            }),
        ));
    }
    let client = crate::utils::matrix_auth::get_cached_client(user_id, state).await?;
    let rooms = match crate::utils::bridge::get_service_rooms(&client, &args.platform).await {
        Ok(rooms) => rooms,
        Err(e) => {
            let error_msg = format!("Failed to fetch {} rooms: {}", capitalized_platform, e);
            if let Err(e) = state
                .twilio_message_service
                .send_sms(&error_msg, None, user)
                .await
            {
                eprintln!("Failed to send error message: {}", e);
            }
            return Ok((
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                Json(TwilioResponse {
                    message: error_msg,
                    created_item_id: None,
                }),
            ));
        }
    };

    // Try ontology Person for room_id lookup
    let best_match = if let Ok(Some(person)) = state.ontology_repository.find_person_by_name(user_id, &args.chat_name) {
        if let Some(channel) = person.channels.iter().find(|c| c.platform == args.platform && c.room_id.is_some()) {
            let rid = channel.room_id.as_ref().unwrap();
            rooms.iter().find(|r| r.room_id == *rid).cloned()
        } else {
            // Person exists but no channel for this platform - fall back to display name search
            crate::utils::bridge::search_best_match(&rooms, &args.chat_name)
        }
    } else {
        // No Person found - search by display name
        crate::utils::bridge::search_best_match(&rooms, &args.chat_name)
    };
    let best_match = match best_match {
        Some(room) => room,
        None => {
            let error_msg = format!(
                "No {} contacts found matching '{}'.",
                capitalized_platform,
                args.chat_name.as_str()
            );
            if let Err(e) = state
                .twilio_message_service
                .send_sms(&error_msg, None, user)
                .await
            {
                eprintln!("Failed to send error message: {}", e);
            }
            return Ok((
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                Json(TwilioResponse {
                    message: error_msg,
                    created_item_id: None,
                }),
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
    match state
        .twilio_message_service
        .send_sms(&queued_msg, None, user)
        .await
    {
        Ok(_) => {
            // SMS credits deducted at Twilio status callback
        }
        Err(e) => {
            eprintln!("Failed to send queued message: {}", e);
            return Ok((
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                Json(TwilioResponse {
                    message: "Failed to send message queue notification".to_string(),
                    created_item_id: None,
                }),
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
            )
            .await
            {
                let error_msg = format!(
                    "Failed to send {} message: {}",
                    cloned_capitalized_platform, e
                );
                if let Err(e) = cloned_state
                    .twilio_message_service
                    .send_sms(&error_msg, None, &cloned_user)
                    .await
                {
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
            created_item_id: None,
        }),
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

    // Search ontology Persons
    let mut person_results = Vec::new();
    if let Ok(persons) = state.ontology_repository.search_persons(user_id, &args.search_term) {
        for p in persons {
            let platforms: Vec<&str> = p.channels.iter()
                .filter(|c| c.platform == args.platform)
                .map(|c| c.platform.as_str())
                .collect();
            if !platforms.is_empty() {
                person_results.push(format!("{} (platforms: {})",
                    p.display_name(),
                    p.channels.iter().map(|c| c.platform.as_str()).collect::<Vec<_>>().join(", ")
                ));
            }
        }
    }

    match crate::utils::bridge::search_bridge_rooms(&args.platform, state, user_id, &args.search_term)
        .await
    {
        Ok(rooms) => {
            if rooms.is_empty() && person_results.is_empty() {
                let capitalized_platform = args
                    .platform
                    .chars()
                    .next()
                    .map(|c| c.to_uppercase().collect::<String>())
                    .unwrap_or_default()
                    + &args.platform[1..];
                format!(
                    "No {} contacts found matching '{}'.",
                    capitalized_platform, args.search_term
                )
            } else {
                let mut response = String::new();

                // Show ontology person results first
                if !person_results.is_empty() {
                    response.push_str("Known contacts:\n");
                    for (i, pr) in person_results.iter().enumerate() {
                        if i > 0 {
                            response.push('\n');
                        }
                        response.push_str(&format!("- {}", pr));
                    }
                    if !rooms.is_empty() {
                        response.push_str("\n\nBridge results:\n");
                    }
                }

                for (i, room) in rooms.iter().take(5).enumerate() {
                    if i == 0 && person_results.is_empty() {
                        response.push_str(&format!(
                            "{}. {} (last active: {})",
                            i + 1,
                            room.display_name
                                .trim_end_matches(" (WA)")
                                .trim_end_matches(" (Telegram)"),
                            room.last_activity_formatted
                        ));
                    } else {
                        response.push_str(&format!(
                            "\n{}. {} (last active: {})",
                            i + 1,
                            room.display_name
                                .trim_end_matches(" (WA)")
                                .trim_end_matches(" (Telegram)"),
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
            // If bridge search fails but we have person results, return those
            if !person_results.is_empty() {
                let mut response = String::from("Known contacts:\n");
                for (i, pr) in person_results.iter().enumerate() {
                    if i > 0 {
                        response.push('\n');
                    }
                    response.push_str(&format!("- {}", pr));
                }
                response
            } else {
                eprintln!("Failed to search rooms: {}", e);
                e.to_string()
            }
        }
    }
}

#[derive(Deserialize)]
struct FetchChatMessagesArgs {
    platform: Option<String>,
    chat_name: String,
    limit: Option<u64>,
}

pub async fn handle_fetch_chat_messages(state: &Arc<AppState>, user_id: i32, args: &str) -> String {
    let args: FetchChatMessagesArgs = match serde_json::from_str(args) {
        Ok(args) => args,
        Err(e) => {
            eprintln!("Failed to parse chat messages arguments: {}", e);
            return "Failed to parse chat messages request.".to_string();
        }
    };

    // Determine platform and chat_name using ontology Person lookup
    let (platform, chat_name) = if let Ok(Some(person)) = state.ontology_repository.find_person_by_name(user_id, &args.chat_name) {
        if let Some(platform) = &args.platform {
            // Platform specified - use it directly
            (platform.clone(), args.chat_name.clone())
        } else {
            // No platform specified - find any channel with a room_id, prefer most recently created
            let best_channel = person.channels.iter()
                .filter(|c| c.room_id.is_some() && ["whatsapp", "telegram", "signal"].contains(&c.platform.as_str()))
                .max_by_key(|c| c.created_at);
            if let Some(ch) = best_channel {
                (ch.platform.clone(), args.chat_name.clone())
            } else if let Some(platform) = &args.platform {
                // Person exists but no channels with room_id - use specified platform
                (platform.clone(), args.chat_name.clone())
            } else {
                return format!("No connected platforms found for '{}'. Please specify a platform (whatsapp, telegram, or signal).", args.chat_name);
            }
        }
    } else if let Some(platform) = &args.platform {
        // No Person found but platform specified - search by display name
        (platform.clone(), args.chat_name.clone())
    } else {
        // No Person and no platform - we need a platform
        return format!("Please specify a platform (whatsapp, telegram, or signal) for '{}'.", args.chat_name);
    };

    match crate::utils::bridge::fetch_bridge_room_messages(
        &platform, state, user_id, &chat_name, args.limit,
    )
    .await
    {
        Ok((messages, room_name)) => {
            if messages.is_empty() {
                format!(
                    "No messages found in chat '{}'.",
                    room_name
                        .trim_end_matches(" (WA)")
                        .trim_end_matches(" (Telegram)")
                )
            } else {
                let mut response = format!(
                    "Messages from '{}':\n\n",
                    room_name
                        .trim_end_matches(" (WA)")
                        .trim_end_matches(" (Telegram)")
                );
                for (i, msg) in messages.iter().take(10).enumerate() {
                    let content = if msg.content.chars().count() > 100 {
                        let truncated: String = msg.content.chars().take(97).collect();
                        format!("{}...", truncated)
                    } else {
                        msg.content.clone()
                    };

                    if i == 0 {
                        response.push_str(&format!(
                            "{}. {} at {}:\n{}",
                            i + 1,
                            msg.room_name,
                            msg.formatted_timestamp,
                            content
                        ));
                    } else {
                        response.push_str(&format!(
                            "\n\n{}. {} at {}:\n{}",
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
    let capitalized_platform = args
        .platform
        .chars()
        .next()
        .map(|c| c.to_uppercase().collect::<String>())
        .unwrap_or_default()
        + &args.platform[1..];
    // Look up user timezone and parse datetime to UTC timestamp
    let user_tz = match state.user_core.get_user_info(user_id) {
        Ok(info) => {
            let tz_str = info.timezone.unwrap_or_else(|| "UTC".to_string());
            tz_str.parse::<chrono_tz::Tz>().unwrap_or(chrono_tz::UTC)
        }
        Err(_) => chrono_tz::UTC,
    };
    let start_time =
        match crate::tool_call_utils::utils::parse_user_datetime_to_utc(&args.start, &user_tz) {
            Ok(dt) => dt.timestamp(),
            Err(e) => {
                eprintln!("Failed to parse start time: {}", e);
                return "Invalid start time format.".to_string();
            }
        };
    match crate::utils::bridge::fetch_bridge_messages(
        &args.platform,
        state,
        user_id,
        start_time,
        false,
    )
    .await
    {
        Ok(messages) => {
            if messages.is_empty() {
                format!(
                    "No {} messages found for this time period.",
                    capitalized_platform
                )
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
                        response.push_str(&format!(
                            "{}. {} at {}:\n{}",
                            i + 1,
                            msg.room_name,
                            msg.formatted_timestamp,
                            content
                        ));
                    } else {
                        response.push_str(&format!(
                            "\n\n{}. {} at {}:\n{}",
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
