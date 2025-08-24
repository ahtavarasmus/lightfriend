use crate::handlers::imap_handlers::{self, ImapError};
use crate::AppState;
use std::sync::Arc;

pub fn get_fetch_emails_tool() -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;

    let mut email_properties = HashMap::new();
    email_properties.insert(
        "param".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Can be anything, will fetch last 5 emails regardless".to_string()),
            ..Default::default()
        }),
    );

    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("fetch_emails"),
            description: Some(String::from("Fetches the last 5 emails using IMAP. Use this when user asks about their recent emails or wants to check their inbox.")),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(email_properties),
                required: None,
            },
        },
    }
}

pub fn get_fetch_specific_email_tool() -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;

    let mut specific_email_properties = HashMap::new();
    specific_email_properties.insert(
        "query".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The search query to find a specific email".to_string()),
            ..Default::default()
        }),
    );

    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("fetch_specific_email"),
            description: Some(String::from("Search and fetch a specific email based on a query. Use this when user asks about a specific email or wants to find an email about a particular topic. You must ALWAYS respond with the whole message body or summary of the body if too long. Never reply with just the subject line!")),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(specific_email_properties),
                required: Some(vec![String::from("query")]),
            },
        },
    }
}

pub fn get_send_email_tool() -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;
    let mut properties = HashMap::new();
    properties.insert(
        "to".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The recipient's email address".to_string()),
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
            description: Some(String::from("Sends an email to the specified recipient using the user's email account. Use this when the user asks to send an new email to someone.")),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(vec![String::from("to"), String::from("subject"), String::from("body")]),
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
) -> Result<(axum::http::StatusCode, [(axum::http::HeaderName, &'static str); 1], axum::Json<crate::api::twilio_sms::TwilioResponse>), Box<dyn std::error::Error>> {
    let args: SendEmailArgs = serde_json::from_str(args)?;
    // Format the queued message
    let queued_msg = format!(
        "Will send email to {} with subject '{}' and body '{}' in 60s. Reply 'cancel' to discard.",
        args.to, args.subject, args.body
    );
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
                axum::http::StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                axum::Json(crate::api::twilio_sms::TwilioResponse {
                    message: "Failed to send message queue notification".to_string(),
                })
            ));
        }
    }
    // Create cancellation channel
    let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
    // Spawn the delayed send task
    let cloned_state = state.clone();
    let cloned_user_id = user_id;
    let cloned_user = user.clone();
    let cloned_to = args.to.clone();
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
                crate::handlers::auth_middleware::AuthUser { user_id: cloned_user_id, is_admin: false },
                axum::Json(email_request)
            ).await {
                Ok(_) => {
                    // No need to send success message 
                }
                Err((status, error_json)) => {
                    let error_msg = format!("Failed to send email: {}", error_json.0.get("error").and_then(|v| v.as_str()).unwrap_or("Unknown error"));
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
        })
    ))
}

pub async fn handle_fetch_emails(state: &Arc<AppState>, user_id: i32) -> String {
    let auth_user = crate::handlers::auth_middleware::AuthUser {
        user_id,
        is_admin: false,
    };

    let query_obj = crate::handlers::imap_handlers::FetchEmailsQuery { limit: None };

    match crate::handlers::imap_handlers::fetch_full_imap_emails(
        axum::extract::State(state.clone()),
        auth_user,
        axum::extract::Query(query_obj),
    ).await {
        Ok(axum::Json(response)) => {
            if let Some(emails) = response.get("emails") {
                if let Some(emails_array) = emails.as_array() {
                    let mut parts: Vec<String> = Vec::new();
                    for (i, email) in emails_array.iter().rev().take(5).enumerate() {
                        let id = email.get("id").and_then(|i| i.as_str()).unwrap_or("Unknown ID");
                        let subject = email.get("subject").and_then(|s| s.as_str()).unwrap_or("No subject");
                        let from = email.get("from").and_then(|f| f.as_str()).unwrap_or("Unknown sender");
                        let date_formatted = email.get("date_formatted")
                            .and_then(|d| d.as_str())
                            .unwrap_or("Unknown date");
                        let snippet = email.get("snippet").and_then(|s| s.as_str()).unwrap_or("No snippet");
                        parts.push(format!("Email ID: {}:\nSubject: {}\nFrom: {}\nDate: {}\nSnippet: {}", id, subject, from, date_formatted, snippet));
                    }
                    let mut response = parts.join("\n\n");

                    
                    if emails_array.is_empty() {
                        response = "No recent emails found.".to_string();
                    }
                    
                    response
                } else {
                    "Failed to parse emails.".to_string()
                }
            } else {
                "No emails found.".to_string()
            }
        }
        Err((status, axum::Json(error))) => {
            let error_message = match status {
                axum::http::StatusCode::BAD_REQUEST => "No IMAP connection found. Please check your email settings.",
                axum::http::StatusCode::UNAUTHORIZED => "Your email credentials need to be updated.",
                _ => "Failed to fetch emails. Please try again later.",
            };
            error_message.to_string()
        }
    }
}


use axum::{
    extract::{State, Json},
};
use crate::{
    handlers::auth_middleware::AuthUser,
};

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
) -> Result<(axum::http::StatusCode, [(axum::http::HeaderName, &'static str); 1], axum::Json<crate::api::twilio_sms::TwilioResponse>), Box<dyn std::error::Error>> {
    let args: RespondToEmailArgs = serde_json::from_str(args)?;
    // Fetch the email details to get the subject
    let email_details = match crate::imap_handlers::fetch_single_imap_email(
        State(state.clone()),
        AuthUser { user_id, is_admin: false },
        axum::extract::Path(args.email_id.clone()),
    ).await {
        Ok(details) => details,
        Err((status, error_json)) => {
            let error_msg = format!("Failed to fetch email details: {}", error_json.0.get("error").and_then(|v| v.as_str()).unwrap_or("Unknown error"));
            if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                state,
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
    };
    let subject = email_details.0.get("email")
        .and_then(|e| e.get("subject"))
        .and_then(|s| s.as_str())
        .unwrap_or("Unknown subject")
        .to_string();
    // Format the queued message using the subject
    let queued_msg = format!(
        "Will respond to email '{}' with '{}' in 60s. Reply 'cancel' to discard.",
        subject, args.response_text
    );
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
                axum::http::StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                axum::Json(crate::api::twilio_sms::TwilioResponse {
                    message: "Failed to send message queue notification".to_string(),
                })
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
            let request = crate::imap_handlers::EmailResponseRequest {
                email_id: cloned_email_id,
                response_text: cloned_response_text,
            };
            match crate::imap_handlers::respond_to_email(
                State(cloned_state.clone()),
                AuthUser { user_id: cloned_user_id, is_admin: false },
                Json(request)
            ).await {
                Ok(_) => {
                    // No need to send success message
                }
                Err((status, error_json)) => {
                    let error_msg = format!("Failed to respond to email: {}", error_json.0.get("error").and_then(|v| v.as_str()).unwrap_or("Unknown error"));
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
        })
    ))
}

pub async fn handle_fetch_specific_email(state: &Arc<AppState>, user_id: i32, query: &str) -> String {
    // Create OpenAI client for email selection
    let client = match crate::tool_call_utils::utils::create_openai_client(&state) {
        Ok(client) => client,
        Err(e) => {
            eprintln!("Failed to create OpenAI client: {}", e);
            return "Failed to process email search".to_string();
        }
    };

    let state_clone = state.clone();
    let user_id_clone = user_id.clone();

    // Fetch the latest 20 emails with full content
    match crate::handlers::imap_handlers::fetch_emails_imap(&state_clone, user_id_clone, true, Some(20), false, false).await {
        Ok(emails) => {
            if emails.is_empty() {
                return "No emails found".to_string();
            }

            // Format emails for LLM analysis
            let mut formatted_emails = String::new();
            for email in emails.iter() {
                let formatted_email = format!(
                    "email_id {}:\nFrom: {}\nSubject: {}\nDate: {}\n\n{}\n\n",
                    email.id,
                    email.from.as_deref().unwrap_or("Unknown"),
                    email.subject.as_deref().unwrap_or("No subject"),
                    email.date_formatted.as_deref().unwrap_or("No date"),
                    email.body.as_deref().unwrap_or("No content"),
                );
                formatted_emails.push_str(&formatted_email);
            }

            // Use LLM to select the most relevant email
            match crate::tool_call_utils::utils::select_most_relevant_email(&client, 
                openai_api_rs::v1::common::GPT4_O.to_string(),
                query, &formatted_emails).await {
                Ok((selected_email_id, _)) => selected_email_id,
                Err(e) => {
                    eprintln!("Failed to select relevant email: {}", e);
                    "Failed to process email search".to_string()
                }
            }
        }
        Err(e) => {
            let error_message = match e {
                ImapError::NoConnection => "No IMAP connection found",
                ImapError::CredentialsError(_) => "Invalid credentials",
                ImapError::ConnectionError(msg) | ImapError::FetchError(msg) | ImapError::ParseError(msg) => {
                    eprintln!("Failed to fetch emails: {}", msg);
                    "Failed to fetch emails"
                }
            };
            error_message.to_string()
        }
    }
}

