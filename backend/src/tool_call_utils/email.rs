use crate::AppState;
use std::sync::Arc;

pub fn get_send_email_tool() -> openai_api_rs::v1::chat_completion::Tool {
    get_send_email_tool_for_user(&[])
}

pub fn get_send_email_tool_for_user(
    selectors: &[String],
) -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;
    let mut properties = HashMap::new();
    properties.insert(
        "to".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The recipient's email address or contact name (e.g., 'mom@email.com' or 'Mom'). If a name is used, the email address from their contact record will be used.".to_string()),
            ..Default::default()
        }),
    );
    properties.insert(
        "subject".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The subject of the email".to_string()),
            ..Default::default()
        }),
    );
    properties.insert(
        "body".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The body content of the email".to_string()),
            ..Default::default()
        }),
    );
    if !selectors.is_empty() {
        let desc = format!(
            "The connected inbox nickname or sender email address to use. Available selectors: {}",
            selectors.join(", ")
        );
        properties.insert(
            "from".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some(desc),
                enum_values: Some(selectors.to_vec()),
                ..Default::default()
            }),
        );
    }
    properties.insert(
        "notify_on_reply".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Boolean),
            description: Some(
                "Optional. Defaults to false. Set to true ONLY when the user explicitly asks to be told when the recipient replies (e.g. \"email Sara and let me know what she says\"). When true, the first email from that recipient to the same account in the next 24 hours is forwarded to the user via SMS. After that one reply, the watch ends automatically.".to_string()
            ),
            ..Default::default()
        }),
    );
    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("send_email"),
            description: Some(String::from(
                "Sends an email immediately. For future-scheduled emails, use create_item instead.",
            )),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(vec![
                    String::from("to"),
                    String::from("subject"),
                    String::from("body"),
                ]),
            },
        },
    }
}

pub fn get_respond_to_email_tool() -> openai_api_rs::v1::chat_completion::Tool {
    get_respond_to_email_tool_for_user(&[])
}

pub fn get_respond_to_email_tool_for_user(
    selectors: &[String],
) -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;
    let mut properties = HashMap::new();
    properties.insert(
        "email_id".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The stable [id=N] returned by query_message for the email to reply to. Legacy IMAP UIDs are also accepted when `from` identifies the inbox.".to_string()),
            ..Default::default()
        }),
    );
    properties.insert(
        "response_text".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The text content of the response".to_string()),
            ..Default::default()
        }),
    );
    if !selectors.is_empty() {
        let desc = format!(
            "The connected inbox nickname or sender email address to use for the reply. Available selectors: {}",
            selectors.join(", ")
        );
        properties.insert(
            "from".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some(desc),
                enum_values: Some(selectors.to_vec()),
                ..Default::default()
            }),
        );
    }
    properties.insert(
        "notify_on_reply".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Boolean),
            description: Some(
                "Optional. Defaults to false. Set to true ONLY when the user explicitly asks to be told when the recipient replies back. When true, the first email from that recipient to the same account in the next 24 hours is forwarded to the user via SMS. After that one reply, the watch ends automatically.".to_string()
            ),
            ..Default::default()
        }),
    );
    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("respond_to_email"),
            description: Some(String::from("Queues a response to a specific email with a 60-second delay, allowing the user to cancel by replying 'cancel'. Use this when the user wants to reply to an email. The response will use the original email's subject with 'Re: ' prefixed automatically.")),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(vec![String::from("email_id"), String::from("response_text")]),
            },
        },
    }
}

use crate::repositories::user_repository::ImapConnectionInfo;
use serde::Deserialize;

/// Resolve which email account to use based on the `from` parameter.
/// - If `from` is specified, look up that nickname or email address.
/// - If only one account exists, use it.
/// - If multiple accounts exist and `from` is not specified, return an error message.
pub fn resolve_email_account(
    state: &Arc<AppState>,
    user_id: i32,
    from: &Option<String>,
) -> Result<ImapConnectionInfo, String> {
    if let Some(selector) = from {
        state
            .user_repository
            .get_imap_credentials_by_selector(user_id, selector)
            .map_err(|_| "Could not look up connected inboxes. Please try again.".to_string())?
            .ok_or_else(|| {
                format!(
                    "No connected inbox matches '{}'. Please choose a listed nickname or email address.",
                    selector
                )
            })
    } else {
        let accounts = state
            .user_repository
            .get_all_imap_credentials(user_id)
            .map_err(|_| "Could not look up connected inboxes. Please try again.".to_string())?;
        match accounts.len() {
            0 => Err("No email account connected.".to_string()),
            1 => Ok(accounts.into_iter().next().unwrap()),
            _ => {
                let selectors: Vec<String> = accounts
                    .iter()
                    .map(|a| a.nickname.clone().unwrap_or_else(|| a.email.clone()))
                    .collect();
                Err(format!(
                    "Multiple inboxes are connected ({}). Please specify which one to use.",
                    selectors.join(", ")
                ))
            }
        }
    }
}

/// Resolve a reply reference without trusting the model to pair a mailbox-
/// local IMAP UID with the correct account. Stable ontology message IDs carry
/// the account ID in their room identity; legacy UID + `from` remains
/// supported for existing notification history.
pub fn resolve_email_reply_target(
    state: &Arc<AppState>,
    user_id: i32,
    email_id: &str,
    from: &Option<String>,
) -> Result<(ImapConnectionInfo, String), String> {
    if let Ok(message_id) = email_id.parse::<i64>() {
        let message = state
            .ontology_repository
            .get_message_by_id_for_user(user_id, message_id)
            .map_err(|_| "Could not look up that email. Please try again.".to_string())?;
        if let Some(message) = message.filter(|message| message.platform == "email") {
            if let Some(identity) =
                crate::handlers::imap_handlers::parse_email_room_id(&message.room_id)
            {
                if let Some(connection_id) = identity.imap_connection_id {
                    let (owner_id, account, status) = state
                        .user_repository
                        .get_imap_connection_by_id(connection_id)
                        .map_err(|_| "Could not look up that inbox. Please try again.".to_string())?
                        .ok_or_else(|| {
                            "The inbox for that email is no longer connected.".to_string()
                        })?;
                    if owner_id != user_id || status != "active" {
                        return Err("The inbox for that email is no longer connected.".to_string());
                    }
                    if let Some(selector) = from {
                        let selected =
                            resolve_email_account(state, user_id, &Some(selector.clone()))?;
                        if selected.id != account.id {
                            return Err(
                                "That email belongs to a different connected inbox. Please confirm which email you want to reply to."
                                    .to_string(),
                            );
                        }
                    }
                    return Ok((account, identity.uid));
                }
            }
        }
    }

    let account = resolve_email_account(state, user_id, from)?;
    Ok((account, email_id.to_string()))
}

#[derive(Deserialize, Debug)]
pub struct SendEmailArgs {
    pub to: String,
    pub subject: String,
    pub body: String,
    pub from: Option<String>,
    #[serde(default)]
    pub notify_on_reply: bool,
}
pub async fn handle_send_email(
    state: &Arc<AppState>,
    user_id: i32,
    args: &str,
    user: &crate::models::user_models::User,
    skip_sms: bool,
) -> Result<
    (
        axum::http::StatusCode,
        [(axum::http::HeaderName, &'static str); 1],
        axum::Json<crate::api::twilio_sms::TwilioResponse>,
    ),
    Box<dyn std::error::Error>,
> {
    let args: SendEmailArgs = serde_json::from_str(args)?;

    // Resolve the requested sender nickname or direct email address.
    let sender_account = match resolve_email_account(state, user_id, &args.from) {
        Ok(account) => account,
        Err(msg) => {
            return Ok((
                axum::http::StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                axum::Json(crate::api::twilio_sms::TwilioResponse {
                    message: msg,
                    created_item_id: None,
                }),
            ));
        }
    };
    let from_email = sender_account.email.clone();

    // Check if 'to' is a contact name and resolve to email address
    let recipient_email = if args.to.contains('@') {
        // Already an email address
        args.to.clone()
    } else {
        // Try ontology Person email channel
        if let Ok(Some(person)) = state
            .ontology_repository
            .find_person_by_name(user_id, &args.to)
        {
            if let Some(email_addr) = person
                .channels
                .iter()
                .find(|c| c.platform == "email")
                .and_then(|c| c.handle.clone())
            {
                email_addr
            } else {
                return Ok((
                    axum::http::StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    axum::Json(crate::api::twilio_sms::TwilioResponse {
                        message: format!("Contact '{}' doesn't have an email address.", args.to),
                        created_item_id: None,
                    }),
                ));
            }
        } else {
            // Not a valid email and no matching contact
            return Ok((
                axum::http::StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                axum::Json(crate::api::twilio_sms::TwilioResponse {
                    message: format!("'{}' is not a valid email address and no matching contact was found. Please provide an email address.", args.to),
                    created_item_id: None,
                })
            ));
        }
    };

    // Non-SMS surfaces cannot receive the later cancellation/failure text.
    // Send synchronously there so a failed provider attempt is not reported
    // as a successful queue operation.
    if skip_sms {
        let request = crate::handlers::imap_handlers::SendEmailRequest {
            to: recipient_email.clone(),
            subject: args.subject.clone(),
            body: args.body.clone(),
            from: Some(from_email.clone()),
        };
        return match crate::handlers::imap_handlers::send_email(
            axum::extract::State(state.clone()),
            crate::handlers::auth_middleware::AuthUser {
                user_id,
                is_admin: false,
            },
            axum::Json(request),
        )
        .await
        {
            Ok(_) => {
                if args.notify_on_reply {
                    if let Some(key) =
                        crate::handlers::imap_handlers::normalize_email_sender_key(&recipient_email)
                    {
                        let _ = state.pending_reply_watches_repository.arm_email(
                            user_id,
                            sender_account.id,
                            &key,
                            &args.to,
                        );
                    }
                }
                Ok((
                    axum::http::StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    axum::Json(crate::api::twilio_sms::TwilioResponse {
                        message: "Email sent".to_string(),
                        created_item_id: None,
                    }),
                ))
            }
            Err((status, error)) => Ok((
                status,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                axum::Json(crate::api::twilio_sms::TwilioResponse {
                    message: error
                        .0
                        .get("error")
                        .and_then(|value| value.as_str())
                        .unwrap_or("The email could not be sent. Please try again.")
                        .to_string(),
                    created_item_id: None,
                }),
            )),
        };
    }

    // Format the queued message
    let queued_msg = format!(
        "Will send email to {} with subject '{}' and body '{}' in 60s. Reply 'C' to discard.",
        recipient_email, args.subject, args.body
    );
    // Send the queued confirmation via SMS (skip when from web dashboard)
    if !skip_sms {
        match state
            .channel_router
            .send_to_user(user, &queued_msg, None)
            .await
        {
            Ok(_) => {
                // SMS credits deducted at Twilio status callback
            }
            Err(e) => {
                eprintln!("Failed to send queued message: {}", e);
                return Ok((
                    axum::http::StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    axum::Json(crate::api::twilio_sms::TwilioResponse {
                        message: "Failed to send message queue notification".to_string(),
                        created_item_id: None,
                    }),
                ));
            }
        }
    }
    // Create cancellation channel
    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
    // Spawn the delayed send task
    let cloned_state = state.clone();
    let cloned_user_id = user_id;
    let cloned_user = user.clone();
    let cloned_to = recipient_email.clone();
    let cloned_subject = args.subject.clone();
    let cloned_body = args.body.clone();
    let cloned_from = Some(from_email);
    let cloned_skip_sms = skip_sms;
    let cloned_notify_on_reply = args.notify_on_reply;
    let cloned_imap_connection_id = sender_account.id;
    let cloned_recipient_display = if args.to.contains('@') {
        recipient_email.clone()
    } else {
        args.to.clone()
    };
    tokio::spawn(async move {
        let reason = tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => "timeout",
            _ = cancel_rx => "cancel",
        };
        if reason == "timeout" {
            let email_request = crate::handlers::imap_handlers::SendEmailRequest {
                to: cloned_to.clone(),
                subject: cloned_subject,
                body: cloned_body,
                from: cloned_from,
            };
            match crate::handlers::imap_handlers::send_email(
                axum::extract::State(cloned_state.clone()),
                crate::handlers::auth_middleware::AuthUser {
                    user_id: cloned_user_id,
                    is_admin: false,
                },
                axum::Json(email_request),
            )
            .await
            {
                Ok(_) => {
                    if cloned_notify_on_reply {
                        if let Some(key) =
                            crate::handlers::imap_handlers::normalize_email_sender_key(&cloned_to)
                        {
                            match cloned_state.pending_reply_watches_repository.arm_email(
                                cloned_user_id,
                                cloned_imap_connection_id,
                                &key,
                                &cloned_recipient_display,
                            ) {
                                Ok(_) => {
                                    tracing::info!(
                                    "REPLY_WATCH armed email watch user={} account={} recipient={}",
                                    cloned_user_id, cloned_imap_connection_id, key
                                )
                                }
                                Err(e) => tracing::warn!(
                                    "REPLY_WATCH failed to arm email watch user={}: {}",
                                    cloned_user_id,
                                    e
                                ),
                            }
                        } else {
                            tracing::warn!(
                                "REPLY_WATCH skipping email watch user={}: recipient '{}' did not normalize",
                                cloned_user_id, cloned_to
                            );
                        }
                    }
                }
                Err((_, error_json)) => {
                    let error_msg = format!(
                        "Failed to send email: {}",
                        error_json
                            .0
                            .get("error")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Unknown error")
                    );
                    if !cloned_skip_sms {
                        if let Err(e) = cloned_state
                            .channel_router
                            .send_to_user(&cloned_user, &error_msg, None)
                            .await
                        {
                            eprintln!("Failed to send error message: {}", e);
                        }
                    }
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
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        axum::Json(crate::api::twilio_sms::TwilioResponse {
            message: "Email queued".to_string(),
            created_item_id: None,
        }),
    ))
}

use crate::handlers::auth_middleware::AuthUser;
use axum::extract::{Json, State};

#[derive(Debug, Deserialize)]
pub struct RespondToEmailArgs {
    pub email_id: String,
    pub response_text: String,
    pub from: Option<String>,
    #[serde(default)]
    pub notify_on_reply: bool,
}
pub async fn handle_respond_to_email(
    state: &Arc<AppState>,
    user_id: i32,
    args: &str,
    user: &crate::models::user_models::User,
    skip_sms: bool,
) -> Result<
    (
        axum::http::StatusCode,
        [(axum::http::HeaderName, &'static str); 1],
        axum::Json<crate::api::twilio_sms::TwilioResponse>,
    ),
    Box<dyn std::error::Error>,
> {
    let args: RespondToEmailArgs = serde_json::from_str(args)?;

    let (sender_account, resolved_email_uid) =
        match resolve_email_reply_target(state, user_id, &args.email_id, &args.from) {
            Ok(target) => target,
            Err(msg) => {
                return Ok((
                    axum::http::StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    axum::Json(crate::api::twilio_sms::TwilioResponse {
                        message: msg,
                        created_item_id: None,
                    }),
                ));
            }
        };
    let from_email = sender_account.email.clone();

    // Fetch the email details to get the subject
    let email_details =
        match crate::handlers::imap_handlers::fetch_single_email_imap_for_connection(
            state,
            user_id,
            sender_account.id,
            &resolved_email_uid,
        )
        .await
        {
            Ok(details) => details,
            Err(error) => {
                tracing::warn!(
                    "Failed to fetch email details for user {} account {}: {:?}",
                    user_id,
                    sender_account.id,
                    error
                );
                let error_msg =
                    "Could not read that email from the selected inbox. Please try again."
                        .to_string();
                if !skip_sms {
                    if let Err(e) = state
                        .channel_router
                        .send_to_user(user, &error_msg, None)
                        .await
                    {
                        eprintln!("Failed to send error message: {}", e);
                    }
                }
                return Ok((
                    axum::http::StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    axum::Json(crate::api::twilio_sms::TwilioResponse {
                        message: error_msg,
                        created_item_id: None,
                    }),
                ));
            }
        };
    let subject = email_details
        .subject
        .as_deref()
        .unwrap_or("Unknown subject")
        .to_string();
    let original_from_email = email_details.from_email.clone();
    let original_from_display = email_details
        .from
        .clone()
        .or_else(|| original_from_email.clone())
        .unwrap_or_else(|| "Unknown sender".to_string());

    if skip_sms {
        let request = crate::handlers::imap_handlers::EmailResponseRequest {
            email_id: resolved_email_uid,
            response_text: args.response_text,
            from: Some(from_email),
        };
        return match crate::handlers::imap_handlers::respond_to_email(
            State(state.clone()),
            AuthUser {
                user_id,
                is_admin: false,
            },
            Json(request),
        )
        .await
        {
            Ok(_) => {
                if args.notify_on_reply {
                    if let Some(key) = original_from_email
                        .as_deref()
                        .and_then(crate::handlers::imap_handlers::normalize_email_sender_key)
                    {
                        let _ = state.pending_reply_watches_repository.arm_email(
                            user_id,
                            sender_account.id,
                            &key,
                            &original_from_display,
                        );
                    }
                }
                Ok((
                    axum::http::StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    axum::Json(crate::api::twilio_sms::TwilioResponse {
                        message: "Email reply sent".to_string(),
                        created_item_id: None,
                    }),
                ))
            }
            Err((status, error)) => Ok((
                status,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                axum::Json(crate::api::twilio_sms::TwilioResponse {
                    message: error
                        .0
                        .get("error")
                        .and_then(|value| value.as_str())
                        .unwrap_or("The reply could not be sent. Please try again.")
                        .to_string(),
                    created_item_id: None,
                }),
            )),
        };
    }
    // Format the queued message using the subject
    let queued_msg = format!(
        "Will respond to email '{}' with '{}' in 60s. Reply 'C' to discard.",
        subject, args.response_text
    );
    // Send the queued confirmation via SMS (skip when from web dashboard)
    if !skip_sms {
        match state
            .channel_router
            .send_to_user(user, &queued_msg, None)
            .await
        {
            Ok(_) => {
                // SMS credits deducted at Twilio status callback
            }
            Err(e) => {
                eprintln!("Failed to send queued message: {}", e);
                return Ok((
                    axum::http::StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    axum::Json(crate::api::twilio_sms::TwilioResponse {
                        message: "Failed to send message queue notification".to_string(),
                        created_item_id: None,
                    }),
                ));
            }
        }
    }
    // Create cancellation channel
    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
    // Spawn the delayed send task
    let cloned_state = state.clone();
    let cloned_user_id = user_id;
    let cloned_user = user.clone();
    let cloned_email_id = resolved_email_uid;
    let cloned_response_text = args.response_text.clone();
    let cloned_from = Some(from_email);
    let cloned_skip_sms = skip_sms;
    let cloned_notify_on_reply = args.notify_on_reply;
    let cloned_imap_connection_id = sender_account.id;
    let cloned_original_from_email = original_from_email;
    let cloned_original_from_display = original_from_display;
    tokio::spawn(async move {
        let reason = tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => "timeout",
            _ = cancel_rx => "cancel",
        };
        if reason == "timeout" {
            let request = crate::handlers::imap_handlers::EmailResponseRequest {
                email_id: cloned_email_id,
                response_text: cloned_response_text,
                from: cloned_from,
            };
            match crate::handlers::imap_handlers::respond_to_email(
                State(cloned_state.clone()),
                AuthUser {
                    user_id: cloned_user_id,
                    is_admin: false,
                },
                Json(request),
            )
            .await
            {
                Ok(_) => {
                    if cloned_notify_on_reply {
                        match cloned_original_from_email.as_deref().and_then(
                            crate::handlers::imap_handlers::normalize_email_sender_key,
                        ) {
                            Some(key) => {
                                match cloned_state.pending_reply_watches_repository.arm_email(
                                    cloned_user_id,
                                    cloned_imap_connection_id,
                                    &key,
                                    &cloned_original_from_display,
                                ) {
                                    Ok(_) => tracing::info!(
                                        "REPLY_WATCH armed email watch user={} account={} recipient={}",
                                        cloned_user_id, cloned_imap_connection_id, key
                                    ),
                                    Err(e) => tracing::warn!(
                                        "REPLY_WATCH failed to arm email watch user={}: {}",
                                        cloned_user_id, e
                                    ),
                                }
                            }
                            None => tracing::warn!(
                                "REPLY_WATCH skipping email watch user={}: no sender email for replied email",
                                cloned_user_id
                            ),
                        }
                    }
                }
                Err((_, error_json)) => {
                    let error_msg = format!(
                        "Failed to respond to email: {}",
                        error_json
                            .0
                            .get("error")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Unknown error")
                    );
                    if !cloned_skip_sms {
                        if let Err(e) = cloned_state
                            .channel_router
                            .send_to_user(&cloned_user, &error_msg, None)
                            .await
                        {
                            eprintln!("Failed to send error message: {}", e);
                        }
                    }
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
        axum::http::StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        axum::Json(crate::api::twilio_sms::TwilioResponse {
            message: "Email response queued".to_string(),
            created_item_id: None,
        }),
    ))
}
