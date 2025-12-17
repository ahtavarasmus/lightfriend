use yew::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use crate::utils::api::Api;
use crate::utils::webauthn;
use crate::components::feature_preview::FeaturePreview;
use serde::Deserialize;
use std::rc::Rc;
use std::cell::RefCell;

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct VehicleInfo {
    pub vin: String,
    pub id: String,
    pub vehicle_id: String,
    pub name: String,
    pub state: String,
    pub selected: bool,
    pub paired: bool,
}

// Response for 2FA requirements check
#[derive(Deserialize, Clone, Debug)]
struct SensitiveChangeRequirements {
    requires_2fa: bool,
    has_passkeys: bool,
    has_totp: bool,
    passkey_options: Option<serde_json::Value>,
}

// State for passkey verification modal
#[derive(Clone, PartialEq)]
enum PasskeyVerifyState {
    Hidden,
    Loading,
    ShowingOptions {
        has_passkeys: bool,
        has_totp: bool,
        passkey_options: Option<serde_json::Value>,
        pending_command: String,
    },
    WaitingForPasskey,
    WaitingForTotp,
    Verifying,
    Error(String),
}

#[function_component(TeslaControls)]
pub fn tesla_controls() -> Html {
    let tesla_connected = use_state(|| false);
    let loading = use_state(|| true);
    let lock_loading = use_state(|| false);
    let climate_loading = use_state(|| false);
    let defrost_loading = use_state(|| false);
    let remote_start_loading = use_state(|| false);
    let cabin_overheat_loading = use_state(|| false);
    let command_result = use_state(|| None::<String>);

    // Passkey verification state
    let passkey_state = use_state(|| PasskeyVerifyState::Hidden);
    let pending_passkey_options = use_state(|| None::<serde_json::Value>);
    let pending_command = use_state(|| String::new());
    let totp_code_input = use_state(String::new);
    let battery_level = use_state(|| None::<i32>);
    let battery_range = use_state(|| None::<f64>);
    let charging_state = use_state(|| None::<String>);
    let battery_loading = use_state(|| false);
    let is_locked = use_state(|| None::<bool>);
    let inside_temp = use_state(|| None::<f64>);
    let outside_temp = use_state(|| None::<f64>);
    let is_climate_on = use_state(|| None::<bool>);
    let is_front_defroster_on = use_state(|| None::<bool>);
    let is_rear_defroster_on = use_state(|| None::<bool>);
    let available_vehicles = use_state(|| Vec::<VehicleInfo>::new());
    let selected_vehicle_name = use_state(|| None::<String>);
    let last_refresh_time: UseStateHandle<Option<String>> = use_state(|| None);
    let last_refresh_epoch: UseStateHandle<Rc<RefCell<f64>>> = use_state(|| Rc::new(RefCell::new(0.0)));

    // Notification monitoring state
    let climate_notify_active = use_state(|| false);
    let climate_notify_loading = use_state(|| false);
    let charging_notify_active = use_state(|| false);
    let charging_notify_loading = use_state(|| false);

    // Auto-hide command result after 10 seconds
    {
        let command_result_for_effect = command_result.clone();
        let command_result_for_dep = command_result.clone();
        use_effect_with_deps(
            move |result: &Option<String>| {
                if result.is_some() {
                    let command_result = command_result_for_effect.clone();
                    let window = web_sys::window().unwrap();
                    let timeout_callback = Closure::wrap(Box::new(move || {
                        command_result.set(None);
                    }) as Box<dyn Fn()>);

                    let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                        timeout_callback.as_ref().unchecked_ref(),
                        10000,
                    );
                    timeout_callback.forget();
                }
                || ()
            },
            (*command_result_for_dep).clone(),
        );
    }

    // Check Tesla connection status on mount
    {
        let tesla_connected = tesla_connected.clone();
        let loading = loading.clone();
        use_effect_with_deps(
            move |_| {
                spawn_local(async move {
                    match Api::get("/api/auth/tesla/status")
                        .send()
                        .await
                    {
                        Ok(response) => {
                            if response.ok() {
                                if let Ok(status) = response.json::<serde_json::Value>().await {
                                    if let Some(has_tesla) = status["has_tesla"].as_bool() {
                                        tesla_connected.set(has_tesla);
                                    }
                                }
                            }
                        }
                        Err(_) => {}
                    }
                    loading.set(false);
                });
                || ()
            },
            (),
        );
    }

    // Auto-refresh vehicle status when Tesla becomes connected
    {
        let tesla_connected = tesla_connected.clone();
        let battery_loading = battery_loading.clone();
        let battery_level = battery_level.clone();
        let battery_range = battery_range.clone();
        let charging_state = charging_state.clone();
        let is_locked = is_locked.clone();
        let inside_temp = inside_temp.clone();
        let outside_temp = outside_temp.clone();
        let is_climate_on = is_climate_on.clone();
        let is_front_defroster_on = is_front_defroster_on.clone();
        let is_rear_defroster_on = is_rear_defroster_on.clone();
        let available_vehicles = available_vehicles.clone();
        let selected_vehicle_name = selected_vehicle_name.clone();
        let last_refresh_time = last_refresh_time.clone();
        let last_refresh_epoch = last_refresh_epoch.clone();
        let climate_notify_active = climate_notify_active.clone();
        let charging_notify_active = charging_notify_active.clone();

        use_effect_with_deps(
            move |connected| {
                if **connected {
                    let battery_loading = battery_loading.clone();
                    let battery_level = battery_level.clone();
                    let battery_range = battery_range.clone();
                    let climate_notify_active = climate_notify_active.clone();
                    let charging_notify_active = charging_notify_active.clone();
                    let charging_state = charging_state.clone();
                    let is_locked = is_locked.clone();
                    let inside_temp = inside_temp.clone();
                    let outside_temp = outside_temp.clone();
                    let is_climate_on = is_climate_on.clone();
                    let is_front_defroster_on = is_front_defroster_on.clone();
                    let is_rear_defroster_on = is_rear_defroster_on.clone();
                    let available_vehicles = available_vehicles.clone();
                    let selected_vehicle_name = selected_vehicle_name.clone();
                    let last_refresh_time = last_refresh_time.clone();
                    let last_refresh_epoch = last_refresh_epoch.clone();

                    spawn_local(async move {
                        battery_loading.set(true);

                        // Fetch battery status
                        match Api::get("/api/tesla/battery-status").send().await {
                            Ok(response) => {
                                if response.ok() {
                                    if let Ok(data) = response.json::<serde_json::Value>().await {
                                        if let Some(level) = data["battery_level"].as_i64() {
                                            battery_level.set(Some(level as i32));
                                        }
                                        if let Some(range) = data["battery_range"].as_f64() {
                                            battery_range.set(Some(range));
                                        }
                                        if let Some(state) = data["charging_state"].as_str() {
                                            charging_state.set(Some(state.to_string()));
                                        }
                                        if let Some(locked) = data["locked"].as_bool() {
                                            is_locked.set(Some(locked));
                                        }
                                        if let Some(temp) = data["inside_temp"].as_f64() {
                                            inside_temp.set(Some(temp));
                                        }
                                        if let Some(temp) = data["outside_temp"].as_f64() {
                                            outside_temp.set(Some(temp));
                                        }
                                        if let Some(climate) = data["is_climate_on"].as_bool() {
                                            is_climate_on.set(Some(climate));
                                        }
                                        if let Some(front_defrost) = data["is_front_defroster_on"].as_bool() {
                                            is_front_defroster_on.set(Some(front_defrost));
                                        }
                                        if let Some(rear_defrost) = data["is_rear_defroster_on"].as_bool() {
                                            is_rear_defroster_on.set(Some(rear_defrost));
                                        }
                                        let now = web_sys::js_sys::Date::new_0();
                                        let time_str = format!(
                                            "{:02}:{:02}",
                                            now.get_hours() as u32,
                                            now.get_minutes() as u32,
                                        );
                                        last_refresh_time.set(Some(time_str));
                                        *last_refresh_epoch.borrow_mut() = now.get_time();
                                    }
                                }
                            }
                            Err(_) => {}
                        }

                        // Fetch vehicles
                        match Api::get("/api/tesla/vehicles").send().await {
                            Ok(response) => {
                                if response.ok() {
                                    if let Ok(data) = response.json::<serde_json::Value>().await {
                                        if let Some(vehicles_array) = data["vehicles"].as_array() {
                                            let vehicles: Vec<VehicleInfo> = vehicles_array
                                                .iter()
                                                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                                                .collect();
                                            let selected_name = vehicles.iter()
                                                .find(|v| v.selected)
                                                .map(|v| v.name.clone());
                                            available_vehicles.set(vehicles);
                                            selected_vehicle_name.set(selected_name);
                                        }
                                    }
                                }
                            }
                            Err(_) => {}
                        }

                        // Fetch climate notify status
                        match Api::get("/api/tesla/climate-notify/status").send().await {
                            Ok(response) => {
                                if response.ok() {
                                    if let Ok(data) = response.json::<serde_json::Value>().await {
                                        if let Some(active) = data["active"].as_bool() {
                                            climate_notify_active.set(active);
                                        }
                                    }
                                }
                            }
                            Err(_) => {}
                        }

                        // Fetch charging notify status
                        match Api::get("/api/tesla/charging-notify/status").send().await {
                            Ok(response) => {
                                if response.ok() {
                                    if let Ok(data) = response.json::<serde_json::Value>().await {
                                        if let Some(active) = data["active"].as_bool() {
                                            charging_notify_active.set(active);
                                        }
                                    }
                                }
                            }
                            Err(_) => {}
                        }

                        battery_loading.set(false);
                    });
                }
                || ()
            },
            tesla_connected.clone(),
        );
    }

    // Handle lock/unlock - unlock requires passkey verification
    let handle_lock = {
        let lock_loading = lock_loading.clone();
        let command_result = command_result.clone();
        let is_locked = is_locked.clone();
        let passkey_state = passkey_state.clone();
        let pending_passkey_options = pending_passkey_options.clone();
        let pending_command = pending_command.clone();

        Callback::from(move |_: MouseEvent| {
            let lock_loading = lock_loading.clone();
            let command_result = command_result.clone();
            let is_locked = is_locked.clone();
            let passkey_state = passkey_state.clone();
            let pending_passkey_options = pending_passkey_options.clone();
            let pending_command = pending_command.clone();

            let command = match *is_locked {
                Some(true) => "unlock",
                Some(false) => "lock",
                None => "lock",
            };

            // Lock doesn't require passkey, only unlock does
            if command == "lock" {
                lock_loading.set(true);
                command_result.set(None);
                spawn_local(async move {
                    execute_tesla_command("lock", lock_loading, command_result, Some(is_locked)).await;
                });
            } else {
                // Unlock requires passkey verification - check if user has 2FA
                passkey_state.set(PasskeyVerifyState::Loading);
                pending_command.set("unlock".to_string());
                spawn_local(async move {
                    match Api::get("/api/profile/sensitive-change-requirements").send().await {
                        Ok(resp) if resp.ok() => {
                            if let Ok(requirements) = resp.json::<SensitiveChangeRequirements>().await {
                                if requirements.requires_2fa {
                                    pending_passkey_options.set(requirements.passkey_options.clone());
                                    passkey_state.set(PasskeyVerifyState::ShowingOptions {
                                        has_passkeys: requirements.has_passkeys,
                                        has_totp: requirements.has_totp,
                                        passkey_options: requirements.passkey_options,
                                        pending_command: "unlock".to_string(),
                                    });
                                } else {
                                    // No 2FA required, execute directly
                                    passkey_state.set(PasskeyVerifyState::Hidden);
                                    lock_loading.set(true);
                                    command_result.set(None);
                                    execute_tesla_command("unlock", lock_loading, command_result, Some(is_locked)).await;
                                }
                            } else {
                                passkey_state.set(PasskeyVerifyState::Error("Failed to check 2FA requirements".to_string()));
                            }
                        }
                        _ => {
                            passkey_state.set(PasskeyVerifyState::Error("Failed to check 2FA requirements".to_string()));
                        }
                    }
                });
            }
        })
    };

    // Handle climate
    let handle_climate = {
        let climate_loading = climate_loading.clone();
        let command_result = command_result.clone();
        let is_climate_on = is_climate_on.clone();

        Callback::from(move |_: MouseEvent| {
            let climate_loading = climate_loading.clone();
            let command_result = command_result.clone();
            let is_climate_on = is_climate_on.clone();

            climate_loading.set(true);
            command_result.set(None);

            spawn_local(async move {
                let command = match *is_climate_on {
                    Some(true) => "climate_off",
                    Some(false) => "climate_on",
                    None => "climate_on",
                };

                let body = serde_json::json!({ "command": command });

                let request = match Api::post("/api/tesla/command").json(&body) {
                    Ok(req) => req.send().await,
                    Err(e) => {
                        command_result.set(Some(format!("Failed: {}", e)));
                        climate_loading.set(false);
                        return;
                    }
                };

                match request {
                    Ok(response) => {
                        if response.ok() {
                            match command {
                                "climate_on" => is_climate_on.set(Some(true)),
                                "climate_off" => is_climate_on.set(Some(false)),
                                _ => {}
                            }
                            if let Ok(data) = response.json::<serde_json::Value>().await {
                                if let Some(msg) = data.get("message").and_then(|m| m.as_str()) {
                                    command_result.set(Some(msg.to_string()));
                                }
                            }
                        } else {
                            command_result.set(Some("Failed to execute command".to_string()));
                        }
                    }
                    Err(e) => {
                        command_result.set(Some(format!("Network error: {}", e)));
                    }
                }
                climate_loading.set(false);
            });
        })
    };

    // Handle defrost
    let handle_defrost = {
        let defrost_loading = defrost_loading.clone();
        let command_result = command_result.clone();
        let is_front_defroster_on = is_front_defroster_on.clone();
        let is_rear_defroster_on = is_rear_defroster_on.clone();
        let is_climate_on = is_climate_on.clone();

        Callback::from(move |_: MouseEvent| {
            let defrost_loading = defrost_loading.clone();
            let command_result = command_result.clone();
            let is_front_defroster_on = is_front_defroster_on.clone();
            let is_rear_defroster_on = is_rear_defroster_on.clone();
            let is_climate_on = is_climate_on.clone();

            defrost_loading.set(true);
            command_result.set(None);

            spawn_local(async move {
                let front_on = (*is_front_defroster_on).unwrap_or(false);
                let rear_on = (*is_rear_defroster_on).unwrap_or(false);
                let any_defrost_on = front_on || rear_on;

                let command = if any_defrost_on { "climate_off" } else { "defrost" };

                let body = serde_json::json!({ "command": command });

                let request = match Api::post("/api/tesla/command").json(&body) {
                    Ok(req) => req.send().await,
                    Err(e) => {
                        command_result.set(Some(format!("Failed: {}", e)));
                        defrost_loading.set(false);
                        return;
                    }
                };

                match request {
                    Ok(response) => {
                        if response.ok() {
                            match command {
                                "defrost" => {
                                    is_front_defroster_on.set(Some(true));
                                    is_rear_defroster_on.set(Some(true));
                                    is_climate_on.set(Some(true));
                                }
                                "climate_off" => {
                                    is_front_defroster_on.set(Some(false));
                                    is_rear_defroster_on.set(Some(false));
                                    is_climate_on.set(Some(false));
                                }
                                _ => {}
                            }
                            if let Ok(data) = response.json::<serde_json::Value>().await {
                                if let Some(msg) = data.get("message").and_then(|m| m.as_str()) {
                                    command_result.set(Some(msg.to_string()));
                                }
                            }
                        } else {
                            command_result.set(Some("Failed to execute command".to_string()));
                        }
                    }
                    Err(e) => {
                        command_result.set(Some(format!("Network error: {}", e)));
                    }
                }
                defrost_loading.set(false);
            });
        })
    };

    // Handle remote start - requires passkey verification
    let handle_remote_start = {
        let remote_start_loading = remote_start_loading.clone();
        let command_result = command_result.clone();
        let passkey_state = passkey_state.clone();
        let pending_passkey_options = pending_passkey_options.clone();
        let pending_command = pending_command.clone();

        Callback::from(move |_: MouseEvent| {
            let remote_start_loading = remote_start_loading.clone();
            let command_result = command_result.clone();
            let passkey_state = passkey_state.clone();
            let pending_passkey_options = pending_passkey_options.clone();
            let pending_command = pending_command.clone();

            // Remote start requires passkey verification - check if user has 2FA
            passkey_state.set(PasskeyVerifyState::Loading);
            pending_command.set("remote_start".to_string());
            spawn_local(async move {
                match Api::get("/api/profile/sensitive-change-requirements").send().await {
                    Ok(resp) if resp.ok() => {
                        if let Ok(requirements) = resp.json::<SensitiveChangeRequirements>().await {
                            if requirements.requires_2fa {
                                pending_passkey_options.set(requirements.passkey_options.clone());
                                passkey_state.set(PasskeyVerifyState::ShowingOptions {
                                    has_passkeys: requirements.has_passkeys,
                                    has_totp: requirements.has_totp,
                                    passkey_options: requirements.passkey_options,
                                    pending_command: "remote_start".to_string(),
                                });
                            } else {
                                // No 2FA required, execute directly
                                passkey_state.set(PasskeyVerifyState::Hidden);
                                remote_start_loading.set(true);
                                command_result.set(None);
                                execute_tesla_command("remote_start", remote_start_loading, command_result, None).await;
                            }
                        } else {
                            passkey_state.set(PasskeyVerifyState::Error("Failed to check 2FA requirements".to_string()));
                        }
                    }
                    _ => {
                        passkey_state.set(PasskeyVerifyState::Error("Failed to check 2FA requirements".to_string()));
                    }
                }
            });
        })
    };

    // Handle cabin overheat protection
    let handle_cabin_overheat = {
        let cabin_overheat_loading = cabin_overheat_loading.clone();
        let command_result = command_result.clone();

        Callback::from(move |mode: String| {
            let cabin_overheat_loading = cabin_overheat_loading.clone();
            let command_result = command_result.clone();

            cabin_overheat_loading.set(true);
            command_result.set(None);

            spawn_local(async move {
                let body = serde_json::json!({ "command": mode });
                let request = match Api::post("/api/tesla/command").json(&body) {
                    Ok(req) => req.send().await,
                    Err(e) => {
                        command_result.set(Some(format!("Failed to create request: {}", e)));
                        cabin_overheat_loading.set(false);
                        return;
                    }
                };

                match request {
                    Ok(response) => {
                        if response.ok() {
                            if let Ok(data) = response.json::<serde_json::Value>().await {
                                if let Some(msg) = data.get("message").and_then(|m| m.as_str()) {
                                    command_result.set(Some(msg.to_string()));
                                }
                            }
                        } else {
                            command_result.set(Some("Failed to execute command".to_string()));
                        }
                    }
                    Err(e) => {
                        command_result.set(Some(format!("Network error: {}", e)));
                    }
                }
                cabin_overheat_loading.set(false);
            });
        })
    };

    // Handle refresh
    let handle_refresh = {
        let battery_loading = battery_loading.clone();
        let battery_level = battery_level.clone();
        let battery_range = battery_range.clone();
        let charging_state = charging_state.clone();
        let is_locked = is_locked.clone();
        let inside_temp = inside_temp.clone();
        let outside_temp = outside_temp.clone();
        let is_climate_on = is_climate_on.clone();
        let is_front_defroster_on = is_front_defroster_on.clone();
        let is_rear_defroster_on = is_rear_defroster_on.clone();
        let last_refresh_time = last_refresh_time.clone();
        let last_refresh_epoch = last_refresh_epoch.clone();

        Callback::from(move |_: MouseEvent| {
            let battery_loading = battery_loading.clone();
            let battery_level = battery_level.clone();
            let battery_range = battery_range.clone();
            let charging_state = charging_state.clone();
            let is_locked = is_locked.clone();
            let inside_temp = inside_temp.clone();
            let outside_temp = outside_temp.clone();
            let is_climate_on = is_climate_on.clone();
            let is_front_defroster_on = is_front_defroster_on.clone();
            let is_rear_defroster_on = is_rear_defroster_on.clone();
            let last_refresh_time = last_refresh_time.clone();
            let last_refresh_epoch = last_refresh_epoch.clone();

            battery_loading.set(true);

            spawn_local(async move {
                match Api::get("/api/tesla/battery-status").send().await {
                    Ok(response) => {
                        if response.ok() {
                            if let Ok(data) = response.json::<serde_json::Value>().await {
                                if let Some(level) = data["battery_level"].as_i64() {
                                    battery_level.set(Some(level as i32));
                                }
                                if let Some(range) = data["battery_range"].as_f64() {
                                    battery_range.set(Some(range));
                                }
                                if let Some(state) = data["charging_state"].as_str() {
                                    charging_state.set(Some(state.to_string()));
                                }
                                if let Some(locked) = data["locked"].as_bool() {
                                    is_locked.set(Some(locked));
                                }
                                if let Some(temp) = data["inside_temp"].as_f64() {
                                    inside_temp.set(Some(temp));
                                }
                                if let Some(temp) = data["outside_temp"].as_f64() {
                                    outside_temp.set(Some(temp));
                                }
                                if let Some(climate) = data["is_climate_on"].as_bool() {
                                    is_climate_on.set(Some(climate));
                                }
                                if let Some(front_defrost) = data["is_front_defroster_on"].as_bool() {
                                    is_front_defroster_on.set(Some(front_defrost));
                                }
                                if let Some(rear_defrost) = data["is_rear_defroster_on"].as_bool() {
                                    is_rear_defroster_on.set(Some(rear_defrost));
                                }
                                let now = web_sys::js_sys::Date::new_0();
                                let time_str = format!(
                                    "{:02}:{:02}",
                                    now.get_hours() as u32,
                                    now.get_minutes() as u32,
                                );
                                last_refresh_time.set(Some(time_str));
                                *last_refresh_epoch.borrow_mut() = now.get_time();
                            }
                        }
                    }
                    Err(_) => {}
                }
                battery_loading.set(false);
            });
        })
    };

    // Handle climate notification toggle
    let handle_climate_notify = {
        let climate_notify_active = climate_notify_active.clone();
        let climate_notify_loading = climate_notify_loading.clone();
        let command_result = command_result.clone();

        Callback::from(move |_: MouseEvent| {
            let climate_notify_active = climate_notify_active.clone();
            let climate_notify_loading = climate_notify_loading.clone();
            let command_result = command_result.clone();
            let is_active = *climate_notify_active;

            climate_notify_loading.set(true);

            spawn_local(async move {
                let endpoint = if is_active {
                    "/api/tesla/climate-notify/cancel"
                } else {
                    "/api/tesla/climate-notify/start"
                };

                match Api::post(endpoint).send().await {
                    Ok(response) => {
                        if response.ok() {
                            climate_notify_active.set(!is_active);
                            if !is_active {
                                command_result.set(Some("You'll be notified when your car is ready to drive".to_string()));
                            } else {
                                command_result.set(Some("Climate notification cancelled".to_string()));
                            }
                        } else {
                            command_result.set(Some("Failed to update notification".to_string()));
                        }
                    }
                    Err(e) => {
                        command_result.set(Some(format!("Error: {}", e)));
                    }
                }
                climate_notify_loading.set(false);
            });
        })
    };

    // Handle charging notification toggle
    let handle_charging_notify = {
        let charging_notify_active = charging_notify_active.clone();
        let charging_notify_loading = charging_notify_loading.clone();
        let command_result = command_result.clone();

        Callback::from(move |_: MouseEvent| {
            let charging_notify_active = charging_notify_active.clone();
            let charging_notify_loading = charging_notify_loading.clone();
            let command_result = command_result.clone();
            let is_active = *charging_notify_active;

            charging_notify_loading.set(true);

            spawn_local(async move {
                let endpoint = if is_active {
                    "/api/tesla/charging-notify/cancel"
                } else {
                    "/api/tesla/charging-notify/start"
                };

                match Api::post(endpoint).send().await {
                    Ok(response) => {
                        if response.ok() {
                            charging_notify_active.set(!is_active);
                            if !is_active {
                                command_result.set(Some("You'll be notified when charging completes".to_string()));
                            } else {
                                command_result.set(Some("Charging notification cancelled".to_string()));
                            }
                        } else {
                            command_result.set(Some("Failed to update notification".to_string()));
                        }
                    }
                    Err(e) => {
                        command_result.set(Some(format!("Error: {}", e)));
                    }
                }
                charging_notify_loading.set(false);
            });
        })
    };

    // Passkey verification handlers
    let on_passkey_cancel = {
        let passkey_state = passkey_state.clone();
        let pending_command = pending_command.clone();
        let totp_code_input = totp_code_input.clone();
        Callback::from(move |_: MouseEvent| {
            passkey_state.set(PasskeyVerifyState::Hidden);
            pending_command.set(String::new());
            totp_code_input.set(String::new());
        })
    };

    let on_use_passkey = {
        let passkey_state = passkey_state.clone();
        let pending_passkey_options = pending_passkey_options.clone();
        let pending_command = pending_command.clone();
        let lock_loading = lock_loading.clone();
        let remote_start_loading = remote_start_loading.clone();
        let command_result = command_result.clone();
        let is_locked = is_locked.clone();

        Callback::from(move |_: MouseEvent| {
            let passkey_state = passkey_state.clone();
            let pending_passkey_options = pending_passkey_options.clone();
            let pending_command = pending_command.clone();
            let lock_loading = lock_loading.clone();
            let remote_start_loading = remote_start_loading.clone();
            let command_result = command_result.clone();
            let is_locked = is_locked.clone();

            passkey_state.set(PasskeyVerifyState::WaitingForPasskey);

            spawn_local(async move {
                if let Some(options) = (*pending_passkey_options).clone() {
                    let auth_options = options.get("options").cloned().unwrap_or(options);

                    match webauthn::get_credential(&auth_options).await {
                        Ok(_credential) => {
                            passkey_state.set(PasskeyVerifyState::Verifying);
                            let cmd = (*pending_command).clone();

                            // Execute the Tesla command after successful verification
                            match cmd.as_str() {
                                "unlock" => {
                                    lock_loading.set(true);
                                    command_result.set(None);
                                    execute_tesla_command("unlock", lock_loading, command_result, Some(is_locked)).await;
                                }
                                "remote_start" => {
                                    remote_start_loading.set(true);
                                    command_result.set(None);
                                    execute_tesla_command("remote_start", remote_start_loading, command_result, None).await;
                                }
                                _ => {}
                            }
                            passkey_state.set(PasskeyVerifyState::Hidden);
                        }
                        Err(e) => {
                            passkey_state.set(PasskeyVerifyState::Error(format!("Passkey verification failed: {}", e)));
                        }
                    }
                } else {
                    passkey_state.set(PasskeyVerifyState::Error("No passkey options available".to_string()));
                }
            });
        })
    };

    let on_use_totp = {
        let passkey_state = passkey_state.clone();
        Callback::from(move |_: MouseEvent| {
            passkey_state.set(PasskeyVerifyState::WaitingForTotp);
        })
    };

    let on_submit_totp = {
        let passkey_state = passkey_state.clone();
        let totp_code_input = totp_code_input.clone();
        let pending_command = pending_command.clone();
        let lock_loading = lock_loading.clone();
        let remote_start_loading = remote_start_loading.clone();
        let command_result = command_result.clone();
        let is_locked = is_locked.clone();

        Callback::from(move |_: MouseEvent| {
            let code = (*totp_code_input).clone();
            if code.is_empty() {
                return;
            }

            let passkey_state = passkey_state.clone();
            let pending_command = pending_command.clone();
            let lock_loading = lock_loading.clone();
            let remote_start_loading = remote_start_loading.clone();
            let command_result = command_result.clone();
            let is_locked = is_locked.clone();
            let totp_code_input = totp_code_input.clone();

            passkey_state.set(PasskeyVerifyState::Verifying);

            spawn_local(async move {
                // Verify TOTP code via backend
                let verify_body = serde_json::json!({ "code": code });
                match Api::post("/api/totp/verify-code")
                    .json(&verify_body)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(resp) if resp.ok() => {
                        let cmd = (*pending_command).clone();
                        match cmd.as_str() {
                            "unlock" => {
                                lock_loading.set(true);
                                command_result.set(None);
                                execute_tesla_command("unlock", lock_loading, command_result, Some(is_locked)).await;
                            }
                            "remote_start" => {
                                remote_start_loading.set(true);
                                command_result.set(None);
                                execute_tesla_command("remote_start", remote_start_loading, command_result, None).await;
                            }
                            _ => {}
                        }
                        passkey_state.set(PasskeyVerifyState::Hidden);
                        totp_code_input.set(String::new());
                    }
                    _ => {
                        passkey_state.set(PasskeyVerifyState::Error("Invalid verification code".to_string()));
                    }
                }
            });
        })
    };

    // Loading state
    if *loading {
        return html! {
            <div class="tesla-controls-container">
                <div class="tesla-controls-loading">
                    <i class="fas fa-spinner fa-spin"></i>
                    {" Loading..."}
                </div>
                <style>{get_styles()}</style>
            </div>
        };
    }

    // Not connected state
    if !*tesla_connected {
        return html! {
            <div class="tesla-controls-container">
                <div class="tesla-not-connected">
                    <img src="https://upload.wikimedia.org/wikipedia/commons/b/bb/Tesla_T_symbol.svg" alt="Tesla" width="48" height="48" style="opacity: 0.5; margin-bottom: 0.5rem;"/>
                    <h3>{"Tesla Not Connected"}</h3>
                    <p>{"Connect your Tesla to control it from here."}</p>

                    <FeaturePreview
                        gif_src="/assets/previews/tesla-controls-preview.gif"
                        caption="Lock, unlock, control climate, defrost, and more"
                        badge_text="Preview"
                        connect_href="/"
                        connect_text="Go to Connections"
                        max_width={380}
                    />
                </div>
                <style>{get_styles()}</style>
            </div>
        };
    }

    // Connected - show controls
    html! {
        <div class="tesla-controls-container">
            // Passkey verification modal
            {
                match &*passkey_state {
                    PasskeyVerifyState::Hidden => html! {},
                    PasskeyVerifyState::Loading => html! {
                        <div class="passkey-dialog-overlay">
                            <div class="passkey-dialog">
                                <h3>{"Checking security..."}</h3>
                                <div class="loading-spinner"></div>
                            </div>
                        </div>
                    },
                    PasskeyVerifyState::ShowingOptions { has_passkeys, has_totp, pending_command, .. } => {
                        let action_name = match pending_command.as_str() {
                            "unlock" => "unlock your Tesla",
                            "remote_start" => "start your Tesla",
                            _ => "perform this action",
                        };
                        html! {
                            <div class="passkey-dialog-overlay">
                                <div class="passkey-dialog">
                                    <h3>{"Verify Your Identity"}</h3>
                                    <p>{format!("Authentication required to {}", action_name)}</p>
                                    <div class="passkey-options">
                                        {
                                            if *has_passkeys {
                                                html! {
                                                    <button
                                                        class="passkey-btn"
                                                        onclick={on_use_passkey.clone()}
                                                    >
                                                        {"Use Passkey"}
                                                    </button>
                                                }
                                            } else {
                                                html! {}
                                            }
                                        }
                                        {
                                            if *has_totp {
                                                html! {
                                                    <button
                                                        class="passkey-btn"
                                                        onclick={on_use_totp.clone()}
                                                    >
                                                        {"Use Authenticator Code"}
                                                    </button>
                                                }
                                            } else {
                                                html! {}
                                            }
                                        }
                                    </div>
                                    <button class="passkey-btn passkey-cancel" onclick={on_passkey_cancel.clone()}>{"Cancel"}</button>
                                </div>
                            </div>
                        }
                    },
                    PasskeyVerifyState::WaitingForPasskey => html! {
                        <div class="passkey-dialog-overlay">
                            <div class="passkey-dialog">
                                <h3>{"Passkey Verification"}</h3>
                                <p>{"Please use your passkey to verify..."}</p>
                                <div class="loading-spinner"></div>
                                <button class="passkey-btn passkey-cancel" onclick={on_passkey_cancel.clone()}>{"Cancel"}</button>
                            </div>
                        </div>
                    },
                    PasskeyVerifyState::WaitingForTotp => {
                        let totp_code_input_clone = totp_code_input.clone();
                        html! {
                            <div class="passkey-dialog-overlay">
                                <div class="passkey-dialog">
                                    <h3>{"Enter Verification Code"}</h3>
                                    <p>{"Enter the 6-digit code from your authenticator app:"}</p>
                                    <input
                                        type="text"
                                        class="totp-input"
                                        placeholder="000000"
                                        maxlength="6"
                                        value={(*totp_code_input).clone()}
                                        oninput={move |e: InputEvent| {
                                            let input: HtmlInputElement = e.target_unchecked_into();
                                            totp_code_input_clone.set(input.value());
                                        }}
                                    />
                                    <div class="passkey-options">
                                        <button class="passkey-btn" onclick={on_submit_totp.clone()}>{"Verify"}</button>
                                        <button class="passkey-btn passkey-cancel" onclick={on_passkey_cancel.clone()}>{"Cancel"}</button>
                                    </div>
                                </div>
                            </div>
                        }
                    },
                    PasskeyVerifyState::Verifying => html! {
                        <div class="passkey-dialog-overlay">
                            <div class="passkey-dialog">
                                <h3>{"Verifying..."}</h3>
                                <div class="loading-spinner"></div>
                            </div>
                        </div>
                    },
                    PasskeyVerifyState::Error(msg) => html! {
                        <div class="passkey-dialog-overlay">
                            <div class="passkey-dialog">
                                <h3>{"Verification Failed"}</h3>
                                <p class="error-message">{msg}</p>
                                <button class="passkey-btn passkey-cancel" onclick={on_passkey_cancel.clone()}>{"Close"}</button>
                            </div>
                        </div>
                    },
                }
            }

            <div class="tesla-controls-header">
                <img src="https://upload.wikimedia.org/wikipedia/commons/b/bb/Tesla_T_symbol.svg" alt="Tesla" width="32" height="32"/>
                <span class="vehicle-name">
                    {(*selected_vehicle_name).clone().unwrap_or_else(|| "Tesla".to_string())}
                </span>
                <button
                    onclick={handle_refresh}
                    disabled={*battery_loading}
                    class="refresh-button"
                >
                    {if *battery_loading {
                        html! { <><i class="fas fa-spinner fa-spin"></i></> }
                    } else {
                        html! { <i class="fas fa-sync-alt"></i> }
                    }}
                </button>
                {
                    if let Some(time) = (*last_refresh_time).clone() {
                        html! { <span class="last-refresh">{format!("Updated {}", time)}</span> }
                    } else {
                        html! {}
                    }
                }
            </div>

            // Status section
            <div class="tesla-status">
                {
                    if let Some(level) = *battery_level {
                        let icon_class = if level <= 10 { "fa-battery-empty" }
                            else if level <= 35 { "fa-battery-quarter" }
                            else if level <= 60 { "fa-battery-half" }
                            else if level <= 90 { "fa-battery-three-quarters" }
                            else { "fa-battery-full" };
                        html! {
                            <div class="status-row">
                                <i class={format!("fa-solid {}", icon_class)} style="font-size: 24px; color: #7EB2FF;"></i>
                                <div class="status-info">
                                    <span class="status-main">{format!("{}%", level)}</span>
                                    {
                                        if let Some(range) = *battery_range {
                                            html! { <span class="status-sub">{format!("{:.0} mi", range)}</span> }
                                        } else { html! {} }
                                    }
                                </div>
                                {
                                    if let Some(state) = (*charging_state).as_ref() {
                                        html! { <span class="charging-state">{state}</span> }
                                    } else { html! {} }
                                }
                            </div>
                        }
                    } else if *battery_loading {
                        html! {
                            <div class="status-loading">
                                <i class="fas fa-spinner fa-spin"></i>
                                {" Loading status..."}
                            </div>
                        }
                    } else {
                        html! {
                            <div class="status-empty">{"Click refresh to load status"}</div>
                        }
                    }
                }

                // Temperature info
                {
                    if inside_temp.is_some() || outside_temp.is_some() {
                        html! {
                            <div class="temp-row">
                                {
                                    if let Some(temp) = *inside_temp {
                                        html! { <span class="temp-item">{"🏠 "}{format!("{:.1}°C", temp)}</span> }
                                    } else { html! {} }
                                }
                                {
                                    if let Some(temp) = *outside_temp {
                                        html! { <span class="temp-item">{"🌡️ "}{format!("{:.1}°C", temp)}</span> }
                                    } else { html! {} }
                                }
                            </div>
                        }
                    } else { html! {} }
                }
            </div>

            // Control buttons
            <div class="tesla-control-buttons">
                <button
                    onclick={handle_lock}
                    disabled={*lock_loading}
                    class="control-btn"
                >
                    {
                        if *lock_loading {
                            html! { <i class="fas fa-spinner fa-spin"></i> }
                        } else if let Some(locked) = *is_locked {
                            if locked {
                                html! { <><i class="fas fa-lock"></i>{" Locked"}</> }
                            } else {
                                html! { <><i class="fas fa-unlock"></i>{" Unlocked"}</> }
                            }
                        } else {
                            html! { <><i class="fas fa-lock"></i>{" Lock"}</> }
                        }
                    }
                </button>

                <button
                    onclick={handle_climate}
                    disabled={*climate_loading}
                    class="control-btn"
                >
                    {
                        if *climate_loading {
                            html! { <i class="fas fa-spinner fa-spin"></i> }
                        } else if let Some(on) = *is_climate_on {
                            if on {
                                html! { <><i class="fas fa-fan"></i>{" Climate On"}</> }
                            } else {
                                html! { <><i class="fas fa-fan"></i>{" Climate Off"}</> }
                            }
                        } else {
                            html! { <><i class="fas fa-fan"></i>{" Climate"}</> }
                        }
                    }
                </button>

                <button
                    onclick={handle_defrost}
                    disabled={*defrost_loading}
                    class="control-btn"
                >
                    {
                        if *defrost_loading {
                            html! { <i class="fas fa-spinner fa-spin"></i> }
                        } else {
                            let front_on = is_front_defroster_on.unwrap_or(false);
                            let rear_on = is_rear_defroster_on.unwrap_or(false);
                            if front_on || rear_on {
                                html! { <><i class="fas fa-snowflake"></i>{" Defrost On"}</> }
                            } else {
                                html! { <><i class="fas fa-snowflake"></i>{" Defrost"}</> }
                            }
                        }
                    }
                </button>

                <button
                    onclick={handle_remote_start}
                    disabled={*remote_start_loading}
                    class="control-btn control-btn-warning"
                >
                    {
                        if *remote_start_loading {
                            html! { <i class="fas fa-spinner fa-spin"></i> }
                        } else {
                            html! { <><i class="fas fa-key"></i>{" Start"}</> }
                        }
                    }
                </button>
            </div>

            // Cabin Overheat Protection section
            <div class="cabin-overheat-section">
                <h4 class="section-title">{"Cabin Overheat Protection"}</h4>
                <div class="cabin-overheat-buttons">
                    <button
                        onclick={
                            let cb = handle_cabin_overheat.clone();
                            Callback::from(move |_: MouseEvent| cb.emit("cabin_overheat_on".to_string()))
                        }
                        disabled={*cabin_overheat_loading}
                        class="cabin-btn"
                    >
                        {
                            if *cabin_overheat_loading {
                                html! { <i class="fas fa-spinner fa-spin"></i> }
                            } else {
                                html! { <><i class="fas fa-temperature-high"></i>{" On (A/C)"}</> }
                            }
                        }
                    </button>
                    <button
                        onclick={
                            let cb = handle_cabin_overheat.clone();
                            Callback::from(move |_: MouseEvent| cb.emit("cabin_overheat_fan_only".to_string()))
                        }
                        disabled={*cabin_overheat_loading}
                        class="cabin-btn"
                    >
                        {
                            if *cabin_overheat_loading {
                                html! { <i class="fas fa-spinner fa-spin"></i> }
                            } else {
                                html! { <><i class="fas fa-fan"></i>{" Fan Only"}</> }
                            }
                        }
                    </button>
                    <button
                        onclick={
                            let cb = handle_cabin_overheat.clone();
                            Callback::from(move |_: MouseEvent| cb.emit("cabin_overheat_off".to_string()))
                        }
                        disabled={*cabin_overheat_loading}
                        class="cabin-btn cabin-btn-off"
                    >
                        {
                            if *cabin_overheat_loading {
                                html! { <i class="fas fa-spinner fa-spin"></i> }
                            } else {
                                html! { <><i class="fas fa-power-off"></i>{" Off"}</> }
                            }
                        }
                    </button>
                </div>
            </div>

            // Notification buttons section
            <div class="notify-buttons-section">
                // Climate notify button - only show when climate is on
                {
                    if is_climate_on.unwrap_or(false) {
                        html! {
                            <button
                                onclick={handle_climate_notify}
                                disabled={*climate_notify_loading}
                                class={format!("notify-btn {}", if *climate_notify_active { "notify-btn-active" } else { "" })}
                            >
                                {
                                    if *climate_notify_loading {
                                        html! { <i class="fas fa-spinner fa-spin"></i> }
                                    } else if *climate_notify_active {
                                        html! { <><i class="fas fa-bell-slash"></i>{" Cancel Climate Notification"}</> }
                                    } else {
                                        html! { <><i class="fas fa-bell"></i>{" Notify When Ready"}</> }
                                    }
                                }
                            </button>
                        }
                    } else {
                        html! {}
                    }
                }

                // Charging notify button - only show when charging
                {
                    if charging_state.as_ref().map(|s| s == "Charging").unwrap_or(false) {
                        html! {
                            <button
                                onclick={handle_charging_notify}
                                disabled={*charging_notify_loading}
                                class={format!("notify-btn {}", if *charging_notify_active { "notify-btn-active" } else { "" })}
                            >
                                {
                                    if *charging_notify_loading {
                                        html! { <i class="fas fa-spinner fa-spin"></i> }
                                    } else if *charging_notify_active {
                                        html! { <><i class="fas fa-bell-slash"></i>{" Cancel Charging Notification"}</> }
                                    } else {
                                        html! { <><i class="fas fa-bell"></i>{" Notify When Charged"}</> }
                                    }
                                }
                            </button>
                        }
                    } else {
                        html! {}
                    }
                }

                // Hint text - show when any notify button is visible
                {
                    if is_climate_on.unwrap_or(false) || charging_state.as_ref().map(|s| s == "Charging").unwrap_or(false) {
                        html! {
                            <div class="notify-hint">
                                {"Won't notify if you're in the vehicle"}
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }
            </div>

            // Command result
            {
                if let Some(result) = (*command_result).as_ref() {
                    html! {
                        <div class="command-result">
                            {result}
                        </div>
                    }
                } else { html! {} }
            }

            <style>{get_styles()}</style>
        </div>
    }
}

fn get_styles() -> &'static str {
    r#"
        .tesla-controls-container {
            background: rgba(30, 30, 30, 0.7);
            border: 1px solid rgba(30, 144, 255, 0.1);
            border-radius: 16px;
            padding: 1.5rem;
            backdrop-filter: blur(10px);
        }
        .tesla-controls-loading {
            text-align: center;
            color: #7EB2FF;
            padding: 2rem;
        }
        .tesla-not-connected {
            text-align: center;
            padding: 2rem;
        }
        .tesla-not-connected h3 {
            color: #fff;
            margin-bottom: 0.5rem;
        }
        .tesla-not-connected p {
            color: #999;
            margin-bottom: 1rem;
        }
        .connect-link {
            display: inline-block;
            background: linear-gradient(45deg, #1E90FF, #4169E1);
            color: white;
            padding: 0.75rem 1.5rem;
            border-radius: 8px;
            text-decoration: none;
            transition: all 0.3s ease;
        }
        .connect-link:hover {
            transform: translateY(-2px);
            box-shadow: 0 4px 20px rgba(30, 144, 255, 0.3);
        }
        .tesla-controls-header {
            display: flex;
            align-items: center;
            gap: 12px;
            margin-bottom: 1rem;
            flex-wrap: wrap;
        }
        .vehicle-name {
            color: #fff;
            font-size: 1.1rem;
            font-weight: 500;
        }
        .refresh-button {
            padding: 8px 12px;
            background: rgba(30, 144, 255, 0.15);
            color: #7EB2FF;
            border: 1px solid rgba(30, 144, 255, 0.3);
            border-radius: 8px;
            cursor: pointer;
            margin-left: auto;
        }
        .refresh-button:disabled {
            opacity: 0.6;
            cursor: not-allowed;
        }
        .last-refresh {
            color: #666;
            font-size: 12px;
        }
        .tesla-status {
            background: rgba(0, 0, 0, 0.2);
            border: 1px solid rgba(30, 144, 255, 0.1);
            border-radius: 12px;
            padding: 1rem;
            margin-bottom: 1rem;
        }
        .status-row {
            display: flex;
            align-items: center;
            gap: 12px;
        }
        .status-info {
            display: flex;
            flex-direction: column;
        }
        .status-main {
            color: #fff;
            font-size: 1.2rem;
            font-weight: 600;
        }
        .status-sub {
            color: #999;
            font-size: 0.9rem;
        }
        .charging-state {
            color: #69f0ae;
            font-size: 0.85rem;
            margin-left: auto;
        }
        .status-loading, .status-empty {
            color: #999;
            text-align: center;
            padding: 1rem;
        }
        .temp-row {
            display: flex;
            gap: 1rem;
            margin-top: 0.75rem;
            padding-top: 0.75rem;
            border-top: 1px solid rgba(30, 144, 255, 0.1);
        }
        .temp-item {
            color: #999;
            font-size: 0.9rem;
        }
        .tesla-control-buttons {
            display: grid;
            grid-template-columns: repeat(2, 1fr);
            gap: 12px;
        }
        .control-btn {
            padding: 14px 20px;
            background: rgba(30, 144, 255, 0.1);
            color: #7EB2FF;
            border: 1px solid rgba(30, 144, 255, 0.2);
            border-radius: 8px;
            font-size: 14px;
            cursor: pointer;
            transition: all 0.2s;
            display: flex;
            align-items: center;
            justify-content: center;
            gap: 8px;
        }
        .control-btn:hover:not(:disabled) {
            background: rgba(30, 144, 255, 0.2);
            border-color: rgba(30, 144, 255, 0.4);
        }
        .control-btn:disabled {
            opacity: 0.6;
            cursor: not-allowed;
        }
        .control-btn-warning {
            background: rgba(255, 152, 0, 0.1);
            color: #FFB74D;
            border-color: rgba(255, 152, 0, 0.2);
        }
        .control-btn-warning:hover:not(:disabled) {
            background: rgba(255, 152, 0, 0.2);
            border-color: rgba(255, 152, 0, 0.4);
        }
        .command-result {
            margin-top: 1rem;
            padding: 10px;
            background: rgba(105, 240, 174, 0.1);
            color: #69f0ae;
            border-radius: 8px;
            font-size: 14px;
            border: 1px solid rgba(105, 240, 174, 0.2);
            text-align: center;
        }
        @media (max-width: 480px) {
            .tesla-control-buttons {
                grid-template-columns: 1fr;
            }
        }
        .passkey-dialog-overlay {
            position: fixed;
            top: 0;
            left: 0;
            right: 0;
            bottom: 0;
            background: rgba(0, 0, 0, 0.8);
            display: flex;
            align-items: center;
            justify-content: center;
            z-index: 1000;
        }
        .passkey-dialog {
            background: #1a1a1a;
            border: 1px solid rgba(30, 144, 255, 0.3);
            border-radius: 16px;
            padding: 1.5rem;
            max-width: 400px;
            width: 90%;
            text-align: center;
        }
        .passkey-dialog h3 {
            color: #fff;
            margin-bottom: 1rem;
        }
        .passkey-dialog p {
            color: #999;
            margin-bottom: 1.5rem;
        }
        .passkey-options {
            display: flex;
            flex-direction: column;
            gap: 12px;
            margin-bottom: 1rem;
        }
        .passkey-btn {
            padding: 12px 20px;
            background: rgba(30, 144, 255, 0.2);
            color: #7EB2FF;
            border: 1px solid rgba(30, 144, 255, 0.3);
            border-radius: 8px;
            cursor: pointer;
            font-size: 14px;
            transition: all 0.2s;
        }
        .passkey-btn:hover {
            background: rgba(30, 144, 255, 0.3);
        }
        .passkey-cancel {
            background: rgba(255, 255, 255, 0.1);
            color: #999;
            border-color: rgba(255, 255, 255, 0.2);
        }
        .passkey-cancel:hover {
            background: rgba(255, 255, 255, 0.2);
        }
        .totp-input {
            width: 100%;
            padding: 12px;
            background: rgba(0, 0, 0, 0.3);
            border: 1px solid rgba(30, 144, 255, 0.3);
            border-radius: 8px;
            color: #fff;
            font-size: 18px;
            text-align: center;
            letter-spacing: 4px;
            margin-bottom: 1rem;
        }
        .totp-input::placeholder {
            color: #666;
            letter-spacing: 4px;
        }
        .error-message {
            color: #ff6b6b;
        }
        .loading-spinner {
            display: inline-block;
            width: 24px;
            height: 24px;
            border: 3px solid rgba(30, 144, 255, 0.3);
            border-radius: 50%;
            border-top-color: #7EB2FF;
            animation: spin 1s linear infinite;
            margin: 1rem auto;
        }
        @keyframes spin {
            to { transform: rotate(360deg); }
        }
        .notify-buttons-section {
            display: flex;
            flex-direction: column;
            gap: 10px;
            margin-top: 1rem;
            padding-top: 1rem;
            border-top: 1px solid rgba(30, 144, 255, 0.1);
        }
        .notify-btn {
            padding: 12px 16px;
            background: rgba(105, 240, 174, 0.1);
            color: #69f0ae;
            border: 1px solid rgba(105, 240, 174, 0.3);
            border-radius: 8px;
            cursor: pointer;
            font-size: 14px;
            transition: all 0.2s;
            display: flex;
            align-items: center;
            justify-content: center;
            gap: 8px;
        }
        .notify-btn:hover {
            background: rgba(105, 240, 174, 0.2);
        }
        .notify-btn:disabled {
            opacity: 0.6;
            cursor: not-allowed;
        }
        .notify-btn-active {
            background: rgba(255, 152, 0, 0.1);
            color: #FFB74D;
            border-color: rgba(255, 152, 0, 0.3);
        }
        .notify-btn-active:hover {
            background: rgba(255, 152, 0, 0.2);
        }
        .notify-hint {
            width: 100%;
            text-align: center;
            color: #666;
            font-size: 12px;
            margin-top: 8px;
            font-style: italic;
        }
        .cabin-overheat-section {
            margin-top: 1.5rem;
            padding-top: 1rem;
            border-top: 1px solid rgba(30, 144, 255, 0.1);
        }
        .cabin-overheat-section .section-title {
            color: #7EB2FF;
            font-size: 14px;
            font-weight: 500;
            margin-bottom: 12px;
        }
        .cabin-overheat-buttons {
            display: flex;
            gap: 10px;
            flex-wrap: wrap;
        }
        .cabin-btn {
            flex: 1;
            min-width: 90px;
            padding: 12px 16px;
            background: rgba(255, 152, 0, 0.1);
            color: #FFB74D;
            border: 1px solid rgba(255, 152, 0, 0.3);
            border-radius: 8px;
            cursor: pointer;
            font-size: 14px;
            transition: all 0.2s;
            display: flex;
            align-items: center;
            justify-content: center;
            gap: 8px;
        }
        .cabin-btn:hover {
            background: rgba(255, 152, 0, 0.2);
        }
        .cabin-btn:disabled {
            opacity: 0.6;
            cursor: not-allowed;
        }
        .cabin-btn-off {
            background: rgba(100, 100, 100, 0.1);
            color: #999;
            border-color: rgba(100, 100, 100, 0.3);
        }
        .cabin-btn-off:hover {
            background: rgba(100, 100, 100, 0.2);
        }
    "#
}

// Helper function to execute Tesla command
async fn execute_tesla_command(
    command: &str,
    loading: UseStateHandle<bool>,
    command_result: UseStateHandle<Option<String>>,
    is_locked: Option<UseStateHandle<Option<bool>>>,
) {
    let body = serde_json::json!({ "command": command });

    let request = match Api::post("/api/tesla/command").json(&body) {
        Ok(req) => req.send().await,
        Err(e) => {
            command_result.set(Some(format!("Failed: {}", e)));
            loading.set(false);
            return;
        }
    };

    match request {
        Ok(response) => {
            if response.ok() {
                // Update lock state if applicable
                if let Some(is_locked) = is_locked {
                    match command {
                        "lock" => is_locked.set(Some(true)),
                        "unlock" => is_locked.set(Some(false)),
                        _ => {}
                    }
                }
                if let Ok(data) = response.json::<serde_json::Value>().await {
                    if let Some(msg) = data.get("message").and_then(|m| m.as_str()) {
                        command_result.set(Some(msg.to_string()));
                    }
                }
            } else {
                command_result.set(Some("Failed to execute command".to_string()));
            }
        }
        Err(e) => {
            command_result.set(Some(format!("Network error: {}", e)));
        }
    }
    loading.set(false);
}
