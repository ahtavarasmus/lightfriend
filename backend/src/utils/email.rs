use std::error::Error;
use crate::repositories::user_repository::UserRepository;

/// Send a magic link email to a new user for password setup
///
/// Uses user 1's IMAP credentials (same as broadcast emails).
///
/// # Arguments
/// * `user_repository` - Repository to fetch IMAP credentials
/// * `to_email` - Recipient email address
/// * `magic_link` - The full URL with token for password setup
///
/// # Returns
/// * `Ok(())` - Email sent successfully
/// * `Err` - Failed to send email
pub async fn send_magic_link_email(
    user_repository: &UserRepository,
    to_email: &str,
    magic_link: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    use lettre::{Message, SmtpTransport, Transport};
    use lettre::transport::smtp::authentication::Credentials;
    use lettre::message::{header::ContentType, SinglePart};

    // Use user 1's IMAP credentials (same as broadcast emails)
    let (from_email, password, imap_server, _) = user_repository
        .get_imap_credentials(1)
        .map_err(|e| format!("Failed to get IMAP credentials: {}", e))?
        .ok_or("No IMAP credentials found for admin user (id=1)")?;

    // Derive SMTP server from IMAP server (e.g., imap.gmail.com -> smtp.gmail.com)
    let smtp_server = imap_server
        .as_deref()
        .unwrap_or("smtp.gmail.com")
        .replace("imap", "smtp");
    let smtp_port: u16 = 587;

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

    let email = Message::builder()
        .from(from_email.parse()?)
        .to(to_email.parse()?)
        .subject("Set your Lightfriend password")
        .singlepart(
            SinglePart::builder()
                .header(ContentType::TEXT_HTML)
                .body(email_body)
        )?;

    let creds = Credentials::new(from_email.clone(), password);

    let mailer = SmtpTransport::starttls_relay(&smtp_server)?
        .port(smtp_port)
        .credentials(creds)
        .build();

    mailer.send(&email)?;

    tracing::info!("Magic link email sent to {}", to_email);

    Ok(())
}

/// Send a password reset link email
///
/// Uses user 1's IMAP credentials (same as broadcast emails).
///
/// # Arguments
/// * `user_repository` - Repository to fetch IMAP credentials
/// * `to_email` - Recipient email address
/// * `reset_link` - The full URL with token for password reset
///
/// # Returns
/// * `Ok(())` - Email sent successfully
/// * `Err` - Failed to send email
pub async fn send_password_reset_email(
    user_repository: &UserRepository,
    to_email: &str,
    reset_link: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    use lettre::{Message, SmtpTransport, Transport};
    use lettre::transport::smtp::authentication::Credentials;
    use lettre::message::{header::ContentType, SinglePart};

    // Use user 1's IMAP credentials (same as broadcast emails)
    let (from_email, password, imap_server, _) = user_repository
        .get_imap_credentials(1)
        .map_err(|e| format!("Failed to get IMAP credentials: {}", e))?
        .ok_or("No IMAP credentials found for admin user (id=1)")?;

    // Derive SMTP server from IMAP server (e.g., imap.gmail.com -> smtp.gmail.com)
    let smtp_server = imap_server
        .as_deref()
        .unwrap_or("smtp.gmail.com")
        .replace("imap", "smtp");
    let smtp_port: u16 = 587;

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

    let email = Message::builder()
        .from(from_email.parse()?)
        .to(to_email.parse()?)
        .subject("Lightfriend Password Reset")
        .singlepart(
            SinglePart::builder()
                .header(ContentType::TEXT_HTML)
                .body(email_body)
        )?;

    let creds = Credentials::new(from_email.clone(), password);

    let mailer = SmtpTransport::starttls_relay(&smtp_server)?
        .port(smtp_port)
        .credentials(creds)
        .build();

    mailer.send(&email)?;

    tracing::info!("Password reset email sent to {}", to_email);

    Ok(())
}

/// Send a subscription activated email to an existing user who subscribed via guest checkout
///
/// Uses user 1's IMAP credentials (same as broadcast emails).
pub async fn send_subscription_activated_email(
    user_repository: &UserRepository,
    to_email: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    use lettre::{Message, SmtpTransport, Transport};
    use lettre::transport::smtp::authentication::Credentials;
    use lettre::message::{header::ContentType, SinglePart};

    let (from_email, password, imap_server, _) = user_repository
        .get_imap_credentials(1)
        .map_err(|e| format!("Failed to get IMAP credentials: {}", e))?
        .ok_or("No IMAP credentials found for admin user (id=1)")?;

    let smtp_server = imap_server
        .as_deref()
        .unwrap_or("smtp.gmail.com")
        .replace("imap", "smtp");
    let smtp_port: u16 = 587;

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

    let email = Message::builder()
        .from(from_email.parse()?)
        .to(to_email.parse()?)
        .subject("Your Lightfriend subscription is active")
        .singlepart(
            SinglePart::builder()
                .header(ContentType::TEXT_HTML)
                .body(email_body)
        )?;

    let creds = Credentials::new(from_email.clone(), password);

    let mailer = SmtpTransport::starttls_relay(&smtp_server)?
        .port(smtp_port)
        .credentials(creds)
        .build();

    mailer.send(&email)?;

    tracing::info!("Subscription activated email sent to {}", to_email);

    Ok(())
}
