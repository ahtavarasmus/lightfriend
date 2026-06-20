use crate::UserCoreOps;
use stripe::{
    BillingPortalSession, CheckoutSession, Client, CreateBillingPortalSession,
    CreateCheckoutSession, CreateCustomer, CreatePaymentIntent, Customer, PaymentIntent, Price,
    Subscription,
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
    /// "assistant" or "autopilot"
    pub plan_type: Option<String>,
}

#[derive(Serialize)]
pub struct SubscriptionMigrationStatusResponse {
    pub show_current_plans: bool,
    pub has_canceling_subscription: bool,
    pub has_legacy_subscription: bool,
}

fn checkout_price_for_plan(plan_type: Option<&str>) -> Result<String, (StatusCode, Json<Value>)> {
    let env_keys = match plan_type {
        Some("assistant") => [
            "STRIPE_ASSISTANT_CHECKOUT_PRICE_ID",
            "STRIPE_ASSISTANT_PLAN_PRICE_ID",
        ],
        Some("autopilot") | None => [
            "STRIPE_AUTOPILOT_CHECKOUT_PRICE_ID",
            "STRIPE_AUTOPILOT_PLAN_PRICE_ID",
        ],
        Some("byot") => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "BYOT is no longer a subscription plan"})),
            ))
        }
        Some(other) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": format!("Unknown subscription plan: {}", other)})),
            ))
        }
    };

    env_keys
        .iter()
        .find_map(|env_key| {
            std::env::var(env_key)
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .ok_or_else(|| {
            tracing::error!(
                "Neither {} nor {} found in environment",
                env_keys[0],
                env_keys[1]
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "This plan is not configured yet. Please contact support."})),
            )
        })
}

fn pricing_table_config() -> Result<(String, String), (StatusCode, Json<Value>)> {
    let pricing_table_id = std::env::var("STRIPE_PRICING_TABLE_ID").map_err(|_| {
        tracing::error!("STRIPE_PRICING_TABLE_ID not found in environment");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Stripe pricing table is not configured"})),
        )
    })?;
    let publishable_key = std::env::var("STRIPE_PUBLISHABLE_KEY").map_err(|_| {
        tracing::error!("STRIPE_PUBLISHABLE_KEY not found in environment");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Stripe publishable key is not configured"})),
        )
    })?;
    Ok((pricing_table_id, publishable_key))
}

async fn retrieve_price_currency(client: &Client, price_id: &str) -> Option<stripe::Currency> {
    let price_id = match price_id.parse() {
        Ok(price_id) => price_id,
        Err(_) => {
            tracing::error!("Invalid Stripe price ID configured: {}", price_id);
            return None;
        }
    };

    match Price::retrieve(client, &price_id, &[]).await {
        Ok(price) => price.currency,
        Err(e) => {
            tracing::error!("Failed to retrieve Stripe price {}: {}", price_id, e);
            None
        }
    }
}

/// Idempotent subscription entitlement setup. Safe to call multiple times.
/// Stripe webhooks update access and Stripe renewal metadata only; Lightfriend
/// grants included usage through its own monthly usage windows.
async fn setup_user_subscription(
    state: &Arc<AppState>,
    user_id: i32,
    product_id: &str,
    subscription_created: i64,
    current_period_end: i64,
    _phone_country: Option<&str>,
) -> Result<(), String> {
    // Set plan_type based on product ID. Unknown legacy products only map to
    // Autopilot while their pre-cutoff Stripe subscription remains active.
    let plan_type =
        crate::utils::country::plan_type_from_product(product_id, Some(subscription_created))
            .ok_or_else(|| {
                format!(
                    "Unknown Stripe product {} for subscription created at {}",
                    product_id, subscription_created
                )
            })?;

    // All subscription products map to tier 2 after the product has been
    // validated.
    if let Err(e) = state
        .user_repository
        .set_subscription_tier(user_id, Some("tier 2"))
    {
        return Err(format!(
            "Failed to set subscription tier for user {}: {}",
            user_id, e
        ));
    }

    if let Err(e) = state
        .user_repository
        .update_plan_type(user_id, Some(plan_type))
    {
        return Err(format!(
            "Failed to set plan_type for user {}: {}",
            user_id, e
        ));
    }

    // Store Stripe's next billing date for billing display/portal context. This
    // can be monthly or yearly and is intentionally separate from the internal
    // included-usage reset window.
    if let Err(e) = state
        .user_core
        .update_next_billing_date(user_id, current_period_end as i32)
    {
        return Err(format!(
            "Failed to update next billing date for user {}: {}",
            user_id, e
        ));
    } else {
        tracing::info!(
            "Updated next billing date for user {}: {}",
            user_id,
            current_period_end
        );
    }

    tracing::info!(
        "Setup subscription entitlement for user {}: plan={}, product={}",
        user_id,
        plan_type,
        product_id
    );

    Ok(())
}

async fn find_or_create_user_for_stripe_customer(
    state: &Arc<AppState>,
    client: &Client,
    customer_id: &stripe::CustomerId,
    email_hint: Option<String>,
    phone_hint: Option<String>,
) -> Result<i32, (StatusCode, Json<Value>)> {
    match state
        .user_repository
        .find_by_stripe_customer_id(customer_id.as_ref())
    {
        Ok(Some(user)) => return Ok(user.id),
        Ok(None) => {}
        Err(e) => {
            tracing::error!("Error finding user by customer ID: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Database error"})),
            ));
        }
    }

    tracing::info!(
        "No user found for customer {}, creating from Stripe customer data",
        customer_id
    );

    let (email, phone) = match (email_hint, phone_hint) {
        (Some(email), phone) if !email.trim().is_empty() => (email, phone.unwrap_or_default()),
        _ => {
            let retrieve_result = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                Customer::retrieve(client, customer_id, &[]),
            )
            .await
            .map_err(|_| {
                tracing::error!("Timed out retrieving Stripe customer {}", customer_id);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Timed out retrieving customer"})),
                )
            })?
            .map_err(|e| {
                tracing::error!("Failed to retrieve Stripe customer: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Failed to retrieve customer"})),
                )
            })?;

            (
                retrieve_result.email.clone().unwrap_or_default(),
                retrieve_result.phone.clone().unwrap_or_default(),
            )
        }
    };

    use crate::repositories::signup_repository_impl::CompositeSignupRepository;
    use crate::services::signup_service::{SignupError, SignupResult, SignupService};

    let signup_repo = std::sync::Arc::new(CompositeSignupRepository::new(
        state.user_core.clone(),
        state.user_repository.clone(),
    ));
    let signup_service = SignupService::new(signup_repo);

    match signup_service.handle_new_subscription(&email, &phone, customer_id.as_ref()) {
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
                        crate::utils::email::send_subscription_activated_email(&email_clone).await
                    {
                        tracing::error!("Failed to send subscription activated email: {}", e);
                    }
                });
            }

            Ok(user_id)
        }
        Ok(SignupResult::NewUserCreated {
            user_id,
            magic_token,
            email,
            phone_skipped,
        }) => {
            tracing::info!(
                "Created new user {} from guest checkout (phone_skipped: {})",
                user_id,
                phone_skipped
            );

            let frontend_url = std::env::var("FRONTEND_URL").unwrap_or_default();
            let magic_link = format!("{}/set-password/{}", frontend_url, magic_token);

            tokio::spawn(async move {
                if let Err(e) = crate::utils::email::send_magic_link_email_with_options(
                    &email,
                    &magic_link,
                    phone_skipped,
                )
                .await
                {
                    tracing::error!("Failed to send magic link email: {}", e);
                }
            });

            Ok(user_id)
        }
        Err(SignupError::EmptyEmail) => {
            tracing::error!("No email found for Stripe customer {}", customer_id);
            Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Customer has no email"})),
            ))
        }
        Err(e) => {
            tracing::error!("Signup error: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to create user"})),
            ))
        }
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
    let client = Client::new(stripe_secret_key.clone());
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
            customer: Some(customer_id.parse().map_err(|_| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "Invalid customer ID"})),
                )
            })?),
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
    let base_price_id = match body.subscription_type {
        SubscriptionType::Hosted => checkout_price_for_plan(body.plan_type.as_deref())?,
    };

    let mut checkout_customer_id = customer_id.clone();
    if let Some(current_subscription) = existing_subscription.data.first() {
        if let Some(new_price_currency) = retrieve_price_currency(&client, &base_price_id).await {
            if new_price_currency != current_subscription.currency {
                tracing::info!(
                    "Creating checkout on a temporary customer for cross-currency migration: current_subscription={}, current_currency={:?}, new_currency={:?}",
                    current_subscription.id,
                    current_subscription.currency,
                    new_price_currency
                );
                checkout_customer_id =
                    create_temporary_checkout_customer(&client, user_id, &user.email).await?;
            }
        }
    }

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
        customer: Some(checkout_customer_id.parse().map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Invalid customer ID"})),
            )
        })?),
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
        metadata.insert("replacing_customer".to_string(), customer_id.clone());
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
    /// "assistant" or "autopilot"
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
    let client = Client::new(stripe_secret_key.clone());

    let domain_url = std::env::var("FRONTEND_URL").expect("FRONTEND_URL not set");
    // Select price ID based on plan choice. Prices are single-currency USD.
    let base_price_id = match body.subscription_type {
        SubscriptionType::Hosted => checkout_price_for_plan(body.plan_type.as_deref())?,
    };

    // Build line items
    let line_items = vec![stripe::CreateCheckoutSessionLineItems {
        price: Some(base_price_id),
        quantity: Some(1),
        ..Default::default()
    }];

    // Success URL redirects to password setup page with session_id
    let success_url = format!("{}/subscription-success", domain_url);
    let cancel_url = format!("{}/?checkout=canceled#plans", domain_url);

    // Build metadata
    let mut metadata = std::collections::HashMap::new();
    metadata.insert(
        "selected_country".to_string(),
        body.selected_country.clone(),
    );
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

pub async fn get_pricing_table_config() -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let (pricing_table_id, publishable_key) = pricing_table_config()?;
    Ok(Json(json!({
        "pricing_table_id": pricing_table_id,
        "publishable_key": publishable_key,
    })))
}

pub async fn create_pricing_table_customer_session(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(user_id): Path<i32>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if auth_user.user_id != user_id {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Access denied"})),
        ));
    }

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
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "User not found"})),
            )
        })?;

    let stripe_secret_key = std::env::var("STRIPE_SECRET_KEY").map_err(|_| {
        tracing::error!("STRIPE_SECRET_KEY not found in environment");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Stripe configuration error"})),
        )
    })?;
    let client = Client::new(stripe_secret_key.clone());

    let customer_id = match state.user_repository.get_stripe_customer_id(user_id) {
        Ok(Some(customer_id)) => customer_id,
        Ok(None) => create_new_customer(&client, user_id, &user.email, &state).await?,
        Err(e) => {
            tracing::error!("Database error getting Stripe customer ID: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            ));
        }
    };

    let customer_session_response = reqwest::Client::new()
        .post("https://api.stripe.com/v1/customer_sessions")
        .bearer_auth(&stripe_secret_key)
        .form(&[
            ("customer", customer_id.as_str()),
            ("components[pricing_table][enabled]", "true"),
        ])
        .send()
        .await
        .map_err(|e| {
            tracing::error!("Failed to create Stripe customer session: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to create Stripe customer session"})),
            )
        })?;
    if !customer_session_response.status().is_success() {
        let status = customer_session_response.status();
        let body = customer_session_response.text().await.unwrap_or_default();
        tracing::error!(
            "Stripe customer session request failed: status={}, body={}",
            status,
            body
        );
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to create Stripe customer session"})),
        ));
    }
    let customer_session: Value = customer_session_response.json().await.map_err(|e| {
        tracing::error!("Failed to parse Stripe customer session response: {:?}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to read Stripe customer session"})),
        )
    })?;
    let client_secret = customer_session
        .get("client_secret")
        .and_then(|value| value.as_str())
        .ok_or_else(|| {
            tracing::error!("Stripe customer session response had no client_secret");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Stripe customer session was incomplete"})),
            )
        })?;

    let (pricing_table_id, publishable_key) = pricing_table_config()?;
    Ok(Json(json!({
        "pricing_table_id": pricing_table_id,
        "publishable_key": publishable_key,
        "customer_session_client_secret": client_secret,
    })))
}

pub async fn get_subscription_migration_status(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(user_id): Path<i32>,
) -> Result<Json<SubscriptionMigrationStatusResponse>, (StatusCode, Json<Value>)> {
    if auth_user.user_id != user_id && !auth_user.is_admin {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Access denied"})),
        ));
    }

    let customer_id = match state.user_repository.get_stripe_customer_id(user_id) {
        Ok(Some(customer_id)) => customer_id,
        Ok(None) => {
            return Ok(Json(SubscriptionMigrationStatusResponse {
                show_current_plans: false,
                has_canceling_subscription: false,
                has_legacy_subscription: false,
            }));
        }
        Err(e) => {
            tracing::error!("Database error getting Stripe customer ID: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": format!("Database error: {}", e)})),
            ));
        }
    };

    let stripe_secret_key = std::env::var("STRIPE_SECRET_KEY").map_err(|_| {
        tracing::error!("STRIPE_SECRET_KEY not found in environment");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Stripe configuration error"})),
        )
    })?;
    let client = Client::new(stripe_secret_key);
    let customer = customer_id.parse().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid customer ID"})),
        )
    })?;

    let subscriptions = Subscription::list(
        &client,
        &stripe::ListSubscriptions {
            customer: Some(customer),
            status: Some(stripe::SubscriptionStatusFilter::All),
            limit: Some(20),
            ..Default::default()
        },
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to list subscriptions for migration status: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to check subscription status"})),
        )
    })?;

    let mut has_canceling_subscription = false;
    let mut has_legacy_subscription = false;

    for subscription in subscriptions.data.iter().filter(|subscription| {
        matches!(
            subscription.status,
            stripe::SubscriptionStatus::Active | stripe::SubscriptionStatus::Trialing
        )
    }) {
        if subscription.cancel_at_period_end {
            has_canceling_subscription = true;
        }

        let product_kind = subscription
            .items
            .data
            .first()
            .and_then(|item| item.price.as_ref())
            .and_then(|price| price.product.as_ref())
            .map(|product| match product {
                stripe::Expandable::Id(id) => crate::utils::country::stripe_product_kind(id),
                stripe::Expandable::Object(product) => {
                    crate::utils::country::stripe_product_kind(product.id.as_ref())
                }
            })
            .unwrap_or(stripe_webhook_logic::ProductKind::Unknown);

        let is_current_plan = matches!(
            product_kind,
            stripe_webhook_logic::ProductKind::Assistant
                | stripe_webhook_logic::ProductKind::Autopilot
        );
        let is_credit_add_on = product_kind == stripe_webhook_logic::ProductKind::CreditsAddOn;
        if !is_current_plan && !is_credit_add_on {
            has_legacy_subscription = true;
        }
    }

    Ok(Json(SubscriptionMigrationStatusResponse {
        show_current_plans: has_canceling_subscription || has_legacy_subscription,
        has_canceling_subscription,
        has_legacy_subscription,
    }))
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
    let client = Client::new(stripe_secret_key.clone());
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
    let customer = customer_id.parse().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid customer ID"})),
        )
    })?;
    let mut create_session = CreateBillingPortalSession::new(customer);
    // Store the formatted URL in a variable first
    let return_url = format!(
        "{}/billing",
        std::env::var("FRONTEND_URL").expect("FRONTEND_URL not set")
    );
    create_session.return_url = Some(&return_url);
    let portal_configuration = std::env::var("STRIPE_CUSTOMER_PORTAL_CONFIG_ID")
        .ok()
        .filter(|value| !value.trim().is_empty());
    if let Some(ref configuration_id) = portal_configuration {
        create_session.configuration = Some(configuration_id);
    }
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
    let detected_country = crate::utils::country::get_country_code_from_phone(&user.phone_number);
    let is_us_ca = matches!(detected_country.as_deref(), Some("US") | Some("CA"));

    if !is_us_ca {
        // Only hosted-credit plans (assistant, autopilot) can buy more credits; BYOT cannot
        if !crate::utils::plan_features::uses_hosted_credits(user.plan_type.as_deref()) {
            tracing::info!(
                "User {} attempted to buy credits but plan_type={:?} does not use hosted credits",
                user_id,
                user.plan_type
            );
            return Err((
                StatusCode::FORBIDDEN,
                Json(json!({
                    "error": "Credit top-ups are not available on your current plan",
                    "upgrade_required": true,
                    "message": "Subscribe to a hosted plan to purchase additional credits."
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

async fn create_temporary_checkout_customer(
    client: &Client,
    user_id: i32,
    email: &str,
) -> Result<String, (StatusCode, Json<Value>)> {
    let customer = Customer::create(
        client,
        CreateCustomer {
            email: Some(email),
            name: Some(&format!("User {}", user_id)),
            address: None,
            ..Default::default()
        },
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to create temporary checkout customer: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to create Stripe customer: {}", e)})),
        )
    })?;

    tracing::info!(
        "Created temporary Stripe customer {} for checkout",
        customer.id
    );
    Ok(customer.id.to_string())
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
    let client = Client::new(stripe_secret_key.clone());
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

                // Get price and product ID from subscription
                let price = subscription
                    .items
                    .data
                    .first()
                    .and_then(|item| item.price.as_ref())
                    .ok_or_else(|| {
                        tracing::error!("No price found in subscription items");
                        (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({"error": "Invalid subscription data"})),
                        )
                    })?;

                let price_id = price.id.to_string();
                let product_id = match &price.product {
                    Some(stripe::Expandable::Id(id)) => id.to_string(),
                    Some(stripe::Expandable::Object(product)) => product.id.to_string(),
                    None => String::new(),
                };
                tracing::info!(
                    "Subscription price_id={}, product_id={}",
                    price_id,
                    product_id
                );

                // Skip subscription updates that are part of a plan change or being cancelled
                let upsert_event = match event.type_ {
                    stripe::EventType::CustomerSubscriptionCreated => {
                        stripe_webhook_logic::SubscriptionUpsertEvent::Created
                    }
                    _ => stripe_webhook_logic::SubscriptionUpsertEvent::Updated,
                };
                let is_plan_change = subscription
                    .metadata
                    .get("plan_change")
                    .map(|val| val == "true")
                    .unwrap_or(false);
                let subscription_status = match subscription.status {
                    stripe::SubscriptionStatus::Active => {
                        stripe_webhook_logic::SubscriptionStatus::Active
                    }
                    stripe::SubscriptionStatus::Trialing => {
                        stripe_webhook_logic::SubscriptionStatus::Trialing
                    }
                    stripe::SubscriptionStatus::Canceled => {
                        stripe_webhook_logic::SubscriptionStatus::Canceled
                    }
                    stripe::SubscriptionStatus::Incomplete => {
                        stripe_webhook_logic::SubscriptionStatus::Incomplete
                    }
                    stripe::SubscriptionStatus::IncompleteExpired => {
                        stripe_webhook_logic::SubscriptionStatus::IncompleteExpired
                    }
                    stripe::SubscriptionStatus::PastDue => {
                        stripe_webhook_logic::SubscriptionStatus::PastDue
                    }
                    stripe::SubscriptionStatus::Paused => {
                        stripe_webhook_logic::SubscriptionStatus::Paused
                    }
                    stripe::SubscriptionStatus::Unpaid => {
                        stripe_webhook_logic::SubscriptionStatus::Unpaid
                    }
                };
                match stripe_webhook_logic::decide_subscription_upsert(
                    upsert_event,
                    subscription_status,
                    is_plan_change,
                    subscription.cancel_at_period_end,
                ) {
                    stripe_webhook_logic::SubscriptionUpsertDecision::ApplySubscription => {}
                    stripe_webhook_logic::SubscriptionUpsertDecision::IgnoreInactiveSubscription => {
                        tracing::info!(
                            "Skipping inactive subscription {} with status {}",
                            subscription.id,
                            subscription.status.as_str()
                        );
                        return Ok(StatusCode::OK);
                    }
                    stripe_webhook_logic::SubscriptionUpsertDecision::IgnorePlanChangeUpdate => {
                        tracing::info!(
                            "Skipping subscription update as it's part of a plan change"
                        );
                        return Ok(StatusCode::OK);
                    }
                    stripe_webhook_logic::SubscriptionUpsertDecision::IgnoreCancelAtPeriodEndUpdate => {
                        tracing::info!(
                            "Skipping subscription update for subscription being cancelled: {}",
                            subscription.id
                        );
                        return Ok(StatusCode::OK);
                    }
                }

                // Cancel existing subscriptions when a new one is created.
                // This is cleanup, not required to grant access, so keep it
                // out of Stripe's webhook response path.
                if event.type_ == stripe::EventType::CustomerSubscriptionCreated {
                    let client_bg = client.clone();
                    let customer_id_bg = customer_id.clone();
                    let current_subscription_id = subscription.id.clone();
                    let replacing_subscription_id =
                        subscription.metadata.get("replacing_subscription").cloned();
                    let stripe_secret_key_bg = stripe_secret_key.clone();
                    tokio::spawn(async move {
                        let list_result = tokio::time::timeout(
                            std::time::Duration::from_secs(10),
                            stripe::Subscription::list(
                                &client_bg,
                                &stripe::ListSubscriptions {
                                    customer: Some(customer_id_bg),
                                    status: Some(stripe::SubscriptionStatusFilter::Active),
                                    ..Default::default()
                                },
                            ),
                        )
                        .await;

                        let existing_subscriptions = match list_result {
                            Ok(Ok(subscriptions)) => subscriptions,
                            Ok(Err(e)) => {
                                tracing::error!("Failed to list existing subscriptions: {}", e);
                                return;
                            }
                            Err(_) => {
                                tracing::error!("Timed out listing existing subscriptions");
                                return;
                            }
                        };

                        let current_subscription_id_string = current_subscription_id.to_string();
                        let mut subscriptions_to_cancel: Vec<String> = existing_subscriptions
                            .data
                            .iter()
                            .filter(|s| s.id != current_subscription_id)
                            .map(|s| s.id.to_string())
                            .collect();

                        if let Some(replacing_subscription_id) = replacing_subscription_id {
                            if replacing_subscription_id != current_subscription_id_string
                                && !subscriptions_to_cancel
                                    .iter()
                                    .any(|id| id == &replacing_subscription_id)
                            {
                                subscriptions_to_cancel.push(replacing_subscription_id);
                            }
                        }

                        let http_client = reqwest::Client::new();
                        for sub_id_string in subscriptions_to_cancel {
                            tracing::info!(
                                "Canceling existing subscription immediately: {}",
                                sub_id_string
                            );
                            let cancel_url = format!(
                                "https://api.stripe.com/v1/subscriptions/{}",
                                sub_id_string
                            );

                            if let Err(e) = http_client
                                .post(&cancel_url)
                                .bearer_auth(&stripe_secret_key_bg)
                                .form(&[("metadata[plan_change]", "true")])
                                .send()
                                .await
                            {
                                tracing::error!(
                                    "Failed to mark subscription {} as plan_change before canceling: {:?}",
                                    sub_id_string,
                                    e
                                );
                            }

                            match http_client
                                .delete(&cancel_url)
                                .bearer_auth(&stripe_secret_key_bg)
                                .form(&[("invoice_now", "false"), ("prorate", "false")])
                                .send()
                                .await
                            {
                                Ok(response) if response.status().is_success() => {
                                    tracing::info!(
                                        "Canceled replaced subscription {} immediately",
                                        sub_id_string
                                    );
                                }
                                Ok(response) => {
                                    let status = response.status();
                                    let body = response.text().await.unwrap_or_default();
                                    tracing::error!(
                                        "Failed to cancel replaced subscription {}: status={}, body={}",
                                        sub_id_string,
                                        status,
                                        body
                                    );
                                }
                                Err(e) => {
                                    tracing::error!(
                                        "Failed to cancel replaced subscription {}: {:?}",
                                        sub_id_string,
                                        e
                                    );
                                }
                            }
                        }
                    });
                }

                // Find or create user (only on Created event, not Updated)
                let user_id: i32 = match state
                    .user_repository
                    .find_by_stripe_customer_id(customer_id.as_str())
                {
                    Ok(Some(user)) => user.id,
                    Ok(None) if event.type_ == stripe::EventType::CustomerSubscriptionCreated => {
                        find_or_create_user_for_stripe_customer(
                            &state,
                            &client,
                            &customer_id,
                            None,
                            None,
                        )
                        .await?
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

                let phone_country =
                    crate::utils::country::get_country_code_from_phone(&user.phone_number);

                // Update subscription country from user's phone number
                if let Err(e) = state
                    .user_core
                    .update_sub_country(user.id, phone_country.as_deref())
                {
                    tracing::error!("Failed to update subscription country: {}", e);
                }
                if let Err(e) = setup_user_subscription(
                    &state,
                    user.id,
                    &product_id,
                    subscription.created,
                    subscription.current_period_end,
                    phone_country.as_deref(),
                )
                .await
                {
                    tracing::error!("Failed to setup subscription: {}", e);
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"error": "Failed to setup subscription"})),
                    ));
                }

                // Set preferred Lightfriend number for hosted phone mode.
                if !user.own_twilio_enabled && user.preferred_number.is_none() {
                    if let Some(ref country) = phone_country {
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

                let is_subscription_change = subscription
                    .metadata
                    .get("plan_change")
                    .map(|val| val == "true")
                    .unwrap_or(false);

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
                    let active_subscription_snapshots: Vec<_> = active_subscriptions
                        .data
                        .iter()
                        .map(|active_subscription| {
                            let product_kind = active_subscription
                                .items
                                .data
                                .first()
                                .and_then(|item| item.price.as_ref())
                                .and_then(|price| price.product.as_ref())
                                .map(|product| match product {
                                    stripe::Expandable::Id(id) => {
                                        crate::utils::country::stripe_product_kind(id)
                                    }
                                    stripe::Expandable::Object(product) => {
                                        crate::utils::country::stripe_product_kind(
                                            product.id.as_ref(),
                                        )
                                    }
                                })
                                .unwrap_or(stripe_webhook_logic::ProductKind::Unknown);

                            stripe_webhook_logic::ActiveSubscriptionSnapshot {
                                is_deleted_subscription: active_subscription.id == subscription.id,
                                product_kind,
                                subscription_age:
                                    crate::utils::country::subscription_age_for_legacy_cutoff(Some(
                                        active_subscription.created,
                                    )),
                            }
                        })
                        .collect();

                    match stripe_webhook_logic::decide_subscription_delete(
                        is_subscription_change,
                        &active_subscription_snapshots,
                    ) {
                        stripe_webhook_logic::SubscriptionDeletedDecision::IgnorePlanChangeDelete => {
                            tracing::info!(
                                "Subscription deletion is part of a plan change, skipping tier update"
                            );
                            return Ok(StatusCode::OK);
                        }
                        stripe_webhook_logic::SubscriptionDeletedDecision::ClearSubscription => {
                        // No active subscriptions left, clear entitlement,
                        // included usage, and Stripe billing date.
                        tracing::info!(
                            "No active plan subscriptions remaining, clearing subscription info"
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
                        if let Err(e) = state.user_repository.clear_included_usage_window(user.id) {
                            tracing::error!("Failed to clear included usage window: {}", e);
                        }
                        // Clear next billing date
                        if let Err(e) = state.user_core.update_next_billing_date(user.id, 0) {
                            tracing::error!("Failed to clear next billing date: {}", e);
                        }
                        }
                        stripe_webhook_logic::SubscriptionDeletedDecision::KeepPlan(remaining_plan) => {
                            // Always tier 2 for any active subscription
                            tracing::info!("User still has active subscription, keeping tier 2");
                            if let Err(e) = state
                                .user_repository
                                .set_subscription_tier(user.id, Some("tier 2"))
                            {
                                tracing::error!("Failed to update subscription tier: {}", e);
                            }
                            if let Err(e) = state
                                .user_repository
                                .update_plan_type(user.id, Some(remaining_plan.as_str()))
                            {
                                tracing::error!("Failed to update plan_type: {}", e);
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
                    // This handler also acts as an idempotent fallback for
                    // Pricing Table guest checkouts because it includes the
                    // customer email/phone that we need to create the account.
                    if matches!(session.mode, stripe::CheckoutSessionMode::Subscription) {
                        let customer_id = session
                            .customer
                            .as_ref()
                            .map(|customer| customer.id())
                            .ok_or_else(|| {
                            tracing::error!("Subscription checkout session had no customer");
                            (
                                StatusCode::BAD_REQUEST,
                                Json(json!({"error": "Checkout session has no customer"})),
                            )
                        })?;

                        let subscription = match session.subscription {
                            Some(stripe::Expandable::Object(subscription)) => *subscription,
                            Some(stripe::Expandable::Id(subscription_id)) => tokio::time::timeout(
                                std::time::Duration::from_secs(10),
                                Subscription::retrieve(&client, &subscription_id, &[]),
                            )
                            .await
                            .map_err(|_| {
                                tracing::error!(
                                    "Timed out retrieving subscription {}",
                                    subscription_id
                                );
                                (
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    Json(json!({"error": "Timed out retrieving subscription"})),
                                )
                            })?
                            .map_err(|e| {
                                tracing::error!(
                                    "Failed to retrieve subscription {}: {}",
                                    subscription_id,
                                    e
                                );
                                (
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    Json(json!({"error": "Failed to retrieve subscription"})),
                                )
                            })?,
                            None => {
                                tracing::error!(
                                    "Subscription checkout session had no subscription"
                                );
                                return Err((
                                    StatusCode::BAD_REQUEST,
                                    Json(json!({"error": "Checkout session has no subscription"})),
                                ));
                            }
                        };

                        let price = subscription
                            .items
                            .data
                            .first()
                            .and_then(|item| item.price.as_ref())
                            .ok_or_else(|| {
                                tracing::error!(
                                    "No price found in subscription {}",
                                    subscription.id
                                );
                                (
                                    StatusCode::BAD_REQUEST,
                                    Json(json!({"error": "Subscription has no price"})),
                                )
                            })?;
                        let product_id = match price.product.as_ref().ok_or_else(|| {
                            tracing::error!("No product found in subscription price {}", price.id);
                            (
                                StatusCode::BAD_REQUEST,
                                Json(json!({"error": "Subscription price has no product"})),
                            )
                        })? {
                            stripe::Expandable::Id(id) => id.to_string(),
                            stripe::Expandable::Object(product) => product.id.to_string(),
                        };

                        let (email_hint, phone_hint) = session
                            .customer_details
                            .as_ref()
                            .map(|details| (details.email.clone(), details.phone.clone()))
                            .unwrap_or((None, None));

                        let user_id = find_or_create_user_for_stripe_customer(
                            &state,
                            &client,
                            &customer_id,
                            email_hint,
                            phone_hint,
                        )
                        .await?;

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
                        let phone_country =
                            crate::utils::country::get_country_code_from_phone(&user.phone_number);

                        if let Err(e) = state
                            .user_core
                            .update_sub_country(user.id, phone_country.as_deref())
                        {
                            tracing::error!("Failed to update subscription country: {}", e);
                        }
                        if let Err(e) = setup_user_subscription(
                            &state,
                            user.id,
                            &product_id,
                            subscription.created,
                            subscription.current_period_end,
                            phone_country.as_deref(),
                        )
                        .await
                        {
                            tracing::error!("Failed to setup subscription: {}", e);
                            return Err((
                                StatusCode::INTERNAL_SERVER_ERROR,
                                Json(json!({"error": "Failed to setup subscription"})),
                            ));
                        }

                        if !user.own_twilio_enabled && user.preferred_number.is_none() {
                            if let Some(ref country) = phone_country {
                                let _ = state
                                    .user_core
                                    .set_preferred_number_for_country(user.id, country);
                            }
                        }

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
