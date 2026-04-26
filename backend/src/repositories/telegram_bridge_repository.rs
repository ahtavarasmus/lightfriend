// Read-only access to mautrix-telegram's PostgreSQL database (telegram_db).
//
// Why this exists: mautrix-telegram (Python, v0.15.3) creates portal Matrix
// rooms lazily and many contacts never get a portal because the bridge has
// not been told to puppet them. The Matrix room list is therefore an
// incomplete view of who the user can message. This repo queries the
// bridge's own tables directly for the full contact + chat surface.
//
// The schema is owned by mautrix-telegram, NOT by Lightfriend. We use raw
// SQL (diesel::sql_query) and never write to this database.
//
// Key tables / columns (verified via /api/admin/telegram-bridge-schema-introspect
// against bridge_schema_version=18, which matches the v0.15.3 source):
//   "user"  : mxid (PK), tgid (UNIQUE), tg_username, tg_phone, ...
//   puppet  : id (PK = TelegramID), displayname, username, phone, custom_mxid
//   portal  : (tgid, tg_receiver) PK, peer_type, mxid (UNIQUE), title
//   contact : ("user", contact)  -- both BIGINT TelegramIDs; "user" needs quotes
//   user_portal : ("user", portal, portal_receiver)
//
// peer_type values: 'user' (DM), 'chat' (small group), 'channel' (broadcast).
// Self-chat (Saved Messages) = peer_type='user' AND tgid=tg_receiver.

use crate::PgDbPool;
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use diesel::sql_types::{BigInt, Text};

#[derive(Debug, Clone)]
pub struct TelegramContact {
    pub tgid: i64,
    pub displayname: Option<String>,
    pub username: Option<String>,
    pub phone: Option<String>,
}

impl TelegramContact {
    pub fn display_name(&self) -> String {
        if let Some(n) = self.displayname.as_deref().filter(|s| !s.is_empty()) {
            return n.to_string();
        }
        if let Some(u) = self.username.as_deref().filter(|s| !s.is_empty()) {
            return format!("@{}", u);
        }
        if let Some(p) = self.phone.as_deref().filter(|s| !s.is_empty()) {
            return p.to_string();
        }
        format!("tg:{}", self.tgid)
    }
}

/// Unified search result for the Telegram send path.
///
/// One row per possible target chat. `mxid` is `Some` once the bridge has
/// materialized the portal on Matrix; `None` for cold DMs. Group/self-chat
/// distinction is carried explicitly so the caller does not have to
/// re-classify by string parsing.
#[derive(Debug, Clone)]
pub struct ChatCandidate {
    pub tgid: i64,
    pub display_name: String,
    pub mxid: Option<String>,
    pub is_group: bool,
    pub is_self_chat: bool,
    /// 'user' | 'chat' | 'channel'
    pub peer_type: String,
}

#[derive(diesel::QueryableByName, Debug)]
struct UserTgidRow {
    #[diesel(sql_type = diesel::sql_types::Nullable<BigInt>)]
    tgid: Option<i64>,
}

#[derive(diesel::QueryableByName, Debug)]
struct ContactRow {
    #[diesel(sql_type = BigInt)]
    id: i64,
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    displayname: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    username: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    phone: Option<String>,
}

#[derive(diesel::QueryableByName, Debug)]
struct ChatCandidateRow {
    #[diesel(sql_type = BigInt)]
    tgid: i64,
    #[diesel(sql_type = Text)]
    peer_type: String,
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    title: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    mxid: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    pp_displayname: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    pp_username: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    pp_phone: Option<String>,
    #[diesel(sql_type = diesel::sql_types::Bool)]
    is_self_chat: bool,
}

#[derive(diesel::QueryableByName, Debug)]
struct PortalMxidRow {
    #[diesel(sql_type = diesel::sql_types::Nullable<Text>)]
    mxid: Option<String>,
}

#[derive(diesel::QueryableByName, Debug)]
struct SelfChatRow {
    #[diesel(sql_type = diesel::sql_types::Bool)]
    is_self: bool,
}

pub struct TelegramBridgeRepository {
    pool: PgDbPool,
}

impl TelegramBridgeRepository {
    pub fn new(pool: PgDbPool) -> Self {
        Self { pool }
    }

    /// Resolve the Matrix user's bridge-side Telegram user ID.
    /// Returns Ok(None) when the user has no row (never logged in) or the
    /// row exists but `tgid` is NULL (logged out).
    pub fn get_user_tgid(&self, matrix_user_id: &str) -> Result<Option<i64>, DieselError> {
        let mut conn = self
            .pool
            .get()
            .expect("Failed to get telegram_db connection");
        let rows: Vec<UserTgidRow> =
            diesel::sql_query(r#"SELECT tgid FROM "user" WHERE mxid = $1 LIMIT 1"#)
                .bind::<Text, _>(matrix_user_id)
                .load(&mut conn)?;

        let tgid = rows.into_iter().next().and_then(|r| r.tgid);
        tracing::info!(
            "telegram_bridge: get_user_tgid mxid={} -> {:?}",
            matrix_user_id,
            tgid
        );
        Ok(tgid)
    }

    /// All saved contacts for this user, joined with puppet for the friendly
    /// fields. This is the cold-DM surface — contacts who haven't been
    /// messaged yet won't have a portal but are still valid send targets.
    pub fn get_contacts_for_user(
        &self,
        user_tgid: i64,
    ) -> Result<Vec<TelegramContact>, DieselError> {
        let mut conn = self
            .pool
            .get()
            .expect("Failed to get telegram_db connection");
        let rows: Vec<ContactRow> = diesel::sql_query(
            r#"SELECT pp.id, pp.displayname, pp.username, pp.phone
               FROM contact c
               INNER JOIN puppet pp ON pp.id = c.contact
               WHERE c."user" = $1
               ORDER BY COALESCE(pp.displayname, pp.username, pp.phone, pp.id::text)
               LIMIT 1000"#,
        )
        .bind::<BigInt, _>(user_tgid)
        .load(&mut conn)?;

        tracing::info!(
            "telegram_bridge: get_contacts_for_user user_tgid={} -> {} contacts",
            user_tgid,
            rows.len()
        );

        Ok(rows
            .into_iter()
            .map(|r| TelegramContact {
                tgid: r.id,
                displayname: r.displayname,
                username: r.username,
                phone: r.phone,
            })
            .collect())
    }

    /// Unified DM + group + self-chat search for the send path.
    ///
    /// - DMs: every saved contact (LEFT JOIN portal for mxid). Covers cold DMs
    ///   that have no portal yet.
    /// - Self-chat (Saved Messages): the portal where tgid = tg_receiver = user_tgid.
    /// - Groups / channels: portals where tg_receiver = user_tgid AND peer_type
    ///   in ('chat', 'channel').
    ///
    /// The caller fuzzy-matches `display_name` against the user's query in
    /// Rust (same locale, same matching logic as person/contact search).
    pub fn search_chats_for_user(&self, user_tgid: i64) -> Result<Vec<ChatCandidate>, DieselError> {
        let mut conn = self
            .pool
            .get()
            .expect("Failed to get telegram_db connection");

        let rows: Vec<ChatCandidateRow> = diesel::sql_query(
            r#"
            -- DMs from saved contacts (with optional portal mxid)
            SELECT pp.id AS tgid,
                   'user'::text AS peer_type,
                   NULL::text AS title,
                   po.mxid AS mxid,
                   pp.displayname AS pp_displayname,
                   pp.username AS pp_username,
                   pp.phone AS pp_phone,
                   false AS is_self_chat
            FROM contact c
            INNER JOIN puppet pp ON pp.id = c.contact
            LEFT JOIN portal po
                   ON po.tgid = pp.id
                  AND po.tg_receiver = $1
                  AND po.peer_type = 'user'
            WHERE c."user" = $1
            UNION ALL
            -- Self-chat (Saved Messages)
            SELECT po.tgid AS tgid,
                   'user'::text AS peer_type,
                   NULL::text AS title,
                   po.mxid AS mxid,
                   pp.displayname AS pp_displayname,
                   pp.username AS pp_username,
                   pp.phone AS pp_phone,
                   true AS is_self_chat
            FROM portal po
            LEFT JOIN puppet pp ON pp.id = po.tgid
            WHERE po.tgid = $1
              AND po.tg_receiver = $1
              AND po.peer_type = 'user'
            UNION ALL
            -- Groups and channels the user is in
            SELECT po.tgid AS tgid,
                   po.peer_type AS peer_type,
                   po.title AS title,
                   po.mxid AS mxid,
                   NULL::text AS pp_displayname,
                   NULL::text AS pp_username,
                   NULL::text AS pp_phone,
                   false AS is_self_chat
            FROM portal po
            WHERE po.tg_receiver = $1
              AND po.peer_type IN ('chat', 'channel')
            "#,
        )
        .bind::<BigInt, _>(user_tgid)
        .load(&mut conn)?;

        let with_mxid = rows.iter().filter(|r| r.mxid.is_some()).count();
        tracing::info!(
            "telegram_bridge: search_chats_for_user user_tgid={} -> {} candidates ({} with mxid)",
            user_tgid,
            rows.len(),
            with_mxid
        );

        Ok(rows
            .into_iter()
            .map(|r| {
                let display_name = if r.is_self_chat {
                    "Saved Messages".to_string()
                } else if matches!(r.peer_type.as_str(), "chat" | "channel") {
                    r.title
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| format!("Group {}", r.tgid))
                } else {
                    // DM: prefer displayname, then @username, then phone, then tg:id
                    if let Some(n) = r.pp_displayname.as_deref().filter(|s| !s.is_empty()) {
                        n.to_string()
                    } else if let Some(u) = r.pp_username.as_deref().filter(|s| !s.is_empty()) {
                        format!("@{}", u)
                    } else if let Some(p) = r.pp_phone.as_deref().filter(|s| !s.is_empty()) {
                        p.to_string()
                    } else {
                        format!("tg:{}", r.tgid)
                    }
                };
                let is_group = matches!(r.peer_type.as_str(), "chat" | "channel");
                ChatCandidate {
                    tgid: r.tgid,
                    display_name,
                    mxid: r.mxid,
                    is_group,
                    is_self_chat: r.is_self_chat,
                    peer_type: r.peer_type,
                }
            })
            .collect())
    }

    /// Resolve a DM portal's Matrix room ID.
    ///
    /// Layout: portal row keyed by (tgid, tg_receiver) where tgid is the
    /// other user (the contact) and tg_receiver is the bridge user. Mxid is
    /// nullable — the row may exist before Matrix has been told about it.
    pub fn get_dm_portal_mxid(
        &self,
        user_tgid: i64,
        contact_tgid: i64,
    ) -> Result<Option<String>, DieselError> {
        let mut conn = self
            .pool
            .get()
            .expect("Failed to get telegram_db connection");
        let rows: Vec<PortalMxidRow> = diesel::sql_query(
            r#"SELECT mxid FROM portal
               WHERE tgid = $1 AND tg_receiver = $2 AND peer_type = 'user'
               LIMIT 1"#,
        )
        .bind::<BigInt, _>(contact_tgid)
        .bind::<BigInt, _>(user_tgid)
        .load(&mut conn)?;
        Ok(rows.into_iter().next().and_then(|r| r.mxid))
    }

    /// Resolve the user's Saved Messages portal Matrix room ID.
    /// Self-chat is encoded as peer_type='user' AND tgid=tg_receiver.
    pub fn get_self_chat_portal_mxid(&self, user_tgid: i64) -> Result<Option<String>, DieselError> {
        let mut conn = self
            .pool
            .get()
            .expect("Failed to get telegram_db connection");
        let rows: Vec<PortalMxidRow> = diesel::sql_query(
            r#"SELECT mxid FROM portal
               WHERE tgid = $1 AND tg_receiver = $1 AND peer_type = 'user'
               LIMIT 1"#,
        )
        .bind::<BigInt, _>(user_tgid)
        .load(&mut conn)?;
        Ok(rows.into_iter().next().and_then(|r| r.mxid))
    }

    /// Test whether a Matrix room is the user's Saved Messages.
    /// Used by handle_bridge_message to tag inbound events as self-chat
    /// without re-running heuristics on the room name.
    pub fn is_self_chat_portal(&self, mxid: &str) -> Result<bool, DieselError> {
        let mut conn = self
            .pool
            .get()
            .expect("Failed to get telegram_db connection");
        let rows: Vec<SelfChatRow> = diesel::sql_query(
            r#"SELECT (tgid = tg_receiver AND peer_type = 'user') AS is_self
               FROM portal
               WHERE mxid = $1
               LIMIT 1"#,
        )
        .bind::<Text, _>(mxid)
        .load(&mut conn)?;
        Ok(rows.into_iter().next().map(|r| r.is_self).unwrap_or(false))
    }
}
