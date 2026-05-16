use crate::repositories::user_repository::LogUsageParams;
use crate::AppState;
use crate::UserCoreOps;
use std::error::Error;
use std::sync::Arc;

// ============================================================================
// Admin Alert System
// ============================================================================

/// Severity levels for admin alerts with different cooldown periods
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertSeverity {
    /// Critical issues requiring immediate attention (1 hour cooldown)
    /// Use for: missing credentials, payment failures, security issues
    Critical,
    /// Errors that need attention but aren't emergencies (6 hour cooldown)
    /// Use for: API failures, integration issues, unexpected states
    Error,
    /// Warnings about potential issues (24 hour cooldown)
    /// Use for: rate limits approaching, deprecation notices, performance issues
    Warning,
}

impl AlertSeverity {
    /// Get cooldown period in seconds for this severity level
    pub fn cooldown_seconds(&self) -> i32 {
        match self {
            AlertSeverity::Critical => 3600,     // 1 hour
            AlertSeverity::Error => 6 * 3600,    // 6 hours
            AlertSeverity::Warning => 24 * 3600, // 24 hours
        }
    }

    /// Get display prefix for email subject
    pub fn prefix(&self) -> &'static str {
        match self {
            AlertSeverity::Critical => "[CRITICAL]",
            AlertSeverity::Error => "[ERROR]",
            AlertSeverity::Warning => "[WARNING]",
        }
    }
}

/// Macro for sending admin alerts with automatic location capture.
///
/// # Usage
///
/// ```ignore
/// // Simple alert
/// admin_alert!(state, Critical, "Database connection lost");
///
/// // Alert with context fields
/// admin_alert!(state, Critical, "Twilio credentials missing",
///     message_sid = payload.MessageSid,
///     status = payload.MessageStatus
/// );
///
/// // Warning level
/// admin_alert!(state, Warning, "Rate limit at 80%",
///     current = count,
///     limit = max_requests
/// );
/// ```
///
/// The macro automatically captures file, line, and module path.
/// Alerts are spawned as background tasks and don't block the caller.
#[macro_export]
macro_rules! admin_alert {
    // With context fields
    ($state:expr, $severity:ident, $message:expr, $($key:ident = $value:expr),+ $(,)?) => {{
        let context = vec![
            $(
                (stringify!($key).to_string(), format!("{}", $value)),
            )+
        ];
        $crate::utils::notification_utils::send_admin_alert_internal(
            $state.clone(),
            $crate::utils::notification_utils::AlertSeverity::$severity,
            $message.to_string(),
            Some(context),
            file!(),
            line!(),
            module_path!(),
        );
    }};
    // Without context fields
    ($state:expr, $severity:ident, $message:expr) => {{
        $crate::utils::notification_utils::send_admin_alert_internal(
            $state.clone(),
            $crate::utils::notification_utils::AlertSeverity::$severity,
            $message.to_string(),
            None,
            file!(),
            line!(),
            module_path!(),
        );
    }};
}

/// Internal function called by admin_alert! macro. Do not call directly.
pub fn send_admin_alert_internal(
    state: Arc<AppState>,
    severity: AlertSeverity,
    message: String,
    context: Option<Vec<(String, String)>>,
    file: &'static str,
    line: u32,
    module: &'static str,
) {
    tokio::spawn(async move {
        if let Err(e) =
            send_alert_with_context(&state, severity, &message, context, file, line, module).await
        {
            tracing::error!(
                "Failed to send {} admin alert '{}': {}",
                format!("{:?}", severity).to_lowercase(),
                message,
                e
            );
        }
    });
}

/// Send an admin alert with full context. Called by the macro's spawned task.
async fn send_alert_with_context(
    state: &Arc<AppState>,
    severity: AlertSeverity,
    message: &str,
    context: Option<Vec<(String, String)>>,
    file: &str,
    line: u32,
    module: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let subject = format!("{} {}", severity.prefix(), message);
    let location = format!("{}:{}", file, line);
    let cooldown_seconds = severity.cooldown_seconds();
    let cooldown_hours = cooldown_seconds / 3600;

    // Build full message with context for database storage
    let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC");
    let mut full_message = format!(
        "{} {}\n\nLocation: {}:{}\nModule: {}\nTime: {}\n",
        severity.prefix(),
        message,
        file,
        line,
        module,
        timestamp
    );

    if let Some(ref ctx) = context {
        full_message.push_str("\nContext:\n");
        for (key, value) in ctx {
            full_message.push_str(&format!("  {}: {}\n", key, value));
        }
    }

    // Always log to database first
    if let Err(e) = state.admin_alert_repository.create_alert(
        &subject,
        &format!("{:?}", severity),
        &full_message,
        &location,
        module,
    ) {
        tracing::warn!("Failed to log admin alert to database: {}", e);
    }

    // Check if this alert type is disabled
    match state
        .admin_alert_repository
        .is_alert_type_disabled(&subject)
    {
        Ok(true) => {
            tracing::debug!(
                "Skipping email for admin alert '{}' - alert type is disabled",
                subject
            );
            return Ok(());
        }
        Ok(false) => {}
        Err(e) => {
            tracing::warn!(
                "Failed to check if alert type is disabled: {}, proceeding with send",
                e
            );
        }
    }

    let admin_email =
        std::env::var("ADMIN_ALERT_EMAIL").unwrap_or_else(|_| "rasmus@ahtava.com".to_string());

    if admin_email.is_empty() {
        tracing::warn!("ADMIN_ALERT_EMAIL is empty, skipping alert email");
        return Ok(());
    }

    // Check cooldown
    match state.user_repository.has_recent_notification(
        1,        // Admin user ID
        &subject, // Use full subject as notification type
        cooldown_seconds,
    ) {
        Ok(true) => {
            tracing::debug!(
                "Skipping admin alert email '{}' - still in {}-hour cooldown period",
                subject,
                cooldown_hours
            );
            return Ok(());
        }
        Ok(false) => {}
        Err(e) => {
            tracing::warn!(
                "Failed to check alert cooldown: {}, proceeding with send",
                e
            );
        }
    }

    // Build email body (simpler version without reply-to-disable instructions)
    let mut body = full_message.clone();
    body.push_str(&format!(
        "\n---\nCooldown: {} hours for {:?} alerts.\nManage alerts at: /admin/alerts",
        cooldown_hours, severity
    ));

    // Out-of-band SMS push for Critical alerts. Best-effort: if the notify-server
    // is unreachable or unconfigured, fall through to email-only. The notify-server
    // does its own dedup so a flapping alert source will not spam SMS.
    if severity == AlertSeverity::Critical {
        push_to_notify_server("critical", &subject, &body, &subject).await;
    } else {
        // Rate-spike escalation: if a non-Critical alert type has fired more than
        // SPIKE_THRESHOLD times in the last hour, push one Critical SMS so the
        // admin notices the volume even if individual alerts are only Warning/Error.
        const SPIKE_THRESHOLD: i64 = 5;
        let one_hour_ago = chrono::Utc::now().timestamp() as i32 - 3600;
        if let Ok(count) = state
            .admin_alert_repository
            .count_by_type_since(&subject, one_hour_ago)
        {
            if count >= SPIKE_THRESHOLD {
                let spike_title = format!("RATE SPIKE: {} ({} hits/hr)", subject, count);
                let spike_body = format!(
                    "Alert type '{}' fired {} times in the last hour (threshold {}). \
                     Most recent alert body follows:\n\n{}",
                    subject, count, SPIKE_THRESHOLD, body
                );
                // dedup_key namespaces spike alerts separately from the underlying
                // alert; the notify-server's 1h Critical TTL prevents repeated SMS.
                let spike_dedup = format!("rate-spike:{}", subject);
                push_to_notify_server("critical", &spike_title, &spike_body, &spike_dedup).await;
            }
        }
    }

    // Log the notification to prevent duplicate sends (for cooldown tracking)
    let _ = state.user_repository.log_usage(LogUsageParams {
        user_id: 1,
        sid: None,
        activity_type: subject.clone(),
        credits: None,
        time_consumed: None,
        success: Some(true),
        reason: None,
        status: None,
        recharge_threshold_timestamp: None,
        zero_credits_timestamp: None,
    });

    // Send the email
    let from_with_name = "Lightfriend Alerts <noreply@lightfriend.ai>".to_string();
    let email = resend_rs::types::CreateEmailBaseOptions::new(
        from_with_name,
        [admin_email.as_str()],
        &subject,
    )
    .with_text(&body);

    let resend_api_key = std::env::var("RESEND_API_KEY").map_err(|_| "RESEND_API_KEY not set")?;
    let resend = resend_rs::Resend::new(&resend_api_key);

    resend
        .emails
        .send(email)
        .await
        .map_err(|e| format!("Failed to send alert email: {:?}", e))?;

    tracing::info!(
        "Sent {} admin alert: {}",
        format!("{:?}", severity).to_lowercase(),
        message
    );
    Ok(())
}

/// POST the alert to the out-of-band Hetzner notify-server. The server decides
/// based on severity whether to SMS-push (Critical) or log only. Skipped
/// silently if env vars are not configured. Errors are logged but do not abort
/// the email path.
pub async fn push_to_notify_server(severity: &str, title: &str, body: &str, dedup_key: &str) {
    let url = match std::env::var("NOTIFY_SERVER_URL") {
        Ok(u) if !u.is_empty() => u,
        _ => {
            tracing::debug!("NOTIFY_SERVER_URL not set; skipping notify-server push");
            return;
        }
    };
    let token = match std::env::var("NOTIFY_SERVER_TOKEN") {
        Ok(t) if !t.is_empty() => t,
        _ => {
            tracing::warn!("NOTIFY_SERVER_URL set but NOTIFY_SERVER_TOKEN missing; skipping push");
            return;
        }
    };

    let endpoint = format!("{}/alert", url.trim_end_matches('/'));
    let payload = serde_json::json!({
        "severity": severity,
        "title": title,
        "body": body,
        "dedup_key": dedup_key,
    });

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Failed to build http client for notify-server: {}", e);
            return;
        }
    };

    match client
        .post(&endpoint)
        .bearer_auth(&token)
        .json(&payload)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            tracing::info!("Pushed {} alert to notify-server: {}", severity, title);
        }
        Ok(resp) => {
            tracing::warn!(
                "notify-server returned {} for alert '{}'",
                resp.status(),
                title
            );
        }
        Err(e) => {
            tracing::warn!("Failed to POST to notify-server for '{}': {}", title, e);
        }
    }
}

// ============================================================================
// Legacy Alert Functions (kept for backwards compatibility)
// ============================================================================

/// Sends an email to the admin (rasmus@ahtava.com) with usage statistics
/// for Tinfoil API key renewals. This helps monitor token consumption patterns.
///
/// # Arguments
/// * `state` - The application state
/// * `user_id` - The user ID requesting renewal
/// * `days_until_renewal` - Days remaining until next billing cycle
/// * `tokens_consumed` - Number of tokens consumed since last renewal
///
/// # Returns
/// * `Ok(())` - Email sent successfully
/// * `Err(Box<dyn Error>)` - Error sending email
pub async fn send_tinfoil_renewal_notification(
    state: &Arc<AppState>,
    user_id: i32,
    days_until_renewal: i32,
    tokens_consumed: i64,
) -> Result<(), Box<dyn Error>> {
    use axum::extract::{Json, State as AxumState};

    // Get user details
    let user = state
        .user_core
        .find_by_id(user_id)
        .map_err(|e| format!("Failed to find user: {}", e))?
        .ok_or("User not found")?;

    // Calculate tokens per day
    let days_elapsed = if days_until_renewal >= 30 {
        1 // Prevent division by zero on first renewal
    } else {
        30 - days_until_renewal
    };
    let tokens_per_day = if days_elapsed > 0 {
        tokens_consumed / days_elapsed as i64
    } else {
        tokens_consumed
    };

    // Prepare email body
    let body = format!(
        "Tinfoil API Key Renewal Request\n\
        =====================================\n\n\
        User ID: {}\n\
        User Email: {}\n\
        Days Until Next Billing: {}\n\
        Days Since Last Renewal: {}\n\
        Total Tokens Consumed: {}\n\
        Average Tokens/Day: {}\n\n\
        A new Tinfoil API key has been automatically generated for this user.\n\
        \n\
        Please review these usage statistics to determine if the monthly token limit should be adjusted.\n\
        ",
        user_id,
        user.email,
        days_until_renewal,
        days_elapsed,
        tokens_consumed,
        tokens_per_day
    );

    // Create email request
    let email_request = crate::handlers::imap_handlers::SendEmailRequest {
        to: "rasmus@ahtava.com".to_string(),
        subject: format!("Tinfoil Key Renewal - User {}", user_id),
        body: body.replace("\n", "\r\n"), // CRLF for email
        from: None,
    };

    // Create a fake auth user for sending (admin context)
    let auth_user = crate::handlers::auth_middleware::AuthUser {
        user_id: 1,
        is_admin: true,
    };

    // Send email
    match crate::handlers::imap_handlers::send_email(
        AxumState(state.clone()),
        auth_user,
        Json(email_request),
    )
    .await
    {
        Ok(_) => {
            tracing::info!(
                "Successfully sent Tinfoil renewal notification for user {}",
                user_id
            );
            Ok(())
        }
        Err((status, err)) => {
            let error_msg = format!(
                "Failed to send Tinfoil renewal notification: {:?} - {:?}",
                status, err
            );
            tracing::error!("{}", error_msg);
            Err(error_msg.into())
        }
    }
}

/// Sends a debug email to admin for logging bridge bot messages (no cooldown).
/// This is specifically for debugging WhatsApp bridge disconnection issues.
///
/// # Arguments
/// * `state` - The application state
///
/// Sends an alert email to the admin with a custom subject and message.
/// This is a generic function that can be used anywhere in the codebase
/// to notify the admin of important events, errors, or issues.
///
/// Includes spam protection:
/// - 6-hour cooldown per alert type (based on subject)
/// - Checks if admin has replied to disable future alerts for this type
/// - Stores alert history in usage_logs table with activity_type = 'admin_alert'
///
/// # Arguments
/// * `state` - The application state
/// * `subject` - Email subject line (also used as alert type identifier)
/// * `message` - Email body content
///
/// # Returns
/// * `Ok(())` - Email sent successfully or skipped due to cooldown/reply
/// * `Err(Box<dyn Error>)` - Error sending email
///
/// # Example
/// ```ignore
/// send_admin_alert(
///     &state,
///     "Bridge Connection Failed - WhatsApp",
///     "WhatsApp bridge connection check failed for user 123"
/// ).await?;
/// ```
pub async fn send_admin_alert(
    state: &Arc<AppState>,
    subject: &str,
    message: &str,
) -> Result<(), Box<dyn Error>> {
    use axum::extract::{Json, State as AxumState};

    const COOLDOWN_HOURS: i32 = 6;
    let cooldown_seconds = COOLDOWN_HOURS * 3600;

    // Always log to database first
    if let Err(e) = state.admin_alert_repository.create_alert(
        subject, "Error", // Legacy function uses Error severity by default
        message, "legacy", // No location info for legacy function
        "legacy",
    ) {
        tracing::warn!("Failed to log admin alert to database: {}", e);
    }

    // Check if this alert type is disabled
    match state.admin_alert_repository.is_alert_type_disabled(subject) {
        Ok(true) => {
            tracing::debug!(
                "Skipping email for admin alert '{}' - alert type is disabled",
                subject
            );
            return Ok(());
        }
        Ok(false) => {}
        Err(e) => {
            tracing::warn!(
                "Failed to check if alert type is disabled: {}, proceeding with send",
                e
            );
        }
    }

    // Get the admin alert email from environment variable or default to rasmus@ahtava.com
    let admin_email =
        std::env::var("ADMIN_ALERT_EMAIL").unwrap_or_else(|_| "rasmus@ahtava.com".to_string());

    if admin_email.is_empty() {
        tracing::warn!("ADMIN_ALERT_EMAIL is empty, skipping alert");
        return Ok(());
    }

    // Check cooldown: has this alert type been sent recently?
    match state.user_repository.has_recent_notification(
        1,       // Admin user ID
        subject, // Use subject as the notification type
        cooldown_seconds,
    ) {
        Ok(true) => {
            tracing::debug!(
                "Skipping admin alert '{}' - still in {}-hour cooldown period",
                subject,
                COOLDOWN_HOURS
            );
            return Ok(());
        }
        Ok(false) => {}
        Err(e) => {
            tracing::warn!(
                "Failed to check alert cooldown: {}, proceeding with send",
                e
            );
        }
    }

    // Build message with admin dashboard link instead of reply-to-disable
    let enhanced_message = format!(
        "{}\n\n\
        ---\n\
        Cooldown: {}-hour for this alert type.\n\
        Manage alerts at: /admin/alerts",
        message, COOLDOWN_HOURS
    );

    // Create email request
    let email_request = crate::handlers::imap_handlers::SendEmailRequest {
        to: admin_email.clone(),
        subject: subject.to_string(),
        body: enhanced_message,
        from: None,
    };

    // Create admin auth context
    let auth_user = crate::handlers::auth_middleware::AuthUser {
        user_id: 1,
        is_admin: true,
    };

    // Send email
    match crate::handlers::imap_handlers::send_email(
        AxumState(state.clone()),
        auth_user,
        Json(email_request),
    )
    .await
    {
        Ok(_) => {
            tracing::info!("Successfully sent admin alert email: {}", subject);

            // Log this alert in usage_logs for cooldown tracking
            if let Err(e) = state.user_repository.log_usage(LogUsageParams {
                user_id: 1,
                sid: None,
                activity_type: subject.to_string(),
                credits: None,
                time_consumed: None,
                success: Some(true),
                reason: None,
                status: Some("sent".to_string()),
                recharge_threshold_timestamp: None,
                zero_credits_timestamp: None,
            }) {
                tracing::warn!("Failed to log admin alert for cooldown tracking: {}", e);
            }

            Ok(())
        }
        Err((status, err)) => {
            let error_msg = format!("Failed to send admin alert email: {:?} - {:?}", status, err);
            tracing::error!("{}", error_msg);
            Err(error_msg.into())
        }
    }
}
