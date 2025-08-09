use axum::{
    http::StatusCode,
    response::Json as AxumJson,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use reqwest;

#[derive(Debug, Deserialize)]
pub struct DirectionsRequest {
    pub start_address: String,
    pub end_address: String,
    pub mode: String, // e.g., "driving", "walking", "transit" (for public transport), "bicycling"
}

#[derive(Debug, Serialize)]
pub struct DirectionsResponse {
    pub instructions: Vec<String>,
    pub duration: String,
    pub distance: String,
}

pub async fn handle_get_directions(
    request: DirectionsRequest,
) -> Result<AxumJson<Value>, (StatusCode, AxumJson<Value>)> {
    let geoapify_api_key = std::env::var("GEOAPIFY_API_KEY")
        .map_err(|_| (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({"error": "Missing GEOAPIFY_API_KEY environment variable"})),
        ))?;

    let google_maps_api_key = std::env::var("GOOGLE_API_KEY")
        .map_err(|_| (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({"error": "Missing GOOGLE_API_KEY environment variable"})),
        ))?;

    let client = reqwest::Client::new();

    // Get starting coordinates
    let (start_lat, start_lon, _start_formatted) = match crate::utils::tool_exec::get_coordinates(&client, &request.start_address, &geoapify_api_key).await {
        Ok(coords) => coords,
        Err(e) => {
            return Err((
                StatusCode::BAD_REQUEST,
                AxumJson(json!({"error": format!("Failed to geocode start address: {}", e)})),
            ));
        }
    };

    // Get ending coordinates
    let (end_lat, end_lon, _end_formatted) = match crate::utils::tool_exec::get_coordinates(&client, &request.end_address, &geoapify_api_key).await {
        Ok(coords) => coords,
        Err(e) => {
            return Err((
                StatusCode::BAD_REQUEST,
                AxumJson(json!({"error": format!("Failed to geocode end address: {}", e)})),
            ));
        }
    };

    // Normalize mode: map "public transport" to "transit", and validate others
    let api_mode = match request.mode.to_lowercase().as_str() {
        "driving" => "driving",
        "walking" => "walking",
        "public transport" => "transit",
        "transit" => "transit",
        "bicycling" => "bicycling",
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                AxumJson(json!({"error": "Invalid mode. Supported: driving, walking, public transport (or transit), bicycling"})),
            ));
        }
    };

    // Call Google Maps Directions API with coordinates and mode
    let directions_url = format!(
        "https://maps.googleapis.com/maps/api/directions/json?origin={},{}&destination={},{}&mode={}&key={}",
        start_lat, start_lon, end_lat, end_lon, api_mode, google_maps_api_key
    );

    let directions_response: Value = match client.get(&directions_url).send().await {
        Ok(res) => match res.json().await {
            Ok(json) => json,
            Err(e) => {
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    AxumJson(json!({"error": format!("Failed to parse directions response: {}", e)})),
                ));
            }
        },
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": format!("Failed to fetch directions: {}", e)})),
            ));
        }
    };

    // Check for API errors
    if directions_response["status"].as_str() != Some("OK") {
        let error_message = directions_response["error_message"].as_str().unwrap_or("Unknown error");
        return Err((
            StatusCode::BAD_REQUEST,
            AxumJson(json!({"error": format!("Directions API error: {}", error_message)})),
        ));
    }

    // Extract total duration and distance from the first leg
    let mut duration = String::from("Unknown");
    let mut distance = String::from("Unknown");
    let mut instructions: Vec<String> = Vec::new();

    if let Some(routes) = directions_response["routes"].as_array() {
        if let Some(first_route) = routes.first() {
            if let Some(legs) = first_route["legs"].as_array() {
                if let Some(first_leg) = legs.first() {
                    if let Some(dur) = first_leg["duration"]["text"].as_str() {
                        duration = dur.to_string();
                    }
                    if let Some(dist) = first_leg["distance"]["text"].as_str() {
                        distance = dist.to_string();
                    }
                    if let Some(steps) = first_leg["steps"].as_array() {
                        for step in steps {
                            if let Some(html_instr) = step["html_instructions"].as_str() {
                                // Simple HTML stripping for plain text
                                let plain_text = html_instr
                                    .replace("<b>", "")
                                    .replace("</b>", "")
                                    .replace("<div style=\"font-size:0.9em\">", " - ")
                                    .replace("</div>", "")
                                    .replace("<wbr/>", "");
                                instructions.push(plain_text);
                            }
                        }
                    }
                }
            }
        }
    }

    if instructions.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            AxumJson(json!({"error": "No directions found"})),
        ));
    }

    Ok(AxumJson(json!({
        "duration": duration,
        "distance": distance,
        "instructions": instructions
    })))
}
