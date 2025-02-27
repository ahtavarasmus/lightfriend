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
    iq_quantity: i32, // Usage since last sync
) -> Result<(), Box<dyn Error>> {
    println!("Syncing subscription {}", subscription_id);

    // Fetch Paddle API key and price IDs from environment variables
    let api_key = env::var("PADDLE_API_KEY")
        .map_err(|_| "PADDLE_API_KEY environment variable not set")?;
    let zero_sub_price_id = env::var("ZERO_SUB_PRICE_ID").expect("ZERO_SUB_PRICE_ID not set");
    let iq_usage_price_id = env::var("IQ_USAGE_PRICE_ID").expect("IQ_USAGE_PRICE_ID not set");

    let client = Client::new();
    let url = format!("https://sandbox-api.paddle.com/subscriptions/{}", subscription_id);

    // Step 1: Check subscription status first
    let status_response = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await?;

    if !status_response.status().is_success() {
        let error_text = status_response.text().await?;
        return Err(format!("Failed to fetch subscription status: {}", error_text).into());
    }

    let status_json: serde_json::Value = status_response.json().await?;
    println!("{:#?}", status_json);
    let current_status = status_json["data"]["status"].as_str().unwrap_or("unknown");
    let scheduled_change = status_json["data"]["scheduled_change"].as_object();
    println!("status: {:#?}", &current_status);
    println!("Scheduled_change: {:#?}", &scheduled_change);
    println!("iq: {:#?}", &iq_quantity);

    println!("{}", current_status == "active" && scheduled_change.is_some() && iq_quantity < 0);

    // Step 2: If subscription is active and no cancellation is scheduled, update as usual
    //if current_status == "active" && scheduled_change.is_none() {
    if false {
        let payload = json!({
            "items": [
                {
                    "price_id": zero_sub_price_id,  // Zero-dollar subscription price ID
                    "quantity": 1                   // Always 1 for the base sub
                },
                {
                    "price_id": iq_usage_price_id,  // IQ usage price ID
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
    }
    // Step 3: If subscription is active but scheduled for cancellation, issue a one-time charge
    else if current_status == "active" {
        println!("Subscription {} is scheduled for cancellation, issuing one-time charge", subscription_id);

        
        // Add the one-time charge
        add_one_time_charge(
            subscription_id,
            iq_quantity,
        ).await?;
        
        println!("Successfully processed one-time charge for {} IQ credits", iq_quantity);
    } else {
        println!(
            "Subscription {} is not active or has no usage to charge (status: {})",
            subscription_id, current_status
        );
    }

    Ok(())

}
pub async fn add_one_time_charge(
    subscription_id: &str,
    iq_amount: i32,
) -> Result<(), Box<dyn Error>> {
    if iq_amount < 300 {
        return Err(format!("Error. While issuing one time charge the amount was lower than 1€").into());
    }
    // return error if iq_amount is less than 300
    println!("Starting one-time charge process for subscription: {}", subscription_id);
    println!("IQ Amount: {}", iq_amount);
    
    let api_key = env::var("PADDLE_API_KEY")
        .map_err(|_| "PADDLE_API_KEY environment variable not set")?;
    println!("API key retrieved successfully");
    
    let client = Client::new();
    let url = format!("https://sandbox-api.paddle.com/subscriptions/{}/charge", subscription_id);
    println!("Request URL: {}", url);

    let iq_usage_price_id = env::var("IQ_USAGE_PRICE_ID").expect("IQ_USAGE_PRICE_ID not set");
    println!("IQ Usage Price ID: {}", iq_usage_price_id);

    let iq_turned_to_paddle_item = iq_amount / 3; // since one quantity of iq usage item is 3 iq == 0.01 €

    let payload = json!({
        "effective_from": "immediately",
        "items": [
            {
                "price_id": iq_usage_price_id,
                "quantity": iq_turned_to_paddle_item
            }
        ]
    });
    println!("Request payload: {}", payload);

    println!("Sending request to Paddle API...");
    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&payload)
        .send()
        .await?;
    println!("Response status: {}", response.status());

    if response.status().is_success() {
        println!("Successfully added one-time charge for subscription {}", subscription_id);
        println!("IQ amount charged: {}", iq_amount);
    } else {
        let error_text = response.text().await?;
        println!("Error response body: {}", error_text);
        eprintln!("Failed to add one-time charge: {}", error_text);
        return Err(format!("Paddle API error: {}", error_text).into());
    }

    println!("One-time charge function completed successfully");
    Ok(())
}
