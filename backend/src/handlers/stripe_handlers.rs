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
};
use crate::handlers::auth_dtos::Claims;
use jsonwebtoken::{DecodingKey, Validation, Algorithm, decode};
use serde::{Deserialize, Serialize};

use axum::{
    extract::{State, Path},
    http::{StatusCode, HeaderMap},
    response::IntoResponse,
    body::Bytes,
    Json,
};
use crate::AppState;
use serde_json::{json, Value};
use std::sync::Arc;

// Assuming BuyCreditsRequest is defined in billing_models.rs
#[derive(Deserialize, Serialize, Clone, PartialEq)]
pub struct BuyCreditsRequest {
    pub amount_dollars: f32,
}

pub async fn create_customer_portal_session(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(user_id): Path<i32>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // Initialize Stripe client
    let stripe_secret_key = std::env::var("STRIPE_SECRET_KEY")
        .expect("STRIPE_SECRET_KEY must be set in environment");
    let client = Client::new(stripe_secret_key);

    // Extract token from Authorization header
    let auth_header = headers
        .get("Authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "));

    let token = match auth_header {
        Some(token) => token,
        None => return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "No authorization token provided"})),
        )),
    };

    // Decode and validate JWT token (assuming Claims and other imports are defined)
    let claims = match decode::<Claims>(
        token,
        &DecodingKey::from_secret(
            std::env::var("JWT_SECRET_KEY")
                .expect("JWT_SECRET_KEY must be set in environment")
                .as_bytes(),
        ),
        &Validation::new(Algorithm::HS256),
    ) {
        Ok(token_data) => token_data.claims,
        Err(_) => return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Invalid token"})),
        )),
    };

    // Fetch user from the database
    let user = state
        .user_repository
        .find_by_id(user_id)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)})),
        ))?
        .ok_or_else(|| (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found"})),
        ))?;

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

    // Create a Billing Portal Session
    // Create a Billing Portal Session
    let mut create_session = CreateBillingPortalSession::new(customer_id.parse().unwrap());

    // Store the formatted URL in a variable first
    let return_url = format!(
        "{}/profile",
        std::env::var("DOMAIN_URL").expect("DOMAIN_URL not set")
    );
    create_session.return_url = Some(&return_url);
    let portal_session = BillingPortalSession::create(
        &client,
create_session,
    )
    .await
    .map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": format!("Failed to create Customer Portal session: {}", e)})),
    ))?;

    // Return the portal URL to redirect the user
    Ok(Json(json!({
        "url": portal_session.url, 
        "message": "Redirecting to Stripe Customer Portal"
    })))
}



pub async fn create_checkout_session(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(user_id): Path<i32>,
    Json(payload): Json<BuyCreditsRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    println!("Starting create_checkout_session for user_id: {}", user_id);
    // Extract token from Authorization header
    let auth_header = headers
        .get("Authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "));

    let token = match auth_header {
        Some(token) => token,
        None => return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "No authorization token provided"})),
        )),
    };

    println!("Token extracted successfully");
    // Decode and validate JWT token
    let claims = match decode::<Claims>(
        token,
        &DecodingKey::from_secret(
            std::env::var("JWT_SECRET_KEY")
                .expect("JWT_SECRET_KEY must be set in environment")
                .as_bytes(),
        ),
        &Validation::new(Algorithm::HS256),
    ) {
        Ok(token_data) => token_data.claims,
        Err(_) => return Err((
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Invalid token"})),
        )),
    };

    println!("JWT token validated successfully");
    // Fetch user from the database
    let user = state
        .user_repository
        .find_by_id(user_id)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)})),
        ))?
        .ok_or_else(|| (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found"})),
        ))?;

    println!("User found: {}", user.email);
    // Initialize Stripe client
    let stripe_secret_key = std::env::var("STRIPE_SECRET_KEY")
        .expect("STRIPE_SECRET_KEY must be set in environment");
    let client = Client::new(stripe_secret_key);

    println!("Stripe client initialized");
    // Check if user has a Stripe customer ID; if not, create one

    // Check if user has a Stripe customer ID; if not, create one
    let customer_id = match state.user_repository.get_stripe_customer_id(user_id) {
        Ok(Some(id)) => {
            println!("Found existing Stripe customer ID: {}", id);
            // Try to retrieve the customer to verify it exists
            match Customer::retrieve(&client, &id.parse().unwrap(), &[]).await {
                Ok(customer) => {
                    // Customer exists 
                    create_new_customer(&client, user_id, &user.email, &state).await?
                    //id // Return as String
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

    let domain_url = std::env::var("DOMAIN_URL").expect("DOMAIN_URL not set");
    println!("Using domain: {}", domain_url);
    
    // Create a Checkout Session with payment method attachment
    println!("Creating Stripe checkout session");
    let checkout_session = CheckoutSession::create(
        &client,
        CreateCheckoutSession {
            success_url: Some(&format!("{}/profile", domain_url)), // Redirect after success
            cancel_url: Some(&format!("{}/profile", domain_url)), // Redirect after cancellation
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
    println!("Returning checkout URL: {}", checkout_session.url.as_ref().unwrap());
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
        println!("Failed to create customer: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to create Stripe customer: {}", e)})),
        )
    })?;

    println!("Created new Stripe customer: {}", customer.id);
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
    println!("Stripe webhook received");

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

    println!("Stripe signature header found");
    // Construct and verify the Stripe event using the signature
    let event = stripe::Webhook::construct_event(
        &payload_str,
        &sig_header,
        &webhook_secret,
    ).map_err(|e| (
        StatusCode::BAD_REQUEST,
        Json(json!({"error": format!("Invalid Stripe webhook signature: {}", e)})),
    ))?;
    
    println!("Stripe event verified successfully: {}", event.type_);
    // Process the event based on its type
    match event.type_ {
        stripe::EventType::CheckoutSessionCompleted => {
            println!("Processing checkout.session.completed event");
            match event.data.object {
                stripe::EventObject::CheckoutSession(session) => {
                    println!("Checkout session found: {}", session.id);
                    if let Some(customer) = &session.customer {
                        let customer_id = match customer {
                            stripe::Expandable::Id(id) => id.clone(),
                            stripe::Expandable::Object(customer) => customer.id.clone(),
                        };
                        println!("Customer ID: {}", customer_id);

                        // Update customer address with billing address from Checkout
                        if let Some(billing_details) = &session.shipping_details {
                            if let Some(address) = &billing_details.address {
                                println!("Updating customer address with billing details");
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
                        
                        println!("Payment intent ID found");
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
                            
                            println!("Payment method ID found");
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

                            println!("Found user with ID: {}", user.id);
                            state
                                .user_repository
                                .set_stripe_payment_method_id(user.id, &payment_method_id)
                                .map_err(|e| (
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    Json(json!({"error": format!("Database error: {}", e)})),
                                ))?;
                            println!("Successfully saved payment method ID for user");

                            let amount_in_cents = session.amount_subtotal.unwrap_or(0);
                            let amount = amount_in_cents as f32 / 100.00;
                            state
                                .user_repository
                                .increase_credits(user.id, amount)
                                .map_err(|e| (
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    Json(json!({"error": format!("Database error: {}", e)})),
                                ))?;
                            println!("Increased the credits amount by {} successfully", amount);
                        }
                    }
                },
                _ => {
                    println!("Checkout session not found in event object");
                }
            }
        },
        _ => {
            println!("Ignoring non-checkout.session.completed event");
        }
    }

    println!("Webhook processed successfully");
    Ok(StatusCode::OK) // Return 200 OK for successful webhook processing
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
        .user_repository
        .find_by_id(user_id)
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)})),
        ))?
        .ok_or_else(|| (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found"})),
        ))?;
    println!("User found: {}", user.email);

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
    println!("Stripe customer ID found: {}", customer_id);

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
    println!("Stripe payment method ID found: {}", payment_method_id);

    // Fetch the user's auto-topup settings to determine the charge amount
    let charge_back_to = user.charge_back_to.unwrap_or(5.00);
    println!("User charge_back_to: {}, current credits: {}", charge_back_to, user.credits);

    let charge_amount = charge_back_to - user.credits; 
    if charge_amount < 0.00 {
        println!("User credits already over the charge back to amount, no charge needed");
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("user credits already over the charge back to amount")})),
        ));
    }
    
    let charge_amount_cents = (charge_amount * 100.0).round() as i64; // Convert to cents for Stripe
    println!("Charging credits (€{})", charge_amount);

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
            .increase_credits(user_id, charge_amount) 
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
            "amount": charge_amount,
        })))
    } else {
        println!("Payment intent failed or requires action, status: {:?}", payment_intent.status);
        Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Payment intent failed or requires action"})),
        ))
    }
}

