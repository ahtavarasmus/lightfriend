use crate::UserCoreOps;
use crate::{handlers::auth_middleware::AuthUser, AppState};
use async_imap::Session;
use async_native_tls::TlsStream;
use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::Json as AxumJson,
};
use chrono::{DateTime, Utc};
use chrono_tz::Tz;
use futures_util::TryStreamExt;
use lettre::transport::smtp::authentication::Credentials;
use lettre::{Message, Transport};
use mail_parser;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tokio::net::TcpStream;

/// Type alias for the async IMAP session we use throughout this module.
pub(crate) type ImapSession = Session<TlsStream<TcpStream>>;

/// Open an authenticated IMAP session over TLS.
///
/// This is the canonical async path used by all IMAP entry points in this
/// crate. Layered timeouts:
///   * `tokio::time::timeout` (15s) on the TCP connect
///   * `tokio::time::timeout` (15s) on the TLS handshake
///   * `tokio::time::timeout` (15s) on the LOGIN command
///
/// Callers should add their own outer wall-clock `tokio::time::timeout` around
/// the full fetch flow as a last-resort guard.
pub(crate) async fn open_imap_session(
    server: &str,
    port: u16,
    email: &str,
    password: &str,
) -> Result<ImapSession, ImapError> {
    use std::time::Duration;

    // 1. TCP connect (uses tokio's async resolver internally)
    let tcp = tokio::time::timeout(Duration::from_secs(15), TcpStream::connect((server, port)))
        .await
        .map_err(|_| {
            ImapError::ConnectionError(format!(
                "TCP connect timed out (15s) for {}:{}",
                server, port
            ))
        })?
        .map_err(|e| {
            ImapError::ConnectionError(format!(
                "Failed to TCP connect to {}:{}: {}",
                server, port, e
            ))
        })?;

    // 2. TLS handshake
    let tls_stream = tokio::time::timeout(
        Duration::from_secs(15),
        async_native_tls::TlsConnector::new().connect(server, tcp),
    )
    .await
    .map_err(|_| ImapError::ConnectionError("TLS handshake timed out (15s)".to_string()))?
    .map_err(|e| ImapError::ConnectionError(format!("TLS handshake failed: {}", e)))?;

    // 3. Build IMAP client and read greeting
    let mut client = async_imap::Client::new(tls_stream);
    let _greeting = tokio::time::timeout(Duration::from_secs(10), client.read_response())
        .await
        .map_err(|_| ImapError::ConnectionError("IMAP greeting timed out (10s)".to_string()))?
        .ok_or_else(|| ImapError::ConnectionError("No IMAP greeting received".to_string()))?
        .map_err(|e| ImapError::ConnectionError(format!("IMAP greeting error: {}", e)))?;

    // 4. Login
    let session = tokio::time::timeout(Duration::from_secs(15), client.login(email, password))
        .await
        .map_err(|_| ImapError::CredentialsError("IMAP login timed out (15s)".to_string()))?
        .map_err(|(e, _client)| ImapError::CredentialsError(format!("Failed to login: {}", e)))?;

    Ok(session)
}
fn format_timestamp(timestamp: i64, timezone: Option<String>) -> String {
    // Convert timestamp to DateTime<Utc>
    let dt_utc = match DateTime::from_timestamp(timestamp, 0) {
        Some(dt) => dt,
        None => return "Invalid timestamp".to_string(),
    };

    // Convert to user's timezone if provided, otherwise use UTC
    let formatted = if let Some(tz_str) = timezone {
        match tz_str.parse::<Tz>() {
            Ok(tz) => dt_utc
                .with_timezone(&tz)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string(),
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
    pub attachments: Vec<String>,
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

/// Pure parser: turn a `async_imap::types::Fetch` message into an
/// `ImapEmailPreview`. No DB access, no ontology interaction. Extracted
/// from the old inline body of `fetch_emails_imap_for_account` so the
/// same parsing is used by the cron and IDLE code paths.
pub fn parse_imap_message(
    message: &async_imap::types::Fetch,
    user_timezone: Option<String>,
) -> Result<ImapEmailPreview, ImapError> {
    let uid = message.uid.unwrap_or(0).to_string();

    let envelope = message
        .envelope()
        .ok_or_else(|| ImapError::ParseError("Failed to get message envelope".to_string()))?;

    let (from, from_email) = envelope
        .from
        .as_ref()
        .and_then(|addrs| addrs.first())
        .map(|addr| {
            let name = addr
                .name
                .as_ref()
                .and_then(|n| String::from_utf8(n.to_vec()).ok())
                .unwrap_or_default();
            let email = addr
                .mailbox
                .as_ref()
                .and_then(|m| {
                    let mailbox = String::from_utf8(m.to_vec()).ok()?;
                    let host = addr
                        .host
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

    let date = raw_date.as_ref().and_then(|date_str| {
        match chrono::DateTime::parse_from_rfc2822(date_str) {
            Ok(dt) => Some(dt.with_timezone(&Utc)),
            Err(e) => {
                tracing::warn!("Failed to parse date '{}': {}", date_str, e);
                None
            }
        }
    });

    let is_read = message
        .flags()
        .any(|flag| flag == async_imap::types::Flag::Seen);

    // Try to get both full body and text body
    let full_body = message
        .body()
        .map(|b| String::from_utf8_lossy(b).into_owned());
    let text_body = message
        .text()
        .map(|b| String::from_utf8_lossy(b).into_owned());

    use mail_parser::MessageParser;
    let body_content = full_body.or(text_body);
    let (body, snippet) = body_content
        .as_ref()
        .map(|content| {
            let parser = MessageParser::default();
            let parsed = parser.parse(content.as_bytes());
            let clean_content = parsed
                .map(|msg| {
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
                })
                .unwrap_or_else(|| String::from("[Failed to parse email body]"));
            let snippet = clean_content.chars().take(200).collect::<String>();
            (clean_content, snippet)
        })
        .unwrap_or_else(|| (String::new(), String::new()));

    let date_formatted = date.map(|dt| format_timestamp(dt.timestamp(), user_timezone));

    Ok(ImapEmailPreview {
        id: uid,
        subject,
        from: Some(from),
        from_email: Some(from_email),
        date,
        date_formatted,
        snippet: Some(snippet),
        body: Some(body),
        is_read,
    })
}

/// Insert an email preview into `ont_messages` and emit an ontology
/// change event so rules can react. Idempotent: if a message with the
/// same `email_<uid>` room_id already exists for this user (e.g. the
/// cron fallback inserted it first), we skip the insert and return the
/// existing id.
///
/// Returns `Ok(Some(id))` on successful insert or discovered duplicate,
/// `Err` on DB failure. The caller should only mark the email as
/// processed after this returns `Ok`.
pub async fn insert_email_into_ontology(
    state: &Arc<AppState>,
    user_id: i32,
    preview: &ImapEmailPreview,
    persons: &[crate::models::ontology_models::PersonWithChannels],
) -> Result<i64, String> {
    let email_uid = preview.id.clone();
    let room_id = format!("email_{}", email_uid);

    // Cron-vs-IDLE dedup: if the row already exists, we're done.
    match state
        .ontology_repository
        .get_message_by_email_room_id(user_id, &room_id)
    {
        Ok(Some(existing)) => {
            tracing::debug!(
                "Email {} already in ontology for user {} (id={}), skipping insert",
                room_id,
                user_id,
                existing.id
            );
            return Ok(existing.id);
        }
        Ok(None) => {}
        Err(e) => return Err(format!("Failed to check existing email message: {}", e)),
    }

    // Match an ontology Person for the sender based on email channel handle.
    let mut matched_person_id: Option<i32> = None;
    let from_lower = preview.from_email.as_deref().unwrap_or("").to_lowercase();
    let from_name_lower = preview.from.as_deref().unwrap_or("").to_lowercase();
    for pwc in persons {
        for channel in &pwc.channels {
            if channel.platform == "email" {
                if let Some(ref handle) = channel.handle {
                    let handle_lower = handle.to_lowercase();
                    if from_lower.contains(&handle_lower) || from_name_lower.contains(&handle_lower)
                    {
                        matched_person_id = Some(pwc.person.id);
                        break;
                    }
                }
            }
        }
        if matched_person_id.is_some() {
            break;
        }
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i32;
    let sender_name = preview.from.as_deref().unwrap_or("Unknown").to_string();
    let content = format!(
        "{}\n{}",
        preview.subject.as_deref().unwrap_or(""),
        preview
            .body
            .as_deref()
            .unwrap_or("")
            .chars()
            .take(500)
            .collect::<String>()
    );

    let msg = crate::models::ontology_models::NewOntMessage {
        user_id,
        room_id: room_id.clone(),
        platform: "email".to_string(),
        sender_name: sender_name.clone(),
        content: content.clone(),
        person_id: matched_person_id,
        created_at: now,
    };

    let created = state
        .ontology_repository
        .insert_message(&msg)
        .map_err(|e| format!("Failed to insert email ont_message: {}", e))?;

    let snapshot = json!({
        "message_id": created.id,
        "platform": "email",
        "sender": sender_name,
        "sender_name": sender_name,
        "content": content,
        "room_id": room_id,
    });

    crate::proactive::rules::emit_ontology_change(
        state,
        user_id,
        "Message",
        created.id as i32,
        "created",
        snapshot,
    )
    .await;

    Ok(created.id)
}

/// Fetch new emails from an already-open IMAP session, insert them into
/// the ontology, and mark them as processed. Used by the IDLE loop on
/// every wake event (NewData) and at startup (initial resync).
///
/// * `since_uid = Some(n)`: fetch UID `(n+1):*` — normal resync path.
/// * `since_uid = None`: first-ever startup for this connection; fetch
///   the last 10 messages via the mailbox's `uid_next` counter.
///
/// Crucially: `mark_email_as_processed` is called ONLY after
/// `insert_email_into_ontology` returns `Ok`. If insertion fails, the
/// email stays unprocessed and will be retried on the next IDLE wake.
/// This fixes the old mark-before-insert ordering bug.
pub async fn process_new_emails(
    state: &Arc<AppState>,
    user_id: i32,
    imap_connection_id: i32,
    session: &mut ImapSession,
    since_uid: Option<u32>,
) -> Result<usize, ImapError> {
    // Select INBOX to get current UID counters.
    let mailbox = session
        .select("INBOX")
        .await
        .map_err(|e| ImapError::FetchError(format!("Failed to select INBOX: {}", e)))?;

    // Compute UID range.
    let uid_range = match since_uid {
        Some(n) => format!("{}:*", n.saturating_add(1)),
        None => {
            // First-ever startup — fetch the last 10 UIDs.
            let uid_next = mailbox.uid_next.unwrap_or(1);
            let start = uid_next.saturating_sub(10).max(1);
            format!("{}:{}", start, uid_next.saturating_sub(1).max(1))
        }
    };

    tracing::debug!(
        "process_new_emails: user={} conn={} fetching UID range {}",
        user_id,
        imap_connection_id,
        uid_range
    );

    let mut messages_stream = session
        .uid_fetch(&uid_range, "(UID FLAGS ENVELOPE BODY.PEEK[])")
        .await
        .map_err(|e| ImapError::FetchError(format!("Failed to uid_fetch: {}", e)))?;

    // Cache persons once for the whole batch.
    let persons = state
        .ontology_repository
        .get_persons_with_channels(user_id, 500, 0)
        .unwrap_or_default();

    let user_timezone = state
        .user_core
        .get_user_info(user_id)
        .ok()
        .and_then(|info| info.timezone);

    let mut new_count = 0usize;
    while let Some(message) = messages_stream
        .try_next()
        .await
        .map_err(|e| ImapError::FetchError(format!("Stream error reading messages: {}", e)))?
    {
        let preview = match parse_imap_message(&message, user_timezone.clone()) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("Skipping malformed IMAP message: {:?}", e);
                continue;
            }
        };

        // Skip if already processed (by UID).
        match state.user_repository.is_email_processed(
            user_id,
            &preview.id,
            Some(imap_connection_id),
        ) {
            Ok(true) => continue,
            Ok(false) => {}
            Err(e) => {
                tracing::warn!("Failed to check processed state for {}: {}", preview.id, e);
                continue;
            }
        }

        let uid_str = preview.id.clone();
        match insert_email_into_ontology(state, user_id, &preview, &persons).await {
            Ok(_) => {
                new_count += 1;
                // Only mark as processed AFTER successful ontology insertion.
                if let Err(e) = state.user_repository.mark_email_as_processed(
                    user_id,
                    &uid_str,
                    Some(imap_connection_id),
                ) {
                    tracing::warn!(
                        "Failed to mark email {} as processed (will retry next IDLE): {}",
                        uid_str,
                        e
                    );
                }
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to insert email {} into ontology (will retry next IDLE): {}",
                    uid_str,
                    e
                );
            }
        }
    }

    drop(messages_stream);
    Ok(new_count)
}
pub async fn fetch_imap_previews(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    axum::extract::Query(params): axum::extract::Query<FetchEmailsQuery>,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::info!(
        "Starting IMAP preview fetch for user {} with limit {:?}",
        auth_user.user_id,
        params.limit
    );
    match fetch_emails_imap(&state, auth_user.user_id, params.limit, false, false).await {
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
                        "date_formatted": p.date_formatted.unwrap_or_else(|| "Unknown date".to_string()),
                        "snippet": p.snippet.unwrap_or_else(|| "No preview".to_string()),
                        "is_read": p.is_read
                    })
                })
                .collect();
            Ok(AxumJson(
                json!({ "success": true, "previews": formatted_previews }),
            ))
        }
        Err(e) => {
            let (status, message) = match e {
                ImapError::NoConnection => (
                    StatusCode::BAD_REQUEST,
                    "No IMAP connection found".to_string(),
                ),
                ImapError::CredentialsError(msg) => (StatusCode::UNAUTHORIZED, msg),
                ImapError::ConnectionError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
                ImapError::FetchError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
                ImapError::ParseError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            };
            tracing::error!("IMAP preview fetch failed: {}", message);
            Err((status, AxumJson(json!({ "error": message }))))
        }
    }
}
pub async fn fetch_full_imap_emails(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    axum::extract::Query(params): axum::extract::Query<FetchEmailsQuery>,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::info!(
        "Starting IMAP full emails fetch for user {} with limit {:?}",
        auth_user.user_id,
        params.limit
    );
    let mut limit = params.limit;
    if limit.is_none() {
        limit = Some(5);
    }
    match fetch_emails_imap(&state, auth_user.user_id, limit, false, false).await {
        Ok(previews) => {
            tracing::info!("Fetched {} IMAP full emails", previews.len());

            let formatted_emails: Vec<_> = previews
                .into_iter()
                .map(|p| {
                    json!({
                        "id": p.id,
                        "subject": p.subject.unwrap_or_else(|| "No subject".to_string()),
                        "from": p.from_email.unwrap_or_else(|| "Unknown sender".to_string()),
                        "date": p.date.map(|dt| dt.to_rfc3339()),
                        "date_formatted": p.date_formatted.unwrap_or_else(|| "Unknown date".to_string()),
                        "snippet": p.snippet.unwrap_or_else(|| "No preview".to_string()),
                        "body": p.body.unwrap_or_else(|| "No content".to_string()),
                        "is_read": p.is_read
                    })
                })
                .collect();
            Ok(AxumJson(
                json!({ "success": true, "emails": formatted_emails }),
            ))
        }
        Err(e) => {
            let (status, message) = match e {
                ImapError::NoConnection => (
                    StatusCode::BAD_REQUEST,
                    "No IMAP connection found".to_string(),
                ),
                ImapError::CredentialsError(msg) => (StatusCode::UNAUTHORIZED, msg),
                ImapError::ConnectionError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
                ImapError::FetchError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
                ImapError::ParseError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            };
            tracing::error!("IMAP full emails fetch failed: {}", message);
            Err((status, AxumJson(json!({ "error": message }))))
        }
    }
}
#[derive(Debug, Deserialize)]
pub struct EmailResponseRequest {
    pub email_id: String,
    pub response_text: String,
    #[serde(default)]
    pub from: Option<String>,
}
// this is not used yet since it didn't work and not my priority rn
pub async fn respond_to_email(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<EmailResponseRequest>,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::info!(
        "Responding to email {} for user {}",
        request.email_id,
        auth_user.user_id
    );
    // Validate email_id is a valid number
    if !request.email_id.chars().all(|c| c.is_ascii_digit()) {
        tracing::error!("Invalid email ID format: {}", request.email_id);
        return Err((
            StatusCode::BAD_REQUEST,
            AxumJson(json!({ "error": "Invalid email ID format" })),
        ));
    }
    // Resolve which email account to use
    let cred = if let Some(ref from_email) = request.from {
        match state
            .user_repository
            .get_imap_credentials_by_email(auth_user.user_id, from_email)
        {
            Ok(Some(c)) => c,
            Ok(None) => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    AxumJson(
                        json!({ "error": format!("No connected email account for '{}'", from_email) }),
                    ),
                ))
            }
            Err(e) => {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    AxumJson(json!({ "error": format!("Failed to get IMAP credentials: {}", e) })),
                ))
            }
        }
    } else {
        match state
            .user_repository
            .get_imap_credentials(auth_user.user_id)
        {
            Ok(Some((email, password, imap_server, imap_port))) => {
                crate::repositories::user_repository::ImapConnectionInfo {
                    id: 0,
                    email,
                    password,
                    imap_server,
                    imap_port,
                }
            }
            Ok(None) => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    AxumJson(json!({ "error": "No IMAP connection found" })),
                ))
            }
            Err(e) => {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    AxumJson(json!({ "error": format!("Failed to get IMAP credentials: {}", e) })),
                ))
            }
        }
    };
    let email = cred.email;
    let password = cred.password;
    let imap_server = cred.imap_server;
    let imap_port = cred.imap_port;
    let server = imap_server
        .as_deref()
        .unwrap_or("imap.gmail.com")
        .to_string();
    let port = imap_port.unwrap_or(993) as u16;

    // Connect, login, fetch envelope, extract reply data — all async, no spawn_blocking.
    // Outer 60s wall-clock guard around the entire IMAP phase.
    let imap_result = tokio::time::timeout(std::time::Duration::from_secs(60), async {
        let mut imap_session = open_imap_session(&server, port, &email, &password).await?;
        tracing::info!("logged in");

        imap_session
            .select("INBOX")
            .await
            .map_err(|e| ImapError::FetchError(format!("Failed to select INBOX: {}", e)))?;

        let mut messages_stream = imap_session
            .uid_fetch(&request.email_id, "(ENVELOPE)")
            .await
            .map_err(|e| {
                ImapError::FetchError(format!("Failed to fetch original message: {}", e))
            })?;
        let original_message = messages_stream
            .try_next()
            .await
            .map_err(|e| ImapError::FetchError(format!("Failed to read original message: {}", e)))?
            .ok_or_else(|| ImapError::FetchError("Original message not found".to_string()))?;

        let envelope = original_message.envelope().ok_or_else(|| {
            ImapError::ParseError("Failed to get original message envelope".to_string())
        })?;

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
            .ok_or_else(|| ImapError::ParseError("Failed to get recipient address".to_string()))?;

        let original_subject = envelope
            .subject
            .as_ref()
            .map(|s| String::from_utf8_lossy(s).into_owned())
            .unwrap_or_else(|| String::from("No subject"));

        // Drain remaining stream items so we can drop it cleanly before logout
        drop(messages_stream);

        // Best-effort logout (ignore errors)
        if let Err(e) = imap_session.logout().await {
            tracing::warn!("Failed to logout from IMAP: {}", e);
        }

        Ok::<(String, String), ImapError>((reply_to_address, original_subject))
    })
    .await;

    let (reply_to_address, original_subject) = match imap_result {
        Ok(Ok(pair)) => pair,
        Ok(Err(ImapError::CredentialsError(msg))) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({ "error": msg })),
            ))
        }
        Ok(Err(ImapError::FetchError(msg))) | Ok(Err(ImapError::ParseError(msg))) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({ "error": msg })),
            ))
        }
        Ok(Err(ImapError::ConnectionError(msg))) => {
            return Err((
                StatusCode::GATEWAY_TIMEOUT,
                AxumJson(json!({ "error": msg })),
            ))
        }
        Ok(Err(ImapError::NoConnection)) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({ "error": "No IMAP connection" })),
            ))
        }
        Err(_) => {
            return Err((
                StatusCode::GATEWAY_TIMEOUT,
                AxumJson(json!({ "error": "IMAP fetch wall-clock timeout (60s)" })),
            ))
        }
    };

    tracing::info!("reply addr: {}", reply_to_address);
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
    let creds = Credentials::new(email.clone(), password.clone());
    tracing::info!("created the smtp transport");
    let mailer = lettre::SmtpTransport::starttls_relay(&smtp_server)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({ "error": format!("Failed to create SMTP relay: {}", e) })),
            )
        })?
        .port(smtp_port)
        .credentials(creds)
        .build();
    // Create email message
    let email_message = match Message::builder()
        .from(email.parse().unwrap())
        .to(reply_to_address.parse().unwrap())
        .subject(subject.clone())
        .body(request.response_text.clone())
    {
        Ok(message) => message,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({ "error": format!("Failed to create email message: {}", e) })),
            ))
        }
    };
    tracing::info!("Attempting to send email via SMTP...");
    tracing::info!(
        "SMTP Configuration - Server: {}, Port: {}",
        smtp_server,
        smtp_port
    );

    // Attempt to send the email with detailed error logging
    let send_result = mailer.send(&email_message);

    match send_result {
        Ok(_) => {
            tracing::info!("Email sent successfully via SMTP");
            Ok(AxumJson(json!({
                "success": true,
                "message": "Email response sent successfully"
            })))
        }
        Err(e) => {
            // Log detailed error information
            tracing::error!("SMTP send error: {:?}", e);
            tracing::error!("SMTP error details: {}", e.to_string());

            // Log SMTP connection details for debugging (excluding credentials)
            tracing::debug!(
                "SMTP connection details - Server: {}, Port: {}",
                smtp_server,
                smtp_port
            );

            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({
                    "error": format!("Failed to send email via SMTP: {}", e),
                    "details": e.to_string()
                })),
            ))
        }
    }
}
pub async fn fetch_single_imap_email(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    axum::extract::Path(email_id): axum::extract::Path<String>,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::info!(
        "Fetching single IMAP email {} for user {}",
        email_id,
        auth_user.user_id
    );
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
            AxumJson(json!({
                "error": error_msg,
                "email_id": email_id
            })),
        ));
    }
    match fetch_single_email_imap(&state, auth_user.user_id, &email_id).await {
        Ok(email) => {
            tracing::debug!("Successfully fetched email {}", email_id);
            Ok(AxumJson(json!({
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
                    (
                        StatusCode::BAD_REQUEST,
                        "No IMAP connection found".to_string(),
                    )
                }
                ImapError::CredentialsError(msg) => {
                    tracing::error!(
                        "IMAP credentials error for user {}: {}",
                        auth_user.user_id,
                        msg
                    );
                    (StatusCode::UNAUTHORIZED, msg)
                }
                ImapError::ConnectionError(msg) => {
                    tracing::error!(
                        "IMAP connection error for user {}: {}",
                        auth_user.user_id,
                        msg
                    );
                    (StatusCode::INTERNAL_SERVER_ERROR, msg)
                }
                ImapError::FetchError(msg) => {
                    tracing::error!(
                        "IMAP fetch error for email {} user {}: {}",
                        email_id,
                        auth_user.user_id,
                        msg
                    );
                    (StatusCode::INTERNAL_SERVER_ERROR, msg)
                }
                ImapError::ParseError(msg) => {
                    tracing::error!(
                        "IMAP parse error for email {} user {}: {}",
                        email_id,
                        auth_user.user_id,
                        msg
                    );
                    (StatusCode::INTERNAL_SERVER_ERROR, msg)
                }
            };
            Err((
                status,
                AxumJson(json!({
                    "error": message,
                    "email_id": email_id
                })),
            ))
        }
    }
}
pub async fn fetch_emails_imap(
    state: &AppState,
    user_id: i32,
    limit: Option<u32>,
    unprocessed: bool,
    unread_only: bool,
) -> Result<Vec<ImapEmailPreview>, ImapError> {
    tracing::debug!(
        "Starting fetch_emails_imap for user {} with limit: {:?}, unprocessed: {}",
        user_id,
        limit,
        unprocessed
    );

    // Fetch from all connected accounts and merge results
    let accounts = state
        .user_repository
        .get_all_imap_credentials(user_id)
        .map_err(|e| ImapError::CredentialsError(e.to_string()))?;

    if accounts.is_empty() {
        return Err(ImapError::NoConnection);
    }

    let mut all_previews = Vec::new();
    for account in &accounts {
        match fetch_emails_imap_for_account(
            state,
            user_id,
            &account.email,
            &account.password,
            account.imap_server.as_deref(),
            account.imap_port,
            limit,
            unprocessed,
            unread_only,
        )
        .await
        {
            Ok(mut previews) => {
                // Tag each preview's account source in the from field
                for p in &mut previews {
                    if p.from.is_none() || p.from.as_deref() == Some("") {
                        p.from = Some(account.email.clone());
                    }
                }
                all_previews.extend(previews);
            }
            Err(e) => {
                tracing::error!(
                    "Failed to fetch emails from {} for user {}: {:?}",
                    account.email,
                    user_id,
                    e
                );
                // Continue with other accounts
            }
        }
    }

    // Sort by date descending (newest first)
    all_previews.sort_by(|a, b| b.date.cmp(&a.date));

    // Apply limit across all accounts
    if let Some(lim) = limit {
        all_previews.truncate(lim as usize);
    }

    Ok(all_previews)
}

/// Fetch emails from a single IMAP account
#[allow(clippy::too_many_arguments)]
async fn fetch_emails_imap_for_account(
    state: &AppState,
    user_id: i32,
    email: &str,
    password: &str,
    imap_server: Option<&str>,
    imap_port: Option<i32>,
    limit: Option<u32>,
    unprocessed: bool,
    unread_only: bool,
) -> Result<Vec<ImapEmailPreview>, ImapError> {
    tracing::debug!("Fetching IMAP emails for user {}", user_id);

    let server = imap_server.unwrap_or("imap.gmail.com").to_string();
    let port = imap_port.unwrap_or(993) as u16;
    let limit = limit.unwrap_or(20);

    // Outer wall-clock guard around the entire fetch flow
    let fetch_result: Result<Vec<ImapEmailPreview>, ImapError> = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        async {
            let mut imap_session = open_imap_session(&server, port, email, password).await?;

            let mailbox = imap_session
                .select("INBOX")
                .await
                .map_err(|e| ImapError::FetchError(format!("Failed to select INBOX: {}", e)))?;

            let sequence_set = format!(
                "{}:{}",
                (mailbox.exists.saturating_sub(limit - 1)),
                mailbox.exists
            );

            let mut messages_stream = imap_session
                .fetch(&sequence_set, "(UID FLAGS ENVELOPE BODY.PEEK[])")
                .await
                .map_err(|e| ImapError::FetchError(format!("Failed to fetch messages: {}", e)))?;

            let user_timezone = state
                .user_core
                .get_user_info(user_id)
                .ok()
                .and_then(|info| info.timezone);

            let mut email_previews = Vec::new();
            while let Some(message) = messages_stream.try_next().await.map_err(|e| {
                ImapError::FetchError(format!("Stream error reading messages: {}", e))
            })? {
                let uid_str = message.uid.unwrap_or(0).to_string();

                // Skip already-processed emails when the caller asks us to.
                // We no longer mark processed inside this fetcher — that
                // happens in the scheduler after successful ontology
                // insertion, so a failed insert gets retried next run.
                if unprocessed {
                    match state
                        .user_repository
                        .is_email_processed(user_id, &uid_str, None)
                    {
                        Ok(true) => continue,
                        Ok(false) => {}
                        Err(e) => {
                            return Err(ImapError::FetchError(format!(
                                "Failed to check email processed status: {}",
                                e
                            )));
                        }
                    }
                }

                let preview = match parse_imap_message(&message, user_timezone.clone()) {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::warn!("Skipping malformed IMAP message: {:?}", e);
                        continue;
                    }
                };

                // Skip read emails if unread_only is true.
                if unread_only && preview.is_read {
                    continue;
                }

                email_previews.push(preview);
            }

            // Drop stream before logout to release the borrow on imap_session
            drop(messages_stream);

            // Logout - don't fail if logout fails, we already have the emails
            if let Err(e) = imap_session.logout().await {
                tracing::warn!("Failed to logout from IMAP session: {} - ignoring since emails were fetched successfully", e);
            }

            Ok(email_previews)
        },
    )
    .await
    .unwrap_or_else(|_| {
        Err(ImapError::ConnectionError(
            "IMAP fetch wall-clock timeout (60s)".to_string(),
        ))
    });

    fetch_result
}
pub async fn fetch_single_email_imap(
    state: &AppState,
    user_id: i32,
    email_id: &str,
) -> Result<ImapEmail, ImapError> {
    // Try all connected accounts to find the email
    let accounts = state
        .user_repository
        .get_all_imap_credentials(user_id)
        .map_err(|e| ImapError::CredentialsError(e.to_string()))?;

    if accounts.is_empty() {
        return Err(ImapError::NoConnection);
    }

    // Try each account - return first success
    let mut last_err = ImapError::NoConnection;
    for account in &accounts {
        match fetch_single_email_from_account(
            state,
            user_id,
            email_id,
            &account.email,
            &account.password,
            account.imap_server.as_deref(),
            account.imap_port,
        )
        .await
        {
            Ok(email) => return Ok(email),
            Err(e) => {
                tracing::debug!(
                    "Email {} not found in account {} for user {}",
                    email_id,
                    account.email,
                    user_id
                );
                last_err = e;
            }
        }
    }
    Err(last_err)
}

async fn fetch_single_email_from_account(
    state: &AppState,
    user_id: i32,
    email_id: &str,
    email: &str,
    password: &str,
    imap_server: Option<&str>,
    imap_port: Option<i32>,
) -> Result<ImapEmail, ImapError> {
    let server = imap_server.unwrap_or("imap.gmail.com").to_string();
    let port = imap_port.unwrap_or(993) as u16;

    // Wrap the entire IMAP session in a wall-clock timeout for safety
    let result: Result<ImapEmail, ImapError> = tokio::time::timeout(
        std::time::Duration::from_secs(60),
        async {
            let mut imap_session = open_imap_session(&server, port, email, password).await?;
            imap_session
                .select("INBOX")
                .await
                .map_err(|e| ImapError::FetchError(format!("Failed to select INBOX: {}", e)))?;

            // Fetch specific message with body structure for attachments
            // Using BODY.PEEK[] to avoid marking the email as read
            let mut messages_stream = imap_session
                .uid_fetch(email_id, "(UID FLAGS ENVELOPE BODY.PEEK[] BODYSTRUCTURE)")
                .await
                .map_err(|e| {
                    tracing::error!("Failed to fetch message with UID {}: {}", email_id, e);
                    ImapError::FetchError(format!("Failed to fetch message: {}", e))
                })?;
            let message = messages_stream
                .try_next()
                .await
                .map_err(|e| {
                    ImapError::FetchError(format!("Stream error reading message: {}", e))
                })?
                .ok_or_else(|| {
                    tracing::error!("No message found with UID {}", email_id);
                    ImapError::FetchError(format!("Message with UID {} not found", email_id))
                })?;
    // Verify the UID matches
    let msg_uid = message.uid.ok_or_else(|| {
        tracing::error!("Message found but has no UID");
        ImapError::ParseError("Message has no UID".to_string())
    })?;
    if msg_uid.to_string() != email_id {
        tracing::error!("UID mismatch: expected {}, got {}", email_id, msg_uid);
        return Err(ImapError::FetchError(format!(
            "Message UID mismatch: expected {}, got {}",
            email_id, msg_uid
        )));
    }
    let envelope = message
        .envelope()
        .ok_or_else(|| ImapError::ParseError("Failed to get message envelope".to_string()))?;
    let from = envelope
        .from
        .as_ref()
        .and_then(|addrs| addrs.first())
        .map(|addr| {
            let name = addr
                .name
                .as_ref()
                .and_then(|n| String::from_utf8(n.to_vec()).ok())
                .unwrap_or_default();
            let email = format!(
                "{}@{}",
                addr.mailbox
                    .as_ref()
                    .and_then(|m| String::from_utf8(m.to_vec()).ok())
                    .unwrap_or_default(),
                addr.host
                    .as_ref()
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
                addr.mailbox
                    .as_ref()
                    .and_then(|m| String::from_utf8(m.to_vec()).ok())
                    .unwrap_or_default(),
                addr.host
                    .as_ref()
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
        .any(|flag| flag == async_imap::types::Flag::Seen);

    // Try to get both full body and text body
    let full_body = message
        .body()
        .map(|b| String::from_utf8_lossy(b).into_owned());
    let text_body = message
        .text()
        .map(|b| String::from_utf8_lossy(b).into_owned());
    use mail_parser::MessageParser;
    let body_content = full_body.or(text_body);
    let (body, snippet, attachments) = match body_content.as_ref() {
        Some(content) => {
            // Create a parser and parse the content into an Option<Message>
            let parser = MessageParser::default();
            let parsed = parser.parse(content.as_bytes());
            // Get the best available body content, if parsing succeeded
            let clean_content = parsed
                .as_ref()
                .map(|msg| {
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
                })
                .unwrap_or_else(|| String::from("[Failed to parse email body]"));
            // Generate a snippet from the clean body
            let snippet = clean_content.chars().take(200).collect::<String>();
            (clean_content, snippet, Vec::new())
        }
        None => (String::new(), String::new(), Vec::new()),
    };
    let date_formatted = date.map(|dt| {
        format_timestamp(
            dt.timestamp(),
            state
                .user_core
                .get_user_info(user_id)
                .ok()
                .and_then(|info| info.timezone),
        )
    });

    let email_struct = ImapEmail {
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
    };

    // Drop stream and logout (best-effort)
    drop(messages_stream);
    if let Err(e) = imap_session.logout().await {
        tracing::warn!("Failed to logout from IMAP session: {} - ignoring since email was fetched successfully", e);
    }

    Ok::<ImapEmail, ImapError>(email_struct)
        },
    )
    .await
    .unwrap_or_else(|_| {
        Err(ImapError::ConnectionError(
            "IMAP fetch wall-clock timeout (60s)".to_string(),
        ))
    });

    result
}

#[derive(Debug, Deserialize)]
pub struct SendEmailRequest {
    pub to: String,
    pub subject: String,
    pub body: String,
    #[serde(default)]
    pub from: Option<String>,
}
pub async fn send_email(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<SendEmailRequest>,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::info!(
        "Sending new email to {} for user {}",
        request.to,
        auth_user.user_id
    );
    // Resolve which email account to use
    let cred = if let Some(ref from_email) = request.from {
        // Use specific account
        match state
            .user_repository
            .get_imap_credentials_by_email(auth_user.user_id, from_email)
        {
            Ok(Some(c)) => c,
            Ok(None) => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    AxumJson(
                        json!({ "error": format!("No connected email account for '{}'", from_email) }),
                    ),
                ))
            }
            Err(e) => {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    AxumJson(json!({ "error": format!("Failed to get email credentials: {}", e) })),
                ))
            }
        }
    } else {
        // Fall back to first active account
        match state
            .user_repository
            .get_imap_credentials(auth_user.user_id)
        {
            Ok(Some((email, password, imap_server, imap_port))) => {
                crate::repositories::user_repository::ImapConnectionInfo {
                    id: 0,
                    email,
                    password,
                    imap_server,
                    imap_port,
                }
            }
            Ok(None) => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    AxumJson(json!({ "error": "No email credentials found" })),
                ))
            }
            Err(e) => {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    AxumJson(json!({ "error": format!("Failed to get email credentials: {}", e) })),
                ))
            }
        }
    };
    let email = cred.email;
    let password = cred.password;
    let imap_server = cred.imap_server;
    // Derive SMTP server from IMAP server (common pattern, e.g., imap.gmail.com -> smtp.gmail.com)
    let smtp_server = imap_server
        .as_deref()
        .unwrap_or("smtp.gmail.com")
        .replace("imap", "smtp");
    let smtp_port = 587; // Standard STARTTLS port for SMTP
                         // Set up credentials and SMTP transport
    let creds = Credentials::new(email.clone(), password.clone());
    let mailer = lettre::SmtpTransport::starttls_relay(&smtp_server)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({ "error": format!("Failed to create SMTP relay: {}", e) })),
            )
        })?
        .port(smtp_port)
        .credentials(creds)
        .build();
    // Build the email message
    use lettre::message::{
        header::{ContentTransferEncoding, ContentType},
        SinglePart,
    };

    // Auto-detect HTML content
    let is_html = request.body.trim_start().starts_with("<!DOCTYPE")
        || request.body.trim_start().starts_with("<html");

    let content_type = if is_html {
        "text/html; charset=utf-8"
    } else {
        "text/plain; charset=us-ascii"
    };

    let part = SinglePart::builder()
        .header(ContentType::parse(content_type).map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                AxumJson(json!({ "error": format!("Invalid content type: {}", e) })),
            )
        })?)
        .header(if is_html {
            ContentTransferEncoding::QuotedPrintable
        } else {
            ContentTransferEncoding::SevenBit
        })
        .body(request.body.clone());
    // Format sender with display name "Lightfriend"
    let from_address = format!("Lightfriend <{}>", email);
    let email_message = match Message::builder()
        .from(from_address.parse().map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                AxumJson(json!({ "error": format!("Invalid sender email format: {}", e) })),
            )
        })?)
        .to(request.to.parse().map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                AxumJson(json!({ "error": format!("Invalid recipient email format: {}", e) })),
            )
        })?)
        .subject(request.subject.clone())
        .singlepart(part)
    {
        Ok(message) => message,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({ "error": format!("Failed to build email message: {}", e) })),
            ))
        }
    };
    // Send the email
    tracing::info!("Attempting to send email via SMTP to {}", request.to);
    tracing::debug!(
        "SMTP Configuration - Server: {}, Port: {}",
        smtp_server,
        smtp_port
    );
    match mailer.send(&email_message) {
        Ok(_) => {
            tracing::info!("Email sent successfully to {}", request.to);
            Ok(AxumJson(json!({
                "success": true,
                "message": "Email sent successfully"
            })))
        }
        Err(e) => {
            tracing::error!("Failed to send email to {}: {:?}", request.to, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({
                    "error": format!("Failed to send email: {}", e),
                    "details": e.to_string()
                })),
            ))
        }
    }
}
