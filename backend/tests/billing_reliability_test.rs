use std::sync::{Arc, Barrier};

use backend::repositories::billing_repository::{BillingRepository, BillingWebhookClaim};
use backend::services::metronome_billing::cost_to_microusd;
use backend::test_utils::{create_test_state, create_test_user, TestUserParams};
use serial_test::serial;

#[test]
#[serial]
fn concurrent_webhook_duplicates_have_one_side_effect_owner_and_can_retry() {
    let state = create_test_state();
    create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let repository = Arc::new(BillingRepository::new(state.pg_pool.clone()));
    let barrier = Arc::new(Barrier::new(3));
    let mut workers = Vec::new();
    for _ in 0..2 {
        let repository = Arc::clone(&repository);
        let barrier = Arc::clone(&barrier);
        workers.push(std::thread::spawn(move || {
            barrier.wait();
            repository
                .claim_webhook("event-concurrent", "payment_gate.payment_status", 120)
                .unwrap()
        }));
    }
    barrier.wait();
    let claims: Vec<BillingWebhookClaim> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert_eq!(
        claims
            .iter()
            .filter(|claim| **claim == BillingWebhookClaim::Claimed)
            .count(),
        1
    );
    assert_eq!(
        claims
            .iter()
            .filter(|claim| **claim == BillingWebhookClaim::InFlight)
            .count(),
        1
    );

    repository
        .fail_webhook("event-concurrent", "processing_failed")
        .unwrap();
    assert_eq!(
        repository
            .claim_webhook("event-concurrent", "payment_gate.payment_status", 120)
            .unwrap(),
        BillingWebhookClaim::Claimed
    );
    repository.complete_webhook("event-concurrent").unwrap();
    assert_eq!(
        repository
            .claim_webhook("event-concurrent", "payment_gate.payment_status", 120)
            .unwrap(),
        BillingWebhookClaim::AlreadyProcessed
    );
}

#[test]
#[serial]
fn pre_action_intent_finalizes_transactionally_into_retryable_outbox() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let repository = BillingRepository::new(state.pg_pool.clone());
    repository.ensure_account(user.id).unwrap();
    repository
        .begin_usage_intent(user.id, "web_chat", "intent-transaction")
        .unwrap();

    let before = repository.reconciliation_summary(0).unwrap();
    assert_eq!(before.stale_open_intents, 1);
    assert!(repository
        .get_usage_event("intent-transaction")
        .unwrap()
        .is_none());

    repository
        .finalize_usage_intent(
            "intent-transaction",
            cost_to_microusd(0.02).unwrap(),
            chrono::Utc::now().timestamp() as i32,
        )
        .unwrap();
    // Finalization is idempotent after a restart/retry.
    repository
        .finalize_usage_intent(
            "intent-transaction",
            cost_to_microusd(0.02).unwrap(),
            chrono::Utc::now().timestamp() as i32,
        )
        .unwrap();
    let event = repository
        .get_usage_event("intent-transaction")
        .unwrap()
        .unwrap();
    assert_eq!(event.status, "pending");
    assert_eq!(event.attempts, 0);
    assert_eq!(repository.claim_due_usage(10).unwrap().len(), 1);
    repository
        .mark_usage_failed("intent-transaction", 0, "provider_unavailable")
        .unwrap();
    let failed = repository
        .get_usage_event("intent-transaction")
        .unwrap()
        .unwrap();
    assert_eq!(failed.status, "failed");
    assert_eq!(failed.last_error.as_deref(), Some("provider_unavailable"));
    // Backlog/provider outage is intentionally not an entitlement webhook.
    assert!(
        repository
            .get_account(user.id)
            .unwrap()
            .unwrap()
            .usage_entitled
    );
}

#[test]
#[serial]
fn reconciliation_summary_contains_only_aggregate_provider_evidence() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let repository = BillingRepository::new(state.pg_pool.clone());
    repository.ensure_account(user.id).unwrap();
    repository
        .enqueue_usage(
            user.id,
            "web_chat",
            cost_to_microusd(0.05).unwrap(),
            chrono::Utc::now().timestamp() as i32,
            Some("reconcile-transaction".to_string()),
        )
        .unwrap();
    assert_eq!(repository.claim_due_usage(10).unwrap().len(), 1);
    repository.mark_usage_sent("reconcile-transaction").unwrap();
    repository
        .mark_usage_reconciled("reconcile-transaction", "matched", true)
        .unwrap();

    let summary = repository.reconciliation_summary(300).unwrap();
    assert_eq!(summary.provider_matched, 1);
    assert_eq!(summary.invoice_visible, 1);
    assert_eq!(summary.provider_unmatched, 0);
    assert_eq!(summary.stale_open_intents, 0);
}

#[test]
#[serial]
fn local_usage_total_includes_all_durable_events_in_the_period_only() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let repository = BillingRepository::new(state.pg_pool.clone());
    repository.ensure_account(user.id).unwrap();

    for (transaction_id, cost, occurred_at) in [
        ("before-period", 5_000_000, 99),
        ("period-pending", 1_250_000, 100),
        ("period-sent", 2_750_000, 150),
        ("after-period", 9_000_000, 200),
    ] {
        repository
            .enqueue_usage(
                user.id,
                "web_chat",
                cost,
                occurred_at,
                Some(transaction_id.to_string()),
            )
            .unwrap();
    }

    assert_eq!(
        repository
            .usage_cost_microusd_between(user.id, 100, 200)
            .unwrap(),
        4_000_000
    );
}
