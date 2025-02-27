use axum::{
    extract::State,
    body::Body,
    Json,
    http::{StatusCode, Request, HeaderMap},
    response::Response,
    middleware::Next,

};
use crate::models::user_models::NewSubscription;
use serde_json::json;
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
    pub event_type: String,
    pub data: Data,
}

#[derive(Debug, Deserialize)]
pub struct CustomData {
    pub user_id: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct Data {
    #[serde(rename = "id")]
    pub subscription_id: String, // subscription id
    pub customer_id: String, // Paddle's customer ID
    pub status: Option<String>, // active, inactive, trialing
    pub next_billed_at: Option<String>,
    pub items: Option<Vec<SubscriptionItem>>,
    pub scheduled_change: Option<ScheduledChange>,
    pub custom_data: Option<CustomData>,
}

#[derive(Debug, Deserialize)]
pub struct ScheduledChange {
    pub action: String, // cancel, pause, resume
    pub effective_at: String, // timestamp where this state will happen(separete cancelled event will be delivered)
    pub resume_at: Option<String>, // when paused sub should resume(only for pause scheduled changes)
}


#[derive(Debug, Deserialize)]
pub struct SubscriptionItem {
    pub price: Price,
    pub product: Product,
    pub status: String, // active, inactive, trialing
    pub quantity: i32,
}

#[derive(Debug, Deserialize)]
pub struct Price {
    pub id: String,
    pub unit_price: UnitPrice,
}

#[derive(Debug, Deserialize)]
pub struct UnitPrice {
    pub amount: String,
    pub currency_code: String,
}

#[derive(Debug, Deserialize)]
pub struct Product {
    pub id: String,
    pub name: String,
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
        Ok(Json(payload)) => {
            println!("Raw webhook payload: {:?}", payload);
            payload
        },
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
        "transaction.billed" => {
            let subscription_id = payload.data.subscription_id;
            let customer_id= &payload.data.customer_id;
            match state.user_subscriptions.reset_user_iq_with_customer_id(customer_id) {
                Ok(_) => {
                    tracing::info!("Successfully resetted user's iq to zero for the next billing period");
                    // Spawn the reset operation as a background task
                    tokio::spawn(async move {
                        // Fetch next_billed_at
                        match crate::api::paddle_utils::get_next_billed_at(&subscription_id).await {
                            Ok(next_billed_at) => {
                                tracing::info!(
                                    "Next billing date for sub {}: {}",
                                    subscription_id,
                                    next_billed_at
                                );
                                // Update the DB with the new next_billed_at
                                if let Err(e) = state.user_subscriptions.update_next_billed_at(&subscription_id, &next_billed_at) {
                                    tracing::error!(
                                        "Failed to update next_billed_at for sub {}: {}",
                                        subscription_id,
                                        e
                                    );
                                }
                            }
                            Err(e) => {
                                tracing::error!(
                                    "Failed to fetch next_billed_at for sub {}: {}",
                                    subscription_id,
                                    e
                                );
                            }
                        }
                        
                        match crate::api::paddle_utils::reset_paddle_subcription_items(&subscription_id).await {
                            Ok(_) => {
                                tracing::info!(
                                    "Successfully reset paddle's subscription items for sub: {}", 
                                    subscription_id
                                );
                            }
                            Err(e) => {
                                tracing::error!(
                                    "Failed to reset paddle subscription items for sub:{}: {}", 
                                    subscription_id, 
                                    e
                                );
                            }
                        }
                    });

                    Ok(Json(WebhookResponse {
                        status: "success".to_string(),
                    }))
                },
                Err(err) => {
                    tracing::error!("Failed to reset user's iq for the next billing period: {:?}", err);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        },
        "subscription.canceled" => {
            let subscription_id = &payload.data.subscription_id;
            match state.user_subscriptions.update_subscription_status(subscription_id, "canceled") {
                Ok(_) => {
                    tracing::info!("Successfully updated subscription {} status to canceled", subscription_id);
                    Ok(Json(WebhookResponse {
                        status: "success".to_string(),
                    }))
                },
                Err(err) => {
                    tracing::error!("Failed to update subscription status: {:?}", err);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        },
        "subscription.updated" => {
            let subscription_id = &payload.data.subscription_id;
            let customer_id = &payload.data.customer_id;
            let status = payload.data.status.as_deref().unwrap_or("canceled");
            
            // Calculate next bill timestamp
            let next_bill_timestamp = payload.data.next_billed_at
                .and_then(|date_str| chrono::DateTime::parse_from_rfc3339(&date_str).ok())
                .map(|dt| dt.timestamp() as i32)
                .unwrap_or_else(|| (chrono::Utc::now().timestamp() + 30 * 24 * 60 * 60) as i32);

            // Verify if it's still a zero subscription
            let zero_sub_exists = payload.data.items
                .as_ref()
                .map(|items| items.iter().any(|item| item.price.id == "pri_01jmqk1r39nk4h7bbr10jbatsz"))
                .unwrap_or(false);

            if !zero_sub_exists {
                tracing::error!("Updated subscription missing zero subscription price");
                return Err(StatusCode::BAD_REQUEST);
            }
            let stage = "tier 1".to_string();

            // Check if the subscription is scheduled to be canceled
            let is_scheduled_to_cancel = payload.data.scheduled_change
                .as_ref()
                .map(|change| change.action == "cancel")
                .unwrap_or(false);

            // Optionally, you could also store or log the effective_at timestamp if needed
            if is_scheduled_to_cancel {
                if let Some(scheduled_change) = &payload.data.scheduled_change {
                    tracing::info!(
                        "Subscription {} is scheduled to cancel at timestamp {}",
                        subscription_id,
                        scheduled_change.effective_at
                    );
                }
            }

            match state.user_subscriptions.update_subscription_with_customer_id(
                subscription_id,
                customer_id,
                status,
                next_bill_timestamp,
                &stage,
                is_scheduled_to_cancel,
            ) {
                Ok(_) => {
                    tracing::info!("Successfully updated subscription {}", subscription_id);
                    Ok(Json(WebhookResponse {
                        status: "success".to_string(),
                    }))
                },
                Err(err) => {
                    tracing::error!("Failed to update subscription: {:?}", err);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }
        },
        "subscription.created" => {
            let custom_data = match payload.data.custom_data {
                Some(data) => data,
                None => {
                    tracing::error!("Subscription created without custom user_id data");
                    return Err(StatusCode::BAD_REQUEST);
                }
            };
            let user_id = match custom_data.user_id {
                Some(id) => id,
                None => {
                    tracing::error!("Subscription created without user_id in custom data");
                    return Err(StatusCode::BAD_REQUEST);
                }
            };
            // Create new subscription
            let next_bill_timestamp = payload.data.next_billed_at
                .and_then(|date_str| chrono::DateTime::parse_from_rfc3339(&date_str).ok())
                .map(|dt| dt.timestamp() as i32)
                .unwrap_or_else(|| (chrono::Utc::now().timestamp() + 30 * 24 * 60 * 60) as i32); // Default to 30 days from now

            
            // search if items have price id == pri_01jmqk1r39nk4h7bbr10jbatsz 
            let zero_sub_exists = payload.data.items
                .as_ref()
                .map(|items| items.iter().any(|item| item.price.id == "pri_01jmqk1r39nk4h7bbr10jbatsz"))
                .unwrap_or(false);

            if !zero_sub_exists {
                tracing::error!("Subscription created without zero subscription price");
                return Err(StatusCode::BAD_REQUEST);
            } 
            let stage = "tier 1".to_string();


            // Get subscription for the user
            let subscription = match state.user_subscriptions.find_by_user_id(user_id) {
                Ok(Some(sub)) => Some(sub),
                Ok(None) => None,
                Err(e) => {
                    tracing::error!("error when fetching subscription by user id: {:?}", e);
                    return Err(StatusCode::BAD_REQUEST);
                }
            };
            if let Some(_) = subscription {
                return match state.user_subscriptions.update_subscription_with_user_id(
                    user_id,
                    &payload.data.subscription_id, 
                    &payload.data.customer_id,
                    &payload.data.status.unwrap_or_else(|| "active".to_string()),
                    next_bill_timestamp,
                    &stage
                ) {
                    Ok(_) => {
                        tracing::info!("Successfully updated subscription for user_id: {}", user_id);
                        state.user_repository.update_user_iq(user_id, 0).unwrap_or_else(|err| {
                            tracing::error!("Failed to update user IQ to 0: {:?}", err);
                        });
                        // TODO think about what happens to the possible existing sub(probably just set it to canceled)
                        Ok(Json(WebhookResponse {
                            status: "success".to_string(),
                        }))
                    },
                    Err(err) => {
                        tracing::error!("Failed to update subscription: {:?}", err);
                        Err(StatusCode::INTERNAL_SERVER_ERROR)
                    }
                };
            }

                        
            let new_subscription = NewSubscription {
                user_id: user_id,
                paddle_subscription_id: payload.data.subscription_id,
                paddle_customer_id: payload.data.customer_id,
                stage: stage,
                status: payload.data.status.unwrap_or_else(|| "active".to_string()),
                next_bill_date: next_bill_timestamp,
                is_scheduled_to_cancel: Some(false),
            };

            match state.user_subscriptions.create_subscription(new_subscription) {
                Ok(_) => {
                    // Update user's IQ (credits) to 0
                    if let Err(err) = state.user_repository.update_user_iq(user_id, 0) {
                        tracing::error!("Failed to update user IQ to 0: {:?}", err);
                        // Continue with subscription creation even if IQ update fails
                    } else {
                        tracing::info!("Successfully updated user_id: {} IQ to 0", user_id);
                    }
                    
                    tracing::info!("Successfully created subscription for user_id: {}", user_id);
                    Ok(Json(WebhookResponse {
                        status: "success".to_string(),
                    }))
                },
                Err(err) => {
                    tracing::error!("Failed to create subscription: {:?}", err);
                    Err(StatusCode::INTERNAL_SERVER_ERROR)
                }
            }


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
