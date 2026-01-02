use std::sync::Arc;
use chrono::Timelike;
use serde_json::Value;
use tracing::{info, error};

use crate::{
    api::tesla::TeslaClient,
    handlers::tesla_auth::get_valid_tesla_access_token,
    AppState,
};

// Tool definition for switching between Tesla vehicles
pub fn get_tesla_switch_vehicle_tool() -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;

    let properties: HashMap<String, Box<types::JSONSchemaDefine>> = HashMap::new();

    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("switch_selected_tesla_vehicle"),
            description: Some(String::from(
                "Switch to the next Tesla vehicle in the user's account. Cycles through available vehicles (after the last vehicle, goes back to the first). Use this when the user wants to control a different Tesla vehicle.",
            )),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: None,
            },
        },
    }
}

// Handle Tesla vehicle switch tool call
pub async fn handle_tesla_switch_vehicle(
    state: &Arc<AppState>,
    user_id: i32,
) -> String {
    info!("Switching Tesla vehicle for user {}", user_id);

    // Check if user has Tier 2 subscription
    let user = match state.user_core.find_by_id(user_id) {
        Ok(Some(u)) => u,
        Ok(None) => return "Error: User not found".to_string(),
        Err(e) => {
            error!("Failed to get user: {}", e);
            return "Error: Failed to verify user".to_string();
        }
    };

    if user.sub_tier != Some("tier 2".to_string()) {
        return "Tesla control requires a Tier 2 (Sentinel) subscription. Please upgrade your plan to use this feature.".to_string();
    }

    // Check if user has Tesla connected
    let has_tesla = match state.user_repository.has_active_tesla(user_id) {
        Ok(has) => has,
        Err(e) => {
            error!("Failed to check Tesla connection: {}", e);
            return "Error: Failed to check Tesla connection".to_string();
        }
    };

    if !has_tesla {
        return "You haven't connected your Tesla account yet. Please connect it first in the app settings.".to_string();
    }

    // Get valid access token
    let access_token = match get_valid_tesla_access_token(state, user_id).await {
        Ok(token) => token,
        Err((_, msg)) => {
            error!("Failed to get Tesla access token: {}", msg);
            return format!("Error: Failed to authenticate with Tesla - {}", msg);
        }
    };

    // Get user's Tesla region
    let region = match state.user_repository.get_tesla_region(user_id) {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to get user's Tesla region: {}", e);
            return "Error: Failed to get your Tesla region settings".to_string();
        }
    };

    // Create Tesla client with user's region and proxy support
    let tesla_client = TeslaClient::new_with_proxy(&region);

    // Get all vehicles
    let vehicles = match tesla_client.get_vehicles(&access_token).await {
        Ok(v) => v,
        Err(e) => {
            error!("Failed to get vehicles: {}", e);
            return format!("Failed to get your vehicles: {}", e);
        }
    };

    if vehicles.is_empty() {
        return "No vehicles found on your Tesla account".to_string();
    }

    if vehicles.len() == 1 {
        let name = vehicles[0].display_name.as_deref().unwrap_or("your Tesla");
        return format!("You only have one Tesla vehicle: {}. No other vehicles to switch to.", name);
    }

    // Get currently selected vehicle VIN
    let selected_vin = state.user_repository
        .get_selected_vehicle_vin(user_id)
        .ok()
        .flatten();

    // Find current vehicle's index
    let current_index = if let Some(vin) = selected_vin.as_ref() {
        vehicles.iter().position(|v| &v.vin == vin).unwrap_or(0)
    } else {
        0
    };

    // Select next vehicle (cycle back to first after last)
    let next_index = (current_index + 1) % vehicles.len();
    let next_vehicle = &vehicles[next_index];
    let next_name = next_vehicle.display_name.as_deref().unwrap_or("Unknown");
    let next_vin = &next_vehicle.vin;
    let next_id = next_vehicle.id.to_string();

    // Update selection in database
    if let Err(e) = state.user_repository.set_selected_vehicle(
        user_id,
        next_vin.to_string(),
        next_name.to_string(),
        next_id.clone(),
    ) {
        error!("Failed to update selected vehicle: {}", e);
        return "Error: Failed to save vehicle selection".to_string();
    }

    info!("Switched user {} to vehicle: {} (VIN: {})", user_id, next_name, next_vin);

    // Build vehicle list string
    let vehicle_list: Vec<String> = vehicles.iter().enumerate().map(|(i, v)| {
        let name = v.display_name.as_deref().unwrap_or("Unknown");
        if i == next_index {
            format!("{} (selected)", name)
        } else {
            name.to_string()
        }
    }).collect();

    format!(
        "Switched to {}. You have {} vehicles: {}.",
        next_name,
        vehicles.len(),
        vehicle_list.join(", ")
    )
}

// Tool definition for OpenAI function calling
pub fn get_tesla_control_tool() -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;

    let mut properties = HashMap::new();

    properties.insert(
        "command".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Command to execute: 'lock', 'unlock', 'climate_on', 'climate_off', 'defrost', 'remote_start', 'charge_status', 'cabin_overheat_on', 'cabin_overheat_off', 'cabin_overheat_fan_only', or 'precondition_battery'".to_string()),
            enum_values: Some(vec![
                "lock".to_string(),
                "unlock".to_string(),
                "climate_on".to_string(),
                "climate_off".to_string(),
                "defrost".to_string(),
                "remote_start".to_string(),
                "charge_status".to_string(),
                "cabin_overheat_on".to_string(),
                "cabin_overheat_off".to_string(),
                "cabin_overheat_fan_only".to_string(),
                "precondition_battery".to_string(),
            ]),
            ..Default::default()
        }),
    );

    properties.insert(
        "notify_when_ready".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Boolean),
            description: Some("If true for climate_on or defrost commands, send SMS notification when car reaches comfortable temperature. Only set to true if user explicitly asks to be notified.".to_string()),
            ..Default::default()
        }),
    );

    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("control_tesla"),
            description: Some(String::from(
                "Control Tesla vehicle functions: lock/unlock doors, start/stop climate control, defrost vehicle (max heat + heated seats/steering wheel for deep ice), remote start driving, check charge status, control cabin overheat protection (on/off/fan-only), or precondition battery for fast charging (warms battery by setting nav to distant Supercharger - use when user is leaving within 30 min to charge). For climate_on/defrost, can optionally notify user when car is ready.",
            )),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(vec![String::from("command")]),
            },
        },
    }
}

// Handle Tesla tool call from AI assistant
// skip_notification: If true, don't send SMS notification when car wakes up (for dashboard calls)
pub async fn handle_tesla_command(
    state: &Arc<AppState>,
    user_id: i32,
    args: &str,
    skip_notification: bool,
) -> String {
    // Parse arguments
    let args_value: Value = match serde_json::from_str(args) {
        Ok(v) => v,
        Err(e) => {
            error!("Failed to parse Tesla command args: {}", e);
            return format!("Error: Invalid command format");
        }
    };

    let command = args_value["command"]
        .as_str()
        .unwrap_or("unknown");

    // Parse notify_when_ready - defaults to false (user must explicitly request notification)
    let notify_when_ready = args_value["notify_when_ready"]
        .as_bool()
        .unwrap_or(false);

    info!("Executing Tesla command '{}' for user {} (notify_when_ready: {})", command, user_id, notify_when_ready);

    // Check if user has Tier 2 subscription
    let user = match state.user_core.find_by_id(user_id) {
        Ok(Some(u)) => u,
        Ok(None) => return "Error: User not found".to_string(),
        Err(e) => {
            error!("Failed to get user: {}", e);
            return "Error: Failed to verify user".to_string();
        }
    };

    if user.sub_tier != Some("tier 2".to_string()) {
        return "Tesla control requires a Tier 2 (Sentinel) subscription. Please upgrade your plan to use this feature.".to_string();
    }

    // Check if user has Tesla connected
    let has_tesla = match state.user_repository.has_active_tesla(user_id) {
        Ok(has) => has,
        Err(e) => {
            error!("Failed to check Tesla connection: {}", e);
            return "Error: Failed to check Tesla connection".to_string();
        }
    };

    if !has_tesla {
        return "You haven't connected your Tesla account yet. Please connect it first in the app settings.".to_string();
    }

    // Get valid access token
    let access_token = match get_valid_tesla_access_token(state, user_id).await {
        Ok(token) => token,
        Err((_, msg)) => {
            error!("Failed to get Tesla access token: {}", msg);
            return format!("Error: Failed to authenticate with Tesla - {}", msg);
        }
    };

    // Get user's Tesla region
    let region = match state.user_repository.get_tesla_region(user_id) {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to get user's Tesla region: {}", e);
            return "Error: Failed to get your Tesla region settings".to_string();
        }
    };

    // Create Tesla client with user's region and proxy support
    let tesla_client = TeslaClient::new_with_proxy(&region);

    // Get all vehicles
    let vehicles = match tesla_client.get_vehicles(&access_token).await {
        Ok(v) => v,
        Err(e) => {
            error!("Failed to get vehicles: {}", e);
            return format!("Failed to get your vehicles: {}", e);
        }
    };

    if vehicles.is_empty() {
        return "No vehicles found on your Tesla account".to_string();
    }

    // Log found vehicles
    info!("Found {} vehicle(s) for user {}", vehicles.len(), user_id);
    for (i, v) in vehicles.iter().enumerate() {
        info!("Vehicle {}: {} (VIN: {}, State: {})", i + 1, v.display_name.as_deref().unwrap_or("Unknown"), v.vin, v.state);
    }

    // Try to use selected vehicle, fall back to first vehicle if none selected
    let selected_vin = state.user_repository
        .get_selected_vehicle_vin(user_id)
        .ok()
        .flatten();

    let vehicle = if let Some(vin) = selected_vin.as_ref() {
        match vehicles.iter().find(|v| &v.vin == vin) {
            Some(v) => {
                info!("Using selected vehicle with VIN: {}", vin);
                v
            }
            None => {
                info!("Selected vehicle VIN {} not found, falling back to first vehicle", vin);
                &vehicles[0]
            }
        }
    } else {
        info!("No vehicle selected, using first vehicle");
        &vehicles[0]
    };

    let vehicle_id = vehicle.id.to_string();
    let vehicle_vin = &vehicle.vin;  // VIN is required for signed commands
    let vehicle_name = vehicle.display_name.as_deref().unwrap_or("your Tesla");

    info!("Using vehicle: {} (ID: {}, VIN: {}, State: {})", vehicle_name, vehicle_id, vehicle_vin, vehicle.state);

    // Handle asleep vehicles: wake up first, then execute command
    // We wait for the full operation to complete so user gets a single response
    // Using deduplicated wake to prevent parallel wake attempts
    if command != "charge_status" && vehicle.state != "online" {
        info!("Vehicle is {}, waking up before executing command", vehicle.state);

        let wake_result = tesla_client.wake_up_deduplicated(&access_token, vehicle_vin, &state.tesla_waking_vehicles).await
            .map_err(|e| e.to_string());

        match wake_result {
            Ok(true) => {
                info!("Vehicle woke up successfully, executing command: {}", command);
                // Continue to execute command below
            }
            Ok(false) => {
                error!("Vehicle wake-up returned false (unexpected)");
                return format!("Couldn't wake up your {}. This may be a Tesla server or connectivity issue - the vehicle might have poor cellular reception.", vehicle_name);
            }
            Err(error_msg) => {
                error!("Failed to wake up vehicle: {}", error_msg);
                return format!("Couldn't reach your {}. This may be a Tesla server or connectivity issue. {}", vehicle_name, error_msg);
            }
        }
    }

    // Execute the command (vehicle is now online, either already was or just woke up)
    let result = execute_tesla_command(&tesla_client, &access_token, vehicle_vin, vehicle_name, command).await;

    // Spawn climate monitoring only if user explicitly requested notification
    if notify_when_ready && (command == "defrost" || command == "climate_on") {
        info!("User requested climate ready notification, starting monitoring");
        spawn_climate_monitoring(state, user_id, region, access_token, vehicle_vin.to_string(), vehicle_name.to_string());
    }

    result
}

async fn execute_tesla_command(
    tesla_client: &crate::api::tesla::TeslaClient,
    access_token: &str,
    vehicle_vin: &str,
    vehicle_name: &str,
    command: &str,
) -> String {
    match command {
        "lock" => {
            match tesla_client.lock_vehicle(&access_token, vehicle_vin).await {
                Ok(true) => format!("Successfully locked your {}", vehicle_name),
                Ok(false) => format!("Failed to lock your {}. This may be a temporary Tesla server issue.", vehicle_name),
                Err(e) => format!("Error locking vehicle. This may be a Tesla server or connectivity issue. {}", e),
            }
        }
        "unlock" => {
            match tesla_client.unlock_vehicle(&access_token, vehicle_vin).await {
                Ok(true) => format!("Successfully unlocked your {}", vehicle_name),
                Ok(false) => format!("Failed to unlock your {}. This may be a temporary Tesla server issue.", vehicle_name),
                Err(e) => format!("Error unlocking vehicle. This may be a Tesla server or connectivity issue. {}", e),
            }
        }
        "climate_on" => {
            match tesla_client.start_climate(&access_token, vehicle_vin).await {
                Ok(true) => format!("Climate control started in your {}. The car will start warming up or cooling down to your preset temperature.", vehicle_name),
                Ok(false) => format!("Failed to start climate in your {}. This may be a temporary Tesla server issue.", vehicle_name),
                Err(e) => format!("Error starting climate. This may be a Tesla server or connectivity issue. {}", e),
            }
        }
        "climate_off" => {
            match tesla_client.stop_climate(&access_token, vehicle_vin).await {
                Ok(true) => format!("Climate control stopped in your {}", vehicle_name),
                Ok(false) => format!("Failed to stop climate in your {}. This may be a temporary Tesla server issue.", vehicle_name),
                Err(e) => format!("Error stopping climate. This may be a Tesla server or connectivity issue. {}", e),
            }
        }
        "defrost" => {
            match tesla_client.defrost_vehicle(&access_token, vehicle_vin).await {
                Ok(msg) => format!("Your {} is now in max defrost mode. {}. The windshield and windows should clear quickly!", vehicle_name, msg),
                Err(e) => format!("Error activating defrost. This may be a Tesla server or connectivity issue. {}", e),
            }
        }
        "remote_start" => {
            match tesla_client.remote_start(&access_token, vehicle_vin).await {
                Ok(true) => format!("Remote start activated for your {}. You can now drive without the key for 2 minutes. Make sure you're near the vehicle.", vehicle_name),
                Ok(false) => format!("Failed to activate remote start for your {}. This may be a temporary Tesla server issue.", vehicle_name),
                Err(e) => format!("Error activating remote start. This may be a Tesla server or connectivity issue. {}", e),
            }
        }
        "charge_status" => {
            match tesla_client.get_vehicle_data(&access_token, vehicle_vin).await {
                Ok(data) => {
                    if let Some(charge_state) = data.charge_state {
                        let charging_status = if charge_state.charging_state == "Charging" {
                            format!(" Currently charging, {} minutes to full.",
                                charge_state.minutes_to_full_charge.unwrap_or(0))
                        } else {
                            String::new()
                        };

                        format!("Your {} battery is at {}% with {:.0} miles of range. Charge limit set to {}%.{}",
                            vehicle_name,
                            charge_state.battery_level,
                            charge_state.battery_range,
                            charge_state.charge_limit_soc,
                            charging_status
                        )
                    } else {
                        format!("Unable to get charge information for your {}", vehicle_name)
                    }
                }
                Err(e) => format!("Error getting charge status. This may be a Tesla server or connectivity issue. {}", e),
            }
        }
        "cabin_overheat_on" => {
            match tesla_client.set_cabin_overheat_protection(&access_token, vehicle_vin, true, false).await {
                Ok(true) => format!("Cabin Overheat Protection enabled for your {}. The car will use A/C to keep the cabin cool when parked in hot conditions.", vehicle_name),
                Ok(false) => format!("Failed to enable Cabin Overheat Protection for your {}. This may be a temporary Tesla server issue.", vehicle_name),
                Err(e) => format!("Error enabling Cabin Overheat Protection. This may be a Tesla server or connectivity issue. {}", e),
            }
        }
        "cabin_overheat_off" => {
            match tesla_client.set_cabin_overheat_protection(&access_token, vehicle_vin, false, false).await {
                Ok(true) => format!("Cabin Overheat Protection disabled for your {}", vehicle_name),
                Ok(false) => format!("Failed to disable Cabin Overheat Protection for your {}. This may be a temporary Tesla server issue.", vehicle_name),
                Err(e) => format!("Error disabling Cabin Overheat Protection. This may be a Tesla server or connectivity issue. {}", e),
            }
        }
        "cabin_overheat_fan_only" => {
            match tesla_client.set_cabin_overheat_protection(&access_token, vehicle_vin, true, true).await {
                Ok(true) => format!("Cabin Overheat Protection set to Fan Only for your {}. The car will use only the fan (no A/C) to keep the cabin cool when parked.", vehicle_name),
                Ok(false) => format!("Failed to set Cabin Overheat Protection to Fan Only for your {}. This may be a temporary Tesla server issue.", vehicle_name),
                Err(e) => format!("Error setting Cabin Overheat Protection to Fan Only. This may be a Tesla server or connectivity issue. {}", e),
            }
        }
        "precondition_battery" => {
            // Set scheduled departure for ~10 minutes from now to trigger preconditioning
            // This tells Tesla "I'm leaving soon" so it starts warming the battery
            let now = chrono::Local::now();
            let departure_minutes = ((now.hour() * 60 + now.minute() + 10) % 1440) as i32;
            let _ = tesla_client.set_scheduled_departure(
                access_token,
                vehicle_vin,
                departure_minutes,
                true,  // preconditioning_enabled
            ).await;

            // Get nearby charging sites to find a Supercharger
            let sites = match tesla_client.get_nearby_charging_sites(access_token, vehicle_vin).await {
                Ok(s) => s,
                Err(e) => {
                    return format!("Error getting nearby Superchargers: {}. Please try again.", e);
                }
            };

            if sites.superchargers.is_empty() {
                return format!("No Superchargers found near your {}. Cannot start battery preconditioning.", vehicle_name);
            }

            // Use the closest Supercharger - Tesla will precondition regardless of distance
            let target = sites.superchargers
                .iter()
                .min_by(|a, b| a.distance_miles.partial_cmp(&b.distance_miles).unwrap_or(std::cmp::Ordering::Equal));

            let Some(supercharger) = target else {
                return format!("No suitable Supercharger found for preconditioning your {}.", vehicle_name);
            };

            let sc_name = supercharger.name.clone();
            let sc_distance = supercharger.distance_miles;

            // Start climate control (also warms battery as side effect)
            let _ = tesla_client.start_climate(access_token, vehicle_vin).await;

            // Use share command with "Tesla Supercharger" prefix so Tesla recognizes it
            // as a Supercharger destination and triggers battery preconditioning
            let destination = format!("Tesla Supercharger {}", sc_name);
            match tesla_client.share_destination(access_token, vehicle_vin, &destination).await {
                Ok(true) => format!(
                    "Battery preconditioning started for your {}! Scheduled departure set for 10 min, navigation to {} ({:.0} miles away), and climate running. Your battery will warm up for fast charging.",
                    vehicle_name, sc_name, sc_distance
                ),
                Ok(false) => format!("Failed to start navigation for battery preconditioning. Please try again."),
                Err(e) => format!("Error starting navigation for preconditioning: {}", e),
            }
        }
        _ => {
            format!("Unknown Tesla command: '{}'. Available commands are: lock, unlock, climate_on, climate_off, defrost, remote_start, charge_status, cabin_overheat_on, cabin_overheat_off, cabin_overheat_fan_only, precondition_battery", command)
        }
    }
}

// Helper to spawn climate monitoring (for synchronous path)
// Note: This function is only called when user explicitly requested notification via notify_when_ready=true
fn spawn_climate_monitoring(
    state: &Arc<AppState>,
    user_id: i32,
    region: String,
    access_token: String,
    vehicle_vin: String,
    vehicle_name: String,
) {
    // Check if already monitoring
    if state.tesla_monitoring_tasks.contains_key(&user_id) {
        info!("Climate monitoring already in progress for user {}", user_id);
        return;
    }

    let state_clone = state.clone();
    let handle = tokio::spawn(async move {
        info!("Starting climate monitoring for user {}", user_id);
        let tesla_client = TeslaClient::new_with_proxy(&region);

        let monitoring_result = tesla_client.monitor_climate_ready(&access_token, &vehicle_vin).await
            .map_err(|e| e.to_string());

        match monitoring_result {
            Ok(Some(temp)) => {
                // Check if user is present in the vehicle before sending notification
                let is_user_present = match tesla_client.get_vehicle_data(&access_token, &vehicle_vin).await {
                    Ok(data) => data.vehicle_state.and_then(|vs| vs.is_user_present).unwrap_or(false),
                    Err(_) => false,
                };

                if is_user_present {
                    info!("User is present in vehicle, skipping climate ready notification for user {}", user_id);
                } else {
                    let msg = format!("Your {} is ready to drive! Cabin temp is {:.1}°C.", &vehicle_name, temp);
                    crate::proactive::utils::send_notification(
                        &state_clone,
                        user_id,
                        &msg,
                        "tesla_ready_to_drive".to_string(),
                        Some(format!("Your {} is warmed up and ready to drive!", &vehicle_name)),
                    ).await;
                }
            }
            Ok(None) => {
                // Check if user is present in the vehicle before sending notification
                let is_user_present = match tesla_client.get_vehicle_data(&access_token, &vehicle_vin).await {
                    Ok(data) => data.vehicle_state.and_then(|vs| vs.is_user_present).unwrap_or(false),
                    Err(_) => false,
                };

                if is_user_present {
                    info!("User is present in vehicle, skipping timeout notification for user {}", user_id);
                } else {
                    let msg = format!("Your {} should be ready by now (climate running 20+ min). Please check if needed.", &vehicle_name);
                    crate::proactive::utils::send_notification(
                        &state_clone,
                        user_id,
                        &msg,
                        "tesla_ready_timeout".to_string(),
                        Some(format!("Your {} should be warmed up by now.", &vehicle_name)),
                    ).await;
                }
            }
            Err(error_msg) => {
                let is_stopped = error_msg.contains("turned off");
                error!("Climate monitoring error for user {}: {}", user_id, error_msg);
                if is_stopped {
                    crate::proactive::utils::send_notification(
                        &state_clone,
                        user_id,
                        "Tesla climate was turned off before reaching target temperature.",
                        "tesla_climate_stopped".to_string(),
                        Some(format!("Your {} climate was stopped early.", &vehicle_name)),
                    ).await;
                }
            }
        }

        state_clone.tesla_monitoring_tasks.remove(&user_id);
        info!("Climate monitoring completed for user {}", user_id);
    });

    state.tesla_monitoring_tasks.insert(user_id, handle);
}