use crate::AppState;
use std::sync::Arc;

pub fn get_send_email_tool() -> openai_api_rs::v1::chat_completion::Tool {
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

use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct SendEmailArgs {
    pub to: String,
    pub subject: String,
    pub body: String,
}
pub async fn handle_send_email(
    state: &Arc<AppState>,
    user_id: i32,
    args: &str,
    user: &crate::models::user_models::User,
) -> Result<
    (
        axum::http::StatusCode,
        [(axum::http::HeaderName, &'static str); 1],
        axum::Json<crate::api::twilio_sms::TwilioResponse>,
    ),
    Box<dyn std::error::Error>,
> {
    let args: SendEmailArgs = serde_json::from_str(args)?;

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
                axum::http::StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                axum::Json(crate::api::twilio_sms::TwilioResponse {
                    message: "Failed to send message queue notification".to_string(),
                    created_item_id: None,
                }),
            ));
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
}
pub async fn handle_respond_to_email(
    state: &Arc<AppState>,
    user_id: i32,
    args: &str,
    user: &crate::models::user_models::User,
) -> Result<
    (
        axum::http::StatusCode,
        [(axum::http::HeaderName, &'static str); 1],
        axum::Json<crate::api::twilio_sms::TwilioResponse>,
    ),
    Box<dyn std::error::Error>,
> {
    let args: RespondToEmailArgs = serde_json::from_str(args)?;
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
            if let Err(e) = state
                .twilio_message_service
                .send_sms(&error_msg, None, user)
                .await
            {
                eprintln!("Failed to send error message: {}", e);
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
                axum::http::StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                axum::Json(crate::api::twilio_sms::TwilioResponse {
                    message: "Failed to send message queue notification".to_string(),
                    created_item_id: None,
                }),
            ));
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
    tokio::spawn(async move {
        let reason = tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => "timeout",
            _ = cancel_rx => "cancel",
        };
        if reason == "timeout" {
            let request = crate::handlers::imap_handlers::EmailResponseRequest {
                email_id: cloned_email_id,
                response_text: cloned_response_text,
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
