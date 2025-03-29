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

pub async fn fetch_imap_previews(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Starting IMAP preview fetch for user {}", auth_user.user_id);

    match fetch_emails_imap(&state, auth_user.user_id, true).await {
        Ok(previews) => {
            tracing::info!("Fetched {} IMAP previews", previews.len());
            
            // Debug print each preview
            for preview in &previews {
                tracing::debug!("Email Preview:\n\
                    ID: {}\n\
                    Subject: {}\n\
                    From: {}\n\
                    Date: {}\n\
                    Is Read: {}\n\
                    Snippet: {}\n",
                    preview.id,
                    preview.subject.as_deref().unwrap_or("No subject"),
                    preview.from.as_deref().unwrap_or("Unknown sender"),
                    preview.date.map_or("No date".to_string(), |d| d.to_rfc3339()),
                    preview.is_read,
                    preview.snippet.as_deref().unwrap_or("No preview")
                );
            }
            
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
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Starting IMAP full emails fetch for user {}", auth_user.user_id);

    match fetch_emails_imap(&state, auth_user.user_id, false).await {
        Ok(previews) => {
            tracing::info!("Fetched {} IMAP full emails", previews.len());
            
            // Debug print each email's full content
            for preview in &previews {
                tracing::info!("Full Email Content:\n\
                    ID: {}\n\
                    Subject: {}\n\
                    From: {}\n\
                    Date: {}\n\
                    Is Read: {}\n\
                    Snippet: {}\n\
                    Body: {}\n\
                    ----------------------------------------",
                    preview.id,
                    preview.subject.as_deref().unwrap_or("No subject"),
                    preview.from.as_deref().unwrap_or("Unknown sender"),
                    preview.date.map_or("No date".to_string(), |d| d.to_rfc3339()),
                    preview.is_read,
                    preview.snippet.as_deref().unwrap_or("No preview"),
                    preview.body.as_deref().unwrap_or("No content")
                );
            }
            
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
            // Debug print the full email
            tracing::debug!("Full Email Content:\n\
                ID: {}\n\
                Subject: {}\n\
                From: {}\n\
                From Email: {}\n\
                Date: {}\n\
                Is Read: {}\n\
                Snippet: {}\n\
                Body: {}\n",
                email.id,
                email.subject.as_deref().unwrap_or("No subject"),
                email.from.as_deref().unwrap_or("Unknown sender"),
                email.from_email.as_deref().unwrap_or("unknown@email.com"),
                email.date.map_or("No date".to_string(), |d| d.to_rfc3339()),
                email.is_read,
                email.snippet.as_deref().unwrap_or("No preview"),
                email.body.as_deref().unwrap_or("No content")
            );

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
) -> Result<Vec<ImapEmailPreview>, ImapError> {
    // Get IMAP credentials
    let (email, password) = state
        .user_repository
        .get_gmail_imap_credentials(user_id)
        .map_err(|e| ImapError::CredentialsError(e.to_string()))?
        .ok_or(ImapError::NoConnection)?;

    // Set up TLS
    let tls = TlsConnector::builder()
        .build()
        .map_err(|e| ImapError::ConnectionError(format!("Failed to create TLS connector: {}", e)))?;

    // Connect to IMAP server
    let client = imap::connect(
        ("imap.gmail.com", 993),
        "imap.gmail.com",
        &tls,
    ).map_err(|e| ImapError::ConnectionError(format!("Failed to connect to IMAP server: {}", e)))?;

    // Login
    let mut imap_session = client
        .login(&email, &password)
        .map_err(|(e, _)| ImapError::CredentialsError(format!("Failed to login: {}", e)))?;

    // Select INBOX
    let mailbox = imap_session
        .select("INBOX")
        .map_err(|e| ImapError::FetchError(format!("Failed to select INBOX: {}", e)))?;

    // Fetch most recent messages (last 20 messages)
    let sequence_set = format!("{}:{}", (mailbox.exists.saturating_sub(19)), mailbox.exists);
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
        
        let body_content = full_body.or(text_body);
        
        let (body, snippet) = body_content.as_ref().map(|content| {
            // Function to decode and clean email content
            fn process_email_content(content: &str) -> (String, String) {
                // First decode quoted-printable content
                let decoded = content.replace("=\r\n", "")
                    .replace("=\n", "");
                
                let decoded = if let Ok(decoded) = quoted_printable::decode(decoded.as_bytes(), quoted_printable::ParseMode::Robust) {
                    String::from_utf8(decoded.clone()).unwrap_or_else(|_| String::from_utf8_lossy(&decoded).into_owned())
                } else {
                    decoded
                };

                // Decode UTF-8 encoded characters
                let decoded = decoded.replace("=3D", "=")
                    .replace("=C3=A4", "ä")
                    .replace("=C3=B6", "ö")
                    .replace("=C3=A5", "å")
                    .replace("=E2=80=91", "-")
                    .replace("=E2=80=99", "'")
                    .replace("=20", " ");

                // Extract and clean HTML content
                let clean_content = if let Some(start_idx) = decoded.find("<!DOCTYPE html>") {
                    if let Some(end_idx) = decoded[start_idx..].find("</html>") {
                        let html_content = &decoded[start_idx..start_idx + end_idx + 7];
                        
                        // Remove style tags and their content
                        let re_style = regex::Regex::new(r"<style\b[^>]*>[\s\S]*?</style>").unwrap();
                        let content = re_style.replace_all(html_content, "");
                        
                        // Remove script tags and their content
                        let re_script = regex::Regex::new(r"<script\b[^>]*>[\s\S]*?</script>").unwrap();
                        let content = re_script.replace_all(&content, "");
                        
                        // Remove HTML tags but preserve line breaks
                        let content = content.replace("<br>", "\n")
                            .replace("<br/>", "\n")
                            .replace("<br />", "\n")
                            .replace("<p>", "\n")
                            .replace("</p>", "\n");
                        
                        // Remove remaining HTML tags
                        let re_tags = regex::Regex::new(r"<[^>]+>").unwrap();
                        let content = re_tags.replace_all(&content, "");
                        
                        // Decode HTML entities
                        let content = content
                            .replace("&nbsp;", " ")
                            .replace("&amp;", "&")
                            .replace("&lt;", "<")
                            .replace("&gt;", ">")
                            .replace("&quot;", "\"")
                            .replace("&#39;", "'");
                        
                // Clean up whitespace and remove zero-width characters
                content.lines()
                    .map(|line| {
                        line.trim()
                            .chars()
                            .filter(|&c| {
                                // Filter out zero-width and other invisible characters
                                !matches!(c, '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{FEFF}' | 
                                               '\u{2060}' | '\u{2061}' | '\u{2062}' | '\u{2063}' |
                                               '\u{2064}' | '\u{2065}' | '\u{2066}' | '\u{2067}' |
                                               '\u{2068}' | '\u{2069}')
                            })
                            .collect::<String>()
                    })
                    .filter(|line| !line.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n")
                    } else {
                        decoded
                    }
                } else {
                    decoded
                };


                // Create snippet
                let snippet = clean_content
                    .lines()
                    .filter(|line| !line.trim().is_empty())
                    .take(3)
                    .collect::<Vec<_>>()
                    .join(" ");

                let snippet = if snippet.chars().count() > 150 {
                    let truncated: String = snippet.chars().take(150).collect();
                    format!("{}...", truncated)
                } else {
                    snippet
                };

                (clean_content, snippet)
            }

            process_email_content(content)
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
    let (email, password) = state
        .user_repository
        .get_gmail_imap_credentials(user_id)
        .map_err(|e| ImapError::CredentialsError(e.to_string()))?
        .ok_or(ImapError::NoConnection)?;

    // Set up TLS
    let tls = TlsConnector::builder()
        .build()
        .map_err(|e| ImapError::ConnectionError(format!("Failed to create TLS connector: {}", e)))?;

    // Connect to IMAP server
    let client = imap::connect(
        ("imap.gmail.com", 993),
        "imap.gmail.com",
        &tls,
    ).map_err(|e| ImapError::ConnectionError(format!("Failed to connect to IMAP server: {}", e)))?;

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
    
    let body_content = full_body.or(text_body);
    
    let (body, snippet) = body_content.as_ref().map(|content| {
        // Reuse the process_email_content function from the preview fetch
        fn process_email_content(content: &str) -> (String, String) {
            // First decode quoted-printable content
            let decoded = content.replace("=\r\n", "")
                .replace("=\n", "");
            
            let decoded = if let Ok(decoded) = quoted_printable::decode(decoded.as_bytes(), quoted_printable::ParseMode::Robust) {
                String::from_utf8(decoded.clone()).unwrap_or_else(|_| String::from_utf8_lossy(&decoded).into_owned())
            } else {
                decoded
            };

            // Decode UTF-8 encoded characters
            let decoded = decoded.replace("=3D", "=")
                .replace("=C3=A4", "ä")
                .replace("=C3=B6", "ö")
                .replace("=C3=A5", "å")
                .replace("=E2=80=91", "-")
                .replace("=E2=80=99", "'")
                .replace("=20", " ");

            // Extract and clean HTML content
            let clean_content = if let Some(start_idx) = decoded.find("<!DOCTYPE html>") {
                if let Some(end_idx) = decoded[start_idx..].find("</html>") {
                    let html_content = &decoded[start_idx..start_idx + end_idx + 7];
                    
                    // Remove style tags and their content
                    let re_style = regex::Regex::new(r"<style\b[^>]*>[\s\S]*?</style>").unwrap();
                    let content = re_style.replace_all(html_content, "");
                    
                    // Remove script tags and their content
                    let re_script = regex::Regex::new(r"<script\b[^>]*>[\s\S]*?</script>").unwrap();
                    let content = re_script.replace_all(&content, "");
                    
                    // Remove HTML tags but preserve line breaks
                    let content = content.replace("<br>", "\n")
                        .replace("<br/>", "\n")
                        .replace("<br />", "\n")
                        .replace("<p>", "\n")
                        .replace("</p>", "\n");
                    
                    // Remove remaining HTML tags
                    let re_tags = regex::Regex::new(r"<[^>]+>").unwrap();
                    let content = re_tags.replace_all(&content, "");
                    
                    // Decode HTML entities
                    let content = content
                        .replace("&nbsp;", " ")
                        .replace("&amp;", "&")
                        .replace("&lt;", "<")
                        .replace("&gt;", ">")
                        .replace("&quot;", "\"")
                        .replace("&#39;", "'");
                    
                    // Clean up whitespace
                    content.lines()
                        .map(|line| line.trim())
                        .filter(|line| !line.is_empty())
                        .collect::<Vec<_>>()
                        .join("\n")
                } else {
                    decoded
                }
            } else {
                decoded
            };

            // Create snippet
            let snippet = clean_content
                .lines()
                .filter(|line| !line.trim().is_empty())
                .take(3)
                .collect::<Vec<_>>()
                .join(" ");

            let snippet = if snippet.chars().count() > 150 {
                let truncated: String = snippet.chars().take(150).collect();
                format!("{}...", truncated)
            } else {
                snippet
            };

            (clean_content, snippet)
        }

        process_email_content(content)
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

