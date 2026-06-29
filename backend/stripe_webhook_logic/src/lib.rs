#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanType {
    Assistant,
    Autopilot,
}

impl PlanType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Assistant => "assistant",
            Self::Autopilot => "autopilot",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductKind {
    Assistant,
    Autopilot,
    CreditsAddOn,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubscriptionAge {
    PreLegacyCutoff,
    AtOrAfterLegacyCutoff,
    Missing,
}

pub fn plan_type_for_product(
    product_kind: ProductKind,
    subscription_age: SubscriptionAge,
) -> Option<PlanType> {
    match product_kind {
        ProductKind::Assistant => Some(PlanType::Autopilot),
        ProductKind::Autopilot => Some(PlanType::Autopilot),
        ProductKind::CreditsAddOn => None,
        ProductKind::Unknown if subscription_age == SubscriptionAge::PreLegacyCutoff => {
            Some(PlanType::Autopilot)
        }
        ProductKind::Unknown => None,
    }
}

pub fn highest_plan(left: PlanType, right: PlanType) -> PlanType {
    match (left, right) {
        (PlanType::Autopilot, _) | (_, PlanType::Autopilot) => PlanType::Autopilot,
        (PlanType::Assistant, PlanType::Assistant) => PlanType::Assistant,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubscriptionUpsertEvent {
    Created,
    Updated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubscriptionStatus {
    Active,
    Trialing,
    Canceled,
    Incomplete,
    IncompleteExpired,
    PastDue,
    Paused,
    Unpaid,
}

pub fn subscription_allows_access(status: SubscriptionStatus) -> bool {
    matches!(
        status,
        SubscriptionStatus::Active | SubscriptionStatus::Trialing
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubscriptionUpsertDecision {
    ApplySubscription,
    IgnoreInactiveSubscription,
    RevokeInactiveSubscription,
    IgnorePlanChangeUpdate,
    IgnoreCancelAtPeriodEndUpdate,
}

pub fn decide_subscription_upsert(
    event: SubscriptionUpsertEvent,
    status: SubscriptionStatus,
    is_plan_change: bool,
    cancel_at_period_end: bool,
) -> SubscriptionUpsertDecision {
    match event {
        SubscriptionUpsertEvent::Created if subscription_allows_access(status) => {
            SubscriptionUpsertDecision::ApplySubscription
        }
        SubscriptionUpsertEvent::Created => SubscriptionUpsertDecision::IgnoreInactiveSubscription,
        SubscriptionUpsertEvent::Updated if is_plan_change => {
            SubscriptionUpsertDecision::IgnorePlanChangeUpdate
        }
        SubscriptionUpsertEvent::Updated if !subscription_allows_access(status) => {
            SubscriptionUpsertDecision::RevokeInactiveSubscription
        }
        SubscriptionUpsertEvent::Updated if cancel_at_period_end => {
            SubscriptionUpsertDecision::IgnoreCancelAtPeriodEndUpdate
        }
        SubscriptionUpsertEvent::Updated => SubscriptionUpsertDecision::ApplySubscription,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActiveSubscriptionSnapshot {
    pub is_deleted_subscription: bool,
    pub product_kind: ProductKind,
    pub subscription_age: SubscriptionAge,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubscriptionDeletedDecision {
    IgnorePlanChangeDelete,
    ClearSubscription,
    KeepPlan(PlanType),
}

pub fn decide_subscription_delete(
    is_plan_change: bool,
    active_subscriptions: &[ActiveSubscriptionSnapshot],
) -> SubscriptionDeletedDecision {
    if is_plan_change {
        return SubscriptionDeletedDecision::IgnorePlanChangeDelete;
    }

    active_subscriptions
        .iter()
        .filter(|subscription| !subscription.is_deleted_subscription)
        .filter_map(|subscription| {
            plan_type_for_product(subscription.product_kind, subscription.subscription_age)
        })
        .reduce(highest_plan)
        .map(SubscriptionDeletedDecision::KeepPlan)
        .unwrap_or(SubscriptionDeletedDecision::ClearSubscription)
}

/// Decide whether a `customer.subscription.deleted` webhook should clear the
/// local subscription state.
///
/// Stripe can briefly return the just-deleted subscription from an immediately
/// following `Subscription::list(status=active)` call. Treating that stale row
/// as active keeps the user subscribed locally after cancellation, so callers
/// must exclude the subscription from the deletion event itself.
pub fn should_clear_subscription_after_delete<'a, I>(
    deleted_subscription_id: &str,
    active_subscription_ids: I,
) -> bool
where
    I: IntoIterator<Item = &'a str>,
{
    !active_subscription_ids
        .into_iter()
        .any(|subscription_id| subscription_id != deleted_subscription_id)
}

#[cfg(kani)]
mod proofs {
    use super::{
        decide_subscription_delete, decide_subscription_upsert, highest_plan,
        plan_type_for_product, ActiveSubscriptionSnapshot, PlanType, ProductKind, SubscriptionAge,
        SubscriptionDeletedDecision, SubscriptionStatus, SubscriptionUpsertDecision,
        SubscriptionUpsertEvent,
    };

    fn any_product_kind() -> ProductKind {
        match kani::any::<u8>() % 4 {
            0 => ProductKind::Assistant,
            1 => ProductKind::Autopilot,
            2 => ProductKind::CreditsAddOn,
            _ => ProductKind::Unknown,
        }
    }

    fn any_subscription_age() -> SubscriptionAge {
        match kani::any::<u8>() % 3 {
            0 => SubscriptionAge::PreLegacyCutoff,
            1 => SubscriptionAge::AtOrAfterLegacyCutoff,
            _ => SubscriptionAge::Missing,
        }
    }

    fn any_subscription_status() -> SubscriptionStatus {
        match kani::any::<u8>() % 8 {
            0 => SubscriptionStatus::Active,
            1 => SubscriptionStatus::Trialing,
            2 => SubscriptionStatus::Canceled,
            3 => SubscriptionStatus::Incomplete,
            4 => SubscriptionStatus::IncompleteExpired,
            5 => SubscriptionStatus::PastDue,
            6 => SubscriptionStatus::Paused,
            _ => SubscriptionStatus::Unpaid,
        }
    }

    fn any_active_subscription() -> ActiveSubscriptionSnapshot {
        ActiveSubscriptionSnapshot {
            is_deleted_subscription: kani::any(),
            product_kind: any_product_kind(),
            subscription_age: any_subscription_age(),
        }
    }

    #[kani::proof]
    fn proof_product_classification_all_states() {
        let product_kind = any_product_kind();
        let subscription_age = any_subscription_age();
        let plan = plan_type_for_product(product_kind, subscription_age);

        match product_kind {
            ProductKind::Assistant => assert_eq!(plan, Some(PlanType::Autopilot)),
            ProductKind::Autopilot => assert_eq!(plan, Some(PlanType::Autopilot)),
            ProductKind::CreditsAddOn => assert_eq!(plan, None),
            ProductKind::Unknown if subscription_age == SubscriptionAge::PreLegacyCutoff => {
                assert_eq!(plan, Some(PlanType::Autopilot));
            }
            ProductKind::Unknown => assert_eq!(plan, None),
        }
    }

    #[kani::proof]
    fn proof_highest_plan_all_states() {
        let left = if kani::any() {
            PlanType::Assistant
        } else {
            PlanType::Autopilot
        };
        let right = if kani::any() {
            PlanType::Assistant
        } else {
            PlanType::Autopilot
        };
        let highest = highest_plan(left, right);

        if left == PlanType::Autopilot || right == PlanType::Autopilot {
            assert_eq!(highest, PlanType::Autopilot);
        } else {
            assert_eq!(highest, PlanType::Assistant);
        }
    }

    #[kani::proof]
    fn proof_subscription_created_active_or_trialing_applies() {
        let is_plan_change: bool = kani::any();
        let cancel_at_period_end: bool = kani::any();
        let status = if kani::any() {
            SubscriptionStatus::Active
        } else {
            SubscriptionStatus::Trialing
        };

        assert_eq!(
            decide_subscription_upsert(
                SubscriptionUpsertEvent::Created,
                status,
                is_plan_change,
                cancel_at_period_end
            ),
            SubscriptionUpsertDecision::ApplySubscription
        );
    }

    #[kani::proof]
    fn proof_subscription_updated_all_states() {
        let is_plan_change: bool = kani::any();
        let cancel_at_period_end: bool = kani::any();
        let status = any_subscription_status();

        let decision = decide_subscription_upsert(
            SubscriptionUpsertEvent::Updated,
            status,
            is_plan_change,
            cancel_at_period_end,
        );

        if !matches!(
            status,
            SubscriptionStatus::Active | SubscriptionStatus::Trialing
        ) {
            if is_plan_change {
                assert_eq!(decision, SubscriptionUpsertDecision::IgnorePlanChangeUpdate);
            } else {
                assert_eq!(
                    decision,
                    SubscriptionUpsertDecision::RevokeInactiveSubscription
                );
            }
        } else if is_plan_change {
            assert_eq!(decision, SubscriptionUpsertDecision::IgnorePlanChangeUpdate);
        } else if cancel_at_period_end {
            assert_eq!(
                decision,
                SubscriptionUpsertDecision::IgnoreCancelAtPeriodEndUpdate
            );
        } else {
            assert_eq!(decision, SubscriptionUpsertDecision::ApplySubscription);
        }
    }

    #[kani::proof]
    #[kani::unwind(5)]
    fn proof_subscription_delete_plan_change_always_ignored() {
        let active_subscriptions = [
            any_active_subscription(),
            any_active_subscription(),
            any_active_subscription(),
            any_active_subscription(),
        ];
        let len: usize = kani::any();
        kani::assume(len <= active_subscriptions.len());

        assert_eq!(
            decide_subscription_delete(true, &active_subscriptions[..len]),
            SubscriptionDeletedDecision::IgnorePlanChangeDelete
        );
    }

    #[kani::proof]
    #[kani::unwind(5)]
    fn proof_subscription_delete_clears_when_no_other_valid_plan_exists() {
        let active_subscriptions = [
            any_active_subscription(),
            any_active_subscription(),
            any_active_subscription(),
            any_active_subscription(),
        ];
        let len: usize = kani::any();
        kani::assume(len <= active_subscriptions.len());

        let has_other_valid_plan = active_subscriptions[..len].iter().any(|subscription| {
            !subscription.is_deleted_subscription
                && plan_type_for_product(subscription.product_kind, subscription.subscription_age)
                    .is_some()
        });
        kani::assume(!has_other_valid_plan);

        assert_eq!(
            decide_subscription_delete(false, &active_subscriptions[..len]),
            SubscriptionDeletedDecision::ClearSubscription
        );
    }

    #[kani::proof]
    #[kani::unwind(5)]
    fn proof_subscription_delete_keeps_highest_other_valid_plan() {
        let active_subscriptions = [
            any_active_subscription(),
            any_active_subscription(),
            any_active_subscription(),
            any_active_subscription(),
        ];
        let len: usize = kani::any();
        kani::assume(len <= active_subscriptions.len());

        let expected_plan = active_subscriptions[..len]
            .iter()
            .filter_map(|subscription| {
                if subscription.is_deleted_subscription {
                    None
                } else {
                    plan_type_for_product(subscription.product_kind, subscription.subscription_age)
                }
            })
            .reduce(highest_plan);
        kani::assume(expected_plan.is_some());

        assert_eq!(
            decide_subscription_delete(false, &active_subscriptions[..len]),
            SubscriptionDeletedDecision::KeepPlan(expected_plan.unwrap())
        );
    }

    #[kani::proof]
    fn proof_deleted_subscription_itself_never_preserves_state() {
        let active_subscriptions = [ActiveSubscriptionSnapshot {
            is_deleted_subscription: true,
            product_kind: ProductKind::Autopilot,
            subscription_age: SubscriptionAge::AtOrAfterLegacyCutoff,
        }];

        assert_eq!(
            decide_subscription_delete(false, &active_subscriptions),
            SubscriptionDeletedDecision::ClearSubscription
        );
    }

    #[kani::proof]
    fn proof_credits_add_on_never_preserves_subscription() {
        let active_subscriptions = [ActiveSubscriptionSnapshot {
            is_deleted_subscription: false,
            product_kind: ProductKind::CreditsAddOn,
            subscription_age: SubscriptionAge::PreLegacyCutoff,
        }];

        assert_eq!(
            decide_subscription_delete(false, &active_subscriptions),
            SubscriptionDeletedDecision::ClearSubscription
        );
    }

    #[kani::proof]
    fn proof_stripe_delete_empty_active_list_clears_state() {
        assert_eq!(
            decide_subscription_delete(false, &[]),
            SubscriptionDeletedDecision::ClearSubscription
        );
    }
}
