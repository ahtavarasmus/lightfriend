use reqwest::Client;
use crate::handlers::imap_handlers::ImapError;
use std::env;
use std::error::Error;
use std::sync::Arc;
use crate::AppState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::cell::RefCell;
use axum::{
    extract::Form,
    response::IntoResponse,
    extract::State,
    http::StatusCode,
    Json,
};
use crate::tool_call_utils::utils::{
    ChatMessage, create_openai_client, create_eval_tools, create_clarify_tools,
    ClarifyResponse, EvalResponse,
};
use chrono::Utc;

// Thread-local storage for media SID mapping
thread_local! {
    static MEDIA_SID_MAP: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
}

use openai_api_rs::v1::{
    chat_completion,
    types,
    api::OpenAIClient,
    common::GPT4_O,
};


#[derive(Debug, Deserialize, Clone)]
pub struct TwilioMessageResponse {
    pub sid: String,
    pub conversation_sid: String,
    pub body: String,
    pub author: String,
}

#[derive(Debug, Deserialize)]
struct TwilioMessagesResponse {
    messages: Vec<TwilioMessageResponse>,
}

#[derive(Deserialize, Clone)]
pub struct MediaItem {
    pub content_type: String,
    pub url: String,
    pub sid: String,
}

#[derive(Deserialize, Clone)]
pub struct TwilioWebhookPayload {
    #[serde(rename = "From")]
    pub from: String,
    #[serde(rename = "To")]
    pub to: String,
    #[serde(rename = "Body")]
    pub body: String,
    #[serde(rename = "NumMedia")]
    pub num_media: Option<String>,
    #[serde(rename = "MediaUrl0")]
    pub media_url0: Option<String>,
    #[serde(rename = "MediaContentType0")]
    pub media_content_type0: Option<String>,
    #[serde(rename = "MessageSid")]
    pub message_sid: String,
}

#[derive(Serialize, Debug)]
pub struct TwilioResponse {
    #[serde(rename = "Message")]
    pub message: String,
}


// New wrapper handler for the regular SMS endpoint
pub async fn handle_regular_sms(
    State(state): State<Arc<AppState>>,
    Form(payload): Form<TwilioWebhookPayload>,
) -> (StatusCode, [(axum::http::HeaderName, &'static str); 1], axum::Json<TwilioResponse>) {
    // First check if this user has a discount_tier == sms - they shouldn't be using this endpoint, but their own dedicated
    match state.user_core.find_by_phone_number(&payload.from) {
        Ok(Some(user)) => {
            if let Some(tier) = user.discount_tier {
                if tier == "msg".to_string() {
                    tracing::warn!("User {} with discount_tier equal to msg attempted to use regular SMS endpoint", user.id);
                    return (
                        StatusCode::FORBIDDEN,
                        [(axum::http::header::CONTENT_TYPE, "application/json")],
                        axum::Json(TwilioResponse {
                            message: "Please use your dedicated SMS endpoint. Contact support if you need help.".to_string(),
                        })
                    );
                }
            }
        },
        Ok(None) => {
            tracing::error!("No user found for phone number: {}", payload.from);
            return (
                StatusCode::NOT_FOUND,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                axum::Json(TwilioResponse {
                    message: "User not found".to_string(),
                })
            );
        },
        Err(e) => {
            tracing::error!("Database error while finding user: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                axum::Json(TwilioResponse {
                    message: "Internal server error".to_string(),
                })
            );
        }
    }

    // If we get here, the user is allowed to use this endpoint
    handle_incoming_sms(State(state), Form(payload)).await
}

// Original handler becomes internal and is used by both routes
pub async fn handle_incoming_sms(
    State(state): State<Arc<AppState>>,
    Form(payload): Form<TwilioWebhookPayload>,
) -> (StatusCode, [(axum::http::HeaderName, &'static str); 1], axum::Json<TwilioResponse>) {
    tracing::debug!("Received SMS from: {} to: {}", payload.from, payload.to);

    // Check for Shazam shortcut ('S' or 's')
    if payload.body.trim() == "S" || payload.body.trim() == "s" {

        return (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            axum::Json(TwilioResponse {
                message: "The Shazam feature has been discontinued due to insufficient usage. Thank you for your understanding.".to_string(),
            })
        );
    }

    // Check for STOP command
    if payload.body.trim().to_uppercase() == "STOP" {
        if let Ok(Some(user)) = state.user_core.find_by_phone_number(&payload.from) {
            if let Err(e) = state.user_core.update_notify(user.id, false) {
                tracing::error!("Failed to update notify status: {}", e);
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

    // Process SMS in the background
    tokio::spawn(async move {
        let result = process_sms(&state, payload.clone(), false).await;
        
        if result.0 != StatusCode::OK {
            tracing::error!("Background SMS processing failed with status: {:?}", result.0);
            tracing::error!("Error response: {:?}", result.1);
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


pub async fn process_sms(
    state: &Arc<AppState>,
    payload: TwilioWebhookPayload,
    is_test: bool,
) -> (StatusCode, [(axum::http::HeaderName, &'static str); 1], axum::Json<TwilioResponse>) {
    let start_time = std::time::Instant::now(); // Track processing time
    let user = match state.user_core.find_by_phone_number(&payload.from) {
        Ok(Some(user)) => user,
        Ok(None) => {
            tracing::error!("No user found for phone number: {}", payload.from);
            return (
                StatusCode::NOT_FOUND,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                axum::Json(TwilioResponse {
                    message: "User not found".to_string(),
                })
            );
        },
        Err(e) => {
            tracing::error!("Database error while finding user for phone number {}: {}", payload.from, e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                axum::Json(TwilioResponse {
                    message: "Database error".to_string(),
                })
            );
        }
    };

    // Check if user has sufficient credits before processing the message
    if let Err(e) = crate::utils::usage::check_user_credits(&state, &user, "message", None).await {
        tracing::warn!("User {} has insufficient credits: {}", user.id, e);
        return (
            StatusCode::PAYMENT_REQUIRED,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            axum::Json(TwilioResponse {
                message: e,
            })
        );
    }
    tracing::info!("Found user with ID: {} for phone number: {}", user.id, payload.from);
    
    // Log media information for admin user
    if user.id == 1 {
        if let (Some(num_media), Some(media_url), Some(content_type)) = (
            payload.num_media.as_ref(),
            payload.media_url0.as_ref(),
            payload.media_content_type0.as_ref()
        ) {
            tracing::debug!("Media information:");
            tracing::debug!("  Number of media items: {}", num_media);
            tracing::debug!("  Media URL: {}", media_url);
            tracing::debug!("  Content type: {}", content_type);
        }
    }

    let conversation = match state.user_conversations.get_conversation(&state, &user, payload.to).await {
        Ok(conv) => conv,
        Err(e) => {
            tracing::error!("Failed to ensure conversation exists: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                axum::Json(TwilioResponse {
                    message: "Failed to create conversation".to_string(),
                })
            );
        }
    };

    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    // Store user's message in history
    let user_message = crate::models::user_models::NewMessageHistory {
        user_id: user.id,
        role: "user".to_string(),
        encrypted_content: payload.body.clone(),
        tool_name: None,
        tool_call_id: None,
        created_at: current_time,
        conversation_id: conversation.conversation_sid.clone(),
    };

    if let Err(e) = state.user_repository.create_message_history(&user_message) {
        tracing::error!("Failed to store user message in history: {}", e);
    }

    if user.confirm_send_event.is_some() {
        // Handle confirmation logic
        let confirmation_result = crate::tool_call_utils::confirm::handle_confirmation(
            &state,
            &user.clone(),
            &conversation.conversation_sid,
            &conversation.twilio_number,
            &user.clone().confirm_send_event.unwrap(),
            &payload.body,
        ).await;

        tracing::info!("Came back from handle_confirmation with should_continue as: {}", confirmation_result.should_continue);

        if !confirmation_result.should_continue {
            if let Some(response) = confirmation_result.response {
                return response;
            }
        }
    }
    
        // Get timezone from user info or default to UTC
    // Get user settings to access timezone
    let user_settings = match state.user_core.get_user_settings(user.id) {
        Ok(settings) => settings,
        Err(e) => {
            tracing::error!("Failed to get user settings: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                axum::Json(TwilioResponse {
                    message: "Failed to process user settings".to_string(),
                })
            );
        }
    };

    let user_info = match user_settings.clone().info {
        Some(info) => info,
        None => "".to_string()
    };

    let timezone_str = match user_settings.timezone {
        Some(ref tz) => tz.as_str(),
        None => "UTC",
    };

    let require_confirmation = user_settings.require_confirmation;

    // Get timezone offset using jiff
    let (hours, minutes) = match crate::api::elevenlabs::get_offset_with_jiff(timezone_str) {
        Ok((h, m)) => (h, m),
        Err(_) => {
            tracing::error!("Failed to get timezone offset for {}, defaulting to UTC", timezone_str);
            (0, 0) // UTC default
        }
    };

    // Calculate total offset in seconds
    let offset_seconds = hours * 3600 + minutes * 60 * if hours >= 0 { 1 } else { -1 };

    // Create FixedOffset for chrono
    let user_timezone = chrono::FixedOffset::east_opt(offset_seconds)
        .unwrap_or_else(|| chrono::FixedOffset::east(0)); // Fallback to UTC if invalid

    // Format current time in RFC3339 for the user's timezone
    let formatted_time = Utc::now().with_timezone(&user_timezone).to_rfc3339();

    // Format offset string (e.g., "+02:00" or "-05:30")
    let offset = format!("{}{:02}:{:02}", 
        if hours >= 0 { "+" } else { "-" },
        hours.abs(),
        minutes.abs()
    );

    // Start with the system message
    let mut chat_messages: Vec<ChatMessage> = vec![ChatMessage {
        role: "system".to_string(),
        content: chat_completion::Content::Text(format!("You are a direct and efficient AI assistant named lightfriend. The current date is {}. You must provide extremely concise responses (max 400 characters) while being accurate and helpful. Since users pay per message, always provide all available information immediately without asking follow-up questions unless confirming details for actions that involve sending information or making changes. Always use all tools immidiately that you think will be needed to complete the user's query and base your response to those responses. IMPORTANT: For calendar events, you must return the exact output from the calendar tool without any modifications, additional text, or formatting. Never add bullet points, markdown formatting (like **, -, #), or any other special characters.\n\n### Tool Usage Guidelines:\n- Provide all relevant details in the response immediately. \n- Tools that involve sending or creating something(eg. send_whatsapp_message), you can call them straight away using the available information without confirming with the user. These tools will send extra confirmation message to user anyways before doing anything.\n\n### Date and Time Handling:\n- Always work with times in the user's timezone: {} with offset {}.\n- When user mentions times without dates, assume they mean the nearest future occurrence.\n- For time inputs to tools, convert to RFC3339 format in UTC (e.g., '2024-03-23T14:30:00Z').\n- For displaying times to users:\n  - Use 12-hour format with AM/PM (e.g., '2:30 PM')\n  - Include timezone-adjusted dates in a friendly format (e.g., 'today', 'tomorrow', or 'Jun 15')\n  - Show full date only when it's not today/tomorrow\n- If no specific time is mentioned:\n  - For calendar queries: Show today's events (and tomorrow's if after 6 PM)\n  - For other time ranges: Use current time to 24 hours ahead\n- For queries about:\n  - 'Today': Use 00:00 to 23:59 of the current day in user's timezone\n  - 'Tomorrow': Use 00:00 to 23:59 of tomorrow in user's timezone\n  - 'This week': Use remaining days of current week\n  - 'Next week': Use Monday to Sunday of next week\n\n### Additional Guidelines:\n- Weather Queries: If no location is specified, assume the user's home location from user info.\n- Email Queries: For fetch_specific_email, provide the whole message body or a summary if too long—never just the subject.\n- WhatsApp/Telegram Fetching: Use the room name directly from the user's message/context without searching rooms.\n\nNever use markdown, HTML, or any special formatting characters in responses. Return all information in plain text only. User information: {}. Always use tools to fetch the latest information before answering.", formatted_time, timezone_str, offset, user_info)),
    }];
    
    // Process the message body to remove "forget" if it exists at the start
    let processed_body = if payload.body.to_lowercase().starts_with("forget") {
        payload.body.trim_start_matches(|c: char| c.is_alphabetic()).trim().to_string()
    } else {
        payload.body.clone()
    };

    // Delete media if present after processing
    if let (Some(num_media), Some(media_url), Some(_)) = (
        payload.num_media.as_ref(),
        payload.media_url0.as_ref(),
        payload.media_content_type0.as_ref()
    ) {
        if num_media != "0" {
            // Extract media SID from URL
            if let Some(media_sid) = media_url.split("/Media/").nth(1) {
                tracing::debug!("Attempting to delete media with SID: {}", media_sid);
                match crate::api::twilio_utils::delete_twilio_message_media(&state, &media_sid, &user).await {
                    Ok(_) => tracing::debug!("Successfully deleted media: {}", media_sid),
                    Err(e) => tracing::error!("Failed to delete media {}: {}", media_sid, e),
                }
            }
        }
    }

    // Only include conversation history if message doesn't start with "forget"
    if !payload.body.to_lowercase().starts_with("forget") {

        // Get user's save_context setting
        let save_context = user_settings.save_context.unwrap_or(0);
        
        if save_context > 0 {
            // Get the last N back-and-forth exchanges based on save_context
            let history = state.user_repository
                .get_conversation_history(
                    user.id,
                    save_context as i64,
                    true,
                )
                .unwrap_or_default();

            let mut context_messages: Vec<ChatMessage> = Vec::new();
            
            // Process messages in chronological order
            for msg in history.into_iter().rev() {
                let content = chat_completion::Content::Text(msg.encrypted_content);
                let role = match msg.role.as_str() {
                    "user" => "user",
                    "assistant" => "assistant",
                    "tool" => "tool",
                    _ => continue,
                };
                
                let mut chat_msg = ChatMessage {
                    role: role.to_string(),
                    content,
                };
                
                context_messages.push(chat_msg);
            }
            
            // Combine system message with conversation history
            chat_messages.extend(context_messages);
        }
    }
    if user.id == 1 {
        println!("history: {:#?}", chat_messages);
    }

    // Handle image if present
    let mut image_url = None;
    
    if let (Some(num_media), Some(media_url), Some(content_type)) = (
        payload.num_media.as_ref(),
        payload.media_url0.as_ref(),
        payload.media_content_type0.as_ref()
    ) {
        if num_media != "0" && content_type.starts_with("image/") {
            image_url = Some(media_url.clone());
            
            tracing::debug!("setting image_url var to: {:#?}", image_url);
            // Add the image URL message with the text
            chat_messages.push(ChatMessage {
                role: "user".to_string(),
                content: chat_completion::Content::ImageUrl(vec![
                    chat_completion::ImageUrl {
                        r#type: chat_completion::ContentType::image_url,
                        text: Some(processed_body.clone()),
                        image_url: Some(chat_completion::ImageUrlType {
                            url: media_url.clone(),
                        }),
                    },
                ]),

            });

            // Also add the text as a separate message if it's not empty 
            if !processed_body.trim().is_empty() {
                chat_messages.push(ChatMessage {
                    role: "user".to_string(),
                    content: chat_completion::Content::Text(format!("Text accompanying the image: {}", processed_body)),
                });
            }
        } else {
            // Add regular text message if no image
            chat_messages.push(ChatMessage {
                role: "user".to_string(),
                content: chat_completion::Content::Text(processed_body),
            });
        }
    } else {
        // Add regular text message if no media
        chat_messages.push(ChatMessage {
            role: "user".to_string(),
            content: chat_completion::Content::Text(processed_body),
        });
    }

    // Define tools
    let tools = vec![
        crate::tool_call_utils::whatsapp::get_send_whatsapp_message_tool(),
        crate::tool_call_utils::whatsapp::get_fetch_whatsapp_messages_tool(),
        crate::tool_call_utils::whatsapp::get_fetch_whatsapp_room_messages_tool(),
        crate::tool_call_utils::whatsapp::get_search_whatsapp_rooms_tool(),
        crate::tool_call_utils::telegram::get_send_telegram_message_tool(),
        crate::tool_call_utils::telegram::get_fetch_telegram_messages_tool(),
        crate::tool_call_utils::telegram::get_fetch_telegram_room_messages_tool(),
        crate::tool_call_utils::telegram::get_search_telegram_rooms_tool(),
        crate::tool_call_utils::email::get_fetch_emails_tool(),
        crate::tool_call_utils::email::get_fetch_specific_email_tool(),
        crate::tool_call_utils::management::get_delete_sms_conversation_history_tool(),
        crate::tool_call_utils::management::get_create_waiting_check_tool(),
        crate::tool_call_utils::internet::get_scan_qr_code_tool(),
        crate::tool_call_utils::internet::get_ask_perplexity_tool(),
        crate::tool_call_utils::internet::get_weather_tool(),
        crate::tool_call_utils::calendar::get_fetch_calendar_event_tool(),
        crate::tool_call_utils::calendar::get_create_calendar_event_tool(),
        crate::tool_call_utils::tasks::get_fetch_tasks_tool(),
        crate::tool_call_utils::tasks::get_create_tasks_tool(),
    ];

    let client = match create_openai_client(&state) {
        Ok(client) => client,
        Err(e) => {
            tracing::error!("Failed to create OpenAI client: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                axum::Json(TwilioResponse {
                    message: "Failed to initialize AI service".to_string(),
                })
            );
        }
    };

    // Convert ChatMessage vec into ChatCompletionMessage vec
    let completion_messages: Vec<chat_completion::ChatCompletionMessage> = chat_messages.clone()
        .into_iter()
        .map(|msg| chat_completion::ChatCompletionMessage {
            role: match msg.role.as_str() {
                "user" => chat_completion::MessageRole::user,
                "assistant" => chat_completion::MessageRole::assistant,
                "system" => chat_completion::MessageRole::system,
                _ => chat_completion::MessageRole::user, // default to user if unknown
            },
            content: msg.content.clone(),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        })
        .collect();

    // right before you convert to ChatCompletionMessage
    for (idx, msg) in chat_messages.iter().enumerate() {
        match &msg.content {
            chat_completion::Content::Text(t) if t.trim().is_empty() => {
                tracing::error!(
                    "⚠️ empty TEXT content at index {idx}; role={}",
                    msg.role
                );
            }
            chat_completion::Content::ImageUrl(urls) => {
                // helpful if an image-only message dropped its text
                if urls.iter().all(|u| u.text.as_deref().unwrap_or("").trim().is_empty()) {
                    tracing::error!("⚠️ image-only message with no text at index {idx}");
                }
            }
            _ => {}
        }
    }

    // If you still want the assert afterwards, leave it here
    assert!(
        chat_messages.iter().all(|m| match &m.content {
            chat_completion::Content::Text(t) => !t.trim().is_empty(),
            _ => true,
        }),
        "Found at least one empty content item – see previous log lines"
    );


    let result = match client.chat_completion(chat_completion::ChatCompletionRequest::new(
        GPT4_O.to_string(),
        completion_messages.clone(),
    )
    .tools(tools)
    .tool_choice(chat_completion::ToolChoiceType::Auto)
    .max_tokens(250)).await {
        Ok(result) => result,
        Err(e) => {
            tracing::error!("Failed to get chat completion: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                axum::Json(TwilioResponse {
                    message: "Failed to process your request".to_string(),
                })
            );
        }
    };

    // TODO remove
    if user.id == 1 {
        println!("result: {:#?}", result);
    }


    let mut fail = false;
    let mut tool_answers: HashMap<String, String> = HashMap::new(); // tool_call id and answer
    let final_response = match result.choices[0].finish_reason {
        None | Some(chat_completion::FinishReason::stop) => {
            tracing::debug!("Model provided direct response (no tool calls needed)");
            // Direct response from the model
            let resp = result.choices[0].message.content.clone().unwrap_or_default();
            resp
        }
        Some(chat_completion::FinishReason::tool_calls) => {
            tracing::debug!("Model requested tool calls - beginning tool execution phase");

                        
            let tool_calls = match result.choices[0].message.tool_calls.as_ref() {
                Some(calls) => {
                    tracing::debug!("Found {} tool call(s) in response", calls.len());
                    calls
                },
                None => {
                    tracing::error!("No tool calls found in response despite tool_calls finish reason");
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        [(axum::http::header::CONTENT_TYPE, "application/json")],
                        axum::Json(TwilioResponse {
                            message: "Failed to process your request".to_string(),
                        })
                    );
                }
            };

            for tool_call in tool_calls {
                let tool_call_id = tool_call.id.clone();
                tracing::debug!("Processing tool call: {:?} with id: {:?}", tool_call, tool_call_id);
                let name = match &tool_call.function.name {
                    Some(n) => {
                        tracing::debug!("Tool call function name: {}", n);
                        n
                    },
                    None => {
                        tracing::debug!("Tool call missing function name, skipping");
                        continue;
                    },
                };

                let tool_call_time= std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i32;

                let history_entry = crate::models::user_models::NewMessageHistory {
                    user_id: user.id,
                    role: "assistant".to_string(),
                    // Store the entire tool-call JSON so it can be replayed later
                    encrypted_content: serde_json::to_string(tool_call)
                        .unwrap_or_else(|_| "{}".to_string()),
                    tool_name: Some(tool_call
                        .function
                        .name
                        .clone()
                        .unwrap_or_else(|| "_tool_call".to_string())),
                    tool_call_id: Some(tool_call.id.clone()),
                    created_at: tool_call_time,
                    conversation_id: conversation.conversation_sid.clone(),
                };

                if let Err(e) = state.user_repository.create_message_history(&history_entry) {
                    tracing::error!("Failed to store tool-call message in history: {e}");
                }

                // Check if user has access to this tool
                if crate::tool_call_utils::utils::requires_subscription(name, user.sub_tier.clone(), user.discount) {
                    tracing::info!("Attempted to use subscription-only tool {} without proper subscription", name);
                    tool_answers.insert(tool_call_id, format!("This feature ({}) requires a subscription. Please visit our website to subscribe.", name));
                    continue;
                }
                let arguments = match &tool_call.function.arguments {
                    Some(args) => args,
                    None => continue,
                };
                if name == "ask_perplexity" {
                    tracing::debug!("Executing ask_perplexity tool call");
                    #[derive(Deserialize, Serialize)]
                    struct PerplexityQuestion {
                        query: String,
                    }

                    let c: PerplexityQuestion = match serde_json::from_str(arguments) {
                        Ok(q) => q,
                        Err(e) => {
                            tracing::error!("Failed to parse perplexity question: {}", e);
                            continue;
                        }
                    };
                    let query = format!("User info: {}. Query: {}", user_info, c.query);

                    let sys_prompt = format!("You are assisting an AI text messaging service. The questions you receive are from text messaging conversations where users are seeking information or help. Please note: 1. Provide clear, conversational responses that can be easily read from a small screen 2. Avoid using any markdown, HTML, or other markup languages 3. Keep responses concise but informative 4. When listing multiple points, use simple numbering (1, 2, 3) 5. Focus on the most relevant information that addresses the user's immediate needs. This is what you should know about the user who this information is going to in their own words: {}", user_info);
                    match crate::utils::tool_exec::ask_perplexity(&state, &query, &sys_prompt).await {
                        Ok(answer) => {
                            tracing::debug!("Successfully received Perplexity answer");
                            tool_answers.insert(tool_call_id, answer);
                        }
                        Err(e) => {
                            tracing::error!("Failed to get perplexity answer: {}", e);
                            continue;
                        }
                    };
                } else if name == "get_weather" {
                    tracing::debug!("Executing get_weather tool call");
                    #[derive(Deserialize, Serialize)]
                    struct WeatherQuestion {
                        location: String,
                        units: String,
                    }
                    let c: WeatherQuestion = match serde_json::from_str(arguments) {
                        Ok(q) => q,
                        Err(e) => {
                            tracing::error!("Failed to parse weather question: {}", e);
                            continue;
                        }
                    };
                    let location= c.location;
                    let units= c.units;

                    match crate::utils::tool_exec::get_weather(&state, &location, &units).await {
                        Ok(answer) => {
                            tracing::debug!("Successfully received weather answer");
                            tool_answers.insert(tool_call_id, answer);
                        }
                        Err(e) => {
                            tracing::error!("Failed to get weather answer: {}", e);
                            continue;
                        }
                    };
                } else if name == "use_shazam" {
                    tool_answers.insert(tool_call_id, "The Shazam feature has been discontinued due to insufficient usage. Thank you for your understanding.".to_string());
                } else if name == "fetch_emails" {
                    tracing::debug!("Executing fetch_emails tool call");
                    let response = crate::tool_call_utils::email::handle_fetch_emails(&state, user.id).await;
                    tool_answers.insert(tool_call_id, response);
                } else if name == "fetch_specific_email" {
                    tracing::debug!("Executing fetch_specific_email tool call");
                    #[derive(Deserialize)]
                    struct EmailQuery {
                        query: String,
                    }
                    
                    let query: EmailQuery = match serde_json::from_str(arguments) {
                        Ok(q) => q,
                        Err(e) => {
                            tracing::error!("Failed to parse email query: {}", e);
                            continue;
                        }
                    };

                    // First get the email ID
                    let email_id = crate::tool_call_utils::email::handle_fetch_specific_email(&state, user.id, &query.query).await;
                    let auth_user = crate::handlers::auth_middleware::AuthUser {
                        user_id: user.id,
                        is_admin: false,
                    };
                    
                    // Then fetch the complete email with that ID
                    match crate::handlers::imap_handlers::fetch_single_imap_email(axum::extract::State(state.clone()), auth_user, axum::extract::Path(email_id)).await {
                        Ok(email) => {
                            let email = &email["email"];
                            
                            // Upload attachments to Twilio if present
                            let mut uploaded_attachments: Vec<(String, String)> = Vec::new(); // (filename, media_sid)
                            if let Some(attachments) = email["attachments"].as_array() {
                                for attachment_url in attachments {
                                    if let Some(url) = attachment_url.as_str() {
                                        // Download attachment content
                                        if let Ok(response) = reqwest::get(url).await {
                                            if let Some(content_type) = response.headers().get("content-type")
                                                .and_then(|ct| ct.to_str().ok())
                                                .map(|s| s.to_string()) {
                                                if let Ok(bytes) = response.bytes().await {
                                                    // Extract filename from URL or use default
                                                    let filename = url.split('/').last()
                                                        .unwrap_or("attachment")
                                                        .to_string();
                                                    
                                                    // Upload to Twilio
                                                    match crate::api::twilio_utils::upload_media_to_twilio(
                                                        &state,
                                                        &conversation.service_sid,
                                                        &bytes,
                                                        &content_type,
                                                        &filename,
                                                        &user
                                                    ).await {
                                                        Ok(media_sid) => {
                                                            // Store in thread-local map
                                                            MEDIA_SID_MAP.with(|map| {
                                                                map.borrow_mut().insert(filename.clone(), media_sid.clone());
                                                            });
                                                            uploaded_attachments.push((filename.clone(), media_sid));
                                                        },
                                                        Err(e) => {
                                                            tracing::error!("Failed to upload attachment to Twilio: {}", e);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            // Format the response with all email details and just filenames for attachments
                            let mut response = format!(
                                "From: {}\nSubject: {}\nDate: {}\n\n{}",
                                email["from"],
                                email["subject"],
                                email["date_formatted"],
                                email["body"]
                            );
                            // Add attachment information with just filenames
                            if !uploaded_attachments.is_empty() {
                                response.push_str("\n\nAttachments:\n");
                                for (filename, _) in &uploaded_attachments {
                                    response.push_str(&format!("- {}\n", filename));
                                }
                            }
                            tool_answers.insert(tool_call_id, response);
                        },
                        Err(e) => {
                            tool_answers.insert(tool_call_id, "Failed to fetch the complete email".to_string());
                        }
                    }
                } else if name == "create_waiting_check" {
                    tracing::debug!("Executing create_waiting_check tool call");
                    match crate::tool_call_utils::management::handle_create_waiting_check(&state, user.id, arguments).await {
                        Ok(answer) => {
                            tool_answers.insert(tool_call_id, answer);
                        }
                        Err(e) => {
                            tracing::error!("Failed to create waiting check: {}", e);
                            tool_answers.insert(tool_call_id, "Sorry, I couldn't create a waiting check. (Contact rasmus@ahtava.com pls:D)".to_string());
                        }
                    }
                } else if name == "create_calendar_event" {
                    tracing::debug!("Executing create_calendar_event tool call");
                    match crate::tool_call_utils::calendar::handle_create_calendar_event(
                        &state,
                        user.id,
                        &conversation.conversation_sid,
                        &conversation.twilio_number,
                        arguments,
                        &user,
                    ).await {
                        Ok((status, headers, Json(twilio_response))) => {
                            let history_entry = crate::models::user_models::NewMessageHistory {
                                user_id: user.id,
                                role: "assistant".to_string(),
                                encrypted_content: twilio_response.message.clone(),
                                tool_name: Some("create_calendar_event".to_string()),
                                tool_call_id: Some(tool_call.id.clone()),
                                created_at: chrono::Utc::now().timestamp() as i32,
                                conversation_id: conversation.conversation_sid.clone(),
                            };

                            if let Err(e) = state.user_repository.create_message_history(&history_entry) {
                                tracing::error!("Failed to store calendar tool message in history: {}", e);
                            }

                            return (status, headers, Json(twilio_response));
                        }
                        Err(e) => {
                            tracing::error!("Failed to handle calendar event creation: {}", e);
                            return (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                [(axum::http::header::CONTENT_TYPE, "application/json")],
                                axum::Json(TwilioResponse {
                                    message: "Failed to process calendar event request".to_string(),
                                })
                            );
                        }
                    }
                } else if name == "create_task" {
                    tracing::debug!("Executing create_task tool call");
                    let response = crate::tool_call_utils::tasks::handle_create_task(&state, user.id, arguments).await;
                    tool_answers.insert(tool_call_id, response);
                } else if name == "fetch_tasks" {
                    tracing::debug!("Executing fetch_tasks tool call");
                    let response = crate::tool_call_utils::tasks::handle_fetch_tasks(&state, user.id, arguments).await;
                    tool_answers.insert(tool_call_id, response);
                } else if name == "send_whatsapp_message" {
                    tracing::debug!("Executing send_whatsapp_message tool call");
                    match crate::tool_call_utils::whatsapp::handle_send_whatsapp_message(
                        &state,
                        user.id,
                        &conversation.conversation_sid,
                        &conversation.twilio_number,
                        arguments,
                        &user,
                        image_url.as_deref(),
                    ).await {
                        Ok((status, headers, Json(twilio_response))) => {
                            // Store the assistant's sent message in history
                            let history_entry = crate::models::user_models::NewMessageHistory {
                                user_id: user.id,
                                role: "assistant".to_string(),
                                encrypted_content: twilio_response.message.clone(),
                                tool_name: Some("send_whatsapp_message".to_string()),
                                tool_call_id: Some(tool_call.id.clone()),
                                created_at: chrono::Utc::now().timestamp() as i32,
                                conversation_id: conversation.conversation_sid.clone(),
                            };
                            if let Err(e) = state.user_repository.create_message_history(&history_entry) {
                                tracing::error!("Failed to store WhatsApp tool message in history: {}", e);
                            }
                            return (status, headers, Json(twilio_response));
                        }
                        Err(e) => {
                            tracing::error!("Failed to handle WhatsApp message sending: {}", e);
                        }
                    }

                } else if name == "send_telegram_message" {
                    tracing::debug!("Executing send_telegram_message tool call");
                    match crate::tool_call_utils::telegram::handle_send_telegram_message(
                        &state,
                        user.id,
                        &conversation.conversation_sid,
                        &conversation.twilio_number,
                        arguments,
                        &user,
                        image_url.as_deref(),
                    ).await {
                        Ok((status, headers, Json(twilio_response))) => {
                            let history_entry = crate::models::user_models::NewMessageHistory {
                                user_id: user.id,
                                role: "assistant".to_string(),
                                encrypted_content: twilio_response.message.clone(),
                                tool_name: Some("send_telegram_message".to_string()),
                                tool_call_id: Some(tool_call.id.clone()),
                                created_at: chrono::Utc::now().timestamp() as i32,
                                conversation_id: conversation.conversation_sid.clone(),
                            };
                            if let Err(e) = state.user_repository.create_message_history(&history_entry) {
                                tracing::error!("Failed to store send telegram message tool message in history: {}", e);
                            }
                            return (status, headers, Json(twilio_response));
                        }

                        Err(e) => {
                            tracing::error!("Failed to handle Telegram message sending: {}", e);
                            return (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                [(axum::http::header::CONTENT_TYPE, "application/json")],
                                axum::Json(TwilioResponse {
                                    message: "Failed to process Telegram message request".to_string(),
                                })
                            );
                        }
                    }
                } else if name == "search_whatsapp_rooms" {
                    tracing::debug!("Executing search_whatsapp_rooms tool call");
                    let response = crate::tool_call_utils::whatsapp::handle_search_whatsapp_rooms(
                        &state,
                        user.id,
                        arguments,
                    ).await;
                    tool_answers.insert(tool_call_id, response);
                } else if name == "search_telegram_rooms" {
                    tracing::debug!("Executing search_telegram_rooms tool call");
                    let response = crate::tool_call_utils::telegram::handle_search_telegram_rooms(
                        &state,
                        user.id,
                        arguments,
                    ).await;
                    tool_answers.insert(tool_call_id, response);
                } else if name == "fetch_whatsapp_room_messages" {
                    tracing::debug!("Executing fetch_whatsapp_room_messages tool call");
                    let response = crate::tool_call_utils::whatsapp::handle_fetch_whatsapp_room_messages(
                        &state,
                        user.id,
                        arguments,
                    ).await;
                    tool_answers.insert(tool_call_id, response);
                } else if name == "fetch_telegram_room_messages" {
                    tracing::debug!("Executing fetch_telegram_room_messages tool call");
                    let response = crate::tool_call_utils::telegram::handle_fetch_telegram_room_messages(
                        &state,
                        user.id,
                        arguments,
                    ).await;
                    tool_answers.insert(tool_call_id, response);
                } else if name == "fetch_whatsapp_messages" {
                    tracing::debug!("Executing fetch_whatsapp_messages tool call");
                    let response = crate::tool_call_utils::whatsapp::handle_fetch_whatsapp_messages(
                        &state,
                        user.id,
                        arguments,
                    ).await;
                    tool_answers.insert(tool_call_id, response);
                } else if name == "fetch_telegram_messages" {
                    tracing::debug!("Executing fetch_telegram_messages tool call");
                    let response = crate::tool_call_utils::telegram::handle_fetch_telegram_messages(
                        &state,
                        user.id,
                        arguments,
                    ).await;
                    tool_answers.insert(tool_call_id, response);
                } else if name == "scan_qr_code" {
                    tracing::debug!("Executing scan_qr_code tool call with url: {:#?}", image_url);
                    let response = crate::tool_call_utils::internet::handle_qr_scan(image_url.as_deref()).await;
                    tool_answers.insert(tool_call_id, response);
                } else if name == "delete_sms_conversation_history" {
                    tracing::debug!("Executing delete_sms_conversation_history tool call");
                    match crate::api::twilio_utils::delete_bot_conversations(&state, &user.phone_number, &user).await {
                        Ok(_) => {
                            tool_answers.insert(tool_call_id, "Successfully deleted all bot conversations.".to_string());
                        }
                        Err(e) => {
                            tracing::error!("Failed to delete bot conversations: {}", e);
                            tool_answers.insert(tool_call_id, 
                                format!("Failed to delete conversations: {}", e)
                            );
                        }
                    }
                } else if name == "fetch_calendar_events" {
                    tracing::debug!("Executing fetch_calendar_events tool call");
                    let response = crate::tool_call_utils::calendar::handle_fetch_calendar_events(
                        &state,
                        user.id,
                        arguments,
                    ).await;
                    tool_answers.insert(tool_call_id, response);
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
                    // TODO remove
                    if user.id == 1 {
                        println!("response: {}", tool_answer);
                    }
                    follow_up_messages.push(chat_completion::ChatCompletionMessage {
                        role: chat_completion::MessageRole::tool,
                        content: chat_completion::Content::Text(tool_answer),
                        name: None,
                        tool_calls: None,
                        tool_call_id: Some(tool_call.id.clone()),
                    });
                }
            }

            let current_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i32;

            // Store tool responses in history
            for (tool_call_id, tool_response) in tool_answers.iter() {
                let tool_message = crate::models::user_models::NewMessageHistory {
                    user_id: user.id,
                    role: "tool".to_string(),
                    encrypted_content: tool_response.clone(),
                    tool_name: None, // We could store this if needed
                    tool_call_id: Some(tool_call_id.clone()),
                    created_at: current_time,
                    conversation_id: conversation.conversation_sid.clone(),
                };

                if let Err(e) = state.user_repository.create_message_history(&tool_message) {
                    tracing::error!("Failed to store tool response in history: {}", e);
                }
            }


            tracing::debug!("Making follow-up request to model with tool call answers");
            let follow_up_req = chat_completion::ChatCompletionRequest::new(
                GPT4_O.to_string(),
                follow_up_messages,
            )
            .max_tokens(100); // Consistent token limit for follow-up messages

            match client.chat_completion(follow_up_req).await {
                Ok(follow_up_result) => {
                    tracing::debug!("Received follow-up response from model");
                    let response = follow_up_result.choices[0].message.content.clone().unwrap_or_default();
                    response
                }
                Err(e) => {
                    tracing::error!("Failed to get follow-up completion: {}", e);
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


    let should_charge = if user.free_reply && !payload.body.to_lowercase().starts_with("forget") {
        false
    } else {
        true
    };
    if let Err(e) = state.user_core.set_free_reply(user.id, false) {
        tracing::error!("Failed to reset set_free_reply flag: {}", e);
    }

    // Perform clarification check
    let (is_clarifying, clarify_explanation) = crate::tool_call_utils::utils::perform_clarification_check(
        &client,
        &chat_messages,
        &payload.body,
        &final_response
    ).await;

    // Perform evaluation
    let (eval_result, eval_reason) = crate::tool_call_utils::utils::perform_evaluation(
        &client,
        &chat_messages,
        &payload.body,
        &final_response,
        fail
    ).await;

    let mut final_response_with_notice = final_response.clone();
    if is_clarifying {
        if let Err(e) = state.user_core.set_free_reply(user.id, true) {
            tracing::error!("Failed to set the set_free_reply flag: {}", e);
        }
        final_response_with_notice = format!("{} (free reply)", final_response);
    }
    tracing::debug!("is_clarifying message: {}", is_clarifying);

    let status = if should_charge {"charging".to_string()} else {"this was free reply".to_string()};
    tracing::debug!("STATUS: {}", status);
    let mut final_eval: String = "".to_string();
    if let Some(eval) = eval_reason {
        if let Some(clarify_expl) = clarify_explanation {
            final_eval = format!("success reason: {}; clarifying explanation: {}",eval,clarify_expl);
        } else {
            final_eval = format!("success reason: {}", eval);
        }
    } else if let Some(clarify_expl) = clarify_explanation {
        final_eval = format!("clarifying reason: {}", clarify_expl);
    }
    tracing::debug!("FINAL_EVAL: {}", final_eval);

    let processing_time_secs = start_time.elapsed().as_secs(); // Calculate processing time



    // Clean up old message history based on save_context setting
    let save_context = user_settings.save_context.unwrap_or(0);
    if let Err(e) = state.user_repository.delete_old_message_history(
        user.id,
        Some(&conversation.conversation_sid),
        save_context as i64
    ) {
        tracing::error!("Failed to clean up old message history: {}", e);
    }

    let current_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    // If in test mode, skip sending the actual message and return the response directly
    if is_test {

        // Log the test usage without actually sending the message
        if let Err(e) = state.user_repository.log_usage(
            user.id,
            None,  // No message SID in test mode
            "sms_test".to_string(),
            None,
            Some(processing_time_secs as i32),
            Some(eval_result),
            Some(final_eval),
            Some(status),
            None,
            None
        ) {
            tracing::error!("Failed to log test SMS usage: {}", e);
        }

        return (
            StatusCode::OK,
            [(axum::http::header::CONTENT_TYPE, "application/json")],
            axum::Json(TwilioResponse {
                message: final_response_with_notice,
            })
        );
    }

    // Extract filenames from the response and look up their media SIDs
    let mut media_sids = Vec::new();
    let clean_response = final_response_with_notice.lines().filter_map(|line| {
        // Look for lines that contain filenames from the media map
        MEDIA_SID_MAP.with(|map| {
            let map = map.borrow();
            for (filename, media_sid) in map.iter() {
                if line.contains(filename) {
                    media_sids.push(media_sid.clone());
                    return None; // Remove the line containing the filename
                }
            }
            Some(line.to_string())
        })
    }).collect::<Vec<String>>().join("\n");

    let media_sid = media_sids.first();
    let state_clone = state.clone();
    let msg_sid = payload.message_sid.clone();
    let user_clone = user.clone();

    tracing::debug!("going into deleting the incoming message handler");
    tokio::spawn(async move {
        if let Err(e) = crate::api::twilio_utils::delete_twilio_message(&state_clone, &msg_sid, &user_clone).await {
            tracing::error!("Failed to delete incoming message {}: {}", msg_sid, e);
        }
    });

    // Send the actual message if not in test mode
    match crate::api::twilio_utils::send_conversation_message(
        &state,
        &conversation.conversation_sid,
        &conversation.twilio_number,
        &clean_response,
        media_sid,
        &user
    ).await {
        Ok(message_sid) => {
            // Log the SMS usage metadata and store message history
            tracing::debug!("status of the message: {}", status);
            

            // Log usage
            if let Err(e) = state.user_repository.log_usage(
                user.id,
                Some(message_sid.clone()),
                "sms".to_string(),
                None,
                Some(processing_time_secs as i32),
                Some(eval_result),
                Some(final_eval.clone()),
                Some(status.clone()),
                None,
                None,
            ) {
                tracing::error!("Failed to log SMS usage: {}", e);
            }


            if should_charge {
                // Only deduct credits if we should charge
                if let Err(e) = crate::utils::usage::deduct_user_credits(&state, user.id, "message", None) {
                    tracing::error!("Failed to deduct user credits: {}", e);
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        [(axum::http::header::CONTENT_TYPE, "application/json")],
                        axum::Json(TwilioResponse {
                            message: "Failed to process credits points".to_string(),
                        })
                    );
                }
                        
                match state.user_repository.is_credits_under_threshold(user.id) {
                    Ok(is_under) => {
                        if is_under {
                            tracing::debug!("User {} credits is under threshold, attempting automatic charge", user.id);
                            // Get user information
                            if user.charge_when_under {
                                use axum::extract::{State, Path};
                                let state_clone = Arc::clone(&state);
                                tokio::spawn(async move {
                                    let _ = crate::handlers::stripe_handlers::automatic_charge(
                                        State(state_clone),
                                        Path(user.id),
                                    ).await;
                                    tracing::debug!("Recharged the user successfully back up!");
                                });
                            }
                        }
                    },
                    Err(e) => tracing::error!("Failed to check if user credits is under threshold: {}", e),
                }
            }

            (
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                axum::Json(TwilioResponse {
                    message: "Message sent successfully".to_string(),
                })
            )
        }
        Err(e) => {
            tracing::error!("Failed to send conversation message: {}", e);
            // Log the failed attempt with error message in status
            let error_status = format!("failed to send: {}", e);
            if let Err(log_err) = state.user_repository.log_usage(
                user.id,
                None,
                "sms".to_string(),
                None,
                Some(processing_time_secs as i32),
                Some(false),  // Mark as unsuccessful
                Some(final_eval),
                Some(error_status),
                None,
                None,
            ) {
                tracing::error!("Failed to log SMS usage after send error: {}", log_err);
            }
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                axum::Json(TwilioResponse {
                    message: "Failed to send message".to_string(),
                })
            )
        }
    }
}

