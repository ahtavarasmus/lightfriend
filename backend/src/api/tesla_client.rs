use async_trait::async_trait;
use std::collections::VecDeque;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::api::tesla::{ChargeState, ClimateState, NearbySitesData, TeslaVehicle, VehicleState};

/// Trait for Tesla API operations, enabling mock implementations for testing.
#[async_trait]
pub trait TeslaClientInterface: Send + Sync {
    /// Get list of vehicles associated with the account
    async fn get_vehicles(
        &self,
        access_token: &str,
    ) -> Result<Vec<TeslaVehicle>, Box<dyn Error + Send + Sync>>;

    /// Get detailed vehicle data including charge, climate, and vehicle state
    async fn get_vehicle_data(
        &self,
        access_token: &str,
        vehicle_id: &str,
    ) -> Result<TeslaVehicle, Box<dyn Error + Send + Sync>>;

    /// Get only climate state data
    async fn get_vehicle_climate_data(
        &self,
        access_token: &str,
        vehicle_id: &str,
    ) -> Result<Option<ClimateState>, Box<dyn Error + Send + Sync>>;

    /// Lock the vehicle
    async fn lock_vehicle(
        &self,
        access_token: &str,
        vehicle_id: &str,
    ) -> Result<bool, Box<dyn Error + Send + Sync>>;

    /// Unlock the vehicle
    async fn unlock_vehicle(
        &self,
        access_token: &str,
        vehicle_id: &str,
    ) -> Result<bool, Box<dyn Error + Send + Sync>>;

    /// Start climate preconditioning
    async fn start_climate(
        &self,
        access_token: &str,
        vehicle_id: &str,
    ) -> Result<bool, Box<dyn Error + Send + Sync>>;

    /// Stop climate preconditioning
    async fn stop_climate(
        &self,
        access_token: &str,
        vehicle_id: &str,
    ) -> Result<bool, Box<dyn Error + Send + Sync>>;

    /// Remote start drive
    async fn remote_start(
        &self,
        access_token: &str,
        vehicle_id: &str,
    ) -> Result<bool, Box<dyn Error + Send + Sync>>;

    /// Set max defrost mode
    async fn set_max_defrost(
        &self,
        access_token: &str,
        vehicle_id: &str,
        on: bool,
    ) -> Result<bool, Box<dyn Error + Send + Sync>>;

    /// Set seat heater level
    async fn set_seat_heater(
        &self,
        access_token: &str,
        vehicle_id: &str,
        heater: u8,
        level: u8,
    ) -> Result<bool, Box<dyn Error + Send + Sync>>;

    /// Set steering wheel heater
    async fn set_steering_wheel_heater(
        &self,
        access_token: &str,
        vehicle_id: &str,
        on: bool,
    ) -> Result<bool, Box<dyn Error + Send + Sync>>;

    /// Comprehensive defrost - starts climate, enables max defrost, heats seats and steering wheel
    async fn defrost_vehicle(
        &self,
        access_token: &str,
        vehicle_id: &str,
    ) -> Result<String, Box<dyn Error + Send + Sync>>;

    /// Set cabin overheat protection
    async fn set_cabin_overheat_protection(
        &self,
        access_token: &str,
        vehicle_id: &str,
        on: bool,
        fan_only: bool,
    ) -> Result<bool, Box<dyn Error + Send + Sync>>;

    /// Set charging limit (percent must be 50-100)
    async fn set_charge_limit(
        &self,
        access_token: &str,
        vehicle_id: &str,
        percent: i32,
    ) -> Result<bool, Box<dyn Error + Send + Sync>>;

    /// Get nearby Tesla charging sites (Superchargers and Destination chargers)
    async fn get_nearby_charging_sites(
        &self,
        access_token: &str,
        vehicle_id: &str,
    ) -> Result<NearbySitesData, Box<dyn Error + Send + Sync>>;

    /// Share a destination to start navigation immediately
    async fn share_destination(
        &self,
        access_token: &str,
        vehicle_id: &str,
        destination: &str,
    ) -> Result<bool, Box<dyn Error + Send + Sync>>;

    /// Navigate to a Supercharger using navigation_sc_request endpoint
    async fn navigate_to_supercharger(
        &self,
        access_token: &str,
        vehicle_id: &str,
        supercharger_id: i64,
    ) -> Result<bool, Box<dyn Error + Send + Sync>>;

    /// Set scheduled departure time
    async fn set_scheduled_departure(
        &self,
        access_token: &str,
        vehicle_id: &str,
        departure_time: i32,
        preconditioning_enabled: bool,
    ) -> Result<bool, Box<dyn Error + Send + Sync>>;

    /// Wake up vehicle (if needed before sending commands)
    async fn wake_up(
        &self,
        access_token: &str,
        vehicle_id: &str,
    ) -> Result<bool, Box<dyn Error + Send + Sync>>;

    /// Wake up vehicle with deduplication - prevents parallel wake attempts for the same vehicle
    async fn wake_up_deduplicated(
        &self,
        access_token: &str,
        vehicle_id: &str,
        waking_vehicles: &dashmap::DashMap<String, tokio::sync::broadcast::Sender<bool>>,
    ) -> Result<bool, Box<dyn Error + Send + Sync>>;

    /// Monitor climate until ready to drive
    async fn monitor_climate_ready(
        &self,
        access_token: &str,
        vehicle_id: &str,
    ) -> Result<Option<f64>, Box<dyn Error + Send + Sync>>;

    /// Monitor charging until complete
    async fn monitor_charging_complete(
        &self,
        access_token: &str,
        vehicle_id: &str,
    ) -> Result<Option<i32>, Box<dyn Error + Send + Sync>>;
}

// Pure functions for testability - no external dependencies

/// Determine if vehicle needs to be woken up based on state
pub fn should_wake_vehicle(state: &str) -> bool {
    state != "online"
}

/// Calculate retry delay for wake attempts using exponential backoff
pub fn wake_retry_delay(attempt: u32) -> std::time::Duration {
    std::time::Duration::from_secs(2u64.pow(attempt.min(5)))
}

/// Check if climate has reached a comfortable temperature
/// Returns true if inside temp is within tolerance of target, or above minimum comfortable temp
pub fn is_climate_ready(inside_temp: f64, target_temp: f64, tolerance: f64) -> bool {
    (target_temp - inside_temp) <= tolerance || inside_temp >= 15.0
}

/// Check if charging is effectively complete
/// Battery at or above charge limit means charging is done
pub fn is_charging_complete(battery_level: i32, charge_limit: i32) -> bool {
    battery_level >= charge_limit
}

/// Parse charging state to determine if vehicle is actively charging
pub fn is_actively_charging(charging_state: &str) -> bool {
    charging_state == "Charging"
}

/// Determine if charging has stopped (complete, disconnected, or no power)
pub fn is_charging_stopped(charging_state: &str) -> bool {
    matches!(
        charging_state,
        "Complete" | "Stopped" | "Disconnected" | "NoPower"
    )
}

/// Format battery status message
pub fn format_battery_status(
    vehicle_name: &str,
    battery_level: i32,
    battery_range: f64,
    charge_limit: i32,
    charging_state: &str,
    minutes_to_full: Option<i32>,
) -> String {
    let charging_status = if charging_state == "Charging" {
        format!(
            " Currently charging, {} minutes to full.",
            minutes_to_full.unwrap_or(0)
        )
    } else {
        String::new()
    };

    format!(
        "Your {} battery is at {}% with {:.0} miles of range. Charge limit set to {}%.{}",
        vehicle_name, battery_level, battery_range, charge_limit, charging_status
    )
}

// Mock implementation for testing (no cfg(test) so it's available to integration tests)
pub mod mock {
    use super::*;
    use std::collections::HashMap;

    /// Enum representing Tesla commands for logging in mock
    #[derive(Debug, Clone, PartialEq)]
    pub enum TeslaCommand {
        Lock,
        Unlock,
        StartClimate,
        StopClimate,
        RemoteStart,
        SetMaxDefrost(bool),
        SetSeatHeater { heater: u8, level: u8 },
        SetSteeringWheelHeater(bool),
        Defrost,
        SetCabinOverheatProtection { on: bool, fan_only: bool },
        SetChargeLimit(i32),
        ShareDestination(String),
        NavigateToSupercharger(i64),
        SetScheduledDeparture { time: i32, preconditioning: bool },
        WakeUp,
    }

    /// Mock Tesla client for testing
    pub struct MockTeslaClient {
        pub vehicles: Vec<TeslaVehicle>,
        pub vehicle_data: HashMap<String, TeslaVehicle>,
        pub wake_responses: Arc<Mutex<VecDeque<bool>>>,
        pub command_log: Arc<Mutex<Vec<TeslaCommand>>>,
        pub nearby_sites: Option<NearbySitesData>,
        pub command_should_fail: bool,
    }

    impl MockTeslaClient {
        pub fn new() -> Self {
            Self {
                vehicles: Vec::new(),
                vehicle_data: HashMap::new(),
                wake_responses: Arc::new(Mutex::new(VecDeque::new())),
                command_log: Arc::new(Mutex::new(Vec::new())),
                nearby_sites: None,
                command_should_fail: false,
            }
        }

        pub fn with_vehicle(mut self, vehicle: TeslaVehicle) -> Self {
            let vin = vehicle.vin.clone();
            self.vehicles.push(vehicle.clone());
            self.vehicle_data.insert(vin, vehicle);
            self
        }

        pub fn with_wake_sequence(self, responses: Vec<bool>) -> Self {
            let wake_responses = Arc::new(Mutex::new(VecDeque::from(responses)));
            Self {
                wake_responses,
                ..self
            }
        }

        pub fn with_nearby_sites(mut self, sites: NearbySitesData) -> Self {
            self.nearby_sites = Some(sites);
            self
        }

        pub fn with_failing_commands(mut self) -> Self {
            self.command_should_fail = true;
            self
        }

        pub async fn get_commands(&self) -> Vec<TeslaCommand> {
            self.command_log.lock().await.clone()
        }
    }

    impl Default for MockTeslaClient {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl TeslaClientInterface for MockTeslaClient {
        async fn get_vehicles(
            &self,
            _access_token: &str,
        ) -> Result<Vec<TeslaVehicle>, Box<dyn Error + Send + Sync>> {
            Ok(self.vehicles.clone())
        }

        async fn get_vehicle_data(
            &self,
            _access_token: &str,
            vehicle_id: &str,
        ) -> Result<TeslaVehicle, Box<dyn Error + Send + Sync>> {
            self.vehicle_data
                .get(vehicle_id)
                .cloned()
                .ok_or_else(|| format!("Vehicle {} not found", vehicle_id).into())
        }

        async fn get_vehicle_climate_data(
            &self,
            _access_token: &str,
            vehicle_id: &str,
        ) -> Result<Option<ClimateState>, Box<dyn Error + Send + Sync>> {
            Ok(self
                .vehicle_data
                .get(vehicle_id)
                .and_then(|v| v.climate_state.clone()))
        }

        async fn lock_vehicle(
            &self,
            _access_token: &str,
            _vehicle_id: &str,
        ) -> Result<bool, Box<dyn Error + Send + Sync>> {
            self.command_log.lock().await.push(TeslaCommand::Lock);
            Ok(!self.command_should_fail)
        }

        async fn unlock_vehicle(
            &self,
            _access_token: &str,
            _vehicle_id: &str,
        ) -> Result<bool, Box<dyn Error + Send + Sync>> {
            self.command_log.lock().await.push(TeslaCommand::Unlock);
            Ok(!self.command_should_fail)
        }

        async fn start_climate(
            &self,
            _access_token: &str,
            _vehicle_id: &str,
        ) -> Result<bool, Box<dyn Error + Send + Sync>> {
            self.command_log
                .lock()
                .await
                .push(TeslaCommand::StartClimate);
            Ok(!self.command_should_fail)
        }

        async fn stop_climate(
            &self,
            _access_token: &str,
            _vehicle_id: &str,
        ) -> Result<bool, Box<dyn Error + Send + Sync>> {
            self.command_log
                .lock()
                .await
                .push(TeslaCommand::StopClimate);
            Ok(!self.command_should_fail)
        }

        async fn remote_start(
            &self,
            _access_token: &str,
            _vehicle_id: &str,
        ) -> Result<bool, Box<dyn Error + Send + Sync>> {
            self.command_log
                .lock()
                .await
                .push(TeslaCommand::RemoteStart);
            Ok(!self.command_should_fail)
        }

        async fn set_max_defrost(
            &self,
            _access_token: &str,
            _vehicle_id: &str,
            on: bool,
        ) -> Result<bool, Box<dyn Error + Send + Sync>> {
            self.command_log
                .lock()
                .await
                .push(TeslaCommand::SetMaxDefrost(on));
            Ok(!self.command_should_fail)
        }

        async fn set_seat_heater(
            &self,
            _access_token: &str,
            _vehicle_id: &str,
            heater: u8,
            level: u8,
        ) -> Result<bool, Box<dyn Error + Send + Sync>> {
            self.command_log
                .lock()
                .await
                .push(TeslaCommand::SetSeatHeater { heater, level });
            Ok(!self.command_should_fail)
        }

        async fn set_steering_wheel_heater(
            &self,
            _access_token: &str,
            _vehicle_id: &str,
            on: bool,
        ) -> Result<bool, Box<dyn Error + Send + Sync>> {
            self.command_log
                .lock()
                .await
                .push(TeslaCommand::SetSteeringWheelHeater(on));
            Ok(!self.command_should_fail)
        }

        async fn defrost_vehicle(
            &self,
            _access_token: &str,
            _vehicle_id: &str,
        ) -> Result<String, Box<dyn Error + Send + Sync>> {
            self.command_log.lock().await.push(TeslaCommand::Defrost);
            if self.command_should_fail {
                Err("Defrost failed".into())
            } else {
                Ok("Max defrost activated with heated front seats and steering wheel".to_string())
            }
        }

        async fn set_cabin_overheat_protection(
            &self,
            _access_token: &str,
            _vehicle_id: &str,
            on: bool,
            fan_only: bool,
        ) -> Result<bool, Box<dyn Error + Send + Sync>> {
            self.command_log
                .lock()
                .await
                .push(TeslaCommand::SetCabinOverheatProtection { on, fan_only });
            Ok(!self.command_should_fail)
        }

        async fn set_charge_limit(
            &self,
            _access_token: &str,
            _vehicle_id: &str,
            percent: i32,
        ) -> Result<bool, Box<dyn Error + Send + Sync>> {
            self.command_log
                .lock()
                .await
                .push(TeslaCommand::SetChargeLimit(percent));
            Ok(!self.command_should_fail)
        }

        async fn get_nearby_charging_sites(
            &self,
            _access_token: &str,
            _vehicle_id: &str,
        ) -> Result<NearbySitesData, Box<dyn Error + Send + Sync>> {
            self.nearby_sites
                .clone()
                .ok_or_else(|| "No nearby sites configured".into())
        }

        async fn share_destination(
            &self,
            _access_token: &str,
            _vehicle_id: &str,
            destination: &str,
        ) -> Result<bool, Box<dyn Error + Send + Sync>> {
            self.command_log
                .lock()
                .await
                .push(TeslaCommand::ShareDestination(destination.to_string()));
            Ok(!self.command_should_fail)
        }

        async fn navigate_to_supercharger(
            &self,
            _access_token: &str,
            _vehicle_id: &str,
            supercharger_id: i64,
        ) -> Result<bool, Box<dyn Error + Send + Sync>> {
            self.command_log
                .lock()
                .await
                .push(TeslaCommand::NavigateToSupercharger(supercharger_id));
            Ok(!self.command_should_fail)
        }

        async fn set_scheduled_departure(
            &self,
            _access_token: &str,
            _vehicle_id: &str,
            departure_time: i32,
            preconditioning_enabled: bool,
        ) -> Result<bool, Box<dyn Error + Send + Sync>> {
            self.command_log
                .lock()
                .await
                .push(TeslaCommand::SetScheduledDeparture {
                    time: departure_time,
                    preconditioning: preconditioning_enabled,
                });
            Ok(!self.command_should_fail)
        }

        async fn wake_up(
            &self,
            _access_token: &str,
            _vehicle_id: &str,
        ) -> Result<bool, Box<dyn Error + Send + Sync>> {
            self.command_log.lock().await.push(TeslaCommand::WakeUp);

            let mut responses = self.wake_responses.lock().await;
            if let Some(result) = responses.pop_front() {
                Ok(result)
            } else {
                Ok(!self.command_should_fail)
            }
        }

        async fn wake_up_deduplicated(
            &self,
            access_token: &str,
            vehicle_id: &str,
            _waking_vehicles: &dashmap::DashMap<String, tokio::sync::broadcast::Sender<bool>>,
        ) -> Result<bool, Box<dyn Error + Send + Sync>> {
            // In mock, just call wake_up directly
            self.wake_up(access_token, vehicle_id).await
        }

        async fn monitor_climate_ready(
            &self,
            _access_token: &str,
            vehicle_id: &str,
        ) -> Result<Option<f64>, Box<dyn Error + Send + Sync>> {
            // Return immediately with current inside temp for testing
            if let Some(vehicle) = self.vehicle_data.get(vehicle_id) {
                if let Some(climate) = &vehicle.climate_state {
                    return Ok(climate.inside_temp);
                }
            }
            Ok(None)
        }

        async fn monitor_charging_complete(
            &self,
            _access_token: &str,
            vehicle_id: &str,
        ) -> Result<Option<i32>, Box<dyn Error + Send + Sync>> {
            // Return immediately with current battery level for testing
            if let Some(vehicle) = self.vehicle_data.get(vehicle_id) {
                if let Some(charge) = &vehicle.charge_state {
                    return Ok(Some(charge.battery_level));
                }
            }
            Ok(None)
        }
    }
}

// Helper function to create test vehicle data (no cfg(test) so it's available to integration tests)
pub fn create_test_vehicle(vin: &str, name: &str, state: &str) -> TeslaVehicle {
    TeslaVehicle {
        id: 12345,
        vehicle_id: 67890,
        vin: vin.to_string(),
        display_name: Some(name.to_string()),
        state: state.to_string(),
        charge_state: None,
        climate_state: None,
        vehicle_state: None,
    }
}

pub fn create_test_vehicle_with_data(
    vin: &str,
    name: &str,
    state: &str,
    battery_level: i32,
    inside_temp: f64,
) -> TeslaVehicle {
    TeslaVehicle {
        id: 12345,
        vehicle_id: 67890,
        vin: vin.to_string(),
        display_name: Some(name.to_string()),
        state: state.to_string(),
        charge_state: Some(ChargeState {
            battery_level,
            battery_range: 200.0,
            charge_limit_soc: 80,
            charging_state: "Stopped".to_string(),
            minutes_to_full_charge: None,
            charge_rate: None,
            charger_power: None,
            time_to_full_charge: None,
            charge_energy_added: None,
        }),
        climate_state: Some(ClimateState {
            inside_temp: Some(inside_temp),
            outside_temp: Some(5.0),
            driver_temp_setting: Some(21.0),
            passenger_temp_setting: Some(21.0),
            is_climate_on: Some(true),
            is_auto_conditioning_on: Some(true),
            is_preconditioning: Some(false),
            is_front_defroster_on: Some(false),
            is_rear_defroster_on: Some(false),
            fan_status: Some(3),
        }),
        vehicle_state: Some(VehicleState {
            locked: Some(true),
            odometer: Some(15000.0),
            car_version: Some("2024.8.7".to_string()),
            is_user_present: Some(false),
        }),
    }
}
