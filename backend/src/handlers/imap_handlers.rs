use std::sync::Arc;
use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use imap;
use native_tls::TlsConnector;
use quoted_printable;
use base64;
use mail_parser;

use crate::{
    AppState,
    handlers::auth_middleware::AuthUser,
};

#[derive(Debug, Serialize)]
pub struct ImapEmailPreview {
    pub id: String,
    pub subject: Option<String>,
    pub from: Option<String>,
    pub date: Option<DateTime<Utc>>,
    pub snippet: Option<String>,
    pub body: Option<String>,
    pub is_read: bool,
}

#[derive(Debug, Serialize)]
pub struct ImapEmail {
    pub id: String,
    pub subject: Option<String>,
    pub from: Option<String>,
    pub from_email: Option<String>,
    pub date: Option<DateTime<Utc>>,
    pub snippet: Option<String>,
    pub body: Option<String>,
    pub is_read: bool,
}

#[derive(Debug)]
pub enum ImapError {
    NoConnection,
    CredentialsError(String),
    ConnectionError(String),
    FetchError(String),
    ParseError(String),
}

#[derive(Debug, Deserialize)]
pub struct FetchEmailsQuery {
    pub limit: Option<u32>,
}

pub async fn fetch_imap_previews(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    axum::extract::Query(params): axum::extract::Query<FetchEmailsQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Starting IMAP preview fetch for user {} with limit {:?}", auth_user.user_id, params.limit);

    match fetch_emails_imap(&state, auth_user.user_id, true, params.limit).await {
        Ok(previews) => {
            tracing::info!("Fetched {} IMAP previews", previews.len());
            
            let formatted_previews: Vec<_> = previews
                .into_iter()
                .map(|p| {
                    json!({
                        "id": p.id,
                        "subject": p.subject.unwrap_or_else(|| "No subject".to_string()),
                        "from": p.from.unwrap_or_else(|| "Unknown sender".to_string()),
                        "date": p.date.map(|dt| dt.to_rfc3339()),
                        "snippet": p.snippet.unwrap_or_else(|| "No preview".to_string()),
                        "is_read": p.is_read
                    })
                })
                .collect();

            Ok(Json(json!({ "success": true, "previews": formatted_previews })))
        }
        Err(e) => {
            let (status, message) = match e {
                ImapError::NoConnection => (StatusCode::BAD_REQUEST, "No IMAP connection found".to_string()),
                ImapError::CredentialsError(msg) => (StatusCode::UNAUTHORIZED, msg),
                ImapError::ConnectionError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
                ImapError::FetchError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
                ImapError::ParseError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            };
            tracing::error!("IMAP preview fetch failed: {}", message);
            Err((status, Json(json!({ "error": message }))))
        }
    }
}

pub async fn fetch_full_imap_emails(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    axum::extract::Query(params): axum::extract::Query<FetchEmailsQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Starting IMAP full emails fetch for user {} with limit {:?}", auth_user.user_id, params.limit);

    match fetch_emails_imap(&state, auth_user.user_id, false, params.limit).await {
        Ok(previews) => {
            tracing::info!("Fetched {} IMAP full emails", previews.len());
            
            
            let formatted_emails: Vec<_> = previews
                .into_iter()
                .map(|p| {

                    json!({
                        "id": p.id,
                        "subject": p.subject.unwrap_or_else(|| "No subject".to_string()),
                        "from": p.from.unwrap_or_else(|| "Unknown sender".to_string()),
                        "date": p.date.map(|dt| dt.to_rfc3339()),
                        "snippet": p.snippet.unwrap_or_else(|| "No preview".to_string()),
                        "body": p.body.unwrap_or_else(|| "No content".to_string()),
                        "is_read": p.is_read
                    })
                })
                .collect();

            Ok(Json(json!({ "success": true, "emails": formatted_emails })))
        }
        Err(e) => {
            let (status, message) = match e {
                ImapError::NoConnection => (StatusCode::BAD_REQUEST, "No IMAP connection found".to_string()),
                ImapError::CredentialsError(msg) => (StatusCode::UNAUTHORIZED, msg),
                ImapError::ConnectionError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
                ImapError::FetchError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
                ImapError::ParseError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            };
            tracing::error!("IMAP full emails fetch failed: {}", message);
            Err((status, Json(json!({ "error": message }))))
        }
    }
}

pub async fn fetch_single_imap_email(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    email_id: String,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Fetching single IMAP email {} for user {}", email_id, auth_user.user_id);

    // Validate email_id is a valid number
    if !email_id.chars().all(|c| c.is_ascii_digit()) {
        tracing::error!("Invalid email ID format: {}", email_id);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Invalid email ID format" }))
        ));
    }

    match fetch_single_email_imap(&state, auth_user.user_id, &email_id).await {
        Ok(email) => {

            Ok(Json(json!({
                "success": true,
                "email": {
                    "id": email.id,
                    "subject": email.subject.unwrap_or_else(|| "No subject".to_string()),
                    "from": email.from.unwrap_or_else(|| "Unknown sender".to_string()),
                    "from_email": email.from_email.unwrap_or_else(|| "unknown@email.com".to_string()),
                    "date": email.date.map(|dt| dt.to_rfc3339()),
                    "snippet": email.snippet.unwrap_or_else(|| "No preview".to_string()),
                    "body": email.body.unwrap_or_else(|| "No content".to_string()),
                    "is_read": email.is_read
                }
            })))
        }
        Err(e) => {
            let (status, message) = match e {
                ImapError::NoConnection => (StatusCode::BAD_REQUEST, "No IMAP connection found".to_string()),
                ImapError::CredentialsError(msg) => (StatusCode::UNAUTHORIZED, msg),
                ImapError::ConnectionError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
                ImapError::FetchError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
                ImapError::ParseError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            };
            tracing::error!("IMAP email fetch failed: {}", message);
            Err((status, Json(json!({ "error": message }))))
        }
    }
}

pub async fn fetch_emails_imap(
    state: &AppState,
    user_id: i32,
    preview_only: bool,
    limit: Option<u32>,
) -> Result<Vec<ImapEmailPreview>, ImapError> {
    // Get IMAP credentials
    let (email, password, imap_server, imap_port) = state
        .user_repository
        .get_imap_credentials(user_id)
        .map_err(|e| ImapError::CredentialsError(e.to_string()))?
        .ok_or_else(|| ImapError::NoConnection)?;

    // Add logging for debugging (remove in production)
    tracing::debug!("Fetching IMAP emails for user {} with email {}", user_id, email);

    // Set up TLS
    let tls = TlsConnector::builder()
        .build()
        .map_err(|e| ImapError::ConnectionError(format!("Failed to create TLS connector: {}", e)))?;

    let server = imap_server.as_deref().unwrap_or("imap.gmail.com");
    let port = imap_port.unwrap_or(993);
    // Connect to IMAP server
    let client = imap::connect((server, port as u16), server, &tls)
    .map_err(|e| ImapError::ConnectionError(format!("Failed to connect to IMAP server: {}", e)))?;

    // Login
    let mut imap_session = client
        .login(&email, &password)
        .map_err(|(e, _)| ImapError::CredentialsError(format!("Failed to login: {}", e)))?;

    // Select INBOX
    let mailbox = imap_session
        .select("INBOX")
        .map_err(|e| ImapError::FetchError(format!("Failed to select INBOX: {}", e)))?;

    // Calculate how many messages to fetch based on limit parameter
    let limit = limit.unwrap_or(20);
    let sequence_set = format!("{}:{}", (mailbox.exists.saturating_sub(limit - 1)), mailbox.exists);
    let messages = imap_session
        .fetch(
            &sequence_set,
            "(UID FLAGS ENVELOPE BODY[] BODY[TEXT])",  // Added BODY[] to get full message content
        )
        .map_err(|e| ImapError::FetchError(format!("Failed to fetch messages: {}", e)))?;

    let mut email_previews = Vec::new();

    for message in messages.iter() {
        let uid = message.uid.unwrap_or(0).to_string();
        let envelope = message.envelope().ok_or_else(|| {
            ImapError::ParseError("Failed to get message envelope".to_string())
        })?;

        let from = envelope
            .from
            .as_ref()
            .and_then(|addrs| addrs.first())
            .map(|addr| {
                format!(
                    "{} <{}>",
                    addr.name.as_ref().and_then(|n| String::from_utf8(n.to_vec()).ok()).unwrap_or_default(),
                    addr.mailbox.as_ref().and_then(|m| String::from_utf8(m.to_vec()).ok()).unwrap_or_default()
                )
            });

        let subject = envelope
            .subject
            .as_ref()
            .and_then(|s| String::from_utf8(s.to_vec()).ok());

        let date = envelope
            .date
            .as_ref()
            .and_then(|d| String::from_utf8(d.to_vec()).ok())
            .and_then(|date_str| {
                chrono::DateTime::parse_from_rfc2822(&date_str)
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc))
            });

        let is_read = message
            .flags()
            .iter()
            .any(|flag| flag.to_string() == "\\Seen");

            // Try to get both full body and text body
        let full_body = message.body().map(|b| String::from_utf8_lossy(b).into_owned());
        let text_body = message.text().map(|b| String::from_utf8_lossy(b).into_owned());
        

        use mail_parser::MessageParser;

        let body_content = full_body.or(text_body);

        let (body, snippet) = body_content.as_ref().map(|content| {
            // Create a parser and parse the content into an Option<Message>
            let parser = MessageParser::default();
            let parsed = parser.parse(content.as_bytes());

            // Get the best available body content, if parsing succeeded
            let clean_content = parsed.map(|msg| {
                let body_text = msg.body_text(0).or_else(|| msg.body_html(0));
                body_text
                    .map(|text| {
                        text.lines()
                            .map(str::trim)
                            .filter(|line| !line.is_empty())
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .unwrap_or_else(|| String::from("[No readable body found]"))
            }).unwrap_or_else(|| String::from("[Failed to parse email body]"));

            // Generate a snippet from the clean body
            let snippet = clean_content.chars().take(200).collect::<String>();

            (clean_content, snippet)
        }).unwrap_or_else(|| (String::new(), String::new()));

            email_previews.push(ImapEmailPreview {
                id: uid,
                subject,
                from,
                date,
                snippet: Some(snippet),
                body: Some(body),
                is_read,
            });
    }

    // Logout
    imap_session
        .logout()
        .map_err(|e| ImapError::ConnectionError(format!("Failed to logout: {}", e)))?;

    // Reverse the order so newest emails appear first
    //email_previews.reverse();

    Ok(email_previews)
}

pub async fn fetch_single_email_imap(
    state: &AppState,
    user_id: i32,
    email_id: &str,
) -> Result<ImapEmail, ImapError> {
    // Get IMAP credentials
    let (email, password, imap_server, imap_port) = state
        .user_repository
        .get_imap_credentials(user_id)
        .map_err(|e| ImapError::CredentialsError(e.to_string()))?
        .ok_or(ImapError::NoConnection)?;

    // Set up TLS
    let tls = TlsConnector::builder()
        .build()
        .map_err(|e| ImapError::ConnectionError(format!("Failed to create TLS connector: {}", e)))?;

    let server = imap_server.as_deref().unwrap_or("imap.gmail.com");
    let port = imap_port.unwrap_or(993);
    // Connect to IMAP server
    let client = imap::connect((server, port as u16), server, &tls)
    .map_err(|e| ImapError::ConnectionError(format!("Failed to connect to IMAP server: {}", e)))?;

    // Login
    let mut imap_session = client
        .login(&email, &password)
        .map_err(|(e, _)| ImapError::CredentialsError(format!("Failed to login: {}", e)))?;

    // Select INBOX
    imap_session
        .select("INBOX")
        .map_err(|e| ImapError::FetchError(format!("Failed to select INBOX: {}", e)))?;

    // Fetch specific message
    let messages = match imap_session.uid_fetch(
        email_id,
        "(UID FLAGS ENVELOPE BODY[] BODY.PEEK[TEXT])",
    ) {
        Ok(messages) => messages,
        Err(e) => {
            tracing::error!("Failed to fetch message with UID {}: {}", email_id, e);
            return Err(ImapError::FetchError(format!("Failed to fetch message: {}", e)));
        }
    };

    let message = match messages.iter().next() {
        Some(msg) => msg,
        None => {
            tracing::error!("No message found with UID {}", email_id);
            return Err(ImapError::FetchError(format!("Message with UID {} not found", email_id)));
        }
    };

    // Verify the UID matches
    let msg_uid = message.uid.ok_or_else(|| {
        tracing::error!("Message found but has no UID");
        ImapError::ParseError("Message has no UID".to_string())
    })?;

    if msg_uid.to_string() != email_id {
        tracing::error!("UID mismatch: expected {}, got {}", email_id, msg_uid);
        return Err(ImapError::FetchError(format!("Message UID mismatch: expected {}, got {}", email_id, msg_uid)));
    }

    let envelope = message
        .envelope()
        .ok_or_else(|| ImapError::ParseError("Failed to get message envelope".to_string()))?;

    let from = envelope
        .from
        .as_ref()
        .and_then(|addrs| addrs.first())
        .map(|addr| {
            format!(
                "{} <{}>",
                addr.name.as_ref().and_then(|n| String::from_utf8(n.to_vec()).ok()).unwrap_or_default(),
                addr.mailbox.as_ref().and_then(|m| String::from_utf8(m.to_vec()).ok()).unwrap_or_default()
            )
        });

    let from_email = envelope
        .from
        .as_ref()
        .and_then(|addrs| addrs.first())
        .and_then(|addr| {
            addr.mailbox
                .as_ref()
                .and_then(|m| String::from_utf8(m.to_vec()).ok())
        });

    let subject = envelope
        .subject
        .as_ref()
        .and_then(|s| String::from_utf8(s.to_vec()).ok());

    let date = envelope
        .date
        .as_ref()
        .and_then(|d| String::from_utf8(d.to_vec()).ok())
        .and_then(|date_str| {
            chrono::DateTime::parse_from_rfc2822(&date_str)
                .ok()
                .map(|dt| dt.with_timezone(&Utc))
        });

    let is_read = message
        .flags()
        .iter()
        .any(|flag| flag.to_string() == "\\Seen");

    // Try to get both full body and text body
    let full_body = message.body().map(|b| String::from_utf8_lossy(b).into_owned());
    let text_body = message.text().map(|b| String::from_utf8_lossy(b).into_owned());
    
    use mail_parser::MessageParser;

    let body_content = full_body.or(text_body);

    let (body, snippet) = body_content.as_ref().map(|content| {
        // Create a parser and parse the content into an Option<Message>
        let parser = MessageParser::default();
        let parsed = parser.parse(content.as_bytes());

        // Get the best available body content, if parsing succeeded
        let clean_content = parsed.map(|msg| {
            let body_text = msg.body_text(0).or_else(|| msg.body_html(0));
            body_text
                .map(|text| {
                    text.lines()
                        .map(str::trim)
                        .filter(|line| !line.is_empty())
                        .collect::<Vec<_>>()
                        .join("\n")
                })
                .unwrap_or_else(|| String::from("[No readable body found]"))
        }).unwrap_or_else(|| String::from("[Failed to parse email body]"));

        // Generate a snippet from the clean body
        let snippet = clean_content.chars().take(200).collect::<String>();

        (clean_content, snippet)
    }).unwrap_or_else(|| (String::new(), String::new()));


        // Logout
    imap_session
        .logout()
        .map_err(|e| ImapError::ConnectionError(format!("Failed to logout: {}", e)))?;

    Ok(ImapEmail {
        id: email_id.to_string(),
        subject,
        from,
        from_email,
        date,
        snippet: Some(snippet),
        body: Some(body),
        is_read,
    })
}

