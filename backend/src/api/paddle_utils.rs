use reqwest::Client;
use serde_json::{json, Value};
use std::env;
use std::error::Error;


pub async fn get_next_billed_at(
    subscription_id: &str
) -> Result<String, Box<dyn Error>> {
    // Fetch the Paddle API key from environment variable
    let api_key = env::var("PADDLE_API_KEY")
        .map_err(|_| "PADDLE_API_KEY environment variable not set")?;

    // Use the sandbox API URL consistent with your other functions
    let url = format!("https://sandbox-api.paddle.com/subscriptions/{}", subscription_id);

    let client = Client::new();

    // Send GET request to retrieve subscription details
    let response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .send()
        .await?;

    if response.status().is_success() {
        let json: Value = response.json().await?;
        let next_billed_at = json["data"]["next_billed_at"]
            .as_str()
            .ok_or("next_billed_at not found in response")?
            .to_string();

        println!(
            "Successfully fetched next_billed_at for subscription {}: {}",
            subscription_id, next_billed_at
        );
        Ok(next_billed_at) // Returns something like "2025-03-26T00:00:00Z"
    } else {
        let error_text = response.text().await?;
        eprintln!("Failed to fetch subscription details: {}", error_text);
        Err(format!("Paddle API error: {}", error_text).into())
    }
}

pub async fn reset_paddle_subcription_items(
    subscription_id: &str,
) -> Result<(), Box<dyn Error>> {
    // Fetch the Paddle API key from environment variable
    let api_key = env::var("PADDLE_API_KEY")
        .map_err(|_| "PADDLE_API_KEY environment variable not set")?;
    let zero_sub_price_id = env::var("ZERO_SUB_PRICE_ID").expect("ZERO_SUB_PRICE_ID not set");

    let url = format!("https://sandbox-api.paddle.com/subscriptions/{}", subscription_id);

    let client = Client::new();

    // Payload with only the zero-dollar subscription item
    let payload = json!({
        "items": [
            {
                "price_id": zero_sub_price_id,  // Zero-dollar subscription price ID
                "quantity": 1                   // Always 1 for the base sub
            }
        ],
        "proration_billing_mode": "prorated_next_billing_period"  // Bill at end of cycle
    });

    let response = client
        .patch(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&payload)
        .send()
        .await?;

    if response.status().is_success() {
        println!(
            "Successfully updated subscription {}",
            subscription_id
        );
    } else {
        let error_text = response.text().await?;
        eprintln!("Failed to update subscription: {}", error_text);
        return Err(format!("Paddle API error: {}", error_text).into());
    }

    Ok(())
}

pub async fn sync_paddle_subscription_items(
    subscription_id: &str,
    iq_quantity: i32,
) -> Result<(), Box<dyn Error>> {
    println!("syncing");
    // Fetch the Paddle API key from environment variable
    let api_key = env::var("PADDLE_API_KEY")
        .map_err(|_| "PADDLE_API_KEY environment variable not set")?;
    let zero_sub_price_id = env::var("ZERO_SUB_PRICE_ID").expect("ZERO_SUB_PRICE_ID not set");
    let iq_usage_price_id= env::var("IQ_USAGE_PRICE_ID").expect("IQ_USAGE_PRICE_ID not set");

    let url = format!("https://sandbox-api.paddle.com/subscriptions/{}", subscription_id);

    let client = Client::new();

    // Payload with both items: zero-dollar sub (quantity 1) and IQ usage (dynamic quantity)
    let payload = json!({
        "items": [
            {
                "price_id": zero_sub_price_id,  // Zero-dollar subscription price ID
                "quantity": 1                   // Always 1 for the base sub
            },
            {
                "price_id": iq_usage_price_id,        // IQ usage price ID
                "quantity": iq_quantity         // Dynamic IQ credits used
            }
        ],
        "proration_billing_mode": "prorated_next_billing_period"  // Bill at end of cycle
    });
    println!("Syncing subscription with IQ quantity: {}", iq_quantity);

    let response = client
        .patch(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&payload)
        .send()
        .await?;

    if response.status().is_success() {
        println!(
            "Successfully updated subscription {} with IQ quantity {}",
            subscription_id, iq_quantity
        );
    } else {
        let error_text = response.text().await?;
        eprintln!("Failed to update subscription: {}", error_text);
        return Err(format!("Paddle API error: {}", error_text).into());
    }

    Ok(())
}

