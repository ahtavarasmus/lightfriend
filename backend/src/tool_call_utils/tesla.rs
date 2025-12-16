use std::sync::Arc;
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
            description: Some("Command to execute: 'lock', 'unlock', 'climate_on', 'climate_off', 'defrost', 'remote_start', or 'charge_status'".to_string()),
            enum_values: Some(vec![
                "lock".to_string(),
                "unlock".to_string(),
                "climate_on".to_string(),
                "climate_off".to_string(),
                "defrost".to_string(),
                "remote_start".to_string(),
                "charge_status".to_string(),
            ]),
            ..Default::default()
        }),
    );

    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("control_tesla"),
            description: Some(String::from(
                "Control Tesla vehicle functions: lock/unlock doors, start/stop climate control, defrost vehicle (max heat + heated seats/steering wheel for deep ice), remote start driving, or check charge status",
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

    info!("Executing Tesla command '{}' for user {}", command, user_id);

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

    // Handle asleep vehicles: move to background for wake-up + command execution
    if command != "charge_status" && vehicle.state != "online" {
        info!("Vehicle is {}, spawning background task to wake and execute command", vehicle.state);

        let state_clone = state.clone();
        let access_token_clone = access_token.clone();
        let vehicle_vin_clone = vehicle_vin.to_string();
        let vehicle_name_clone = vehicle_name.to_string();
        let command_clone = command.to_string();
        let region_clone = region.clone();

        tokio::task::spawn(async move {
            let tesla_client = crate::api::tesla::TeslaClient::new_with_proxy(&region_clone);

            let wake_result = tesla_client.wake_up(&access_token_clone, &vehicle_vin_clone).await
                .map_err(|e| e.to_string());

            match wake_result {
                Ok(true) => {
                    info!("Vehicle woke up successfully, executing command: {}", command_clone);

                    let result = execute_tesla_command(
                        &tesla_client,
                        &access_token_clone,
                        &vehicle_vin_clone,
                        &vehicle_name_clone,
                        &command_clone,
                    ).await;

                    // Only send notification if not called from dashboard
                    if !skip_notification {
                        let notification_msg = format!("Tesla command completed: {}", result);
                        let first_msg = result.clone();

                        crate::proactive::utils::send_notification(
                            &state_clone,
                            user_id,
                            &notification_msg,
                            "tesla_command_success".to_string(),
                            Some(first_msg),
                        ).await;
                    } else {
                        info!("Skipping notification for dashboard-initiated Tesla command");
                    }

                    // Spawn climate monitoring for defrost and climate_on commands
                    if command_clone == "defrost" || command_clone == "climate_on" {
                        spawn_climate_monitoring_internal(
                            state_clone.clone(),
                            user_id,
                            region_clone.clone(),
                            access_token_clone.clone(),
                            vehicle_vin_clone.clone(),
                            vehicle_name_clone.clone(),
                        );
                    }
                }
                Ok(false) => {
                    error!("Vehicle wake-up returned false (unexpected)");
                    // Only send notification if not called from dashboard
                    if !skip_notification {
                        let failure_msg = format!("Your {} couldn't be woken up. Please try again or use the Tesla app.", vehicle_name_clone);

                        crate::proactive::utils::send_notification(
                            &state_clone,
                            user_id,
                            "Tesla wake-up failed unexpectedly",
                            "tesla_command_error".to_string(),
                            Some(failure_msg),
                        ).await;
                    }
                }
                Err(error_msg) => {
                    error!("Failed to wake up vehicle: {}", error_msg);
                    // Only send notification if not called from dashboard
                    if !skip_notification {
                        let notification_msg = format!("Tesla wake-up failed: {}", error_msg);
                        let failure_msg = format!("Your {} couldn't be woken up. Please try again or use the Tesla app.", vehicle_name_clone);

                        crate::proactive::utils::send_notification(
                            &state_clone,
                            user_id,
                            &notification_msg,
                            "tesla_command_error".to_string(),
                            Some(failure_msg),
                        ).await;
                    }
                }
            }
        });

        return format!(
            "Your {} is waking up. I'll {} and notify you when it's done (this may take up to 30 seconds).",
            vehicle_name,
            match command {
                "lock" => "lock it",
                "unlock" => "unlock it",
                "climate_on" => "start the climate",
                "climate_off" => "stop the climate",
                "defrost" => "activate max defrost mode with heated seats and steering wheel",
                "remote_start" => "activate remote start",
                _ => "send the command",
            }
        );
    }

    // Vehicle is already online, execute command immediately
    let result = execute_tesla_command(&tesla_client, &access_token, vehicle_vin, vehicle_name, command).await;

    // Spawn climate monitoring for defrost and climate_on commands
    if command == "defrost" || command == "climate_on" {
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
                Ok(false) => format!("Failed to lock your {}", vehicle_name),
                Err(e) => format!("Error locking vehicle: {}", e),
            }
        }
        "unlock" => {
            match tesla_client.unlock_vehicle(&access_token, vehicle_vin).await {
                Ok(true) => format!("Successfully unlocked your {}", vehicle_name),
                Ok(false) => format!("Failed to unlock your {}", vehicle_name),
                Err(e) => format!("Error unlocking vehicle: {}", e),
            }
        }
        "climate_on" => {
            match tesla_client.start_climate(&access_token, vehicle_vin).await {
                Ok(true) => format!("Climate control started in your {}. The car will start warming up or cooling down to your preset temperature.", vehicle_name),
                Ok(false) => format!("Failed to start climate in your {}", vehicle_name),
                Err(e) => format!("Error starting climate: {}", e),
            }
        }
        "climate_off" => {
            match tesla_client.stop_climate(&access_token, vehicle_vin).await {
                Ok(true) => format!("Climate control stopped in your {}", vehicle_name),
                Ok(false) => format!("Failed to stop climate in your {}", vehicle_name),
                Err(e) => format!("Error stopping climate: {}", e),
            }
        }
        "defrost" => {
            match tesla_client.defrost_vehicle(&access_token, vehicle_vin).await {
                Ok(msg) => format!("Your {} is now in max defrost mode. {}. The windshield and windows should clear quickly!", vehicle_name, msg),
                Err(e) => format!("Error activating defrost: {}", e),
            }
        }
        "remote_start" => {
            match tesla_client.remote_start(&access_token, vehicle_vin).await {
                Ok(true) => format!("Remote start activated for your {}. You can now drive without the key for 2 minutes. Make sure you're near the vehicle.", vehicle_name),
                Ok(false) => format!("Failed to activate remote start for your {}", vehicle_name),
                Err(e) => format!("Error activating remote start: {}", e),
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
                Err(e) => format!("Error getting charge status: {}", e),
            }
        }
        _ => {
            format!("Unknown Tesla command: '{}'. Available commands are: lock, unlock, climate_on, climate_off, defrost, remote_start, charge_status", command)
        }
    }
}

// Helper to spawn climate monitoring (for synchronous path)
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

        // Check if user wants notifications at time of sending
        let should_notify = state_clone.user_core.get_notify_on_climate_ready(user_id).unwrap_or(true);

        match monitoring_result {
            Ok(Some(temp)) => {
                if should_notify {
                    let msg = format!("Your {} is ready to drive! Cabin temp is {:.1}°C.", &vehicle_name, temp);
                    crate::proactive::utils::send_notification(
                        &state_clone,
                        user_id,
                        &msg,
                        "tesla_ready_to_drive".to_string(),
                        Some(format!("Your {} is warmed up and ready to drive!", &vehicle_name)),
                    ).await;
                } else {
                    info!("User {} has climate notifications disabled, skipping ready notification", user_id);
                }
            }
            Ok(None) => {
                if should_notify {
                    let msg = format!("Your {} should be ready by now (climate running 20+ min). Please check if needed.", &vehicle_name);
                    crate::proactive::utils::send_notification(
                        &state_clone,
                        user_id,
                        &msg,
                        "tesla_ready_timeout".to_string(),
                        Some(format!("Your {} should be warmed up by now.", &vehicle_name)),
                    ).await;
                } else {
                    info!("User {} has climate notifications disabled, skipping timeout notification", user_id);
                }
            }
            Err(error_msg) => {
                let is_stopped = error_msg.contains("turned off");
                error!("Climate monitoring error for user {}: {}", user_id, error_msg);
                if is_stopped && should_notify {
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

// Helper for async path (already inside tokio::spawn)
fn spawn_climate_monitoring_internal(
    state: Arc<AppState>,
    user_id: i32,
    region: String,
    access_token: String,
    vehicle_vin: String,
    vehicle_name: String,
) {
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

        // Check if user wants notifications at time of sending
        let should_notify = state_clone.user_core.get_notify_on_climate_ready(user_id).unwrap_or(true);

        match monitoring_result {
            Ok(Some(temp)) => {
                if should_notify {
                    let msg = format!("Your {} is ready to drive! Cabin temp is {:.1}°C.", &vehicle_name, temp);
                    crate::proactive::utils::send_notification(
                        &state_clone,
                        user_id,
                        &msg,
                        "tesla_ready_to_drive".to_string(),
                        Some(format!("Your {} is warmed up and ready to drive!", &vehicle_name)),
                    ).await;
                } else {
                    info!("User {} has climate notifications disabled, skipping ready notification", user_id);
                }
            }
            Ok(None) => {
                if should_notify {
                    let msg = format!("Your {} should be ready by now (climate running 20+ min). Please check if needed.", &vehicle_name);
                    crate::proactive::utils::send_notification(
                        &state_clone,
                        user_id,
                        &msg,
                        "tesla_ready_timeout".to_string(),
                        Some(format!("Your {} should be warmed up by now.", &vehicle_name)),
                    ).await;
                } else {
                    info!("User {} has climate notifications disabled, skipping timeout notification", user_id);
                }
            }
            Err(error_msg) => {
                let is_stopped = error_msg.contains("turned off");
                error!("Climate monitoring error for user {}: {}", user_id, error_msg);
                if is_stopped && should_notify {
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