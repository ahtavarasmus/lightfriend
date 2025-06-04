use std::sync::Arc;
use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
};
use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};
use serde_json::json;
use imap;
use native_tls::TlsConnector;
use base64;
use mail_parser::{self, MimeHeaders};

use crate::{
    AppState,
    handlers::auth_middleware::AuthUser,
};

fn format_timestamp(timestamp: i64, timezone: Option<String>) -> String {
    // Convert timestamp to DateTime<Utc>
    let dt_utc = match DateTime::from_timestamp(timestamp, 0) {
        Some(dt) => dt,
        None => return "Invalid timestamp".to_string(),
    };
    
    // Convert to user's timezone if provided, otherwise use UTC
    let formatted = if let Some(tz_str) = timezone {
        match tz_str.parse::<Tz>() {
            Ok(tz) => dt_utc.with_timezone(&tz).format("%Y-%m-%d %H:%M:%S").to_string(),
            Err(_) => {
                tracing::warn!("Invalid timezone '{}', falling back to UTC", tz_str);
                dt_utc.format("%Y-%m-%d %H:%M:%S UTC").to_string()
            }
        }
    } else {
        tracing::debug!("No timezone provided, using UTC");
        dt_utc.format("%Y-%m-%d %H:%M:%S UTC").to_string()
    };
    
    formatted
}

#[derive(Debug, Serialize, Clone)]
pub struct ImapEmailPreview {
    pub id: String,
    pub subject: Option<String>,
    pub from: Option<String>,
    pub from_email: Option<String>,
    pub date: Option<DateTime<Utc>>,
    pub date_formatted: Option<String>,
    pub snippet: Option<String>,
    pub body: Option<String>,
    pub is_read: bool,
}

#[derive(Debug, Serialize)]
pub struct EmailAttachment {
    pub filename: Option<String>,
    pub content_type: String,
    pub data: String, // base64 encoded
    pub size: usize,
}

#[derive(Debug, Serialize)]
pub struct ImapEmail {
    pub id: String,
    pub subject: Option<String>,
    pub from: Option<String>,
    pub from_email: Option<String>,
    pub date: Option<DateTime<Utc>>,
    pub date_formatted: Option<String>,
    pub snippet: Option<String>,
    pub body: Option<String>,
    pub is_read: bool,
    pub attachments: Vec<EmailAttachment>,
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

    match fetch_emails_imap(&state, auth_user.user_id, true, params.limit, false).await {
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
    let mut limit = params.limit;
    let mut testing = false;
    if let None = limit {
        limit = Some(5);
        testing = true;
    }

    match fetch_emails_imap(&state, auth_user.user_id, false, limit, false).await {
        Ok(previews) => {
            tracing::info!("Fetched {} IMAP full emails", previews.len());
            
            
            let formatted_emails: Vec<_> = previews
                .into_iter()
                .map(|p| {

                    if testing {
                        println!("from_email: {:#?}", p.from_email.clone());
                        println!("from: {:#?}", p.from.clone());
                        println!("from: {:#?}", p.from.clone());
                    }

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

#[derive(Debug, Deserialize)]
pub struct EmailResponseRequest {
    pub email_id: String,
    pub response_text: String,
}

pub async fn respond_to_email(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<EmailResponseRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Responding to email {} for user {}", request.email_id, auth_user.user_id);

    // Validate email_id is a valid number
    if !request.email_id.chars().all(|c| c.is_ascii_digit()) {
        tracing::error!("Invalid email ID format: {}", request.email_id);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Invalid email ID format" }))
        ));
    }

    // Get IMAP credentials
    let (email, password, imap_server, imap_port) = match state
        .user_repository
        .get_imap_credentials(auth_user.user_id)
    {
        Ok(Some(creds)) => creds,
        Ok(None) => return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "No IMAP connection found" }))
        )),
        Err(e) => return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to get IMAP credentials: {}", e) }))
        )),
    };
    tracing::info!("setting up tls");

    // Set up TLS
    let tls = match TlsConnector::builder().build() {
        Ok(tls) => tls,
        Err(e) => return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to create TLS connector: {}", e) }))
        )),
    };

    let server = imap_server.as_deref().unwrap_or("imap.gmail.com");
    let port = imap_port.unwrap_or(993);

    // Connect to IMAP server
    let client = match imap::connect((server, port as u16), server, &tls) {
        Ok(client) => client,
        Err(e) => return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to connect to IMAP server: {}", e) }))
        )),
    };

    // Login
    let mut imap_session = match client.login(&email, &password) {
        Ok(session) => session,
        Err((e, _)) => return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": format!("Failed to login: {}", e) }))
        )),
    };

    tracing::info!("logged in");

    // Select INBOX
    if let Err(e) = imap_session.select("INBOX") {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to select INBOX: {}", e) }))
        ));
    }

    // Fetch the original message to get subject and other details
    let messages = match imap_session.uid_fetch(&request.email_id, "(ENVELOPE)") {
        Ok(messages) => messages,
        Err(e) => return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch original message: {}", e) }))
        )),
    };

    let original_message = match messages.iter().next() {
        Some(msg) => msg,
        None => return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Original message not found" }))
        )),
    };

    let envelope = match original_message.envelope() {
        Some(env) => env,
        None => return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "Failed to get original message envelope" }))
        )),
    };

    tracing::info!("getting the reply address");
    // Get the recipient's email address from the original sender
    let reply_to_address = envelope
        .from
        .as_ref()
        .and_then(|addrs| addrs.first())
        .and_then(|addr| {
            let mailbox = addr.mailbox.as_ref()?.to_vec();
            let host = addr.host.as_ref()?.to_vec();
            Some(format!(
                "{}@{}",
                String::from_utf8_lossy(&mailbox),
                String::from_utf8_lossy(&host)
            ))
        })
        .ok_or_else(|| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "Failed to get recipient address" }))
        ))?;

    // Get original subject
    let original_subject = envelope
        .subject
        .as_ref()
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .unwrap_or_else(|| String::from("No subject"));

    let subject = if !original_subject.to_lowercase().starts_with("re:") {
        format!("Re: {}", original_subject)
    } else {
        original_subject
    };

    // Create SMTP transport
    let smtp_server = imap_server
        .as_deref()
        .unwrap_or("smtp.gmail.com")
        .replace("imap", "smtp");
    
    let smtp_port = 587; // Standard SMTP port

    let creds = lettre::transport::smtp::authentication::Credentials::new(
        email.clone(),
        password.clone(),
    );
    tracing::info!("created the smtp transport");

    let mailer = lettre::SmtpTransport::relay(&smtp_server)
        .unwrap()
        .port(smtp_port)
        .credentials(creds)
        .build();

    use lettre::{Message, Transport};

    // Create email message
    let email_message = match Message::builder()
        .from(email.parse().unwrap())
        .to(reply_to_address.parse().unwrap())
        .subject(subject)
        .body(request.response_text.clone())
    {
        Ok(message) => message,
        Err(e) => return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to create email message: {}", e) }))
        )),
    };

    tracing::info!("sending the mail finally");
    // Send the email
    match mailer.send(&email_message) {
        Ok(_) => {
            // Logout from IMAP
            if let Err(e) = imap_session.logout() {
                tracing::warn!("Failed to logout from IMAP: {}", e);
            }
            tracing::info!("sent the mail successfully and logged out");

            Ok(Json(json!({
                "success": true,
                "message": "Email response sent successfully"
            })))
        }
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to send email: {}", e) }))
        )),
    }
}

pub async fn fetch_single_imap_email(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    axum::extract::Path(email_id): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Fetching single IMAP email {} for user {}", email_id, auth_user.user_id);

    // Validate email_id is a valid number and not empty
    if email_id.trim().is_empty() || !email_id.chars().all(|c| c.is_ascii_digit()) {
        let error_msg = if email_id.trim().is_empty() {
            "Email ID cannot be empty"
        } else {
            "Invalid email ID format"
        };
        tracing::error!("{}: {}", error_msg, email_id);
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ 
                "error": error_msg,
                "email_id": email_id
            }))
        ));
    }

    match fetch_single_email_imap(&state, auth_user.user_id, &email_id).await {
        Ok(email) => {
            tracing::debug!("Successfully fetched email {}", email_id);
            // if admin testing their own account
            if auth_user.user_id == 1 {
                println!("email addr: {:#?}", email.from.clone());
                println!("from_email addr: {:#?}", email.from_email.clone());
            }
            Ok(Json(json!({
                "success": true,
                "email": {
                    "id": email.id,
                    "subject": email.subject.unwrap_or_else(|| "No subject".to_string()),
                    "from": email.from.unwrap_or_else(|| "Unknown sender".to_string()),
                    "from_email": email.from_email.unwrap_or_else(|| "unknown@email.com".to_string()),
                    "date": email.date.map(|dt| dt.to_rfc3339()),
                    "date_formatted": email.date_formatted,
                    "snippet": email.snippet.unwrap_or_else(|| "No preview".to_string()),
                    "body": email.body.unwrap_or_else(|| "No content".to_string()),
                    "is_read": email.is_read,
                    "attachments": email.attachments
                }
            })))
        }
        Err(e) => {
            let (status, message) = match e {
                ImapError::NoConnection => {
                    tracing::error!("No IMAP connection found for user {}", auth_user.user_id);
                    (StatusCode::BAD_REQUEST, "No IMAP connection found".to_string())
                },
                ImapError::CredentialsError(msg) => {
                    tracing::error!("IMAP credentials error for user {}: {}", auth_user.user_id, msg);
                    (StatusCode::UNAUTHORIZED, msg)
                },
                ImapError::ConnectionError(msg) => {
                    tracing::error!("IMAP connection error for user {}: {}", auth_user.user_id, msg);
                    (StatusCode::INTERNAL_SERVER_ERROR, msg)
                },
                ImapError::FetchError(msg) => {
                    tracing::error!("IMAP fetch error for email {} user {}: {}", email_id, auth_user.user_id, msg);
                    (StatusCode::INTERNAL_SERVER_ERROR, msg)
                },
                ImapError::ParseError(msg) => {
                    tracing::error!("IMAP parse error for email {} user {}: {}", email_id, auth_user.user_id, msg);
                    (StatusCode::INTERNAL_SERVER_ERROR, msg)
                },
            };
            Err((status, Json(json!({ 
                "error": message,
                "email_id": email_id
            }))))
        }
    }
}

pub async fn fetch_emails_imap(
    state: &AppState,
    user_id: i32,
    preview_only: bool,
    limit: Option<u32>,
    unprocessed: bool, 
) -> Result<Vec<ImapEmailPreview>, ImapError> {
    tracing::debug!("Starting fetch_emails_imap for user {} with preview_only: {}, limit: {:?}, unprocessed: {}", 
        user_id, preview_only, limit, unprocessed);
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
            "(UID FLAGS ENVELOPE BODY.PEEK[])",  // PEEK to not mark the email as read
        )
        .map_err(|e| ImapError::FetchError(format!("Failed to fetch messages: {}", e)))?;

    let mut email_previews = Vec::new();

    for message in messages.iter() {
        let uid = message.uid.unwrap_or(0).to_string();
        
        // Check if email is already processed using repository method
        let is_processed = state.user_repository.is_email_processed(user_id, &uid)
            .map_err(|e| ImapError::FetchError(format!("Failed to check email processed status: {}", e)))?;
        
        // Skip processed emails if unprocessed is true
        if unprocessed && is_processed {
            continue;
        }

        let envelope = message.envelope().ok_or_else(|| {
            ImapError::ParseError("Failed to get message envelope".to_string())
        })?;

        let (from, from_email) = envelope
            .from
            .as_ref()
            .and_then(|addrs| addrs.first())
            .map(|addr| {
                let name = addr.name
                    .as_ref()
                    .and_then(|n| String::from_utf8(n.to_vec()).ok())
                    .unwrap_or_default();
                let email = addr.mailbox
                    .as_ref()
                    .and_then(|m| {
                        let mailbox = String::from_utf8(m.to_vec()).ok()?;
                        let host = addr.host
                            .as_ref()
                            .and_then(|h| String::from_utf8(h.to_vec()).ok())?;
                        Some(format!("{}@{}", mailbox, host))
                    })
                    .unwrap_or_default();
                (name, email)
            })
            .unwrap_or_default();

        let subject = envelope
            .subject
            .as_ref()
            .and_then(|s| String::from_utf8(s.to_vec()).ok());

        let raw_date = envelope
            .date
            .as_ref()
            .and_then(|d| String::from_utf8(d.to_vec()).ok());
        
        tracing::debug!("Raw date from envelope: {:?}", raw_date);

        let date = raw_date.as_ref().and_then(|date_str| {
            match chrono::DateTime::parse_from_rfc2822(date_str) {
                Ok(dt) => {
                    let utc_dt = dt.with_timezone(&Utc);
                    tracing::debug!("Successfully parsed date '{}' to UTC: {}", date_str, utc_dt);
                    Some(utc_dt)
                }
                Err(e) => {
                    tracing::warn!("Failed to parse date '{}': {}", date_str, e);
                    None
                }
            }
        });

        tracing::debug!("Final processed date: {:?}", date);

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

            let user_timezone = state.user_repository.find_by_id(user_id)
                .ok()
                .and_then(|u| u.unwrap().timezone);
            
            tracing::debug!("User timezone from repository: {:?}", user_timezone);

            let date_formatted = date.map(|dt| {
                let timestamp = dt.timestamp();
                tracing::debug!("Converting timestamp {} with timezone {:?}", timestamp, user_timezone);
                let formatted = format_timestamp(timestamp, user_timezone);
                tracing::debug!("Formatted date result: {}", formatted);
                formatted
            });

            tracing::debug!("Final formatted date: {:?}", date_formatted);

            email_previews.push(ImapEmailPreview {
                id: uid.clone(),
                subject: subject.clone(),
                from: Some(from.clone()),
                from_email: Some(from_email.clone()),
                date,
                date_formatted,
                snippet: Some(snippet),
                body: Some(body),
                is_read,
            });

        // Mark email as processed if unprocessed is true
        if unprocessed {
            match state.user_repository.mark_email_as_processed(user_id, &uid) {
                Ok(_) => {
                    tracing::info!("Marked email {} as processed", uid);
                }
                Err(e) => {
                    tracing::error!("Failed to mark email {} as processed: {}", uid, e);
                    // Continue processing other emails even if marking as processed fails
                }
            }
        }
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

    // Fetch specific message with body structure for attachments
    // Using BODY.PEEK[] to avoid marking the email as read
    let messages = match imap_session.uid_fetch(
        email_id,
        "(UID FLAGS ENVELOPE BODY.PEEK[] BODYSTRUCTURE)",
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
            let name = addr.name
                .as_ref()
                .and_then(|n| String::from_utf8(n.to_vec()).ok())
                .unwrap_or_default();
            let email = format!(
                "{}@{}",
                addr.mailbox.as_ref()
                    .and_then(|m| String::from_utf8(m.to_vec()).ok())
                    .unwrap_or_default(),
                addr.host.as_ref()
                    .and_then(|h| String::from_utf8(h.to_vec()).ok())
                    .unwrap_or_default()
            );
            if name.is_empty() {
                email.clone()
            } else {
                format!("{} <{}>", name, email)
            }
        });

    let from_email = envelope
        .from
        .as_ref()
        .and_then(|addrs| addrs.first())
        .map(|addr| {
            format!(
                "{}@{}",
                addr.mailbox.as_ref()
                    .and_then(|m| String::from_utf8(m.to_vec()).ok())
                    .unwrap_or_default(),
                addr.host.as_ref()
                    .and_then(|h| String::from_utf8(h.to_vec()).ok())
                    .unwrap_or_default()
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

    let (body, snippet, attachments) = body_content.as_ref().map(|content| {
        // Create a parser and parse the content into an Option<Message>
        let parser = MessageParser::default();
        let parsed = parser.parse(content.as_bytes());

        // Get the best available body content, if parsing succeeded
        let clean_content = parsed.as_ref().map(|msg| {
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

        // Extract image attachments (PNG and JPEG only)
        let attachments = parsed.map(|msg| {
            msg.attachments()
                .filter_map(|attachment| {
                    if let Some(content_type) = attachment.content_type() {
                        let ctype = content_type.ctype();
                        if let Some(subtype) = content_type.subtype() {
                            let content_type_str = format!("{}/{}", ctype, subtype).to_lowercase();
                            if content_type_str == "image/png" ||
                               content_type_str == "image/jpeg" ||
                               content_type_str == "image/jpg" {
                                let filename = attachment.attachment_name()
                                    .map(|name| name.to_string());
                                let attachment_data = attachment.contents();
                                let base64_data = base64::encode(attachment_data);

                                tracing::info!("Found image attachment: {:?} ({})", filename, content_type_str);

                                return Some(EmailAttachment {
                                    filename,
                                    content_type: content_type_str,
                                    data: base64_data,
                                    size: attachment_data.len(),
                                });
                            }
                        }
                    }
                    None
                })
                .collect::<Vec<_>>()
        }).unwrap_or_default();

        (clean_content, snippet, attachments)
    }).unwrap_or_else(|| (String::new(), String::new(), Vec::new()));

    // Logout
    imap_session
        .logout()
        .map_err(|e| ImapError::ConnectionError(format!("Failed to logout: {}", e)))?;

    let date_formatted = date.map(|dt| format_timestamp(dt.timestamp(), state.user_repository.find_by_id(user_id)
        .ok()
        .and_then(|u| u.unwrap().timezone)));

    Ok(ImapEmail {
        id: email_id.to_string(),
        subject,
        from,
        from_email,
        date,
        date_formatted,
        snippet: Some(snippet),
        body: Some(body),
        is_read,
        attachments,
    })
}
