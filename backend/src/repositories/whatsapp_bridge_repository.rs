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
use diesel::sql_types::Text;

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

pub struct WhatsAppBridgeRepository {
    pool: PgDbPool,
}

impl WhatsAppBridgeRepository {
    pub fn new(pool: PgDbPool) -> Self {
        Self { pool }
    }

    /// Look up the WhatsApp JID that a Matrix user is logged into the bridge as.
    /// Returns Ok(None) if the user is not currently logged in.
    ///
    /// `matrix_user_id` is the full Matrix ID, e.g. "@appuser_xxx:localhost".
    pub fn get_our_jid_for_matrix_user(
        &self,
        matrix_user_id: &str,
    ) -> Result<Option<String>, DieselError> {
        let mut conn = self
            .pool
            .get()
            .expect("Failed to get whatsapp_db connection");
        let rows: Vec<JidRow> =
            diesel::sql_query("SELECT id FROM user_logins WHERE user_mxid = $1 LIMIT 1")
                .bind::<Text, _>(matrix_user_id)
                .load(&mut conn)?;
        Ok(rows.into_iter().next().map(|r| r.id))
    }

    /// Fetch all WhatsApp contacts synced for a given `our_jid`.
    pub fn get_contacts_by_our_jid(
        &self,
        our_jid: &str,
    ) -> Result<Vec<WhatsAppContact>, DieselError> {
        let mut conn = self
            .pool
            .get()
            .expect("Failed to get whatsapp_db connection");
        let rows: Vec<ContactRow> = diesel::sql_query(
            "SELECT their_jid, first_name, full_name, push_name, business_name \
             FROM whatsmeow_contacts \
             WHERE our_jid = $1 \
             ORDER BY COALESCE(full_name, push_name, business_name, their_jid) \
             LIMIT 1000",
        )
        .bind::<Text, _>(our_jid)
        .load(&mut conn)?;

        Ok(rows.into_iter().map(row_to_contact).collect())
    }

    /// Convenience: resolve the JID for a Matrix user, then fetch contacts.
    /// Returns an empty vec if the user isn't logged in.
    pub fn get_contacts_for_matrix_user(
        &self,
        matrix_user_id: &str,
    ) -> Result<Vec<WhatsAppContact>, DieselError> {
        let Some(our_jid) = self.get_our_jid_for_matrix_user(matrix_user_id)? else {
            return Ok(Vec::new());
        };
        self.get_contacts_by_our_jid(&our_jid)
    }
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
