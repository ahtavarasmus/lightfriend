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
        r#"Welcome to Lightfriend!

You've successfully subscribed. Click the link below to set your password and access your account:

{}

This link doesn't expire, but can only be used once to set your password.

If you didn't sign up for Lightfriend, you can ignore this email.

Best,
The Lightfriend Team"#,
        magic_link
    );

    let email = Message::builder()
        .from(from_email.parse()?)
        .to(to_email.parse()?)
        .subject("Set your Lightfriend password")
        .singlepart(
            SinglePart::builder()
                .header(ContentType::TEXT_PLAIN)
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
        r#"Password Reset Request

You requested a password reset for your Lightfriend account.

Click the link below to set a new password:

{}

This link is valid for 24 hours and can only be used once.

If you didn't request a password reset, you can safely ignore this email.

Best,
The Lightfriend Team"#,
        reset_link
    );

    let email = Message::builder()
        .from(from_email.parse()?)
        .to(to_email.parse()?)
        .subject("Lightfriend Password Reset")
        .singlepart(
            SinglePart::builder()
                .header(ContentType::TEXT_PLAIN)
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
