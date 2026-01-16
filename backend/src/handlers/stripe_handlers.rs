use stripe::{
    BillingPortalSession, CheckoutSession, Client, CreateBillingPortalSession,
    CreateCheckoutSession, CreateCustomer, CreatePaymentIntent, Customer, PaymentIntent,
    Subscription, UpdateSubscription,
};
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub enum SubscriptionType {
    Hosted,
}
use crate::handlers::auth_middleware::AuthUser;
use crate::AppState;
use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
// Assuming BuyCreditsRequest is defined in billing_models.rs
#[derive(Deserialize, Serialize, Clone, PartialEq)]
pub struct BuyCreditsRequest {
    pub amount_dollars: f32,
}
#[derive(Deserialize)]
pub struct SubscriptionCheckoutBody {
    pub subscription_type: SubscriptionType,
    /// For euro countries: "monitor" (€29/40 msgs) or "digest" (€49/120 msgs)
    pub plan_type: Option<String>,
}

/// Calculate euro credit allocation for Monitor/Digest plans based on user's country pricing.
///
/// Uses dynamic Twilio pricing when available, falls back to €0.39/message otherwise.
/// Sends admin email notification when fallback pricing is used.
///
/// # Arguments
/// * `state` - Application state for database/pricing access
/// * `country_code` - User's phone number country code (e.g., "FI", "DE")
/// * `message_count` - Number of messages the plan includes (40 for Monitor, 120 for Digest)
/// * `plan_name` - Plan name for logging ("Monitor" or "Digest")
///
/// # Returns
/// The euro value to allocate as credits_left (message_count × price_per_message)
async fn calculate_euro_credit_allocation(
    state: &Arc<AppState>,
    country_code: &str,
    message_count: f32,
    plan_name: &str,
) -> f32 {
    use crate::api::twilio_pricing::get_euro_country_pricing;

    // Fallback price: €0.39 per regular message (0.10 × 3 segments × 1.3 margin)
    const FALLBACK_REGULAR_MSG_PRICE: f32 = 0.39;

    match get_euro_country_pricing(state, country_code).await {
        Ok(pricing) => {
            let allocation = message_count * pricing.regular_message_price;
            tracing::info!(
                "{} plan credit allocation for {}: {} messages × €{:.3}/msg = €{:.2}",
                plan_name,
                country_code,
                message_count,
                pricing.regular_message_price,
                allocation
            );
            allocation
        }
        Err(e) => {
            let fallback_allocation = message_count * FALLBACK_REGULAR_MSG_PRICE;
            tracing::warn!(
                "Failed to get pricing for {}, using fallback: {} - allocating €{:.2}",
                country_code,
                e,
                fallback_allocation
            );

            // Send admin notification about fallback usage
            let subject = format!("Pricing Fallback Used - {} Plan", plan_name);
            let message = format!(
                "Dynamic Twilio pricing lookup failed for country '{}' during {} plan subscription.\n\n\
                Error: {}\n\n\
                Fallback pricing used: €{:.2}/message\n\
                Allocated credits: €{:.2} (~{} messages)\n\n\
                Consider checking:\n\
                1. Twilio pricing API availability\n\
                2. Country code validity\n\
                3. Network connectivity",
                country_code, plan_name, e, FALLBACK_REGULAR_MSG_PRICE, fallback_allocation, message_count as i32
            );

            // Spawn email notification (don't block the webhook)
            let state_clone = state.clone();
            tokio::spawn(async move {
                if let Err(e) = crate::utils::notification_utils::send_admin_alert(
                    &state_clone,
                    &subject,
                    &message,
                )
                .await
                {
                    tracing::error!("Failed to send admin alert about pricing fallback: {}", e);
                }
            });

            fallback_allocation
        }
    }
}

/// Idempotent subscription setup. Safe to call multiple times.
/// Uses billing period end as idempotency key - if already set to current period, skips.
async fn setup_user_subscription(
    state: &Arc<AppState>,
    user_id: i32,
    price_id: &str,
    current_period_end: i64,
    phone_country: Option<&str>,
) -> Result<(), String> {
    // Idempotency: skip if billing date already matches this period
    if let Ok(Some(existing)) = state.user_core.get_next_billing_date(user_id) {
        if existing == current_period_end as i32 {
            tracing::info!(
                "User {} already set up for billing period ending {}, skipping",
                user_id,
                current_period_end
            );
            return Ok(());
        }
    }

    let sub_info = extract_subscription_info(price_id);

    // Set subscription tier
    if let Err(e) = state
        .user_repository
        .set_subscription_tier(user_id, Some(sub_info.tier))
    {
        tracing::error!(
            "Failed to set subscription tier for user {}: {}",
            user_id,
            e
        );
    }

    // Set plan_type
    use crate::utils::country::{
        is_byot_plan_price, is_digest_plan_price, is_legacy_euro_plan_price, is_monitor_plan_price,
    };
    let plan_type = if is_digest_plan_price(price_id) {
        "digest"
    } else if is_byot_plan_price(price_id) {
        "byot"
    } else {
        "monitor" // includes legacy sentinel price IDs
    };
    if let Err(e) = state
        .user_repository
        .update_plan_type(user_id, Some(plan_type))
    {
        tracing::error!("Failed to set plan_type for user {}: {}", user_id, e);
    }

    // Calculate credits based on tier and country
    let credits: f32 = if sub_info.tier == "tier 2" {
        let country = match phone_country {
            Some(c) => c,
            None => {
                tracing::warn!(
                    "No phone country for user {}, using FI as fallback for credit calculation",
                    user_id
                );
                "FI"
            }
        };
        if country == "US" || country == "CA" {
            400.0
        } else if is_monitor_plan_price(price_id) || is_legacy_euro_plan_price(price_id) {
            // Legacy sentinel price IDs (€19) get same credits as Monitor plan (40 messages)
            calculate_euro_credit_allocation(state, country, 40.0, "Monitor").await
        } else if is_digest_plan_price(price_id) {
            calculate_euro_credit_allocation(state, country, 120.0, "Digest").await
        } else if is_byot_plan_price(price_id) {
            0.0 // BYOT users pay as they go
        } else {
            // Unknown price ID - log warning but give Monitor credits as fallback
            tracing::warn!(
                "Unknown price_id {} for user {}, giving Monitor credits as fallback",
                price_id,
                user_id
            );
            calculate_euro_credit_allocation(state, country, 40.0, "Monitor").await
        }
    } else {
        0.0
    };

    if let Err(e) = state.user_repository.update_sub_credits(user_id, credits) {
        tracing::error!("Failed to set credits for user {}: {}", user_id, e);
    } else {
        tracing::info!("Set {} monthly credits for user {}", credits, user_id);
    }

    // Set next billing date (this is what makes it idempotent)
    if let Err(e) = state
        .user_core
        .update_next_billing_date(user_id, current_period_end as i32)
    {
        tracing::error!(
            "Failed to update next billing date for user {}: {}",
            user_id,
            e
        );
    } else {
        tracing::info!(
            "Updated next billing date for user {}: {}",
            user_id,
            current_period_end
        );
    }

    tracing::info!(
        "Setup subscription for user {}: tier={}, plan={}, credits={}",
        user_id,
        sub_info.tier,
        plan_type,
        credits
    );

    Ok(())
}

/// Check if user is eligible for automatic upgrade refund when switching from Monitor/BYOT to Digest.
/// Returns Some((payment_intent_id, amount_cents)) if eligible, None otherwise.
///
/// Eligibility criteria:
/// - Old subscription was Monitor or BYOT
/// - Within 7 days of last payment on old subscription
/// - For Monitor users: usage < 30%
/// - For BYOT users: no usage check needed
async fn check_upgrade_refund_eligibility(
    client: &Client,
    state: &Arc<AppState>,
    user: &crate::models::user_models::User,
    old_subscription: &stripe::Subscription,
) -> Option<(String, i64)> {
    use crate::handlers::refund_handlers::get_max_credits_left;
    use crate::utils::country::{is_byot_plan_price, is_monitor_plan_price};

    // Get old price ID
    let old_price_id = old_subscription
        .items
        .data
        .first()
        .and_then(|item| item.price.as_ref())
        .map(|p| p.id.to_string())?;

    let was_byot = is_byot_plan_price(&old_price_id);
    let was_monitor = is_monitor_plan_price(&old_price_id);

    if !was_byot && !was_monitor {
        tracing::debug!(
            "Old subscription was not Monitor or BYOT, not eligible for upgrade refund"
        );
        return None;
    }

    // Get last payment timestamp for old subscription
    let invoices = stripe::Invoice::list(
        client,
        &stripe::ListInvoices {
            subscription: Some(old_subscription.id.clone()),
            status: Some(stripe::InvoiceStatus::Paid),
            limit: Some(1),
            ..Default::default()
        },
    )
    .await
    .ok()?;

    let last_invoice = invoices.data.first()?;
    let last_payment_ts = last_invoice.created?;

    let now = chrono::Utc::now().timestamp();
    let days_since_payment = (now - last_payment_ts) / 86400;

    if days_since_payment > 7 {
        tracing::debug!(
            "Outside 7-day refund window ({} days since payment)",
            days_since_payment
        );
        return None;
    }

    // For Monitor users, check credit usage
    if was_monitor {
        let max_credits =
            get_max_credits_left(state, user.phone_number_country.as_deref(), Some("monitor"))
                .await;
        let credits_used = max_credits - user.credits_left;
        let usage_percent = if max_credits > 0.0 {
            (credits_used / max_credits * 100.0).max(0.0)
        } else {
            0.0
        };

        if usage_percent >= 30.0 {
            tracing::info!(
                "User {} not eligible for upgrade refund: Monitor usage {:.1}% >= 30%",
                user.id,
                usage_percent
            );
            return None;
        }
        tracing::debug!(
            "Monitor user {} usage {:.1}% < 30%, eligible for refund",
            user.id,
            usage_percent
        );
    } else {
        tracing::debug!("BYOT user {}, no usage check needed for refund", user.id);
    }

    // Get payment intent for refund
    match &last_invoice.payment_intent {
        Some(stripe::Expandable::Id(id)) => {
            Some((id.to_string(), last_invoice.amount_paid.unwrap_or(0)))
        }
        _ => None,
    }
}

pub async fn create_unified_subscription_checkout(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(user_id): Path<i32>,
    Json(body): Json<SubscriptionCheckoutBody>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // Security: Verify user can only create checkout for themselves
    if auth_user.user_id != user_id {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Access denied"})),
        ));
    }
    tracing::debug!(
        "Starting create_subscription_checkout for user_id: {}",
        user_id
    );

    // Verify user exists in database
    let user = state
        .user_core
        .find_by_id(user_id)
        .map_err(|e| {
            tracing::error!("Database error when finding user: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to verify user"})),
            )
        })?
        .ok_or_else(|| {
            tracing::debug!("User not found: {}", user_id);
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "User not found"})),
            )
        })?;
    // Initialize Stripe client
    let stripe_secret_key = std::env::var("STRIPE_SECRET_KEY").map_err(|_| {
        tracing::error!("STRIPE_SECRET_KEY not found in environment");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Stripe configuration error"})),
        )
    })?;
    let client = Client::new(stripe_secret_key);
    tracing::debug!("Stripe client initialized");
    // Get or create Stripe customer
    let customer_id = match state.user_repository.get_stripe_customer_id(user_id) {
        Ok(Some(id)) => id,
        Ok(None) => {
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
            create_new_customer(&client, user_id, &user.email, &state).await?
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            ))
        }
    };
    // Check for existing active subscription
    let existing_subscription = stripe::Subscription::list(
        &client,
        &stripe::ListSubscriptions {
            customer: Some(customer_id.parse().unwrap()),
            status: Some(stripe::SubscriptionStatusFilter::Active),
            limit: Some(1),
            ..Default::default()
        },
    )
    .await
    .map_err(|e| {
        tracing::error!("Error fetching existing subscriptions: {:?}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to check existing subscriptions"})),
        )
    })?;
    let domain_url = std::env::var("FRONTEND_URL").expect("FRONTEND_URL not set");
    // Select price ID based on subscription type and user's phone number country
    let country = user.phone_number_country.as_deref().unwrap_or("OTHER");
    tracing::debug!("country: {}", country);

    // Import euro plan country check
    use crate::utils::country::is_euro_plan_country;

    let base_price_id = match body.subscription_type {
        SubscriptionType::Hosted => {
            if country == "US" || country == "CA" {
                std::env::var("STRIPE_SUBSCRIPTION_HOSTED_PLAN_PRICE_ID_US")
                    .expect("STRIPE_SUBSCRIPTION_HOSTED_PLAN_PRICE_ID_US not set")
            } else if is_euro_plan_country(country) {
                // Euro countries: Monitor (€29) or Digest (€59) plan
                match body.plan_type.as_deref() {
                    Some("digest") => std::env::var("STRIPE_DIGEST_PLAN_PRICE_ID")
                        .expect("STRIPE_DIGEST_PLAN_PRICE_ID not set"),
                    _ => std::env::var("STRIPE_MONITOR_PLAN_PRICE_ID")
                        .expect("STRIPE_MONITOR_PLAN_PRICE_ID not set"), // Default to Monitor
                }
            } else {
                // Fallback for any other countries
                std::env::var("STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_OTHER")
                    .expect("STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_OTHER not set")
            }
        }
    };
    // Build line items
    let line_items = vec![stripe::CreateCheckoutSessionLineItems {
        price: Some(base_price_id),
        quantity: Some(1),
        ..Default::default()
    }];

    let success_url = format!("{}/?subscription=success", domain_url);
    let cancel_url = format!("{}/?subscription=canceled", domain_url);
    let mut create_params = CreateCheckoutSession {
        success_url: Some(&success_url),
        cancel_url: Some(&cancel_url),
        mode: Some(stripe::CheckoutSessionMode::Subscription),
        line_items: Some(line_items),
        customer: Some(customer_id.parse().unwrap()),
        allow_promotion_codes: Some(true),
        billing_address_collection: Some(stripe::CheckoutSessionBillingAddressCollection::Required),
        automatic_tax: Some(stripe::CreateCheckoutSessionAutomaticTax {
            enabled: true,
            liability: None,
        }),
        tax_id_collection: Some(stripe::CreateCheckoutSessionTaxIdCollection { enabled: true }),
        customer_update: Some(stripe::CreateCheckoutSessionCustomerUpdate {
            address: Some(stripe::CreateCheckoutSessionCustomerUpdateAddress::Auto),
            name: Some(stripe::CreateCheckoutSessionCustomerUpdateName::Auto),
            shipping: Some(stripe::CreateCheckoutSessionCustomerUpdateShipping::Auto),
        }),
        custom_fields: Some(vec![stripe::CreateCheckoutSessionCustomFields {
            key: "referral_source".to_string(),
            label: stripe::CreateCheckoutSessionCustomFieldsLabel {
                custom: "Where did you hear about Lightfriend?".to_string(),
                type_: stripe::CreateCheckoutSessionCustomFieldsLabelType::Custom,
            },
            type_: stripe::CreateCheckoutSessionCustomFieldsType::Text,
            optional: Some(false),
            ..Default::default()
        }]),
        ..Default::default()
    };
    // Handle metadata for plan changes
    let success_url1 = format!("{}/?subscription=changed", domain_url);
    if let Some(current_subscription) = existing_subscription.data.first() {
        tracing::debug!("Found existing subscription: {}", current_subscription.id);

        // Create metadata to track the subscription change
        let mut metadata = std::collections::HashMap::new();
        metadata.insert(
            "replacing_subscription".to_string(),
            current_subscription.id.to_string(),
        );
        metadata.insert("plan_change".to_string(), "true".to_string());
        metadata.insert("user_id".to_string(), user_id.to_string());

        // Set metadata on both subscription_data AND session itself
        // subscription_data.metadata goes on the subscription object
        // session metadata is needed for CheckoutSessionCompleted webhook to detect plan changes
        let sub_data = stripe::CreateCheckoutSessionSubscriptionData {
            metadata: Some(metadata.clone()),
            ..Default::default()
        };
        create_params.subscription_data = Some(sub_data);
        create_params.metadata = Some(metadata);
        // Update success URL to indicate plan change
        create_params.success_url = Some(&success_url1);
    }

    let checkout_session = CheckoutSession::create(
        &client,
        create_params,
    )
    .await
    .map_err(|e| {
        tracing::error!("Stripe error details: {:?}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to create Subscription Checkout Session: {}", e)})),
        )
    })?;
    tracing::info!("Subscription checkout session created successfully");
    // Return the Checkout session URL
    Ok(Json(json!({
        "url": checkout_session.url.unwrap(),
        "message": "Redirecting to Stripe Checkout for subscription"
    })))
}

// ==================== Guest Checkout (No Auth Required) ====================

#[derive(Deserialize)]
pub struct GuestCheckoutBody {
    pub subscription_type: SubscriptionType,
    pub selected_country: String,
    /// For euro countries: "monitor" (€29/40 msgs) or "digest" (€49/120 msgs)
    pub plan_type: Option<String>,
}

/// Create a checkout session for users without an account
/// POST /api/stripe/guest-checkout
pub async fn create_guest_checkout(
    Json(body): Json<GuestCheckoutBody>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    tracing::debug!(
        "Starting guest checkout for country: {}",
        body.selected_country
    );

    // Initialize Stripe client
    let stripe_secret_key = std::env::var("STRIPE_SECRET_KEY").map_err(|_| {
        tracing::error!("STRIPE_SECRET_KEY not found in environment");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Stripe configuration error"})),
        )
    })?;
    let client = Client::new(stripe_secret_key);

    let domain_url = std::env::var("FRONTEND_URL").expect("FRONTEND_URL not set");
    let country = body.selected_country.as_str();

    // Import euro plan country check
    use crate::utils::country::is_euro_plan_country;

    // Select price ID based on subscription type and country
    let base_price_id = match body.subscription_type {
        SubscriptionType::Hosted => {
            if country == "US" || country == "CA" {
                std::env::var("STRIPE_SUBSCRIPTION_HOSTED_PLAN_PRICE_ID_US")
                    .expect("STRIPE_SUBSCRIPTION_HOSTED_PLAN_PRICE_ID_US not set")
            } else if is_euro_plan_country(country) {
                // Euro countries: use monitor or digest plan based on selection
                let plan_type = body.plan_type.as_deref().unwrap_or("monitor");
                if plan_type == "digest" {
                    std::env::var("STRIPE_DIGEST_PLAN_PRICE_ID")
                        .expect("STRIPE_DIGEST_PLAN_PRICE_ID not set")
                } else {
                    std::env::var("STRIPE_MONITOR_PLAN_PRICE_ID")
                        .expect("STRIPE_MONITOR_PLAN_PRICE_ID not set")
                }
            } else {
                // Other countries fallback
                std::env::var("STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_OTHER")
                    .expect("STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_OTHER not set")
            }
        }
    };

    // Build line items
    let line_items = vec![stripe::CreateCheckoutSessionLineItems {
        price: Some(base_price_id),
        quantity: Some(1),
        ..Default::default()
    }];

    // Success URL redirects to password setup page with session_id
    let success_url = format!("{}/subscription-success", domain_url);
    let cancel_url = format!("{}/pricing?checkout=canceled", domain_url);

    // Build metadata
    let mut metadata = std::collections::HashMap::new();
    metadata.insert("selected_country".to_string(), country.to_string());
    metadata.insert("is_guest_checkout".to_string(), "true".to_string());
    if let Some(ref plan_type) = body.plan_type {
        metadata.insert("plan_type".to_string(), plan_type.clone());
    }

    // Subscription data
    let sub_data = stripe::CreateCheckoutSessionSubscriptionData {
        metadata: Some(metadata.clone()),
        ..Default::default()
    };

    let create_params = CreateCheckoutSession {
        success_url: Some(&success_url),
        cancel_url: Some(&cancel_url),
        mode: Some(stripe::CheckoutSessionMode::Subscription),
        line_items: Some(line_items),
        // Collect phone number
        phone_number_collection: Some(stripe::CreateCheckoutSessionPhoneNumberCollection {
            enabled: true,
        }),
        allow_promotion_codes: Some(true),
        billing_address_collection: Some(stripe::CheckoutSessionBillingAddressCollection::Required),
        automatic_tax: Some(stripe::CreateCheckoutSessionAutomaticTax {
            enabled: true,
            liability: None,
        }),
        tax_id_collection: Some(stripe::CreateCheckoutSessionTaxIdCollection {
            enabled: true,
        }),
        custom_fields: Some(vec![
            stripe::CreateCheckoutSessionCustomFields {
                key: "referral_source".to_string(),
                label: stripe::CreateCheckoutSessionCustomFieldsLabel {
                    custom: "Where did you hear about Lightfriend?".to_string(),
                    type_: stripe::CreateCheckoutSessionCustomFieldsLabelType::Custom,
                },
                type_: stripe::CreateCheckoutSessionCustomFieldsType::Text,
                optional: Some(false),
                ..Default::default()
            },
        ]),
        custom_text: Some(stripe::CreateCheckoutSessionCustomText {
            after_submit: None,
            shipping_address: None,
            submit: Some(stripe::CreateCheckoutSessionCustomTextSubmit {
                message: "Your email and phone number will be your Lightfriend login credentials. The phone number is also how you'll interact with Lightfriend via SMS. Both can be changed later.".to_string(),
            }),
            terms_of_service_acceptance: None,
        }),
        metadata: Some(metadata.clone()),
        subscription_data: Some(sub_data),
        ..Default::default()
    };

    let checkout_session = CheckoutSession::create(&client, create_params)
        .await
        .map_err(|e| {
            tracing::error!("Stripe error creating guest checkout: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to create checkout session: {}", e)})),
            )
        })?;

    tracing::info!("Guest checkout session created: {}", checkout_session.id);

    Ok(Json(json!({
        "url": checkout_session.url.unwrap(),
        "session_id": checkout_session.id.to_string()
    })))
}

pub async fn create_customer_portal_session(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(user_id): Path<i32>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    tracing::debug!(
        "Starting create_customer_portal_session for user_id: {}",
        user_id
    );
    // Check if user is accessing their own data or is an admin
    if auth_user.user_id != user_id && !auth_user.is_admin {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Access denied"})),
        ));
    }
    // Initialize Stripe client
    let stripe_secret_key =
        std::env::var("STRIPE_SECRET_KEY").expect("STRIPE_SECRET_KEY must be set in environment");
    let client = Client::new(stripe_secret_key);
    tracing::debug!("Stripe client initialized");
    tracing::debug!("JWT token validated successfully");
    // Get Stripe customer ID
    let customer_id = state
        .user_repository
        .get_stripe_customer_id(user_id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "No Stripe customer ID found for user"})),
            )
        })?;
    tracing::debug!("Found Stripe customer ID: {}", customer_id);
    // Create a Billing Portal Session
    // Create a Billing Portal Session
    let mut create_session = CreateBillingPortalSession::new(customer_id.parse().unwrap());
    // Store the formatted URL in a variable first
    let return_url = format!(
        "{}/billing",
        std::env::var("FRONTEND_URL").expect("FRONTEND_URL not set")
    );
    create_session.return_url = Some(&return_url);
    tracing::debug!("Creating portal session with return URL: {}", return_url);
    let portal_session = BillingPortalSession::create(&client, create_session)
        .await
        .map_err(|e| {
            tracing::error!("{}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to create Customer Portal session: {}", e)})),
            )
        })?;
    tracing::debug!(
        "Portal session created successfully with URL: {}",
        portal_session.url
    );
    // Return the portal URL to redirect the user
    Ok(Json(json!({
        "url": portal_session.url,
        "message": "Redirecting to Stripe Customer Portal"
    })))
}

pub async fn create_checkout_session(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(user_id): Path<i32>,
    Json(payload): Json<BuyCreditsRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // Security: Verify user can only create checkout for themselves
    if auth_user.user_id != user_id {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Access denied"})),
        ));
    }
    tracing::debug!("Starting create_checkout_session for user_id: {}", user_id);
    // Fetch user from the database
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
    tracing::debug!("User found: {}", user.id);
    // Initialize Stripe client
    let stripe_secret_key =
        std::env::var("STRIPE_SECRET_KEY").expect("STRIPE_SECRET_KEY must be set in environment");
    let client = Client::new(stripe_secret_key.clone());

    // Check if user is on Digest plan (only Digest users can buy overage credits)
    // US/CA users are exempt from this check
    let is_us_ca = user.phone_number_country == Some("US".to_string())
        || user.phone_number_country == Some("CA".to_string());

    if !is_us_ca {
        // Check if user is on Digest plan (only Digest plan users can buy overage credits)
        if user.plan_type.as_deref() != Some("digest") {
            tracing::info!(
                "User {} attempted to buy credits but is not on Digest plan (plan_type={:?})",
                user_id,
                user.plan_type
            );
            return Err((
                StatusCode::FORBIDDEN,
                Json(json!({
                    "error": "Credit top-ups are only available on the Digest plan",
                    "upgrade_required": true,
                    "message": "Upgrade to Digest for €49/month to get 120 credits and the ability to purchase additional credits when needed."
                })),
            ));
        }
    }
    tracing::debug!("Stripe client initialized");
    // Check if user has a Stripe customer ID; if not, create one
    // Check if user has a Stripe customer ID; if not, create one
    let customer_id = match state.user_repository.get_stripe_customer_id(user_id) {
        Ok(Some(id)) => {
            tracing::debug!("Found existing Stripe customer ID");
            // Try to retrieve the customer to verify it exists
            match Customer::retrieve(&client, &id.parse().unwrap(), &[]).await {
                Ok(_customer) => {
                    // Customer exists
                    id // Return as String
                }
                Err(e) => match e {
                    stripe::StripeError::Stripe(stripe_error) => {
                        if stripe_error.error_type == stripe::ErrorType::Api {
                            // Handle the case where the customer doesn't exist
                            tracing::warn!("Customer {} does not exist, creating new customer", id);
                            create_new_customer(&client, user_id, &user.email, &state).await?
                        } else {
                            // Handle other types of API errors
                            let error = stripe_error.message.unwrap();
                            tracing::error!("API error: {}", error);
                            return Err((
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(json!({"error": format!("Stripe API error: {}", error)})),
                            ));
                        }
                    }
                    _ => {
                        // Handle other types of errors
                        tracing::error!("An error occurred: {:?}", e);
                        return Err((
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({"error": format!("Stripe error: {:?}", e)})),
                        ));
                    }
                },
            }
        }
        Ok(None) => {
            tracing::debug!("No Stripe customer ID found, creating new customer");
            create_new_customer(&client, user_id, &user.email, &state).await?
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            ))
        }
    };

    let amount_dollars = payload.amount_dollars; // From BuyCreditsRequest
    let amount_cents = (amount_dollars * 100.0).round() as i64; // Convert to cents for Stripe
    tracing::info!(
        "Processing payment of {} EUR ({} cents)",
        amount_dollars,
        amount_cents
    );
    let domain_url = std::env::var("FRONTEND_URL").expect("FRONTEND_URL not set");

    // Create a Checkout Session with payment method attachment
    tracing::debug!("Creating Stripe checkout session");
    let checkout_session = CheckoutSession::create(
        &client,
        CreateCheckoutSession {
            success_url: Some(&format!("{}/?credits=success", domain_url)), // Redirect after success
            cancel_url: Some(&format!("{}/?credits=canceled", domain_url)), // Redirect after cancellation
            payment_method_types: Some(vec![stripe::CreateCheckoutSessionPaymentMethodTypes::Card]), // Allow card payments
            mode: Some(stripe::CheckoutSessionMode::Payment), // One-time payment mode
            line_items: Some(vec![stripe::CreateCheckoutSessionLineItems {
                price_data: Some(stripe::CreateCheckoutSessionLineItemsPriceData {
                    currency: stripe::Currency::EUR,
                    product: Some(
                        std::env::var("STRIPE_CREDITS_PRODUCT_ID")
                            .expect("STRIPE_CREDITS_PRODUCT_ID not set"),
                    ),
                    unit_amount: Some(amount_cents), // Amount in cents
                    ..Default::default()
                }),
                quantity: Some(1),
                ..Default::default()
            }]),
            customer: Some(customer_id.parse().unwrap()),
            customer_update: Some(stripe::CreateCheckoutSessionCustomerUpdate {
                address: Some(stripe::CreateCheckoutSessionCustomerUpdateAddress::Auto),
                ..Default::default()
            }),
            payment_intent_data: Some(stripe::CreateCheckoutSessionPaymentIntentData {
                setup_future_usage: Some(
                    stripe::CreateCheckoutSessionPaymentIntentDataSetupFutureUsage::OffSession,
                ),
                ..Default::default()
            }),
            automatic_tax: Some(stripe::CreateCheckoutSessionAutomaticTax {
                enabled: true,   // Enable Stripe Tax to calculate taxes automatically
                liability: None, // default behavior
            }),
            billing_address_collection: Some(
                stripe::CheckoutSessionBillingAddressCollection::Required,
            ),
            allow_promotion_codes: Some(true), // Allow discount codes
            ..Default::default()
        },
    )
    .await
    .map_err(|e| {
        tracing::error!("Stripe error details: {:?}", e); // Log the full error
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to create Checkout Session: {}", e)})),
        )
    })?;
    tracing::info!("Checkout session created successfully");
    // Save the session ID for later use (optional, if you need to track it)
    state
        .user_repository
        .set_stripe_checkout_session_id(user_id, checkout_session.id.as_ref())
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?;
    tracing::debug!("Checkout session ID saved to database");
    // Return the Checkout session URL to redirect the user
    Ok(Json(json!({
        "url": checkout_session.url.unwrap(), // Safe to unwrap as it's always present for Checkout
        "message": "Redirecting to Stripe Checkout for payment"
    })))
}
// Helper function to create a new Stripe customer
async fn create_new_customer(
    client: &Client,
    user_id: i32,
    email: &str,
    state: &Arc<AppState>,
) -> Result<String, (StatusCode, Json<Value>)> {
    let customer = Customer::create(
        client,
        CreateCustomer {
            email: Some(email),
            name: Some(&format!("User {}", user_id)),
            address: None, // Explicitly set no address to avoid pre-filling
            ..Default::default()
        },
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to create customer: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to create Stripe customer: {}", e)})),
        )
    })?;
    tracing::info!("Created new Stripe customer");
    state
        .user_repository
        .set_stripe_customer_id(user_id, customer.id.as_ref())
        .map_err(|e| {
            tracing::error!("Failed to update database with new customer ID: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?;
    Ok(customer.id.to_string())
}
#[derive(Debug, Clone)]
struct SubscriptionInfo {
    country: Option<&'static str>,
    tier: &'static str,
}

// Helper function to extract subscription info from price ID
fn extract_subscription_info(price_id: &str) -> SubscriptionInfo {
    // Default values - all subscriptions map to tier 2 (hosted)
    let mut info = SubscriptionInfo {
        country: None,
        tier: "tier 2",
    };

    // Helper macro to reduce code duplication
    macro_rules! check_price_id {
        ($country:expr, $env_var:expr, $tier:expr) => {
            if price_id == std::env::var($env_var).unwrap_or_default() {
                info.country = Some($country);
                info.tier = $tier;
                return info;
            }
        };
    }

    // Check for new Monitor and Digest plans (tier 2)
    if price_id == std::env::var("STRIPE_MONITOR_PLAN_PRICE_ID").unwrap_or_default() {
        info.tier = "tier 2";
        return info;
    }
    if price_id == std::env::var("STRIPE_DIGEST_PLAN_PRICE_ID").unwrap_or_default() {
        info.tier = "tier 2";
        return info;
    }

    // All legacy and current hosted plans map to tier 2
    // Check all regional variants for both legacy and current price IDs
    for country in ["US", "FI", "NL", "UK", "AU", "OTHER", "CA"] {
        // Legacy price IDs (all map to tier 2 now)
        check_price_id!(
            country,
            format!("STRIPE_SUBSCRIPTION_HARD_MODE_PRICE_ID_{}", country),
            "tier 2"
        );
        check_price_id!(
            country,
            format!("STRIPE_SUBSCRIPTION_BASIC_DAILY_PRICE_ID_{}", country),
            "tier 2"
        );
        check_price_id!(
            country,
            format!("STRIPE_SUBSCRIPTION_BASIC_PRICE_ID_{}", country),
            "tier 2"
        );
        check_price_id!(
            country,
            format!("STRIPE_SUBSCRIPTION_WORLD_PRICE_ID_{}", country),
            "tier 2"
        );
        check_price_id!(
            country,
            format!("STRIPE_SUBSCRIPTION_ESCAPE_DAILY_PRICE_ID_{}", country),
            "tier 2"
        );
        check_price_id!(
            country,
            format!("STRIPE_SUBSCRIPTION_MONITORING_PRICE_ID_{}", country),
            "tier 2"
        );
        check_price_id!(
            country,
            format!("STRIPE_SUBSCRIPTION_ORACLE_PRICE_ID_{}", country),
            "tier 2"
        );

        // Current price IDs
        check_price_id!(
            country,
            format!("STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_{}", country),
            "tier 2"
        );
        check_price_id!(
            country,
            format!("STRIPE_SUBSCRIPTION_HOSTED_PLAN_PRICE_ID_{}", country),
            "tier 2"
        );
    }

    info
}

pub async fn stripe_webhook(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<impl IntoResponse, (StatusCode, Json<Value>)> {
    let payload_str = String::from_utf8(body.to_vec()).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid payload encoding"})),
        )
    })?;
    tracing::info!("Stripe webhook received");
    // Initialize Stripe client
    let stripe_secret_key =
        std::env::var("STRIPE_SECRET_KEY").expect("STRIPE_SECRET_KEY must be set in environment");
    let client = Client::new(stripe_secret_key);
    // Get the webhook secret from environment
    let webhook_secret = std::env::var("STRIPE_WEBHOOK_SECRET")
        .expect("STRIPE_WEBHOOK_SECRET must be set in environment");
    // Extract the stripe-signature header
    let sig_header = headers
        .get("stripe-signature")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Missing Stripe-Signature header"})),
            )
        })?;
    tracing::info!("Stripe signature header found");
    // Construct and verify the Stripe event using the signature
    let event = stripe::Webhook::construct_event(&payload_str, sig_header, &webhook_secret)
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": format!("Invalid Stripe webhook signature: {}", e)})),
            )
        })?;

    tracing::info!("Stripe event verified successfully: {}", event.type_);
    // Process the event based on its type
    match event.type_ {
        stripe::EventType::CustomerSubscriptionCreated
        | stripe::EventType::CustomerSubscriptionUpdated => {
            tracing::info!("Processing subscription created/updated event");
            if let stripe::EventObject::Subscription(subscription) = event.data.object {
                let customer_id = match subscription.customer {
                    stripe::Expandable::Id(id) => id,
                    stripe::Expandable::Object(customer) => customer.id,
                };

                // Get price ID from subscription
                let price_id = subscription
                    .items
                    .data
                    .first()
                    .and_then(|item| item.price.as_ref())
                    .map(|price| price.id.to_string())
                    .ok_or_else(|| {
                        tracing::error!("No price found in subscription items");
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({"error": "Invalid subscription data"})),
                        )
                    })?;

                let sub_info = extract_subscription_info(&price_id);

                // Skip subscription updates that are part of a plan change or being cancelled
                if event.type_ == stripe::EventType::CustomerSubscriptionUpdated {
                    let is_plan_change = subscription
                        .metadata
                        .get("plan_change")
                        .map(|val| val == "true")
                        .unwrap_or(false);
                    if is_plan_change {
                        tracing::info!(
                            "Skipping subscription update as it's part of a plan change"
                        );
                        return Ok(StatusCode::OK);
                    }
                    if subscription.cancel_at_period_end {
                        tracing::info!(
                            "Skipping subscription update for subscription being cancelled: {}",
                            subscription.id
                        );
                        return Ok(StatusCode::OK);
                    }
                }

                // Cancel existing subscriptions when a new one is created
                if event.type_ == stripe::EventType::CustomerSubscriptionCreated {
                    use crate::utils::country::is_digest_plan_price;

                    let existing_subscriptions = stripe::Subscription::list(
                        &client,
                        &stripe::ListSubscriptions {
                            customer: Some(customer_id.clone()),
                            status: Some(stripe::SubscriptionStatusFilter::Active),
                            ..Default::default()
                        },
                    )
                    .await
                    .map_err(|e| {
                        tracing::error!("Failed to list existing subscriptions: {}", e);
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({"error": "Failed to check existing subscriptions"})),
                        )
                    })?;

                    for existing_sub in existing_subscriptions.data.iter() {
                        if existing_sub.id != subscription.id {
                            // Check if upgrading to Digest and eligible for automatic refund
                            if is_digest_plan_price(&price_id) {
                                // Get user for credit usage check
                                if let Ok(Some(user)) = state
                                    .user_repository
                                    .find_by_stripe_customer_id(customer_id.as_str())
                                {
                                    if let Some((payment_intent_id, amount)) =
                                        check_upgrade_refund_eligibility(
                                            &client,
                                            &state,
                                            &user,
                                            existing_sub,
                                        )
                                        .await
                                    {
                                        // Process automatic refund
                                        match stripe::Refund::create(
                                            &client,
                                            stripe::CreateRefund {
                                                payment_intent: Some(
                                                    payment_intent_id.parse().unwrap(),
                                                ),
                                                reason: Some(
                                                    stripe::RefundReasonFilter::RequestedByCustomer,
                                                ),
                                                ..Default::default()
                                            },
                                        )
                                        .await
                                        {
                                            Ok(refund) => {
                                                tracing::info!(
                                                    "Automatic upgrade refund processed for user {}: {} cents, refund_id={}",
                                                    user.id, amount, refund.id
                                                );
                                            }
                                            Err(e) => {
                                                tracing::error!(
                                                    "Failed to process automatic upgrade refund for user {}: {}",
                                                    user.id, e
                                                );
                                            }
                                        }
                                    }
                                }
                            }

                            tracing::info!("Canceling existing subscription: {}", existing_sub.id);
                            // Add plan_change metadata so the deletion webhook knows to skip processing
                            let mut metadata = std::collections::HashMap::new();
                            metadata.insert("plan_change".to_string(), "true".to_string());
                            let _ = Subscription::update(
                                &client,
                                &existing_sub.id,
                                UpdateSubscription {
                                    cancel_at_period_end: Some(true),
                                    metadata: Some(metadata),
                                    ..Default::default()
                                },
                            )
                            .await;
                        }
                    }
                }

                // Find or create user (only on Created event, not Updated)
                let user_id: i32 = match state
                    .user_repository
                    .find_by_stripe_customer_id(customer_id.as_str())
                {
                    Ok(Some(user)) => user.id,
                    Ok(None) if event.type_ == stripe::EventType::CustomerSubscriptionCreated => {
                        // Guest checkout - create user from Stripe customer
                        tracing::info!(
                            "No user found for customer {}, creating from Stripe customer data",
                            customer_id
                        );

                        let customer = Customer::retrieve(&client, &customer_id, &[])
                            .await
                            .map_err(|e| {
                                tracing::error!("Failed to retrieve Stripe customer: {}", e);
                                (
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    Json(json!({"error": "Failed to retrieve customer"})),
                                )
                            })?;

                        let email = customer.email.clone().unwrap_or_default();
                        let phone = customer.phone.clone().unwrap_or_default();

                        // Use SignupService to handle user creation/linking
                        use crate::repositories::signup_repository_impl::CompositeSignupRepository;
                        use crate::services::signup_service::{
                            SignupError, SignupResult, SignupService,
                        };

                        let signup_repo = std::sync::Arc::new(CompositeSignupRepository::new(
                            state.user_core.clone(),
                            state.user_repository.clone(),
                        ));
                        let signup_service = SignupService::new(signup_repo);

                        match signup_service.handle_new_subscription(
                            &email,
                            &phone,
                            customer_id.as_ref(),
                        ) {
                            Ok(SignupResult::ExistingUserLinked {
                                user_id,
                                send_welcome_email,
                                ..
                            }) => {
                                tracing::info!(
                                    "Linked existing user {} to Stripe customer {}",
                                    user_id,
                                    customer_id
                                );

                                if send_welcome_email {
                                    let email_clone = email.clone();
                                    tokio::spawn(async move {
                                        if let Err(e) =
                                            crate::utils::email::send_subscription_activated_email(
                                                &email_clone,
                                            )
                                            .await
                                        {
                                            tracing::error!(
                                                "Failed to send subscription activated email: {}",
                                                e
                                            );
                                        }
                                    });
                                }

                                user_id
                            }
                            Ok(SignupResult::NewUserCreated {
                                user_id,
                                magic_token,
                                email,
                                phone_skipped_duplicate,
                            }) => {
                                tracing::info!(
                                    "Created new user {} from guest checkout (phone_skipped: {})",
                                    user_id,
                                    phone_skipped_duplicate
                                );

                                let frontend_url =
                                    std::env::var("FRONTEND_URL").unwrap_or_default();
                                let magic_link =
                                    format!("{}/set-password/{}", frontend_url, magic_token);

                                tokio::spawn(async move {
                                    if let Err(e) =
                                        crate::utils::email::send_magic_link_email_with_options(
                                            &email,
                                            &magic_link,
                                            phone_skipped_duplicate,
                                        )
                                        .await
                                    {
                                        tracing::error!("Failed to send magic link email: {}", e);
                                    }
                                });

                                user_id
                            }
                            Err(SignupError::EmptyEmail) => {
                                tracing::error!(
                                    "No email found for Stripe customer {}",
                                    customer_id
                                );
                                return Err((
                                    StatusCode::BAD_REQUEST,
                                    Json(json!({"error": "Customer has no email"})),
                                ));
                            }
                            Err(e) => {
                                tracing::error!("Signup error: {}", e);
                                return Err((
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    Json(json!({"error": "Failed to create user"})),
                                ));
                            }
                        }
                    }
                    Ok(None) => {
                        // Subscription update for non-existent user - shouldn't normally happen
                        tracing::warn!(
                            "Subscription update received for unknown customer {}, skipping",
                            customer_id
                        );
                        return Ok(StatusCode::OK);
                    }
                    Err(e) => {
                        tracing::error!("Error finding user by customer ID: {}", e);
                        return Err((
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({"error": "Database error"})),
                        ));
                    }
                };

                // Get fresh user data for subscription setup
                let user = state
                    .user_core
                    .find_by_id(user_id)
                    .map_err(|e| {
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({"error": format!("DB error: {}", e)})),
                        )
                    })?
                    .ok_or_else(|| {
                        (
                            StatusCode::NOT_FOUND,
                            Json(json!({"error": "User not found"})),
                        )
                    })?;

                // Update subscription country
                if let Err(e) = state
                    .user_core
                    .update_sub_country(user.id, sub_info.country)
                {
                    tracing::error!("Failed to update subscription country: {}", e);
                }

                // Use centralized subscription setup (idempotent)
                let phone_country = user.phone_number_country.as_deref();
                if let Err(e) = setup_user_subscription(
                    &state,
                    user.id,
                    &price_id,
                    subscription.current_period_end,
                    phone_country,
                )
                .await
                {
                    tracing::error!("Failed to setup subscription: {}", e);
                }

                // Clear BYOT credentials if switching to non-BYOT plan
                if !crate::utils::country::is_byot_plan_price(&price_id)
                    && state.user_core.has_twilio_credentials(user.id)
                {
                    tracing::info!(
                        "User {} switching to non-BYOT plan, clearing BYOT credentials",
                        user.id
                    );
                    let _ = state.user_core.clear_twilio_credentials(user.id);
                }

                // Set preferred Lightfriend number (for non-BYOT plans)
                if !crate::utils::country::is_byot_plan_price(&price_id)
                    && user.preferred_number.is_none()
                {
                    if let Some(ref country) = user.phone_number_country {
                        let _ = state
                            .user_core
                            .set_preferred_number_for_country(user.id, country);
                    }
                }
            }
        }
        stripe::EventType::CustomerSubscriptionDeleted => {
            tracing::info!("Processing customer.subscription.deleted event");
            if let stripe::EventObject::Subscription(subscription) = event.data.object {
                let customer_id = match subscription.customer {
                    stripe::Expandable::Id(id) => id,
                    stripe::Expandable::Object(customer) => customer.id,
                };

                // Check if this deletion is part of a subscription change/upgrade
                let is_subscription_change = subscription
                    .metadata
                    .get("plan_change")
                    .map(|val| val == "true")
                    .unwrap_or(false);
                if is_subscription_change {
                    tracing::info!(
                        "Subscription deletion is part of a plan change, skipping tier update"
                    );
                    return Ok(StatusCode::OK);
                }

                if let Ok(Some(user)) = state
                    .user_repository
                    .find_by_stripe_customer_id(customer_id.as_str())
                {
                    // Check for other active subscriptions
                    let active_subscriptions = stripe::Subscription::list(
                        &client,
                        &stripe::ListSubscriptions {
                            customer: Some(customer_id.clone()),
                            status: Some(stripe::SubscriptionStatusFilter::Active),
                            ..Default::default()
                        },
                    )
                    .await
                    .map_err(|e| {
                        tracing::error!("Failed to list subscriptions: {}", e);
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({"error": "Failed to check existing subscriptions"})),
                        )
                    })?;
                    // Simplified deletion logic - just check if any active subscriptions remain
                    if active_subscriptions.data.is_empty() {
                        // No active subscriptions left, clear subscription tier, country, plan_type, credits, and billing date
                        tracing::info!(
                            "No active subscriptions remaining, clearing subscription info"
                        );
                        if let Err(e) = state.user_repository.set_subscription_tier(user.id, None) {
                            tracing::error!("Failed to clear subscription tier: {}", e);
                        }
                        if let Err(e) = state.user_core.update_sub_country(user.id, None) {
                            tracing::error!("Failed to clear subscription country: {}", e);
                        }
                        // Clear plan_type
                        if let Err(e) = state.user_repository.update_plan_type(user.id, None) {
                            tracing::error!("Failed to clear plan_type: {}", e);
                        }
                        // Clear monthly credits
                        if let Err(e) = state.user_repository.update_sub_credits(user.id, 0.0) {
                            tracing::error!("Failed to clear subscription credits: {}", e);
                        }
                        // Clear next billing date
                        if let Err(e) = state.user_core.update_next_billing_date(user.id, 0) {
                            tracing::error!("Failed to clear next billing date: {}", e);
                        }
                    } else {
                        // User still has active subscriptions - update to the first one found
                        if let Some(remaining_sub) = active_subscriptions.data.first() {
                            if let Some(tier_info) = remaining_sub
                                .items
                                .data
                                .first()
                                .and_then(|item| item.price.as_ref())
                                .map(|price| extract_subscription_info(&price.id))
                            {
                                tracing::info!("Updating subscription tier to {} based on remaining subscription", tier_info.tier);
                                if let Err(e) = state
                                    .user_repository
                                    .set_subscription_tier(user.id, Some(tier_info.tier))
                                {
                                    tracing::error!("Failed to update subscription tier: {}", e);
                                }
                                if let Err(e) = state
                                    .user_core
                                    .update_sub_country(user.id, tier_info.country)
                                {
                                    tracing::error!("Failed to update subscription country: {}", e);
                                }
                            }
                        }
                    }
                }
            }
        }
        stripe::EventType::CheckoutSessionCompleted => {
            tracing::info!("Processing checkout.session.completed event");
            match event.data.object {
                stripe::EventObject::CheckoutSession(session) => {
                    tracing::info!("Checkout session found: {}", session.id);

                    // Subscription handling is now done in CustomerSubscriptionCreated webhook
                    // This handler only processes credit pack purchases (payment mode)
                    if matches!(session.mode, stripe::CheckoutSessionMode::Subscription) {
                        tracing::info!("Subscription checkout completed - setup handled by CustomerSubscriptionCreated webhook");
                        return Ok(StatusCode::OK);
                    }

                    // Payment mode (credit pack purchases) - not subscription mode
                    if let Some(customer) = &session.customer {
                        let customer_id = match customer {
                            stripe::Expandable::Id(id) => id.clone(),
                            stripe::Expandable::Object(customer) => customer.id.clone(),
                        };
                        tracing::info!("Customer ID: {}", customer_id);
                        // Update customer address with billing address from Checkout
                        if let Some(billing_details) = &session.shipping_details {
                            if let Some(address) = &billing_details.address {
                                tracing::info!("Updating customer address with billing details");
                                Customer::update(
                                    &client,
                                    &customer_id,
                                    stripe::UpdateCustomer {
                                        address: Some(stripe::Address {
                                            line1: address.line1.clone(),
                                            city: address.city.clone(),
                                            country: address.country.clone(),
                                            postal_code: address.postal_code.clone(),
                                            state: address.state.clone(),
                                            ..Default::default()
                                        }),
                                        ..Default::default()
                                    },
                                )
                                .await
                                .map_err(|e| {
                                    tracing::error!("Failed to update customer address: {}", e);
                                    // Continue processing even if address update fails (non-critical)
                                })
                                .ok();
                            }
                        }
                        let payment_intent = session.payment_intent.as_ref().ok_or_else(|| {
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(json!({"error": "No payment intent in session"})),
                            )
                        })?;
                        // Retrieve the payment method from the payment intent
                        let payment_intent_id = match payment_intent {
                            stripe::Expandable::Id(id) => id.clone(),
                            stripe::Expandable::Object(pi) => pi.id.clone(),
                        };

                        tracing::info!("Payment intent ID found");
                        let payment_intent = PaymentIntent::retrieve(&client, &payment_intent_id, &[])
                        .await
                        .map_err(|e| (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({"error": format!("Failed to retrieve PaymentIntent: {}", e)})),
                        ))?;
                        if let Some(payment_method) = payment_intent.payment_method {
                            // Extract the payment method ID from the Expandable enum
                            let payment_method_id = match payment_method {
                                stripe::Expandable::Id(id) => id,
                                stripe::Expandable::Object(pm) => pm.id.clone(),
                            };

                            // Save the payment method ID to your database for the customer
                            let user = state
                                .user_repository
                                .find_by_stripe_customer_id(&customer_id)
                                .map_err(|e| {
                                    (
                                        StatusCode::INTERNAL_SERVER_ERROR,
                                        Json(json!({"error": format!("Database error: {}", e)})),
                                    )
                                })?
                                .ok_or_else(|| {
                                    (
                                        StatusCode::NOT_FOUND,
                                        Json(json!({"error": "Customer not found"})),
                                    )
                                })?;
                            tracing::info!("Found user with ID: {}", user.id);
                            state
                                .user_repository
                                .set_stripe_payment_method_id(user.id, &payment_method_id)
                                .map_err(|e| {
                                    (
                                        StatusCode::INTERNAL_SERVER_ERROR,
                                        Json(json!({"error": format!("Database error: {}", e)})),
                                    )
                                })?;
                            tracing::info!("Successfully saved payment method ID for user");
                            let amount_in_cents = session.amount_subtotal.unwrap_or(0);
                            let amount = amount_in_cents as f32 / 100.00;
                            state
                                .user_repository
                                .increase_credits(user.id, amount)
                                .map_err(|e| {
                                    (
                                        StatusCode::INTERNAL_SERVER_ERROR,
                                        Json(json!({"error": format!("Database error: {}", e)})),
                                    )
                                })?;
                            tracing::info!(
                                "Increased the credits amount by {} successfully",
                                amount
                            );

                            // Track credit pack purchase for refund eligibility
                            let now = chrono::Utc::now().timestamp() as i32;
                            if let Err(e) = state
                                .user_repository
                                .update_last_credit_pack_purchase(user.id, amount, now)
                            {
                                tracing::warn!(
                                    "Failed to track credit pack purchase for refund: {}",
                                    e
                                );
                            }
                        }
                    }
                }
                _ => {
                    tracing::error!("Checkout session not found in event object");
                }
            }
        }
        _ => {
            tracing::info!("Ignoring non-checkout.session.completed event");
        }
    }
    tracing::info!("Webhook processed successfully");
    Ok(StatusCode::OK) // Return 200 OK for successful webhook processing
}

pub async fn automatic_charge(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<i32>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    tracing::debug!("Starting automatic_charge for user_id: {}", user_id);
    // Initialize Stripe client
    let stripe_secret_key =
        std::env::var("STRIPE_SECRET_KEY").expect("STRIPE_SECRET_KEY must be set in environment");
    let client = Client::new(stripe_secret_key);
    tracing::debug!("Stripe client initialized");
    // Fetch user from the database
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
    tracing::debug!("User found: {}", user.id);
    // Get Stripe customer ID and payment method ID
    let customer_id = state
        .user_repository
        .get_stripe_customer_id(user_id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "No Stripe customer ID found for user"})),
            )
        })?;
    println!("Stripe customer ID found");
    let payment_method_id = state
        .user_repository
        .get_stripe_payment_method_id(user_id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "No Stripe payment method found for user"})),
            )
        })?;
    tracing::debug!("Stripe payment method ID found");
    let charge_back_to = user.charge_back_to.unwrap_or(5.00);
    tracing::debug!(
        "User charge_back_to: {}, current credits: {}",
        charge_back_to,
        user.credits
    );

    let charge_amount_cents = (charge_back_to * 100.0).round() as i64; // Convert to cents for Stripe
    tracing::info!("Charging credits (€{})", charge_back_to);
    // Create a PaymentIntent for the off-session charge
    tracing::debug!("Creating payment intent");
    let mut create_intent = CreatePaymentIntent::new(charge_amount_cents, stripe::Currency::EUR);
    create_intent.customer = Some(customer_id.parse().unwrap());
    create_intent.payment_method = Some(payment_method_id.parse().unwrap());
    create_intent.confirm = Some(true); // Confirm the payment immediately
    create_intent.off_session = Some(stripe::PaymentIntentOffSession::Exists(true)); // Off-session payment
    create_intent.payment_method_types = Some(vec!["card".to_string()]); // Card payment method
    let payment_intent = PaymentIntent::create(&client, create_intent)
        .await
        .map_err(|e| {
            tracing::error!("Failed to create PaymentIntent: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to create PaymentIntent: {}", e)})),
            )
        })?;

    tracing::debug!(
        "Payment intent created with status: {:?}",
        payment_intent.status
    );

    // Check if the payment was successful
    if payment_intent.status == stripe::PaymentIntentStatus::Succeeded {
        tracing::info!("Payment succeeded, updating user credits");
        // Update user's credits
        state
            .user_repository
            .increase_credits(user_id, charge_back_to)
            .map_err(|e| {
                tracing::error!("Failed to update user credits: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Database error updating credits: {}", e)})),
                )
            })?;
        tracing::info!("User credits updated successfully, returning success response");
        Ok(Json(json!({
            "message": "Automatic charge successful, credits updated",
            "amount": charge_back_to,
        })))
    } else {
        tracing::warn!(
            "Payment intent failed or requires action, status: {:?}",
            payment_intent.status
        );
        Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Payment intent failed or requires action"})),
        ))
    }
}
