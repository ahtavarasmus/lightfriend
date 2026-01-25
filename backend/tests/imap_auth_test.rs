//! Tests for IMAP authentication and server detection

use backend::handlers::imap_auth::{
    detect_imap_config, detect_imap_server, extract_email_domain, is_known_email_provider,
    is_valid_email,
};

// ============================================================
// IMAP Server Detection Tests
// ============================================================

#[test]
fn test_detect_imap_server_icloud() {
    assert_eq!(
        detect_imap_server("user@icloud.com"),
        ("imap.mail.me.com", 993)
    );
}

#[test]
fn test_detect_imap_server_me_com() {
    assert_eq!(detect_imap_server("user@me.com"), ("imap.mail.me.com", 993));
}

#[test]
fn test_detect_imap_server_mac_com() {
    assert_eq!(
        detect_imap_server("user@mac.com"),
        ("imap.mail.me.com", 993)
    );
}

#[test]
fn test_detect_imap_server_gmail() {
    assert_eq!(
        detect_imap_server("user@gmail.com"),
        ("imap.gmail.com", 993)
    );
}

#[test]
fn test_detect_imap_server_googlemail() {
    assert_eq!(
        detect_imap_server("user@googlemail.com"),
        ("imap.gmail.com", 993)
    );
}

#[test]
fn test_detect_imap_server_outlook() {
    assert_eq!(
        detect_imap_server("user@outlook.com"),
        ("outlook.office365.com", 993)
    );
}

#[test]
fn test_detect_imap_server_hotmail() {
    assert_eq!(
        detect_imap_server("user@hotmail.com"),
        ("outlook.office365.com", 993)
    );
}

#[test]
fn test_detect_imap_server_live() {
    assert_eq!(
        detect_imap_server("user@live.com"),
        ("outlook.office365.com", 993)
    );
}

#[test]
fn test_detect_imap_server_msn() {
    assert_eq!(
        detect_imap_server("user@msn.com"),
        ("outlook.office365.com", 993)
    );
}

#[test]
fn test_detect_imap_server_yahoo() {
    assert_eq!(
        detect_imap_server("user@yahoo.com"),
        ("imap.mail.yahoo.com", 993)
    );
}

#[test]
fn test_detect_imap_server_yahoo_uk() {
    assert_eq!(
        detect_imap_server("user@yahoo.co.uk"),
        ("imap.mail.yahoo.com", 993)
    );
}

#[test]
fn test_detect_imap_server_aol() {
    assert_eq!(detect_imap_server("user@aol.com"), ("imap.aol.com", 993));
}

#[test]
fn test_detect_imap_server_zoho() {
    assert_eq!(detect_imap_server("user@zoho.com"), ("imap.zoho.com", 993));
}

#[test]
fn test_detect_imap_server_fastmail() {
    assert_eq!(
        detect_imap_server("user@fastmail.com"),
        ("imap.fastmail.com", 993)
    );
}

#[test]
fn test_detect_imap_server_unknown_defaults_to_gmail() {
    // Unknown domains default to Gmail (legacy behavior)
    assert_eq!(
        detect_imap_server("user@customdomain.com"),
        ("imap.gmail.com", 993)
    );
}

#[test]
fn test_detect_imap_server_case_insensitive() {
    // Should handle uppercase domains
    assert_eq!(
        detect_imap_server("user@ICLOUD.COM"),
        ("imap.mail.me.com", 993)
    );
    assert_eq!(
        detect_imap_server("user@Gmail.Com"),
        ("imap.gmail.com", 993)
    );
}

#[test]
fn test_detect_imap_server_invalid_email_no_at() {
    // Invalid email without @ defaults to Gmail
    assert_eq!(detect_imap_server("invalidemail"), ("imap.gmail.com", 993));
}

#[test]
fn test_detect_imap_server_empty_string() {
    // Empty string defaults to Gmail
    assert_eq!(detect_imap_server(""), ("imap.gmail.com", 993));
}

// ============================================================
// Email Validation Tests
// ============================================================

#[test]
fn test_is_valid_email_valid() {
    assert!(is_valid_email("user@example.com"));
    assert!(is_valid_email("user.name@example.com"));
    assert!(is_valid_email("user+tag@example.com"));
    assert!(is_valid_email("user@sub.domain.com"));
    assert!(is_valid_email("user123@example.co.uk"));
}

#[test]
fn test_is_valid_email_invalid_no_at() {
    assert!(!is_valid_email("userexample.com"));
    assert!(!is_valid_email("user"));
}

#[test]
fn test_is_valid_email_invalid_multiple_at() {
    assert!(!is_valid_email("user@@example.com"));
    assert!(!is_valid_email("user@name@example.com"));
}

#[test]
fn test_is_valid_email_invalid_empty_parts() {
    assert!(!is_valid_email("@example.com"));
    assert!(!is_valid_email("user@"));
    assert!(!is_valid_email("@"));
}

#[test]
fn test_is_valid_email_invalid_no_dot_in_domain() {
    assert!(!is_valid_email("user@example"));
    assert!(!is_valid_email("user@localhost"));
}

#[test]
fn test_is_valid_email_invalid_domain_dot_position() {
    assert!(!is_valid_email("user@.example.com"));
    assert!(!is_valid_email("user@example.com."));
    assert!(!is_valid_email("user@example..com"));
}

#[test]
fn test_is_valid_email_empty() {
    assert!(!is_valid_email(""));
}

// ============================================================
// Extract Email Domain Tests
// ============================================================

#[test]
fn test_extract_email_domain_valid() {
    assert_eq!(
        extract_email_domain("user@example.com"),
        Some("example.com")
    );
    assert_eq!(
        extract_email_domain("user@sub.domain.co.uk"),
        Some("sub.domain.co.uk")
    );
}

#[test]
fn test_extract_email_domain_invalid() {
    assert_eq!(extract_email_domain("userexample.com"), None);
    assert_eq!(extract_email_domain(""), None);
}

// ============================================================
// Detect IMAP Config Tests (struct-based)
// ============================================================

#[test]
fn test_detect_imap_config_gmail() {
    let config = detect_imap_config("user@gmail.com").unwrap();
    assert_eq!(config.host, "imap.gmail.com");
    assert_eq!(config.port, 993);
    assert!(config.tls);
}

#[test]
fn test_detect_imap_config_icloud() {
    let config = detect_imap_config("user@icloud.com").unwrap();
    assert_eq!(config.host, "imap.mail.me.com");
    assert_eq!(config.port, 993);
    assert!(config.tls);
}

#[test]
fn test_detect_imap_config_outlook() {
    let config = detect_imap_config("user@outlook.com").unwrap();
    assert_eq!(config.host, "outlook.office365.com");
    assert_eq!(config.port, 993);
}

#[test]
fn test_detect_imap_config_unknown() {
    assert!(detect_imap_config("user@unknown-provider.xyz").is_none());
}

#[test]
fn test_detect_imap_config_invalid_email() {
    assert!(detect_imap_config("not-an-email").is_none());
}

// ============================================================
// Is Known Email Provider Tests
// ============================================================

#[test]
fn test_is_known_email_provider_known() {
    assert!(is_known_email_provider("user@gmail.com"));
    assert!(is_known_email_provider("user@icloud.com"));
    assert!(is_known_email_provider("user@outlook.com"));
    assert!(is_known_email_provider("user@yahoo.com"));
    assert!(is_known_email_provider("user@aol.com"));
    assert!(is_known_email_provider("user@zoho.com"));
    assert!(is_known_email_provider("user@fastmail.com"));
}

#[test]
fn test_is_known_email_provider_unknown() {
    assert!(!is_known_email_provider("user@custom-domain.com"));
    assert!(!is_known_email_provider("user@mycompany.org"));
}

#[test]
fn test_is_known_email_provider_invalid() {
    assert!(!is_known_email_provider("not-an-email"));
    assert!(!is_known_email_provider(""));
}
