//! Storage for per-user webhook tokens. Each row is keyed by `token_hash`
//! (SHA-256 hex of the raw bearer string) so a DB read leaks neither the
//! plaintext token nor anything an attacker could replay.
//!
//! The send path goes through `claim_send_slot`, which combines daily-cap
//! window reset and per-request increment in a single atomic UPDATE.
//! Two concurrent requests therefore cannot both pass the cap check.

use crate::{
    models::user_models::{NewWebhookToken, WebhookToken},
    pg_schema::webhook_tokens,
    PgDbPool,
};
use diesel::prelude::*;
use diesel::result::Error as DieselError;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct WebhookTokensRepository {
    pool: PgDbPool,
}

/// Outcome of attempting to reserve one send slot for a token.
#[derive(Debug)]
pub enum ClaimResult {
    /// Slot reserved; caller may send.
    Ok { token: WebhookToken },
    /// Token has been revoked.
    Revoked,
    /// Token is valid but the daily cap is exhausted.
    OverCap { daily_cap: i32, daily_sent: i32 },
}

impl WebhookTokensRepository {
    pub fn new(pool: PgDbPool) -> Self {
        Self { pool }
    }

    /// Insert a new token row. Caller supplies the SHA-256 hash and the
    /// short prefix; the raw token never reaches this layer.
    pub fn create(&self, row: &NewWebhookToken) -> Result<WebhookToken, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::insert_into(webhook_tokens::table)
            .values(row)
            .get_result::<WebhookToken>(&mut conn)
    }

    /// List active (non-revoked) tokens for the dashboard UI, newest first.
    pub fn list_for_user(&self, user_id: i32) -> Result<Vec<WebhookToken>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        webhook_tokens::table
            .filter(webhook_tokens::user_id.eq(user_id))
            .filter(webhook_tokens::revoked_at.is_null())
            .order(webhook_tokens::created_at.desc())
            .select(WebhookToken::as_select())
            .load::<WebhookToken>(&mut conn)
    }

    /// Mark a token as revoked. Idempotent; rows already revoked are left
    /// untouched. Returns whether the row exists and belongs to this user.
    pub fn revoke(&self, user_id: i32, token_id: i32) -> Result<bool, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let now = now_unix();
        let affected = diesel::update(
            webhook_tokens::table
                .filter(webhook_tokens::id.eq(token_id))
                .filter(webhook_tokens::user_id.eq(user_id))
                .filter(webhook_tokens::revoked_at.is_null()),
        )
        .set(webhook_tokens::revoked_at.eq(now))
        .execute(&mut conn)?;
        Ok(affected > 0)
    }

    /// Find a token row by its SHA-256 hash. Returns None when no row
    /// matches; callers must not distinguish "no such hash" from
    /// "revoked" in responses to keep the bearer 401 path uniform.
    pub fn find_by_hash(&self, token_hash: &str) -> Result<Option<WebhookToken>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        webhook_tokens::table
            .filter(webhook_tokens::token_hash.eq(token_hash))
            .select(WebhookToken::as_select())
            .first::<WebhookToken>(&mut conn)
            .optional()
    }

    /// Atomically reserve one send slot. Resets the daily window when the
    /// stored `daily_reset_at` is in the past, then increments `daily_sent`
    /// only if it would stay strictly less than `daily_cap`. Either the
    /// row is updated and the new state returned, or the cap was hit and
    /// `OverCap` is returned without sending.
    ///
    /// Race-safety: the eligibility predicate and the increment live in a
    /// single UPDATE, so two callers cannot both pass.
    pub fn claim_send_slot(&self, token_hash: &str) -> Result<ClaimResult, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let now = now_unix();

        // 1. Reset the daily window if it has expired. Always touches
        //    last_used_at would be a write on every poll, so we only
        //    reset rows whose window is actually stale.
        let next_reset = next_utc_midnight(now);
        diesel::update(
            webhook_tokens::table
                .filter(webhook_tokens::token_hash.eq(token_hash))
                .filter(webhook_tokens::daily_reset_at.le(now)),
        )
        .set((
            webhook_tokens::daily_sent.eq(0),
            webhook_tokens::daily_reset_at.eq(next_reset),
        ))
        .execute(&mut conn)?;

        // 2. Re-read to decide the response shape. We already know the
        //    row exists by hash (handler looked it up first), but we
        //    re-read here so revoked/cap state is fresh.
        let row = match webhook_tokens::table
            .filter(webhook_tokens::token_hash.eq(token_hash))
            .select(WebhookToken::as_select())
            .first::<WebhookToken>(&mut conn)
            .optional()?
        {
            Some(r) => r,
            None => return Ok(ClaimResult::Revoked), // treat missing as 401-equivalent
        };

        if row.revoked_at.is_some() {
            return Ok(ClaimResult::Revoked);
        }

        // 3. Atomic claim: increment only when there's headroom.
        let affected = diesel::update(
            webhook_tokens::table
                .filter(webhook_tokens::id.eq(row.id))
                .filter(webhook_tokens::revoked_at.is_null())
                .filter(webhook_tokens::daily_sent.lt(webhook_tokens::daily_cap)),
        )
        .set((
            webhook_tokens::daily_sent.eq(webhook_tokens::daily_sent + 1),
            webhook_tokens::last_used_at.eq(now),
        ))
        .execute(&mut conn)?;

        if affected == 0 {
            // A concurrent revoke between step 2 and step 3 would also
            // produce affected==0; re-read to distinguish revoked from
            // cap-exhausted so the API returns the right status code.
            let fresh = webhook_tokens::table
                .filter(webhook_tokens::id.eq(row.id))
                .select(WebhookToken::as_select())
                .first::<WebhookToken>(&mut conn)?;
            if fresh.revoked_at.is_some() {
                return Ok(ClaimResult::Revoked);
            }
            return Ok(ClaimResult::OverCap {
                daily_cap: fresh.daily_cap,
                daily_sent: fresh.daily_sent,
            });
        }

        // 4. Return the updated row.
        let updated = webhook_tokens::table
            .filter(webhook_tokens::id.eq(row.id))
            .select(WebhookToken::as_select())
            .first::<WebhookToken>(&mut conn)?;
        Ok(ClaimResult::Ok { token: updated })
    }
}

fn now_unix() -> i32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32
}

/// Next UTC midnight after `now`. Used as the daily-cap reset boundary so
/// users see predictable "X requests remaining today" semantics regardless
/// of when their first request of the day arrived.
fn next_utc_midnight(now: i32) -> i32 {
    let day = 86_400;
    ((now / day) + 1) * day
}
