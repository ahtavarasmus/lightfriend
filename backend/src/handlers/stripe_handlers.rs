use stripe::{
    Client,
    Customer,
    CheckoutSession,
    CreateCheckoutSession,
    CreateCustomer,
    PaymentIntent,
    CreatePaymentIntent,
    BillingPortalSession,
    CreateBillingPortalSession,
    Subscription,
    UpdateSubscription,
};
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub enum SubscriptionType {
    Hosted,
    SelfHosting,
}
use serde::{Deserialize, Serialize};
use axum::{
    extract::{State, Path},
    http::{StatusCode, HeaderMap},
    response::IntoResponse,
    body::Bytes,
    Json,
};
use crate::handlers::auth_middleware::AuthUser;
use crate::AppState;
use serde_json::{json, Value};
use std::sync::Arc;
use crate::utils::self_host_twilio;
// Assuming BuyCreditsRequest is defined in billing_models.rs
#[derive(Deserialize, Serialize, Clone, PartialEq)]
pub struct BuyCreditsRequest {
    pub amount_dollars: f32,
}
#[derive(Deserialize)]
pub struct SubscriptionCheckoutBody {
    pub subscription_type: SubscriptionType,
    /// For euro countries: "monitor" (€29/50 msgs) or "digest" (€59/150 msgs)
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
/// * `message_count` - Number of messages the plan includes (50 for Monitor, 150 for Digest)
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
                plan_name, country_code, message_count, pricing.regular_message_price, allocation
            );
            allocation
        }
        Err(e) => {
            let fallback_allocation = message_count * FALLBACK_REGULAR_MSG_PRICE;
            tracing::warn!(
                "Failed to get pricing for {}, using fallback: {} - allocating €{:.2}",
                country_code, e, fallback_allocation
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
                ).await {
                    tracing::error!("Failed to send admin alert about pricing fallback: {}", e);
                }
            });

            fallback_allocation
        }
    }
}

pub async fn create_unified_subscription_checkout(
    State(state): State<Arc<AppState>>,
    _auth_user: AuthUser,
    Path(user_id): Path<i32>,
    Json(body): Json<SubscriptionCheckoutBody>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    tracing::debug!("Starting create_subscription_checkout for user_id: {}", user_id);

    // Check if new subscriptions are blocked
    if let Ok(blocked) = std::env::var("BLOCK_NEW_SUBSCRIPTIONS") {
        if blocked == "true" || blocked == "1" {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "error": "New subscriptions are temporarily unavailable",
                    "message": "We're currently not accepting new subscriptions as we prepare for improvements to our service. You'll be notified via email when subscriptions are available again."
                })),
            ));
        }
    }

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
                .map_err(|e| (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Database error: {}", e)})),
                ))?
                .ok_or_else(|| (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": "User not found"})),
                ))?;
            create_new_customer(&client, user_id, &user.email, &state).await?
        },
        Err(e) => return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)})),
        )),
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
    ).await.map_err(|e| {
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
        },
        SubscriptionType::SelfHosting => {
            std::env::var("STRIPE_SUBSCRIPTION_SELF_HOSTING_PRICE_ID")
                .expect("STRIPE_SUBSCRIPTION_SELF_HOSTING_PRICE_ID not set")
        },
    };
    let line_items = vec![
        stripe::CreateCheckoutSessionLineItems {
            price: Some(base_price_id),
            quantity: Some(1),
            ..Default::default()
        }
    ];
    let success_url = format!("{}/billing?subscription=success", domain_url);
    let cancel_url = format!("{}/billing?subscription=canceled", domain_url);
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
        tax_id_collection: Some(stripe::CreateCheckoutSessionTaxIdCollection {
            enabled: true,
        }),
        customer_update: Some(stripe::CreateCheckoutSessionCustomerUpdate {
            address: Some(stripe::CreateCheckoutSessionCustomerUpdateAddress::Auto),
            name: Some(stripe::CreateCheckoutSessionCustomerUpdateName::Auto),
            shipping: Some(stripe::CreateCheckoutSessionCustomerUpdateShipping::Auto),
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
        ..Default::default()
    };
    // Handle metadata if needed (removed cold_turkey)
    let mut metadata = std::collections::HashMap::new();
    let mut sub_data = stripe::CreateCheckoutSessionSubscriptionData {
        ..Default::default()
    };
    let success_url1 = format!("{}/billing?subscription=changed", domain_url);
    if let Some(current_subscription) = existing_subscription.data.first() {
        tracing::debug!("Found existing subscription: {}", current_subscription.id);
  
        // Create metadata to track the subscription change
        metadata.insert("replacing_subscription".to_string(), current_subscription.id.to_string());
        metadata.insert("plan_change".to_string(), "true".to_string());
        metadata.insert("user_id".to_string(), user_id.to_string());
  
        sub_data.metadata = Some(metadata.clone());
        // Update success URL to indicate plan change
        create_params.success_url = Some(&success_url1);
        // No trial for upgrades
    } else if body.subscription_type == SubscriptionType::Hosted && (country == "US" || country == "CA") {
        // Add free 7-day trial for new Hosted subscriptions in US/CA
        sub_data.trial_period_days = Some(7);
        if !metadata.is_empty() {
            sub_data.metadata = Some(metadata);
        }
    } else if body.subscription_type == SubscriptionType::Hosted && is_euro_plan_country(country) {
        // Add 7-day trial for euro countries (€10 setup fee handled via Stripe product config)
        sub_data.trial_period_days = Some(7);
        if !metadata.is_empty() {
            sub_data.metadata = Some(metadata);
        }
    }
    create_params.subscription_data = Some(sub_data);

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
    /// For euro countries: "monitor" (€29/50 msgs) or "digest" (€59/150 msgs)
    pub plan_type: Option<String>,
}

/// Create a checkout session for users without an account
/// POST /api/stripe/guest-checkout
pub async fn create_guest_checkout(
    State(state): State<Arc<AppState>>,
    Json(body): Json<GuestCheckoutBody>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    tracing::debug!("Starting guest checkout for country: {}", body.selected_country);

    // Check if new subscriptions are blocked
    if let Ok(blocked) = std::env::var("BLOCK_NEW_SUBSCRIPTIONS") {
        if blocked == "true" || blocked == "1" {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "error": "New subscriptions are temporarily unavailable",
                    "message": "We're currently not accepting new subscriptions. Please check back later."
                })),
            ));
        }
    }

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
        },
        SubscriptionType::SelfHosting => {
            std::env::var("STRIPE_SUBSCRIPTION_SELF_HOSTING_PRICE_ID")
                .expect("STRIPE_SUBSCRIPTION_SELF_HOSTING_PRICE_ID not set")
        },
    };

    let line_items = vec![
        stripe::CreateCheckoutSessionLineItems {
            price: Some(base_price_id),
            quantity: Some(1),
            ..Default::default()
        },
    ];

    // Success URL redirects to password setup page with session_id
    let success_url = format!("{}/set-password?session_id={{CHECKOUT_SESSION_ID}}", domain_url);
    let cancel_url = format!("{}/pricing?checkout=canceled", domain_url);

    // Build metadata
    let mut metadata = std::collections::HashMap::new();
    metadata.insert("selected_country".to_string(), country.to_string());
    metadata.insert("is_guest_checkout".to_string(), "true".to_string());
    if let Some(ref plan_type) = body.plan_type {
        metadata.insert("plan_type".to_string(), plan_type.clone());
    }

    // Subscription data with trial for eligible countries
    let mut sub_data = stripe::CreateCheckoutSessionSubscriptionData {
        metadata: Some(metadata.clone()),
        ..Default::default()
    };

    // Add 7-day trial for new subscriptions in eligible countries
    if body.subscription_type == SubscriptionType::Hosted {
        if country == "US" || country == "CA" || is_euro_plan_country(country) {
            sub_data.trial_period_days = Some(7);
        }
    }

    let create_params = CreateCheckoutSession {
        success_url: Some(&success_url),
        cancel_url: Some(&cancel_url),
        mode: Some(stripe::CheckoutSessionMode::Subscription),
        line_items: Some(line_items),
        // No customer - Stripe will create one
        customer_creation: Some(stripe::CheckoutSessionCustomerCreation::Always),
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
    tracing::debug!("Starting create_customer_portal_session for user_id: {}", user_id);
    // Check if user is accessing their own data or is an admin
    if auth_user.user_id != user_id && !auth_user.is_admin {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Access denied"})),
        ));
    }
    // Initialize Stripe client
    let stripe_secret_key = std::env::var("STRIPE_SECRET_KEY")
        .expect("STRIPE_SECRET_KEY must be set in environment");
    let client = Client::new(stripe_secret_key);
    tracing::debug!("Stripe client initialized");
    tracing::debug!("JWT token validated successfully");
    // Get Stripe customer ID
    let customer_id = state
        .user_repository
        .get_stripe_customer_id(user_id)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)})),
        ))?
        .ok_or_else(|| (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "No Stripe customer ID found for user"})),
        ))?;
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
    let portal_session = BillingPortalSession::create(
        &client,
create_session,
    )
    .await
    .map_err(|e| {
            tracing::error!("{}", e);
        (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": format!("Failed to create Customer Portal session: {}", e)})),
    )})?;
    tracing::debug!("Portal session created successfully with URL: {}", portal_session.url);
    // Return the portal URL to redirect the user
    Ok(Json(json!({
        "url": portal_session.url,
        "message": "Redirecting to Stripe Customer Portal"
    })))
}

pub async fn create_checkout_session(
    State(state): State<Arc<AppState>>,
    _auth_user: AuthUser,
    Path(user_id): Path<i32>,
    Json(payload): Json<BuyCreditsRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    tracing::debug!("Starting create_checkout_session for user_id: {}", user_id);
    // Fetch user from the database
    let user = state
        .user_core
        .find_by_id(user_id)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)})),
        ))?
        .ok_or_else(|| (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found"})),
        ))?;
    tracing::debug!("User found: {}", user.id);
    // Initialize Stripe client
    let stripe_secret_key = std::env::var("STRIPE_SECRET_KEY")
        .expect("STRIPE_SECRET_KEY must be set in environment");
    let client = Client::new(stripe_secret_key.clone());

    // Check if user is on Digest plan (only Digest users can buy overage credits)
    // US/CA users are exempt from this check
    let is_us_ca = user.phone_number_country == Some("US".to_string())
        || user.phone_number_country == Some("CA".to_string());

    if !is_us_ca {
        // First check database for plan_type
        let plan_type = match user.plan_type.as_deref() {
            Some(pt) => Some(pt.to_string()),
            None => {
                // Lazy migration: plan_type not set, fetch from Stripe and save to DB
                crate::utils::country::lazy_migrate_plan_type(user_id, &state).await
            }
        };

        // Check if user is on Digest plan
        if plan_type.as_deref() != Some("digest") {
            tracing::info!("User {} attempted to buy credits but is not on Digest plan (plan_type={:?})", user_id, plan_type);
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
                },
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
                            ))
                        }
                    },
                    _ => {
                        // Handle other types of errors
                        tracing::error!("An error occurred: {:?}", e);
                        return Err((
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({"error": format!("Stripe error: {:?}", e)})),
                        ));
                    }
                }
            }
        },
        Ok(None) => {
            tracing::debug!("No Stripe customer ID found, creating new customer");
            create_new_customer(&client, user_id, &user.email, &state).await?
        },
        Err(e) => return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)})),
        )),
    };
   
    let amount_dollars = payload.amount_dollars; // From BuyCreditsRequest
    let amount_cents = (amount_dollars * 100.0).round() as i64; // Convert to cents for Stripe
    tracing::info!("Processing payment of {} EUR ({} cents)", amount_dollars, amount_cents);
    let domain_url = std::env::var("FRONTEND_URL").expect("FRONTEND_URL not set");
   
    // Create a Checkout Session with payment method attachment
    tracing::debug!("Creating Stripe checkout session");
    let checkout_session = CheckoutSession::create(
        &client,
        CreateCheckoutSession {
            success_url: Some(&format!("{}/billing", domain_url)), // Redirect after success
            cancel_url: Some(&format!("{}/billing", domain_url)), // Redirect after cancellation
            payment_method_types: Some(vec![stripe::CreateCheckoutSessionPaymentMethodTypes::Card]), // Allow card payments
            mode: Some(stripe::CheckoutSessionMode::Payment), // One-time payment mode
            line_items: Some(vec![
                stripe::CreateCheckoutSessionLineItems {
                    price_data: Some(stripe::CreateCheckoutSessionLineItemsPriceData {
                        currency: stripe::Currency::EUR,
                        product: Some(std::env::var("STRIPE_CREDITS_PRODUCT_ID").expect("STRIPE_CREDITS_PRODUCT_ID not set")),
                        unit_amount: Some(amount_cents), // Amount in cents
                        ..Default::default()
                    }),
                    quantity: Some(1),
                    ..Default::default()
                }
            ]),
            customer: Some(customer_id.parse().unwrap()),
            customer_update: Some(stripe::CreateCheckoutSessionCustomerUpdate {
                address: Some(stripe::CreateCheckoutSessionCustomerUpdateAddress::Auto),
                ..Default::default()
            }),
            payment_intent_data: Some(stripe::CreateCheckoutSessionPaymentIntentData {
                setup_future_usage: Some(stripe::CreateCheckoutSessionPaymentIntentDataSetupFutureUsage::OffSession),
                ..Default::default()
            }),
            automatic_tax: Some(stripe::CreateCheckoutSessionAutomaticTax {
                enabled: true, // Enable Stripe Tax to calculate taxes automatically
                liability: None, // default behavior
            }),
            billing_address_collection: Some(stripe::CheckoutSessionBillingAddressCollection::Required),
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
        .set_stripe_checkout_session_id(user_id, &checkout_session.id.to_string())
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)})),
        ))?;
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
        .set_stripe_customer_id(user_id, &customer.id.to_string())
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
fn base_price_id(items: &[stripe::SubscriptionItem]) -> Option<String> {
    let topup_id = std::env::var("STRIPE_TOPUP_PRICE_ID").unwrap_or_default();
    items.iter()
         .find(|it| it.price.as_ref().map(|p| p.id.as_str()) != Some(topup_id.as_str()))
         .and_then(|it| it.price.as_ref())
         .map(|p| p.id.to_string())
}
// Helper function to check if DigitalDetox one-time fee is present
fn is_digital_detox_subscription(items: &[stripe::SubscriptionItem]) -> bool {
    let digital_detox_fee_id_us = std::env::var("STRIPE_DIGITALDETOX_ONETIME_FEE_ID_US").unwrap_or_default();
    let digital_detox_fee_id_other = std::env::var("STRIPE_DIGITALDETOX_ONETIME_FEE_ID_OTHER").unwrap_or_default();
    items.iter().any(|item| {
        item.price.as_ref().map(|p| {
            p.id.as_str() == digital_detox_fee_id_us || p.id.as_str() == digital_detox_fee_id_other
        }).unwrap_or(false)
    })
}
// Helper function to extract subscription info from price ID
fn extract_subscription_info(price_id: &str) -> SubscriptionInfo {
    // Default values - all subscriptions map to tier 2 (hosted) unless it's tier 3 (self-hosted)
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

    // Check if it's self-hosted (tier 3)
    if price_id == std::env::var("STRIPE_SUBSCRIPTION_SELF_HOSTING_PRICE_ID").unwrap_or_default() {
        info.tier = "tier 3";
        return info;
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
        check_price_id!(country, format!("STRIPE_SUBSCRIPTION_HARD_MODE_PRICE_ID_{}", country), "tier 2");
        check_price_id!(country, format!("STRIPE_SUBSCRIPTION_BASIC_DAILY_PRICE_ID_{}", country), "tier 2");
        check_price_id!(country, format!("STRIPE_SUBSCRIPTION_BASIC_PRICE_ID_{}", country), "tier 2");
        check_price_id!(country, format!("STRIPE_SUBSCRIPTION_WORLD_PRICE_ID_{}", country), "tier 2");
        check_price_id!(country, format!("STRIPE_SUBSCRIPTION_ESCAPE_DAILY_PRICE_ID_{}", country), "tier 2");
        check_price_id!(country, format!("STRIPE_SUBSCRIPTION_MONITORING_PRICE_ID_{}", country), "tier 2");
        check_price_id!(country, format!("STRIPE_SUBSCRIPTION_ORACLE_PRICE_ID_{}", country), "tier 2");

        // Current price IDs
        check_price_id!(country, format!("STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_{}", country), "tier 2");
        check_price_id!(country, format!("STRIPE_SUBSCRIPTION_HOSTED_PLAN_PRICE_ID_{}", country), "tier 2");
    }

    info
}
fn is_sentinel_price_id(price_id: &str) -> bool {
    const COUNTRIES: [&str; 6] = ["US", "FI", "UK", "NL", "AU", "OTHER"];
    COUNTRIES.iter().any(|cty| {
        let var_name = format!("STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_{}", cty);
        let var_name_two = format!("STRIPE_SUBSCRIPTION_HOSTED_PLAN_PRICE_ID_{}", cty);
        // If the env-var is missing, simply skip that entry.
        if std::env::var(var_name).map_or(false, |v| v == price_id) || std::env::var(var_name_two).map_or(false, |v| v == price_id) {
            true
        } else {
            false
        }
    })
}

pub async fn stripe_webhook(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<impl IntoResponse, (StatusCode, Json<Value>)> {
    let payload_str = String::from_utf8(body.to_vec())
        .map_err(|_| (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid payload encoding"})),
        ))?;
    tracing::info!("Stripe webhook received");
    // Initialize Stripe client
    let stripe_secret_key = std::env::var("STRIPE_SECRET_KEY")
        .expect("STRIPE_SECRET_KEY must be set in environment");
    let client = Client::new(stripe_secret_key);
    // Get the webhook secret from environment
    let webhook_secret = std::env::var("STRIPE_WEBHOOK_SECRET")
        .expect("STRIPE_WEBHOOK_SECRET must be set in environment");
    // Extract the stripe-signature header
    let sig_header = headers
        .get("stripe-signature")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Missing Stripe-Signature header"})),
        ))?;
    tracing::info!("Stripe signature header found");
    // Construct and verify the Stripe event using the signature
    let event = stripe::Webhook::construct_event(
        &payload_str,
        &sig_header,
        &webhook_secret,
    ).map_err(|e| (
        StatusCode::BAD_REQUEST,
        Json(json!({"error": format!("Invalid Stripe webhook signature: {}", e)})),
    ))?;
  
    tracing::info!("Stripe event verified successfully: {}", event.type_);
    // Process the event based on its type
    match event.type_ {
      
        stripe::EventType::CustomerSubscriptionCreated | stripe::EventType::CustomerSubscriptionUpdated => {
            tracing::info!("Processing subscription created/updated event");
            if let stripe::EventObject::Subscription(subscription) = event.data.object {
                let customer_id = match subscription.customer {
                    stripe::Expandable::Id(id) => id,
                    stripe::Expandable::Object(customer) => customer.id,
                };
                // Get the base price ID from the subscription items
                let base_price = subscription.items.data.first()
                    .and_then(|item| item.price.as_ref())
                    .map(|price| price.id.to_string())
                    .ok_or_else(|| {
                        tracing::error!("No price found in subscription items");
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({"error": "Invalid subscription data"})),
                        )
                    })?;
                let _new_sub_info = extract_subscription_info(&base_price);
                // Always check and cancel existing subscriptions when a new one is created
                if event.type_ == stripe::EventType::CustomerSubscriptionCreated {
                    tracing::info!("New subscription created, checking for existing subscriptions to cancel");

                    // ========== GUEST CHECKOUT: Create user if not exists ==========
                    // Check if this is a guest checkout (user doesn't exist for this customer)
                    let existing_user = state.user_repository.find_by_stripe_customer_id(&customer_id.as_str());
                    if let Ok(None) = existing_user {
                        tracing::info!("Guest checkout detected - creating user for customer {}", customer_id);

                        // Fetch customer details from Stripe
                        if let Ok(customer) = Customer::retrieve(&client, &customer_id, &[]).await {
                            let email = customer.email.clone().unwrap_or_default();
                            let phone = customer.phone.clone().unwrap_or_default();

                            if !email.is_empty() {
                                // Check if user with this email already exists
                                let email_exists = state.user_core.find_by_email(&email);

                                match email_exists {
                                    Ok(Some(existing_user_by_email)) => {
                                        // Link existing user to this Stripe customer
                                        tracing::info!("Found existing user {} with email {}, linking to Stripe customer",
                                            existing_user_by_email.id, email);
                                        let _ = state.user_repository.set_stripe_customer_id(
                                            existing_user_by_email.id,
                                            &customer_id.to_string()
                                        );
                                        // Update phone if provided and user doesn't have one
                                        if !phone.is_empty() && existing_user_by_email.phone_number.is_empty() {
                                            let _ = state.user_core.update_phone_number(existing_user_by_email.id, &phone);
                                        }
                                    },
                                    Ok(None) => {
                                        // Create new user (no password - they'll set it via magic link)
                                        use crate::handlers::auth_dtos::NewUser;
                                        use rand::Rng;

                                        // Generate magic token
                                        let magic_token: String = rand::thread_rng()
                                            .sample_iter(&rand::distributions::Alphanumeric)
                                            .take(64)
                                            .map(char::from)
                                            .collect();

                                        let new_user = NewUser {
                                            email: email.clone(),
                                            password_hash: "NOT_SET".to_string(), // Placeholder - user sets via magic link
                                            phone_number: phone.clone(),
                                            time_to_live: 0, // No expiry for paid users
                                            verified: true, // Payment = verified
                                            credits: 0.0,
                                            credits_left: 0.0,
                                            charge_when_under: false,
                                            waiting_checks_count: 0,
                                            discount: false,
                                            sub_tier: None,
                                        };

                                        if let Ok(()) = state.user_core.create_user(new_user) {
                                            if let Ok(Some(created_user)) = state.user_core.find_by_email(&email) {
                                                tracing::info!("Created new user {} from guest checkout", created_user.id);

                                                // Link Stripe customer
                                                let _ = state.user_repository.set_stripe_customer_id(
                                                    created_user.id,
                                                    &customer_id.to_string()
                                                );

                                                // Set phone country from phone number
                                                if !phone.is_empty() {
                                                    let _ = crate::handlers::profile_handlers::set_user_phone_country(
                                                        &state, created_user.id, &phone
                                                    ).await;
                                                }

                                                // Ensure user settings and info exist
                                                let _ = state.user_core.ensure_user_settings_exist(created_user.id);
                                                let _ = state.user_core.ensure_user_info_exists(created_user.id);

                                                // Store magic token
                                                let _ = state.user_core.set_magic_token(created_user.id, &magic_token);

                                                // Store session_id -> token mapping for redirect flow
                                                // We need to get the checkout session ID from the subscription metadata
                                                if let Some(session_id) = subscription.metadata.get("checkout_session_id") {
                                                    state.session_to_token.insert(session_id.clone(), magic_token.clone());
                                                }

                                                // Send magic link email
                                                let frontend_url = std::env::var("FRONTEND_URL").unwrap_or_default();
                                                let magic_link = format!("{}/set-password/{}", frontend_url, magic_token);
                                                let email_clone = email.clone();

                                                tokio::spawn(async move {
                                                    if let Err(e) = crate::utils::email::send_magic_link_email(&email_clone, &magic_link).await {
                                                        tracing::error!("Failed to send magic link email: {}", e);
                                                    }
                                                });
                                            }
                                        }
                                    },
                                    Err(e) => {
                                        tracing::error!("Error checking if email exists: {}", e);
                                    }
                                }
                            }
                        }
                    }
                    // ========== END GUEST CHECKOUT ==========
                  
                    // List all active subscriptions for the customer
                    let existing_subscriptions = stripe::Subscription::list(
                        &client,
                        &stripe::ListSubscriptions {
                            customer: Some(customer_id.clone()),
                            status: Some(stripe::SubscriptionStatusFilter::Active),
                            ..Default::default()
                        },
                    ).await.map_err(|e| {
                        tracing::error!("Failed to list existing subscriptions: {}", e);
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({"error": "Failed to check existing subscriptions"})),
                        )
                    })?;
                    // Cancel all existing subscriptions except the new one
                    for existing_sub in existing_subscriptions.data.iter() {
                        if existing_sub.id != subscription.id {
                            tracing::info!("Canceling existing subscription: {}", existing_sub.id);
                            if let Err(e) = Subscription::update(
                                &client,
                                &existing_sub.id,
                                UpdateSubscription {
                                    cancel_at_period_end: Some(true),
                                    ..Default::default()
                                },
                            ).await {
                                tracing::error!("Failed to cancel subscription {}: {}", existing_sub.id, e);
                            } else {
                                tracing::info!("Successfully scheduled cancellation for subscription: {}", existing_sub.id);
                            }
                        }
                    }
                    // Add first-time bonus credits for non-US/CA users on new hosted plans
                    if let Ok(Some(user)) = state.user_repository.find_by_stripe_customer_id(&customer_id.as_str()) {
                        let sub_info = extract_subscription_info(&base_price);
                        let is_hosted = sub_info.tier == "tier 2";

                        // Check if user is NOT from US/CA
                        let is_non_us_ca = user.phone_number_country.as_deref() != Some("US")
                            && user.phone_number_country.as_deref() != Some("CA");

                        // Give 10€ starter bonus to non-US/CA users on their first subscription
                        if is_hosted && is_non_us_ca {
                            if let Err(e) = state.user_repository.increase_credits(user.id, 10.0) {
                                tracing::error!("Failed to add first-time bonus credits: {}", e);
                            } else {
                                tracing::info!("Added 10€ first-time bonus credits for non-US/CA user {}", user.id);
                            }
                        }

                        // Automatically create Twilio subaccount for tier 3 (self-hosted) users
                        if sub_info.tier == "tier 3" {
                            tracing::info!("Tier 3 subscription detected, creating Twilio subaccount for user {}", user.id);
                            let state_clone = state.clone();
                            let user_id = user.id;

                            tokio::spawn(async move {
                                let auth_user = AuthUser {
                                    user_id,
                                    is_admin: false,
                                };

                                match self_host_twilio::create_twilio_subaccount(
                                    axum::extract::State(state_clone),
                                    auth_user,
                                ).await {
                                    Ok(_) => {
                                        tracing::info!("Successfully created Twilio subaccount for tier 3 user {}", user_id);
                                    },
                                    Err((status, err)) => {
                                        tracing::error!("Failed to create Twilio subaccount for user {}: {:?} - {:?}", user_id, status, err);
                                    }
                                }
                            });
                        }
                    }
                }
                // Skip processing subscription updates that are part of a plan change
                let is_subscription_change = subscription.metadata.get("plan_change")
                    .map(|val| val == "true")
                    .unwrap_or(false);
                if event.type_ == stripe::EventType::CustomerSubscriptionUpdated && is_subscription_change {
                    tracing::info!("Skipping subscription update as it's part of a plan change");
                    return Ok(StatusCode::OK);
                }
                // Get the base price ID from the subscription items
                let base_price = if let Some(first_item) = subscription.items.data.first() {
                    if let Some(price) = &first_item.price {
                        price.id.to_string()
                    } else {
                        tracing::warn!("No price found in subscription item");
                        return Ok(StatusCode::OK);
                    }
                } else {
                    tracing::warn!("No items found in subscription");
                    return Ok(StatusCode::OK);
                };
                // Extract subscription info to determine the tier
                let _new_sub_info = extract_subscription_info(&base_price);
                if let Ok(Some(user)) = state.user_repository.find_by_stripe_customer_id(&customer_id.as_str()) {
                    let items = &subscription.items.data;
                    let base_price = match base_price_id(items) {
                        Some(id) => id,
                        None => {
                            tracing::warn!("subscription without base price – skipping");
                            return Ok(StatusCode::OK);
                        }
                    };
                    let _sub_info = extract_subscription_info(&base_price);
                    if let Some(price_id) = subscription.items.data.first()
                        .and_then(|item| item.price.as_ref())
                        .map(|price| price.id.to_string())
                    {
                        // Extract subscription info (both country and tier)
                        let sub_info = extract_subscription_info(&price_id);
                        let _is_sentinel_price_id = is_sentinel_price_id(&price_id);
                        let _is_digital_detox = is_digital_detox_subscription(&subscription.items.data);
                      
                        // Update subscription country
                        if let Err(e) = state.user_core.update_sub_country(user.id, sub_info.country) {
                            tracing::error!("Failed to update subscription country: {}", e);
                        }
                        // Update subscription tier
                        if let Err(e) = state.user_repository.set_subscription_tier(user.id, Some(sub_info.tier)) {
                            tracing::error!("Failed to update subscription tier: {}", e);
                        }
                        // Credit allocation logic based on plan type
                        use crate::utils::country::{is_monitor_plan_price, is_digest_plan_price, is_legacy_euro_plan_price, is_byot_plan_price};

                        let messages: f32;
                        tracing::debug!("sub_tier: {}", sub_info.tier);

                        // Tier 3 (self-hosted) gets 0 credits
                        if sub_info.tier == "tier 3" {
                            messages = 0.0;
                            tracing::info!("Self-hosted subscription (tier 3), no monthly credits");
                        }
                        // Tier 2 (hosted) - check plan type for credit allocation
                        else if sub_info.tier == "tier 2" {
                            // US/CA users get 200 monthly credits (same pricing as Monitor)
                            if user.phone_number_country == Some("US".to_string())
                                || user.phone_number_country == Some("CA".to_string()) {
                                messages = 200.0;
                                if let Err(e) = state.user_repository.update_plan_type(user.id, Some("monitor")) {
                                    tracing::error!("Failed to update plan_type: {}", e);
                                }
                                tracing::info!("US/CA tier 2 subscription, allocating 200 monthly credits");
                            }
                            // Monitor plan (€29) gets credits for ~40 regular messages
                            else if is_monitor_plan_price(&price_id) {
                                let country = user.phone_number_country.as_deref().unwrap_or("FI");
                                messages = calculate_euro_credit_allocation(&state, country, 40.0, "Monitor").await;
                                if let Err(e) = state.user_repository.update_plan_type(user.id, Some("monitor")) {
                                    tracing::error!("Failed to update plan_type: {}", e);
                                }
                            }
                            // Digest plan (€49) gets credits for ~120 regular messages
                            else if is_digest_plan_price(&price_id) {
                                let country = user.phone_number_country.as_deref().unwrap_or("FI");
                                messages = calculate_euro_credit_allocation(&state, country, 120.0, "Digest").await;
                                if let Err(e) = state.user_repository.update_plan_type(user.id, Some("digest")) {
                                    tracing::error!("Failed to update plan_type: {}", e);
                                }
                            }
                            // Legacy euro plans (€19) get 0 credits - treat as monitor
                            else if is_legacy_euro_plan_price(&price_id) {
                                messages = 0.0;
                                if let Err(e) = state.user_repository.update_plan_type(user.id, Some("monitor")) {
                                    tracing::error!("Failed to update plan_type: {}", e);
                                }
                                tracing::info!("Legacy euro plan subscription, allocating 0 monthly credits (pay-per-use model)");
                            }
                            // BYOT plan (€19) - bring your own Twilio, 0 credits (pay-per-use)
                            else if is_byot_plan_price(&price_id) {
                                messages = 0.0;
                                if let Err(e) = state.user_repository.update_plan_type(user.id, Some("byot")) {
                                    tracing::error!("Failed to update plan_type: {}", e);
                                }
                                tracing::info!("BYOT plan subscription, allocating 0 monthly credits (pay-per-use model)");
                            }
                            // Fallback for other regions
                            else {
                                messages = 0.0;
                                tracing::info!("Non-US/CA tier 2 subscription, allocating 0 monthly credits (pay-per-use model)");
                            }
                        }
                        // Fallback (should not happen with migration, but keeping for safety)
                        else {
                            messages = 0.0;
                            tracing::warn!("Unknown subscription tier: {}, defaulting to 0 credits", sub_info.tier);
                        }
                        if let Err(e) = state.user_repository.update_sub_credits(user.id, messages) {
                            tracing::error!("Failed to update subscription credits: {}", e);
                        } else {
                            tracing::info!("Set daily credits to 40 for user {}", user.id);
                        }
                        // Reset monthly message count for tier 3 users on billing cycle renewal
                        if sub_info.tier == "tier 3" {
                            if let Err(e) = state.user_core.reset_monthly_message_count(user.id) {
                                tracing::error!("Failed to reset monthly message count for tier 3 user {}: {}", user.id, e);
                            } else {
                                tracing::info!("Reset monthly message count for tier 3 user {} on billing renewal", user.id);
                            }
                        }
                        // Update next billing date
                        if let Err(e) = state.user_core.update_next_billing_date(user.id, subscription.current_period_end as i32) {
                            tracing::error!("Failed to update next billing date: {}", e);
                        } else {
                            tracing::info!("Updated next billing date for user {}: {}", user.id, subscription.current_period_end);
                        }
                        tracing::info!("Updated subscription info for user {}: country={:#?}, tier={}",
                            user.id, sub_info.country, sub_info.tier);

                        // Clear BYOT credentials if user is subscribing to a non-BYOT plan
                        // This ensures users switching from BYOT to Lightfriend-managed plans
                        // don't have mismatched routing (their credentials vs Lightfriend's)
                        if !crate::utils::country::is_byot_plan_price(&price_id) {
                            if state.user_core.has_twilio_credentials(user.id) {
                                tracing::info!("User {} switching to non-BYOT plan, clearing BYOT credentials", user.id);
                                if let Err(e) = state.user_core.clear_twilio_credentials(user.id) {
                                    tracing::error!("Failed to clear BYOT credentials for user {}: {}", user.id, e);
                                }
                            }
                        }

                        // Tier 3: Provision subaccount or regenerate Tinfoil key
                        if sub_info.tier == "tier 3" {
                            if event.type_ == stripe::EventType::CustomerSubscriptionCreated {
                                // New tier 3 subscription - provision subaccount
                                tracing::info!("New tier 3 subscription - provisioning subaccount for user {}", user.id);

                                // Check if user already has a subaccount (shouldn't happen, but check anyway)
                                match state.user_core.find_subaccount_by_user_id(user.id) {
                                    Ok(Some(existing_sub)) => {
                                        tracing::warn!("User {} already has subaccount {}, skipping provisioning",
                                            user.id, existing_sub.subaccount_sid);
                                    }
                                    Ok(None) => {
                                        // No existing subaccount - create one
                                        let state_clone = state.clone();
                                        let user_id_clone = user.id;

                                        // Spawn background task to provision subaccount
                                        tokio::spawn(async move {
                                            // Create subaccount using existing function
                                            // Note: We can't call the handler directly, so we'll need to duplicate some logic
                                            match crate::utils::self_host_twilio::provision_tier3_subaccount(&state_clone, user_id_clone).await {
                                                Ok(_) => {
                                                    tracing::info!("Successfully provisioned tier 3 subaccount for user {}", user_id_clone);

                                                    // Maintain US buffer pool if user is US
                                                    if let Ok(Some(u)) = state_clone.user_core.find_by_id(user_id_clone) {
                                                        if u.phone_number_country == Some("US".to_string()) {
                                                            if let Err(e) = crate::utils::us_number_pool::maintain_us_buffer_pool(&state_clone).await {
                                                                tracing::error!("Failed to maintain US buffer pool: {}", e);
                                                            }
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    tracing::error!("Failed to provision tier 3 subaccount for user {}: {}", user_id_clone, e);
                                                }
                                            }
                                        });
                                    }
                                    Err(e) => {
                                        tracing::error!("Error checking for existing subaccount: {}", e);
                                    }
                                }
                            } else if event.type_ == stripe::EventType::CustomerSubscriptionUpdated {
                                // Renewal - regenerate Tinfoil key
                                let new_expiry = subscription.current_period_end;
                                tracing::info!("Tier 3 subscription renewal for user {} - regenerating Tinfoil key", user.id);

                                let state_clone = state.clone();
                                let user_id_clone = user.id;

                                tokio::spawn(async move {
                                    match crate::utils::subaccount_lifecycle::regenerate_tinfoil_key(
                                        &state_clone,
                                        user_id_clone,
                                        new_expiry,
                                    ).await {
                                        Ok(_) => {
                                            tracing::info!("Successfully regenerated Tinfoil key for user {}", user_id_clone);
                                        }
                                        Err(e) => {
                                            tracing::error!("Failed to regenerate Tinfoil key for user {}: {}", user_id_clone, e);
                                        }
                                    }
                                });
                            }
                        }
                    }
                }
            }
        },
        stripe::EventType::CustomerSubscriptionDeleted => {
            tracing::info!("Processing customer.subscription.deleted event");
            if let stripe::EventObject::Subscription(subscription) = event.data.object {
                let customer_id = match subscription.customer {
                    stripe::Expandable::Id(id) => id,
                    stripe::Expandable::Object(customer) => customer.id,
                };
              
                // Check if this deletion is part of a subscription change/upgrade
                let is_subscription_change = subscription.metadata.get("plan_change")
                    .map(|val| val == "true")
                    .unwrap_or(false);
                if is_subscription_change {
                    tracing::info!("Subscription deletion is part of a plan change, skipping tier update");
                    return Ok(StatusCode::OK);
                }
              
                if let Ok(Some(user)) = state.user_repository.find_by_stripe_customer_id(&customer_id.as_str()) {
                    // Check for other active subscriptions
                    let active_subscriptions = stripe::Subscription::list(
                        &client,
                        &stripe::ListSubscriptions {
                            customer: Some(customer_id.clone()),
                            status: Some(stripe::SubscriptionStatusFilter::Active),
                            ..Default::default()
                        },
                    ).await.map_err(|e| {
                        tracing::error!("Failed to list subscriptions: {}", e);
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({"error": "Failed to check existing subscriptions"})),
                        )
                    })?;
                    // Simplified deletion logic - just check if any active subscriptions remain
                    if active_subscriptions.data.is_empty() {
                        // Get the deleted subscription's tier
                        let deleted_tier = subscription.items.data.first()
                            .and_then(|item| item.price.as_ref())
                            .map(|price| extract_subscription_info(&price.id).tier);

                        // If tier 3, handle subaccount cleanup
                        if deleted_tier == Some("tier 3") {
                            tracing::info!("Tier 3 subscription cancelled for user {}, handling subaccount cleanup", user.id);
                            let state_clone = state.clone();
                            let user_id_clone = user.id;

                            tokio::spawn(async move {
                                match crate::utils::subaccount_lifecycle::handle_tier3_cancellation(
                                    &state_clone,
                                    user_id_clone,
                                ).await {
                                    Ok(_) => {
                                        tracing::info!("Successfully handled tier 3 cancellation for user {}", user_id_clone);
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to handle tier 3 cancellation for user {}: {}", user_id_clone, e);
                                    }
                                }
                            });
                        }

                        // No active subscriptions left, clear subscription tier and country
                        tracing::info!("No active subscriptions remaining, clearing subscription info");
                        if let Err(e) = state.user_repository.set_subscription_tier(user.id, None) {
                            tracing::error!("Failed to clear subscription tier: {}", e);
                        }
                        if let Err(e) = state.user_core.update_sub_country(user.id, None) {
                            tracing::error!("Failed to clear subscription country: {}", e);
                        }
                        // Clear monthly credits
                        if let Err(e) = state.user_repository.update_sub_credits(user.id, 0.0) {
                            tracing::error!("Failed to clear subscription credits: {}", e);
                        }
                    } else {
                        // User still has active subscriptions - update to the first one found
                        // (since all tiers are either tier 2 or tier 3 now, this is simple)
                        if let Some(remaining_sub) = active_subscriptions.data.first() {
                            if let Some(tier_info) = remaining_sub.items.data.first()
                                .and_then(|item| item.price.as_ref())
                                .map(|price| extract_subscription_info(&price.id))
                            {
                                tracing::info!("Updating subscription tier to {} based on remaining subscription", tier_info.tier);
                                if let Err(e) = state.user_repository.set_subscription_tier(user.id, Some(tier_info.tier)) {
                                    tracing::error!("Failed to update subscription tier: {}", e);
                                }
                                if let Err(e) = state.user_core.update_sub_country(user.id, tier_info.country) {
                                    tracing::error!("Failed to update subscription country: {}", e);
                                }
                            }
                        }
                    }
                }
            }
        },
        stripe::EventType::CheckoutSessionCompleted => {
            tracing::info!("Processing checkout.session.completed event");
            match event.data.object {
                stripe::EventObject::CheckoutSession(session) => {
                    tracing::info!("Checkout session found: {}", session.id);
                  
                    // Skip processing if this is a subscription checkout
                    if matches!(session.mode, stripe::CheckoutSessionMode::Subscription) {
                        tracing::info!("Ignoring subscription checkout session");
                        return Ok(StatusCode::OK);
                    }
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
                        let payment_intent = session.payment_intent.as_ref()
                            .ok_or_else(|| (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(json!({"error": "No payment intent in session"})),
                            ))?;
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
                                .map_err(|e| (
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    Json(json!({"error": format!("Database error: {}", e)})),
                                ))?
                                .ok_or_else(|| (
                                    StatusCode::NOT_FOUND,
                                    Json(json!({"error": "Customer not found"})),
                                ))?;
                            tracing::info!("Found user with ID: {}", user.id);
                            state
                                .user_repository
                                .set_stripe_payment_method_id(user.id, &payment_method_id)
                                .map_err(|e| (
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    Json(json!({"error": format!("Database error: {}", e)})),
                                ))?;
                            tracing::info!("Successfully saved payment method ID for user");
                            let amount_in_cents = session.amount_subtotal.unwrap_or(0);
                            let amount = amount_in_cents as f32 / 100.00;
                            state
                                .user_repository
                                .increase_credits(user.id, amount)
                                .map_err(|e| (
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    Json(json!({"error": format!("Database error: {}", e)})),
                                ))?;
                            tracing::info!("Increased the credits amount by {} successfully", amount);
                        }
                    }
                },
                _ => {
                    tracing::error!("Checkout session not found in event object");
                }
            }
        },
        _ => {
            tracing::info!("Ignoring non-checkout.session.completed event");
        }
    }
    tracing::info!("Webhook processed successfully");
    Ok(StatusCode::OK) // Return 200 OK for successful webhook processing
}

pub async fn fetch_next_billing_date(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(user_id): Path<i32>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    tracing::debug!("Starting fetch_next_billing_date for user_id: {}", user_id);
    // Check if user is accessing their own data or is an admin
    if auth_user.user_id != user_id && !auth_user.is_admin {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Access denied"})),
        ));
    }
    // Initialize Stripe client
    let stripe_secret_key = std::env::var("STRIPE_SECRET_KEY")
        .expect("STRIPE_SECRET_KEY must be set in environment");
    let client = Client::new(stripe_secret_key);
    tracing::debug!("Stripe client initialized");
    // Get Stripe customer ID
    let customer_id = state
        .user_repository
        .get_stripe_customer_id(user_id)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)})),
        ))?
        .ok_or_else(|| (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "No Stripe customer ID found for user"})),
        ))?;
    tracing::debug!("Found Stripe customer ID: {}", customer_id);
    // List all subscriptions for the customer
    let subscriptions = stripe::Subscription::list(
        &client,
        &stripe::ListSubscriptions {
            customer: Some(customer_id.parse().unwrap()),
            status: Some(stripe::SubscriptionStatusFilter::Active),
            ..Default::default()
        },
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to list subscriptions: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to fetch subscriptions: {}", e)})),
        )
    })?;
    // Find the latest end date among all active subscriptions
    let latest_end_date = subscriptions
        .data
        .iter()
        .map(|sub| sub.current_period_end)
        .max()
        .ok_or_else(|| (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "No active subscriptions found"})),
        ))?;
    // Update the user's next billing date
    state
        .user_core
        .update_next_billing_date(user_id, latest_end_date as i32)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to update next billing date: {}", e)})),
        ))?;
    tracing::info!("Successfully updated next billing date to {}", latest_end_date);
    Ok(Json(json!({
        "message": "Successfully updated next billing date",
        "next_billing_date": latest_end_date
    })))
}

pub async fn automatic_charge(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<i32>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    tracing::debug!("Starting automatic_charge for user_id: {}", user_id);
    // Initialize Stripe client
    let stripe_secret_key = std::env::var("STRIPE_SECRET_KEY")
        .expect("STRIPE_SECRET_KEY must be set in environment");
    let client = Client::new(stripe_secret_key);
    tracing::debug!("Stripe client initialized");
    // Fetch user from the database
    let user = state
        .user_core
        .find_by_id(user_id)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)})),
        ))?
        .ok_or_else(|| (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found"})),
        ))?;
    tracing::debug!("User found: {}", user.id);
    // Get Stripe customer ID and payment method ID
    let customer_id = state
        .user_repository
        .get_stripe_customer_id(user_id)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)})),
        ))?
        .ok_or_else(|| (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "No Stripe customer ID found for user"})),
        ))?;
    println!("Stripe customer ID found");
    let payment_method_id = state
        .user_repository
        .get_stripe_payment_method_id(user_id)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)})),
        ))?
        .ok_or_else(|| (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "No Stripe payment method found for user"})),
        ))?;
    tracing::debug!("Stripe payment method ID found");
    let charge_back_to = user.charge_back_to.unwrap_or(5.00);
    tracing::debug!("User charge_back_to: {}, current credits: {}", charge_back_to, user.credits);
   
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
   
    tracing::debug!("Payment intent created with status: {:?}", payment_intent.status);
   
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
        tracing::warn!("Payment intent failed or requires action, status: {:?}", payment_intent.status);
        Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Payment intent failed or requires action"})),
        ))
    }
}
