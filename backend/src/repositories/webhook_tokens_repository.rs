//! Storage for per-user webhook tokens. Each row is keyed by `token_hash`
//! (SHA-256 hex of the raw bearer string) so a DB read leaks neither the
//! plaintext token nor anything an attacker could replay.
//!
//! The send path goes through `claim_send_slot`, which combines daily-cap
//! window reset and per-request increment in a single atomic UPDATE.
//! Two concurrent requests therefore cannot both pass the cap check.

use crate::{
    models::user_models::{NewWebhookIdempotencyKey, NewWebhookToken, WebhookToken},
    pg_schema::{webhook_idempotency_keys, webhook_tokens},
    PgDbPool,
};
use diesel::prelude::*;
use diesel::result::{DatabaseErrorKind, Error as DieselError};
use std::time::{SystemTime, UNIX_EPOCH};

/// Idempotency keys older than this are treated as if they don't exist.
/// 24h matches the Stripe convention; long enough that legitimate retries
/// always hit, short enough that an attacker reusing a key the next day
/// can't replay a stale response.
const IDEMPOTENCY_TTL_SECS: i32 = 86_400;

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

/// Outcome of looking up / reserving an idempotency key.
#[derive(Debug)]
pub enum IdempotencyResult {
    /// First time we've seen this (token_id, key). Caller should
    /// proceed with the send and then call `complete_idempotency` with
    /// the resulting SID so future replays can be answered.
    Fresh { id: i32 },
    /// We've seen this key before and the prior request completed.
    /// Caller should return the cached SID without re-billing or
    /// re-consuming the daily cap.
    Replayed { sid: String },
    /// A prior request with the same key is still in flight. Reject
    /// with 409 so the client backs off.
    InFlight,
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

    /// Mark a token as revoked. Truly idempotent: a second revoke of an
    /// already-revoked token still returns `Ok(true)` so client retries
    /// (double-click, network retry) don't surface as 404. The original
    /// `revoked_at` timestamp is preserved — we only stamp `now` when the
    /// row is transitioning from active to revoked.
    ///
    /// Returns `Ok(true)` if the row exists and belongs to this user
    /// (regardless of whether this call was the one that flipped it),
    /// `Ok(false)` only when the row is truly not the caller's.
    pub fn revoke(&self, user_id: i32, token_id: i32) -> Result<bool, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let now = now_unix();
        // First: confirm the row belongs to this user. If not → 404.
        let owned: i64 = webhook_tokens::table
            .filter(webhook_tokens::id.eq(token_id))
            .filter(webhook_tokens::user_id.eq(user_id))
            .count()
            .get_result(&mut conn)?;
        if owned == 0 {
            return Ok(false);
        }
        // Then: only stamp `revoked_at` if currently NULL. The second
        // revoke matches 0 rows but still returns success above.
        diesel::update(
            webhook_tokens::table
                .filter(webhook_tokens::id.eq(token_id))
                .filter(webhook_tokens::user_id.eq(user_id))
                .filter(webhook_tokens::revoked_at.is_null()),
        )
        .set(webhook_tokens::revoked_at.eq(now))
        .execute(&mut conn)?;
        Ok(true)
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

    /// Atomically reserve one send slot. Resets the daily window when
    /// the stored `daily_reset_at` is in the past, then increments
    /// `daily_sent` only if it would stay strictly less than `daily_cap`.
    /// Either the row is updated and the new state returned, or the cap
    /// was hit and `OverCap` is returned without sending.
    ///
    /// Race-safety: the whole reset + read + claim sequence runs inside
    /// a single database transaction, so concurrent callers serialize
    /// at the row level. The claim itself is a single UPDATE with the
    /// `daily_sent < daily_cap` predicate baked into the WHERE clause,
    /// so even without transaction isolation the cap could not be
    /// exceeded; the transaction is what guarantees the *response* we
    /// return reflects the same row state we acted on (no torn read
    /// where the post-claim re-read happens to land in a later window).
    pub fn claim_send_slot(&self, token_hash: &str) -> Result<ClaimResult, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let now = now_unix();
        let next_reset = next_utc_midnight(now);
        let token_hash = token_hash.to_string();

        conn.transaction::<ClaimResult, DieselError, _>(|conn| {
            // 1. Reset the daily window if it has expired. The predicate
            //    `daily_reset_at <= now` means rows still inside their
            //    current window are no-ops; we don't burn a write per
            //    request.
            diesel::update(
                webhook_tokens::table
                    .filter(webhook_tokens::token_hash.eq(&token_hash))
                    .filter(webhook_tokens::daily_reset_at.le(now)),
            )
            .set((
                webhook_tokens::daily_sent.eq(0),
                webhook_tokens::daily_reset_at.eq(next_reset),
            ))
            .execute(conn)?;

            // 2. Re-read inside the same transaction so we see our own
            //    write from step 1, and so revoked/cap state is fresh.
            let row = match webhook_tokens::table
                .filter(webhook_tokens::token_hash.eq(&token_hash))
                .select(WebhookToken::as_select())
                .first::<WebhookToken>(conn)
                .optional()?
            {
                Some(r) => r,
                None => return Ok(ClaimResult::Revoked),
            };
            if row.revoked_at.is_some() {
                return Ok(ClaimResult::Revoked);
            }

            // 3. Atomic claim: predicate-guarded increment.
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
            .execute(conn)?;

            if affected == 0 {
                // A concurrent revoke between step 2 and step 3 would
                // also produce affected==0 even inside this transaction
                // (transactions in PG use read-committed by default and
                // another committed revoke is visible to subsequent
                // statements). Re-read to distinguish revoked from cap.
                let fresh = webhook_tokens::table
                    .filter(webhook_tokens::id.eq(row.id))
                    .select(WebhookToken::as_select())
                    .first::<WebhookToken>(conn)?;
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
                .first::<WebhookToken>(conn)?;
            Ok(ClaimResult::Ok { token: updated })
        })
    }

    /// Look up an idempotency key, reserving a fresh row if the key
    /// is unseen. The whole sequence runs in a transaction so concurrent
    /// requests with the same (token_id, key) cannot both reserve.
    ///
    /// Behavior:
    /// - Existing row, within TTL, completed (sid set)  → `Replayed { sid }`
    /// - Existing row, within TTL, in-flight (sid NULL) → `InFlight`
    /// - Existing row but expired (older than TTL)      → treat as fresh:
    ///   delete the stale row, insert a new one. This keeps key reuse
    ///   well-defined after the TTL elapses.
    /// - No row                                          → `Fresh { id }`
    pub fn reserve_idempotency_key(
        &self,
        token_id: i32,
        key: &str,
    ) -> Result<IdempotencyResult, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        let now = now_unix();
        let key = key.to_string();

        conn.transaction::<IdempotencyResult, DieselError, _>(|conn| {
            // Look up existing row.
            let existing = webhook_idempotency_keys::table
                .filter(webhook_idempotency_keys::token_id.eq(token_id))
                .filter(webhook_idempotency_keys::idempotency_key.eq(&key))
                .select((
                    webhook_idempotency_keys::id,
                    webhook_idempotency_keys::response_sid,
                    webhook_idempotency_keys::created_at,
                ))
                .first::<(i32, Option<String>, i32)>(conn)
                .optional()?;

            if let Some((row_id, response_sid, created_at)) = existing {
                if now - created_at <= IDEMPOTENCY_TTL_SECS {
                    return Ok(match response_sid {
                        Some(sid) => IdempotencyResult::Replayed { sid },
                        None => IdempotencyResult::InFlight,
                    });
                }
                // Stale: drop it and fall through to insert a fresh row.
                diesel::delete(
                    webhook_idempotency_keys::table.filter(webhook_idempotency_keys::id.eq(row_id)),
                )
                .execute(conn)?;
            }

            // Insert a fresh row with response_sid = NULL (in-flight).
            // The UNIQUE (token_id, idempotency_key) constraint defends
            // against a concurrent insert that beat us between the SELECT
            // and the INSERT — translated to InFlight below.
            let new_row = NewWebhookIdempotencyKey {
                token_id,
                idempotency_key: key.clone(),
                response_sid: None,
                created_at: now,
            };
            match diesel::insert_into(webhook_idempotency_keys::table)
                .values(&new_row)
                .returning(webhook_idempotency_keys::id)
                .get_result::<i32>(conn)
            {
                Ok(id) => Ok(IdempotencyResult::Fresh { id }),
                Err(DieselError::DatabaseError(DatabaseErrorKind::UniqueViolation, _)) => {
                    // Lost the race; another request just reserved.
                    // Whatever they're doing, we treat as in-flight.
                    Ok(IdempotencyResult::InFlight)
                }
                Err(e) => Err(e),
            }
        })
    }

    /// Stamp the SID on a previously reserved idempotency row so future
    /// replays can return it. Best-effort: if the row was already
    /// deleted (e.g. via CASCADE on token revoke), we silently no-op.
    pub fn complete_idempotency(&self, row_id: i32, sid: &str) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::update(
            webhook_idempotency_keys::table.filter(webhook_idempotency_keys::id.eq(row_id)),
        )
        .set(webhook_idempotency_keys::response_sid.eq(sid))
        .execute(&mut conn)?;
        Ok(())
    }

    /// Clear an in-flight reservation when the send fails. Lets the
    /// client retry with the same key without waiting for the 24h TTL.
    /// Safe to call multiple times.
    pub fn clear_idempotency(&self, row_id: i32) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get DB connection");
        diesel::delete(
            webhook_idempotency_keys::table.filter(webhook_idempotency_keys::id.eq(row_id)),
        )
        .execute(&mut conn)?;
        Ok(())
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
