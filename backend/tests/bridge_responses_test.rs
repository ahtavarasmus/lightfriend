//! Exact-match tests for bridge bot response classifiers.
//!
//! Every expected-value string here is empirically verified against the live
//! bridges. If a bridge upgrade changes the wire format, these tests fail
//! first - which is correct behaviour; the new format must be re-verified by
//! hand-probing the bot before updating the constants.

use backend::utils::bridge_responses::{
    any_connected, classify_bridgev2_list_logins, classify_telegram_ping, first_connected,
    first_connected_identifier, first_connected_login_id, is_signal_list_logins_empty,
    is_signal_logout_success, is_telegram_logout_success, is_whatsapp_list_logins_empty,
    is_whatsapp_logout_not_found, is_whatsapp_logout_success, parse_list_logins,
    parse_list_logins_line, BridgeLoginHealth, ListLoginsEntry, TelegramPingStatus,
};

// --- telegram ping ---

#[test]
fn telegram_ping_logged_in_matches_verified_prefix() {
    // Verified body: "You're logged in as @<username>"
    let status = classify_telegram_ping("You're logged in as @exampleuser").unwrap();
    assert_eq!(
        status,
        TelegramPingStatus::LoggedIn {
            username: "exampleuser".to_string()
        }
    );
}

#[test]
fn telegram_ping_not_logged_in_matches_exact() {
    let status = classify_telegram_ping("You're not logged in.").unwrap();
    assert_eq!(status, TelegramPingStatus::NotLoggedIn);
}

#[test]
fn telegram_ping_missing_period_does_not_match() {
    // No trailing period - unrecognized.
    assert_eq!(classify_telegram_ping("You're not logged in"), None);
}

#[test]
fn telegram_ping_empty_username_rejected() {
    assert_eq!(classify_telegram_ping("You're logged in as @"), None);
}

#[test]
fn telegram_ping_multiline_username_rejected() {
    // A trailing newline payload is suspicious and should not match.
    assert_eq!(
        classify_telegram_ping("You're logged in as @foo\nextra"),
        None
    );
}

#[test]
fn telegram_ping_logout_success_does_not_classify_as_ping() {
    // The logout reply must never be confused with a ping reply.
    assert_eq!(classify_telegram_ping("Logged out successfully."), None);
}

#[test]
fn telegram_ping_help_does_not_match() {
    assert_eq!(
        classify_telegram_ping(
            "This is a management room: prefixing commands with `!tg` is not required."
        ),
        None
    );
}

// --- list-logins parsing ---

#[test]
fn whatsapp_connected_line_parses() {
    let entry = parse_list_logins_line("* `10000000000` (+10000000000) - `CONNECTED`").unwrap();
    assert_eq!(
        entry,
        ListLoginsEntry {
            login_id: "10000000000".to_string(),
            identifier: "+10000000000".to_string(),
            status: "CONNECTED".to_string(),
        }
    );
}

#[test]
fn signal_connected_line_parses() {
    let entry = parse_list_logins_line(
        "* `00000000-0000-0000-0000-000000000000` (+10000000000) - `CONNECTED`",
    )
    .unwrap();
    assert_eq!(entry.login_id, "00000000-0000-0000-0000-000000000000");
    assert_eq!(entry.identifier, "+10000000000");
    assert_eq!(entry.status, "CONNECTED");
}

#[test]
fn list_logins_empty_body_yields_empty_vec() {
    assert_eq!(parse_list_logins(""), Vec::<ListLoginsEntry>::new());
}

#[test]
fn list_logins_garbage_line_rejected() {
    assert_eq!(parse_list_logins_line("random bridge chatter"), None);
    assert_eq!(
        parse_list_logins_line("* connected without backticks - CONNECTED"),
        None
    );
}

#[test]
fn any_connected_true_on_connected_entry() {
    assert!(any_connected(
        "* `10000000000` (+10000000000) - `CONNECTED`"
    ));
}

#[test]
fn any_connected_false_on_logged_out_entry() {
    assert!(!any_connected("* `foo` (+1) - `LOGGED_OUT`"));
}

#[test]
fn any_connected_false_on_missing_backticks_around_status() {
    // Variant without backticks around CONNECTED is NOT verified, so reject.
    assert!(!any_connected("* `foo` (+1) - CONNECTED"));
}

#[test]
fn first_connected_identifier_returns_phone() {
    let ident = first_connected_identifier("* `abc` (+10000000000) - `CONNECTED`").unwrap();
    assert_eq!(ident, "+10000000000");
}

#[test]
fn first_connected_login_id_returns_wa_phone_without_plus() {
    // WhatsApp login_id is the phone without the leading `+`.
    let id = first_connected_login_id("* `10000000000` (+10000000000) - `CONNECTED`").unwrap();
    assert_eq!(id, "10000000000");
}

#[test]
fn first_connected_login_id_returns_signal_uuid() {
    // Signal login_id is a UUID.
    let id = first_connected_login_id(
        "* `00000000-0000-0000-0000-000000000000` (+10000000000) - `CONNECTED`",
    )
    .unwrap();
    assert_eq!(id, "00000000-0000-0000-0000-000000000000");
}

#[test]
fn first_connected_login_id_skips_disconnected_entries() {
    let body = "* `aaa` (+1) - `LOGGED_OUT`\n* `bbb` (+2) - `CONNECTED`";
    assert_eq!(first_connected_login_id(body).unwrap(), "bbb");
}

#[test]
fn first_connected_login_id_none_on_empty_state() {
    // The verified "not logged in" body is not parseable and has no logins.
    assert_eq!(first_connected_login_id("You're not logged in"), None);
    assert_eq!(first_connected_login_id(""), None);
}

#[test]
fn first_connected_picks_connected_among_mixed() {
    let body = "* `a` (+1) - `LOGGED_OUT`\n* `b` (+2) - `CONNECTED`\n* `c` (+3) - `RECONNECTING`";
    let entries = parse_list_logins(body);
    assert_eq!(entries.len(), 3);
    let connected = first_connected(&entries).unwrap();
    assert_eq!(connected.login_id, "b");
    assert_eq!(connected.identifier, "+2");
}

// --- logout classifiers ---

#[test]
fn telegram_logout_exact_match() {
    assert!(is_telegram_logout_success("Logged out successfully."));
}

#[test]
fn telegram_logout_wa_style_rejected() {
    // WA uses "Logged out" (no period). Must NOT match Telegram classifier.
    assert!(!is_telegram_logout_success("Logged out"));
}

#[test]
fn whatsapp_logout_exact_match() {
    assert!(is_whatsapp_logout_success("Logged out"));
}

#[test]
fn whatsapp_logout_tg_style_rejected() {
    // Telegram's "Logged out successfully." must NOT classify as WA success.
    assert!(!is_whatsapp_logout_success("Logged out successfully."));
}

#[test]
fn signal_logout_exact_match() {
    assert!(is_signal_logout_success("Logged out"));
}

#[test]
fn signal_logout_trailing_newline_rejected() {
    // Exact means exact.
    assert!(!is_signal_logout_success("Logged out\n"));
    assert!(!is_signal_logout_success(" Logged out"));
}

#[test]
fn whatsapp_logout_not_found_recognised() {
    assert!(is_whatsapp_logout_not_found(
        "Login `10000000000` not found"
    ));
    assert!(is_whatsapp_logout_not_found(
        "Login `00000000-0000-0000-0000-000000000000` not found"
    ));
}

#[test]
fn whatsapp_logout_not_found_rejects_neighbouring_variants() {
    // Missing leading prefix
    assert!(!is_whatsapp_logout_not_found("`abc` not found"));
    // Missing suffix
    assert!(!is_whatsapp_logout_not_found("Login `abc` missing"));
    // Empty id
    assert!(!is_whatsapp_logout_not_found("Login `` not found"));
    // Injected backtick inside id - suspicious, reject
    assert!(!is_whatsapp_logout_not_found("Login `a`b` not found"));
    // Must not classify as success
    assert!(!is_whatsapp_logout_success("Login `abc` not found"));
}

#[test]
fn whatsapp_list_logins_empty_exact_match() {
    // Verified body, no trailing period.
    assert!(is_whatsapp_list_logins_empty("You're not logged in"));
}

#[test]
fn signal_list_logins_empty_exact_match() {
    assert!(is_signal_list_logins_empty("You're not logged in"));
    assert!(!is_signal_list_logins_empty("You're not logged in."));
    assert!(!is_signal_list_logins_empty("not logged in"));
}

#[test]
fn whatsapp_list_logins_empty_rejects_variants() {
    // Trailing period is the TG style, must not match WA empty-state.
    assert!(!is_whatsapp_list_logins_empty("You're not logged in."));
    // Any other wording
    assert!(!is_whatsapp_list_logins_empty("not logged in"));
    // And any_connected should also return false for the empty-state body.
    assert!(!any_connected("You're not logged in"));
}

#[test]
fn logout_does_not_match_arbitrary_text_with_substring() {
    // Crucial regression: an earlier bug matched any body containing
    // "logged out" as a disconnection signal, which tripped on the normal
    // reply "You are not logged out" or similar. Exact matching means
    // partial substrings never count.
    assert!(!is_telegram_logout_success(
        "You have successfully Logged out of the bridge now."
    ));
    assert!(!is_whatsapp_logout_success("Already Logged out from phone"));
    assert!(!is_signal_logout_success(
        "Logged out.\nAlso see: !signal help"
    ));
}

// --- bridgev2 list-logins health classification ---
//
// Every body string below is empirically captured from the live
// mautrix-whatsapp v26.04 bridge via the admin probe endpoint. See
// project_matrix_lifecycle_review.md / session transcripts for the raw
// probe output. If these fail after a bridge version bump, re-probe and
// update constants in bridge_responses.rs first.

#[test]
fn bridgev2_health_connected() {
    // Verified: `!wa list-logins` while phone is linked.
    let body = "* `10000000000` (+10000000000) - `CONNECTED`";
    assert_eq!(
        classify_bridgev2_list_logins(body),
        BridgeLoginHealth::Connected
    );
}

#[test]
fn bridgev2_health_bad_credentials_after_phone_side_unlink() {
    // Verified on 2026-04-19: after unlinking lightfriend from WhatsApp
    // mobile's Linked Devices, the status token flipped from CONNECTED
    // to BAD_CREDENTIALS. No passive push was emitted - this is the sole
    // detection signal.
    let body = "* `10000000000` (+10000000000) - `BAD_CREDENTIALS`";
    assert_eq!(
        classify_bridgev2_list_logins(body),
        BridgeLoginHealth::BadCredentials
    );
}

#[test]
fn bridgev2_health_empty_when_never_linked() {
    // Verified: fresh bridge, no login ever completed.
    assert_eq!(
        classify_bridgev2_list_logins("You're not logged in"),
        BridgeLoginHealth::Empty
    );
}

#[test]
fn bridgev2_health_unknown_for_unparseable_body() {
    // Help text, errors, or future bridge output must not be treated as
    // either connected or disconnected.
    assert_eq!(
        classify_bridgev2_list_logins("Unknown command, use the `help` command for help."),
        BridgeLoginHealth::Unknown
    );
    assert_eq!(
        classify_bridgev2_list_logins(""),
        BridgeLoginHealth::Unknown
    );
}

#[test]
fn bridgev2_health_connected_wins_over_bad_credentials() {
    // Two parallel logins on same account: one CONNECTED, one revoked.
    // Working session exists, so health is Connected - we don't want to
    // tear down a bridge that still has a functional login.
    let body = "\
* `111111111111` (+111111111111) - `BAD_CREDENTIALS`
* `222222222222` (+222222222222) - `CONNECTED`";
    assert_eq!(
        classify_bridgev2_list_logins(body),
        BridgeLoginHealth::Connected
    );
}

#[test]
fn bridgev2_health_ignores_unknown_statuses_when_no_connected() {
    // If the bridge ever reports something we don't recognise (say a
    // future RECONNECTING status), and no CONNECTED or BAD_CREDENTIALS
    // entry exists, the result is Unknown - NOT BadCredentials. We
    // never auto-disconnect on an unknown status.
    let body = "* `10000000000` (+10000000000) - `RECONNECTING`";
    assert_eq!(
        classify_bridgev2_list_logins(body),
        BridgeLoginHealth::Unknown
    );
}

#[test]
fn bridgev2_health_bad_credentials_exact_token_required() {
    // Substring matches must not fire. Only exact `BAD_CREDENTIALS` counts.
    let body = "* `10000000000` (+10000000000) - `BAD_CREDENTIALS_EXTENDED`";
    assert_eq!(
        classify_bridgev2_list_logins(body),
        BridgeLoginHealth::Unknown
    );
}
