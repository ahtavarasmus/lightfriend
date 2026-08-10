use crate::models::user_models::User;
use crate::pg_models::{BillingAccount, BillingUsageEvent};
use crate::{AppState, BillingRepository, UserCoreOps};
use anyhow::{anyhow, Context, Result};
use reqwest::Client as HttpClient;
use serde_json::{json, Value};
use std::collections::{hash_map::Entry, HashMap};
use std::sync::Arc;

pub const OVERAGE_CONSENT_VERSION: &str = "2026-07-21";
pub const LEGACY_OVERAGE_CONSENT_VERSION: &str = "legacy-auto-topup-migration-2026-07-23";
/// Provider outages never revoke access by themselves. Entitlement changes
/// only from verified billing webhooks; locally queued usage keeps retrying.
pub const ENTITLEMENT_OUTAGE_POLICY: &str = "continue_service_with_durable_backlog";
const CUSTOMER_ALIAS_PREFIX: &str = "lightfriend-user-";
const MONTHLY_INCLUDED_USAGE_USD: f64 = 25.0;

#[derive(Clone, Debug, Default)]
pub struct CustomerUsageBalance {
    pub available_usage_usd: f64,
    pub included_allowance_usd: f64,
    pub included_usage_used_usd: f64,
    pub overage_usage_usd: Option<f64>,
    pub period_start_at: Option<String>,
    pub resets_at: Option<String>,
}

fn numeric_value(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_i64().map(|number| number as f64))
}

pub fn customer_usage_balance_from_response(
    response: &Value,
    now: chrono::DateTime<chrono::Utc>,
) -> CustomerUsageBalance {
    struct ActiveCredit {
        available_cents: f64,
        granted_cents: f64,
        starts_at: chrono::DateTime<chrono::Utc>,
        ends_at: chrono::DateTime<chrono::Utc>,
    }

    let mut active_credits = Vec::new();
    for balance in response["data"].as_array().into_iter().flatten() {
        if balance["type"].as_str() != Some("CREDIT") {
            continue;
        }
        let available_cents = numeric_value(&balance["balance"]).unwrap_or(0.0).max(0.0);
        let active_segment = balance["access_schedule"]["schedule_items"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|segment| {
                let starts_at = segment["starting_at"]
                    .as_str()
                    .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())?
                    .with_timezone(&chrono::Utc);
                let ends_at = segment["ending_before"]
                    .as_str()
                    .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())?
                    .with_timezone(&chrono::Utc);
                (starts_at <= now && now < ends_at).then_some((starts_at, ends_at, segment))
            })
            .min_by_key(|(_, ends_at, _)| *ends_at);
        if let Some((starts_at, ends_at, segment)) = active_segment {
            active_credits.push(ActiveCredit {
                available_cents,
                granted_cents: numeric_value(&segment["amount"])
                    .unwrap_or(MONTHLY_INCLUDED_USAGE_USD * 100.0)
                    .max(0.0),
                starts_at,
                ends_at,
            });
        }
    }

    let Some(resets_at) = active_credits.iter().map(|credit| credit.ends_at).min() else {
        return CustomerUsageBalance {
            included_allowance_usd: MONTHLY_INCLUDED_USAGE_USD,
            ..CustomerUsageBalance::default()
        };
    };
    // The recurring monthly allowance is the active credit segment that expires
    // first. Longer-lived promotional or migrated balances must not inflate it.
    let monthly_credits: Vec<&ActiveCredit> = active_credits
        .iter()
        .filter(|credit| credit.ends_at == resets_at)
        .collect();
    let available_usage_usd = monthly_credits
        .iter()
        .map(|credit| credit.available_cents)
        .sum::<f64>()
        / 100.0;
    let included_allowance_usd = monthly_credits
        .iter()
        .map(|credit| credit.granted_cents)
        .sum::<f64>()
        / 100.0;
    let period_start_at = monthly_credits
        .iter()
        .map(|credit| credit.starts_at)
        .min()
        .map(|value| value.to_rfc3339());

    CustomerUsageBalance {
        available_usage_usd,
        included_allowance_usd,
        included_usage_used_usd: (included_allowance_usd - available_usage_usd).max(0.0),
        overage_usage_usd: None,
        period_start_at,
        resets_at: Some(resets_at.to_rfc3339()),
    }
}

pub fn usage_invoice_total_usd(
    invoice_response: &Value,
    contract_id: &str,
    period_start: chrono::DateTime<chrono::Utc>,
    period_end: chrono::DateTime<chrono::Utc>,
) -> f64 {
    invoice_response["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|invoice| invoice["contract_id"].as_str() == Some(contract_id))
        .filter(|invoice| {
            matches!(
                invoice["type"].as_str(),
                Some("USAGE") | Some("CONTRACT_USAGE")
            ) && invoice["status"].as_str() != Some("VOID")
        })
        .filter(|invoice| {
            let starts_at = invoice["start_timestamp"]
                .as_str()
                .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
                .map(|value| value.with_timezone(&chrono::Utc));
            let ends_at = invoice["end_timestamp"]
                .as_str()
                .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
                .map(|value| value.with_timezone(&chrono::Utc));
            starts_at.is_some_and(|starts_at| starts_at < period_end)
                && ends_at.is_some_and(|ends_at| ends_at > period_start)
        })
        .filter_map(|invoice| numeric_value(&invoice["total"]))
        .map(|total_cents| total_cents.max(0.0))
        .sum::<f64>()
        / 100.0
}

pub fn billing_period_from_anchor(
    anchor_timestamp: i32,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<(chrono::DateTime<chrono::Utc>, chrono::DateTime<chrono::Utc>)> {
    let mut period_start = chrono::DateTime::from_timestamp(anchor_timestamp as i64, 0)?;
    let mut period_end = period_start.checked_add_months(chrono::Months::new(1))?;
    while period_end <= now {
        period_start = period_end;
        period_end = period_start.checked_add_months(chrono::Months::new(1))?;
    }
    Some((period_start, period_end))
}

pub fn local_usage_balance_from_total(
    total_usage_microusd: i64,
    period_start: chrono::DateTime<chrono::Utc>,
    period_end: chrono::DateTime<chrono::Utc>,
) -> CustomerUsageBalance {
    let total_usage_usd = (total_usage_microusd.max(0) as f64) / 1_000_000.0;
    CustomerUsageBalance {
        available_usage_usd: (MONTHLY_INCLUDED_USAGE_USD - total_usage_usd).max(0.0),
        included_allowance_usd: MONTHLY_INCLUDED_USAGE_USD,
        included_usage_used_usd: total_usage_usd.min(MONTHLY_INCLUDED_USAGE_USD),
        overage_usage_usd: Some((total_usage_usd - MONTHLY_INCLUDED_USAGE_USD).max(0.0)),
        period_start_at: Some(period_start.to_rfc3339()),
        resets_at: Some(period_end.to_rfc3339()),
    }
}

pub fn local_usage_balance(
    repository: &BillingRepository,
    account: &BillingAccount,
    now: chrono::DateTime<chrono::Utc>,
) -> Result<CustomerUsageBalance> {
    let (period_start, period_end) = billing_period_from_anchor(account.created_at, now)
        .ok_or_else(|| anyhow!("Billing account has an invalid period anchor"))?;
    let total_usage_microusd = repository.usage_cost_microusd_between(
        account.user_id,
        period_start.timestamp() as i32,
        period_end.timestamp() as i32,
    )?;
    Ok(local_usage_balance_from_total(
        total_usage_microusd,
        period_start,
        period_end,
    ))
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
    pub billable_metric_id: Option<String>,
    pub usage_product_id: Option<String>,
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
            package_alias: std::env::var("METRONOME_PACKAGE_ALIAS").unwrap_or_default(),
            event_type: std::env::var("METRONOME_USAGE_EVENT_TYPE").unwrap_or_default(),
            billable_metric_id: std::env::var("METRONOME_BILLABLE_METRIC_ID")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            usage_product_id: std::env::var("METRONOME_USAGE_PRODUCT_ID")
                .ok()
                .filter(|value| !value.trim().is_empty()),
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
        if !self.enabled {
            return Ok(());
        }
        let mut missing = Vec::new();
        if self.api_key.trim().is_empty() {
            missing.push("METRONOME_API_KEY");
        }
        if self.package_alias.trim().is_empty() {
            missing.push("METRONOME_PACKAGE_ALIAS");
        }
        if self.event_type.trim().is_empty() {
            missing.push("METRONOME_USAGE_EVENT_TYPE");
        }
        if self.billable_metric_id.is_none() {
            missing.push("METRONOME_BILLABLE_METRIC_ID");
        }
        if self.usage_product_id.is_none() {
            missing.push("METRONOME_USAGE_PRODUCT_ID");
        }
        if self.webhook_secret.trim().is_empty() {
            missing.push("METRONOME_WEBHOOK_SECRET");
        }
        if self.legacy_credit_product_id.is_none() {
            missing.push("METRONOME_LEGACY_CREDIT_PRODUCT_ID");
        }
        if self.credit_type_id.is_none() {
            missing.push("METRONOME_CREDIT_TYPE_ID");
        }
        if !missing.is_empty() {
            return Err(anyhow!(
                "Metronome billing configuration is incomplete; missing {}",
                missing.join(", ")
            ));
        }
        let parsed = reqwest::Url::parse(&self.api_url)
            .map_err(|_| anyhow!("METRONOME_API_URL must be a valid HTTPS URL"))?;
        if parsed.scheme() != "https"
            && !(parsed.scheme() == "http" && parsed.host_str() == Some("127.0.0.1"))
            && !(parsed.scheme() == "http" && parsed.host_str() == Some("localhost"))
        {
            return Err(anyhow!(
                "METRONOME_API_URL must use HTTPS outside local tests"
            ));
        }
        Ok(())
    }
}

/// Resolve the one Lightfriend-owned contract without depending on API order.
/// An exact uniqueness-key match wins; a single legacy result is accepted,
/// while multiple non-matching contracts require operator reconciliation.
pub fn select_contract_id(
    response: &Value,
    expected_uniqueness_key: &str,
) -> Result<Option<String>> {
    let items = response["data"]
        .as_array()
        .ok_or_else(|| anyhow!("Metronome contract list returned an invalid response"))?;
    let exact: Vec<&Value> = items
        .iter()
        .filter(|item| item["uniqueness_key"].as_str() == Some(expected_uniqueness_key))
        .collect();
    match exact.as_slice() {
        [item] => return Ok(item["id"].as_str().map(ToString::to_string)),
        [] => {}
        _ => {
            return Err(anyhow!(
                "Multiple Metronome contracts share the expected uniqueness key"
            ))
        }
    }
    match items.as_slice() {
        [] => Ok(None),
        [item] => item["id"]
            .as_str()
            .map(|id| Some(id.to_string()))
            .ok_or_else(|| anyhow!("Metronome contract response did not contain data.id")),
        _ => Err(anyhow!(
            "Multiple Metronome contracts exist and none matches Lightfriend's uniqueness key"
        )),
    }
}

pub fn billing_error_code(error: &anyhow::Error) -> &'static str {
    let message = error.to_string().to_ascii_lowercase();
    if message.contains("configuration is incomplete") || message.contains("api_url") {
        "configuration_error"
    } else if message.contains("http 401") || message.contains("http 403") {
        "authentication_error"
    } else if message.contains("http 429") {
        "rate_limited"
    } else if message.contains("http 5") || message.contains("timeout") {
        "provider_unavailable"
    } else if message.contains("contract") && message.contains("multiple") {
        "ambiguous_contract"
    } else if message.contains("connect") || message.contains("transport") {
        "transport_error"
    } else {
        "billing_error"
    }
}

pub fn provider_http_error(status: reqwest::StatusCode) -> anyhow::Error {
    anyhow!("Metronome request failed (HTTP {})", status.as_u16())
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
        if !status.is_success() {
            // Never propagate provider response bodies: they can contain
            // customer fields, request echoes, or internal diagnostics.
            return Err(provider_http_error(status));
        }
        response
            .json::<Value>()
            .await
            .map_err(|_| anyhow!("Metronome returned an invalid response"))
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

    async fn get_read(&self, path: &str) -> Result<Value> {
        let response = self
            .http
            .get(self.url(path))
            .bearer_auth(&self.config.api_key)
            .send()
            .await?;
        Self::response_json(response).await
    }

    async fn search_events(&self, transaction_ids: &[String]) -> Result<Value> {
        self.post_read(
            "/v1/events/search",
            &json!({"transactionIds": transaction_ids}),
        )
        .await
    }

    async fn list_invoices(&self, customer_id: &str) -> Result<Value> {
        self.get_read(&format!("/v1/customers/{}/invoices?limit=100", customer_id))
            .await
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

        Ok(customer_usage_balance_from_response(&response, now))
    }

    pub async fn customer_billing_summary(
        &self,
        account: &BillingAccount,
    ) -> Result<CustomerUsageBalance> {
        let mut summary = self.customer_usage_balance(account).await?;
        let (Some(customer_id), Some(contract_id), Some(period_start_at), Some(resets_at)) = (
            account.metronome_customer_id.as_deref(),
            account.metronome_contract_id.as_deref(),
            summary.period_start_at.as_deref(),
            summary.resets_at.as_deref(),
        ) else {
            return Ok(summary);
        };
        let period_start = chrono::DateTime::parse_from_rfc3339(period_start_at)?.to_utc();
        let period_end = chrono::DateTime::parse_from_rfc3339(resets_at)?.to_utc();
        match self.list_invoices(customer_id).await {
            Ok(invoices) => {
                summary.overage_usage_usd = Some(usage_invoice_total_usd(
                    &invoices,
                    contract_id,
                    period_start,
                    period_end,
                ));
            }
            Err(error) => {
                tracing::warn!(
                    user_id = account.user_id,
                    "Failed to load current overage usage: {error}"
                );
            }
        }
        Ok(summary)
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

    async fn find_contract(&self, user_id: i32, customer_id: &str) -> Result<Option<String>> {
        let body = self
            .post(
                "/v2/contracts/list",
                &json!({"customer_id": customer_id}),
                &format!("lightfriend-list-contracts-{}", customer_id),
            )
            .await?;
        select_contract_id(&body, &format!("lightfriend-contract-{}", user_id))
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
                self.find_contract(user_id, customer_id).await?.ok_or(error)
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

pub fn provider_event_status(
    search_response: &Value,
    transaction_id: &str,
    billable_metric_id: &str,
) -> &'static str {
    let Some(event) = search_response
        .as_array()
        .into_iter()
        .flatten()
        .find(|event| event["transaction_id"].as_str() == Some(transaction_id))
    else {
        return "missing";
    };
    let customer_matched = event["matched_customer"].is_object();
    let metric_matched = event["matched_billable_metrics"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|metric| metric["id"].as_str() == Some(billable_metric_id));
    if customer_matched && metric_matched {
        "matched"
    } else {
        "unmatched"
    }
}

pub fn invoice_contains_usage(
    invoice_response: &Value,
    contract_id: &str,
    usage_product_id: &str,
    occurred_at: i32,
) -> bool {
    let Some(occurred) = chrono::DateTime::from_timestamp(occurred_at as i64, 0) else {
        return false;
    };
    invoice_response["data"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|invoice| invoice["contract_id"].as_str() == Some(contract_id))
        .filter(|invoice| {
            let starts = invoice["start_timestamp"]
                .as_str()
                .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
                .map(|value| value.to_utc());
            let ends = invoice["end_timestamp"]
                .as_str()
                .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
                .map(|value| value.to_utc());
            starts.is_some_and(|start| start <= occurred) && ends.is_some_and(|end| occurred < end)
        })
        .any(|invoice| {
            invoice["line_items"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|line| line["product_id"].as_str() == Some(usage_product_id))
        })
}

pub fn ordered_payment_method_candidates(
    subscription_payment_method_ids: impl IntoIterator<Item = String>,
    stored_payment_method_id: Option<&str>,
) -> Vec<String> {
    let mut candidates = Vec::new();
    for payment_method_id in subscription_payment_method_ids {
        if !payment_method_id.is_empty() && !candidates.contains(&payment_method_id) {
            candidates.push(payment_method_id);
        }
    }
    if let Some(payment_method_id) = stored_payment_method_id.filter(|id| !id.is_empty()) {
        if !candidates
            .iter()
            .any(|candidate| candidate == payment_method_id)
        {
            candidates.push(payment_method_id.to_string());
        }
    }
    candidates
}

pub fn payment_method_owner_matches(
    expected_customer_id: &str,
    payment_method_customer_id: Option<&str>,
) -> bool {
    payment_method_customer_id == Some(expected_customer_id)
}

async fn ensure_stripe_payment_method(user: &User) -> Result<bool> {
    use stripe::{
        Client, Customer, CustomerInvoiceSettings, ListSubscriptions, PaymentMethod, Subscription,
        SubscriptionStatus, UpdateCustomer,
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
    let subscriptions = Subscription::list(
        &client,
        &ListSubscriptions {
            customer: Some(customer_id.clone()),
            limit: Some(100),
            ..Default::default()
        },
    )
    .await?;
    let subscription_payment_method_ids = subscriptions
        .data
        .iter()
        .filter(|subscription| {
            matches!(
                subscription.status,
                SubscriptionStatus::Active
                    | SubscriptionStatus::Trialing
                    | SubscriptionStatus::PastDue
                    | SubscriptionStatus::Unpaid
            )
        })
        .filter_map(|subscription| {
            subscription
                .default_payment_method
                .as_ref()
                .map(|payment_method| payment_method.id().to_string())
        });
    let candidates = ordered_payment_method_candidates(
        subscription_payment_method_ids,
        user.stripe_payment_method_id.as_deref(),
    );
    if candidates.is_empty() {
        return Ok(false);
    }

    let mut last_update_error = None;
    for (candidate_index, payment_method_id) in candidates.into_iter().enumerate() {
        let parsed_payment_method_id = match payment_method_id.parse() {
            Ok(payment_method_id) => payment_method_id,
            Err(_error) => {
                tracing::warn!(
                    user_id = user.id,
                    candidate_index,
                    "Ignoring invalid Stripe payment method ID"
                );
                continue;
            }
        };
        let payment_method =
            match PaymentMethod::retrieve(&client, &parsed_payment_method_id, &[]).await {
                Ok(payment_method) => payment_method,
                Err(_error) => {
                    tracing::warn!(
                        user_id = user.id,
                        candidate_index,
                        "Could not retrieve Stripe payment method candidate"
                    );
                    continue;
                }
            };
        let owner_id = payment_method
            .customer
            .as_ref()
            .map(|customer| customer.id());
        if !payment_method_owner_matches(customer_id.as_ref(), owner_id.as_ref().map(AsRef::as_ref))
        {
            tracing::warn!(
                user_id = user.id,
                candidate_index,
                "Ignoring Stripe payment method attached to a different customer"
            );
            continue;
        }

        match Customer::update(
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
        .await
        {
            Ok(_) => return Ok(true),
            Err(error) => {
                tracing::warn!(
                    user_id = user.id,
                    candidate_index,
                    "Could not set Stripe invoice default payment method"
                );
                last_update_error = Some(error);
            }
        }
    }

    if let Some(error) = last_update_error {
        return Err(error.into());
    }
    Ok(false)
}

async fn stripe_payment_ready(user: &User) -> bool {
    match ensure_stripe_payment_method(user).await {
        Ok(payment_ready) => payment_ready,
        Err(_error) => {
            tracing::warn!(user_id = user.id, "Stripe payment readiness check failed");
            false
        }
    }
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
            let payment_ready = stripe_payment_ready(user).await;
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
    let payment_ready = stripe_payment_ready(user).await;
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
            let error_code = billing_error_code(&error);
            let _ = repository.ensure_account(user.id);
            let _ = repository.mark_provisioning_failed(user.id, error_code);
            tracing::error!(
                user_id = user.id,
                error_code,
                "Failed to provision user in Metronome"
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
                let error_code = billing_error_code(&error);
                for event in chunk {
                    let _ = repository.mark_usage_failed(
                        &event.transaction_id,
                        event.attempts,
                        error_code,
                    );
                }
                tracing::warn!(
                    error_code,
                    "Metronome usage ingest failed; events queued for retry"
                );
            }
        }
    }
}

/// Sample the durable outbox against Metronome's observability APIs. This is
/// read-only at the provider: it verifies event acceptance/customer+metric
/// matching and whether the corresponding product is visible on an invoice.
pub async fn reconcile_usage_outbox(state: Arc<AppState>) {
    let client = match MetronomeClient::from_env() {
        Ok(client) if client.is_enabled() => client,
        _ => return,
    };
    let repository = BillingRepository::new(state.pg_pool.clone());
    let events = match repository.usage_for_reconciliation(25) {
        Ok(events) if !events.is_empty() => events,
        Ok(_) => return,
        Err(error) => {
            tracing::error!("Failed to load billing reconciliation sample: {}", error);
            return;
        }
    };
    let transaction_ids: Vec<String> = events
        .iter()
        .map(|event| event.transaction_id.clone())
        .collect();
    let search = match client.search_events(&transaction_ids).await {
        Ok(search) => search,
        Err(error) => {
            tracing::warn!(
                error_code = billing_error_code(&error),
                "Metronome event reconciliation deferred"
            );
            return;
        }
    };
    let metric_id = client
        .config
        .billable_metric_id
        .as_deref()
        .expect("validated configuration has billable metric ID");
    let product_id = client
        .config
        .usage_product_id
        .as_deref()
        .expect("validated configuration has usage product ID");
    let mut invoices_by_user: HashMap<i32, Value> = HashMap::new();
    for event in events {
        let provider_status = provider_event_status(&search, &event.transaction_id, metric_id);
        let invoice_visible = if provider_status == "matched" {
            let account = match repository.get_account(event.user_id) {
                Ok(Some(account)) => account,
                _ => continue,
            };
            let (Some(customer_id), Some(contract_id)) = (
                account.metronome_customer_id.as_deref(),
                account.metronome_contract_id.as_deref(),
            ) else {
                continue;
            };
            if let Entry::Vacant(entry) = invoices_by_user.entry(event.user_id) {
                match client.list_invoices(customer_id).await {
                    Ok(invoices) => {
                        entry.insert(invoices);
                    }
                    Err(error) => {
                        tracing::warn!(
                            user_id = event.user_id,
                            error_code = billing_error_code(&error),
                            "Metronome invoice reconciliation deferred"
                        );
                        continue;
                    }
                }
            }
            invoice_contains_usage(
                &invoices_by_user[&event.user_id],
                contract_id,
                product_id,
                event.occurred_at,
            )
        } else {
            false
        };
        if let Err(error) = repository.mark_usage_reconciled(
            &event.transaction_id,
            provider_status,
            invoice_visible,
        ) {
            tracing::error!("Failed to persist billing reconciliation result: {}", error);
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
    MetronomeConfig::from_env().validate()?;
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

pub fn begin_usage_intent(state: &Arc<AppState>, user_id: i32, event_type: &str) -> Result<String> {
    MetronomeConfig::from_env().validate()?;
    let transaction_id = uuid::Uuid::new_v4().to_string();
    let repository = BillingRepository::new(state.pg_pool.clone());
    repository.ensure_account(user_id)?;
    repository.begin_usage_intent(user_id, event_type, &transaction_id)?;
    Ok(transaction_id)
}

pub fn finalize_usage_intent(
    state: &Arc<AppState>,
    transaction_id: &str,
    cost_usd: f64,
) -> Result<()> {
    let repository = BillingRepository::new(state.pg_pool.clone());
    if cost_usd <= 0.0 {
        repository.abandon_usage_intent(transaction_id)?;
        return Ok(());
    }
    repository.finalize_usage_intent(
        transaction_id,
        cost_to_microusd(cost_usd)?,
        chrono::Utc::now().timestamp() as i32,
    )?;
    Ok(())
}

pub fn abandon_usage_intent(state: &Arc<AppState>, transaction_id: &str) {
    let repository = BillingRepository::new(state.pg_pool.clone());
    let _ = repository.abandon_usage_intent(transaction_id);
}

pub fn usage_entitled_from_account_state(
    usage_entitled: bool,
    overage_enabled: bool,
    payment_ready: bool,
    available_usage_usd: Option<f64>,
) -> bool {
    usage_entitled
        || available_usage_usd.is_some_and(|available| available > 0.0)
        || (overage_enabled && payment_ready)
}

pub fn has_usage_entitlement(state: &Arc<AppState>, user_id: i32) -> Result<bool> {
    MetronomeConfig::from_env().validate()?;
    let repository = BillingRepository::new(state.pg_pool.clone());
    let account = repository.ensure_account(user_id)?;
    let available_usage_usd = if account.usage_entitled {
        None
    } else {
        // The usage outbox is the durable source for metered usage. A billing
        // webhook can race or carry stale balance state, so a cached false flag
        // must not override included allowance that is still available.
        Some(local_usage_balance(&repository, &account, chrono::Utc::now())?.available_usage_usd)
    };
    let entitled = usage_entitled_from_account_state(
        account.usage_entitled,
        account.overage_enabled,
        account.payment_ready,
        available_usage_usd,
    );
    if entitled && !account.usage_entitled {
        repository.set_usage_entitled(user_id, true)?;
        let _ = state.user_core.clear_last_credits_notification(user_id);
    }
    Ok(entitled)
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
