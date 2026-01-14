use std::error::Error;
use resend_rs::{Resend, types::CreateEmailBaseOptions};


/// Lightfriend brand colors
const PRIMARY_BLUE: &str = "#1E90FF";
const LIGHT_BLUE: &str = "#7EB2FF";

/// Generate branded email HTML wrapper with header and footer
fn wrap_email_body(title: &str, content: &str, signature: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
</head>
<body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; line-height: 1.6; color: #333; max-width: 600px; margin: 0 auto; padding: 20px; background-color: #fafafa;">
    <!-- Main Content -->
    <div style="background-color: white; border-radius: 8px; padding: 30px; box-shadow: 0 1px 3px rgba(0,0,0,0.1);">
        <h2 style="color: #333; margin-top: 0;">{title}</h2>
        {content}
        <p style="margin-top: 20px;">{signature}</p>
    </div>

    <!-- Footer -->
    <div style="text-align: center; padding: 20px 0; margin-top: 20px;">
        <p style="font-size: 12px; color: #888; margin: 0;">
            <a href="https://lightfriend.ai" style="color: {light_blue}; text-decoration: none;">lightfriend.ai</a>
        </p>
    </div>
</body>
</html>"#,
        light_blue = LIGHT_BLUE,
        title = title,
        content = content,
        signature = signature
    )
}

/// Get Resend client, from email, and reply-to email from environment.
/// Returns None if RESEND_API_KEY is not set (email sending is optional).
fn get_resend_config() -> Option<(Resend, String, String)> {
    let api_key = std::env::var("RESEND_API_KEY").ok()?;
    if api_key.is_empty() {
        return None;
    }

    let from_email = std::env::var("RESEND_FROM_EMAIL")
        .unwrap_or_else(|_| "notifications@lightfriend.ai".to_string());

    // Reply-to email - where replies should go (defaults to rasmus@lightfriend.ai)
    let reply_to = std::env::var("RESEND_REPLY_TO_EMAIL")
        .unwrap_or_else(|_| "rasmus@lightfriend.ai".to_string());

    Some((Resend::new(&api_key), from_email, reply_to))
}

/// Send a magic link email to a new user for password setup
///
/// Uses Resend API for reliable email delivery.
/// If Resend is not configured, logs a warning and returns Ok (graceful fallback).
///
/// # Arguments
/// * `to_email` - Recipient email address
/// * `magic_link` - The full URL with token for password setup
///
/// # Returns
/// * `Ok(())` - Email sent successfully (or skipped if Resend not configured)
/// * `Err` - Failed to send email
pub async fn send_magic_link_email(
    to_email: &str,
    magic_link: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    send_magic_link_email_with_options(to_email, magic_link, false).await
}

/// Send a magic link email for new subscribers, with option to include phone number warning
///
/// Uses Resend API for reliable email delivery.
/// If Resend is not configured, logs a warning and returns Ok (graceful fallback).
///
/// # Arguments
/// * `to_email` - Recipient email address
/// * `magic_link` - The full URL with token for password setup
/// * `phone_skipped_duplicate` - If true, includes a message that the phone number was already in use
pub async fn send_magic_link_email_with_options(
    to_email: &str,
    magic_link: &str,
    phone_skipped_duplicate: bool,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let (resend, from_email, reply_to) = match get_resend_config() {
        Some(config) => config,
        None => {
            tracing::warn!("Magic link email NOT sent to {} (RESEND_API_KEY not configured)", to_email);
            return Ok(());
        }
    };
    let _ = reply_to; // Used in other email functions

    let phone_warning = if phone_skipped_duplicate {
        r#"<p style="margin: 20px 0; padding: 15px; background-color: #fff3cd; border: 1px solid #ffc107; border-radius: 6px; color: #856404;">
            <strong>Note:</strong> The phone number you entered is already associated with another account.
            Please update your phone number in your account settings after logging in.
            Your phone number is needed to use Lightfriend via SMS.
        </p>"#
    } else {
        ""
    };

    let content = format!(
        r#"<p>You've successfully subscribed. Click the button below to set your password and access your account:</p>

        {phone_warning}

        <p style="margin: 30px 0; text-align: center;">
            <a href="{link}" style="display: inline-block; background-color: {blue}; color: white; padding: 12px 24px; text-decoration: none; border-radius: 6px; font-weight: 500;">Set Your Password</a>
        </p>

        <p style="font-size: 14px; color: #666;">Or copy and paste this link into your browser:</p>
        <p style="font-size: 14px; word-break: break-all;"><a href="{link}" style="color: {blue};">{link}</a></p>

        <p style="margin-top: 30px; font-size: 14px; color: #666;">This link doesn't expire, but can only be used once to set your password.</p>

        <p style="font-size: 14px; color: #666;">If you didn't sign up for Lightfriend, you can ignore this email.</p>

        <p style="margin-top: 30px; font-size: 14px; color: #666;">Have questions or feature requests? Just reply to this email - I'd love to hear from you!</p>"#,
        link = magic_link,
        blue = PRIMARY_BLUE,
        phone_warning = phone_warning
    );

    let email_body = wrap_email_body(
        "Welcome to Lightfriend!",
        &content,
        "-Rasmus from Lightfriend"
    );

    let from_with_name = format!("Lightfriend <{}>", from_email);
    let email = CreateEmailBaseOptions::new(from_with_name, [to_email], "Set your Lightfriend password")
        .with_html(&email_body)
        .with_reply(&reply_to);

    resend.emails.send(email).await?;

    tracing::info!("Magic link email sent to {}", to_email);

    Ok(())
}

/// Send a password reset link email
///
/// Uses Resend API for reliable email delivery.
/// If Resend is not configured, logs a warning and returns Ok (graceful fallback).
///
/// # Arguments
/// * `to_email` - Recipient email address
/// * `reset_link` - The full URL with token for password reset
///
/// # Returns
/// * `Ok(())` - Email sent successfully (or skipped if Resend not configured)
/// * `Err` - Failed to send email
pub async fn send_password_reset_email(
    to_email: &str,
    reset_link: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let (resend, from_email, reply_to) = match get_resend_config() {
        Some(config) => config,
        None => {
            tracing::warn!("Password reset email NOT sent to {} (RESEND_API_KEY not configured)", to_email);
            return Ok(());
        }
    };
    let _ = reply_to; // Used in other email functions

    let content = format!(
        r#"<p>You requested a password reset for your Lightfriend account.</p>

        <p>Click the button below to set a new password:</p>

        <p style="margin: 30px 0; text-align: center;">
            <a href="{link}" style="display: inline-block; background-color: {blue}; color: white; padding: 12px 24px; text-decoration: none; border-radius: 6px; font-weight: 500;">Reset Password</a>
        </p>

        <p style="font-size: 14px; color: #666;">Or copy and paste this link into your browser:</p>
        <p style="font-size: 14px; word-break: break-all;"><a href="{link}" style="color: {blue};">{link}</a></p>

        <p style="margin-top: 30px; font-size: 14px; color: #666;">This link is valid for 24 hours and can only be used once.</p>

        <p style="font-size: 14px; color: #666;">If you didn't request a password reset, you can safely ignore this email.</p>

        <p style="margin-top: 30px; font-size: 14px; color: #666;">Have questions or feature requests? Just reply to this email - I'd love to hear from you!</p>"#,
        link = reset_link,
        blue = PRIMARY_BLUE
    );

    let email_body = wrap_email_body(
        "Password Reset Request",
        &content,
        "-Rasmus from Lightfriend"
    );

    let from_with_name = format!("Lightfriend <{}>", from_email);
    let email = CreateEmailBaseOptions::new(from_with_name, [to_email], "Lightfriend Password Reset")
        .with_html(&email_body)
        .with_reply(&reply_to);

    resend.emails.send(email).await?;

    tracing::info!("Password reset email sent to {}", to_email);

    Ok(())
}

/// Send a subscription activated email to an existing user who subscribed via guest checkout
///
/// Uses Resend API for reliable email delivery.
/// If Resend is not configured, logs a warning and returns Ok (graceful fallback).
pub async fn send_subscription_activated_email(
    to_email: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let (resend, from_email, reply_to) = match get_resend_config() {
        Some(config) => config,
        None => {
            tracing::warn!("Subscription activated email NOT sent to {} (RESEND_API_KEY not configured)", to_email);
            return Ok(());
        }
    };
    let _ = reply_to; // Used in other email functions

    let frontend_url = std::env::var("FRONTEND_URL").unwrap_or_else(|_| "https://lightfriend.io".to_string());
    let login_url = format!("{}/login", frontend_url);

    let content = format!(
        r#"<p>We noticed you already have an account with this email. Your subscription has been linked to your existing account.</p>

        <p>Log in to get started:</p>

        <p style="margin: 30px 0; text-align: center;">
            <a href="{link}" style="display: inline-block; background-color: {blue}; color: white; padding: 12px 24px; text-decoration: none; border-radius: 6px; font-weight: 500;">Log In</a>
        </p>

        <p style="font-size: 14px; color: #666;">Or copy and paste this link into your browser:</p>
        <p style="font-size: 14px; word-break: break-all;"><a href="{link}" style="color: {blue};">{link}</a></p>

        <p style="margin-top: 30px; font-size: 14px; color: #666;">Have questions or feature requests? Just reply to this email - I'd love to hear from you!</p>"#,
        link = login_url,
        blue = PRIMARY_BLUE
    );

    let email_body = wrap_email_body(
        "Your Lightfriend subscription is now active!",
        &content,
        "-Rasmus from Lightfriend"
    );

    let from_with_name = format!("Lightfriend <{}>", from_email);
    let email = CreateEmailBaseOptions::new(from_with_name, [to_email], "Your Lightfriend subscription is active")
        .with_html(&email_body)
        .with_reply(&reply_to);

    resend.emails.send(email).await?;

    tracing::info!("Subscription activated email sent to {}", to_email);

    Ok(())
}


/// Send a broadcast email (admin feature updates, announcements, etc.)
///
/// Uses Resend API for reliable email delivery.
/// Unlike other email functions, this returns an error if Resend is not configured.
///
/// # Arguments
/// * `to_email` - Recipient email address
/// * `subject` - Email subject line
/// * `html_body` - Pre-formatted HTML body (caller handles formatting)
///
/// # Returns
/// * `Ok(())` - Email sent successfully
/// * `Err` - Failed to send email or Resend not configured
pub async fn send_broadcast_email(
    to_email: &str,
    subject: &str,
    html_body: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let (resend, from_email, reply_to) = match get_resend_config() {
        Some(config) => config,
        None => {
            return Err("RESEND_API_KEY not configured - cannot send broadcast emails".into());
        }
    };

    // Format as "Lightfriend <email>" for display name
    let from_with_name = format!("Lightfriend <{}>", from_email);

    let email = CreateEmailBaseOptions::new(from_with_name, [to_email], subject)
        .with_html(html_body)
        .with_reply(&reply_to);

    resend.emails.send(email).await?;

    tracing::info!("Broadcast email sent to {}", to_email);

    Ok(())
}

/// Check if Resend is configured
pub fn is_resend_configured() -> bool {
    get_resend_config().is_some()
}

