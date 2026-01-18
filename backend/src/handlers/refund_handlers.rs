use axum::{extract::State, http::StatusCode, Json};
use serde::Serialize;
use serde_json::json;
use std::sync::Arc;
use stripe::Client;

use crate::{handlers::auth_middleware::AuthUser, AppState};

/// Response for refund eligibility check
#[derive(Serialize)]
pub struct RefundEligibilityResponse {
    pub eligible: bool,
    pub refund_type: Option<String>, // "subscription"
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

const SUBSCRIPTION_USAGE_THRESHOLD: f32 = 30.0; // 30%
const REFUND_WINDOW_DAYS: i64 = 7;
const CONTACT_EMAIL: &str = "rasmus@ahtava.com";

/// Calculate max credits_left for a user based on country and plan type
/// Mirrors logic from profile_handlers.rs:recalculate_credits_for_country_change
pub async fn get_max_credits_left(
    state: &Arc<AppState>,
    country: Option<&str>,
    plan_type: Option<&str>,
) -> f32 {
    use crate::api::twilio_pricing::get_euro_country_pricing;

    let plan_messages: f32 = match plan_type {
        Some("digest") => 120.0,
        _ => 40.0, // monitor or default
    };

    // US/CA: credits_left is message count (always 400 for all plans)
    if matches!(country, Some("US") | Some("CA")) {
        400.0
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
            customer: Some(
                customer_id
                    .parse()
                    .map_err(|e| format!("Invalid customer ID: {}", e))?,
            ),
            status: Some(stripe::InvoiceStatus::Paid),
            limit: Some(100),
            ..Default::default()
        },
    )
    .await
    .map_err(|e| format!("Failed to list invoices: {}", e))?;

    // Find the earliest paid invoice that's not a trial-related €0 invoice
    // Sort by created date and find first non-zero payment
    let mut paid_invoices: Vec<_> = invoices
        .data
        .into_iter()
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
            customer: Some(
                customer_id
                    .parse()
                    .map_err(|e| format!("Invalid customer ID: {}", e))?,
            ),
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
            customer: Some(
                customer_id
                    .parse()
                    .map_err(|e| format!("Invalid customer ID: {}", e))?,
            ),
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
    let user = state
        .user_core
        .find_by_id(user_id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "User not found"})),
            )
        })?;

    // Check if already refunded
    let refund_info = state
        .user_repository
        .get_refund_info(user_id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?;

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

    let stripe_secret_key =
        std::env::var("STRIPE_SECRET_KEY").expect("STRIPE_SECRET_KEY must be set");
    let client = Client::new(stripe_secret_key);

    let now = chrono::Utc::now().timestamp();

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

    // Check if BYOT plan - no credit usage check needed, only 7-day window
    if user.plan_type.as_deref() == Some("byot") {
        let first_paid_timestamp = get_first_paid_invoice_timestamp(&client, &customer_id)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))))?;

        let first_paid_timestamp = match first_paid_timestamp {
            Some(ts) => ts,
            None => {
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
                reason: format!(
                    "Refund window expired {} days ago.",
                    days_since_payment - REFUND_WINDOW_DAYS
                ),
                usage_percent: None,
                days_remaining: Some(0),
                refund_amount_cents: None,
                already_refunded: false,
                contact_email: CONTACT_EMAIL.to_string(),
            }));
        }

        // BYOT is eligible - no usage check needed
        let payment_info = get_latest_subscription_payment_intent(&client, &customer_id)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))))?;

        let refund_amount_cents = payment_info.map(|(_, amount)| amount);
        let days_remaining = (REFUND_WINDOW_DAYS - days_since_payment) as i32;

        return Ok(Json(RefundEligibilityResponse {
            eligible: true,
            refund_type: Some("subscription".to_string()),
            reason: "BYOT plan - eligible for refund within 7 days.".to_string(),
            usage_percent: None,
            days_remaining: Some(days_remaining),
            refund_amount_cents,
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
            reason: format!(
                "Refund window expired {} days ago.",
                days_since_payment - REFUND_WINDOW_DAYS
            ),
            usage_percent: None,
            days_remaining: Some(0),
            refund_amount_cents: None,
            already_refunded: false,
            contact_email: CONTACT_EMAIL.to_string(),
        }));
    }

    // Calculate usage percentage
    let detected_country = crate::utils::country::get_country_code_from_phone(&user.phone_number);
    let max_credits = get_max_credits_left(
        &state,
        detected_country.as_deref(),
        user.plan_type.as_deref(),
    )
    .await;

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
            reason: format!(
                "You've used {:.0}% of your credits. Refunds require less than {}% usage.",
                usage_percent, SUBSCRIPTION_USAGE_THRESHOLD as i32
            ),
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
        reason: format!(
            "You've used {:.0}% of your credits (max {}% for refund).",
            usage_percent, SUBSCRIPTION_USAGE_THRESHOLD as i32
        ),
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
    let eligibility = get_refund_eligibility(State(state.clone()), auth_user).await?;

    if !eligibility.eligible {
        return Ok(Json(RefundRequestResponse {
            success: false,
            message: eligibility.reason.clone(),
            refund_id: None,
        }));
    }

    let user = state
        .user_core
        .find_by_id(user_id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "User not found"})),
            )
        })?;

    let customer_id = user.stripe_customer_id.as_ref().ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "No Stripe customer ID"})),
        )
    })?;

    let stripe_secret_key =
        std::env::var("STRIPE_SECRET_KEY").expect("STRIPE_SECRET_KEY must be set");
    let client = Client::new(stripe_secret_key);

    // Get payment intent to refund
    let (payment_intent_id, _amount) = get_latest_subscription_payment_intent(&client, customer_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))))?
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "No payment found to refund"})),
            )
        })?;

    // Create the refund
    let refund = stripe::Refund::create(
        &client,
        stripe::CreateRefund {
            payment_intent: Some(payment_intent_id.parse().map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Invalid payment intent ID: {}", e)})),
                )
            })?),
            reason: Some(stripe::RefundReasonFilter::RequestedByCustomer),
            ..Default::default()
        },
    )
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to create refund: {}", e)})),
        )
    })?;

    // Cancel the subscription
    if let Ok(Some(sub_id)) = get_active_subscription_id(&client, customer_id).await {
        let _ = stripe::Subscription::cancel(
            &client,
            &sub_id.parse().unwrap(),
            stripe::CancelSubscription::default(),
        )
        .await;

        // Clear subscription data
        let _ = state.user_repository.set_subscription_tier(user_id, None);
        let _ = state.user_repository.update_sub_credits(user_id, 0.0);
    }

    // Mark as refunded
    let now = chrono::Utc::now().timestamp() as i32;
    state
        .user_repository
        .set_has_refunded(user_id, now)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to update refund status: {}", e)})),
            )
        })?;

    tracing::info!(
        "Refund processed for user {}: refund_id={}",
        user_id,
        refund.id
    );

    Ok(Json(RefundRequestResponse {
        success: true,
        message: "Refund processed successfully. You should see the refund in 5-10 business days."
            .to_string(),
        refund_id: Some(refund.id.to_string()),
    }))
}
