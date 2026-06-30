// Read-only access to mautrix-whatsapp's PostgreSQL database (whatsapp_db).
//
// Why this exists: mautrix-whatsapp creates DM Matrix rooms lazily - only when
// a message is exchanged. Contacts who haven't messaged the user are invisible
// via the Matrix room list. To populate the rule builder dropdown with the
// full WhatsApp contact list, we query the bridge's own contact table directly.
//
// The schema is owned by mautrix-whatsapp / whatsmeow, NOT by Lightfriend.
// We use raw SQL (diesel::sql_query) and never write to this database.

use crate::PgDbPool;
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use diesel::sql_types::{BigInt, Text};

#[derive(Debug, Clone)]
pub struct WhatsAppContact {
    pub jid: String,           // e.g. "5218117978256@s.whatsapp.net"
    pub phone: Option<String>, // user-part of the JID, e.g. "5218117978256"
    pub full_name: Option<String>,
    pub first_name: Option<String>,
    pub push_name: Option<String>,
    pub business_name: Option<String>,
}

impl WhatsAppContact {
    /// Mirrors the displayname_template in enclave/configs/whatsapp.yaml.template:
    ///   {{if .FullName}}{{.FullName}}{{else}}{{or .PushName .BusinessName .Phone}}{{end}}
    pub fn display_name(&self) -> String {
        if let Some(n) = self.full_name.as_deref().filter(|s| !s.is_empty()) {
            return n.to_string();
        }
        if let Some(n) = self.push_name.as_deref().filter(|s| !s.is_empty()) {
            return n.to_string();
        }
        if let Some(n) = self.business_name.as_deref().filter(|s| !s.is_empty()) {
            return n.to_string();
        }
        if let Some(p) = self.phone.as_deref().filter(|s| !s.is_empty()) {
            return p.to_string();
        }
        self.jid.clone()
    }

    /// True when the contact is from the user's phone book (has a full_name).
    pub fn is_phone_contact(&self) -> bool {
        self.full_name
            .as_deref()
            .map(|s| !s.is_empty())
            .unwrap_or(false)
    }
}

#[derive(diesel::QueryableByName, Debug)]
struct ContactRow {
    #[diesel(sql_type = Text)]
    their_jid: String,
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    first_name: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    full_name: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    push_name: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    business_name: Option<String>,
}

#[derive(diesel::QueryableByName, Debug)]
struct JidRow {
    #[diesel(sql_type = Text)]
    id: String,
}

#[derive(diesel::QueryableByName, Debug)]
struct PortalMxidRow {
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    mxid: Option<String>,
}

#[derive(diesel::QueryableByName, Debug)]
struct PortalChatRow {
    #[diesel(sql_type = Text)]
    id: String,
    #[diesel(sql_type = Text)]
    receiver: String,
}

#[derive(diesel::QueryableByName, Debug)]
struct MuteEndTimeRow {
    #[diesel(sql_type = diesel::sql_types::Nullable<BigInt>)]
    mute_end_time: Option<i64>,
}

/// Unified search result: either a DM contact or a group portal.
///
/// Used by the send path to resolve `name_query -> (chat_id, mxid, is_group)`
/// in a single DB hit. `mxid` is `None` for DMs that haven't been messaged
/// yet (caller falls back to `!wa start-chat`) and generally `Some` for
/// groups the user is a member of.
#[derive(Debug, Clone)]
pub struct ChatCandidate {
    pub chat_id: String, // "<phone>@s.whatsapp.net" or "<gid>@g.us"
    pub display_name: String,
    pub is_group: bool,
    pub mxid: Option<String>,
}

#[derive(diesel::QueryableByName, Debug)]
struct ChatCandidateRow {
    #[diesel(sql_type = Text)]
    chat_id: String,
    #[diesel(sql_type = diesel::sql_types::Bool)]
    is_group: bool,
    #[diesel(sql_type = Text)]
    display_name: String,
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    mxid: Option<String>,
}

pub struct WhatsAppBridgeRepository {
    pool: PgDbPool,
}

impl WhatsAppBridgeRepository {
    pub fn new(pool: PgDbPool) -> Self {
        Self { pool }
    }

    /// Look up the WhatsApp phone number that a Matrix user is logged into the
    /// bridge as. Returns Ok(None) if the user is not currently logged in.
    ///
    /// Note: this returns the value stored in `user_logins.id`, which is
    /// produced by `waid.MakeUserLoginID` in mautrix-whatsapp and equals just
    /// the user-part of the JID — i.e. the phone number string like
    /// "5218127329906", NOT a full JID. The contact lookup below joins
    /// against `whatsmeow_device` to get the real JID.
    ///
    /// Tries both `user_logins` (pre-megabridge) and `user_login` (megabridge/
    /// bridgev2) table names since the mautrix-whatsapp version may vary.
    ///
    /// `matrix_user_id` is the full Matrix ID, e.g. "@appuser_xxx:localhost".
    pub fn get_login_phone_for_matrix_user(
        &self,
        matrix_user_id: &str,
    ) -> Result<Option<String>, DieselError> {
        let mut conn = self
            .pool
            .get()
            .expect("Failed to get whatsapp_db connection");
        // Try plural table name first (pre-megabridge mautrix-whatsapp)
        let rows: Result<Vec<JidRow>, _> =
            diesel::sql_query("SELECT id FROM user_logins WHERE user_mxid = $1 LIMIT 1")
                .bind::<Text, _>(matrix_user_id)
                .load(&mut conn);
        match rows {
            Ok(r) if !r.is_empty() => {
                tracing::info!(
                    "whatsapp_bridge: found login phone in user_logins for {}",
                    matrix_user_id
                );
                return Ok(Some(r.into_iter().next().unwrap().id));
            }
            Ok(_) => {
                tracing::info!(
                    "whatsapp_bridge: user_logins exists but no row for {}",
                    matrix_user_id
                );
            }
            Err(e) => {
                tracing::info!(
                    "whatsapp_bridge: user_logins query failed ({}), trying user_login (singular)",
                    e
                );
            }
        }
        // Try singular table name (megabridge/bridgev2 mautrix-whatsapp)
        let rows: Result<Vec<JidRow>, _> =
            diesel::sql_query("SELECT id FROM user_login WHERE user_mxid = $1 LIMIT 1")
                .bind::<Text, _>(matrix_user_id)
                .load(&mut conn);
        match rows {
            Ok(r) if !r.is_empty() => {
                tracing::info!(
                    "whatsapp_bridge: found login phone in user_login (singular) for {}",
                    matrix_user_id
                );
                return Ok(Some(r.into_iter().next().unwrap().id));
            }
            Ok(_) => {
                tracing::info!(
                    "whatsapp_bridge: user_login exists but no row for {}",
                    matrix_user_id
                );
            }
            Err(e) => {
                tracing::info!("whatsapp_bridge: user_login query also failed ({})", e);
            }
        }
        Ok(None)
    }

    /// Fetch all WhatsApp contacts synced for a user identified by their
    /// login phone (the value of `user_logins.id`).
    ///
    /// The contacts table (`whatsmeow_contacts.our_jid`) is populated by the
    /// whatsmeow library with the *full* JID of the logged-in device (via
    /// `types.JID.String()`), which looks like `"{phone}@{server}"` or
    /// `"{phone}:{device}@{server}"` — NOT just the phone. So we can't match
    /// by equality on the phone. Instead we JOIN through `whatsmeow_device`
    /// (the source of truth that `our_jid` has a FK to) and prefix-match the
    /// `phone` onto `whatsmeow_device.jid` with an explicit boundary char
    /// after it so `"1234"` can't false-match `"12345@..."` for another
    /// user.
    ///
    /// The two LIKE patterns together cover every shape emitted by whatsmeow's
    /// `JID.String()`:
    ///   - `"{phone}@{server}"`            (device == 0)
    ///   - `"{phone}:{device}@{server}"`   (device >  0)
    ///
    /// We don't hardcode the server domain so a future whatsmeow migration
    /// that renames `s.whatsapp.net` won't silently break this.
    pub fn get_contacts_by_login_phone(
        &self,
        phone: &str,
    ) -> Result<Vec<WhatsAppContact>, DieselError> {
        let mut conn = self
            .pool
            .get()
            .expect("Failed to get whatsapp_db connection");
        let rows: Vec<ContactRow> = diesel::sql_query(
            "SELECT c.their_jid, c.first_name, c.full_name, c.push_name, c.business_name \
             FROM whatsmeow_contacts c \
             INNER JOIN whatsmeow_device d ON c.our_jid = d.jid \
             WHERE d.jid LIKE $1 || '@%' \
                OR d.jid LIKE $1 || ':%@%' \
             ORDER BY COALESCE(c.full_name, c.push_name, c.business_name, c.their_jid) \
             LIMIT 1000",
        )
        .bind::<Text, _>(phone)
        .load(&mut conn)?;

        tracing::info!(
            "whatsapp_bridge: get_contacts_by_login_phone phone_len={} -> {} contacts",
            phone.len(),
            rows.len()
        );

        Ok(rows.into_iter().map(row_to_contact).collect())
    }

    /// Fetch all contacts directly without user filtering.
    /// Safe for single-user enclave setups where only one user's contacts exist.
    pub fn get_all_contacts(&self) -> Result<Vec<WhatsAppContact>, DieselError> {
        let mut conn = self
            .pool
            .get()
            .expect("Failed to get whatsapp_db connection");
        let rows: Vec<ContactRow> = diesel::sql_query(
            "SELECT their_jid, first_name, full_name, push_name, business_name \
             FROM whatsmeow_contacts \
             WHERE their_jid LIKE '%@s.whatsapp.net' \
             ORDER BY COALESCE(full_name, push_name, business_name, their_jid) \
             LIMIT 1000",
        )
        .load(&mut conn)?;

        tracing::info!(
            "whatsapp_bridge: get_all_contacts -> {} contacts",
            rows.len()
        );

        Ok(rows.into_iter().map(row_to_contact).collect())
    }

    /// Unified DM+group search for the send path.
    ///
    /// Returns one `ChatCandidate` per possible target chat for this login,
    /// with `mxid` pre-fetched via LEFT JOIN on the `portal` table. The
    /// caller fuzzy-matches `display_name` against the user's query in
    /// Rust (no ILIKE in SQL so we can use the same matching logic as
    /// contacts-only search and avoid Postgres locale surprises).
    ///
    /// - DMs come from `whatsmeow_contacts` joined to `portal` for optional
    ///   mxid.
    /// - Groups come from `portal` directly (they don't exist in
    ///   whatsmeow_contacts; user's membership is implicit via whatsmeow
    ///   syncing them on login).
    ///
    /// For DMs `mxid` is `None` until the user has exchanged at least one
    /// message with that contact. For groups the user is a member of, the
    /// bridge materializes portals on login-time sync so `mxid` is almost
    /// always `Some`.
    pub fn search_chats_for_login(
        &self,
        login_phone: &str,
    ) -> Result<Vec<ChatCandidate>, DieselError> {
        let mut conn = self
            .pool
            .get()
            .expect("Failed to get whatsapp_db connection");
        let rows: Vec<ChatCandidateRow> = diesel::sql_query(
            "\
            SELECT c.their_jid AS chat_id, \
                   false AS is_group, \
                   COALESCE(NULLIF(c.full_name, ''), NULLIF(c.push_name, ''), NULLIF(c.business_name, ''), c.their_jid) AS display_name, \
                   p.mxid AS mxid \
            FROM whatsmeow_contacts c \
            INNER JOIN whatsmeow_device d ON c.our_jid = d.jid \
            LEFT JOIN portal p \
              ON p.bridge_id = 'whatsapp' AND p.id = c.their_jid \
                 AND (p.receiver = $1 OR p.receiver = '') \
            WHERE (d.jid LIKE $1 || '@%' OR d.jid LIKE $1 || ':%@%') \
              AND c.their_jid LIKE '%@s.whatsapp.net' \
            UNION ALL \
            SELECT p.id AS chat_id, \
                   true AS is_group, \
                   p.name AS display_name, \
                   p.mxid AS mxid \
            FROM portal p \
            WHERE p.bridge_id = 'whatsapp' \
              AND p.id LIKE '%@g.us' \
              AND (p.receiver = $1 OR p.receiver = '')",
        )
        .bind::<Text, _>(login_phone)
        .load(&mut conn)?;

        tracing::info!(
            "whatsapp_bridge: search_chats_for_login -> {} candidates ({} with mxid)",
            rows.len(),
            rows.iter().filter(|r| r.mxid.is_some()).count(),
        );

        Ok(rows
            .into_iter()
            .map(|r| ChatCandidate {
                chat_id: r.chat_id,
                display_name: r.display_name,
                is_group: r.is_group,
                mxid: r.mxid,
            })
            .collect())
    }

    /// Look up the Matrix room ID for a given chat on this WA login.
    ///
    /// Source of truth: mautrix-whatsapp's bridgev2 `portal` table
    /// (schema: github.com/mautrix/go `bridgev2/database/upgrades/00-latest.sql`).
    /// The table is keyed by `(bridge_id, id, receiver)` where:
    /// - `bridge_id` = constant "whatsapp" for this bridge
    /// - `id` = WhatsApp chat_id (jid), e.g. "358442055570@s.whatsapp.net"
    ///   for a DM or "<gid>@g.us" for a group
    /// - `receiver` = user's login_id (bare phone string). Can also be ''
    ///   for global portals (legacy), so we match either.
    /// - `mxid` = the Matrix room ID, nullable until the portal is
    ///   materialized on Matrix side.
    ///
    /// Returns:
    /// - `Ok(Some(mxid))` - portal materialized, ready to send.
    /// - `Ok(None)` - portal row missing OR present but mxid is NULL.
    ///   Caller should treat both as "not yet materialized" and fall back
    ///   to `!wa start-chat` for DMs.
    /// - `Err(_)` - DB error (e.g. table schema surprise).
    pub fn get_portal_mxid(
        &self,
        chat_id: &str,
        login_phone: &str,
    ) -> Result<Option<String>, DieselError> {
        let mut conn = self
            .pool
            .get()
            .expect("Failed to get whatsapp_db connection");
        let rows: Vec<PortalMxidRow> = diesel::sql_query(
            "SELECT mxid FROM portal \
             WHERE bridge_id = 'whatsapp' \
               AND id = $1 \
               AND (receiver = $2 OR receiver = '') \
             LIMIT 1",
        )
        .bind::<Text, _>(chat_id)
        .bind::<Text, _>(login_phone)
        .load(&mut conn)?;

        Ok(rows.into_iter().next().and_then(|r| r.mxid))
    }

    pub fn is_room_muted(&self, matrix_user_id: &str, room_id: &str) -> Result<bool, DieselError> {
        let mut conn = self
            .pool
            .get()
            .expect("Failed to get whatsapp_db connection");
        let portals: Vec<PortalChatRow> = diesel::sql_query(
            "SELECT id, receiver FROM portal \
             WHERE bridge_id = 'whatsapp' AND mxid = $1 \
             LIMIT 1",
        )
        .bind::<Text, _>(room_id)
        .load(&mut conn)?;
        let Some(portal) = portals.into_iter().next() else {
            return Ok(false);
        };

        let PortalChatRow {
            id: chat_id,
            receiver,
        } = portal;
        let mut login_id = receiver;
        drop(conn);
        if login_id.is_empty() {
            let Some(fallback) = self.get_login_phone_for_matrix_user(matrix_user_id)? else {
                return Ok(false);
            };
            login_id = fallback;
        }

        let mut conn = self
            .pool
            .get()
            .expect("Failed to get whatsapp_db connection");
        let rows: Vec<MuteEndTimeRow> = diesel::sql_query(
            "SELECT mute_end_time FROM whatsapp_history_sync_conversation \
             WHERE bridge_id = 'whatsapp' \
               AND user_login_id = $1 \
               AND chat_jid = $2 \
             LIMIT 1",
        )
        .bind::<Text, _>(&login_id)
        .bind::<Text, _>(&chat_id)
        .load(&mut conn)?;
        let mute_end_time = rows
            .into_iter()
            .next()
            .and_then(|r| r.mute_end_time)
            .unwrap_or(0);
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        Ok(whatsapp_mute_is_active(mute_end_time, now_secs))
    }

    /// Convenience: resolve the login phone for a Matrix user, then fetch
    /// contacts. Falls back to fetching all contacts if user lookup fails
    /// (safe in single-user enclave).
    pub fn get_contacts_for_matrix_user(
        &self,
        matrix_user_id: &str,
    ) -> Result<Vec<WhatsAppContact>, DieselError> {
        // Try the proper path: user_login(s) -> phone -> contacts via device JOIN
        match self.get_login_phone_for_matrix_user(matrix_user_id) {
            Ok(Some(phone)) => {
                let contacts = self.get_contacts_by_login_phone(&phone)?;
                if !contacts.is_empty() {
                    tracing::info!(
                        "WHATSAPP_CONTACT_RESULT: method=login_phone, count={}, matrix_user={}",
                        contacts.len(),
                        matrix_user_id
                    );
                    return Ok(contacts);
                }
                tracing::info!(
                    "whatsapp_bridge: phone lookup returned 0 contacts, trying direct query"
                );
            }
            Ok(None) => {
                tracing::info!(
                    "whatsapp_bridge: no login row for {}, trying direct contact query",
                    matrix_user_id
                );
            }
            Err(e) => {
                tracing::warn!(
                    "whatsapp_bridge: login lookup failed for {}: {}, trying direct contact query",
                    matrix_user_id,
                    e
                );
            }
        }
        // Fallback: query all contacts directly (safe for single-user enclave)
        let contacts = self.get_all_contacts()?;
        tracing::info!(
            "WHATSAPP_CONTACT_RESULT: method=direct_all_contacts, count={}, matrix_user={}",
            contacts.len(),
            matrix_user_id
        );
        Ok(contacts)
    }
}

pub fn whatsapp_mute_is_active(mute_end_time: i64, now_secs: i64) -> bool {
    if mute_end_time < 0 {
        return true;
    }
    let mute_end_secs = if mute_end_time > 10_000_000_000 {
        mute_end_time / 1000
    } else {
        mute_end_time
    };
    mute_end_secs > now_secs
}

fn row_to_contact(row: ContactRow) -> WhatsAppContact {
    let phone = extract_phone_from_jid(&row.their_jid);
    WhatsAppContact {
        jid: row.their_jid,
        phone,
        full_name: row.full_name,
        first_name: row.first_name,
        push_name: row.push_name,
        business_name: row.business_name,
    }
}

/// Extract the user-part of a WhatsApp JID. For "5218117978256@s.whatsapp.net"
/// this returns "5218117978256". Returns None for non-individual JIDs.
fn extract_phone_from_jid(jid: &str) -> Option<String> {
    let (user, server) = jid.split_once('@')?;
    if server != "s.whatsapp.net" {
        return None;
    }
    // Some JIDs have a device suffix like "5218117978256.0:0". Strip it.
    let phone = user.split(['.', ':']).next().unwrap_or(user);
    if phone.is_empty() {
        None
    } else {
        Some(phone.to_string())
    }
}
