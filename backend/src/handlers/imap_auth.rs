use crate::{handlers::auth_middleware::AuthUser, AppState};
use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::Json as AxumJson,
};
use imap::Session;
use native_tls::TlsConnector;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::error::Error;
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::time::Duration;

// ============================================================
// Pure Functions for Testability
// ============================================================

/// Validate email format using basic rules.
/// Returns true if email appears to be valid format.
/// Note: This is a basic validation, not a full RFC 5322 check.
pub fn is_valid_email(email: &str) -> bool {
    // Check for exactly one @ symbol
    let parts: Vec<&str> = email.split('@').collect();
    if parts.len() != 2 {
        return false;
    }

    let local_part = parts[0];
    let domain_part = parts[1];

    // Local part cannot be empty
    if local_part.is_empty() {
        return false;
    }

    // Domain must contain at least one dot
    if !domain_part.contains('.') {
        return false;
    }

    // Domain cannot start or end with a dot
    if domain_part.starts_with('.') || domain_part.ends_with('.') {
        return false;
    }

    // Domain parts cannot be empty (no consecutive dots)
    let domain_parts: Vec<&str> = domain_part.split('.').collect();
    if domain_parts.iter().any(|p| p.is_empty()) {
        return false;
    }

    true
}

/// Extract domain from email address.
/// Returns None if email is invalid.
pub fn extract_email_domain(email: &str) -> Option<&str> {
    email.split('@').nth(1)
}

/// IMAP server configuration
#[derive(Debug, Clone, PartialEq)]
pub struct ImapConfig {
    pub host: &'static str,
    pub port: u16,
    pub tls: bool,
}

/// Detect IMAP server configuration from email address.
/// Returns ImapConfig with server details, or None if unknown provider.
/// This is an alternative to detect_imap_server that returns a struct.
pub fn detect_imap_config(email: &str) -> Option<ImapConfig> {
    let domain = extract_email_domain(email)?.to_lowercase();

    let config = match domain.as_str() {
        "icloud.com" | "me.com" | "mac.com" => ImapConfig {
            host: "imap.mail.me.com",
            port: 993,
            tls: true,
        },
        "gmail.com" | "googlemail.com" => ImapConfig {
            host: "imap.gmail.com",
            port: 993,
            tls: true,
        },
        "outlook.com" | "hotmail.com" | "live.com" | "msn.com" => ImapConfig {
            host: "outlook.office365.com",
            port: 993,
            tls: true,
        },
        "yahoo.com" | "yahoo.co.uk" | "yahoo.fr" | "yahoo.de" => ImapConfig {
            host: "imap.mail.yahoo.com",
            port: 993,
            tls: true,
        },
        "aol.com" => ImapConfig {
            host: "imap.aol.com",
            port: 993,
            tls: true,
        },
        "zoho.com" | "zohomail.com" => ImapConfig {
            host: "imap.zoho.com",
            port: 993,
            tls: true,
        },
        "fastmail.com" | "fastmail.fm" => ImapConfig {
            host: "imap.fastmail.com",
            port: 993,
            tls: true,
        },
        _ => return None,
    };

    Some(config)
}

/// Check if an email provider is a known supported provider.
pub fn is_known_email_provider(email: &str) -> bool {
    detect_imap_config(email).is_some()
}

// Struct to deserialize the incoming IMAP credentials from the frontend
#[derive(Deserialize)]
pub struct ImapCredentials {
    email: String,
    password: String,
    #[serde(default)]
    imap_server: Option<String>, // e.g., "mail.privateemail.com" or "imap.gmail.com"
    #[serde(default)]
    imap_port: Option<u16>, // e.g., 993
}

/// Detects the IMAP server based on email domain.
/// Returns (server, port) tuple.
pub fn detect_imap_server(email: &str) -> (&'static str, u16) {
    let domain = email.split('@').nth(1).unwrap_or("").to_lowercase();

    match domain.as_str() {
        // iCloud
        "icloud.com" | "me.com" | "mac.com" => ("imap.mail.me.com", 993),
        // Google
        "gmail.com" | "googlemail.com" => ("imap.gmail.com", 993),
        // Microsoft
        "outlook.com" | "hotmail.com" | "live.com" | "msn.com" => ("outlook.office365.com", 993),
        // Yahoo
        "yahoo.com" | "yahoo.co.uk" | "yahoo.fr" | "yahoo.de" => ("imap.mail.yahoo.com", 993),
        // AOL
        "aol.com" => ("imap.aol.com", 993),
        // Zoho
        "zoho.com" | "zohomail.com" => ("imap.zoho.com", 993),
        // FastMail
        "fastmail.com" | "fastmail.fm" => ("imap.fastmail.com", 993),
        // Default to Gmail (legacy behavior, but log warning)
        _ => ("imap.gmail.com", 993),
    }
}

// ─── Provider auto-detection ────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct DetectProviderRequest {
    pub email: String,
}

#[derive(Serialize)]
pub struct DetectedProvider {
    /// Provider key (e.g. "gmail", "outlook", "privateemail", "unknown")
    pub provider: String,
    /// Human-readable label
    pub label: String,
    /// IMAP server (empty if unknown)
    pub imap_server: String,
    /// IMAP port
    pub imap_port: u16,
    /// Instructions for the user (e.g. "Create an App Password")
    pub instructions: Option<String>,
    /// Link for creating app password
    pub instructions_url: Option<String>,
}

/// Detect email provider from the email address using domain matching and MX record lookup.
pub async fn detect_provider(
    _auth_user: AuthUser,
    Json(payload): Json<DetectProviderRequest>,
) -> Result<AxumJson<DetectedProvider>, (StatusCode, AxumJson<serde_json::Value>)> {
    let email = payload.email.trim().to_lowercase();

    if !is_valid_email(&email) {
        return Err((
            StatusCode::BAD_REQUEST,
            AxumJson(json!({"error": "Invalid email address"})),
        ));
    }

    let domain = extract_email_domain(&email).unwrap_or("");

    // Step 1: Try direct domain match
    if let Some(provider) = detect_provider_from_domain(domain) {
        return Ok(AxumJson(provider));
    }

    // Step 2: MX record lookup for custom domains
    if let Some(provider) = detect_provider_from_mx(domain).await {
        return Ok(AxumJson(provider));
    }

    // Step 3: Unknown - user will need to pick manually
    Ok(AxumJson(DetectedProvider {
        provider: "unknown".to_string(),
        label: "Unknown Provider".to_string(),
        imap_server: String::new(),
        imap_port: 993,
        instructions: None,
        instructions_url: None,
    }))
}

fn detect_provider_from_domain(domain: &str) -> Option<DetectedProvider> {
    let domain = domain.to_lowercase();
    match domain.as_str() {
        "gmail.com" | "googlemail.com" => Some(DetectedProvider {
            provider: "gmail".to_string(),
            label: "Gmail".to_string(),
            imap_server: "imap.gmail.com".to_string(),
            imap_port: 993,
            instructions: Some("Gmail requires an App Password (2FA must be enabled).".to_string()),
            instructions_url: Some("https://myaccount.google.com/apppasswords".to_string()),
        }),
        "icloud.com" | "me.com" | "mac.com" => Some(DetectedProvider {
            provider: "icloud".to_string(),
            label: "iCloud".to_string(),
            imap_server: "imap.mail.me.com".to_string(),
            imap_port: 993,
            instructions: Some("iCloud requires an App-Specific Password. Go to Apple ID Settings > Sign-In and Security > App-Specific Passwords.".to_string()),
            instructions_url: Some("https://appleid.apple.com/account/manage".to_string()),
        }),
        "outlook.com" | "hotmail.com" | "live.com" | "msn.com" => Some(DetectedProvider {
            provider: "outlook".to_string(),
            label: "Outlook / Hotmail".to_string(),
            imap_server: "outlook.office365.com".to_string(),
            imap_port: 993,
            instructions: Some("Use your regular password, or create an app password if 2FA is enabled.".to_string()),
            instructions_url: Some("https://account.microsoft.com/security".to_string()),
        }),
        "yahoo.com" | "yahoo.co.uk" | "yahoo.fr" | "yahoo.de" => Some(DetectedProvider {
            provider: "yahoo".to_string(),
            label: "Yahoo Mail".to_string(),
            imap_server: "imap.mail.yahoo.com".to_string(),
            imap_port: 993,
            instructions: Some("Yahoo requires an App Password (2FA must be enabled).".to_string()),
            instructions_url: Some("https://login.yahoo.com/myaccount/security/app-password".to_string()),
        }),
        "aol.com" => Some(DetectedProvider {
            provider: "aol".to_string(),
            label: "AOL".to_string(),
            imap_server: "imap.aol.com".to_string(),
            imap_port: 993,
            instructions: Some("AOL requires an App Password (2FA must be enabled).".to_string()),
            instructions_url: Some("https://login.aol.com/myaccount/security/app-password".to_string()),
        }),
        "zoho.com" | "zohomail.com" => Some(DetectedProvider {
            provider: "zoho".to_string(),
            label: "Zoho Mail".to_string(),
            imap_server: "imap.zoho.com".to_string(),
            imap_port: 993,
            instructions: Some("Make sure IMAP is enabled in your Zoho settings.".to_string()),
            instructions_url: None,
        }),
        "fastmail.com" | "fastmail.fm" => Some(DetectedProvider {
            provider: "fastmail".to_string(),
            label: "Fastmail".to_string(),
            imap_server: "imap.fastmail.com".to_string(),
            imap_port: 993,
            instructions: Some("Fastmail recommends using an App Password.".to_string()),
            instructions_url: Some("https://www.fastmail.com/settings/security/devicekeys".to_string()),
        }),
        "yandex.com" | "yandex.ru" => Some(DetectedProvider {
            provider: "yandex".to_string(),
            label: "Yandex".to_string(),
            imap_server: "imap.yandex.com".to_string(),
            imap_port: 993,
            instructions: None,
            instructions_url: None,
        }),
        "gmx.com" | "gmx.net" => Some(DetectedProvider {
            provider: "gmx".to_string(),
            label: "GMX".to_string(),
            imap_server: "imap.gmx.com".to_string(),
            imap_port: 993,
            instructions: None,
            instructions_url: None,
        }),
        _ => None,
    }
}

/// Look up MX records for a domain and try to match against known providers.
async fn detect_provider_from_mx(domain: &str) -> Option<DetectedProvider> {
    use hickory_resolver::TokioResolver;

    let resolver = match TokioResolver::builder_tokio() {
        Ok(builder) => builder.build(),
        Err(e) => {
            tracing::debug!("Failed to create DNS resolver: {}", e);
            return None;
        }
    };
    let mx_records = match resolver.mx_lookup(domain).await {
        Ok(records) => records,
        Err(e) => {
            tracing::debug!("MX lookup failed for {}: {}", domain, e);
            return None;
        }
    };

    let mx_output: String = mx_records
        .iter()
        .map(|mx| mx.exchange().to_lowercase().to_string())
        .collect::<Vec<_>>()
        .join(" ");
    tracing::debug!("MX lookup for {}: {}", domain, mx_output);

    // Match MX records to known providers
    if mx_output.contains("google.com") || mx_output.contains("googlemail.com") {
        Some(DetectedProvider {
            provider: "gmail".to_string(),
            label: "Google Workspace".to_string(),
            imap_server: "imap.gmail.com".to_string(),
            imap_port: 993,
            instructions: Some(
                "Google Workspace requires an App Password (2FA must be enabled).".to_string(),
            ),
            instructions_url: Some("https://myaccount.google.com/apppasswords".to_string()),
        })
    } else if mx_output.contains("outlook.com")
        || mx_output.contains("protection.outlook.com")
        || mx_output.contains("microsoft.com")
    {
        Some(DetectedProvider {
            provider: "outlook".to_string(),
            label: "Microsoft 365".to_string(),
            imap_server: "outlook.office365.com".to_string(),
            imap_port: 993,
            instructions: Some(
                "Use your regular password, or create an app password if 2FA is enabled."
                    .to_string(),
            ),
            instructions_url: Some("https://account.microsoft.com/security".to_string()),
        })
    } else if mx_output.contains("zoho.com") {
        Some(DetectedProvider {
            provider: "zoho".to_string(),
            label: "Zoho Mail".to_string(),
            imap_server: "imap.zoho.com".to_string(),
            imap_port: 993,
            instructions: Some("Make sure IMAP is enabled in your Zoho settings.".to_string()),
            instructions_url: None,
        })
    } else if mx_output.contains("yahoodns.net") || mx_output.contains("yahoo.com") {
        Some(DetectedProvider {
            provider: "yahoo".to_string(),
            label: "Yahoo Mail".to_string(),
            imap_server: "imap.mail.yahoo.com".to_string(),
            imap_port: 993,
            instructions: Some("Yahoo requires an App Password (2FA must be enabled).".to_string()),
            instructions_url: Some(
                "https://login.yahoo.com/myaccount/security/app-password".to_string(),
            ),
        })
    } else if mx_output.contains("icloud.com") || mx_output.contains("apple.com") {
        Some(DetectedProvider {
            provider: "icloud".to_string(),
            label: "iCloud".to_string(),
            imap_server: "imap.mail.me.com".to_string(),
            imap_port: 993,
            instructions: Some("iCloud requires an App-Specific Password.".to_string()),
            instructions_url: Some("https://appleid.apple.com/account/manage".to_string()),
        })
    } else if mx_output.contains("fastmail.com") || mx_output.contains("messagingengine.com") {
        Some(DetectedProvider {
            provider: "fastmail".to_string(),
            label: "Fastmail".to_string(),
            imap_server: "imap.fastmail.com".to_string(),
            imap_port: 993,
            instructions: Some("Fastmail recommends using an App Password.".to_string()),
            instructions_url: Some(
                "https://www.fastmail.com/settings/security/devicekeys".to_string(),
            ),
        })
    } else if mx_output.contains("privateemail.com") {
        Some(DetectedProvider {
            provider: "privateemail".to_string(),
            label: "PrivateEmail (Namecheap)".to_string(),
            imap_server: "mail.privateemail.com".to_string(),
            imap_port: 993,
            instructions: Some("Use your email password.".to_string()),
            instructions_url: None,
        })
    } else if mx_output.contains("secureserver.net") {
        Some(DetectedProvider {
            provider: "godaddy".to_string(),
            label: "GoDaddy".to_string(),
            imap_server: "imap.secureserver.net".to_string(),
            imap_port: 993,
            instructions: Some("Use your email password.".to_string()),
            instructions_url: None,
        })
    } else if mx_output.contains("hostinger.com") {
        Some(DetectedProvider {
            provider: "hostinger".to_string(),
            label: "Hostinger".to_string(),
            imap_server: "imap.hostinger.com".to_string(),
            imap_port: 993,
            instructions: Some("Use your email password.".to_string()),
            instructions_url: None,
        })
    } else {
        None
    }
}

// Struct to serialize the IMAP status response (single - kept for backwards compat)
#[derive(Serialize)]
pub struct ImapStatus {
    connected: bool,
    email: Option<String>,
}

// Multi-account status response
#[derive(Serialize)]
pub struct ImapAccountStatus {
    pub email: String,
    pub connected: bool,
}

#[derive(Serialize)]
pub struct ImapStatusMulti {
    pub connected: bool,
    pub connections: Vec<ImapAccountStatus>,
}

#[derive(Deserialize)]
pub struct DeleteImapRequest {
    pub email: String,
}

use native_tls::TlsStream;

// Function to establish an IMAP connection for credential verification
async fn connect_imap(
    email: &str,
    password: &str,
    imap_server: Option<&str>,
    imap_port: Option<u16>,
) -> Result<Session<TlsStream<TcpStream>>, Box<dyn Error>> {
    let tls = TlsConnector::builder().build()?;

    // Use provided server/port or auto-detect from email domain
    let (detected_server, detected_port) = detect_imap_server(email);
    let server = imap_server.unwrap_or(detected_server);
    let port = imap_port.unwrap_or(detected_port);

    tracing::debug!(
        "Connecting to IMAP server {} on port {} for email {}",
        server,
        port,
        email
    );

    // Resolve hostname first. Parsing "host:port" as SocketAddr only works for
    // numeric IPs and breaks normal IMAP hosts like imap.gmail.com.
    let resolved_addrs: Vec<_> = (server, port).to_socket_addrs()?.collect();
    if resolved_addrs.is_empty() {
        return Err(format!("No socket addresses resolved for {}:{}", server, port).into());
    }

    // Use TCP connect with timeout to avoid hanging on unreachable servers.
    let mut last_error = None;
    let mut connected = None;
    for addr in resolved_addrs {
        match TcpStream::connect_timeout(&addr, Duration::from_secs(15)) {
            Ok(stream) => {
                connected = Some(stream);
                break;
            }
            Err(err) => last_error = Some(err),
        }
    }

    let tcp_stream = connected.ok_or_else(|| {
        last_error
            .map(|e| format!("Failed to connect to {}:{}: {}", server, port, e))
            .unwrap_or_else(|| format!("Failed to connect to {}:{}", server, port))
    })?;
    tcp_stream.set_read_timeout(Some(Duration::from_secs(15)))?;
    tcp_stream.set_write_timeout(Some(Duration::from_secs(15)))?;

    let tls_stream = tls.connect(server, tcp_stream)?;
    let client = imap::Client::new(tls_stream);

    match client.login(email, password) {
        Ok(session) => Ok(session),
        Err((err, _orig_client)) => Err(Box::new(err)),
    }
}

// Handler to authenticate and store Gmail IMAP credentials
pub async fn imap_login(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(payload): Json<ImapCredentials>,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::info!(
        "Received request to /api/auth/gmail/imap/login for user {}",
        auth_user.user_id
    );

    let email = payload.email;
    let password = payload.password;
    let imap_server = payload.imap_server.as_deref(); // Convert Option<String> to Option<&str>
    let imap_port = payload.imap_port;

    // Attempt to connect to Gmail's IMAP server to verify credentials
    match connect_imap(&email, &password, imap_server, imap_port).await {
        Ok(mut session) => {
            // Logout immediately after verification to avoid keeping the session open
            if let Err(e) = session.logout() {
                tracing::warn!("Failed to logout IMAP session: {}", e);
            }

            if let Err(e) = state.user_repository.set_imap_credentials(
                auth_user.user_id,
                &email,
                &password,
                imap_server,
                imap_port,
            ) {
                tracing::error!("Failed to store IMAP credentials: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    AxumJson(json!({"error": "Failed to store IMAP credentials"})),
                ));
            }

            tracing::info!(
                "Successfully stored IMAP credentials for user {}",
                auth_user.user_id
            );
            Ok(AxumJson(json!({"message": "IMAP connected successfully"})))
        }
        Err(e) => {
            tracing::error!(
                "IMAP connection failed for user {}: {}",
                auth_user.user_id,
                e
            );
            Err((
                StatusCode::BAD_REQUEST,
                AxumJson(json!({"error": format!("IMAP connection failed: {}", e)})),
            ))
        }
    }
}

// Handler to check the IMAP connection status (returns all connections)
pub async fn imap_status(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<AxumJson<ImapStatusMulti>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::info!("Checking IMAP status for user {}", auth_user.user_id);

    let connections = state
        .user_repository
        .get_all_imap_credentials(auth_user.user_id)
        .map_err(|e| {
            tracing::error!("Failed to fetch IMAP credentials: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to fetch IMAP status"})),
            )
        })?;

    // Return status based on DB state (no live IMAP testing to avoid N connections per page load)
    let accounts: Vec<ImapAccountStatus> = connections
        .into_iter()
        .map(|c| ImapAccountStatus {
            email: c.email,
            connected: true, // active in DB = connected
        })
        .collect();

    let connected = !accounts.is_empty();
    Ok(Json(ImapStatusMulti {
        connected,
        connections: accounts,
    }))
}

// Handler to delete a specific IMAP connection by email address
pub async fn delete_imap_connection(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(payload): Json<DeleteImapRequest>,
) -> Result<AxumJson<serde_json::Value>, (StatusCode, AxumJson<serde_json::Value>)> {
    tracing::info!(
        "Received request to delete IMAP connection {} for user {}",
        payload.email,
        auth_user.user_id
    );

    if let Err(e) = state
        .user_repository
        .delete_imap_connection_by_email(auth_user.user_id, &payload.email)
    {
        tracing::error!("Failed to delete IMAP credentials: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({"error": "Failed to delete IMAP credentials"})),
        ));
    }

    tracing::info!(
        "Successfully deleted IMAP connection {} for user {}",
        payload.email,
        auth_user.user_id
    );
    Ok(AxumJson(
        json!({"message": "IMAP connection deleted successfully"}),
    ))
}
