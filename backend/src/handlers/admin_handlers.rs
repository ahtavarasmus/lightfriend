use crate::UserCoreOps;
use axum::{extract::State, http::StatusCode, Json};
use diesel::prelude::*;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::models::user_models::{AdminAlert, DisabledAlertType, MessageStatusLog, WaitlistEntry};
use crate::schema::{message_status_log, waitlist};

#[derive(Deserialize)]
pub struct BroadcastMessageRequest {
    pub message: String,
}

#[derive(Deserialize, Clone)]
pub struct EmailBroadcastRequest {
    pub subject: String,
    pub message: String,
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
                        "Failed to unsubscribe. Sorry about this, send email to rasmus@ahtava.com"
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

            let mut conn = state.db_pool.get().map_err(|e| {
                tracing::error!("Failed to get DB connection: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Database error".to_string(),
                )
            })?;

            // Try to delete from waitlist
            let deleted = diesel::delete(
                waitlist::table.filter(waitlist::email.eq(&params.email.to_lowercase())),
            )
            .execute(&mut conn)
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
        let mut conn = state.db_pool.get().map_err(|e| {
            tracing::error!("Failed to get DB connection for waitlist: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Database error"})),
            )
        })?;
        waitlist::table
            .select(WaitlistEntry::as_select())
            .load(&mut conn)
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
        for entry in waitlist_entries {
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

    // Send email with reset link
    if let Err(e) = crate::utils::email::send_password_reset_email(&user.email, &reset_link).await {
        tracing::error!(
            "Failed to send password reset email to {}: {}",
            user.email,
            e
        );
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to send reset email. Please try again."})),
        ));
    }

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

    let conn = &mut state.db_pool.get().map_err(|e| {
        tracing::error!("Failed to get DB connection: {}", e);
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
        .load(conn)
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
    pub id: Option<i32>,
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
    use crate::schema::users;

    let conn = &mut state.db_pool.get().map_err(|e| {
        tracing::error!("Failed to get DB connection: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Database connection error"})),
        )
    })?;

    // Get all messages (last 1000 for stats)
    let all_messages: Vec<MessageStatusLog> = message_status_log::table
        .order(message_status_log::created_at.desc())
        .limit(1000)
        .load(conn)
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
        .load(conn)
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
