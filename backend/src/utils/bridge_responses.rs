//! Empirically verified bridge bot responses.
//!
//! Every constant and parser in this file is backed by a hand-probed response
//! captured from the live deployment. No fuzzy substring matching. No
//! speculative patterns. Classification functions return `None` when input
//! does not match the verified format exactly; the caller must treat `None`
//! as "unknown state" and never as a positive or negative signal.
//!
//! Probed bridge versions (see project notes):
//! - Telegram:  mautrix-telegram v0.15.3 (Python bridge, older)
//! - WhatsApp:  mautrix-whatsapp v26.04 (bridgev2)
//! - Signal:    mautrix-signal v26.04 (bridgev2)
//!
//! Signal's management room requires commands to be prefixed with `!signal`.
//! WhatsApp and Telegram management rooms accept bare commands too.

/// Exact response bodies observed from each bridge.
///
/// These strings are copied byte-for-byte from probe output. Variable portions
/// (username, phone, uuid) are marked by `_PREFIX` suffix in the name and are
/// validated via prefix-strip in the classifier functions below.
pub mod verified {
    // mautrix-telegram v0.15.3 (Python)
    pub mod telegram {
        /// `!tg ping` when logged in: body starts with this prefix. The
        /// username that follows is variable (Telegram @username).
        ///
        /// Full observed form: `You're logged in as @<username>`
        pub const PING_LOGGED_IN_PREFIX: &str = "You're logged in as @";

        /// `!tg ping` when not logged in: exact body.
        pub const PING_NOT_LOGGED_IN: &str = "You're not logged in.";

        /// `!tg logout` on success: exact body.
        pub const LOGOUT_SUCCESS: &str = "Logged out successfully.";
    }

    // mautrix-whatsapp v26.04 (bridgev2)
    pub mod whatsapp {
        /// `!wa logout <id>` on success: exact body.
        pub const LOGOUT_SUCCESS: &str = "Logged out";

        /// `!wa logout <id>` when no such login exists: the body includes the
        /// id the caller passed back in backticks, so this is a prefix+suffix
        /// pair rather than a single constant. Use
        /// `is_whatsapp_logout_not_found` below.
        pub const LOGOUT_NOT_FOUND_PREFIX: &str = "Login `";
        pub const LOGOUT_NOT_FOUND_SUFFIX: &str = "` not found";

        /// `!wa list-logins` when no logins are bound: exact body (no period).
        pub const LIST_LOGINS_EMPTY: &str = "You're not logged in";
    }

    // mautrix-signal v26.04 (bridgev2)
    pub mod signal {
        /// `!signal logout <id>` on success: exact body.
        pub const LOGOUT_SUCCESS: &str = "Logged out";

        /// `!signal list-logins` when no logins are bound: exact body.
        /// Matches the WhatsApp string byte-for-byte, but documented
        /// separately because nothing in the bridgev2 contract guarantees
        /// they stay in sync.
        pub const LIST_LOGINS_EMPTY: &str = "You're not logged in";
    }

    // Shared across bridgev2 (whatsapp + signal) list-logins format.
    pub mod bridgev2 {
        /// Status suffix on a CONNECTED login line.
        pub const STATUS_CONNECTED: &str = "CONNECTED";

        /// Status suffix when the bridge's session has been invalidated by
        /// the upstream service. Verified on mautrix-whatsapp v26.04 on
        /// 2026-04-19 by unlinking lightfriend from the WhatsApp mobile
        /// client's "Linked Devices" list and then probing `!wa list-logins`:
        /// the status token flipped from `CONNECTED` to `BAD_CREDENTIALS`.
        /// No passive push from the bot when this happens; the only way to
        /// detect it is to issue `list-logins` and observe the token change.
        /// Presence of this status means the user must re-pair.
        pub const STATUS_BAD_CREDENTIALS: &str = "BAD_CREDENTIALS";
    }
}

/// Classified `!tg ping` response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TelegramPingStatus {
    LoggedIn { username: String },
    NotLoggedIn,
}

/// Strict classifier for `!tg ping` responses.
///
/// Returns:
/// - `Some(LoggedIn { username })` if body starts with the verified prefix
///   and a non-empty username follows.
/// - `Some(NotLoggedIn)` if body is exactly the verified not-logged-in string.
/// - `None` for anything else. The caller MUST NOT treat `None` as either
///   positive or negative - only as "unrecognized, do not change state".
pub fn classify_telegram_ping(body: &str) -> Option<TelegramPingStatus> {
    use verified::telegram::*;
    if body == PING_NOT_LOGGED_IN {
        return Some(TelegramPingStatus::NotLoggedIn);
    }
    if let Some(rest) = body.strip_prefix(PING_LOGGED_IN_PREFIX) {
        if !rest.is_empty() && !rest.contains('\n') {
            return Some(TelegramPingStatus::LoggedIn {
                username: rest.to_string(),
            });
        }
    }
    None
}

/// A single parsed entry from a bridgev2 `list-logins` response.
///
/// Verified line format:
///   `* `<login_id>` (<identifier>) - `<STATUS>``
///
/// Example WhatsApp: `* `10000000000` (+10000000000) - `CONNECTED``
/// Example Signal:   `* `00000000-0000-0000-0000-000000000000` (+10000000000) - `CONNECTED``
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListLoginsEntry {
    pub login_id: String,
    pub identifier: String,
    pub status: String,
}

/// Parse one line of a bridgev2 `list-logins` response. Returns `None` if the
/// line does not match the verified format exactly. The caller should feed
/// body.lines() through this and ignore `None` results (whitespace, blank
/// lines, unexpected output).
pub fn parse_list_logins_line(line: &str) -> Option<ListLoginsEntry> {
    let line = line.trim();
    // Verified prefix: "* `"
    let rest = line.strip_prefix("* `")?;
    // login_id ends at the next backtick
    let (login_id, rest) = rest.split_once('`')?;
    if login_id.is_empty() {
        return None;
    }
    // Then " ("
    let rest = rest.strip_prefix(" (")?;
    // identifier ends at ")"
    let (identifier, rest) = rest.split_once(')')?;
    if identifier.is_empty() {
        return None;
    }
    // Then " - `"
    let rest = rest.strip_prefix(" - `")?;
    // status ends at trailing backtick, nothing after
    let status = rest.strip_suffix('`')?;
    if status.is_empty() {
        return None;
    }
    Some(ListLoginsEntry {
        login_id: login_id.to_string(),
        identifier: identifier.to_string(),
        status: status.to_string(),
    })
}

/// Parse a full bridgev2 `list-logins` body into a vec of entries. Unknown
/// lines (help text, errors) are skipped; only lines that match the verified
/// format are returned. An empty result is the valid representation of
/// "no logins".
pub fn parse_list_logins(body: &str) -> Vec<ListLoginsEntry> {
    body.lines().filter_map(parse_list_logins_line).collect()
}

/// Return the first entry whose status is exactly `CONNECTED`. This is the
/// only positive signal we emit from `list-logins` today.
pub fn first_connected(entries: &[ListLoginsEntry]) -> Option<&ListLoginsEntry> {
    entries
        .iter()
        .find(|e| e.status == verified::bridgev2::STATUS_CONNECTED)
    // Note: exact equality against "CONNECTED". Any other status
    // (LOGGED_OUT, RECONNECTING, whatever) is ignored here - callers that
    // want to distinguish those should inspect entries directly.
}

/// Strictly classified bridgev2 login health, derived from `list-logins` output.
///
/// Four cases matter for the health checker:
///
/// - `Connected`: at least one entry reports `CONNECTED`. Healthy.
/// - `BadCredentials`: no CONNECTED entry, and at least one entry reports
///   `BAD_CREDENTIALS`. Verified signal that the upstream service
///   invalidated the session (e.g. user unlinked lightfriend from WhatsApp
///   mobile's Linked Devices). Disconnect is confirmed.
/// - `Empty`: the bridge has no logins at all (e.g. user already logged out
///   via our own flow, or never logged in).
/// - `Unknown`: something else (other statuses, help text, parse failures).
///   Caller MUST NOT treat as a disconnect signal - log and leave state
///   unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BridgeLoginHealth {
    Connected,
    BadCredentials,
    Empty,
    Unknown,
}

/// Classify the health of a bridgev2 bridge from the full `list-logins`
/// response body.
///
/// Verified against mautrix-whatsapp v26.04. Same parser shape works for
/// mautrix-signal v26.04 because both bridges share the bridgev2
/// list-logins format.
///
/// Preference order is Connected > BadCredentials > Empty > Unknown: if any
/// login is currently CONNECTED we report Connected even when other logins
/// on the same account are in other states, because the user has a working
/// session. BadCredentials is only returned when there are no working
/// sessions and at least one session is explicitly revoked.
pub fn classify_bridgev2_list_logins(body: &str) -> BridgeLoginHealth {
    // `is_whatsapp_list_logins_empty` / `is_signal_list_logins_empty` share
    // the same body; match first so we don't hand empty bodies to the parser.
    let trimmed = body.trim();
    if trimmed == verified::whatsapp::LIST_LOGINS_EMPTY {
        return BridgeLoginHealth::Empty;
    }

    let entries = parse_list_logins(body);
    if entries.is_empty() {
        // Parser rejected every line. Could be help text, an error, or
        // a bridge version change. Don't treat as either healthy or
        // disconnected; that's the "Unknown" contract.
        return BridgeLoginHealth::Unknown;
    }
    if entries
        .iter()
        .any(|e| e.status == verified::bridgev2::STATUS_CONNECTED)
    {
        return BridgeLoginHealth::Connected;
    }
    if entries
        .iter()
        .any(|e| e.status == verified::bridgev2::STATUS_BAD_CREDENTIALS)
    {
        return BridgeLoginHealth::BadCredentials;
    }
    BridgeLoginHealth::Unknown
}

/// Convenience: does any parsed login show CONNECTED?
pub fn any_connected(body: &str) -> bool {
    first_connected(&parse_list_logins(body)).is_some()
}

/// Convenience: identifier (usually phone number) of the first CONNECTED login.
///
/// The identifier is the value inside parentheses in the `list-logins` line,
/// e.g. `+10000000000` for WhatsApp. It is the user-facing label. It is
/// NOT suitable as an argument to `!<prefix> logout` - use
/// `first_connected_login_id` for that.
pub fn first_connected_identifier(body: &str) -> Option<String> {
    first_connected(&parse_list_logins(body)).map(|e| e.identifier.clone())
}

/// Bridge-internal login id of the first CONNECTED login (the value inside
/// the FIRST backtick pair on a `list-logins` line). Use this as the argument
/// to `!<prefix> logout <login_id>`.
///
/// Shape is bridge-specific:
/// - WhatsApp: phone number without the leading `+` (e.g. `10000000000`)
/// - Signal:   UUID (e.g. `00000000-0000-0000-0000-000000000000`)
///
/// Empirically, passing the identifier (the `+`-prefixed phone from parens)
/// to logout returns `` `Login `+…` not found` ``, so this distinction
/// matters.
pub fn first_connected_login_id(body: &str) -> Option<String> {
    first_connected(&parse_list_logins(body)).map(|e| e.login_id.clone())
}

/// Strict classifier for logout responses. Returns true ONLY if the body
/// exactly matches the verified success string for the given bridge.
///
/// Any other body (including empty, help text, errors) returns false.
pub fn is_telegram_logout_success(body: &str) -> bool {
    body == verified::telegram::LOGOUT_SUCCESS
}

pub fn is_whatsapp_logout_success(body: &str) -> bool {
    body == verified::whatsapp::LOGOUT_SUCCESS
}

/// Recognise the `!wa logout <id>` error response when the id doesn't match a
/// known login. Body format: `Login \`<id>\` not found`.
pub fn is_whatsapp_logout_not_found(body: &str) -> bool {
    use verified::whatsapp::{LOGOUT_NOT_FOUND_PREFIX, LOGOUT_NOT_FOUND_SUFFIX};
    let Some(rest) = body.strip_prefix(LOGOUT_NOT_FOUND_PREFIX) else {
        return false;
    };
    let Some(id) = rest.strip_suffix(LOGOUT_NOT_FOUND_SUFFIX) else {
        return false;
    };
    !id.is_empty() && !id.contains('`') && !id.contains('\n')
}

/// Recognise the `!wa list-logins` empty-state body (user is not logged in).
pub fn is_whatsapp_list_logins_empty(body: &str) -> bool {
    body == verified::whatsapp::LIST_LOGINS_EMPTY
}

/// Recognise the `!signal list-logins` empty-state body.
pub fn is_signal_list_logins_empty(body: &str) -> bool {
    body == verified::signal::LIST_LOGINS_EMPTY
}

pub fn is_signal_logout_success(body: &str) -> bool {
    body == verified::signal::LOGOUT_SUCCESS
}
