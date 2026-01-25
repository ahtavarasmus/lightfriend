use reqwest;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::time::Duration;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TeslaVehicle {
    pub id: i64,
    pub vehicle_id: i64,
    pub vin: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub charge_state: Option<ChargeState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub climate_state: Option<ClimateState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vehicle_state: Option<VehicleState>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ChargeState {
    pub battery_level: i32,
    pub battery_range: f64,
    pub charge_limit_soc: i32,
    pub charging_state: String,
    pub minutes_to_full_charge: Option<i32>,
    pub charge_rate: Option<f64>,   // Miles/hr or km/hr charging rate
    pub charger_power: Option<i32>, // kW being delivered
    pub time_to_full_charge: Option<f64>, // Hours (float, more precise than minutes)
    pub charge_energy_added: Option<f64>, // kWh added in current charging session
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ClimateState {
    pub inside_temp: Option<f64>,
    pub outside_temp: Option<f64>,
    pub driver_temp_setting: Option<f64>,
    pub passenger_temp_setting: Option<f64>,
    pub is_climate_on: Option<bool>,
    pub is_auto_conditioning_on: Option<bool>,
    pub is_preconditioning: Option<bool>,
    pub is_front_defroster_on: Option<bool>,
    pub is_rear_defroster_on: Option<bool>,
    pub fan_status: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct VehicleState {
    pub locked: Option<bool>,
    pub odometer: Option<f64>,
    pub car_version: Option<String>,
    pub is_user_present: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct VehiclesResponse {
    pub response: Vec<TeslaVehicle>,
}

#[derive(Debug, Deserialize)]
pub struct VehicleDataResponse {
    pub response: TeslaVehicle,
}

#[derive(Debug, Deserialize)]
pub struct CommandResponse {
    pub response: CommandResult,
}

#[derive(Debug, Deserialize)]
pub struct CommandResult {
    pub result: bool,
    pub reason: Option<String>,
}

// Nearby charging sites response structures
#[derive(Debug, Deserialize)]
pub struct NearbySitesResponse {
    pub response: NearbySitesData,
}

#[derive(Debug, Deserialize, Clone)]
pub struct NearbySitesData {
    pub superchargers: Vec<ChargingSite>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ChargingSite {
    pub location: ChargingLocation,
    pub name: String,
    pub distance_miles: f64,
    #[serde(default)]
    pub id: Option<i64>, // Supercharger ID for navigation_sc_request
}

#[derive(Debug, Deserialize, Clone)]
pub struct ChargingLocation {
    pub lat: f64,
    pub long: f64,
}

pub struct TeslaClient {
    client: reqwest::Client,
    base_url: String,
    proxy_url: Option<String>,
    proxy_client: Option<reqwest::Client>,
}

impl TeslaClient {
    pub fn new_with_region(region: &str) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(60))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            base_url: region.to_string(),
            proxy_url: None,
            proxy_client: None,
        }
    }

    pub fn new_with_proxy(region: &str) -> Self {
        // Try to get proxy URL from environment
        let proxy_url = std::env::var("TESLA_HTTP_PROXY_URL")
            .ok()
            .filter(|s| !s.is_empty());

        // Build proxy client if URL is provided
        let proxy_client = proxy_url.as_ref().map(|_| {
            // Create client that accepts self-signed certificates for the internal proxy
            // The proxy is internal-only (localhost) and uses self-signed certs
            // Tesla's security requirement is for the public key endpoint, not the proxy
            // 95 second timeout - slightly higher than proxy's 90s timeout
            let builder = reqwest::Client::builder()
                .timeout(Duration::from_secs(95))
                .danger_accept_invalid_certs(true);

            tracing::info!(
                "Tesla proxy client configured to accept internal self-signed certificates"
            );

            builder.build().unwrap_or_else(|_| reqwest::Client::new())
        });

        if let Some(ref url) = proxy_url {
            tracing::info!("Tesla proxy enabled at: {}", url);
        } else {
            tracing::warn!("Tesla proxy not configured - write commands will fail. Set TESLA_HTTP_PROXY_URL environment variable.");
        }

        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(60))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            base_url: region.to_string(),
            proxy_url,
            proxy_client,
        }
    }

    // Register the app in the current region (required by Tesla)
    // This requires a partner authentication token, not a user token
    pub async fn register_in_region(&self) -> Result<bool, Box<dyn Error>> {
        use crate::handlers::tesla_auth::get_partner_access_token;

        // Get partner token (app-level authentication)
        let partner_token = get_partner_access_token().await?;

        // Get domain from environment variable and strip protocol
        // Use TESLA_REDIRECT_URL for registration (must match the domain used in virtual key pairing)
        let domain = std::env::var("TESLA_REDIRECT_URL")
            .or_else(|_| std::env::var("SERVER_URL"))
            .or_else(|_| std::env::var("SERVER_URL_OAUTH"))
            .unwrap_or_else(|_| "localhost:3000".to_string());

        // Remove protocol (https:// or http://) if present
        let domain = domain
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .to_string();

        let url = format!("{}/api/1/partner_accounts", self.base_url);

        tracing::info!(
            "Attempting to register app in region: {} with domain: {}",
            self.base_url,
            domain
        );

        // Tesla requires domain in the request body
        let body = serde_json::json!({
            "domain": domain
        });

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", partner_token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        let response_text = response.text().await?;

        tracing::info!("Registration response ({}): {}", status, response_text);

        if status.is_success() {
            tracing::info!("Successfully registered app in region");
            Ok(true)
        } else if response_text.contains("already registered") {
            tracing::info!("App already registered in region");
            Ok(true)
        } else {
            tracing::error!("Failed to register app: {}", response_text);
            Err(format!("Registration failed: {}", response_text).into())
        }
    }

    // Get list of vehicles
    pub async fn get_vehicles(
        &self,
        access_token: &str,
    ) -> Result<Vec<TeslaVehicle>, Box<dyn Error>> {
        let url = format!("{}/api/1/vehicles", self.base_url);

        let response = self
            .client
            .get(&url)
            .bearer_auth(access_token)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("Tesla API error: {}", error_text).into());
        }

        let vehicles_response: VehiclesResponse = response.json().await?;
        Ok(vehicles_response.response)
    }

    // Get vehicle data including charge, climate, and vehicle state
    pub async fn get_vehicle_data(
        &self,
        access_token: &str,
        vehicle_id: &str,
    ) -> Result<TeslaVehicle, Box<dyn Error>> {
        let url = format!(
            "{}/api/1/vehicles/{}/vehicle_data",
            self.base_url, vehicle_id
        );

        let response = self
            .client
            .get(&url)
            .bearer_auth(access_token)
            .query(&[("endpoints", "charge_state;climate_state;vehicle_state")])
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("Tesla API error: {}", error_text).into());
        }

        let vehicle_data: VehicleDataResponse = response.json().await?;
        Ok(vehicle_data.response)
    }

    // Get vehicle climate data
    pub async fn get_vehicle_climate_data(
        &self,
        access_token: &str,
        vehicle_id: &str,
    ) -> Result<Option<ClimateState>, Box<dyn Error>> {
        let url = format!(
            "{}/api/1/vehicles/{}/vehicle_data",
            self.base_url, vehicle_id
        );

        let response = self
            .client
            .get(&url)
            .bearer_auth(access_token)
            .query(&[("endpoints", "climate_state")])
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("Tesla API error: {}", error_text).into());
        }

        let vehicle_data: VehicleDataResponse = response.json().await?;
        Ok(vehicle_data.response.climate_state)
    }

    // Lock vehicle
    pub async fn lock_vehicle(
        &self,
        access_token: &str,
        vehicle_id: &str,
    ) -> Result<bool, Box<dyn Error>> {
        self.send_command(access_token, vehicle_id, "door_lock")
            .await
    }

    // Unlock vehicle
    pub async fn unlock_vehicle(
        &self,
        access_token: &str,
        vehicle_id: &str,
    ) -> Result<bool, Box<dyn Error>> {
        self.send_command(access_token, vehicle_id, "door_unlock")
            .await
    }

    // Start climate preconditioning
    pub async fn start_climate(
        &self,
        access_token: &str,
        vehicle_id: &str,
    ) -> Result<bool, Box<dyn Error>> {
        self.send_command(access_token, vehicle_id, "auto_conditioning_start")
            .await
    }

    // Stop climate preconditioning
    pub async fn stop_climate(
        &self,
        access_token: &str,
        vehicle_id: &str,
    ) -> Result<bool, Box<dyn Error>> {
        self.send_command(access_token, vehicle_id, "auto_conditioning_stop")
            .await
    }

    // Remote start drive
    pub async fn remote_start(
        &self,
        access_token: &str,
        vehicle_id: &str,
    ) -> Result<bool, Box<dyn Error>> {
        self.send_command(access_token, vehicle_id, "remote_start_drive")
            .await
    }

    // Set max defrost mode
    pub async fn set_max_defrost(
        &self,
        access_token: &str,
        vehicle_id: &str,
        on: bool,
    ) -> Result<bool, Box<dyn Error>> {
        let body = serde_json::json!({"on": on});
        self.send_command_with_body(access_token, vehicle_id, "set_preconditioning_max", &body)
            .await
    }

    // Set seat heater
    pub async fn set_seat_heater(
        &self,
        access_token: &str,
        vehicle_id: &str,
        heater: u8,
        level: u8,
    ) -> Result<bool, Box<dyn Error>> {
        let body = serde_json::json!({"heater": heater, "level": level});
        self.send_command_with_body(
            access_token,
            vehicle_id,
            "remote_seat_heater_request",
            &body,
        )
        .await
    }

    // Set steering wheel heater
    pub async fn set_steering_wheel_heater(
        &self,
        access_token: &str,
        vehicle_id: &str,
        on: bool,
    ) -> Result<bool, Box<dyn Error>> {
        let body = serde_json::json!({"on": on});
        self.send_command_with_body(
            access_token,
            vehicle_id,
            "remote_steering_wheel_heater_request",
            &body,
        )
        .await
    }

    // Comprehensive defrost - starts climate, enables max defrost, heats seats and steering wheel
    pub async fn defrost_vehicle(
        &self,
        access_token: &str,
        vehicle_id: &str,
    ) -> Result<String, Box<dyn Error>> {
        self.start_climate(access_token, vehicle_id).await?;

        self.set_max_defrost(access_token, vehicle_id, true).await?;

        let _ = self.set_seat_heater(access_token, vehicle_id, 0, 3).await;
        let _ = self.set_seat_heater(access_token, vehicle_id, 1, 3).await;
        let _ = self
            .set_steering_wheel_heater(access_token, vehicle_id, true)
            .await;

        Ok("Max defrost activated with heated front seats and steering wheel".to_string())
    }

    // Set cabin overheat protection on/off
    pub async fn set_cabin_overheat_protection(
        &self,
        access_token: &str,
        vehicle_id: &str,
        on: bool,
        fan_only: bool,
    ) -> Result<bool, Box<dyn Error>> {
        let body = serde_json::json!({
            "on": on,
            "fan_only": fan_only
        });
        self.send_command_with_body(
            access_token,
            vehicle_id,
            "set_cabin_overheat_protection",
            &body,
        )
        .await
    }

    // Set charging limit (percent must be 50-100)
    pub async fn set_charge_limit(
        &self,
        access_token: &str,
        vehicle_id: &str,
        percent: i32,
    ) -> Result<bool, Box<dyn Error>> {
        let body = serde_json::json!({"percent": percent});
        self.send_command_with_body(access_token, vehicle_id, "set_charge_limit", &body)
            .await
    }

    // Get nearby Tesla charging sites (Superchargers and Destination chargers)
    // Uses detail=true to get site IDs needed for navigation_sc_request
    pub async fn get_nearby_charging_sites(
        &self,
        access_token: &str,
        vehicle_id: &str,
    ) -> Result<NearbySitesData, Box<dyn Error>> {
        let url = format!(
            "{}/api/1/vehicles/{}/nearby_charging_sites?detail=true",
            self.base_url, vehicle_id
        );

        let response = self
            .client
            .get(&url)
            .bearer_auth(access_token)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!(
                "Tesla API error getting nearby charging sites: {}",
                error_text
            )
            .into());
        }

        let sites_response: NearbySitesResponse = response.json().await?;
        Ok(sites_response.response)
    }

    // Share a destination to start navigation immediately (uses the share endpoint)
    // This actually starts turn-by-turn navigation, unlike navigation_gps_request which only suggests
    pub async fn share_destination(
        &self,
        access_token: &str,
        vehicle_id: &str,
        destination: &str,
    ) -> Result<bool, Box<dyn Error>> {
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .to_string();

        let body = serde_json::json!({
            "type": "share_ext_content_raw",
            "locale": "en-US",
            "timestamp_ms": timestamp_ms,
            "value": {
                "android.intent.extra.TEXT": destination
            }
        });
        // Use direct API for share command
        self.send_command_direct_api(access_token, vehicle_id, "share", &body)
            .await
    }

    /// Navigate to a Supercharger using navigation_sc_request endpoint
    /// This should trigger battery preconditioning since Tesla knows it's a Supercharger
    /// Requires the Supercharger ID from nearby_charging_sites (with detail=true)
    pub async fn navigate_to_supercharger(
        &self,
        access_token: &str,
        vehicle_id: &str,
        supercharger_id: i64,
    ) -> Result<bool, Box<dyn Error>> {
        let body = serde_json::json!({
            "id": supercharger_id,
            "order": 1
        });
        self.send_command_direct_api(access_token, vehicle_id, "navigation_sc_request", &body)
            .await
    }

    /// Set a scheduled departure time to trigger preconditioning
    /// departure_time is minutes after midnight in vehicle local time (e.g., 480 = 8:00 AM)
    pub async fn set_scheduled_departure(
        &self,
        access_token: &str,
        vehicle_id: &str,
        departure_time: i32,
        preconditioning_enabled: bool,
    ) -> Result<bool, Box<dyn Error>> {
        let body = serde_json::json!({
            "enable": true,
            "departure_time": departure_time,
            "preconditioning_enabled": preconditioning_enabled,
            "preconditioning_weekdays_only": false
        });
        self.send_command_with_body(access_token, vehicle_id, "set_scheduled_departure", &body)
            .await
    }

    // Generic command sender
    async fn send_command(
        &self,
        access_token: &str,
        vehicle_id: &str,
        command: &str,
    ) -> Result<bool, Box<dyn Error>> {
        // Use proxy for signed commands if available, otherwise fall back to direct API
        let (client, base_url) = if let (Some(proxy_client), Some(proxy_url)) =
            (&self.proxy_client, &self.proxy_url)
        {
            tracing::info!(
                "Sending signed command '{}' via proxy to vehicle {}",
                command,
                vehicle_id
            );
            (proxy_client, proxy_url.as_str())
        } else {
            tracing::warn!("Proxy not available - attempting direct command (will likely fail with Protocol error)");
            (&self.client, self.base_url.as_str())
        };

        let url = format!(
            "{}/api/1/vehicles/{}/command/{}",
            base_url, vehicle_id, command
        );

        let response = client.post(&url).bearer_auth(access_token).send().await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("Tesla API error for command {}: {}", command, error_text).into());
        }

        let command_response: CommandResponse = response.json().await?;

        if !command_response.response.result {
            let reason = command_response
                .response
                .reason
                .unwrap_or_else(|| "Unknown error".to_string());
            return Err(format!("Command {} failed: {}", command, reason).into());
        }

        Ok(command_response.response.result)
    }

    // Generic command sender with JSON body
    async fn send_command_with_body(
        &self,
        access_token: &str,
        vehicle_id: &str,
        command: &str,
        body: &serde_json::Value,
    ) -> Result<bool, Box<dyn Error>> {
        let (client, base_url) = if let (Some(proxy_client), Some(proxy_url)) =
            (&self.proxy_client, &self.proxy_url)
        {
            tracing::info!(
                "Sending signed command '{}' via proxy to vehicle {}",
                command,
                vehicle_id
            );
            (proxy_client, proxy_url.as_str())
        } else {
            tracing::warn!("Proxy not available - attempting direct command (will likely fail with Protocol error)");
            (&self.client, self.base_url.as_str())
        };

        let url = format!(
            "{}/api/1/vehicles/{}/command/{}",
            base_url, vehicle_id, command
        );

        let response = client
            .post(&url)
            .bearer_auth(access_token)
            .json(body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("Tesla API error for command {}: {}", command, error_text).into());
        }

        let command_response: CommandResponse = response.json().await?;

        if !command_response.response.result {
            let reason = command_response
                .response
                .reason
                .unwrap_or_else(|| "Unknown error".to_string());
            return Err(format!("Command {} failed: {}", command, reason).into());
        }

        Ok(command_response.response.result)
    }

    // Direct API command sender - bypasses proxy for commands that don't require signing
    // Navigation commands must use direct API as the proxy doesn't support them
    async fn send_command_direct_api(
        &self,
        access_token: &str,
        vehicle_id: &str,
        command: &str,
        body: &serde_json::Value,
    ) -> Result<bool, Box<dyn Error>> {
        let url = format!(
            "{}/api/1/vehicles/{}/command/{}",
            self.base_url, vehicle_id, command
        );

        tracing::info!(
            "Sending direct API command '{}' to vehicle {} (bypassing proxy)",
            command,
            vehicle_id
        );

        let response = self
            .client
            .post(&url)
            .bearer_auth(access_token)
            .json(body)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("Tesla API error for command {}: {}", command, error_text).into());
        }

        let command_response: CommandResponse = response.json().await?;

        if !command_response.response.result {
            let reason = command_response
                .response
                .reason
                .unwrap_or_else(|| "Unknown error".to_string());
            return Err(format!("Command {} failed: {}", command, reason).into());
        }

        Ok(command_response.response.result)
    }

    /// Wake up vehicle with deduplication - prevents parallel wake attempts for the same vehicle.
    /// If another request is already waking this vehicle, this will wait for that result.
    pub async fn wake_up_deduplicated(
        &self,
        access_token: &str,
        vehicle_id: &str,
        waking_vehicles: &dashmap::DashMap<String, tokio::sync::broadcast::Sender<bool>>,
    ) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let vin = vehicle_id.to_string();

        // Check if vehicle is already being woken
        if let Some(sender) = waking_vehicles.get(&vin) {
            tracing::info!(
                "Vehicle {} is already being woken by another request, waiting...",
                vin
            );
            let mut receiver = sender.subscribe();
            drop(sender); // Release the read lock

            // Wait for the wake result from the other request
            match receiver.recv().await {
                Ok(result) => {
                    tracing::info!("Received wake result from other request: {}", result);
                    return if result {
                        Ok(true)
                    } else {
                        Err("Vehicle wake-up failed (from parallel request)".into())
                    };
                }
                Err(_) => {
                    // Channel closed, the other request may have failed/panicked
                    tracing::warn!("Wake broadcast channel closed, will retry wake");
                    // Fall through to try waking ourselves
                }
            }
        }

        // Create a broadcast channel to notify any parallel requests
        let (tx, _) = tokio::sync::broadcast::channel::<bool>(1);
        waking_vehicles.insert(vin.clone(), tx.clone());

        // Perform the actual wake-up
        let result = self.wake_up(access_token, vehicle_id).await;

        // Broadcast the result to any waiting requests
        let success = result.is_ok() && result.as_ref().map(|&r| r).unwrap_or(false);
        let _ = tx.send(success); // Ignore error if no receivers

        // Remove from the map
        waking_vehicles.remove(&vin);

        // Convert the error type
        result.map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { e.to_string().into() })
    }

    // Wake up vehicle (if needed before sending commands)
    pub async fn wake_up(
        &self,
        access_token: &str,
        vehicle_id: &str,
    ) -> Result<bool, Box<dyn Error>> {
        const POLL_INTERVAL_SECS: u64 = 2;
        const MAX_ATTEMPTS: u32 = 23;

        tracing::info!("Waking up vehicle {}", vehicle_id);

        let (client, base_url) =
            if let (Some(proxy_client), Some(proxy_url)) = (&self.proxy_client, &self.proxy_url) {
                (proxy_client, proxy_url.as_str())
            } else {
                (&self.client, self.base_url.as_str())
            };

        let url = format!("{}/api/1/vehicles/{}/wake_up", base_url, vehicle_id);

        let response = client.post(&url).bearer_auth(access_token).send().await?;

        let status = response.status();

        if !status.is_success() {
            let error_text = response.text().await?;
            tracing::error!("Wake-up failed with status {}: {}", status, error_text);
            return Err(format!("Failed to wake vehicle: {}", error_text).into());
        }

        let response_text = response.text().await?;
        tracing::debug!("Wake-up response: {}", response_text);

        let response_json: serde_json::Value = serde_json::from_str(&response_text)
            .map_err(|e| format!("Failed to parse wake-up response: {}", e))?;

        let initial_state = response_json["response"]["state"]
            .as_str()
            .unwrap_or("unknown");

        tracing::info!(
            "Initial vehicle state after wake command: {}",
            initial_state
        );

        if initial_state == "online" {
            return Ok(true);
        }

        tracing::info!(
            "Vehicle is waking up, polling for online state (up to {} seconds)...",
            POLL_INTERVAL_SECS * MAX_ATTEMPTS as u64
        );

        for attempt in 1..=MAX_ATTEMPTS {
            tokio::time::sleep(tokio::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;

            let vehicles = self.get_vehicles(access_token).await?;

            if let Some(vehicle) = vehicles
                .iter()
                .find(|v| v.id.to_string() == vehicle_id || v.vin == vehicle_id)
            {
                tracing::debug!(
                    "Poll attempt {}/{}: vehicle state = {}",
                    attempt,
                    MAX_ATTEMPTS,
                    vehicle.state
                );

                if vehicle.state == "online" {
                    tracing::info!(
                        "Vehicle is now online after {} seconds",
                        (attempt as u64) * POLL_INTERVAL_SECS
                    );
                    return Ok(true);
                }
            }
        }

        tracing::error!(
            "Vehicle failed to wake up after {} seconds",
            MAX_ATTEMPTS as u64 * POLL_INTERVAL_SECS
        );
        Err("Vehicle didn't respond after 46 seconds. This may be due to poor cellular reception at the vehicle or Tesla server issues.".into())
    }

    // Monitor climate until ready to drive
    pub async fn monitor_climate_ready(
        &self,
        access_token: &str,
        vehicle_id: &str,
    ) -> Result<Option<f64>, Box<dyn Error>> {
        const POLL_INTERVAL_SECS: u64 = 60;
        const MAX_DURATION_SECS: u64 = 20 * 60;
        const MIN_RUNTIME_SECS: u64 = 5 * 60;
        const TEMP_THRESHOLD_DIFF: f64 = 3.0;
        const MIN_COMFORTABLE_TEMP: f64 = 15.0;

        let start_time = std::time::Instant::now();

        loop {
            let elapsed = start_time.elapsed().as_secs();

            if elapsed > MAX_DURATION_SECS {
                tracing::warn!(
                    "Climate monitoring timed out after {} minutes",
                    MAX_DURATION_SECS / 60
                );
                return Ok(None);
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;

            match self
                .get_vehicle_climate_data(access_token, vehicle_id)
                .await
            {
                Ok(Some(climate)) => {
                    let is_climate_on = climate.is_climate_on.unwrap_or(false);

                    if !is_climate_on {
                        tracing::info!("Climate was turned off manually");
                        return Err("Climate turned off before reaching target temperature".into());
                    }

                    if let (Some(inside_temp), Some(target_temp)) =
                        (climate.inside_temp, climate.driver_temp_setting)
                    {
                        let elapsed_mins = elapsed / 60;
                        let temp_diff = target_temp - inside_temp;

                        tracing::debug!(
                            "Climate check: inside={}°C, target={}°C, diff={}°C, runtime={}min",
                            inside_temp,
                            target_temp,
                            temp_diff,
                            elapsed_mins
                        );

                        let temp_is_ready = (temp_diff <= TEMP_THRESHOLD_DIFF)
                            || (inside_temp >= MIN_COMFORTABLE_TEMP);
                        let runtime_is_ready = elapsed >= MIN_RUNTIME_SECS;

                        if temp_is_ready && runtime_is_ready {
                            tracing::info!("Vehicle is ready to drive: temp={}°C (target={}°C) after {} minutes",
                                inside_temp, target_temp, elapsed_mins);
                            return Ok(Some(inside_temp));
                        }
                    } else {
                        tracing::warn!("Missing temperature data in climate state");
                    }
                }
                Ok(None) => {
                    tracing::warn!("No climate state data available");
                }
                Err(e) => {
                    tracing::warn!("Error fetching climate data: {}, continuing...", e);
                }
            }
        }
    }

    // Monitor charging until complete
    pub async fn monitor_charging_complete(
        &self,
        access_token: &str,
        vehicle_id: &str,
    ) -> Result<Option<i32>, Box<dyn Error>> {
        const POLL_INTERVAL_SECS: u64 = 5 * 60; // Poll every 5 minutes (charging takes hours)
        const MAX_DURATION_SECS: u64 = 12 * 60 * 60; // Max 12 hours

        let start_time = std::time::Instant::now();

        loop {
            let elapsed = start_time.elapsed().as_secs();

            if elapsed > MAX_DURATION_SECS {
                tracing::warn!(
                    "Charging monitoring timed out after {} hours",
                    MAX_DURATION_SECS / 3600
                );
                return Ok(None);
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(POLL_INTERVAL_SECS)).await;

            match self.get_vehicle_data(access_token, vehicle_id).await {
                Ok(vehicle) => {
                    if let Some(charge_state) = vehicle.charge_state {
                        let charging_state = charge_state.charging_state.as_str();
                        let battery_level = charge_state.battery_level;

                        tracing::debug!(
                            "Charging check: state={}, battery={}%",
                            charging_state,
                            battery_level
                        );

                        match charging_state {
                            "Complete" => {
                                tracing::info!("Charging complete at {}%", battery_level);
                                return Ok(Some(battery_level));
                            }
                            "Stopped" | "Disconnected" | "NoPower" => {
                                tracing::info!(
                                    "Charging stopped ({}), battery at {}%",
                                    charging_state,
                                    battery_level
                                );
                                return Ok(Some(battery_level));
                            }
                            "Charging" => {
                                // Still charging, continue monitoring
                            }
                            _ => {
                                tracing::warn!("Unknown charging state: {}", charging_state);
                            }
                        }
                    } else {
                        tracing::warn!("No charge state data available");
                    }
                }
                Err(e) => {
                    tracing::warn!("Error fetching vehicle data: {}, continuing...", e);
                }
            }
        }
    }
}
