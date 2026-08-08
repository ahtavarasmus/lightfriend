use crate::pg_models::{
    BillingAccount, BillingUsageEvent, BillingUsageIntent, BillingWebhookEvent, NewBillingAccount,
    NewBillingUsageEvent, NewBillingUsageIntent, NewBillingWebhookEvent,
};
use crate::pg_schema::{
    billing_accounts, billing_usage_events, billing_usage_intents, billing_webhook_events,
};
use crate::PgDbPool;
use diesel::prelude::*;
use diesel::result::Error as DieselError;

pub struct BillingRepository {
    pool: PgDbPool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BillingWebhookClaim {
    Claimed,
    AlreadyProcessed,
    InFlight,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BillingReconciliationSummary {
    pub pending: i64,
    pub failed: i64,
    pub sent_unverified: i64,
    pub provider_matched: i64,
    pub provider_unmatched: i64,
    pub invoice_visible: i64,
    pub stale_open_intents: i64,
}

impl BillingRepository {
    pub fn new(pool: PgDbPool) -> Self {
        Self { pool }
    }

    pub fn ensure_account(&self, user_id: i32) -> Result<BillingAccount, DieselError> {
        let now = chrono::Utc::now().timestamp() as i32;
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        diesel::insert_into(billing_accounts::table)
            .values(NewBillingAccount {
                user_id,
                created_at: now,
                updated_at: now,
            })
            .on_conflict(billing_accounts::user_id)
            .do_nothing()
            .execute(&mut conn)?;
        billing_accounts::table.find(user_id).first(&mut conn)
    }

    pub fn get_account(&self, user_id: i32) -> Result<Option<BillingAccount>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        billing_accounts::table
            .find(user_id)
            .first(&mut conn)
            .optional()
    }

    pub fn find_by_metronome_customer_id(
        &self,
        customer_id: &str,
    ) -> Result<Option<BillingAccount>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        billing_accounts::table
            .filter(billing_accounts::metronome_customer_id.eq(customer_id))
            .first(&mut conn)
            .optional()
    }

    pub fn mark_provisioned(
        &self,
        user_id: i32,
        customer_id: &str,
        contract_id: &str,
        payment_ready: bool,
    ) -> Result<(), DieselError> {
        let now = chrono::Utc::now().timestamp() as i32;
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        diesel::update(billing_accounts::table.find(user_id))
            .set((
                billing_accounts::metronome_customer_id.eq(customer_id),
                billing_accounts::metronome_contract_id.eq(contract_id),
                billing_accounts::payment_ready.eq(payment_ready),
                billing_accounts::provisioning_status.eq("provisioned"),
                billing_accounts::provisioning_error.eq::<Option<String>>(None),
                billing_accounts::updated_at.eq(now),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn mark_provisioning_failed(&self, user_id: i32, error: &str) -> Result<(), DieselError> {
        let now = chrono::Utc::now().timestamp() as i32;
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        diesel::update(billing_accounts::table.find(user_id))
            .set((
                billing_accounts::provisioning_status.eq("failed"),
                billing_accounts::provisioning_error.eq(error),
                billing_accounts::updated_at.eq(now),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn set_overage(
        &self,
        user_id: i32,
        enabled: bool,
        consent_version: Option<&str>,
    ) -> Result<(), DieselError> {
        let now = chrono::Utc::now().timestamp() as i32;
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        let consent_at = enabled.then_some(now);
        diesel::update(billing_accounts::table.find(user_id))
            .set((
                billing_accounts::overage_enabled.eq(enabled),
                billing_accounts::overage_consent_at.eq(consent_at),
                billing_accounts::overage_consent_version.eq(consent_version),
                billing_accounts::legacy_overage_preference_migrated.eq(true),
                billing_accounts::updated_at.eq(now),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn set_payment_ready(&self, user_id: i32, ready: bool) -> Result<(), DieselError> {
        let now = chrono::Utc::now().timestamp() as i32;
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        diesel::update(billing_accounts::table.find(user_id))
            .set((
                billing_accounts::payment_ready.eq(ready),
                billing_accounts::updated_at.eq(now),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn set_usage_entitled(&self, user_id: i32, entitled: bool) -> Result<(), DieselError> {
        let now = chrono::Utc::now().timestamp() as i32;
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        diesel::update(billing_accounts::table.find(user_id))
            .set((
                billing_accounts::usage_entitled.eq(entitled),
                billing_accounts::updated_at.eq(now),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn mark_legacy_credit_migrated(&self, user_id: i32) -> Result<(), DieselError> {
        let now = chrono::Utc::now().timestamp() as i32;
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        diesel::update(billing_accounts::table.find(user_id))
            .set((
                billing_accounts::legacy_credit_migrated.eq(true),
                billing_accounts::updated_at.eq(now),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn complete_legacy_overage_preference_migration(
        &self,
        user_id: i32,
        enabled: bool,
        consent_version: Option<&str>,
    ) -> Result<(), DieselError> {
        let now = chrono::Utc::now().timestamp() as i32;
        let consent_at = enabled.then_some(now);
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        diesel::update(billing_accounts::table.find(user_id))
            .set((
                billing_accounts::overage_enabled.eq(enabled),
                billing_accounts::overage_consent_at.eq(consent_at),
                billing_accounts::overage_consent_version.eq(consent_version),
                billing_accounts::legacy_overage_preference_migrated.eq(true),
                billing_accounts::updated_at.eq(now),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn enqueue_usage(
        &self,
        user_id: i32,
        event_type: &str,
        cost_microusd: i64,
        occurred_at: i32,
        transaction_id: Option<String>,
    ) -> Result<String, DieselError> {
        let transaction_id = transaction_id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let now = chrono::Utc::now().timestamp() as i32;
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        diesel::insert_into(billing_usage_events::table)
            .values(NewBillingUsageEvent {
                transaction_id: transaction_id.clone(),
                user_id,
                event_type: event_type.to_string(),
                cost_microusd,
                occurred_at,
                next_attempt_at: now,
                created_at: now,
            })
            .on_conflict(billing_usage_events::transaction_id)
            .do_nothing()
            .execute(&mut conn)?;
        Ok(transaction_id)
    }

    /// Persist a billing intent before a user-visible action begins. Dynamic
    /// cost can be finalized later without a crash silently erasing evidence
    /// that a billable action ran.
    pub fn begin_usage_intent(
        &self,
        user_id: i32,
        event_type: &str,
        transaction_id: &str,
    ) -> Result<(), DieselError> {
        let now = chrono::Utc::now().timestamp() as i32;
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        diesel::insert_into(billing_usage_intents::table)
            .values(NewBillingUsageIntent {
                transaction_id,
                user_id,
                event_type,
                created_at: now,
            })
            .on_conflict(billing_usage_intents::transaction_id)
            .do_nothing()
            .execute(&mut conn)?;
        let intent = billing_usage_intents::table
            .find(transaction_id)
            .first::<BillingUsageIntent>(&mut conn)?;
        if intent.user_id != user_id || intent.event_type != event_type {
            return Err(DieselError::RollbackTransaction);
        }
        Ok(())
    }

    pub fn finalize_usage_intent(
        &self,
        transaction_id: &str,
        cost_microusd: i64,
        occurred_at: i32,
    ) -> Result<(), DieselError> {
        let now = chrono::Utc::now().timestamp() as i32;
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        conn.transaction(|conn| {
            let intent = billing_usage_intents::table
                .find(transaction_id)
                .for_update()
                .first::<BillingUsageIntent>(conn)?;
            if intent.status == "finalized" {
                return Ok(());
            }
            if intent.status != "open" {
                return Err(DieselError::RollbackTransaction);
            }
            diesel::insert_into(billing_usage_events::table)
                .values(NewBillingUsageEvent {
                    transaction_id: transaction_id.to_string(),
                    user_id: intent.user_id,
                    event_type: intent.event_type,
                    cost_microusd,
                    occurred_at,
                    next_attempt_at: now,
                    created_at: now,
                })
                .on_conflict(billing_usage_events::transaction_id)
                .do_nothing()
                .execute(conn)?;
            diesel::update(billing_usage_intents::table.find(transaction_id))
                .set((
                    billing_usage_intents::status.eq("finalized"),
                    billing_usage_intents::finalized_at.eq(Some(now)),
                    billing_usage_intents::last_error.eq::<Option<String>>(None),
                ))
                .execute(conn)?;
            Ok(())
        })
    }

    pub fn abandon_usage_intent(&self, transaction_id: &str) -> Result<(), DieselError> {
        let now = chrono::Utc::now().timestamp() as i32;
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        diesel::update(
            billing_usage_intents::table
                .find(transaction_id)
                .filter(billing_usage_intents::status.eq("open")),
        )
        .set((
            billing_usage_intents::status.eq("abandoned"),
            billing_usage_intents::finalized_at.eq(Some(now)),
        ))
        .execute(&mut conn)?;
        Ok(())
    }

    pub fn claim_due_usage(&self, limit: i64) -> Result<Vec<BillingUsageEvent>, DieselError> {
        let now = chrono::Utc::now().timestamp() as i32;
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        let candidates = billing_usage_events::table
            .filter(billing_usage_events::status.eq_any(["pending", "failed", "sending"]))
            .filter(billing_usage_events::next_attempt_at.le(now))
            .order(billing_usage_events::created_at.asc())
            .limit(limit)
            .load::<BillingUsageEvent>(&mut conn)?;

        let mut claimed = Vec::with_capacity(candidates.len());
        for event in candidates {
            let affected = diesel::update(
                billing_usage_events::table
                    .find(&event.transaction_id)
                    .filter(billing_usage_events::status.eq_any(["pending", "failed", "sending"]))
                    .filter(billing_usage_events::next_attempt_at.le(now)),
            )
            .set((
                billing_usage_events::status.eq("sending"),
                billing_usage_events::next_attempt_at.eq(now.saturating_add(60)),
            ))
            .execute(&mut conn)?;
            if affected == 1 {
                claimed.push(event);
            }
        }
        Ok(claimed)
    }

    pub fn get_usage_event(
        &self,
        transaction_id: &str,
    ) -> Result<Option<BillingUsageEvent>, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        billing_usage_events::table
            .find(transaction_id)
            .first(&mut conn)
            .optional()
    }

    pub fn mark_usage_sent(&self, transaction_id: &str) -> Result<(), DieselError> {
        let now = chrono::Utc::now().timestamp() as i32;
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        diesel::update(billing_usage_events::table.find(transaction_id))
            .set((
                billing_usage_events::status.eq("sent"),
                billing_usage_events::sent_at.eq(Some(now)),
                billing_usage_events::last_error.eq::<Option<String>>(None),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn mark_usage_failed(
        &self,
        transaction_id: &str,
        previous_attempts: i32,
        error: &str,
    ) -> Result<(), DieselError> {
        let now = chrono::Utc::now().timestamp() as i32;
        let attempts = previous_attempts.saturating_add(1);
        let delay_seconds = 2_i32.saturating_pow(attempts.min(10) as u32).min(3600);
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        diesel::update(billing_usage_events::table.find(transaction_id))
            .set((
                billing_usage_events::status.eq("failed"),
                billing_usage_events::attempts.eq(attempts),
                billing_usage_events::next_attempt_at.eq(now.saturating_add(delay_seconds)),
                billing_usage_events::last_error.eq(error),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn record_webhook_once(
        &self,
        event_id: &str,
        event_type: &str,
    ) -> Result<bool, DieselError> {
        let now = chrono::Utc::now().timestamp() as i32;
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        let inserted = diesel::insert_into(billing_webhook_events::table)
            .values(NewBillingWebhookEvent {
                event_id,
                event_type,
                received_at: now,
                status: "processed",
                attempts: 1,
                lease_until: None,
                processed_at: Some(now),
                last_error: None,
            })
            .on_conflict(billing_webhook_events::event_id)
            .do_nothing()
            .execute(&mut conn)?;
        Ok(inserted == 1)
    }

    pub fn webhook_seen(&self, event_id: &str) -> Result<bool, DieselError> {
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        Ok(billing_webhook_events::table
            .find(event_id)
            .select(billing_webhook_events::event_id)
            .first::<String>(&mut conn)
            .optional()?
            .is_some())
    }

    /// Atomically reserve a webhook before any entitlement side effects.
    /// A failed/crashed delivery becomes claimable when its lease expires.
    pub fn claim_webhook(
        &self,
        event_id: &str,
        event_type: &str,
        lease_seconds: i32,
    ) -> Result<BillingWebhookClaim, DieselError> {
        let now = chrono::Utc::now().timestamp() as i32;
        let lease_until = now.saturating_add(lease_seconds);
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        let inserted = diesel::insert_into(billing_webhook_events::table)
            .values(NewBillingWebhookEvent {
                event_id,
                event_type,
                received_at: now,
                status: "processing",
                attempts: 1,
                lease_until: Some(lease_until),
                processed_at: None,
                last_error: None,
            })
            .on_conflict(billing_webhook_events::event_id)
            .do_nothing()
            .execute(&mut conn)?;
        if inserted == 1 {
            return Ok(BillingWebhookClaim::Claimed);
        }

        let existing = billing_webhook_events::table
            .find(event_id)
            .first::<BillingWebhookEvent>(&mut conn)?;
        if existing.status == "processed" {
            return Ok(BillingWebhookClaim::AlreadyProcessed);
        }
        let affected = diesel::update(
            billing_webhook_events::table
                .find(event_id)
                .filter(billing_webhook_events::status.ne("processed"))
                .filter(
                    billing_webhook_events::lease_until
                        .is_null()
                        .or(billing_webhook_events::lease_until.le(now)),
                ),
        )
        .set((
            billing_webhook_events::status.eq("processing"),
            billing_webhook_events::event_type.eq(event_type),
            billing_webhook_events::attempts.eq(billing_webhook_events::attempts + 1),
            billing_webhook_events::lease_until.eq(Some(lease_until)),
            billing_webhook_events::last_error.eq::<Option<String>>(None),
        ))
        .execute(&mut conn)?;
        Ok(if affected == 1 {
            BillingWebhookClaim::Claimed
        } else {
            BillingWebhookClaim::InFlight
        })
    }

    pub fn complete_webhook(&self, event_id: &str) -> Result<(), DieselError> {
        let now = chrono::Utc::now().timestamp() as i32;
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        diesel::update(
            billing_webhook_events::table
                .find(event_id)
                .filter(billing_webhook_events::status.eq("processing")),
        )
        .set((
            billing_webhook_events::status.eq("processed"),
            billing_webhook_events::processed_at.eq(Some(now)),
            billing_webhook_events::lease_until.eq::<Option<i32>>(None),
            billing_webhook_events::last_error.eq::<Option<String>>(None),
        ))
        .execute(&mut conn)?;
        Ok(())
    }

    pub fn fail_webhook(&self, event_id: &str, error_code: &str) -> Result<(), DieselError> {
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        diesel::update(
            billing_webhook_events::table
                .find(event_id)
                .filter(billing_webhook_events::status.eq("processing")),
        )
        .set((
            billing_webhook_events::status.eq("failed"),
            billing_webhook_events::lease_until.eq::<Option<i32>>(None),
            billing_webhook_events::last_error.eq(Some(error_code)),
        ))
        .execute(&mut conn)?;
        Ok(())
    }

    pub fn usage_for_reconciliation(
        &self,
        limit: i64,
    ) -> Result<Vec<BillingUsageEvent>, DieselError> {
        let now = chrono::Utc::now().timestamp() as i32;
        let retry_before = now.saturating_sub(3600);
        let oldest_searchable = now.saturating_sub(33 * 24 * 3600);
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        billing_usage_events::table
            .filter(billing_usage_events::status.eq("sent"))
            .filter(billing_usage_events::occurred_at.ge(oldest_searchable))
            .filter(
                billing_usage_events::provider_reconciled_at
                    .is_null()
                    .or(billing_usage_events::provider_reconciled_at.le(retry_before)),
            )
            .order(billing_usage_events::sent_at.asc())
            .limit(limit)
            .load(&mut conn)
    }

    pub fn mark_usage_reconciled(
        &self,
        transaction_id: &str,
        provider_status: &str,
        invoice_visible: bool,
    ) -> Result<(), DieselError> {
        let now = chrono::Utc::now().timestamp() as i32;
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        diesel::update(billing_usage_events::table.find(transaction_id))
            .set((
                billing_usage_events::provider_reconciled_at.eq(Some(now)),
                billing_usage_events::provider_status.eq(provider_status),
                billing_usage_events::invoice_visible.eq(invoice_visible),
            ))
            .execute(&mut conn)?;
        Ok(())
    }

    pub fn reconciliation_summary(
        &self,
        stale_intent_age_seconds: i32,
    ) -> Result<BillingReconciliationSummary, DieselError> {
        use diesel::dsl::count_star;
        let now = chrono::Utc::now().timestamp() as i32;
        let mut conn = self.pool.get().expect("Failed to get PG connection");
        let count_usage = |conn: &mut PgConnection,
                           status: Option<&str>,
                           provider_status: Option<&str>,
                           invoice_visible: Option<bool>|
         -> Result<i64, DieselError> {
            let mut query = billing_usage_events::table.into_boxed();
            if let Some(value) = status {
                query = query.filter(billing_usage_events::status.eq(value));
            }
            if let Some(value) = provider_status {
                query = query.filter(billing_usage_events::provider_status.eq(value));
            }
            if let Some(value) = invoice_visible {
                query = query.filter(billing_usage_events::invoice_visible.eq(value));
            }
            query.select(count_star()).first(conn)
        };
        Ok(BillingReconciliationSummary {
            pending: count_usage(&mut conn, Some("pending"), None, None)?,
            failed: count_usage(&mut conn, Some("failed"), None, None)?,
            sent_unverified: count_usage(&mut conn, Some("sent"), Some("unverified"), None)?,
            provider_matched: count_usage(&mut conn, Some("sent"), Some("matched"), None)?,
            provider_unmatched: count_usage(&mut conn, Some("sent"), Some("unmatched"), None)?,
            invoice_visible: count_usage(&mut conn, Some("sent"), None, Some(true))?,
            stale_open_intents: billing_usage_intents::table
                .filter(billing_usage_intents::status.eq("open"))
                .filter(
                    billing_usage_intents::created_at
                        .le(now.saturating_sub(stale_intent_age_seconds)),
                )
                .select(count_star())
                .first(&mut conn)?,
        })
    }
}
