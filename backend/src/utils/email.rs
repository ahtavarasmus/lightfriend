use std::error::Error;

/// Send a magic link email to a new user for password setup
///
/// # Arguments
/// * `to_email` - Recipient email address
/// * `magic_link` - The full URL with token for password setup
///
/// # Returns
/// * `Ok(())` - Email sent successfully
/// * `Err` - Failed to send email
pub async fn send_magic_link_email(to_email: &str, magic_link: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
    use lettre::{Message, SmtpTransport, Transport};
    use lettre::transport::smtp::authentication::Credentials;

    let smtp_server = std::env::var("SMTP_SERVER").unwrap_or_else(|_| "smtp.gmail.com".to_string());
    let smtp_port: u16 = std::env::var("SMTP_PORT")
        .unwrap_or_else(|_| "587".to_string())
        .parse()
        .unwrap_or(587);

    let smtp_username = std::env::var("SMTP_USERNAME").map_err(|_| "SMTP_USERNAME not set")?;
    let smtp_password = std::env::var("SMTP_PASSWORD").map_err(|_| "SMTP_PASSWORD not set")?;
    let from_email = std::env::var("FROM_EMAIL").unwrap_or_else(|_| "noreply@lightfriend.ai".to_string());

    let email_body = format!(
        r#"Welcome to Lightfriend!

You've successfully subscribed. If you weren't redirected to set your password after payment, or if you closed that page before completing it, use the link below:

{}

This link doesn't expire and can be used anytime to set your password or log in.

If you didn't sign up for Lightfriend, you can ignore this email.

Best,
The Lightfriend Team"#,
        magic_link
    );

    let email = Message::builder()
        .from(from_email.parse()?)
        .to(to_email.parse()?)
        .subject("Set your Lightfriend password")
        .body(email_body)?;

    let creds = Credentials::new(smtp_username, smtp_password);

    let mailer = SmtpTransport::starttls_relay(&smtp_server)?
        .port(smtp_port)
        .credentials(creds)
        .build();

    mailer.send(&email)?;

    tracing::info!("Magic link email sent to {}", to_email);

    Ok(())
}
