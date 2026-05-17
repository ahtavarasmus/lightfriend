use diesel::prelude::*;
use diesel::result::Error as DieselError;

use crate::models::commitment_models::{
    CommitmentLabelEmbedding, CommitmentPrompt, CommitmentSenderRule, NewCommitmentLabelEmbedding,
    NewCommitmentPrompt, NewCommitmentSenderRule,
};
use crate::pg_schema::{commitment_label_embeddings, commitment_prompts, commitment_sender_rules};
use crate::PgDbPool;

pub const RULE_MUTE: &str = "mute";
pub const RULE_ALWAYS_TRACK: &str = "always_track";

pub const LABEL_TRACK: &str = "track";
pub const LABEL_WRONG: &str = "wrong";

pub const RULE_SOURCE_SMS: &str = "sms_reply";
pub const RULE_SOURCE_DASHBOARD: &str = "dashboard";

/// User-visible SMS reply values mapped to the 2x2 signal grid.
/// 1 = track this message, 2 = always track from this sender,
/// 3 = mute sender, 4 = not a commitment.
pub const REPLY_TRACK: &str = "1";
pub const REPLY_ALWAYS: &str = "2";
pub const REPLY_MUTE: &str = "3";
pub const REPLY_WRONG: &str = "4";

/// A bare `1/2/3/4` SMS reply older than this is no longer eligible to match
/// an unresolved prompt - the user might have meant something else entirely
/// hours later. Forces them to act on prompts while context is fresh and
/// prevents accidental hijacking of a stale prompt by an unrelated reply.
pub const REPLY_MATCH_WINDOW_SECS: i32 = 60 * 60;

pub struct CommitmentRepository {
    pub pool: PgDbPool,
}

impl CommitmentRepository {
    pub fn new(pool: PgDbPool) -> Self {
        Self { pool }
    }

    fn now() -> i32 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i32
    }

    // -------------------------------------------------------------------------
    // Sender rules
    // -------------------------------------------------------------------------

    /// Look up the active sender rule for a given (user, platform, sender_key).
    /// Returns None when no active rule exists. If multiple rules exist (e.g.,
    /// mute and always_track for the same sender, which shouldn't normally
    /// happen but is possible across history), the most recent wins.
    pub fn lookup_sender_rule(
        &self,
        user_id: i32,
        platform: &str,
        sender_key: &str,
    ) -> Result<Option<CommitmentSenderRule>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        commitment_sender_rules::table
            .filter(commitment_sender_rules::user_id.eq(user_id))
            .filter(commitment_sender_rules::platform.eq(platform))
            .filter(commitment_sender_rules::sender_key.eq(sender_key))
            .filter(commitment_sender_rules::active.eq(true))
            .order(commitment_sender_rules::created_at.desc())
            .first::<CommitmentSenderRule>(&mut conn)
            .optional()
    }

    /// Insert a new active sender rule. Caller should typically deactivate any
    /// pre-existing rule for the same (user, platform, sender_key) first so
    /// lookups are unambiguous.
    pub fn create_sender_rule(
        &self,
        rule: &NewCommitmentSenderRule,
    ) -> Result<CommitmentSenderRule, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::insert_into(commitment_sender_rules::table)
            .values(rule)
            .get_result(&mut conn)
    }

    /// Deactivate any active rules for a (user, platform, sender_key). Used
    /// before creating a new rule so the old one doesn't shadow the new one.
    pub fn deactivate_existing_rules(
        &self,
        user_id: i32,
        platform: &str,
        sender_key: &str,
    ) -> Result<usize, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let now = Self::now();
        diesel::update(
            commitment_sender_rules::table
                .filter(commitment_sender_rules::user_id.eq(user_id))
                .filter(commitment_sender_rules::platform.eq(platform))
                .filter(commitment_sender_rules::sender_key.eq(sender_key))
                .filter(commitment_sender_rules::active.eq(true)),
        )
        .set((
            commitment_sender_rules::active.eq(false),
            commitment_sender_rules::deactivated_at.eq(Some(now)),
        ))
        .execute(&mut conn)
    }

    /// Upsert a sender rule: deactivate any existing active rules for that
    /// (user, platform, sender_key), then insert the new one. Returns the new
    /// rule row.
    pub fn upsert_sender_rule(
        &self,
        user_id: i32,
        platform: &str,
        sender_key: &str,
        rule_type: &str,
        source: &str,
    ) -> Result<CommitmentSenderRule, DieselError> {
        self.deactivate_existing_rules(user_id, platform, sender_key)?;
        let rule = NewCommitmentSenderRule {
            user_id,
            platform: platform.to_string(),
            sender_key: sender_key.to_string(),
            rule_type: rule_type.to_string(),
            source: source.to_string(),
            active: true,
            created_at: Self::now(),
        };
        self.create_sender_rule(&rule)
    }

    /// List a user's active rules of a given type (e.g. all muted senders).
    pub fn list_active_rules(
        &self,
        user_id: i32,
        rule_type: &str,
    ) -> Result<Vec<CommitmentSenderRule>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        commitment_sender_rules::table
            .filter(commitment_sender_rules::user_id.eq(user_id))
            .filter(commitment_sender_rules::rule_type.eq(rule_type))
            .filter(commitment_sender_rules::active.eq(true))
            .order(commitment_sender_rules::created_at.desc())
            .load(&mut conn)
    }

    /// Deactivate a specific rule (e.g. user removes it from the dashboard).
    pub fn deactivate_rule(&self, user_id: i32, rule_id: i32) -> Result<usize, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let now = Self::now();
        diesel::update(
            commitment_sender_rules::table
                .filter(commitment_sender_rules::id.eq(rule_id))
                .filter(commitment_sender_rules::user_id.eq(user_id))
                .filter(commitment_sender_rules::active.eq(true)),
        )
        .set((
            commitment_sender_rules::active.eq(false),
            commitment_sender_rules::deactivated_at.eq(Some(now)),
        ))
        .execute(&mut conn)
    }

    // -------------------------------------------------------------------------
    // SMS prompts (live + history)
    // -------------------------------------------------------------------------

    /// Insert a new prompt row. Returns `Ok(None)` when the unique partial
    /// index on (user_id, platform, sender_key) WHERE resolved_at IS NULL
    /// already covers an active prompt - the race lost. Callers should treat
    /// that as "another worker already sent the SMS" and skip both the
    /// outbound and the local row.
    pub fn create_prompt(
        &self,
        prompt: &NewCommitmentPrompt,
    ) -> Result<Option<CommitmentPrompt>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        match diesel::insert_into(commitment_prompts::table)
            .values(prompt)
            .get_result::<CommitmentPrompt>(&mut conn)
        {
            Ok(p) => Ok(Some(p)),
            Err(DieselError::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                _,
            )) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Most recently-sent unresolved prompt for a user *within the reply
    /// window*. Anything older than `REPLY_MATCH_WINDOW_SECS` is excluded so
    /// a bare `1/2/3/4` reply can't accidentally fire on a forgotten prompt
    /// from yesterday.
    pub fn find_latest_unresolved_for_user(
        &self,
        user_id: i32,
    ) -> Result<Option<CommitmentPrompt>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let cutoff = Self::now() - REPLY_MATCH_WINDOW_SECS;
        commitment_prompts::table
            .filter(commitment_prompts::user_id.eq(user_id))
            .filter(commitment_prompts::resolved_at.is_null())
            .filter(commitment_prompts::sent_at.ge(cutoff))
            .order(commitment_prompts::sent_at.desc())
            .first::<CommitmentPrompt>(&mut conn)
            .optional()
    }

    /// Live prompt for a (user, platform, sender_key) - used to dedup further
    /// detections from the same sender while the first prompt is unresolved.
    pub fn find_unresolved_for_sender(
        &self,
        user_id: i32,
        platform: &str,
        sender_key: &str,
    ) -> Result<Option<CommitmentPrompt>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        commitment_prompts::table
            .filter(commitment_prompts::user_id.eq(user_id))
            .filter(commitment_prompts::platform.eq(platform))
            .filter(commitment_prompts::sender_key.eq(sender_key))
            .filter(commitment_prompts::resolved_at.is_null())
            .order(commitment_prompts::sent_at.desc())
            .first::<CommitmentPrompt>(&mut conn)
            .optional()
    }

    /// Atomically claim a prompt for resolution. Updates only when the
    /// prompt is still unresolved - duplicate Twilio retries or concurrent
    /// replies see `Ok(0)` and the caller must skip side effects to avoid
    /// double-creating events / sender rules.
    ///
    /// Does NOT set resulting_event_id - that's a separate update once the
    /// event has actually been created. This keeps the claim atomic without
    /// needing a wrapping transaction.
    pub fn claim_prompt(&self, prompt_id: i32, user_label: &str) -> Result<usize, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let now = Self::now();
        diesel::update(
            commitment_prompts::table
                .filter(commitment_prompts::id.eq(prompt_id))
                .filter(commitment_prompts::resolved_at.is_null()),
        )
        .set((
            commitment_prompts::user_label.eq(Some(user_label.to_string())),
            commitment_prompts::labeled_at.eq(Some(now)),
            commitment_prompts::resolved_at.eq(Some(now)),
        ))
        .execute(&mut conn)
    }

    /// Attach the resulting ont_event id after a successful track / always
    /// action. Separate from claim_prompt because the event id isn't known
    /// until the event is actually created.
    pub fn set_prompt_event_id(
        &self,
        prompt_id: i32,
        resulting_event_id: i32,
    ) -> Result<usize, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(commitment_prompts::table.filter(commitment_prompts::id.eq(prompt_id)))
            .set(commitment_prompts::resulting_event_id.eq(Some(resulting_event_id)))
            .execute(&mut conn)
    }

    /// Backfill the channel SID on a freshly-created prompt row. The row is
    /// inserted before the outbound send so the unique partial index can
    /// reject racing duplicates, then the SID is patched in after the send
    /// returns. Best-effort: a failure here doesn't break the prompt flow.
    pub fn set_prompt_sms_sid(
        &self,
        prompt_id: i32,
        sms_message_sid: &str,
    ) -> Result<usize, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(commitment_prompts::table.filter(commitment_prompts::id.eq(prompt_id)))
            .set(commitment_prompts::sms_message_sid.eq(Some(sms_message_sid.to_string())))
            .execute(&mut conn)
    }

    /// Mark a prompt resolved without a user reply (e.g. expired by TTL job).
    pub fn expire_prompt(&self, prompt_id: i32) -> Result<usize, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let now = Self::now();
        diesel::update(commitment_prompts::table.filter(commitment_prompts::id.eq(prompt_id)))
            .set(commitment_prompts::resolved_at.eq(Some(now)))
            .execute(&mut conn)
    }

    pub fn list_recent_prompts(
        &self,
        user_id: i32,
        limit: i64,
    ) -> Result<Vec<CommitmentPrompt>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        commitment_prompts::table
            .filter(commitment_prompts::user_id.eq(user_id))
            .order(commitment_prompts::sent_at.desc())
            .limit(limit)
            .load(&mut conn)
    }

    // -------------------------------------------------------------------------
    // Label embeddings (similarity memory)
    // -------------------------------------------------------------------------

    pub fn store_label_embedding(
        &self,
        embedding: &NewCommitmentLabelEmbedding,
    ) -> Result<CommitmentLabelEmbedding, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::insert_into(commitment_label_embeddings::table)
            .values(embedding)
            .get_result(&mut conn)
    }

    /// All embeddings for a user filtered by label type ('track' or 'wrong').
    /// Returned in created_at desc order so callers can easily take the N most
    /// recent. Brute-force cosine similarity is fine for the per-user scale
    /// (hundreds of vectors at most).
    pub fn list_embeddings(
        &self,
        user_id: i32,
        label_type: &str,
    ) -> Result<Vec<CommitmentLabelEmbedding>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        commitment_label_embeddings::table
            .filter(commitment_label_embeddings::user_id.eq(user_id))
            .filter(commitment_label_embeddings::label_type.eq(label_type))
            .order(commitment_label_embeddings::created_at.desc())
            .load(&mut conn)
    }
}
