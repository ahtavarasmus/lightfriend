use crate::pg_models::{
    BillingAccount, BillingUsageEvent, NewBillingAccount, NewBillingUsageEvent,
    NewBillingWebhookEvent,
};
use crate::pg_schema::{billing_accounts, billing_usage_events, billing_webhook_events};
use crate::PgDbPool;
use diesel::prelude::*;
use diesel::result::Error as DieselError;

pub struct BillingRepository {
    pool: PgDbPool,
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
}
