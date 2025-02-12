use std::sync::Arc;
use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde_json::json;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use crate::AppState;

#[derive(Deserialize)]
pub struct BuyIqRequest {
    pub amount: i32,
    pub user_id: i32,
}

#[derive(Serialize)]
struct LemonSqueezyCheckoutRequest {
    data: CheckoutData,
}

#[derive(Serialize)]
struct CheckoutData {
    #[serde(rename = "type")]
    type_field: String,
    attributes: CheckoutAttributes,
    relationships: CheckoutRelationships,
}

#[derive(Serialize)]
struct CheckoutAttributes {
    custom_price: i32,
    checkout_data: serde_json::Value,
    checkout_options: CheckoutOptions,
}

#[derive(Serialize)]
struct CheckoutOptions {
embed: bool,
media: bool,
logo: bool,
}

#[derive(Serialize)]
struct CheckoutRelationships {
    store: Relationship,
    variant: Relationship,
}

#[derive(Serialize)]
struct Relationship {
    data: RelationshipData,
}

#[derive(Serialize)]
struct RelationshipData {
    #[serde(rename = "type")]
    type_field: String,
    id: String,
}

#[derive(Deserialize)]
struct LemonSqueezyResponse {
    data: CheckoutResponseData,
}

#[derive(Deserialize)]
struct CheckoutResponseData {
    attributes: CheckoutResponseAttributes,
}

#[derive(Deserialize)]
struct CheckoutResponseAttributes {
    url: String,
}
pub async fn create_checkout(
    State(state): State<Arc<AppState>>,
    Json(request): Json<BuyIqRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    println!("Creating checkout for user_id: {}, amount: {}", request.user_id, request.amount);
    
    println!("Starting checkout process with amount: {} IQ for user: {}", request.amount, request.user_id);
    let client = reqwest::Client::new();
    
    // Calculate price in cents (0.2€ per 60 IQ)
    let price_in_cents = ((request.amount as f64 / 60.0) * 0.2 * 100.0) as i32;
    println!("Calculated price in cents: {}", price_in_cents);

    let store_id = std::env::var("LEMON_SQUEEZY_STORE_ID")
        .expect("No LEMON_SQUEEZY_STORE_ID");
    
    let variant_id = std::env::var("LEMON_SQUEEZY_VARIANT_ID")
        .expect("No LEMON_SQUEEZY_VARIANT_ID");

    let checkout_request = LemonSqueezyCheckoutRequest {
        data: CheckoutData {
            type_field: "checkouts".to_string(),
            attributes: CheckoutAttributes {
                custom_price: price_in_cents,
                checkout_data: json!({
                    "custom": {
                        "user_id": request.user_id.to_string(),
                        "iq_amount": request.amount.to_string()
                    }
                }),
                checkout_options: CheckoutOptions {
                    embed: true,
                    media: true,
                    logo: true,
                },
            },
        relationships: CheckoutRelationships {
            store: Relationship {
                data: RelationshipData {
                    type_field: "stores".to_string(),
                    id: store_id,
                },
            },
            variant: Relationship {
                data: RelationshipData {
                    type_field: "variants".to_string(),
                    id: variant_id,
                },
            },
        },
        },
    };

    let mut headers = HeaderMap::new();
    let api_key = std::env::var("LEMON_SQUEEZY_API_KEY").expect("no LEMON_SQUEEZY_API_KEY found");

    let auth_header = match HeaderValue::from_str(&format!("Bearer {}", api_key)) {
        Ok(header) => header,
        Err(e) => {
            println!("Invalid API key format: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Invalid API key format"}))
            ));
        }
    };
    headers.insert(AUTHORIZATION, auth_header);
    headers.insert(ACCEPT, HeaderValue::from_static("application/vnd.api+json"));
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/vnd.api+json"));
    
    println!("Headers set for request:");
    for (key, value) in headers.iter() {
        println!("  {}: {}", key, value.to_str().unwrap_or("[invalid header value]"));
    }

    let response = client
        .post("https://api.lemonsqueezy.com/v1/checkouts")
        .headers(headers)
        .json(&checkout_request)
        .send()
        .await
        .map_err(|e| {
            println!("Failed to send request to Lemon Squeezy: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to connect to payment service"})),
            )
        })?;

    let status = response.status();
    if !status.is_success() {
        let error_body = response.text().await.unwrap_or_else(|_| "No error details available".to_string());
        println!(
            "Lemon Squeezy API error - Status: {}, Body: {}",
            status,
            error_body
        );
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "error": "Payment service error",
                "details": error_body
            }))
        ));
    }

    let checkout_response: LemonSqueezyResponse = response.json().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Failed to parse response: {}", e)}))
        )
    })?;

    Ok(Json(serde_json::json!({
        "checkout_url": checkout_response.data.attributes.url
    })))
}
