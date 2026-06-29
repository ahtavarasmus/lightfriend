use backend::utils::stripe_webhook::should_clear_subscription_after_delete;
use stripe_webhook_logic::{
    decide_subscription_delete, decide_subscription_upsert, plan_type_for_product,
    ActiveSubscriptionSnapshot, PlanType, ProductKind, SubscriptionAge,
    SubscriptionDeletedDecision, SubscriptionStatus, SubscriptionUpsertDecision,
    SubscriptionUpsertEvent,
};

#[test]
fn deleted_subscription_absent_from_active_list_clears_state() {
    assert!(should_clear_subscription_after_delete("sub_deleted", []));
}

#[test]
fn stale_deleted_subscription_in_active_list_still_clears_state() {
    assert!(should_clear_subscription_after_delete(
        "sub_deleted",
        ["sub_deleted"]
    ));
    assert!(should_clear_subscription_after_delete(
        "sub_deleted",
        ["sub_deleted", "sub_deleted"]
    ));
}

#[test]
fn any_other_active_subscription_preserves_state() {
    assert!(!should_clear_subscription_after_delete(
        "sub_deleted",
        ["sub_other"]
    ));
    assert!(!should_clear_subscription_after_delete(
        "sub_deleted",
        ["sub_deleted", "sub_other"]
    ));
    assert!(!should_clear_subscription_after_delete(
        "sub_deleted",
        ["sub_other", "sub_deleted"]
    ));
}

#[test]
fn product_classification_handles_new_legacy_and_credit_products() {
    assert_eq!(
        plan_type_for_product(
            ProductKind::Assistant,
            SubscriptionAge::AtOrAfterLegacyCutoff
        ),
        Some(PlanType::Autopilot)
    );
    assert_eq!(
        plan_type_for_product(
            ProductKind::Autopilot,
            SubscriptionAge::AtOrAfterLegacyCutoff
        ),
        Some(PlanType::Autopilot)
    );
    assert_eq!(
        plan_type_for_product(ProductKind::Unknown, SubscriptionAge::PreLegacyCutoff),
        Some(PlanType::Autopilot)
    );
    assert_eq!(
        plan_type_for_product(ProductKind::Unknown, SubscriptionAge::AtOrAfterLegacyCutoff),
        None
    );
    assert_eq!(
        plan_type_for_product(ProductKind::CreditsAddOn, SubscriptionAge::PreLegacyCutoff),
        None
    );
}

#[test]
fn subscription_update_skip_rules_match_webhook_intent() {
    assert_eq!(
        decide_subscription_upsert(
            SubscriptionUpsertEvent::Created,
            SubscriptionStatus::Active,
            true,
            true
        ),
        SubscriptionUpsertDecision::ApplySubscription
    );
    assert_eq!(
        decide_subscription_upsert(
            SubscriptionUpsertEvent::Updated,
            SubscriptionStatus::Active,
            true,
            true
        ),
        SubscriptionUpsertDecision::IgnorePlanChangeUpdate
    );
    assert_eq!(
        decide_subscription_upsert(
            SubscriptionUpsertEvent::Updated,
            SubscriptionStatus::Active,
            false,
            true
        ),
        SubscriptionUpsertDecision::IgnoreCancelAtPeriodEndUpdate
    );
    assert_eq!(
        decide_subscription_upsert(
            SubscriptionUpsertEvent::Updated,
            SubscriptionStatus::Active,
            false,
            false
        ),
        SubscriptionUpsertDecision::ApplySubscription
    );
}

#[test]
fn canceled_subscription_update_never_regrants_access() {
    assert_eq!(
        decide_subscription_upsert(
            SubscriptionUpsertEvent::Updated,
            SubscriptionStatus::Canceled,
            false,
            false
        ),
        SubscriptionUpsertDecision::RevokeInactiveSubscription
    );
}

#[test]
fn delete_ignores_plan_change_events() {
    let active_subscriptions = [ActiveSubscriptionSnapshot {
        is_deleted_subscription: false,
        product_kind: ProductKind::Autopilot,
        subscription_age: SubscriptionAge::AtOrAfterLegacyCutoff,
    }];

    assert_eq!(
        decide_subscription_delete(true, &active_subscriptions),
        SubscriptionDeletedDecision::IgnorePlanChangeDelete
    );
}

#[test]
fn delete_clears_when_only_stale_or_non_plan_subscriptions_remain() {
    let active_subscriptions = [
        ActiveSubscriptionSnapshot {
            is_deleted_subscription: true,
            product_kind: ProductKind::Autopilot,
            subscription_age: SubscriptionAge::AtOrAfterLegacyCutoff,
        },
        ActiveSubscriptionSnapshot {
            is_deleted_subscription: false,
            product_kind: ProductKind::CreditsAddOn,
            subscription_age: SubscriptionAge::PreLegacyCutoff,
        },
    ];

    assert_eq!(
        decide_subscription_delete(false, &active_subscriptions),
        SubscriptionDeletedDecision::ClearSubscription
    );
}

#[test]
fn delete_keeps_highest_remaining_valid_plan() {
    let active_subscriptions = [
        ActiveSubscriptionSnapshot {
            is_deleted_subscription: true,
            product_kind: ProductKind::Autopilot,
            subscription_age: SubscriptionAge::AtOrAfterLegacyCutoff,
        },
        ActiveSubscriptionSnapshot {
            is_deleted_subscription: false,
            product_kind: ProductKind::Assistant,
            subscription_age: SubscriptionAge::AtOrAfterLegacyCutoff,
        },
        ActiveSubscriptionSnapshot {
            is_deleted_subscription: false,
            product_kind: ProductKind::Autopilot,
            subscription_age: SubscriptionAge::AtOrAfterLegacyCutoff,
        },
    ];

    assert_eq!(
        decide_subscription_delete(false, &active_subscriptions),
        SubscriptionDeletedDecision::KeepPlan(PlanType::Autopilot)
    );
}
