use crate::UserCoreOps;
use axum::{extract::State, http::StatusCode, Json};
use diesel::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::models::user_models::{AdminAlert, DisabledAlertType, MessageStatusLog, WaitlistEntry};
use crate::pg_schema::{message_status_log, waitlist};

#[derive(Deserialize)]
pub struct BroadcastMessageRequest {
    pub message: String,
}

#[derive(Deserialize, Clone)]
pub struct EmailBroadcastRequest {
    pub subject: String,
    pub message: String,
    #[serde(default = "default_audience")]
    pub audience: String,
}

fn default_audience() -> String {
    "all".to_string()
}

#[derive(Serialize)]
pub struct UsageLogResponse {
    id: i32,
    user_id: i32,
    activity_type: String,
    timestamp: i32,
    sid: Option<String>,
    status: Option<String>,
    success: Option<bool>,
    credits: Option<f32>,
    time_consumed: Option<i32>,
    reason: Option<String>,
    recharge_threshold_timestamp: Option<i32>,
    zero_credits_timestamp: Option<i32>,
}

use crate::AppState;

pub async fn update_preferred_number_admin(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(user_id): axum::extract::Path<i32>,
    Json(preferred_number): Json<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Get allowed numbers from environment
    let allowed_numbers = [
        std::env::var("USA_PHONE").expect("USA_PHONE must be set in environment"),
        std::env::var("FIN_PHONE").expect("FIN_PHONE must be set in environment"),
        std::env::var("AUS_PHONE").expect("AUS_PHONE must be set in environment"),
        std::env::var("GB_PHONE").expect("GB_PHONE must be set in environment"),
    ];

    // Validate that the preferred number is in the allowed list
    if !allowed_numbers.contains(&preferred_number) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid preferred number"})),
        ));
    }

    // Update the user's preferred number
    state
        .user_core
        .update_preferred_number(user_id, &preferred_number)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?;

    Ok(Json(json!({
        "message": "Preferred number updated successfully"
    })))
}

#[derive(Debug, Deserialize)]
pub struct UnsubscribeParams {
    pub email: String,
}

use axum::extract::Query;
use axum::response::Html;

pub async fn unsubscribe(
    State(state): State<Arc<AppState>>,
    Query(params): Query<UnsubscribeParams>,
) -> Result<Html<String>, (StatusCode, String)> {
    tracing::info!(
        "Unsubscribe request received for raw email param: {}",
        params.email
    );

    // First try to find a registered user
    match state.user_core.find_by_email(&params.email) {
        Ok(Some(user)) => {
            tracing::info!("Found user {} for email: {}", user.id, params.email);
            match state.user_core.update_notify(user.id, false) {
                Ok(_) => {
                    tracing::info!("User {} unsubscribed from notifications", user.id);
                    Ok(Html("<h1>You have been unsubscribed!</h1>".to_string()))
                }
                Err(e) => {
                    tracing::error!("Failed to update notify for user {}: {}", user.id, e);
                    Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to unsubscribe. Sorry about this, send email to rasmus@lightfriend.ai"
                            .to_string(),
                    ))
                }
            }
        }
        Ok(None) => {
            // No registered user found, check waitlist
            tracing::info!(
                "No registered user found for email: {}, checking waitlist",
                params.email
            );

            let mut pg_conn = state.pg_pool.get().map_err(|e| {
                tracing::error!("Failed to get PG connection: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Database error".to_string(),
                )
            })?;

            // Try to delete from waitlist
            let deleted = diesel::delete(
                waitlist::table.filter(waitlist::email.eq(&params.email.to_lowercase())),
            )
            .execute(&mut pg_conn)
            .map_err(|e| {
                tracing::error!("Failed to delete from waitlist: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to process request.".to_string(),
                )
            })?;

            if deleted > 0 {
                tracing::info!("Removed {} from waitlist", params.email);
                Ok(Html("<h1>You have been unsubscribed!</h1>".to_string()))
            } else {
                tracing::warn!(
                    "No user or waitlist entry found for email: {}",
                    params.email
                );
                Err((StatusCode::BAD_REQUEST, "Invalid email.".to_string()))
            }
        }
        Err(e) => {
            tracing::error!("Failed to find user by email {}: {}", params.email, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to process request.".to_string(),
            ))
        }
    }
}

pub async fn broadcast_email(
    State(state): State<Arc<AppState>>,
    Json(request): Json<EmailBroadcastRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Validate input
    if request.subject.is_empty() || request.message.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Subject and message cannot be empty"})),
        ));
    }

    // Fetch users outside the spawn to avoid DB issues, then move into task
    let users = state.user_core.get_all_users().map_err(|e| {
        tracing::error!("Database error when fetching users: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)})),
        )
    })?;

    // Fetch waitlist entries (updates-only subscribers who haven't signed up yet)
    let waitlist_entries: Vec<WaitlistEntry> = {
        let mut pg_conn = state.pg_pool.get().map_err(|e| {
            tracing::error!("Failed to get PG connection for waitlist: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Database error"})),
            )
        })?;
        waitlist::table
            .select(WaitlistEntry::as_select())
            .load(&mut pg_conn)
            .map_err(|e| {
                tracing::error!("Failed to fetch waitlist entries: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Database error: {}", e)})),
                )
            })?
    };

    // Clone what we need for the task
    let state_clone = state.clone();
    let request_clone = request.clone();

    // Spawn the background task
    tokio::spawn(async move {
        let mut success_count = 0;
        let mut failed_count = 0;
        let mut error_details = Vec::new();

        // Collect registered user emails to avoid sending duplicates to waitlist entries
        let mut sent_emails: std::collections::HashSet<String> = std::collections::HashSet::new();

        // Send to registered users with notify enabled
        for user in users {
            let user_settings = match state_clone.user_core.get_user_settings(user.id) {
                Ok(settings) => settings,
                Err(e) => {
                    tracing::error!("Failed to get settings for user {}: {}", user.id, e);
                    failed_count += 1;
                    error_details.push(format!("Failed to get settings for {}: {}", user.email, e));
                    continue;
                }
            };

            if !user_settings.notify {
                tracing::info!("skipping user since they don't have notify on");
                // Still track email to avoid duplicate from waitlist
                sent_emails.insert(user.email.to_lowercase());
                continue;
            }

            // Audience filter
            let has_sub = user.sub_tier.is_some();
            match request_clone.audience.as_str() {
                "only_subs" if !has_sub => {
                    sent_emails.insert(user.email.to_lowercase());
                    continue;
                }
                "only_non_subs" if has_sub => {
                    sent_emails.insert(user.email.to_lowercase());
                    continue;
                }
                _ => {} // "all" or matched filter
            }

            // Skip users with invalid or empty email addresses
            if user.email.is_empty() || !user.email.contains('@') || !user.email.contains('.') {
                tracing::warn!("Skipping invalid email address: {}", user.email);
                continue;
            }

            // Track this email as sent
            sent_emails.insert(user.email.to_lowercase());

            // Prepare the unsubscribe link
            let encoded_email = urlencoding::encode(&user.email);
            let server_url = std::env::var("SERVER_URL").expect("SERVER_URL not set");
            let unsubscribe_link =
                format!("{}/api/unsubscribe?email={}", server_url, encoded_email);

            // Convert message newlines to HTML paragraphs
            let html_message = request_clone
                .message
                .split("\n\n")
                .map(|p| format!("<p>{}</p>", p.replace('\n', "<br>")))
                .collect::<Vec<_>>()
                .join("\n");

            // Prepare HTML body with Lightfriend branding
            let html_body = format!(
                r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
</head>
<body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; line-height: 1.6; color: #333; max-width: 600px; margin: 0 auto; padding: 20px; background-color: #fafafa;">
    <!-- Main Content -->
    <div style="background-color: white; border-radius: 8px; padding: 30px; box-shadow: 0 1px 3px rgba(0,0,0,0.1);">
        {}

        <p style="margin-top: 30px; font-size: 14px; color: #666;">Have questions or feature requests? Just reply to this email - I'd love to hear from you!</p>

        <p style="margin-top: 20px;">-Rasmus from Lightfriend</p>
    </div>

    <!-- Footer -->
    <div style="text-align: center; padding: 20px 0; margin-top: 20px;">
        <p style="font-size: 12px; color: #888; margin: 0;">
            <a href="https://lightfriend.ai" style="color: #7EB2FF; text-decoration: none;">lightfriend.ai</a>
        </p>
        <p style="margin-top: 15px; font-size: 12px; color: #999;">
            <a href="{}" style="color: #999;">Unsubscribe from feature updates</a>
        </p>
    </div>
</body>
</html>"#,
                html_message, unsubscribe_link
            );

            // Send via Resend
            match crate::utils::email::send_broadcast_email(
                &user.email,
                &request_clone.subject,
                &html_body,
            )
            .await
            {
                Ok(_) => {
                    success_count += 1;
                    tracing::info!("Successfully sent email to {}", user.email);
                }
                Err(e) => {
                    failed_count += 1;
                    let error_msg = format!("Failed to send to {}: {}", user.email, e);
                    tracing::error!("{}", error_msg);
                    error_details.push(error_msg);
                }
            }

            // Add a small delay to avoid hitting rate limits
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        // Send to waitlist entries (updates-only subscribers)
        // Waitlist = non-subscribers, skip when targeting subscribers only
        if request_clone.audience == "only_subs" {
            tracing::info!("Skipping waitlist entries for subscribers-only broadcast");
        }
        for entry in waitlist_entries {
            if request_clone.audience == "only_subs" {
                break;
            }
            // Skip if already sent to this email (user is both registered and on waitlist)
            if sent_emails.contains(&entry.email.to_lowercase()) {
                tracing::info!(
                    "Skipping waitlist entry {} - already sent as registered user",
                    entry.email
                );
                continue;
            }

            // Skip invalid email addresses
            if entry.email.is_empty() || !entry.email.contains('@') || !entry.email.contains('.') {
                tracing::warn!("Skipping invalid waitlist email address: {}", entry.email);
                continue;
            }

            // Prepare the unsubscribe link
            let encoded_email = urlencoding::encode(&entry.email);
            let server_url = std::env::var("SERVER_URL").expect("SERVER_URL not set");
            let unsubscribe_link =
                format!("{}/api/unsubscribe?email={}", server_url, encoded_email);

            // Convert message newlines to HTML paragraphs
            let html_message = request_clone
                .message
                .split("\n\n")
                .map(|p| format!("<p>{}</p>", p.replace('\n', "<br>")))
                .collect::<Vec<_>>()
                .join("\n");

            // Prepare HTML body with Lightfriend branding
            let html_body = format!(
                r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
</head>
<body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; line-height: 1.6; color: #333; max-width: 600px; margin: 0 auto; padding: 20px; background-color: #fafafa;">
    <!-- Main Content -->
    <div style="background-color: white; border-radius: 8px; padding: 30px; box-shadow: 0 1px 3px rgba(0,0,0,0.1);">
        {}

        <p style="margin-top: 30px; font-size: 14px; color: #666;">Have questions or feature requests? Just reply to this email - I'd love to hear from you!</p>

        <p style="margin-top: 20px;">-Rasmus from Lightfriend</p>
    </div>

    <!-- Footer -->
    <div style="text-align: center; padding: 20px 0; margin-top: 20px;">
        <p style="font-size: 12px; color: #888; margin: 0;">
            <a href="https://lightfriend.ai" style="color: #7EB2FF; text-decoration: none;">lightfriend.ai</a>
        </p>
        <p style="margin-top: 15px; font-size: 12px; color: #999;">
            <a href="{}" style="color: #999;">Unsubscribe from feature updates</a>
        </p>
    </div>
</body>
</html>"#,
                html_message, unsubscribe_link
            );

            // Send via Resend
            match crate::utils::email::send_broadcast_email(
                &entry.email,
                &request_clone.subject,
                &html_body,
            )
            .await
            {
                Ok(_) => {
                    success_count += 1;
                    tracing::info!("Successfully sent email to waitlist entry {}", entry.email);
                }
                Err(e) => {
                    failed_count += 1;
                    let error_msg = format!("Failed to send to waitlist {}: {}", entry.email, e);
                    tracing::error!("{}", error_msg);
                    error_details.push(error_msg);
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        // Log final stats since we can't return them
        tracing::info!(
            "Email broadcast completed: success={}, failed={}, errors={:?}",
            success_count,
            failed_count,
            error_details
        );
    });

    // Respond immediately
    Ok(Json(json!({
        "message": "Email broadcast queued and will process in the background"
    })))
}

pub async fn update_monthly_credits(
    State(state): State<Arc<AppState>>,
    axum::extract::Path((user_id, amount)): axum::extract::Path<(f32, f32)>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Get current user
    let user = state
        .user_core
        .find_by_id(user_id as i32)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "User not found"})),
            )
        })?;

    // Calculate new credits count, ensuring it doesn't go below 0
    let new_credits = (user.credits_left + amount).max(0.0);

    // Update credits count
    state
        .user_repository
        .update_user_credits_left(user.id, new_credits)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to update monthly credits: {}", e)})),
            )
        })?;

    Ok(Json(json!({
        "message": "Monthly credits updated successfully",
        "new_count": new_credits
    })))
}

pub async fn update_subscription_tier(
    State(state): State<Arc<AppState>>,
    axum::extract::Path((user_id, tier)): axum::extract::Path<(i32, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let tier = if tier == "tier 0" {
        None
    } else {
        Some(tier.as_str())
    };

    // Update the subscription tier
    state
        .user_repository
        .set_subscription_tier(user_id, tier)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?;
    tracing::info!("subscription tier set successfully");

    Ok(Json(json!({
        "message": "Subscription tier updated successfully"
    })))
}

/// Update a user's plan type (monitor, digest, byot, or none)
pub async fn update_plan_type(
    State(state): State<Arc<AppState>>,
    axum::extract::Path((user_id, plan_type)): axum::extract::Path<(i32, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let plan_type = match plan_type.as_str() {
        "none" => None,
        other => Some(other),
    };

    // Update the plan type
    state
        .user_repository
        .update_plan_type(user_id, plan_type)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?;
    tracing::info!("plan type updated to {:?} for user {}", plan_type, user_id);

    Ok(Json(json!({
        "message": "Plan type updated successfully"
    })))
}

pub async fn get_usage_logs(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<UsageLogResponse>>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("getting usage logs");
    // Get all usage logs from the database
    let logs = state.user_repository.get_all_usage_logs().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)})),
        )
    })?;

    // Transform the logs into the response format
    let response_logs: Vec<UsageLogResponse> = logs
        .into_iter()
        .map(|log| UsageLogResponse {
            id: log.id,
            user_id: log.user_id,
            activity_type: log.activity_type,
            timestamp: log.created_at,
            sid: log.sid,
            status: log.status,
            success: log.success,
            credits: log.credits,
            time_consumed: log.time_consumed,
            reason: log.reason,
            recharge_threshold_timestamp: log.recharge_threshold_timestamp,
            zero_credits_timestamp: log.zero_credits_timestamp,
        })
        .collect();

    tracing::info!("returning response_logs");
    Ok(Json(response_logs))
}

/// Admin-only endpoint to generate and send a password reset link to a user.
///
/// This generates a secure one-time token, stores it with 24-hour expiry,
/// and sends the reset link to the user's email.
pub async fn send_password_reset_link(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(user_id): axum::extract::Path<i32>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Look up user by ID to get their email
    let user = state
        .user_core
        .find_by_id(user_id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "User not found"})),
            )
        })?;

    // Generate cryptographically secure token (32 alphanumeric chars)
    let token: String = rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();

    // Set expiry to 24 hours from now
    let expiry = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
        + (24 * 60 * 60); // 24 hours

    // Store the token with user_id and expiry
    state
        .pending_password_resets
        .insert(token.clone(), (user_id, expiry));

    // Build reset URL
    let frontend_url =
        std::env::var("FRONTEND_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
    let reset_link = format!("{}/password-reset/{}", frontend_url, token);

    let email = user.email.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::utils::email::send_password_reset_email(&email, &reset_link).await {
            tracing::error!("Failed to send password reset email to {}: {}", email, e);
        }
    });

    tracing::info!(
        "Password reset link sent to user {} ({})",
        user_id,
        user.email
    );

    Ok(Json(json!({
        "message": format!("Password reset link sent to {}", user.email),
        "email": user.email
    })))
}

#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    pub new_password: String,
}

/// Change admin's own password
/// POST /api/admin/change-password
pub async fn change_admin_password(
    State(state): State<Arc<AppState>>,
    auth_user: crate::handlers::auth_middleware::AuthUser,
    Json(req): Json<ChangePasswordRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Validate password length
    if req.new_password.len() < 6 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Password must be at least 6 characters"})),
        ));
    }

    // Hash the new password
    let password_hash = bcrypt::hash(&req.new_password, bcrypt::DEFAULT_COST).map_err(|e| {
        tracing::error!("Failed to hash password: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to hash password"})),
        )
    })?;

    // Update the password
    state
        .user_core
        .update_password(auth_user.user_id, &password_hash)
        .map_err(|e| {
            tracing::error!(
                "Failed to update password for admin {}: {}",
                auth_user.user_id,
                e
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to update password"})),
            )
        })?;

    tracing::info!("Admin {} changed their password", auth_user.user_id);

    Ok(Json(json!({
        "message": "Password updated successfully"
    })))
}

#[derive(Deserialize)]
pub struct SetTwilioCredsRequest {
    pub user_id: i32,
    pub account_sid: String,
    pub auth_token: String,
}

pub async fn set_user_twilio_credentials(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetTwilioCredsRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    state
        .user_repository
        .update_twilio_credentials(req.user_id, &req.account_sid, &req.auth_token)
        .map_err(|e| {
            tracing::error!("Failed to set twilio creds: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": e.to_string()})),
            )
        })?;

    tracing::info!("Set Twilio credentials for user {}", req.user_id);
    Ok(Json(json!({"success": true})))
}

/// Response for message stats endpoint
#[derive(Serialize)]
pub struct MessageStatsResponse {
    pub user_id: i32,
    pub total_messages: i64,
    pub delivered: i64,
    pub failed: i64,
    pub undelivered: i64,
    pub queued: i64,
    pub sent: i64,
    pub recent_messages: Vec<MessageStatusLog>,
}

/// Get message delivery stats for a user
/// GET /api/admin/users/:id/message-stats
pub async fn get_user_message_stats(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(user_id): axum::extract::Path<i32>,
) -> Result<Json<MessageStatsResponse>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Getting message stats for user_id={}", user_id);

    let pg_conn = &mut state.pg_pool.get().map_err(|e| {
        tracing::error!("Failed to get PG connection: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Database connection error"})),
        )
    })?;

    // Get recent messages (last 50)
    let recent_messages: Vec<MessageStatusLog> = message_status_log::table
        .filter(message_status_log::user_id.eq(user_id))
        .order(message_status_log::created_at.desc())
        .limit(50)
        .load(pg_conn)
        .map_err(|e| {
            tracing::error!("Failed to get message stats: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to get message stats"})),
            )
        })?;

    tracing::info!(
        "Found {} messages for user_id={}",
        recent_messages.len(),
        user_id
    );
    for msg in &recent_messages {
        tracing::info!(
            "  Message: sid={}, status={}, to={}",
            msg.message_sid,
            msg.status,
            msg.to_number
        );
    }

    // Count by status
    let total_messages = recent_messages.len() as i64;
    let delivered = recent_messages
        .iter()
        .filter(|m| m.status == "delivered")
        .count() as i64;
    let failed = recent_messages
        .iter()
        .filter(|m| m.status == "failed")
        .count() as i64;
    let undelivered = recent_messages
        .iter()
        .filter(|m| m.status == "undelivered")
        .count() as i64;
    let queued = recent_messages
        .iter()
        .filter(|m| m.status == "queued")
        .count() as i64;
    let sent = recent_messages
        .iter()
        .filter(|m| m.status == "sent")
        .count() as i64;

    tracing::info!(
        "Stats: total={}, delivered={}, failed={}, undelivered={}, queued={}, sent={}",
        total_messages,
        delivered,
        failed,
        undelivered,
        queued,
        sent
    );

    Ok(Json(MessageStatsResponse {
        user_id,
        total_messages,
        delivered,
        failed,
        undelivered,
        queued,
        sent,
        recent_messages,
    }))
}

/// Message status log with user info for global stats
#[derive(Serialize)]
pub struct MessageStatusLogWithUser {
    pub id: i32,
    pub message_sid: String,
    pub user_id: i32,
    pub user_email: Option<String>,
    pub user_phone: Option<String>,
    pub direction: String,
    pub to_number: String,
    pub from_number: Option<String>,
    pub status: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub price: Option<f32>,
    pub price_unit: Option<String>,
    pub created_at: i32,
    pub updated_at: i32,
}

/// Response for global message stats endpoint
#[derive(Serialize)]
pub struct GlobalMessageStatsResponse {
    pub total_messages: i64,
    pub delivered: i64,
    pub failed: i64,
    pub undelivered: i64,
    pub queued: i64,
    pub sent: i64,
    pub recent_failed: Vec<MessageStatusLogWithUser>,
}

/// Get global message delivery stats across all users
/// GET /api/admin/global-message-stats
pub async fn get_global_message_stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<GlobalMessageStatsResponse>, (StatusCode, Json<serde_json::Value>)> {
    use crate::pg_schema::users;

    let pg_conn = &mut state.pg_pool.get().map_err(|e| {
        tracing::error!("Failed to get PG connection: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Database connection error"})),
        )
    })?;

    // Get all messages (last 1000 for stats)
    let all_messages: Vec<MessageStatusLog> = message_status_log::table
        .order(message_status_log::created_at.desc())
        .limit(1000)
        .load(pg_conn)
        .map_err(|e| {
            tracing::error!("Failed to get global message stats: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to get message stats"})),
            )
        })?;

    // Count by status
    let total_messages = all_messages.len() as i64;
    let delivered = all_messages
        .iter()
        .filter(|m| m.status == "delivered")
        .count() as i64;
    let failed = all_messages.iter().filter(|m| m.status == "failed").count() as i64;
    let undelivered = all_messages
        .iter()
        .filter(|m| m.status == "undelivered")
        .count() as i64;
    let queued = all_messages.iter().filter(|m| m.status == "queued").count() as i64;
    let sent = all_messages.iter().filter(|m| m.status == "sent").count() as i64;

    // Get recent failed/undelivered messages with user info (last 20)
    let failed_messages: Vec<MessageStatusLog> = all_messages
        .iter()
        .filter(|m| m.status == "failed" || m.status == "undelivered")
        .take(20)
        .cloned()
        .collect();

    // Get user info for failed messages
    let user_ids: Vec<i32> = failed_messages.iter().map(|m| m.user_id).collect();
    let users_info: Vec<(i32, String, String)> = users::table
        .filter(users::id.eq_any(&user_ids))
        .select((users::id, users::email, users::phone_number))
        .load(pg_conn)
        .unwrap_or_default();

    let users_map: std::collections::HashMap<i32, (String, String)> = users_info
        .into_iter()
        .map(|(id, email, phone)| (id, (email, phone)))
        .collect();

    let recent_failed: Vec<MessageStatusLogWithUser> = failed_messages
        .into_iter()
        .map(|m| {
            let (user_email, user_phone) = users_map
                .get(&m.user_id)
                .map(|(e, p)| (Some(e.clone()), Some(p.clone())))
                .unwrap_or((None, None));

            MessageStatusLogWithUser {
                id: m.id,
                message_sid: m.message_sid,
                user_id: m.user_id,
                user_email,
                user_phone,
                direction: m.direction,
                to_number: m.to_number,
                from_number: m.from_number,
                status: m.status,
                error_code: m.error_code,
                error_message: m.error_message,
                price: m.price,
                price_unit: m.price_unit,
                created_at: m.created_at,
                updated_at: m.updated_at,
            }
        })
        .collect();

    tracing::info!(
        "Global stats: total={}, delivered={}, failed={}, undelivered={}, queued={}, sent={}",
        total_messages,
        delivered,
        failed,
        undelivered,
        queued,
        sent
    );

    Ok(Json(GlobalMessageStatsResponse {
        total_messages,
        delivered,
        failed,
        undelivered,
        queued,
        sent,
        recent_failed,
    }))
}

// ============================================================================
// Admin Alert Management Endpoints
// ============================================================================

/// Query params for listing alerts
#[derive(Deserialize)]
pub struct AlertsQueryParams {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub severity: Option<String>,
}

/// Response for listing alerts
#[derive(Serialize)]
pub struct AlertsResponse {
    pub alerts: Vec<AdminAlert>,
    pub total: i64,
    pub unacknowledged_count: i64,
}

/// Response for unacknowledged count
#[derive(Serialize)]
pub struct AlertCountResponse {
    pub count: i64,
}

/// Response for disabled types
#[derive(Serialize)]
pub struct DisabledTypesResponse {
    pub disabled_types: Vec<DisabledAlertType>,
}

/// Get paginated list of alerts
/// GET /api/admin/alerts
pub async fn get_alerts(
    State(state): State<Arc<AppState>>,
    Query(params): Query<AlertsQueryParams>,
) -> Result<Json<AlertsResponse>, (StatusCode, Json<serde_json::Value>)> {
    let limit = params.limit.unwrap_or(50);
    let offset = params.offset.unwrap_or(0);
    let severity_filter = params.severity.as_deref();

    let alerts = state
        .admin_alert_repository
        .get_alerts(limit, offset, severity_filter)
        .map_err(|e| {
            tracing::error!("Failed to get alerts: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to get alerts"})),
            )
        })?;

    let total = state
        .admin_alert_repository
        .get_total_count(severity_filter)
        .map_err(|e| {
            tracing::error!("Failed to get total alert count: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to get alert count"})),
            )
        })?;

    let unacknowledged_count = state
        .admin_alert_repository
        .get_unacknowledged_count()
        .map_err(|e| {
            tracing::error!("Failed to get unacknowledged count: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to get unacknowledged count"})),
            )
        })?;

    Ok(Json(AlertsResponse {
        alerts,
        total,
        unacknowledged_count,
    }))
}

/// Get unacknowledged alert count
/// GET /api/admin/alerts/count
pub async fn get_alert_count(
    State(state): State<Arc<AppState>>,
) -> Result<Json<AlertCountResponse>, (StatusCode, Json<serde_json::Value>)> {
    let count = state
        .admin_alert_repository
        .get_unacknowledged_count()
        .map_err(|e| {
            tracing::error!("Failed to get alert count: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to get alert count"})),
            )
        })?;

    Ok(Json(AlertCountResponse { count }))
}

/// Acknowledge a single alert
/// POST /api/admin/alerts/:id/acknowledge
pub async fn acknowledge_alert(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(alert_id): axum::extract::Path<i32>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    state
        .admin_alert_repository
        .acknowledge_alert(alert_id)
        .map_err(|e| {
            tracing::error!("Failed to acknowledge alert {}: {}", alert_id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to acknowledge alert"})),
            )
        })?;

    Ok(Json(json!({"message": "Alert acknowledged"})))
}

/// Acknowledge all alerts
/// POST /api/admin/alerts/acknowledge-all
pub async fn acknowledge_all_alerts(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let count = state
        .admin_alert_repository
        .acknowledge_all()
        .map_err(|e| {
            tracing::error!("Failed to acknowledge all alerts: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to acknowledge alerts"})),
            )
        })?;

    Ok(Json(json!({
        "message": "All alerts acknowledged",
        "count": count
    })))
}

/// Get list of disabled alert types
/// GET /api/admin/alerts/disabled-types
pub async fn get_disabled_alert_types(
    State(state): State<Arc<AppState>>,
) -> Result<Json<DisabledTypesResponse>, (StatusCode, Json<serde_json::Value>)> {
    let disabled_types = state
        .admin_alert_repository
        .get_disabled_types()
        .map_err(|e| {
            tracing::error!("Failed to get disabled types: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to get disabled types"})),
            )
        })?;

    Ok(Json(DisabledTypesResponse { disabled_types }))
}

/// Disable an alert type
/// POST /api/admin/alerts/disable/:alert_type
pub async fn disable_alert_type(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(alert_type): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // URL decode the alert_type since it may contain special characters
    let alert_type = urlencoding::decode(&alert_type)
        .map_err(|e| {
            tracing::error!("Failed to decode alert type: {}", e);
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Invalid alert type encoding"})),
            )
        })?
        .into_owned();

    state
        .admin_alert_repository
        .disable_alert_type(&alert_type)
        .map_err(|e| {
            tracing::error!("Failed to disable alert type '{}': {}", alert_type, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to disable alert type"})),
            )
        })?;

    tracing::info!("Alert type disabled: {}", alert_type);
    Ok(Json(json!({
        "message": "Alert type disabled",
        "alert_type": alert_type
    })))
}

/// Enable an alert type (remove from disabled list)
/// POST /api/admin/alerts/enable/:alert_type
pub async fn enable_alert_type(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(alert_type): axum::extract::Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // URL decode the alert_type since it may contain special characters
    let alert_type = urlencoding::decode(&alert_type)
        .map_err(|e| {
            tracing::error!("Failed to decode alert type: {}", e);
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Invalid alert type encoding"})),
            )
        })?
        .into_owned();

    state
        .admin_alert_repository
        .enable_alert_type(&alert_type)
        .map_err(|e| {
            tracing::error!("Failed to enable alert type '{}': {}", alert_type, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to enable alert type"})),
            )
        })?;

    tracing::info!("Alert type enabled: {}", alert_type);
    Ok(Json(json!({
        "message": "Alert type enabled",
        "alert_type": alert_type
    })))
}

/// Nuclear disaster recovery: rebuild user accounts from Resend contacts + Stripe.
/// SAFETY: Refuses to run if the database already has users. Only works on an empty database.
/// After creating accounts, sends each user a password reset link so they can log in.
/// User ID 1 is always rasmus@ahtava.com (admin).
///
/// Only accessible via /api/internal/recover-users with X-Maintenance-Secret header.
/// Triggered by the disaster-recovery GitHub Actions workflow for nuclear recovery.
pub async fn recover_users_from_external(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    if !crate::handlers::maintenance_handlers::check_secret(&headers) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Invalid recovery secret"})),
        ));
    }
    // HARD GUARD: refuse to run if database has any users
    let existing_users = state.user_core.get_all_users().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Database error: {}", e) })),
        )
    })?;
    if !existing_users.is_empty() {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({
                "error": "REFUSED: database has existing users. Recovery only works on an empty database.",
                "user_count": existing_users.len()
            })),
        ));
    }

    // Step 1: Fetch emails from Resend contacts
    let contacts = crate::utils::resend_contacts::list_all_contacts()
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to fetch Resend contacts: {}", e) })),
            )
        })?;

    if contacts.is_empty() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "No contacts found in Resend" })),
        ));
    }

    // Step 2: Fetch customers from Stripe
    let stripe_key = std::env::var("STRIPE_SECRET_KEY").map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "STRIPE_SECRET_KEY not set" })),
        )
    })?;

    let http_client = reqwest::Client::new();
    // Stripe customer data: email -> (phone, customer_id, has_active_sub, price_id)
    struct StripeInfo {
        phone: Option<String>,
        customer_id: String,
        has_active_sub: bool,
        price_id: Option<String>,
    }
    let mut stripe_customers: std::collections::HashMap<String, StripeInfo> =
        std::collections::HashMap::new();

    // Paginate through all Stripe customers
    let mut starting_after: Option<String> = None;
    loop {
        let mut url =
            "https://api.stripe.com/v1/customers?limit=100&expand[]=data.subscriptions".to_string();
        if let Some(ref cursor) = starting_after {
            url.push_str(&format!("&starting_after={}", cursor));
        }

        let resp = http_client
            .get(&url)
            .header("Authorization", format!("Bearer {}", stripe_key))
            .send()
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": format!("Stripe API error: {}", e) })),
                )
            })?;

        let body: serde_json::Value = resp.json().await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to parse Stripe response: {}", e) })),
            )
        })?;

        let empty = vec![];
        let data = body["data"].as_array().unwrap_or(&empty);
        if data.is_empty() {
            break;
        }

        for customer in data {
            let email = customer["email"].as_str().unwrap_or("").to_lowercase();
            let phone = customer["phone"].as_str().map(|s| s.to_string());
            let customer_id = customer["id"].as_str().unwrap_or("").to_string();

            // Check for active subscription and get price ID
            let mut has_active_sub = false;
            let mut price_id = None;
            if let Some(subs) = customer["subscriptions"]["data"].as_array() {
                for sub in subs {
                    let status = sub["status"].as_str().unwrap_or("");
                    if status == "active" || status == "trialing" {
                        has_active_sub = true;
                        // Get the price ID from the first subscription item
                        if let Some(items) = sub["items"]["data"].as_array() {
                            if let Some(item) = items.first() {
                                price_id = item["price"]["id"].as_str().map(|s| s.to_string());
                            }
                        }
                        break;
                    }
                }
            }

            if !email.is_empty() {
                stripe_customers.insert(
                    email,
                    StripeInfo {
                        phone,
                        customer_id,
                        has_active_sub,
                        price_id,
                    },
                );
            }
            starting_after = customer["id"].as_str().map(|s| s.to_string());
        }

        if !body["has_more"].as_bool().unwrap_or(false) {
            break;
        }
    }

    tracing::info!(
        "Recovery: found {} Resend contacts, {} Stripe customers",
        contacts.len(),
        stripe_customers.len()
    );

    // Step 3: Create users - admin first, then everyone else
    let admin_email = "rasmus@ahtava.com";
    let mut created = 0;
    let mut skipped = 0;
    let mut errors = Vec::new();

    // Collect all emails: Resend contacts + Stripe customers (union)
    let mut all_emails: Vec<String> = contacts.iter().map(|c| c.email.to_lowercase()).collect();
    for email in stripe_customers.keys() {
        if !all_emails.contains(email) {
            all_emails.push(email.clone());
        }
    }

    // Sort so admin email comes first
    all_emails.sort_by(|a, b| {
        if a == admin_email {
            std::cmp::Ordering::Less
        } else if b == admin_email {
            std::cmp::Ordering::Greater
        } else {
            a.cmp(b)
        }
    });

    for email in &all_emails {
        let stripe_info = stripe_customers.get(email);
        let phone = stripe_info
            .and_then(|s| s.phone.clone())
            .unwrap_or_default();
        let has_active_sub = stripe_info.map(|s| s.has_active_sub).unwrap_or(false);

        // Generate a random temporary password (user will reset via link)
        let temp_password: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();
        let password_hash = match bcrypt::hash(&temp_password, bcrypt::DEFAULT_COST) {
            Ok(h) => h,
            Err(e) => {
                errors.push(format!("{}: hash failed: {}", email, e));
                continue;
            }
        };

        // Set sub_tier based on Stripe subscription status
        let sub_tier = if has_active_sub {
            Some("tier 2".to_string())
        } else {
            None
        };

        let new_user = crate::handlers::auth_dtos::NewUser {
            email: email.clone(),
            password_hash,
            phone_number: phone,
            time_to_live: 0, // No TTL - permanent user
            credits: 0.0,
            credits_left: 0.0,
            charge_when_under: false,
            sub_tier,
        };

        match state.user_core.create_user(new_user) {
            Ok(_) => {
                created += 1;
                tracing::info!("Recovery: created user {}", email);

                // Set plan_type and stripe_customer_id on the newly created user
                if let Some(info) = stripe_info {
                    if let Ok(Some(user)) = state.user_core.find_by_email(email) {
                        // Set stripe_customer_id so Stripe webhooks reconnect
                        let _ = state
                            .user_repository
                            .set_stripe_customer_id(user.id, &info.customer_id);

                        // Determine plan_type from price ID
                        if let Some(ref price_id) = info.price_id {
                            use crate::utils::country::{
                                is_assistant_plan_price, is_byot_plan_price,
                            };
                            let plan_type = if is_assistant_plan_price(price_id) {
                                "assistant"
                            } else if is_byot_plan_price(price_id) {
                                "byot"
                            } else {
                                "autopilot"
                            };
                            let _ = state
                                .user_repository
                                .update_plan_type(user.id, Some(plan_type));
                        }

                        // Set credits for active subscribers
                        if info.has_active_sub {
                            use crate::utils::country::is_byot_plan_price;
                            use crate::utils::plan_features::MONTHLY_CREDIT_BUDGET;
                            let is_byot = info
                                .price_id
                                .as_deref()
                                .map(is_byot_plan_price)
                                .unwrap_or(false);
                            let credits = if is_byot { 0.0 } else { MONTHLY_CREDIT_BUDGET };
                            let _ = state.user_repository.update_user_credits(user.id, credits);
                            let _ = state
                                .user_repository
                                .update_user_credits_left(user.id, credits);
                        }
                    }
                }
            }
            Err(e) => {
                errors.push(format!("{}: {}", email, e));
                skipped += 1;
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
    }

    // Step 4: Send password reset links to all created users
    let all_users = state.user_core.get_all_users().unwrap_or_default();
    let mut reset_sent = 0;
    let frontend_url =
        std::env::var("FRONTEND_URL").unwrap_or_else(|_| "https://lightfriend.ai".to_string());

    for user in &all_users {
        let token: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();

        let expiry = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            + (7 * 24 * 60 * 60); // 7 days for recovery

        state
            .pending_password_resets
            .insert(token.clone(), (user.id, expiry));

        let reset_link = format!("{}/password-reset/{}", frontend_url, token);
        let email = user.email.clone();
        tokio::spawn(async move {
            if let Err(e) =
                crate::utils::email::send_password_reset_email(&email, &reset_link).await
            {
                tracing::error!("Failed to send recovery email to {}: {}", email, e);
            }
        });
        reset_sent += 1;

        // Rate limit emails
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    }

    tracing::info!(
        "Recovery complete: {} created, {} skipped, {} reset emails sent",
        created,
        skipped,
        reset_sent
    );

    Ok(Json(json!({
        "message": "User recovery complete",
        "contacts_found": contacts.len(),
        "stripe_customers_found": stripe_customers.len(),
        "users_created": created,
        "users_skipped": skipped,
        "reset_emails_sent": reset_sent,
        "errors": errors,
        "admin_user": admin_email
    })))
}

/// Sync all existing user emails to Resend contacts for disaster recovery.
/// This is a one-time operation to backfill existing users. New signups are
/// automatically synced.
pub async fn sync_all_users_to_resend(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let users = state.user_core.get_all_users().map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to get users: {}", e) })),
        )
    })?;

    let total = users.len();

    for user in &users {
        crate::utils::resend_contacts::sync_contact(&user.email).await;
        // Small delay to avoid rate limiting
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    tracing::info!("Resend sync complete: {}/{} synced", total, total);

    Ok(Json(json!({
        "message": "Resend sync complete",
        "total": total,
        "synced": total,
    })))
}

/// Reinitialize all Matrix clients and sync tasks.
/// Use this to recover from dead sync tasks without restarting the server.
pub async fn reinit_matrix(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Admin: reinitializing all Matrix clients and sync tasks");

    // Check how many sync tasks are currently dead
    let (total, dead) = {
        let sync_tasks = state.matrix_sync_tasks.lock().await;
        let total = sync_tasks.len();
        let dead = sync_tasks.values().filter(|h| h.is_finished()).count();
        (total, dead)
    };

    crate::jobs::scheduler::initialize_matrix_clients(Arc::clone(&state)).await;

    let new_total = state.matrix_sync_tasks.lock().await.len();

    tracing::info!(
        "Matrix reinit complete: {} tasks before ({} dead), {} tasks after",
        total,
        dead,
        new_total
    );

    Ok(Json(json!({
        "message": "Matrix clients reinitialized",
        "previous_tasks": total,
        "dead_tasks": dead,
        "new_tasks": new_total,
    })))
}

/// Admin probe endpoint: send a read-only command to a bridge management room and
/// return the bridge bot's response(s). Used to discover what `help`/`ping`/`version`
/// actually return for the specific deployed bridge versions, so we can write an
/// accurate health check.
///
/// GET /api/admin/bridge-probe/{bridge_type}/{cmd}
///   bridge_type: telegram | whatsapp | signal
///   cmd:         help | ping | version
///
/// The caller must already have that bridge connected (we read the management
/// room_id from their existing bridge record).
pub async fn probe_bridge_command(
    State(state): State<Arc<AppState>>,
    auth_user: crate::handlers::auth_middleware::AuthUser,
    axum::extract::Path((bridge_type, cmd)): axum::extract::Path<(String, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    use matrix_sdk::config::SyncSettings as MatrixSyncSettings;
    use matrix_sdk::ruma::events::room::message::{
        MessageType, RoomMessageEventContent, SyncRoomMessageEvent,
    };
    use matrix_sdk::ruma::events::AnySyncTimelineEvent;
    use matrix_sdk::ruma::{OwnedRoomId, OwnedUserId};
    use tokio::time::{sleep, Duration};

    // Whitelist bridge_type
    let prefix = match bridge_type.as_str() {
        "telegram" => "!tg",
        "whatsapp" => "!wa",
        "signal" => "!signal",
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "invalid bridge_type (expected telegram|whatsapp|signal)"})),
            ));
        }
    };

    // Whitelist cmd - read-only only, no side effects
    match cmd.as_str() {
        "help" | "ping" | "version" | "list-logins" => {}
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "invalid cmd (expected help|ping|version|list-logins)"})),
            ));
        }
    };

    let bridge_bot_env = match bridge_type.as_str() {
        "telegram" => "TELEGRAM_BRIDGE_BOT",
        "whatsapp" => "WHATSAPP_BRIDGE_BOT",
        "signal" => "SIGNAL_BRIDGE_BOT",
        _ => unreachable!(),
    };
    let bridge_bot = std::env::var(bridge_bot_env).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("{} env var not set", bridge_bot_env)})),
        )
    })?;
    let bot_user_id = OwnedUserId::try_from(bridge_bot.as_str()).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("invalid bridge bot user id: {}", e)})),
        )
    })?;

    // Look up management room from user's bridge record
    let bridge = state
        .user_repository
        .get_bridge(auth_user.user_id, &bridge_type)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("db error: {}", e)})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": format!("no {} bridge found for this user; connect it first so we have a management room to probe", bridge_type)})),
            )
        })?;

    let room_id_str = bridge.room_id.unwrap_or_default();
    let room_id = OwnedRoomId::try_from(room_id_str.as_str()).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "invalid room id on bridge record"})),
        )
    })?;

    let client = crate::utils::matrix_auth::get_cached_client(auth_user.user_id, &state)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("matrix client: {}", e)})),
            )
        })?;
    let room = client.get_room(&room_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "bridge management room not found in matrix client"})),
        )
    })?;

    // Record send timestamp BEFORE sending so we can filter stale messages
    let cmd_sent_ts_ms: u64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let full_cmd = format!("{} {}", prefix, cmd);
    tracing::info!(
        "[BRIDGE-PROBE] user={} sending {:?} to {} ts_ms={}",
        auth_user.user_id,
        full_cmd,
        room_id_str,
        cmd_sent_ts_ms
    );

    room.send(RoomMessageEventContent::text_plain(&full_cmd))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("failed to send command: {}", e)})),
            )
        })?;

    // Poll: sync and look for bridge bot messages with origin_server_ts > cmd_sent_ts_ms.
    // Up to ~9s total (6 iterations of sync_once(timeout=1s) + 500ms sleep each).
    let sync_settings = MatrixSyncSettings::default().timeout(Duration::from_secs(1));
    let mut responses: Vec<serde_json::Value> = Vec::new();

    for iter in 0..9 {
        let _ = client.sync_once(sync_settings.clone()).await;

        let mut opts =
            matrix_sdk::room::MessagesOptions::new(matrix_sdk::ruma::api::Direction::Backward);
        opts.limit = matrix_sdk::ruma::UInt::new(30).unwrap();
        if let Ok(messages) = room.messages(opts).await {
            for msg in messages.chunk.iter().rev() {
                // oldest-first within this window
                if let Ok(event) = msg.raw().deserialize() {
                    if event.sender() != bot_user_id {
                        continue;
                    }
                    let ts_ms: u64 = i64::from(event.origin_server_ts().0) as u64;
                    if ts_ms <= cmd_sent_ts_ms {
                        continue;
                    }
                    // avoid duplicates across iterations
                    if responses
                        .iter()
                        .any(|r| r.get("ts_ms").and_then(|v| v.as_u64()) == Some(ts_ms))
                    {
                        continue;
                    }
                    if let AnySyncTimelineEvent::MessageLike(
                        matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(sync_event),
                    ) = event
                    {
                        let content = match sync_event {
                            SyncRoomMessageEvent::Original(e) => e.content,
                            SyncRoomMessageEvent::Redacted(_) => continue,
                        };
                        let (msgtype, text, formatted) = match content.msgtype {
                            MessageType::Text(t) => {
                                let f = t.formatted.map(|f| f.body);
                                ("m.text", t.body, f)
                            }
                            MessageType::Notice(n) => {
                                let f = n.formatted.map(|f| f.body);
                                ("m.notice", n.body, f)
                            }
                            other => {
                                tracing::info!(
                                    "[BRIDGE-PROBE] skipping non-text bridge bot msg type={:?}",
                                    other
                                );
                                continue;
                            }
                        };
                        responses.push(json!({
                            "ts_ms": ts_ms,
                            "msgtype": msgtype,
                            "body": text,
                            "formatted": formatted,
                        }));
                    }
                }
            }
        }

        if !responses.is_empty() && iter >= 2 {
            // got at least one response and waited a bit extra for follow-ups
            break;
        }
        sleep(Duration::from_millis(500)).await;
    }

    tracing::info!(
        "[BRIDGE-PROBE] user={} cmd={:?} got {} response(s)",
        auth_user.user_id,
        full_cmd,
        responses.len()
    );

    Ok(Json(json!({
        "bridge_type": bridge_type,
        "command_sent": full_cmd,
        "cmd_sent_ts_ms": cmd_sent_ts_ms,
        "response_count": responses.len(),
        "responses": responses,
    })))
}

/// Admin endpoint to send an ARBITRARY (potentially side-effecting) command to
/// a bridge management room and capture the response. Used for empirical
/// verification of bridge bot replies (e.g. `logout`, `login qr`).
///
/// POST /api/admin/bridge-send/{bridge_type}
///   body: {"command": "!wa logout <login_id>"}
///
/// Unlike `probe_bridge_command` (GET, whitelisted read-only), this accepts
/// arbitrary commands. Use deliberately - sending `logout` will actually log
/// you out, sending `login qr` will start a login flow, etc.
#[derive(Deserialize)]
pub struct BridgeSendRequest {
    pub command: String,
}

pub async fn send_bridge_command(
    State(state): State<Arc<AppState>>,
    auth_user: crate::handlers::auth_middleware::AuthUser,
    axum::extract::Path(bridge_type): axum::extract::Path<String>,
    Json(body): Json<BridgeSendRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    use matrix_sdk::ruma::OwnedRoomId;
    use matrix_sdk::ruma::OwnedUserId;
    use tokio::time::Duration;

    if !matches!(bridge_type.as_str(), "telegram" | "whatsapp" | "signal") {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid bridge_type"})),
        ));
    }

    let bridge_bot_env = match bridge_type.as_str() {
        "telegram" => "TELEGRAM_BRIDGE_BOT",
        "whatsapp" => "WHATSAPP_BRIDGE_BOT",
        "signal" => "SIGNAL_BRIDGE_BOT",
        _ => unreachable!(),
    };
    let bridge_bot = std::env::var(bridge_bot_env).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("{} env var not set", bridge_bot_env)})),
        )
    })?;
    let bot_user_id = OwnedUserId::try_from(bridge_bot.as_str()).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("invalid bridge bot user id: {}", e)})),
        )
    })?;

    let bridge = state
        .user_repository
        .get_bridge(auth_user.user_id, &bridge_type)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("db error: {}", e)})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": format!("no {} bridge - connect first so we have a management room", bridge_type)})),
            )
        })?;

    let room_id_str = bridge.room_id.unwrap_or_default();
    let room_id = OwnedRoomId::try_from(room_id_str.as_str()).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "invalid room id on bridge record"})),
        )
    })?;

    let client = crate::utils::matrix_auth::get_cached_client(auth_user.user_id, &state)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("matrix client: {}", e)})),
            )
        })?;
    let room = client.get_room(&room_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "bridge management room not found in matrix client"})),
        )
    })?;

    tracing::warn!(
        "[BRIDGE-SEND] user={} bridge={} sending arbitrary command: {:?}",
        auth_user.user_id,
        bridge_type,
        body.command
    );

    let cmd_sent_ts_ms: u64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let responses = match crate::utils::bridge::probe_bridge_room(
        &client,
        &room,
        &bot_user_id,
        &body.command,
        Duration::from_secs(10),
    )
    .await
    {
        Ok(r) => r,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("probe failed: {}", e)})),
            ));
        }
    };

    Ok(Json(json!({
        "bridge_type": bridge_type,
        "command_sent": body.command,
        "cmd_sent_ts_ms": cmd_sent_ts_ms,
        "response_count": responses.len(),
        "responses": responses,
    })))
}

/// Admin passive-read endpoint: dump recent bot messages from a bridge
/// management room WITHOUT sending anything. Used to capture spontaneous push
/// events (e.g. the bridge notifying us that a session was revoked externally
/// when the user unlinked the device from their phone app).
///
/// GET /api/admin/bridge-recent-bot-messages/{bridge_type}?since_mins=N
///   bridge_type: telegram | whatsapp | signal
///   since_mins:  optional, default 10, max 120
///
/// Triggers a single sync_once to pull recent room history into the cache,
/// then returns every bot message whose ts falls within the window. Returns
/// bodies with their origin_server_ts so the caller can reconstruct ordering.
#[derive(Deserialize)]
pub struct RecentBotMessagesQuery {
    pub since_mins: Option<u64>,
}

pub async fn recent_bot_messages(
    State(state): State<Arc<AppState>>,
    auth_user: crate::handlers::auth_middleware::AuthUser,
    axum::extract::Path(bridge_type): axum::extract::Path<String>,
    axum::extract::Query(q): axum::extract::Query<RecentBotMessagesQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    use matrix_sdk::config::SyncSettings as MatrixSyncSettings;
    use matrix_sdk::ruma::events::room::message::{MessageType, SyncRoomMessageEvent};
    use matrix_sdk::ruma::events::AnySyncTimelineEvent;
    use matrix_sdk::ruma::{OwnedRoomId, OwnedUserId};
    use tokio::time::Duration;

    if !matches!(bridge_type.as_str(), "telegram" | "whatsapp" | "signal") {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid bridge_type"})),
        ));
    }

    let since_mins = q.since_mins.unwrap_or(10).min(120);
    let window_start_ts_ms: u64 = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
        - (since_mins * 60 * 1000);

    let bridge_bot_env = match bridge_type.as_str() {
        "telegram" => "TELEGRAM_BRIDGE_BOT",
        "whatsapp" => "WHATSAPP_BRIDGE_BOT",
        "signal" => "SIGNAL_BRIDGE_BOT",
        _ => unreachable!(),
    };
    let bridge_bot = std::env::var(bridge_bot_env).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("{} env var not set", bridge_bot_env)})),
        )
    })?;
    let bot_user_id = OwnedUserId::try_from(bridge_bot.as_str()).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("invalid bridge bot user id: {}", e)})),
        )
    })?;

    let bridge = state
        .user_repository
        .get_bridge(auth_user.user_id, &bridge_type)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("db error: {}", e)})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "error": format!(
                        "no {} bridge found for this user; connect it first so we have a management room",
                        bridge_type
                    )
                })),
            )
        })?;

    let room_id_str = bridge.room_id.unwrap_or_default();
    let room_id = OwnedRoomId::try_from(room_id_str.as_str()).map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "invalid room id on bridge record"})),
        )
    })?;

    let client = crate::utils::matrix_auth::get_cached_client(auth_user.user_id, &state)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("matrix client: {}", e)})),
            )
        })?;
    let room = client.get_room(&room_id).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "bridge management room not found in matrix client"})),
        )
    })?;

    // Sync once to pull the latest timeline events into the local store. No
    // command is sent - this is a pure read.
    let sync_settings = MatrixSyncSettings::default().timeout(Duration::from_secs(2));
    let _ = client.sync_once(sync_settings).await;

    let mut opts =
        matrix_sdk::room::MessagesOptions::new(matrix_sdk::ruma::api::Direction::Backward);
    opts.limit = matrix_sdk::ruma::UInt::new(100).unwrap();
    let messages = room.messages(opts).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("fetch messages failed: {}", e)})),
        )
    })?;

    let mut bot_messages: Vec<serde_json::Value> = Vec::new();
    for msg in &messages.chunk {
        let Ok(event) = msg.raw().deserialize() else {
            continue;
        };
        if event.sender() != bot_user_id {
            continue;
        }
        let ts_ms: u64 = i64::from(event.origin_server_ts().0) as u64;
        if ts_ms < window_start_ts_ms {
            continue;
        }
        if let AnySyncTimelineEvent::MessageLike(
            matrix_sdk::ruma::events::AnySyncMessageLikeEvent::RoomMessage(sync_event),
        ) = event
        {
            let content = match sync_event {
                SyncRoomMessageEvent::Original(e) => e.content,
                SyncRoomMessageEvent::Redacted(_) => continue,
            };
            let (body, msgtype) = match content.msgtype {
                MessageType::Text(t) => (t.body, "m.text"),
                MessageType::Notice(n) => (n.body, "m.notice"),
                _ => continue,
            };
            bot_messages.push(json!({
                "ts_ms": ts_ms,
                "msgtype": msgtype,
                "body": body,
            }));
        }
    }

    // Chronological ascending order for human readability
    bot_messages.sort_by_key(|v| v.get("ts_ms").and_then(|t| t.as_u64()).unwrap_or(0));

    Ok(Json(json!({
        "bridge_type": bridge_type,
        "since_mins": since_mins,
        "window_start_ts_ms": window_start_ts_ms,
        "message_count": bot_messages.len(),
        "messages": bot_messages,
    })))
}
