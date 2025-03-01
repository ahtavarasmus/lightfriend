use std::sync::Arc;
use axum::{
    extract::{Json, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::AppState;
use crate::handlers::auth_dtos::Claims;

#[derive(Deserialize)]
pub struct CreateSessionPayload {
    amount: u64, // amount in cents
}

#[derive(Serialize)]
pub struct CheckoutSessionResponse {
    session_id: String,
}

pub async fn create_checkout_session(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<CreateSessionPayload>,
) -> Result<Json<CheckoutSessionResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Extract and validate token
    let auth_header = headers
        .get("Authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "));

    let token = match auth_header {
        Some(token) => token,
        None => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "No authorization token provided"})),
            ))
        }
    };

    // Decode JWT token
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
        Err(_) => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Invalid token"})),
            ))
        }
    };

    // For now, we'll just return a simulated session ID
    // In a real implementation, you would call the Stripe API here
    let user_id = claims.sub;
    let session_id = format!("cs_test_simulated_{user_id}_{}", payload.amount);

    // Calculate how many IQ points the user will receive for informational purposes
    let iq_amount = (payload.amount as f64 / 100.0 * 300.0) as i32; // 300 IQ per euro

    // Log this transaction attempt
    println!("User {} is attempting to purchase {} IQ for {} cents", user_id, iq_amount, payload.amount);
    
    // Return session ID to frontend
    Ok(Json(CheckoutSessionResponse {
        session_id,
    }))
}

// Simple webhook handler for completed payments
#[derive(Deserialize)]
pub struct StripeWebhookPayload {
    #[serde(rename = "type")]
    event_type: String,
    data: StripeWebhookData,
}

#[derive(Deserialize)]
pub struct StripeWebhookData {
    object: StripeSessionObject,
}

#[derive(Deserialize)]
pub struct StripeSessionObject {
    id: String,
    client_reference_id: Option<String>,
    amount_total: Option<u64>,
    payment_status: String,
}

pub async fn handle_stripe_webhook(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<StripeWebhookPayload>,
) -> impl IntoResponse {
    // Process completed checkout sessions
    if payload.event_type == "checkout.session.completed" && payload.data.object.payment_status == "paid" {
        // Get user ID from client_reference_id
        if let Some(user_id_str) = &payload.data.object.client_reference_id {
            if let Ok(user_id) = user_id_str.parse::<i32>() {
                // Calculate IQ amount based on payment amount
                if let Some(amount) = payload.data.object.amount_total {
                    let iq_amount = (amount as f64 / 100.0 * 300.0) as i32; // 300 IQ per euro
                    
                    // Add IQ to user account
                    if let Err(e) = state.user_repository.increase_iq(user_id, iq_amount) {
                        eprintln!("Failed to add IQ to user account: {}", e);
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(json!({"error": "Failed to process payment"})),
                        );
                    }
                    
                    return (
                        StatusCode::OK,
                        Json(json!({"success": true})),
                    );
                }
            }
        }
    }

    (StatusCode::OK, Json(json!({"received": true})))
}