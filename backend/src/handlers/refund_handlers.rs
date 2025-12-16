use std::sync::Arc;
use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::Serialize;
use serde_json::json;
use stripe::Client;

use crate::{
    AppState,
    handlers::auth_middleware::AuthUser,
};

/// Response for refund eligibility check
#[derive(Serialize)]
pub struct RefundEligibilityResponse {
    pub eligible: bool,
    pub refund_type: Option<String>,  // "subscription" or "credit_pack"
    pub reason: String,
    pub usage_percent: Option<f32>,
    pub days_remaining: Option<i32>,
    pub refund_amount_cents: Option<i64>,
    pub already_refunded: bool,
    pub contact_email: String,
}

/// Response for refund request
#[derive(Serialize)]
pub struct RefundRequestResponse {
    pub success: bool,
    pub message: String,
    pub refund_id: Option<String>,
}

const SUBSCRIPTION_USAGE_THRESHOLD: f32 = 30.0;  // 30%
const CREDIT_PACK_USAGE_THRESHOLD: f32 = 20.0;   // 20%
const REFUND_WINDOW_DAYS: i64 = 7;
const CONTACT_EMAIL: &str = "rasmus@ahtava.com";

/// Calculate max credits_left for a user based on country and plan type
/// Mirrors logic from profile_handlers.rs:recalculate_credits_for_country_change
async fn get_max_credits_left(
    state: &Arc<AppState>,
    country: Option<&str>,
    plan_type: Option<&str>,
) -> f32 {
    use crate::api::twilio_pricing::get_euro_country_pricing;

    let plan_messages: f32 = match plan_type {
        Some("digest") => 120.0,
        _ => 40.0, // monitor or default
    };

    // US/CA: credits_left is message count
    if matches!(country, Some("US") | Some("CA")) {
        if plan_messages >= 120.0 { 400.0 } else { 200.0 }
    } else if let Some(c) = country {
        // Euro: credits_left is € value based on SMS pricing
        match get_euro_country_pricing(state, c).await {
            Ok(pricing) => plan_messages * pricing.regular_message_price,
            Err(_) => {
                // Fallback: €0.39 per message (0.10 × 3 segments × 1.3 margin)
                plan_messages * 0.39
            }
        }
    } else {
        // Unknown country fallback
        plan_messages * 0.39
    }
}

/// Get the timestamp of the first real payment for a subscription
/// For US/CA: first payment after trial ends
/// For others: first payment (no trial)
async fn get_first_paid_invoice_timestamp(
    client: &Client,
    customer_id: &str,
) -> Result<Option<i64>, String> {
    let invoices = stripe::Invoice::list(
        client,
        &stripe::ListInvoices {
            customer: Some(customer_id.parse().map_err(|e| format!("Invalid customer ID: {}", e))?),
            status: Some(stripe::InvoiceStatus::Paid),
            limit: Some(100),
            ..Default::default()
        },
    )
    .await
    .map_err(|e| format!("Failed to list invoices: {}", e))?;

    // Find the earliest paid invoice that's not a trial-related €0 invoice
    // Sort by created date and find first non-zero payment
    let mut paid_invoices: Vec<_> = invoices.data.into_iter()
        .filter(|inv| inv.amount_paid.unwrap_or(0) > 0)
        .collect();

    paid_invoices.sort_by_key(|inv| inv.created);

    Ok(paid_invoices.first().and_then(|inv| inv.created))
}

/// Get the latest payment intent for refunding
async fn get_latest_subscription_payment_intent(
    client: &Client,
    customer_id: &str,
) -> Result<Option<(String, i64)>, String> {
    let invoices = stripe::Invoice::list(
        client,
        &stripe::ListInvoices {
            customer: Some(customer_id.parse().map_err(|e| format!("Invalid customer ID: {}", e))?),
            status: Some(stripe::InvoiceStatus::Paid),
            limit: Some(10),
            ..Default::default()
        },
    )
    .await
    .map_err(|e| format!("Failed to list invoices: {}", e))?;

    // Get most recent paid invoice with a payment intent
    for invoice in invoices.data {
        if invoice.amount_paid.unwrap_or(0) > 0 {
            if let Some(stripe::Expandable::Id(pi_id)) = invoice.payment_intent {
                return Ok(Some((pi_id.to_string(), invoice.amount_paid.unwrap_or(0))));
            }
        }
    }

    Ok(None)
}

/// Get active subscription ID for user
async fn get_active_subscription_id(
    client: &Client,
    customer_id: &str,
) -> Result<Option<String>, String> {
    let subscriptions = stripe::Subscription::list(
        client,
        &stripe::ListSubscriptions {
            customer: Some(customer_id.parse().map_err(|e| format!("Invalid customer ID: {}", e))?),
            status: Some(stripe::SubscriptionStatusFilter::Active),
            ..Default::default()
        },
    )
    .await
    .map_err(|e| format!("Failed to list subscriptions: {}", e))?;

    // Return first active subscription (tier 2)
    for sub in &subscriptions.data {
        // Check metadata to find tier 2 subscription
        if let Some(tier) = sub.metadata.get("tier") {
            if tier == "tier 2" {
                return Ok(Some(sub.id.to_string()));
            }
        }
    }

    // If no tier 2 found, return first active
    Ok(subscriptions.data.first().map(|s| s.id.to_string()))
}

/// GET /api/refund/eligibility
pub async fn get_refund_eligibility(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<RefundEligibilityResponse>, (StatusCode, Json<serde_json::Value>)> {
    let user_id = auth_user.user_id;

    // Get user
    let user = state.user_core.find_by_id(user_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Database error: {}", e)}))))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, Json(json!({"error": "User not found"}))))?;

    // Check if already refunded
    let refund_info = state.user_repository.get_refund_info(user_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Database error: {}", e)}))))?;

    if let Some(ref info) = refund_info {
        if info.has_refunded == 1 {
            return Ok(Json(RefundEligibilityResponse {
                eligible: false,
                refund_type: None,
                reason: "You have already received a refund.".to_string(),
                usage_percent: None,
                days_remaining: None,
                refund_amount_cents: None,
                already_refunded: true,
                contact_email: CONTACT_EMAIL.to_string(),
            }));
        }
    }

    // Check if user has Stripe customer ID
    let customer_id = match &user.stripe_customer_id {
        Some(id) => id.clone(),
        None => {
            return Ok(Json(RefundEligibilityResponse {
                eligible: false,
                refund_type: None,
                reason: "No payment history found.".to_string(),
                usage_percent: None,
                days_remaining: None,
                refund_amount_cents: None,
                already_refunded: false,
                contact_email: CONTACT_EMAIL.to_string(),
            }));
        }
    };

    let stripe_secret_key = std::env::var("STRIPE_SECRET_KEY")
        .expect("STRIPE_SECRET_KEY must be set");
    let client = Client::new(stripe_secret_key);

    let now = chrono::Utc::now().timestamp();

    // Check credit pack refund eligibility first
    if let Some(ref info) = refund_info {
        if let (Some(pack_amount), Some(pack_timestamp)) = (info.last_credit_pack_amount, info.last_credit_pack_purchase_timestamp) {
            let days_since_purchase = (now - pack_timestamp as i64) / 86400;

            if days_since_purchase <= REFUND_WINDOW_DAYS {
                // Calculate how much of the pack was used
                // We track pack purchases, so we check if credits decreased significantly
                let credits_consumed = pack_amount - user.credits.max(0.0);
                let usage_percent = if pack_amount > 0.0 {
                    (credits_consumed / pack_amount * 100.0).max(0.0)
                } else {
                    0.0
                };

                if usage_percent < CREDIT_PACK_USAGE_THRESHOLD {
                    let days_remaining = (REFUND_WINDOW_DAYS - days_since_purchase) as i32;
                    return Ok(Json(RefundEligibilityResponse {
                        eligible: true,
                        refund_type: Some("credit_pack".to_string()),
                        reason: format!("You've used {:.0}% of your credit pack (max {}% for refund).", usage_percent, CREDIT_PACK_USAGE_THRESHOLD as i32),
                        usage_percent: Some(usage_percent),
                        days_remaining: Some(days_remaining),
                        refund_amount_cents: Some((pack_amount * 100.0) as i64),
                        already_refunded: false,
                        contact_email: CONTACT_EMAIL.to_string(),
                    }));
                } else {
                    return Ok(Json(RefundEligibilityResponse {
                        eligible: false,
                        refund_type: Some("credit_pack".to_string()),
                        reason: format!("You've used {:.0}% of your credit pack. Refunds require less than {}% usage.", usage_percent, CREDIT_PACK_USAGE_THRESHOLD as i32),
                        usage_percent: Some(usage_percent),
                        days_remaining: Some((REFUND_WINDOW_DAYS - days_since_purchase) as i32),
                        refund_amount_cents: None,
                        already_refunded: false,
                        contact_email: CONTACT_EMAIL.to_string(),
                    }));
                }
            }
        }
    }

    // Check subscription refund eligibility
    if user.sub_tier.is_none() {
        return Ok(Json(RefundEligibilityResponse {
            eligible: false,
            refund_type: None,
            reason: "No active subscription found.".to_string(),
            usage_percent: None,
            days_remaining: None,
            refund_amount_cents: None,
            already_refunded: false,
            contact_email: CONTACT_EMAIL.to_string(),
        }));
    }

    // Get first paid invoice timestamp from Stripe
    let first_paid_timestamp = get_first_paid_invoice_timestamp(&client, &customer_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))))?;

    let first_paid_timestamp = match first_paid_timestamp {
        Some(ts) => ts,
        None => {
            // User might still be in trial
            return Ok(Json(RefundEligibilityResponse {
                eligible: false,
                refund_type: Some("subscription".to_string()),
                reason: "Your trial hasn't ended yet, no payment to refund.".to_string(),
                usage_percent: None,
                days_remaining: None,
                refund_amount_cents: None,
                already_refunded: false,
                contact_email: CONTACT_EMAIL.to_string(),
            }));
        }
    };

    let days_since_payment = (now - first_paid_timestamp) / 86400;

    if days_since_payment > REFUND_WINDOW_DAYS {
        return Ok(Json(RefundEligibilityResponse {
            eligible: false,
            refund_type: Some("subscription".to_string()),
            reason: format!("Refund window expired {} days ago.", days_since_payment - REFUND_WINDOW_DAYS),
            usage_percent: None,
            days_remaining: Some(0),
            refund_amount_cents: None,
            already_refunded: false,
            contact_email: CONTACT_EMAIL.to_string(),
        }));
    }

    // Calculate usage percentage
    let max_credits = get_max_credits_left(
        &state,
        user.phone_number_country.as_deref(),
        user.plan_type.as_deref(),
    ).await;

    let credits_used = max_credits - user.credits_left;
    let usage_percent = if max_credits > 0.0 {
        (credits_used / max_credits * 100.0).max(0.0)
    } else {
        0.0
    };

    if usage_percent >= SUBSCRIPTION_USAGE_THRESHOLD {
        return Ok(Json(RefundEligibilityResponse {
            eligible: false,
            refund_type: Some("subscription".to_string()),
            reason: format!("You've used {:.0}% of your credits. Refunds require less than {}% usage.", usage_percent, SUBSCRIPTION_USAGE_THRESHOLD as i32),
            usage_percent: Some(usage_percent),
            days_remaining: Some((REFUND_WINDOW_DAYS - days_since_payment) as i32),
            refund_amount_cents: None,
            already_refunded: false,
            contact_email: CONTACT_EMAIL.to_string(),
        }));
    }

    // Get refund amount from latest invoice
    let payment_info = get_latest_subscription_payment_intent(&client, &customer_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))))?;

    let refund_amount_cents = payment_info.map(|(_, amount)| amount);

    let days_remaining = (REFUND_WINDOW_DAYS - days_since_payment) as i32;

    Ok(Json(RefundEligibilityResponse {
        eligible: true,
        refund_type: Some("subscription".to_string()),
        reason: format!("You've used {:.0}% of your credits (max {}% for refund).", usage_percent, SUBSCRIPTION_USAGE_THRESHOLD as i32),
        usage_percent: Some(usage_percent),
        days_remaining: Some(days_remaining),
        refund_amount_cents,
        already_refunded: false,
        contact_email: CONTACT_EMAIL.to_string(),
    }))
}

/// POST /api/refund/request
pub async fn request_refund(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<RefundRequestResponse>, (StatusCode, Json<serde_json::Value>)> {
    let user_id = auth_user.user_id;

    // First check eligibility (re-verify, don't trust client)
    let eligibility = get_refund_eligibility(State(state.clone()), auth_user.clone()).await?;

    if !eligibility.eligible {
        return Ok(Json(RefundRequestResponse {
            success: false,
            message: eligibility.reason.clone(),
            refund_id: None,
        }));
    }

    let user = state.user_core.find_by_id(user_id)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Database error: {}", e)}))))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, Json(json!({"error": "User not found"}))))?;

    let customer_id = user.stripe_customer_id.as_ref()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, Json(json!({"error": "No Stripe customer ID"}))))?;

    let stripe_secret_key = std::env::var("STRIPE_SECRET_KEY")
        .expect("STRIPE_SECRET_KEY must be set");
    let client = Client::new(stripe_secret_key);

    let refund_type = eligibility.refund_type.as_deref().unwrap_or("subscription");

    // Get payment intent to refund
    let (payment_intent_id, _amount) = if refund_type == "credit_pack" {
        // For credit packs, we need the payment intent from the checkout session
        // This is trickier - for now we'll get the most recent one
        get_latest_subscription_payment_intent(&client, customer_id)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))))?
            .ok_or_else(|| (StatusCode::BAD_REQUEST, Json(json!({"error": "No payment found to refund"}))))?
    } else {
        get_latest_subscription_payment_intent(&client, customer_id)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))))?
            .ok_or_else(|| (StatusCode::BAD_REQUEST, Json(json!({"error": "No payment found to refund"}))))?
    };

    // Create the refund
    let refund = stripe::Refund::create(
        &client,
        stripe::CreateRefund {
            payment_intent: Some(payment_intent_id.parse().map_err(|e| {
                (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Invalid payment intent ID: {}", e)})))
            })?),
            reason: Some(stripe::RefundReasonFilter::RequestedByCustomer),
            ..Default::default()
        },
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to create refund: {}", e)}))))?;

    // If subscription refund, cancel the subscription
    if refund_type == "subscription" {
        if let Ok(Some(sub_id)) = get_active_subscription_id(&client, customer_id).await {
            let _ = stripe::Subscription::cancel(
                &client,
                &sub_id.parse().unwrap(),
                stripe::CancelSubscription::default(),
            ).await;

            // Clear subscription data
            let _ = state.user_repository.set_subscription_tier(user_id, None);
            let _ = state.user_repository.update_sub_credits(user_id, 0.0);
        }
    }

    // If credit pack refund, deduct the credits
    if refund_type == "credit_pack" {
        if let Some(ref info) = state.user_repository.get_refund_info(user_id).ok().flatten() {
            if let Some(pack_amount) = info.last_credit_pack_amount {
                // Remove the pack credits (user may have used some)
                let new_credits = (user.credits - pack_amount).max(0.0);
                let _ = state.user_repository.update_user_credits(user_id, new_credits);
            }
        }
    }

    // Mark as refunded
    let now = chrono::Utc::now().timestamp() as i32;
    state.user_repository.set_has_refunded(user_id, now)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to update refund status: {}", e)}))))?;

    tracing::info!(
        "Refund processed for user {}: type={}, refund_id={}",
        user_id, refund_type, refund.id
    );

    Ok(Json(RefundRequestResponse {
        success: true,
        message: format!("Refund processed successfully. You should see the refund in 5-10 business days."),
        refund_id: Some(refund.id.to_string()),
    }))
}
