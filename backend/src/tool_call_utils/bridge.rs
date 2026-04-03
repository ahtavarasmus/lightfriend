use crate::AppState;
use crate::UserCoreOps;
use axum::Json;
use serde::Deserialize;
use std::sync::Arc;

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
/// Fuzzy search Persons for a matching name/nickname with a room_id on the given platform.
/// Returns a BridgeRoom if found, None otherwise. No Matrix client needed.
fn find_person_room(
    state: &Arc<AppState>,
    user_id: i32,
    search_term: &str,
    platform: &str,
) -> Option<crate::utils::bridge::BridgeRoom> {
    let search_lower = search_term.trim().to_lowercase();

    // Search persons by substring match (name or nickname)
    let persons = match state
        .ontology_repository
        .search_persons(user_id, search_term)
    {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("SEND_FLOW find_person_room search_persons failed: {}", e);
            return None;
        }
    };

    // Among matched persons, find one with a room_id for this platform
    // Prefer exact name match, then substring, then we'll try fuzzy below
    for person in &persons {
        let name = person.display_name().to_lowercase();
        if name == search_lower {
            if let Some(ch) = person
                .channels
                .iter()
                .find(|c| c.platform == platform && c.room_id.is_some())
            {
                let rid = ch.room_id.as_ref().unwrap();
                tracing::info!(
                    "SEND_FLOW find_person_room: exact Person match '{}', room_id={}",
                    person.display_name(),
                    rid
                );
                return Some(crate::utils::bridge::BridgeRoom {
                    room_id: rid.clone(),
                    display_name: person.display_name().to_string(),
                    last_activity: 0,
                    last_activity_formatted: String::new(),
                    is_group: false,
                });
            }
        }
    }

    // Substring matches (search_persons already filtered these)
    for person in &persons {
        if let Some(ch) = person
            .channels
            .iter()
            .find(|c| c.platform == platform && c.room_id.is_some())
        {
            let rid = ch.room_id.as_ref().unwrap();
            tracing::info!(
                "SEND_FLOW find_person_room: substring Person match '{}', room_id={}",
                person.display_name(),
                rid
            );
            return Some(crate::utils::bridge::BridgeRoom {
                room_id: rid.clone(),
                display_name: person.display_name().to_string(),
                last_activity: 0,
                last_activity_formatted: String::new(),
                is_group: false,
            });
        }
    }

    // No substring match found - try fuzzy similarity on all persons
    let all_persons = match state
        .ontology_repository
        .get_persons_with_channels(user_id, 500, 0)
    {
        Ok(p) => p,
        Err(_) => return None,
    };

    let best = all_persons
        .iter()
        .filter_map(|p| {
            let name = p.display_name().to_lowercase();
            let score = strsim::jaro_winkler(&search_lower, &name);
            if score >= 0.7 {
                p.channels
                    .iter()
                    .find(|c| c.platform == platform && c.room_id.is_some())
                    .map(|ch| (score, p, ch))
            } else {
                None
            }
        })
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    if let Some((score, person, ch)) = best {
        let rid = ch.room_id.as_ref().unwrap();
        tracing::info!(
            "SEND_FLOW find_person_room: fuzzy Person match '{}' (score={}), room_id={}",
            person.display_name(),
            score,
            rid
        );
        return Some(crate::utils::bridge::BridgeRoom {
            room_id: rid.clone(),
            display_name: person.display_name().to_string(),
            last_activity: 0,
            last_activity_formatted: String::new(),
            is_group: false,
        });
    }

    tracing::info!(
        "SEND_FLOW find_person_room: no Person match for '{}'",
        search_term
    );
    None
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
    tracing::info!(
        "SEND_FLOW handle_send_chat_message ENTER: user={}, raw_args={}",
        user_id,
        args
    );
    let args: SendChatMessageArgs = serde_json::from_str(args)?;
    tracing::info!(
        "SEND_FLOW Parsed args: platform={}, chat_name={}, message_len={}, has_image={}",
        args.platform,
        args.chat_name,
        args.message.len(),
        image_url.is_some()
    );
    let capitalized_platform = args
        .platform
        .chars()
        .next()
        .map(|c| c.to_uppercase().collect::<String>())
        .unwrap_or_default()
        + &args.platform[1..];
    let bridge = state.user_repository.get_bridge(user_id, &args.platform)?;
    tracing::info!(
        "SEND_FLOW Bridge lookup: found={}, status={:?}",
        bridge.is_some(),
        bridge.as_ref().map(|b| b.status.clone())
    );
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
    // Step 1: Try to find a Person with a room_id for this platform (fast, no Matrix needed)
    tracing::info!(
        "SEND_FLOW Step 1: Fuzzy searching Persons for '{}' on {} for user={}",
        args.chat_name,
        args.platform,
        user_id
    );
    let best_match = find_person_room(state, user_id, &args.chat_name, &args.platform);

    // Step 2: If no Person match, fall back to Matrix room search
    let best_match = if best_match.is_some() {
        best_match
    } else {
        tracing::info!("SEND_FLOW Step 2: No Person match, falling back to Matrix room search");
        let client = crate::utils::matrix_auth::get_cached_client(user_id, state).await?;
        match crate::utils::bridge::get_service_rooms(&client, &args.platform).await {
            Ok(rooms) => {
                tracing::info!(
                    "SEND_FLOW Got {} rooms for platform={}",
                    rooms.len(),
                    args.platform
                );
                crate::utils::bridge::search_best_match(&rooms, &args.chat_name)
            }
            Err(e) => {
                tracing::error!("SEND_FLOW Failed to fetch rooms: {}", e);
                None
            }
        }
    };
    tracing::info!(
        "SEND_FLOW best_match result: found={}, display_name={:?}, room_id={:?}",
        best_match.is_some(),
        best_match.as_ref().map(|r| &r.display_name),
        best_match.as_ref().map(|r| &r.room_id)
    );
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
    tracing::info!(
        "SEND_FLOW Matched room: display_name='{}', exact_name='{}', room_id={}",
        best_match.display_name,
        exact_name,
        best_match.room_id
    );
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
    // Send the queued confirmation SMS (best-effort, don't block the actual send)
    tracing::info!("SEND_FLOW Sending confirmation SMS (best-effort)...");
    match state
        .twilio_message_service
        .send_sms(&queued_msg, None, user)
        .await
    {
        Ok(_) => tracing::info!("SEND_FLOW Confirmation SMS sent successfully"),
        Err(e) => tracing::warn!("SEND_FLOW Confirmation SMS failed (non-fatal): {}", e),
    }
    // Create cancellation channel
    tracing::info!("SEND_FLOW Creating cancellation channel and preparing delayed send task");
    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
    // Spawn the delayed send task after sending the message
    let cloned_state = state.clone();
    let cloned_user_id = user_id;
    let cloned_user = user.clone();
    let cloned_capitalized_platform = capitalized_platform.clone();
    let cloned_platform = args.platform.clone();
    let cloned_exact_name = exact_name.clone();
    let cloned_message = args.message.clone();
    let cloned_room_id = best_match.room_id.clone();
    // Log outbound bandwidth estimate
    let outbound_bytes = args.message.len() as i32 + if image_url.is_some() { 50_000 } else { 0 };
    if let Err(e) = state.bandwidth_repository.log_bandwidth(
        user_id,
        &args.platform,
        "outbound",
        outbound_bytes,
    ) {
        tracing::warn!(
            "Failed to log outbound bandwidth for user {}: {}",
            user_id,
            e
        );
    }

    let cloned_image_url = image_url.map(|s| s.to_string());
    tracing::info!(
        "SEND_FLOW About to tokio::spawn delayed send task for user={}, room_id={}",
        user_id,
        cloned_room_id
    );
    tokio::spawn(async move {
        tracing::info!(
            "SEND_FLOW_TASK Delayed task STARTED for user={}, waiting 60s or cancel. platform={}, recipient={}, room_id={}",
            cloned_user_id, cloned_platform, cloned_exact_name, cloned_room_id
        );
        let reason = tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => "timeout",
            _ = cancel_rx => "cancel",
        };
        tracing::info!(
            "SEND_FLOW_TASK Select resolved: reason={}, user={}, platform={}, recipient={}",
            reason,
            cloned_user_id,
            cloned_platform,
            cloned_exact_name
        );
        if reason == "timeout" {
            tracing::info!(
                "SEND_FLOW_TASK Calling send_bridge_message: service={}, user={}, chat_name={}, room_id={}",
                cloned_platform, cloned_user_id, cloned_exact_name, cloned_room_id
            );
            match crate::utils::bridge::send_bridge_message(
                &cloned_platform,
                &cloned_state,
                cloned_user_id,
                &cloned_exact_name,
                &cloned_message,
                cloned_image_url,
                Some(cloned_room_id.as_str()),
            )
            .await
            {
                Ok(msg) => {
                    tracing::info!(
                        "SEND_FLOW_TASK SUCCESS: Sent {} message to '{}' for user {} (room={:?})",
                        cloned_capitalized_platform,
                        cloned_exact_name,
                        cloned_user_id,
                        msg.room_id
                    );
                }
                Err(e) => {
                    tracing::error!(
                        "SEND_FLOW_TASK FAILED: send_bridge_message error for user={}: {}",
                        cloned_user_id,
                        e
                    );
                    let error_msg = format!(
                        "Failed to send {} message: {}",
                        cloned_capitalized_platform, e
                    );
                    if let Err(e) = cloned_state
                        .twilio_message_service
                        .send_sms(&error_msg, None, &cloned_user)
                        .await
                    {
                        tracing::error!("SEND_FLOW_TASK Also failed to send error SMS: {}", e);
                    }
                }
            }
        } else {
            tracing::info!(
                "SEND_FLOW_TASK Message to '{}' was CANCELLED by user",
                cloned_exact_name
            );
        }
        // Remove from map
        tracing::info!(
            "SEND_FLOW_TASK Removing user {} from pending_message_senders map",
            cloned_user_id
        );
        let mut senders = cloned_state.pending_message_senders.lock().await;
        senders.remove(&cloned_user_id);
        tracing::info!("SEND_FLOW_TASK Task complete for user={}", cloned_user_id);
    });
    tracing::info!(
        "SEND_FLOW tokio::spawn returned, storing cancel_tx in pending_message_senders for user={}",
        user_id
    );
    // Store the cancel sender in the map
    {
        let mut senders = state.pending_message_senders.lock().await;
        senders.insert(user_id, cancel_tx);
        tracing::info!(
            "SEND_FLOW Stored cancel_tx, pending_message_senders now has {} entries",
            senders.len()
        );
    }
    tracing::info!(
        "SEND_FLOW handle_send_chat_message RETURNING OK for user={} - delayed task is running in background",
        user_id
    );
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
    if let Ok(persons) = state
        .ontology_repository
        .search_persons(user_id, &args.search_term)
    {
        for p in persons {
            let platforms: Vec<&str> = p
                .channels
                .iter()
                .filter(|c| c.platform == args.platform)
                .map(|c| c.platform.as_str())
                .collect();
            if !platforms.is_empty() {
                person_results.push(format!(
                    "{} (platforms: {})",
                    p.display_name(),
                    p.channels
                        .iter()
                        .map(|c| c.platform.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
    }

    match crate::utils::bridge::search_bridge_rooms(
        &args.platform,
        state,
        user_id,
        &args.search_term,
    )
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
    let (platform, chat_name) = if let Ok(Some(person)) = state
        .ontology_repository
        .find_person_by_name(user_id, &args.chat_name)
    {
        if let Some(platform) = &args.platform {
            // Platform specified - use it directly
            (platform.clone(), args.chat_name.clone())
        } else {
            // No platform specified - find any channel with a room_id, prefer most recently created
            let best_channel = person
                .channels
                .iter()
                .filter(|c| {
                    c.room_id.is_some()
                        && ["whatsapp", "telegram", "signal"].contains(&c.platform.as_str())
                })
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
        return format!(
            "Please specify a platform (whatsapp, telegram, or signal) for '{}'.",
            args.chat_name
        );
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
