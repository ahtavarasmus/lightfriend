use std::error::Error;
use resend_rs::{Resend, types::CreateEmailBaseOptions};

/// Get Resend client and from email from environment.
/// Returns None if RESEND_API_KEY is not set (email sending is optional).
fn get_resend_config() -> Option<(Resend, String)> {
    let api_key = std::env::var("RESEND_API_KEY").ok()?;
    if api_key.is_empty() {
        return None;
    }

    let from_email = std::env::var("RESEND_FROM_EMAIL")
        .unwrap_or_else(|_| "notifications@lightfriend.ai".to_string());

    Some((Resend::new(&api_key), from_email))
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
    let (resend, from_email) = match get_resend_config() {
        Some(config) => config,
        None => {
            tracing::warn!("Magic link email NOT sent to {} (RESEND_API_KEY not configured)", to_email);
            return Ok(());
        }
    };

    let email_body = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
</head>
<body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; line-height: 1.6; color: #333; max-width: 600px; margin: 0 auto; padding: 20px;">
    <h2 style="color: #333;">Welcome to Lightfriend!</h2>

    <p>You've successfully subscribed. Click the button below to set your password and access your account:</p>

    <p style="margin: 30px 0;">
        <a href="{}" style="display: inline-block; background-color: #4F46E5; color: white; padding: 12px 24px; text-decoration: none; border-radius: 6px; font-weight: 500;">Set Your Password</a>
    </p>

    <p style="font-size: 14px; color: #666;">Or copy and paste this link into your browser:</p>
    <p style="font-size: 14px; word-break: break-all;"><a href="{}">{}</a></p>

    <p style="margin-top: 30px; font-size: 14px; color: #666;">This link doesn't expire, but can only be used once to set your password.</p>

    <p style="font-size: 14px; color: #666;">If you didn't sign up for Lightfriend, you can ignore this email.</p>

    <p style="margin-top: 30px;">-Rasmus</p>
</body>
</html>"#,
        magic_link, magic_link, magic_link
    );

    let email = CreateEmailBaseOptions::new(from_email, [to_email], "Set your Lightfriend password")
        .with_html(&email_body);

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
    let (resend, from_email) = match get_resend_config() {
        Some(config) => config,
        None => {
            tracing::warn!("Password reset email NOT sent to {} (RESEND_API_KEY not configured)", to_email);
            return Ok(());
        }
    };

    let email_body = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
</head>
<body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; line-height: 1.6; color: #333; max-width: 600px; margin: 0 auto; padding: 20px;">
    <h2 style="color: #333;">Password Reset Request</h2>

    <p>You requested a password reset for your Lightfriend account.</p>

    <p>Click the button below to set a new password:</p>

    <p style="margin: 30px 0;">
        <a href="{}" style="display: inline-block; background-color: #4F46E5; color: white; padding: 12px 24px; text-decoration: none; border-radius: 6px; font-weight: 500;">Reset Password</a>
    </p>

    <p style="font-size: 14px; color: #666;">Or copy and paste this link into your browser:</p>
    <p style="font-size: 14px; word-break: break-all;"><a href="{}">{}</a></p>

    <p style="margin-top: 30px; font-size: 14px; color: #666;">This link is valid for 24 hours and can only be used once.</p>

    <p style="font-size: 14px; color: #666;">If you didn't request a password reset, you can safely ignore this email.</p>

    <p style="margin-top: 30px;">-Rasmus</p>
</body>
</html>"#,
        reset_link, reset_link, reset_link
    );

    let email = CreateEmailBaseOptions::new(from_email, [to_email], "Lightfriend Password Reset")
        .with_html(&email_body);

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
    let (resend, from_email) = match get_resend_config() {
        Some(config) => config,
        None => {
            tracing::warn!("Subscription activated email NOT sent to {} (RESEND_API_KEY not configured)", to_email);
            return Ok(());
        }
    };

    let frontend_url = std::env::var("FRONTEND_URL").unwrap_or_else(|_| "https://lightfriend.io".to_string());
    let login_url = format!("{}/login", frontend_url);

    let email_body = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
</head>
<body style="font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; line-height: 1.6; color: #333; max-width: 600px; margin: 0 auto; padding: 20px;">
    <h2 style="color: #333;">Your Lightfriend subscription is now active!</h2>

    <p>We noticed you already have an account with this email. Your subscription has been linked to your existing account.</p>

    <p>Log in to get started:</p>

    <p style="margin: 30px 0;">
        <a href="{}" style="display: inline-block; background-color: #4F46E5; color: white; padding: 12px 24px; text-decoration: none; border-radius: 6px; font-weight: 500;">Log In</a>
    </p>

    <p style="font-size: 14px; color: #666;">Or copy and paste this link into your browser:</p>
    <p style="font-size: 14px; word-break: break-all;"><a href="{}">{}</a></p>

    <p style="margin-top: 30px; font-size: 14px; color: #666;">If you have any questions, just reply to this email.</p>

    <p style="margin-top: 30px;">-Rasmus</p>
</body>
</html>"#,
        login_url, login_url, login_url
    );

    let email = CreateEmailBaseOptions::new(from_email, [to_email], "Your Lightfriend subscription is active")
        .with_html(&email_body);

    resend.emails.send(email).await?;

    tracing::info!("Subscription activated email sent to {}", to_email);

    Ok(())
}
