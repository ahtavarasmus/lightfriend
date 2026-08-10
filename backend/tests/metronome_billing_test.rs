use backend::services::metronome_billing::{
    billing_period_from_anchor, contract_starting_at, cost_to_microusd,
    customer_usage_balance_from_response, invoice_contains_usage, legacy_overage_migration_target,
    local_usage_balance_from_total, ordered_payment_method_candidates,
    payment_method_owner_matches, provider_event_status, provider_http_error, select_contract_id,
    usage_entitled_from_account_state, usage_invoice_total_usd, verify_webhook_signature,
    MetronomeConfig,
};
use hmac::{Hmac, Mac};
use sha2::Sha256;

fn sign(secret: &str, date: &str, body: &[u8]) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(date.as_bytes());
    mac.update(b"\n");
    mac.update(body);
    hex::encode(mac.finalize().into_bytes())
}

#[test]
fn verifies_exact_body_and_rejects_tampering() {
    let secret = "correct-horse-battery-staple";
    let date = "Mon, 02 Jan 2006 22:04:05 GMT";
    let body = br#"{"id":"evt_1","type":"payment_gate.payment_status"}"#;
    let signature = sign(secret, date, body);
    let now = chrono::DateTime::parse_from_rfc2822(date)
        .unwrap()
        .timestamp();

    verify_webhook_signature(secret, date, body, &signature, now).unwrap();
    assert!(verify_webhook_signature(secret, date, b"{}", &signature, now).is_err());
}

#[test]
fn rejects_webhooks_outside_the_five_minute_window() {
    let secret = "secret";
    let date = "Mon, 02 Jan 2006 22:04:05 GMT";
    let body = b"{}";
    let signature = sign(secret, date, body);
    let sent_at = chrono::DateTime::parse_from_rfc2822(date)
        .unwrap()
        .timestamp();

    assert!(verify_webhook_signature(secret, date, body, &signature, sent_at + 301).is_err());
}

#[test]
fn converts_fractional_dollar_costs_without_float_ledger_values() {
    assert_eq!(cost_to_microusd(0.013).unwrap(), 13_000);
    assert_eq!(cost_to_microusd(25.0).unwrap(), 25_000_000);
    assert!(cost_to_microusd(0.0).is_err());
    assert!(cost_to_microusd(f64::NAN).is_err());
}

#[test]
fn rounds_contract_start_to_the_current_hour() {
    let now = chrono::DateTime::parse_from_rfc3339("2026-07-24T15:47:31.987654321Z")
        .unwrap()
        .to_utc();

    assert_eq!(contract_starting_at(now), "2026-07-24T15:00:00+00:00");
}

#[test]
fn local_usage_summary_rolls_monthly_and_reports_overage() {
    let anchor = chrono::DateTime::parse_from_rfc3339("2026-06-15T10:30:00Z")
        .unwrap()
        .timestamp() as i32;
    let now = chrono::DateTime::parse_from_rfc3339("2026-08-20T12:00:00Z")
        .unwrap()
        .to_utc();
    let (period_start, period_end) = billing_period_from_anchor(anchor, now).unwrap();

    assert_eq!(period_start.to_rfc3339(), "2026-08-15T10:30:00+00:00");
    assert_eq!(period_end.to_rfc3339(), "2026-09-15T10:30:00+00:00");

    let summary = local_usage_balance_from_total(27_500_000, period_start, period_end);
    assert_eq!(summary.available_usage_usd, 0.0);
    assert_eq!(summary.included_allowance_usd, 25.0);
    assert_eq!(summary.included_usage_used_usd, 25.0);
    assert_eq!(summary.overage_usage_usd, Some(2.5));
    assert_eq!(
        summary.resets_at.as_deref(),
        Some("2026-09-15T10:30:00+00:00")
    );
}

#[test]
fn preserves_legacy_auto_topup_opt_in_when_payment_is_ready() {
    assert_eq!(
        legacy_overage_migration_target(true, true, false),
        Some(true)
    );
}

#[test]
fn keeps_overage_off_for_users_without_a_legacy_opt_in() {
    assert_eq!(
        legacy_overage_migration_target(false, true, false),
        Some(false)
    );
}

#[test]
fn waits_for_payment_setup_before_preserving_legacy_opt_in() {
    assert_eq!(legacy_overage_migration_target(true, false, false), None);
}

#[test]
fn never_reapplies_an_already_migrated_preference() {
    assert_eq!(legacy_overage_migration_target(true, true, true), None);
    assert_eq!(legacy_overage_migration_target(false, true, true), None);
}

#[test]
fn ready_overage_consent_survives_a_stale_entitlement_flag() {
    assert!(usage_entitled_from_account_state(false, true, true));
    assert!(!usage_entitled_from_account_state(false, true, false));
    assert!(!usage_entitled_from_account_state(false, false, true));
    assert!(usage_entitled_from_account_state(true, false, false));
}

#[test]
fn prefers_current_subscription_payment_method_over_legacy_reference() {
    assert_eq!(
        ordered_payment_method_candidates(["pm_current".to_string()], Some("pm_stale_legacy")),
        vec!["pm_current".to_string(), "pm_stale_legacy".to_string()]
    );
}

#[test]
fn deduplicates_payment_method_candidates() {
    assert_eq!(
        ordered_payment_method_candidates(
            ["pm_current".to_string(), "pm_current".to_string()],
            Some("pm_current")
        ),
        vec!["pm_current".to_string()]
    );
}

#[test]
fn accepts_only_payment_methods_attached_to_the_current_customer() {
    assert!(payment_method_owner_matches(
        "cus_current",
        Some("cus_current")
    ));
    assert!(!payment_method_owner_matches(
        "cus_current",
        Some("cus_other")
    ));
    assert!(!payment_method_owner_matches("cus_current", None));
}

fn complete_config() -> MetronomeConfig {
    MetronomeConfig {
        enabled: true,
        api_url: "https://api.metronome.com".to_string(),
        api_key: "test-key".to_string(),
        package_alias: "lightfriend-monthly".to_string(),
        event_type: "lightfriend_usage".to_string(),
        billable_metric_id: Some("metric-1".to_string()),
        usage_product_id: Some("product-1".to_string()),
        webhook_secret: "webhook-secret".to_string(),
        legacy_credit_product_id: Some("legacy-product".to_string()),
        credit_type_id: Some("credit-type".to_string()),
    }
}

#[test]
fn enabled_billing_requires_complete_configuration() {
    let mut config = complete_config();
    config.webhook_secret.clear();
    config.billable_metric_id = None;
    let error = config.validate().unwrap_err().to_string();
    assert!(error.contains("METRONOME_WEBHOOK_SECRET"));
    assert!(error.contains("METRONOME_BILLABLE_METRIC_ID"));
    config.enabled = false;
    assert!(config.validate().is_ok());
}

#[test]
fn contract_selection_rejects_ambiguous_provider_results() {
    let ambiguous = serde_json::json!({"data": [
        {"id": "contract-a", "uniqueness_key": "other-a"},
        {"id": "contract-b", "uniqueness_key": "other-b"}
    ]});
    assert!(select_contract_id(&ambiguous, "lightfriend-contract-7").is_err());

    let exact = serde_json::json!({"data": [
        {"id": "contract-a", "uniqueness_key": "other-a"},
        {"id": "contract-b", "uniqueness_key": "lightfriend-contract-7"}
    ]});
    assert_eq!(
        select_contract_id(&exact, "lightfriend-contract-7").unwrap(),
        Some("contract-b".to_string())
    );
}

#[test]
fn provider_http_errors_never_include_raw_response_bodies() {
    let raw_secret = "customer_email@example.com token=super-secret";
    let error = provider_http_error(reqwest::StatusCode::BAD_REQUEST).to_string();
    assert!(error.contains("HTTP 400"));
    assert!(!error.contains(raw_secret));
    assert!(!error.contains("customer_email"));
}

#[test]
fn reconciliation_requires_customer_and_expected_metric_matches() {
    let response = serde_json::json!([
        {
            "transaction_id": "tx-good",
            "matched_customer": {"id": "customer-1"},
            "matched_billable_metrics": [{"id": "metric-1"}]
        },
        {
            "transaction_id": "tx-wrong-metric",
            "matched_customer": {"id": "customer-1"},
            "matched_billable_metrics": [{"id": "metric-other"}]
        }
    ]);
    assert_eq!(
        provider_event_status(&response, "tx-good", "metric-1"),
        "matched"
    );
    assert_eq!(
        provider_event_status(&response, "tx-wrong-metric", "metric-1"),
        "unmatched"
    );
    assert_eq!(
        provider_event_status(&response, "tx-missing", "metric-1"),
        "missing"
    );
}

#[test]
fn reconciliation_detects_usage_product_on_the_covering_invoice() {
    let invoices = serde_json::json!({"data": [{
        "contract_id": "contract-1",
        "start_timestamp": "2026-08-01T00:00:00Z",
        "end_timestamp": "2026-09-01T00:00:00Z",
        "line_items": [{"product_id": "product-1"}]
    }]});
    let occurred_at = chrono::DateTime::parse_from_rfc3339("2026-08-15T12:00:00Z")
        .unwrap()
        .timestamp() as i32;
    assert!(invoice_contains_usage(
        &invoices,
        "contract-1",
        "product-1",
        occurred_at
    ));
    assert!(!invoice_contains_usage(
        &invoices,
        "contract-1",
        "product-other",
        occurred_at
    ));
}

#[test]
fn billing_summary_uses_the_monthly_credit_instead_of_long_lived_credits() {
    let response = serde_json::json!({"data": [
        {
            "type": "CREDIT",
            "balance": 1875,
            "access_schedule": {"schedule_items": [{
                "amount": 2500,
                "starting_at": "2026-08-01T00:00:00Z",
                "ending_before": "2026-09-01T00:00:00Z"
            }]}
        },
        {
            "type": "CREDIT",
            "balance": 4000,
            "access_schedule": {"schedule_items": [{
                "amount": 4000,
                "starting_at": "2026-01-01T00:00:00Z",
                "ending_before": "2036-01-01T00:00:00Z"
            }]}
        }
    ]});
    let now = chrono::DateTime::parse_from_rfc3339("2026-08-09T12:00:00Z")
        .unwrap()
        .to_utc();

    let summary = customer_usage_balance_from_response(&response, now);

    assert_eq!(summary.available_usage_usd, 18.75);
    assert_eq!(summary.included_allowance_usd, 25.0);
    assert_eq!(summary.included_usage_used_usd, 6.25);
    assert_eq!(
        summary.period_start_at.as_deref(),
        Some("2026-08-01T00:00:00+00:00")
    );
    assert_eq!(
        summary.resets_at.as_deref(),
        Some("2026-09-01T00:00:00+00:00")
    );
}

#[test]
fn overage_total_includes_current_period_usage_invoices_only() {
    let invoices = serde_json::json!({"data": [
        {
            "contract_id": "contract-1",
            "type": "USAGE",
            "status": "FINALIZED",
            "start_timestamp": "2026-08-01T00:00:00Z",
            "end_timestamp": "2026-08-08T00:00:00Z",
            "total": 1000
        },
        {
            "contract_id": "contract-1",
            "type": "USAGE",
            "status": "DRAFT",
            "start_timestamp": "2026-08-08T00:00:00Z",
            "end_timestamp": "2026-09-01T00:00:00Z",
            "total": 325
        },
        {
            "contract_id": "contract-1",
            "type": "USAGE",
            "status": "VOID",
            "start_timestamp": "2026-08-01T00:00:00Z",
            "end_timestamp": "2026-08-08T00:00:00Z",
            "total": 9999
        },
        {
            "contract_id": "contract-1",
            "type": "CONTRACT_SCHEDULED",
            "status": "FINALIZED",
            "start_timestamp": "2026-08-01T00:00:00Z",
            "end_timestamp": "2026-09-01T00:00:00Z",
            "total": 6000
        },
        {
            "contract_id": "contract-other",
            "type": "USAGE",
            "status": "DRAFT",
            "start_timestamp": "2026-08-01T00:00:00Z",
            "end_timestamp": "2026-09-01T00:00:00Z",
            "total": 5000
        }
    ]});
    let period_start = chrono::DateTime::parse_from_rfc3339("2026-08-01T00:00:00Z")
        .unwrap()
        .to_utc();
    let period_end = chrono::DateTime::parse_from_rfc3339("2026-09-01T00:00:00Z")
        .unwrap()
        .to_utc();

    assert_eq!(
        usage_invoice_total_usd(&invoices, "contract-1", period_start, period_end),
        13.25
    );
}
