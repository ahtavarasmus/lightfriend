//! Tests for IMAP authentication and server detection

use backend::handlers::imap_auth::detect_imap_server;

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
