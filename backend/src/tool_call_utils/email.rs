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
                    let mut response = String::new();
                    for (i, email) in emails_array.iter().rev().take(5).enumerate() {
                        let subject = email.get("subject").and_then(|s| s.as_str()).unwrap_or("No subject");
                        let from = email.get("from").and_then(|f| f.as_str()).unwrap_or("Unknown sender");
                        let date_formatted = email.get("date_formatted")
                            .and_then(|d| d.as_str())
                            .unwrap_or("Unknown date");
                        
                        if i == 0 {
                            response.push_str(&format!("{}. {} from {} ({}):\n", i + 1, subject, from, date_formatted));
                        } else {
                            response.push_str(&format!("\n\n{}. {} from {} ({}):\n", i + 1, subject, from, date_formatted));
                        }
                    }
                    
                    if emails_array.len() > 5 {
                        response.push_str(&format!("\n\n(+ {} more emails)", emails_array.len() - 5));
                    }
                    
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

pub async fn handle_fetch_specific_email(state: &Arc<AppState>, user_id: i32, query: &str) -> String {
    // Fetch the latest 20 emails with full content
    match crate::handlers::imap_handlers::fetch_emails_imap(&state, user_id, true, Some(20), false).await {
        Ok(emails) => {
            if emails.is_empty() {
                return "No emails found.".to_string();
            }

            // Format all emails into a searchable response
            let mut response = format!("Search query: '{}'\n\nLatest emails (newest first):\n\n", query);
            for (i, email) in emails.iter().enumerate() {
                let formatted_email = format!(
                    "Email {}:\nFrom: {}\nSubject: {}\nDate: {}\n\n{}\n",
                    i + 1,
                    email.from.as_deref().unwrap_or("Unknown"),
                    email.subject.as_deref().unwrap_or("No subject"),
                    email.date_formatted.as_deref().unwrap_or("No date"),
                    email.body.as_deref().unwrap_or("No content"),
                );
                response.push_str(&formatted_email);
            }

            response
        }
        Err(e) => {
            let error_message = match e {
                ImapError::NoConnection => "No IMAP connection found. Please check your email settings.",
                ImapError::CredentialsError(_) => "Your email credentials need to be updated.",
                ImapError::ConnectionError(msg) | ImapError::FetchError(msg) | ImapError::ParseError(msg) => {
                    eprintln!("Failed to fetch emails: {}", msg);
                    "Failed to fetch emails. Please try again later."
                }
            };
            error_message.to_string()
        }
    }
}

