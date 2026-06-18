//! SMS body sanitization for A2P 10DLC carrier compliance.
//!
//! Carrier anti-phishing filters (T-Mobile/AT&T/Verizon) aggressively flag
//! and block A2P SMS containing URLs that match phishing patterns. To keep
//! deliverability healthy on US traffic, we sanitize outbound message bodies
//! before handing them to Twilio:
//!
//! - URLs to trusted domains (our own) are preserved as-is.
//! - URL shorteners are stripped to a bare `[link]` placeholder.
//! - Other URLs are replaced with `[link: domain]` so the user retains
//!   context about the source without an actual URL string in the SMS.
//!
//! This is opt-in via `apply_sms_url_filter`, which is called from the
//! Twilio send path. The filter is intentionally conservative: it only
//! touches `http://` and `https://` URLs and never edits surrounding text.

use regex::Regex;
use std::sync::OnceLock;

/// Twilio's hard maximum for one SMS/MMS `Body` request.
pub const SMS_BODY_CHARACTER_LIMIT: usize = 1600;

const SMS_TRUNCATION_NOTICE: &str = "\n\n[truncated]";

fn url_re() -> &'static Regex {
    static URL_RE: OnceLock<Regex> = OnceLock::new();
    URL_RE.get_or_init(|| Regex::new(r#"https?://[^\s<>"'\)\]\}]+"#).expect("valid url regex"))
}

/// Domains we trust to appear unmodified in outbound SMS.
///
/// Add a domain here only when it is registered on our 10DLC campaign
/// (or the equivalent for other regulated regions).
const TRUSTED_DOMAINS: &[&str] = &["lightfriend.ai", "lightfriend.app"];

/// URL shorteners are an automatic phishing flag on US A2P. Always replaced.
const SHORTENER_DOMAINS: &[&str] = &[
    "bit.ly",
    "t.co",
    "tinyurl.com",
    "goo.gl",
    "ow.ly",
    "is.gd",
    "buff.ly",
    "rebrand.ly",
    "cutt.ly",
    "shorturl.at",
    "tiny.cc",
    "rb.gy",
    "lnkd.in",
];

/// Sanitize a message body for A2P 10DLC delivery.
///
/// Two passes:
/// 1. Re-fang defanged emails and domains (e.g. `user[.]name(at)example[.]com`
///    → `user.name@example.com`). Defanging is a phishing-evasion signal that
///    carrier filters score against, even when the underlying content is
///    legitimate.
/// 2. URL filter: trusted domains preserved, shorteners replaced with
///    `[link]`, others replaced with `[link: domain]`. Trailing punctuation
///    (`.,;!?`) on a URL is excluded from the match so it stays attached to
///    the surrounding sentence.
pub fn apply_sms_url_filter(body: &str) -> String {
    let body = unfang_text(body);
    apply_url_filter_only(&body)
}

/// Clamp a body to the provider-safe SMS request size while preserving UTF-8
/// boundaries and making the truncation visible to the recipient.
pub fn clamp_sms_body(body: &str) -> String {
    if body.chars().count() <= SMS_BODY_CHARACTER_LIMIT {
        return body.to_string();
    }

    let notice_len = SMS_TRUNCATION_NOTICE.chars().count();
    let keep = SMS_BODY_CHARACTER_LIMIT.saturating_sub(notice_len);
    let mut out: String = body.chars().take(keep).collect();
    out.push_str(SMS_TRUNCATION_NOTICE);
    out
}

/// Re-fang defanged emails and domains. Patterns handled:
/// - `[.]` / `(.)` / `[dot]` / `(dot)` between word chars → `.`
/// - `(at)` / `[at]` / `[@]` between word chars → `@`
///
/// Only replaces when surrounded by identifier-like characters, so plain
/// prose like "meet (at) 5pm" is left alone.
pub fn unfang_text(body: &str) -> String {
    let mut out = body.to_string();

    // Require word chars touching the bracket on both sides AND no
    // whitespace inside the bracket. This catches real defanged identifiers
    // like `user[.]name(at)gmail[.]com` while leaving prose like
    // `meet me (at) noon` or `press [Enter]` alone.
    let patterns: &[(&str, &str)] = &[
        // (.)  [.]
        (r"(\w)[\[\(]\.[\]\)](\w)", "$1.$2"),
        // (dot) [dot] (case-insensitive)
        (r"(?i)(\w)[\[\(]dot[\]\)](\w)", "$1.$2"),
        // (at) [at] [@] (case-insensitive)
        (r"(?i)(\w)[\[\(](?:at|@)[\]\)](\w)", "$1@$2"),
    ];

    for (pat, replacement) in patterns {
        let re = regex::Regex::new(pat).expect("valid defang regex");
        // Loop because a single pass leaves overlapping matches like
        // `a[.]b[.]c` → `a.b[.]c` after one pass; rerun until stable.
        loop {
            let next = re.replace_all(&out, *replacement).into_owned();
            if next == out {
                break;
            }
            out = next;
        }
    }
    out
}

fn apply_url_filter_only(body: &str) -> String {
    url_re()
        .replace_all(body, |caps: &regex::Captures| {
            let raw = &caps[0];
            let (url, trailing) = split_trailing_punctuation(raw);
            let domain = extract_domain(url).unwrap_or("");

            let replacement = if domain.is_empty() {
                "[link]".to_string()
            } else if is_trusted(domain) {
                url.to_string()
            } else if is_shortener(domain) {
                "[link]".to_string()
            } else {
                format!("[link: {}]", domain)
            };

            format!("{}{}", replacement, trailing)
        })
        .into_owned()
}

fn split_trailing_punctuation(url: &str) -> (&str, &str) {
    let trailing_chars: &[char] = &['.', ',', ';', '!', '?', ':'];
    let trim_at = url.trim_end_matches(trailing_chars).len();
    url.split_at(trim_at)
}

fn extract_domain(url: &str) -> Option<&str> {
    let after_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    let host = after_scheme.split(['/', '?', '#']).next()?;
    let host = host.split('@').next_back()?;
    let host = host.split(':').next()?;
    if host.is_empty() {
        None
    } else {
        Some(host)
    }
}

fn is_trusted(domain: &str) -> bool {
    let domain = domain.to_ascii_lowercase();
    TRUSTED_DOMAINS
        .iter()
        .any(|t| domain == *t || domain.ends_with(&format!(".{}", t)))
}

fn is_shortener(domain: &str) -> bool {
    let domain = domain.to_ascii_lowercase();
    SHORTENER_DOMAINS.iter().any(|s| domain == *s)
}
