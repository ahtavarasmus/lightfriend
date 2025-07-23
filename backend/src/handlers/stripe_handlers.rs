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
    ListSubscriptions,
};

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub enum SubscriptionType {
    Hosted,
    SelfHosting,
    DigitalDetox,
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

// Assuming BuyCreditsRequest is defined in billing_models.rs
#[derive(Deserialize, Serialize, Clone, PartialEq)]
pub struct BuyCreditsRequest {
    pub amount_dollars: f32,
}

#[derive(Deserialize)]
pub struct SubscriptionCheckoutBody {
    pub subscription_type: SubscriptionType,
}

pub async fn create_unified_subscription_checkout(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(user_id): Path<i32>,
    Json(body): Json<SubscriptionCheckoutBody>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    println!("Starting create_subscription_checkout for user_id: {}", user_id);

    // Verify user exists in database
    let user = state
        .user_core
        .find_by_id(user_id)
        .map_err(|e| {
            println!("Database error when finding user: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to verify user"})),
            )
        })?
        .ok_or_else(|| {
            println!("User not found: {}", user_id);
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "User not found"})),
            )
        })?;

    // Initialize Stripe client
    let stripe_secret_key = std::env::var("STRIPE_SECRET_KEY").map_err(|_| {
        println!("STRIPE_SECRET_KEY not found in environment");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Stripe configuration error"})),
        )
    })?;
    let client = Client::new(stripe_secret_key);
    println!("Stripe client initialized");

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
        println!("Error fetching existing subscriptions: {:?}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to check existing subscriptions"})),
        )
    })?;

    let domain_url = std::env::var("FRONTEND_URL").expect("FRONTEND_URL not set");

    // Select price ID based on subscription type and user's phone number
    // Select price ID based on subscription type and user's phone number
    let (base_price_id, trial_days, one_time_fee_id) = match body.subscription_type {
        SubscriptionType::Hosted => {
            let price_id = if user.phone_number.starts_with("+1") {
                std::env::var("STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_US")
            } else if user.phone_number.starts_with("+358") {
                std::env::var("STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_FI")
            } else if user.phone_number.starts_with("+44") {
                std::env::var("STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_UK")
            } else if user.phone_number.starts_with("+61") {
                std::env::var("STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_AU")
            } else {
                std::env::var("STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_OTHER")
            }.expect("Stripe price ID not found for region");
            (price_id, None, None)
        },
        SubscriptionType::SelfHosting => {
            let price_id = std::env::var("STRIPE_SUBSCRIPTION_SELF_HOSTING_PRICE_ID")
                .expect("Stripe price ID not found for region");
            (price_id, None, None)
        },
        SubscriptionType::DigitalDetox => {
            let price_id = if user.phone_number.starts_with("+1") {
                std::env::var("STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_US")
            } else if user.phone_number.starts_with("+358") {
                std::env::var("STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_FI")
            } else if user.phone_number.starts_with("+44") {
                std::env::var("STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_UK")
            } else if user.phone_number.starts_with("+61") {
                std::env::var("STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_AU")
            } else {
                std::env::var("STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_OTHER")
            }.expect("Stripe price ID not found for region");
            let one_time_fee_id = if user.phone_number.starts_with("+1") {
                std::env::var("STRIPE_DIGITALDETOX_ONETIME_FEE_ID_US")
            } else {
                std::env::var("STRIPE_DIGITALDETOX_ONETIME_FEE_ID_OTHER")
            }
                .expect("Stripe one-time fee ID not found");
            (price_id, Some(7), Some(one_time_fee_id))
        }
    };

    let mut line_items = vec![
        stripe::CreateCheckoutSessionLineItems {
            price: Some(base_price_id),
            quantity: Some(1),
            ..Default::default()
        }
    ];

    // Add one-time fee for DigitalDetox
    if let Some(fee_id) = one_time_fee_id {
        line_items.push(stripe::CreateCheckoutSessionLineItems {
            price: Some(fee_id),
            quantity: Some(1),
            ..Default::default()
        });
    }

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
            shipping: None,
        }),
        ..Default::default()
    };

    // Add trial period for DigitalDetox
    if let Some(trial_days) = trial_days {
        create_params.subscription_data = Some(stripe::CreateCheckoutSessionSubscriptionData {
            trial_period_days: Some(trial_days),
            ..Default::default()
        });
    }

    let success_url1 = format!("{}/billing?subscription=changed", domain_url);
    if let Some(current_subscription) = existing_subscription.data.first() {
        println!("Found existing subscription: {}", current_subscription.id);
        
        // Create metadata to track the subscription change
        let mut metadata = std::collections::HashMap::new();
        metadata.insert("replacing_subscription".to_string(), current_subscription.id.to_string());
        metadata.insert("plan_change".to_string(), "true".to_string());
        metadata.insert("user_id".to_string(), user_id.to_string());
        
        create_params.subscription_data = Some(stripe::CreateCheckoutSessionSubscriptionData {
            metadata: Some(metadata),
            trial_period_days: trial_days, // Preserve trial_days from earlier
            ..Default::default()
        });

        // Update success URL to indicate plan change
        create_params.success_url = Some(&success_url1);
    }

    let checkout_session = CheckoutSession::create(
        &client,
        create_params,
    )
    .await
    .map_err(|e| {
        println!("Stripe error details: {:?}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to create Subscription Checkout Session: {}", e)})),
        )
    })?;

    println!("Subscription checkout session created successfully");
    
    // Return the Checkout session URL
    Ok(Json(json!({
        "url": checkout_session.url.unwrap(),
        "message": "Redirecting to Stripe Checkout for subscription"
    })))
}


pub async fn create_customer_portal_session(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(user_id): Path<i32>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    println!("Starting create_customer_portal_session for user_id: {}", user_id);

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
    println!("Stripe client initialized");

    println!("JWT token validated successfully");

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
    println!("Found Stripe customer ID: {}", customer_id);

    // Create a Billing Portal Session
    // Create a Billing Portal Session
    let mut create_session = CreateBillingPortalSession::new(customer_id.parse().unwrap());

    // Store the formatted URL in a variable first
    let return_url = format!(
        "{}/billing",
        std::env::var("FRONTEND_URL").expect("FRONTEND_URL not set")
    );
    create_session.return_url = Some(&return_url);
    println!("Creating portal session with return URL: {}", return_url);

    let portal_session = BillingPortalSession::create(
        &client,
create_session,
    )
    .await
    .map_err(|e| {
            eprintln!("{}", e);
        (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": format!("Failed to create Customer Portal session: {}", e)})),
    )})?;
    println!("Portal session created successfully with URL: {}", portal_session.url);

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
    println!("Starting create_checkout_session for user_id: {}", user_id);

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

    println!("User found: {}", user.id);
    // Initialize Stripe client
    let stripe_secret_key = std::env::var("STRIPE_SECRET_KEY")
        .expect("STRIPE_SECRET_KEY must be set in environment");
    let client = Client::new(stripe_secret_key);

    println!("Stripe client initialized");
    // Check if user has a Stripe customer ID; if not, create one

    // Check if user has a Stripe customer ID; if not, create one
    let customer_id = match state.user_repository.get_stripe_customer_id(user_id) {
        Ok(Some(id)) => {
            println!("Found existing Stripe customer ID");
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
                            println!("Customer {} does not exist, creating new customer", id);
                            create_new_customer(&client, user_id, &user.email, &state).await?
                        } else {
                            // Handle other types of API errors
                            let error = stripe_error.message.unwrap();
                            println!("API error: {}", error);
                            return Err((
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(json!({"error": format!("Stripe API error: {}", error)})),
                            ))
                        }
                    },
                    _ => {
                        // Handle other types of errors
                        println!("An error occurred: {:?}", e);
                        return Err((
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({"error": format!("Stripe error: {:?}", e)})),
                        ));
                    }
                }
            }
        },
        Ok(None) => {
            println!("No Stripe customer ID found, creating new customer");
            create_new_customer(&client, user_id, &user.email, &state).await?
        },
        Err(e) => return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)})),
        )),
    };
    
    let amount_dollars = payload.amount_dollars; // From BuyCreditsRequest
    let amount_cents = (amount_dollars * 100.0).round() as i64; // Convert to cents for Stripe

    println!("Processing payment of {} EUR ({} cents)", amount_dollars, amount_cents);

    let domain_url = std::env::var("FRONTEND_URL").expect("FRONTEND_URL not set");
    
    // Create a Checkout Session with payment method attachment
    println!("Creating Stripe checkout session");
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
    println!("Stripe error details: {:?}", e); // Log the full error
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to create Checkout Session: {}", e)})),
        )
    })?;

    println!("Checkout session created successfully");
    // Save the session ID for later use (optional, if you need to track it)
    state
        .user_repository
        .set_stripe_checkout_session_id(user_id, &checkout_session.id.to_string())
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)})),
        ))?;

    println!("Checkout session ID saved to database");
    // Return the Checkout session URL to redirect the user
    Ok(Json(json!({
        "url": checkout_session.url.unwrap(), // Safe to unwrap as it's always present for Checkout
        "message": "Redirecting to Stripe Checkout for payment"
    })))
}


// Helper function to cancel existing subscriptions of a different tier
async fn cancel_existing_subscriptions_of_different_tier(
    client: &Client,
    customer_id: &str,
    new_tier: &str,
) -> Result<(), (StatusCode, Json<Value>)> {
    println!("Checking for existing subscriptions to cancel for customer: {}", customer_id);
    
    // List all active subscriptions for the customer
    let subscriptions = Subscription::list(
        client,
        &ListSubscriptions {
            customer: Some(customer_id.parse().unwrap()),
            status: Some(stripe::SubscriptionStatusFilter::Active),
            ..Default::default()
        },
    )
    .await
    .map_err(|e| {
        println!("Failed to list subscriptions: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to fetch existing subscriptions: {}", e)})),
        )
    })?;

    // Check each subscription and cancel if it's a different tier
    for subscription in subscriptions.data {
        if let Some(price_id) = subscription.items.data.first()
            .and_then(|item| item.price.as_ref())
            .map(|price| price.id.to_string())
        {
            let existing_tier = extract_subscription_info(&price_id).tier;
            
            // If the existing subscription is a different tier, cancel it
            if existing_tier != new_tier {
                println!("Canceling existing {} subscription: {}", existing_tier, subscription.id);
                
                Subscription::update(
                    client,
                    &subscription.id,
                    UpdateSubscription {
                        cancel_at_period_end: Some(true),
                        ..Default::default()
                    },
                )
                .await
                .map_err(|e| {
                    println!("Failed to cancel subscription {}: {}", subscription.id, e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": format!("Failed to cancel existing subscription: {}", e)})),
                    )
                })?;
                
                println!("Successfully scheduled cancellation for subscription: {}", subscription.id);
            } else {
                println!("Existing subscription {} is same tier ({}), not canceling", subscription.id, existing_tier);
            }
        }
    }
    
    Ok(())
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
        println!("Failed to create customer: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to create Stripe customer: {}", e)})),
        )
    })?;

    println!("Created new Stripe customer");
    state
        .user_repository
        .set_stripe_customer_id(user_id, &customer.id.to_string())
        .map_err(|e| {
            println!("Failed to update database with new customer ID: {}", e);
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
    // Default values
    let mut info = SubscriptionInfo {
        country: None,
        tier: "tier 2", // Default to Sentinel tier
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

    // Tier 1 Plans (Hard Mode and Basic Daily)
    for country in ["US", "FI", "UK", "AU", "OTHER"] {
        // Check Hard Mode price IDs (older subscriptions)
        check_price_id!(
            country,
            format!("STRIPE_SUBSCRIPTION_HARD_MODE_PRICE_ID_{}", country),
            "tier 1"
        );
        
        // Check Basic Daily price IDs (older subscriptions)

        check_price_id!(
            country,
            format!("STRIPE_SUBSCRIPTION_BASIC_DAILY_PRICE_ID_{}", country),
            "tier 1"
        );

        // Check Basic price IDs
        check_price_id!(
            country,
            format!("STRIPE_SUBSCRIPTION_BASIC_PRICE_ID_{}", country),
            "tier 1"
        );
    }

    for country in ["US", "FI", "UK", "AU", "OTHER"] {
        // Check World price IDs (older subscriptions)
        check_price_id!(
            country,
            format!("STRIPE_SUBSCRIPTION_WORLD_PRICE_ID_{}", country),
            "tier 2"
        );
        
        // Check Escape Daily price IDs (older subscriptions)

        check_price_id!(
            country,
            format!("STRIPE_SUBSCRIPTION_ESCAPE_DAILY_PRICE_ID_{}", country),
            "tier 2"
        );

        // Check Monitoring price IDs (older subscriptions)
        check_price_id!(
            country,
            format!("STRIPE_SUBSCRIPTION_MONITORING_PRICE_ID_{}", country),
            "tier 2"
        );

        // Check Sentinel price IDs
        check_price_id!(
            country,
            format!("STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_{}", country),
            "tier 2"
        );
    }

    info
}

fn is_sentinel_price_id(price_id: &str) -> bool {
    const COUNTRIES: [&str; 5] = ["US", "FI", "UK", "AU", "OTHER"];

    COUNTRIES.iter().any(|cty| {
        let var_name = format!("STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_{}", cty);
        // If the env-var is missing, simply skip that entry.
        std::env::var(var_name).map_or(false, |v| v == price_id)
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

                let new_sub_info = extract_subscription_info(&base_price);

                // Always check and cancel existing subscriptions when a new one is created
                if event.type_ == stripe::EventType::CustomerSubscriptionCreated {
                    tracing::info!("New subscription created, checking for existing subscriptions to cancel");
                    
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
                let new_sub_info = extract_subscription_info(&base_price);

                if let Ok(Some(user)) = state.user_repository.find_by_stripe_customer_id(&customer_id.as_str()) {
                    let items        = &subscription.items.data;
                    let base_price   = match base_price_id(items) {
                        Some(id) => id,
                        None => {
                            tracing::warn!("subscription without base price – skipping");
                            return Ok(StatusCode::OK);
                        }
                    };
                    let sub_info = extract_subscription_info(&base_price);

                    if let Some(price_id) = subscription.items.data.first()
                        .and_then(|item| item.price.as_ref())
                        .map(|price| price.id.to_string())
                    {

                        // Extract subscription info (both country and tier)
                        let sub_info = extract_subscription_info(&price_id);
                        let is_sentinel_price_id = is_sentinel_price_id(&price_id);
                        let is_digital_detox = is_digital_detox_subscription(&subscription.items.data);
                        
                        // Update subscription country
                        if let Err(e) = state.user_core.update_sub_country(user.id, sub_info.country) {
                            tracing::error!("Failed to update subscription country: {}", e);
                        }

                        // Update subscription tier
                        if let Err(e) = state.user_repository.set_subscription_tier(user.id, Some(sub_info.tier)) {
                            tracing::error!("Failed to update subscription tier: {}", e);
                        }

                        let messages: f32;

                        println!("sub_tier: {}", sub_info.tier);

                        let sentinel_us_id = std::env::var("STRIPE_SUBSCRIPTION_SENTINEL_PRICE_ID_US")
                                .unwrap_or_default();

                        let self_hosting_id = std::env::var("STRIPE_SUBSCRIPTION_SELF_HOSTING_PRICE_ID")
                                .unwrap_or_default();

                        // Calculate days until next billing for the tier 2 sentinel plans
                        let days_until_billing = Some(subscription.current_period_end).map(|date| {
                            let current_time = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs() as i32;
                            (date - current_time as i64) / (24 * 60 * 60)
                        }).unwrap_or(30); // Default to 30 days if we can't calculate

                        // Get user's active digests and count them
                        let amount_of_digests = match state.user_core.get_digests(user.id) {
                            Ok((morning, day, evening)) => {
                                let mut count = 0;
                                if morning.is_some() { count += 1; }
                                if day.is_some() { count += 1; }
                                if evening.is_some() { count += 1; }
                                count as i64
                            },
                            Err(e) => {
                                tracing::error!("Failed to get user digests: {}", e);
                                0 // Default to 0 if there's an error
                            }
                        };
                        
                        if is_digital_detox {
                            messages = 50.0;
                        } else if base_price == sentinel_us_id {
                            messages = 400.00 - (days_until_billing * amount_of_digests) as f32;
                        } else if is_sentinel_price_id {
                            messages = 200.00 - (days_until_billing * amount_of_digests) as f32;
                        } else if base_price == self_hosting_id {
                            // Self-hosting subscription - no messages
                            messages = 0.0;
                            // Update subscription tier to tier 3
                            if let Err(e) = state.user_repository.set_subscription_tier(user.id, Some("tier 3")) {
                                tracing::error!("Failed to update subscription tier: {}", e);
                            }
                            if let Err(e) = state.user_core.generate_pairing_code(user.id) {
                                tracing::error!("Failed to generate pairing code: {}", e);
                            }
                        } else if sub_info.tier == "tier 2" {
                            // legacy sub
                            messages = 120.00 - (days_until_billing * amount_of_digests) as f32;
                            println!("with messages: {}", messages);
                        } else {
                            println!("User subscribed to tier 1");
                            messages = 40.00;
                        }


                        if let Err(e) = state.user_repository.update_sub_credits(user.id, messages) {
                            tracing::error!("Failed to update subscription credits: {}", e);
                        } else {
                            tracing::info!("Set daily credits to 40 for user {}", user.id);
                        }

                        // Update next billing date
                        if let Err(e) = state.user_core.update_next_billing_date(user.id, subscription.current_period_end as i32) {
                            tracing::error!("Failed to update next billing date: {}", e);
                        } else {
                            tracing::info!("Updated next billing date for user {}: {}", user.id, subscription.current_period_end);
                        }

                        tracing::info!("Updated subscription info for user {}: country={:#?}, tier={}", 
                            user.id, sub_info.country, sub_info.tier);
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

                    // Get the tier of the subscription being deleted
                    let deleted_tier = subscription.items.data.first()
                        .and_then(|item| item.price.as_ref())
                        .map(|price| extract_subscription_info(&price.id).tier);

                    // Only update subscription info if the deleted subscription's tier matches the current tier
                    let current_tier = state.user_repository.get_subscription_tier(user.id)
                        .map_err(|e| {
                            tracing::error!("Failed to get current subscription tier: {}", e);
                            (
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(json!({"error": "Failed to get current subscription tier"})),
                            )
                        })?;

                    if deleted_tier == current_tier.as_deref() {
                        if active_subscriptions.data.is_empty() {
                            // No active subscriptions left, clear subscription tier and country
                            tracing::info!("No active subscriptions remaining, clearing subscription info");
                            if let Err(e) = state.user_repository.set_subscription_tier(user.id, None) {
                                tracing::error!("Failed to clear subscription tier: {}", e);
                            }
                            if let Err(e) = state.user_core.update_sub_country(user.id, None) {
                                tracing::error!("Failed to clear subscription country: {}", e);
                            }
                        } else {
                            // Find the highest tier among remaining subscriptions
                            let highest_tier = active_subscriptions.data.iter()
                                .filter_map(|sub| {
                                    sub.items.data.first()
                                        .and_then(|item| item.price.as_ref())
                                        .map(|price| extract_subscription_info(&price.id))
                                })
                                .max_by(|a, b| {
                                    // Compare tiers (tier 2 > tier 1.5 > tier 1)
                                    match (a.tier, b.tier) {
                                        ("tier 2", _) => std::cmp::Ordering::Greater,
                                        (_, "tier 2") => std::cmp::Ordering::Less,
                                        ("tier 1.5", "tier 1") => std::cmp::Ordering::Greater,
                                        ("tier 1", "tier 1.5") => std::cmp::Ordering::Less,
                                        _ => std::cmp::Ordering::Equal,
                                    }
                                });

                            if let Some(tier_info) = highest_tier {
                                tracing::info!("Updating subscription tier to {} based on remaining subscriptions", tier_info.tier);
                                if let Err(e) = state.user_repository.set_subscription_tier(user.id, Some(tier_info.tier)) {
                                    tracing::error!("Failed to update subscription tier: {}", e);
                                }
                                if let Err(e) = state.user_core.update_sub_country(user.id, tier_info.country) {
                                    tracing::error!("Failed to update subscription country: {}", e);
                                }
                            }
                        }
                    } else {
                        tracing::info!("Deleted subscription tier ({:?}) doesn't match current tier ({:?}), skipping update", deleted_tier, current_tier);
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
                                    eprintln!("Failed to update customer address: {}", e);
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
    println!("Starting fetch_next_billing_date for user_id: {}", user_id);

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
    println!("Stripe client initialized");

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
    println!("Found Stripe customer ID: {}", customer_id);

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
        println!("Failed to list subscriptions: {}", e);
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

    println!("Successfully updated next billing date to {}", latest_end_date);

    Ok(Json(json!({
        "message": "Successfully updated next billing date",
        "next_billing_date": latest_end_date
    })))
}

pub async fn automatic_charge(
    State(state): State<Arc<AppState>>,
    Path(user_id): Path<i32>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    println!("Starting automatic_charge for user_id: {}", user_id);
    // Initialize Stripe client
    let stripe_secret_key = std::env::var("STRIPE_SECRET_KEY")
        .expect("STRIPE_SECRET_KEY must be set in environment");
    let client = Client::new(stripe_secret_key);
    println!("Stripe client initialized");

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
    println!("User found: {}", user.id);

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
    println!("Stripe payment method ID found");

    let charge_back_to = user.charge_back_to.unwrap_or(5.00);
    println!("User charge_back_to: {}, current credits: {}", charge_back_to, user.credits);
    
    let charge_amount_cents = (charge_back_to * 100.0).round() as i64; // Convert to cents for Stripe
    println!("Charging credits (€{})", charge_back_to);

    // Create a PaymentIntent for the off-session charge
    println!("Creating payment intent");
    let mut create_intent = CreatePaymentIntent::new(charge_amount_cents, stripe::Currency::EUR);
    create_intent.customer = Some(customer_id.parse().unwrap());
    create_intent.payment_method = Some(payment_method_id.parse().unwrap());
    create_intent.confirm = Some(true); // Confirm the payment immediately
    create_intent.off_session = Some(stripe::PaymentIntentOffSession::Exists(true)); // Off-session payment
    create_intent.payment_method_types = Some(vec!["card".to_string()]); // Card payment method

    let payment_intent = PaymentIntent::create(&client, create_intent)
        .await
        .map_err(|e| {
            println!("Failed to create PaymentIntent: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Failed to create PaymentIntent: {}", e)})),
            )
        })?;
    
    println!("Payment intent created with status: {:?}", payment_intent.status);
    
    // Check if the payment was successful
    if payment_intent.status == stripe::PaymentIntentStatus::Succeeded {
        println!("Payment succeeded, updating user credits");
        // Update user's credits 
        state
            .user_repository
            .increase_credits(user_id, charge_back_to) 
            .map_err(|e| {
                println!("Failed to update user credits: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Database error updating credits: {}", e)})),
                )
            })?;

        println!("User credits updated successfully, returning success response");
        Ok(Json(json!({
            "message": "Automatic charge successful, credits updated",
            "amount": charge_back_to,
        })))
    } else {
        println!("Payment intent failed or requires action, status: {:?}", payment_intent.status);
        Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Payment intent failed or requires action"})),
        ))
    }
}

