use crate::models::user_models::User;
use crate::pg_models::{BillingAccount, BillingUsageEvent};
use crate::{AppState, BillingRepository, UserCoreOps};
use anyhow::{anyhow, Context, Result};
use reqwest::Client as HttpClient;
use serde_json::{json, Value};
use std::sync::Arc;

pub const OVERAGE_CONSENT_VERSION: &str = "2026-07-21";
pub const LEGACY_OVERAGE_CONSENT_VERSION: &str = "legacy-auto-topup-migration-2026-07-23";
const CUSTOMER_ALIAS_PREFIX: &str = "lightfriend-user-";
const MONTHLY_INCLUDED_USAGE_USD: f64 = 25.0;

#[derive(Clone, Debug, Default)]
pub struct CustomerUsageBalance {
    pub available_usage_usd: f64,
    pub resets_at: Option<String>,
}

pub fn verify_webhook_signature(
    secret: &str,
    date: &str,
    body: &[u8],
    signature: &str,
    now_timestamp: i64,
) -> Result<()> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let sent_at = chrono::DateTime::parse_from_rfc2822(date).context("Invalid Date header")?;
    if (now_timestamp - sent_at.timestamp()).abs() > 300 {
        return Err(anyhow!("Stale webhook"));
    }
    let provided_signature = hex::decode(signature).context("Invalid webhook signature hex")?;
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC accepts keys of any size");
    mac.update(date.as_bytes());
    mac.update(b"\n");
    mac.update(body);
    mac.verify_slice(&provided_signature)
        .map_err(|_| anyhow!("Invalid webhook signature"))
}

pub fn cost_to_microusd(cost_usd: f64) -> Result<i64> {
    if !cost_usd.is_finite() || cost_usd <= 0.0 {
        return Err(anyhow!("Usage cost must be a positive finite number"));
    }
    Ok((cost_usd * 1_000_000.0).round() as i64)
}

fn hour_boundary(now: chrono::DateTime<chrono::Utc>) -> chrono::DateTime<chrono::Utc> {
    let hour_timestamp = now.timestamp().div_euclid(3600) * 3600;
    chrono::DateTime::<chrono::Utc>::from_timestamp(hour_timestamp, 0)
        .expect("a valid DateTime has a valid hour boundary")
}

pub fn contract_starting_at(now: chrono::DateTime<chrono::Utc>) -> String {
    hour_boundary(now).to_rfc3339()
}

/// Returns the one-time Metronome overage state to persist during cutover.
///
/// Existing users who explicitly enabled legacy auto top-up keep that opt-in
/// once a reusable payment method is ready. Everyone else is marked migrated
/// with overage disabled. `None` means either the preference was already
/// migrated or payment setup must be retried before preserving an opt-in.
pub fn legacy_overage_migration_target(
    legacy_auto_topup_enabled: bool,
    payment_ready: bool,
    already_migrated: bool,
) -> Option<bool> {
    if already_migrated {
        return None;
    }
    if legacy_auto_topup_enabled && !payment_ready {
        return None;
    }
    Some(legacy_auto_topup_enabled)
}

#[derive(Clone, Debug)]
pub struct MetronomeConfig {
    pub enabled: bool,
    pub api_url: String,
    pub api_key: String,
    pub package_alias: String,
    pub event_type: String,
    pub webhook_secret: String,
    pub legacy_credit_product_id: Option<String>,
    pub credit_type_id: Option<String>,
}

impl MetronomeConfig {
    pub fn from_env() -> Self {
        Self {
            enabled: std::env::var("METRONOME_BILLING_ENABLED")
                .map(|value| value.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
            api_url: std::env::var("METRONOME_API_URL")
                .unwrap_or_else(|_| "https://api.metronome.com".to_string()),
            api_key: std::env::var("METRONOME_API_KEY").unwrap_or_default(),
            package_alias: std::env::var("METRONOME_PACKAGE_ALIAS")
                .unwrap_or_else(|_| "lightfriend-monthly".to_string()),
            event_type: std::env::var("METRONOME_USAGE_EVENT_TYPE")
                .unwrap_or_else(|_| "lightfriend_usage".to_string()),
            webhook_secret: std::env::var("METRONOME_WEBHOOK_SECRET").unwrap_or_default(),
            legacy_credit_product_id: std::env::var("METRONOME_LEGACY_CREDIT_PRODUCT_ID")
                .ok()
                .filter(|value| !value.is_empty()),
            credit_type_id: std::env::var("METRONOME_CREDIT_TYPE_ID")
                .ok()
                .filter(|value| !value.is_empty()),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.enabled && self.api_key.is_empty() {
            return Err(anyhow!(
                "METRONOME_API_KEY is required when METRONOME_BILLING_ENABLED=true"
            ));
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct MetronomeClient {
    config: MetronomeConfig,
    http: HttpClient,
}

impl MetronomeClient {
    pub fn from_env() -> Result<Self> {
        let config = MetronomeConfig::from_env();
        config.validate()?;
        Ok(Self {
            config,
            http: HttpClient::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()?,
        })
    }

    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    pub fn webhook_secret(&self) -> &str {
        &self.config.webhook_secret
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.config.api_url.trim_end_matches('/'), path)
    }

    async fn response_json(response: reqwest::Response) -> Result<Value> {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(anyhow!("Metronome returned {}: {}", status, body));
        }
        serde_json::from_str(&body).context("Metronome returned invalid JSON")
    }

    async fn post(&self, path: &str, body: &Value, idempotency_key: &str) -> Result<Value> {
        let response = self
            .http
            .post(self.url(path))
            .bearer_auth(&self.config.api_key)
            .header("Idempotency-Key", idempotency_key)
            .json(body)
            .send()
            .await?;
        Self::response_json(response).await
    }

    async fn post_read(&self, path: &str, body: &Value) -> Result<Value> {
        let response = self
            .http
            .post(self.url(path))
            .bearer_auth(&self.config.api_key)
            .json(body)
            .send()
            .await?;
        Self::response_json(response).await
    }

    pub async fn customer_usage_balance(
        &self,
        account: &BillingAccount,
    ) -> Result<CustomerUsageBalance> {
        let customer_id = account
            .metronome_customer_id
            .as_deref()
            .ok_or_else(|| anyhow!("Billing account is not provisioned"))?;
        let now = chrono::Utc::now();
        let response = self
            .post_read(
                "/v1/contracts/customerBalances/list",
                &json!({
                    "customer_id": customer_id,
                    "covering_date": now.to_rfc3339(),
                    "include_balance": true,
                    "include_contract_balances": true,
                    "limit": 25
                }),
            )
            .await?;

        let mut available_cents = 0.0;
        let mut resets_at: Option<chrono::DateTime<chrono::Utc>> = None;
        for balance in response["data"].as_array().into_iter().flatten() {
            if balance["type"].as_str() != Some("CREDIT") {
                continue;
            }
            available_cents += balance["balance"]
                .as_f64()
                .or_else(|| balance["balance"].as_i64().map(|value| value as f64))
                .unwrap_or(0.0)
                .max(0.0);

            for segment in balance["access_schedule"]["schedule_items"]
                .as_array()
                .into_iter()
                .flatten()
            {
                let starts = segment["starting_at"]
                    .as_str()
                    .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
                    .map(|value| value.with_timezone(&chrono::Utc));
                let ends = segment["ending_before"]
                    .as_str()
                    .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
                    .map(|value| value.with_timezone(&chrono::Utc));
                if starts.is_some_and(|start| start <= now) && ends.is_some_and(|end| end > now) {
                    if let Some(end) = ends {
                        if resets_at.is_none_or(|current| end < current) {
                            resets_at = Some(end);
                        }
                    }
                }
            }
        }

        Ok(CustomerUsageBalance {
            available_usage_usd: available_cents / 100.0,
            resets_at: resets_at.map(|value| value.to_rfc3339()),
        })
    }

    async fn find_customer(&self, alias: &str) -> Result<Option<String>> {
        let response = self
            .http
            .get(self.url("/v1/customers"))
            .bearer_auth(&self.config.api_key)
            .query(&[("ingest_alias", alias)])
            .send()
            .await?;
        let body = Self::response_json(response).await?;
        Ok(body["data"]
            .as_array()
            .and_then(|items| items.first())
            .and_then(|item| item["id"].as_str())
            .map(ToString::to_string))
    }

    async fn create_customer(&self, user: &User, stripe_customer_id: &str) -> Result<String> {
        let alias = format!("{}{}", CUSTOMER_ALIAS_PREFIX, user.id);
        if let Some(customer_id) = self.find_customer(&alias).await? {
            return Ok(customer_id);
        }

        let body = json!({
            "name": user.email,
            "ingest_aliases": [alias],
            "customer_billing_provider_configurations": [{
                "billing_provider": "stripe",
                "delivery_method": "direct_to_billing_provider",
                "configuration": {
                    "stripe_customer_id": stripe_customer_id,
                    "stripe_collection_method": "charge_automatically"
                }
            }]
        });
        let response = self
            .post(
                "/v1/customers",
                &body,
                &format!("lightfriend-customer-{}", user.id),
            )
            .await;
        match response {
            Ok(value) => value["data"]["id"]
                .as_str()
                .map(ToString::to_string)
                .ok_or_else(|| anyhow!("Metronome customer response did not contain data.id")),
            Err(error) if error.to_string().contains("409") => self
                .find_customer(&format!("{}{}", CUSTOMER_ALIAS_PREFIX, user.id))
                .await?
                .ok_or(error),
            Err(error) => Err(error),
        }
    }

    async fn find_contract(&self, customer_id: &str) -> Result<Option<String>> {
        let body = self
            .post(
                "/v2/contracts/list",
                &json!({"customer_id": customer_id}),
                &format!("lightfriend-list-contracts-{}", customer_id),
            )
            .await?;
        Ok(body["data"]
            .as_array()
            .and_then(|items| items.first())
            .and_then(|item| item["id"].as_str())
            .map(ToString::to_string))
    }

    async fn create_contract(&self, user_id: i32, customer_id: &str) -> Result<String> {
        let starting_at = contract_starting_at(chrono::Utc::now());
        let body = json!({
            "customer_id": customer_id,
            "starting_at": starting_at,
            "package_alias": self.config.package_alias,
            "uniqueness_key": format!("lightfriend-contract-{}", user_id)
        });
        match self
            .post(
                "/v1/contracts/create",
                &body,
                &format!("lightfriend-contract-{}", user_id),
            )
            .await
        {
            Ok(value) => value["data"]["id"]
                .as_str()
                .map(ToString::to_string)
                .ok_or_else(|| anyhow!("Metronome contract response did not contain data.id")),
            Err(error) if error.to_string().contains("409") => {
                self.find_contract(customer_id).await?.ok_or(error)
            }
            Err(error) => Err(error),
        }
    }

    async fn set_contract_overage(
        &self,
        customer_id: &str,
        contract_id: &str,
        user_id: i32,
        enabled: bool,
    ) -> Result<()> {
        let operation_id = format!(
            "lightfriend-overage-{}-{}-{}",
            user_id,
            enabled,
            chrono::Utc::now().timestamp_millis()
        );
        self.post(
            "/v2/contracts/edit",
            &json!({
                "customer_id": customer_id,
                "contract_id": contract_id,
                "update_spend_threshold_configuration": {"is_enabled": enabled},
                "uniqueness_key": operation_id.clone()
            }),
            &operation_id,
        )
        .await?;
        Ok(())
    }

    pub async fn set_overage(&self, account: &BillingAccount, enabled: bool) -> Result<()> {
        let customer_id = account
            .metronome_customer_id
            .as_deref()
            .ok_or_else(|| anyhow!("Billing account is not provisioned"))?;
        let contract_id = account
            .metronome_contract_id
            .as_deref()
            .ok_or_else(|| anyhow!("Billing contract is not provisioned"))?;
        self.set_contract_overage(customer_id, contract_id, account.user_id, enabled)
            .await
    }

    async fn migrate_legacy_credit(
        &self,
        user: &User,
        customer_id: &str,
        contract_id: &str,
    ) -> Result<bool> {
        if user.credits <= 0.0 {
            return Ok(true);
        }
        let Some(product_id) = self.config.legacy_credit_product_id.as_deref() else {
            return Ok(false);
        };
        let Some(credit_type_id) = self.config.credit_type_id.as_deref() else {
            return Ok(false);
        };
        let starting_at = hour_boundary(chrono::Utc::now());
        let ending_before = starting_at + chrono::Duration::days(3650);
        let amount_cents = (user.credits as f64 * 100.0).round() as i64;
        let result = self
            .post(
                "/v1/contracts/customerCredits/create",
                &json!({
                    "customer_id": customer_id,
                    "name": "Migrated Lightfriend credit balance",
                    "description": "One-time balance imported during the Metronome cutover",
                    "priority": 1,
                    "product_id": product_id,
                    "applicable_contract_ids": [contract_id],
                    "uniqueness_key": format!("lightfriend-legacy-credit-{}", user.id),
                    "access_schedule": {
                        "credit_type_id": credit_type_id,
                        "schedule_items": [{
                            "amount": amount_cents,
                            "starting_at": starting_at.to_rfc3339(),
                            "ending_before": ending_before.to_rfc3339()
                        }]
                    }
                }),
                &format!("lightfriend-legacy-credit-{}", user.id),
            )
            .await;
        match result {
            Ok(_) => Ok(true),
            Err(error) if error.to_string().contains("409") => Ok(true),
            Err(error) => Err(error),
        }
    }

    pub async fn ingest(&self, events: &[BillingUsageEvent]) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }
        let payload: Vec<Value> = events
            .iter()
            .map(|event| {
                json!({
                    "transaction_id": event.transaction_id,
                    "customer_id": format!("{}{}", CUSTOMER_ALIAS_PREFIX, event.user_id),
                    "event_type": self.config.event_type,
                    "timestamp": chrono::DateTime::from_timestamp(event.occurred_at as i64, 0)
                        .unwrap_or_else(chrono::Utc::now)
                        .to_rfc3339(),
                    "properties": {
                        "cost_usd": event.cost_microusd as f64 / 1_000_000.0,
                        "source": event.event_type
                    }
                })
            })
            .collect();
        self.post(
            "/v1/ingest",
            &Value::Array(payload),
            &format!("lightfriend-ingest-{}", uuid::Uuid::new_v4()),
        )
        .await?;
        Ok(())
    }
}

async fn ensure_stripe_payment_method(user: &User) -> Result<bool> {
    use stripe::{
        Client, Customer, CustomerInvoiceSettings, ListSubscriptions, Subscription, UpdateCustomer,
    };
    let Some(customer_id) = user.stripe_customer_id.as_deref() else {
        return Ok(false);
    };
    let secret = std::env::var("STRIPE_SECRET_KEY").context("STRIPE_SECRET_KEY not set")?;
    let client = Client::new(secret);
    let customer_id = customer_id.parse().context("Invalid Stripe customer ID")?;
    let customer = Customer::retrieve(&client, &customer_id, &[]).await?;
    if customer
        .invoice_settings
        .as_ref()
        .and_then(|settings| settings.default_payment_method.as_ref())
        .is_some()
    {
        return Ok(true);
    }
    let payment_method_id = if let Some(payment_method_id) = user.stripe_payment_method_id.clone() {
        Some(payment_method_id)
    } else {
        let subscriptions = Subscription::list(
            &client,
            &ListSubscriptions {
                customer: Some(customer_id.clone()),
                limit: Some(10),
                ..Default::default()
            },
        )
        .await?;
        subscriptions.data.iter().find_map(|subscription| {
            subscription
                .default_payment_method
                .as_ref()
                .map(|payment_method| payment_method.id().to_string())
        })
    };
    let Some(payment_method_id) = payment_method_id else {
        return Ok(false);
    };
    Customer::update(
        &client,
        &customer_id,
        UpdateCustomer {
            invoice_settings: Some(CustomerInvoiceSettings {
                default_payment_method: Some(payment_method_id),
                ..Default::default()
            }),
            ..Default::default()
        },
    )
    .await?;
    Ok(true)
}

fn enqueue_cutover_usage(repository: &BillingRepository, user: &User) -> Result<()> {
    if user.included_usage_window_start_timestamp.is_none() {
        return Ok(());
    }
    let remaining = (user.credits_left as f64).clamp(0.0, MONTHLY_INCLUDED_USAGE_USD);
    let already_used = MONTHLY_INCLUDED_USAGE_USD - remaining;
    if already_used < 0.000_001 {
        return Ok(());
    }
    repository.enqueue_usage(
        user.id,
        "cutover_included_usage",
        cost_to_microusd(already_used)?,
        chrono::Utc::now().timestamp() as i32,
        Some(format!("metronome-cutover-included-{}", user.id)),
    )?;
    Ok(())
}

async fn migrate_legacy_overage_preference(
    client: &MetronomeClient,
    repository: &BillingRepository,
    user: &User,
    account: BillingAccount,
) -> Result<BillingAccount> {
    let target = legacy_overage_migration_target(
        user.charge_when_under,
        account.payment_ready,
        account.legacy_overage_preference_migrated,
    );
    if account.legacy_overage_preference_migrated {
        return Ok(account);
    }
    let Some(enabled) = target else {
        client
            .set_overage(&account, false)
            .await
            .context("Failed to keep overage disabled while payment setup is incomplete")?;
        return Ok(account);
    };

    client
        .set_overage(&account, enabled)
        .await
        .context("Failed to apply migrated overage preference")?;
    repository.complete_legacy_overage_preference_migration(
        user.id,
        enabled,
        enabled.then_some(LEGACY_OVERAGE_CONSENT_VERSION),
    )?;
    repository
        .get_account(user.id)?
        .ok_or_else(|| anyhow!("Billing account disappeared during overage preference migration"))
}

pub async fn provision_user(state: &Arc<AppState>, user: &User) -> Result<BillingAccount> {
    let repository = BillingRepository::new(state.pg_pool.clone());
    let account = repository.ensure_account(user.id)?;
    let client = MetronomeClient::from_env()?;
    if !client.is_enabled() {
        return Ok(account);
    }
    if account.provisioning_status == "provisioned" {
        enqueue_cutover_usage(&repository, user)?;
        if !account.legacy_credit_migrated {
            let customer_id = account
                .metronome_customer_id
                .as_deref()
                .ok_or_else(|| anyhow!("Provisioned account has no Metronome customer ID"))?;
            let contract_id = account
                .metronome_contract_id
                .as_deref()
                .ok_or_else(|| anyhow!("Provisioned account has no Metronome contract ID"))?;
            if !client
                .migrate_legacy_credit(user, customer_id, contract_id)
                .await?
            {
                return Err(anyhow!(
                    "Legacy credit migration IDs are required for user {} with ${:.2} remaining",
                    user.id,
                    user.credits
                ));
            }
            repository.mark_legacy_credit_migrated(user.id)?;
        }
        let mut current_account = account;
        if !current_account.payment_ready {
            let payment_ready = ensure_stripe_payment_method(user).await.unwrap_or(false);
            repository.set_payment_ready(user.id, payment_ready)?;
            current_account = repository
                .get_account(user.id)?
                .ok_or_else(|| anyhow!("Provisioned billing account disappeared"))?;
        }
        return migrate_legacy_overage_preference(&client, &repository, user, current_account)
            .await;
    }
    let stripe_customer_id = user
        .stripe_customer_id
        .as_deref()
        .ok_or_else(|| anyhow!("User has no Stripe customer ID"))?;
    let payment_ready = ensure_stripe_payment_method(user).await.unwrap_or(false);
    let customer_id = client.create_customer(user, stripe_customer_id).await?;
    let contract_id = client.create_contract(user.id, &customer_id).await?;
    // Packages can carry an enabled spend threshold by default. Disable it
    // before exposing the contract to usage ingestion, then apply any migrated
    // legacy opt-in after payment readiness is known.
    client
        .set_contract_overage(&customer_id, &contract_id, user.id, false)
        .await?;
    repository.mark_provisioned(user.id, &customer_id, &contract_id, payment_ready)?;
    enqueue_cutover_usage(&repository, user)?;

    if !account.legacy_credit_migrated {
        match client
            .migrate_legacy_credit(user, &customer_id, &contract_id)
            .await
        {
            Ok(true) => repository.mark_legacy_credit_migrated(user.id)?,
            Ok(false) => {
                return Err(anyhow!(
                    "Legacy credit migration IDs are required for user {} with ${:.2} remaining",
                    user.id,
                    user.credits
                ))
            }
            Err(error) => return Err(error.context("Failed to migrate legacy credit balance")),
        }
    }

    let current_account = repository
        .get_account(user.id)?
        .ok_or_else(|| anyhow!("Provisioned billing account disappeared"))?;
    migrate_legacy_overage_preference(&client, &repository, user, current_account).await
}

pub async fn provision_subscribers(state: Arc<AppState>) {
    let client = match MetronomeClient::from_env() {
        Ok(client) if client.is_enabled() => client,
        Ok(_) => return,
        Err(error) => {
            tracing::error!("Metronome configuration error: {}", error);
            return;
        }
    };
    drop(client);

    let users = match state.user_core.get_users_by_tier("tier 2") {
        Ok(users) => users,
        Err(error) => {
            tracing::error!("Failed to load Metronome migration users: {}", error);
            return;
        }
    };
    let repository = BillingRepository::new(state.pg_pool.clone());
    for user in users {
        if state.user_core.is_byot_user(user.id) {
            continue;
        }
        if let Err(error) = provision_user(&state, &user).await {
            let _ = repository.ensure_account(user.id);
            let _ = repository.mark_provisioning_failed(user.id, &error.to_string());
            tracing::error!(
                "Failed to provision user {} in Metronome: {}",
                user.id,
                error
            );
        }
    }
}

pub async fn flush_usage_outbox(state: Arc<AppState>) {
    let client = match MetronomeClient::from_env() {
        Ok(client) if client.is_enabled() => client,
        _ => return,
    };
    let repository = BillingRepository::new(state.pg_pool.clone());
    let events = match repository.claim_due_usage(100) {
        Ok(events) => events,
        Err(error) => {
            tracing::error!("Failed to claim Metronome usage events: {}", error);
            return;
        }
    };
    for chunk in events.chunks(100) {
        match client.ingest(chunk).await {
            Ok(()) => {
                for event in chunk {
                    if let Err(error) = repository.mark_usage_sent(&event.transaction_id) {
                        tracing::error!(
                            "Failed to mark usage {} sent: {}",
                            event.transaction_id,
                            error
                        );
                    }
                }
            }
            Err(error) => {
                for event in chunk {
                    let _ = repository.mark_usage_failed(
                        &event.transaction_id,
                        event.attempts,
                        &error.to_string(),
                    );
                }
                tracing::warn!(
                    "Metronome usage ingest failed; events queued for retry: {}",
                    error
                );
            }
        }
    }
}

pub fn metronome_enabled() -> bool {
    MetronomeConfig::from_env().enabled
}

pub fn enqueue_usage(
    state: &Arc<AppState>,
    user_id: i32,
    event_type: &str,
    cost_usd: f32,
    transaction_id: Option<String>,
) -> Result<String> {
    let repository = BillingRepository::new(state.pg_pool.clone());
    repository.ensure_account(user_id)?;
    let cost_microusd = cost_to_microusd(cost_usd as f64)?;
    Ok(repository.enqueue_usage(
        user_id,
        event_type,
        cost_microusd,
        chrono::Utc::now().timestamp() as i32,
        transaction_id,
    )?)
}

pub fn has_usage_entitlement(state: &Arc<AppState>, user_id: i32) -> Result<bool> {
    let repository = BillingRepository::new(state.pg_pool.clone());
    let account = repository.ensure_account(user_id)?;
    Ok(account.usage_entitled)
}

pub async fn customer_reset_date_label(state: &Arc<AppState>, user_id: i32) -> Option<String> {
    let repository = BillingRepository::new(state.pg_pool.clone());
    let account = repository.get_account(user_id).ok().flatten()?;
    if let Ok(client) = MetronomeClient::from_env() {
        if let Ok(balance) = client.customer_usage_balance(&account).await {
            if let Some(reset_at) = balance.resets_at {
                if let Ok(reset_at) = chrono::DateTime::parse_from_rfc3339(&reset_at) {
                    return Some(reset_at.format("%b %-d").to_string());
                }
            }
        }
    }

    // The contract starts when the local billing account is provisioned, so this is a stable
    // fallback if the balance API is briefly unavailable while an alert webhook is handled.
    let mut reset_at = chrono::DateTime::from_timestamp(account.created_at as i64, 0)?;
    let now = chrono::Utc::now();
    while reset_at <= now {
        reset_at = reset_at.checked_add_months(chrono::Months::new(1))?;
    }
    Some(reset_at.format("%b %-d").to_string())
}
