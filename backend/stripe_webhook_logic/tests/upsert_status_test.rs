use stripe_webhook_logic::{
    decide_subscription_upsert, SubscriptionStatus, SubscriptionUpsertDecision,
    SubscriptionUpsertEvent,
};

#[test]
fn active_or_trialing_subscriptions_can_apply() {
    for status in [SubscriptionStatus::Active, SubscriptionStatus::Trialing] {
        assert_eq!(
            decide_subscription_upsert(SubscriptionUpsertEvent::Created, status, false, false),
            SubscriptionUpsertDecision::ApplySubscription
        );
        assert_eq!(
            decide_subscription_upsert(SubscriptionUpsertEvent::Updated, status, false, false),
            SubscriptionUpsertDecision::ApplySubscription
        );
    }
}

#[test]
fn inactive_subscription_updates_never_regrant_access() {
    for status in [
        SubscriptionStatus::Canceled,
        SubscriptionStatus::Incomplete,
        SubscriptionStatus::IncompleteExpired,
        SubscriptionStatus::PastDue,
        SubscriptionStatus::Paused,
        SubscriptionStatus::Unpaid,
    ] {
        assert_eq!(
            decide_subscription_upsert(SubscriptionUpsertEvent::Updated, status, false, false),
            SubscriptionUpsertDecision::IgnoreInactiveSubscription
        );
    }
}
