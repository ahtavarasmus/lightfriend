use crate::AppState;
use std::sync::Arc;

pub fn get_send_email_tool() -> openai_api_rs::v1::chat_completion::Tool {
    get_send_email_tool_for_user(&[])
}

pub fn get_send_email_tool_for_user(emails: &[String]) -> openai_api_rs::v1::chat_completion::Tool {
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
    if !emails.is_empty() {
        let desc = format!(
            "The sender email address to use. Connected accounts: {}",
            emails.join(", ")
        );
        properties.insert(
            "from".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some(desc),
                enum_values: Some(emails.to_vec()),
                ..Default::default()
            }),
        );
    }
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
    emails: &[String],
) -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;
    let mut properties = HashMap::new();
    properties.insert(
        "email_id".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The ID of the email to respond to".to_string()),
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
    if !emails.is_empty() {
        let desc = format!(
            "The sender email address to use for the reply. Connected accounts: {}",
            emails.join(", ")
        );
        properties.insert(
            "from".to_string(),
            Box::new(types::JSONSchemaDefine {
                schema_type: Some(types::JSONSchemaType::String),
                description: Some(desc),
                enum_values: Some(emails.to_vec()),
                ..Default::default()
            }),
        );
    }
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
/// - If `from` is specified, look up that specific account.
/// - If only one account exists, use it.
/// - If multiple accounts exist and `from` is not specified, return an error message.
fn resolve_email_account(
    state: &Arc<AppState>,
    user_id: i32,
    from: &Option<String>,
) -> Result<ImapConnectionInfo, String> {
    if let Some(from_email) = from {
        state
            .user_repository
            .get_imap_credentials_by_email(user_id, from_email)
            .map_err(|e| format!("Failed to look up email account: {}", e))?
            .ok_or_else(|| format!("No connected email account found for '{}'", from_email))
    } else {
        let accounts = state
            .user_repository
            .get_all_imap_credentials(user_id)
            .map_err(|e| format!("Failed to fetch email accounts: {}", e))?;
        match accounts.len() {
            0 => Err("No email account connected.".to_string()),
            1 => Ok(accounts.into_iter().next().unwrap()),
            _ => {
                let emails: Vec<String> = accounts.iter().map(|a| a.email.clone()).collect();
                Err(format!(
                    "Multiple email accounts connected ({}). Please specify which one to use.",
                    emails.join(", ")
                ))
            }
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct SendEmailArgs {
    pub to: String,
    pub subject: String,
    pub body: String,
    pub from: Option<String>,
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

    // Resolve which email account to send from
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

    // Format the queued message
    let queued_msg = format!(
        "Will send email to {} with subject '{}' and body '{}' in 60s. Reply 'C' to discard.",
        recipient_email, args.subject, args.body
    );
    // Send the queued confirmation via SMS (skip when from web dashboard)
    if !skip_sms {
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
    tokio::spawn(async move {
        let reason = tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => "timeout",
            _ = cancel_rx => "cancel",
        };
        if reason == "timeout" {
            let email_request = crate::handlers::imap_handlers::SendEmailRequest {
                to: cloned_to,
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
                    // No need to send success message
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
                            .twilio_message_service
                            .send_sms(&error_msg, None, &cloned_user)
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

    // Resolve which email account to send from
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

    // Fetch the email details to get the subject
    let email_details = match crate::handlers::imap_handlers::fetch_single_imap_email(
        State(state.clone()),
        AuthUser {
            user_id,
            is_admin: false,
        },
        axum::extract::Path(args.email_id.clone()),
    )
    .await
    {
        Ok(details) => details,
        Err((_, error_json)) => {
            let error_msg = format!(
                "Failed to fetch email details: {}",
                error_json
                    .0
                    .get("error")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown error")
            );
            if !skip_sms {
                if let Err(e) = state
                    .twilio_message_service
                    .send_sms(&error_msg, None, user)
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
        .0
        .get("email")
        .and_then(|e| e.get("subject"))
        .and_then(|s| s.as_str())
        .unwrap_or("Unknown subject")
        .to_string();
    // Format the queued message using the subject
    let queued_msg = format!(
        "Will respond to email '{}' with '{}' in 60s. Reply 'C' to discard.",
        subject, args.response_text
    );
    // Send the queued confirmation via SMS (skip when from web dashboard)
    if !skip_sms {
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
    let cloned_email_id = args.email_id.clone();
    let cloned_response_text = args.response_text.clone();
    let cloned_from = Some(from_email);
    let cloned_skip_sms = skip_sms;
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
                    // No need to send success message
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
                            .twilio_message_service
                            .send_sms(&error_msg, None, &cloned_user)
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
