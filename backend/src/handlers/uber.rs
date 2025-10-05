use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::Json as AxumJson,
};
use reqwest;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::env;
use std::error::Error;
use std::sync::Arc;
use tracing;
use urlencoding;

use crate::AppState;
use crate::handlers::auth_middleware::AuthUser;

#[derive(Deserialize)]
pub struct UberOptionsRequest {
    current_address: String,
    target_address: String,
}

pub async fn get_uber_options(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    AxumJson(input): AxumJson<UberOptionsRequest>,
) -> Result<AxumJson<Value>, (StatusCode, AxumJson<Value>)> {
    tracing::info!(
        "Fetching Uber options for user {} from {} to {}",
        auth_user.user_id,
        input.current_address,
        input.target_address
    );

    let tokens = state.user_repository.get_uber_tokens();
    let access_token = match tokens {
        Ok(Some((access, _))) => access,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                AxumJson(json!({"error": "No active Uber connection found"})),
            ))
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": format!("Failed to fetch Uber tokens: {}", e)})),
            ))
        }
    };

    let client = reqwest::Client::new();
    let geoapify_key = env::var("GEOAPIFY_API_KEY").expect("GEOAPIFY_API_KEY must be set");

    let (start_lat, start_lon, _start_formatted) = match crate::utils::tool_exec::get_coordinates(
        &client,
        &input.current_address,
        &geoapify_key,
    )
    .await
    {
        Ok(coords) => coords,
        Err(e) => {
            tracing::error!("Failed to get start coordinates: {}", e);
            return Err((
                StatusCode::BAD_REQUEST,
                AxumJson(json!({"error": format!("Failed to geocode current address: {}", e)})),
            ));
        }
    };

    let (end_lat, end_lon, _end_formatted) = match crate::utils::tool_exec::get_coordinates(
        &client,
        &input.target_address,
        &geoapify_key,
    )
    .await
    {
        Ok(coords) => coords,
        Err(e) => {
            tracing::error!("Failed to get end coordinates: {}", e);
            return Err((
                StatusCode::BAD_REQUEST,
                AxumJson(json!({"error": format!("Failed to geocode target address: {}", e)})),
            ));
        }
    };

    // Fetch products
    let products_url = format!(
        "https://api.uber.com/v1.2/products?latitude={}&longitude={}",
        start_lat, start_lon
    );
    let products_response: Value = match client
        .get(&products_url)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Accept-Language", "en_US")
        .header("Content-Type", "application/json")
        .send()
        .await
    {
        Ok(resp) => match resp.json().await {
            Ok(json) => json,
            Err(e) => {
                tracing::error!("Failed to parse products response: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    AxumJson(json!({"error": "Failed to parse Uber products response"})),
                ));
            }
        },
        Err(e) => {
            tracing::error!("Failed to fetch Uber products: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to fetch Uber products"})),
            ));
        }
    };
    let products = products_response["products"]
        .as_array()
        .ok_or_else(|| (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({"error": "No products found"})),
        ))?;

    // Fetch price estimates
    let price_url = format!(
        "https://api.uber.com/v1.2/estimates/price?start_latitude={}&start_longitude={}&end_latitude={}&end_longitude={}",
        start_lat, start_lon, end_lat, end_lon
    );
    let prices_response: Value = match client
        .get(&price_url)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Accept-Language", "en_US")
        .header("Content-Type", "application/json")
        .send()
        .await
    {
        Ok(resp) => match resp.json().await {
            Ok(json) => json,
            Err(e) => {
                tracing::error!("Failed to parse price estimates response: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    AxumJson(json!({"error": "Failed to parse Uber price estimates response"})),
                ));
            }
        },
        Err(e) => {
            tracing::error!("Failed to fetch Uber price estimates: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to fetch Uber price estimates"})),
            ));
        }
    };
    let prices = prices_response["prices"]
        .as_array()
        .ok_or_else(|| (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({"error": "No price estimates found"})),
        ))?;

    // Fetch time estimates
    let time_url = format!(
        "https://api.uber.com/v1.2/estimates/time?start_latitude={}&start_longitude={}",
        start_lat, start_lon
    );
    let times_response: Value = match client
        .get(&time_url)
        .header("Authorization", format!("Bearer {}", access_token))
        .header("Accept-Language", "en_US")
        .header("Content-Type", "application/json")
        .send()
        .await
    {
        Ok(resp) => match resp.json().await {
            Ok(json) => json,
            Err(e) => {
                tracing::error!("Failed to parse time estimates response: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    AxumJson(json!({"error": "Failed to parse Uber time estimates response"})),
                ));
            }
        },
        Err(e) => {
            tracing::error!("Failed to fetch Uber time estimates: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to fetch Uber time estimates"})),
            ));
        }
    };
    let times = times_response["times"]
        .as_array()
        .ok_or_else(|| (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({"error": "No time estimates found"})),
        ))?;

    // Combine data
    let mut options: Vec<Value> = Vec::new();
    for price in prices {
        let product_id = match price["product_id"].as_str() {
            Some(id) => id,
            None => continue,
        };

        let product_opt = products.iter().find(|p| p["product_id"].as_str() == Some(product_id));
        let time_opt = times.iter().find(|t| t["product_id"].as_str() == Some(product_id));

        if let (Some(product), Some(time)) = (product_opt, time_opt) {
            if price["low_estimate"].as_f64().is_some() {
                let opt = json!({
                    "display_name": price["display_name"],
                    "estimate": price["estimate"],
                    "low_estimate": price["low_estimate"],
                    "high_estimate": price["high_estimate"],
                    "duration": price["duration"],
                    "eta": time["estimate"],
                    "distance": price["distance"],
                    "currency_code": price["currency_code"],
                    "capacity": product["capacity"],
                    "description": product["description"],
                    "product_group": product["product_group"],
                    "product_id": product_id,
                });
                options.push(opt);
            }
        }
    }

    // Sort by low_estimate ascending
    options.sort_by(|a, b| {
        let a_low = a["low_estimate"].as_f64().unwrap_or(f64::MAX);
        let b_low = b["low_estimate"].as_f64().unwrap_or(f64::MAX);
        a_low.partial_cmp(&b_low).unwrap()
    });

    // Take top 5
    let best_options = options.into_iter().take(5).collect::<Vec<_>>();

    Ok(AxumJson(json!({
        "options": best_options
    })))
}
