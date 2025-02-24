use axum::{
    extract::State,
    body::Body,
    Json,
    http::{StatusCode, Request, HeaderMap},
    response::Response,
    middleware::Next,

};
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use axum::body::to_bytes;

use hmac::{Hmac, Mac};
use sha2::Sha256;
use hex;


use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::AppState;


type HmacSha256 = Hmac<Sha256>;


#[derive(Debug, Deserialize)]
pub struct PaddleWebhookPayload {
    event_id: String,
    event_type: String,
    occurred_at: String,
    notification_id: String,
    data: SubscriptionData,
}

#[derive(Debug, Deserialize)]
pub struct SubscriptionData {
    id: String,
    status: String,
    customer_id: String,
    items: Vec<SubscriptionItem>,
    currency_code: String,
    billing_cycle: BillingCycle,
    next_billed_at: String,
}

#[derive(Debug, Deserialize)]
pub struct SubscriptionItem {
    price: Price,
    product: Product,
    status: String,
    quantity: i32,
}

#[derive(Debug, Deserialize)]
pub struct Price {
    id: String,
    unit_price: UnitPrice,
}

#[derive(Debug, Deserialize)]
pub struct UnitPrice {
    amount: String,
    currency_code: String,
}

#[derive(Debug, Deserialize)]
pub struct Product {
    id: String,
    name: String,
}

#[derive(Debug, Deserialize)]
pub struct BillingCycle {
    interval: String,
    frequency: i32,
}

#[derive(Debug, Serialize)]
pub struct WebhookResponse {
    status: String,
}

pub async fn handle_subscription_webhook(
    State(state): State<Arc<AppState>>,
    payload_result: Result<Json<PaddleWebhookPayload>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<WebhookResponse>, StatusCode> {
    // Handle potential JSON parsing errors
    let payload = match payload_result {
        Ok(Json(payload)) => payload,
        Err(err) => {
            tracing::error!("Failed to parse webhook payload: {:?}", err);
            return Err(StatusCode::UNPROCESSABLE_ENTITY);
        }
    };
    // Print the entire webhook payload for debugging
    tracing::info!(
        "Received Paddle webhook payload: {:#?}", 
        payload
    );

    match payload.event_type.as_str() {
        "subscription.created" => {
            // Handle subscription creation
            // TODO: Update user's subscription status in the database
            // You'll need to add subscription-related fields to your user model
            // and create methods in UserRepository to handle subscription updates
            Ok(Json(WebhookResponse {
                status: "success".to_string(),
            }))
        }
        // Add other event types as needed
        _ => {
            tracing::warn!("Unhandled webhook event type: {}", payload.event_type);
            Ok(Json(WebhookResponse {
                status: "unhandled_event".to_string(),
            }))
        }
    }
}

pub async fn validate_paddle_secret(
    headers: HeaderMap,
    mut request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let signature = match headers.get("Paddle-Signature") {
        Some(value) => match value.to_str() {
            Ok(sig) => sig,
            Err(_) => return Err(StatusCode::BAD_REQUEST),
        },
        None => return Err(StatusCode::BAD_REQUEST),
    };

    let parts: Vec<&str> = signature.split(';').collect();
    let mut timestamp = "";
    let mut h1_signature = "";

    for part in parts {
        let key_value: Vec<&str> = part.split('=').collect();
        if key_value.len() == 2 {
            match key_value[0] {
                "ts" => timestamp = key_value[1],
                "h1" => h1_signature = key_value[1],
                _ => {},
            }
        }
    }

    // Set a reasonable size limit for the webhook payload (e.g., 1MB)
    let size_limit = 1024 * 1024;
    let body = request.body_mut();
    let body_bytes = to_bytes(std::mem::replace(body, Body::empty()), size_limit).await
        .map_err(|_| StatusCode::BAD_REQUEST)?;


    let signed_payload = format!("{}:{}", timestamp, String::from_utf8_lossy(&body_bytes));

    let secret_key = std::env::var("PADDLE_WEBHOOK_SECRET").expect("PADDLE_WEBHOOK_SECRET not set");
    let mut mac = Hmac::<Sha256>::new_from_slice(secret_key.as_bytes())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    mac.update(signed_payload.as_bytes());
    let result = mac.finalize();
    let computed_hash = hex::encode(result.into_bytes());

    if computed_hash == h1_signature {
        // Reconstruct the request with the body
        *request.body_mut() = Body::from(body_bytes);
        println!("Paddle secret verified!");
        Ok(next.run(request).await)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}
