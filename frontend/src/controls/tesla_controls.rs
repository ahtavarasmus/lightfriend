use yew::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlInputElement, EventSource, MessageEvent};
use crate::utils::api::Api;
use crate::utils::webauthn;
use crate::components::feature_preview::FeaturePreview;
use crate::config;
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
    let precondition_loading = use_state(|| false);
    let command_result = use_state(|| None::<String>);
    let status_message = use_state(|| None::<String>);  // SSE status updates

    // Passkey verification state
    let passkey_state = use_state(|| PasskeyVerifyState::Hidden);
    let pending_passkey_options = use_state(|| None::<serde_json::Value>);
    let pending_command = use_state(|| String::new());
    let totp_code_input = use_state(String::new);
    let battery_level = use_state(|| None::<i32>);
    let battery_range = use_state(|| None::<f64>);
    let charging_state = use_state(|| None::<String>);
    let charge_limit_soc = use_state(|| None::<i32>);
    let charge_rate = use_state(|| None::<f64>);
    let charger_power = use_state(|| None::<i32>);
    let time_to_full_charge = use_state(|| None::<f64>);
    let charge_energy_added = use_state(|| None::<f64>);
    let uses_miles = use_state(|| true);  // Default to miles, updated from API
    let charge_limit_loading = use_state(|| false);
    let charge_limit_editing = use_state(|| false);
    let charge_limit_input = use_state(|| 80i32);
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

    // Scope-based feature gating state
    // Default to true until we've checked - prevents flashing disabled state
    let has_vehicle_device_data = use_state(|| true);
    let has_vehicle_cmds = use_state(|| true);
    let has_vehicle_charging_cmds = use_state(|| true);

    // Auto-refresh state - pauses after 30 minutes of inactivity
    let auto_refresh_paused = use_state(|| false);
    let refresh_count = use_state(|| 0u32);  // Count refreshes, pause after 60 (30 min at 30s interval)

    // Track if we just turned climate on (to detect auto-defrost)
    let climate_just_activated = use_state(|| false);
    let defrost_was_off_before_climate = use_state(|| false);

    // Remote start countdown timer (seconds remaining)
    let remote_start_countdown = use_state(|| None::<u32>);

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

    // Remote start countdown timer effect
    {
        let remote_start_countdown = remote_start_countdown.clone();
        let remote_start_countdown_dep = remote_start_countdown.clone();

        use_effect_with_deps(
            move |countdown: &Option<u32>| {
                let interval_id: Rc<RefCell<Option<i32>>> = Rc::new(RefCell::new(None));

                if let Some(secs) = countdown {
                    if *secs > 0 {
                        if let Some(window) = web_sys::window() {
                            let remote_start_countdown = remote_start_countdown.clone();
                            let interval_id_inner = interval_id.clone();

                            let interval_callback = Closure::wrap(Box::new(move || {
                                let current = *remote_start_countdown;
                                if let Some(secs) = current {
                                    if secs > 1 {
                                        remote_start_countdown.set(Some(secs - 1));
                                    } else {
                                        remote_start_countdown.set(None);
                                        // Clear the interval when done
                                        if let Some(id) = *interval_id_inner.borrow() {
                                            if let Some(w) = web_sys::window() {
                                                w.clear_interval_with_handle(id);
                                            }
                                        }
                                    }
                                }
                            }) as Box<dyn Fn()>);

                            if let Ok(id) = window.set_interval_with_callback_and_timeout_and_arguments_0(
                                interval_callback.as_ref().unchecked_ref(),
                                1000, // 1 second
                            ) {
                                *interval_id.borrow_mut() = Some(id);
                            }
                            interval_callback.forget();
                        }
                    }
                }

                // Cleanup
                let interval_id_cleanup = interval_id;
                move || {
                    if let Some(id) = *interval_id_cleanup.borrow() {
                        if let Some(window) = web_sys::window() {
                            window.clear_interval_with_handle(id);
                        }
                    }
                }
            },
            (*remote_start_countdown_dep).clone(),
        );
    }

    // Detect auto-defrost activation when climate is turned on
    {
        let is_front_defroster_on = is_front_defroster_on.clone();
        let is_rear_defroster_on = is_rear_defroster_on.clone();
        let climate_just_activated = climate_just_activated.clone();
        let defrost_was_off_before_climate = defrost_was_off_before_climate.clone();
        let command_result = command_result.clone();

        use_effect_with_deps(
            move |(front, rear)| {
                let defrost_now_on = front.unwrap_or(false) || rear.unwrap_or(false);

                // If climate was just activated, defrost was off before, and now defrost is on
                if *climate_just_activated && *defrost_was_off_before_climate && defrost_now_on {
                    command_result.set(Some("Defrost auto-activated by Tesla (cold weather)".to_string()));
                    // Reset the tracking flags
                    climate_just_activated.set(false);
                    defrost_was_off_before_climate.set(false);
                }
                || ()
            },
            (*is_front_defroster_on, *is_rear_defroster_on),
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
        let charge_limit_soc = charge_limit_soc.clone();
        let charge_rate = charge_rate.clone();
        let charger_power = charger_power.clone();
        let time_to_full_charge = time_to_full_charge.clone();
        let charge_energy_added = charge_energy_added.clone();
        let uses_miles = uses_miles.clone();
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
                    let charge_limit_soc = charge_limit_soc.clone();
                    let charge_rate = charge_rate.clone();
                    let charger_power = charger_power.clone();
                    let time_to_full_charge = time_to_full_charge.clone();
                    let charge_energy_added = charge_energy_added.clone();
                    let uses_miles = uses_miles.clone();
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
                                        if let Some(limit) = data["charge_limit_soc"].as_i64() {
                                            charge_limit_soc.set(Some(limit as i32));
                                        }
                                        charge_rate.set(data["charge_rate"].as_f64());
                                        charger_power.set(data["charger_power"].as_i64().map(|p| p as i32));
                                        time_to_full_charge.set(data["time_to_full_charge"].as_f64());
                                        charge_energy_added.set(data["charge_energy_added"].as_f64());
                                        uses_miles.set(data["uses_miles"].as_bool().unwrap_or(true));
                                        if let Some(locked) = data["locked"].as_bool() {
                                            is_locked.set(Some(locked));
                                        }
                                        if let Some(temp) = data["inside_temp"].as_f64() {
                                            inside_temp.set(Some(temp));
                                        }
                                        if let Some(temp) = data["outside_temp"].as_f64() {
                                            outside_temp.set(Some(temp));
                                        }
                                        // For climate/defrost: if field is present, use it; if null, default to false
                                        is_climate_on.set(Some(data["is_climate_on"].as_bool().unwrap_or(false)));
                                        is_front_defroster_on.set(Some(data["is_front_defroster_on"].as_bool().unwrap_or(false)));
                                        is_rear_defroster_on.set(Some(data["is_rear_defroster_on"].as_bool().unwrap_or(false)));
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

    // Auto-refresh status every 30 seconds when visible (pauses after 30 min)
    {
        let tesla_connected = tesla_connected.clone();
        let battery_level = battery_level.clone();
        let battery_range = battery_range.clone();
        let charging_state = charging_state.clone();
        let charge_limit_soc = charge_limit_soc.clone();
        let charge_rate = charge_rate.clone();
        let charger_power = charger_power.clone();
        let time_to_full_charge = time_to_full_charge.clone();
        let charge_energy_added = charge_energy_added.clone();
        let uses_miles = uses_miles.clone();
        let is_locked = is_locked.clone();
        let inside_temp = inside_temp.clone();
        let outside_temp = outside_temp.clone();
        let is_climate_on = is_climate_on.clone();
        let is_front_defroster_on = is_front_defroster_on.clone();
        let is_rear_defroster_on = is_rear_defroster_on.clone();
        let last_refresh_time = last_refresh_time.clone();
        let last_refresh_epoch = last_refresh_epoch.clone();
        let climate_notify_active = climate_notify_active.clone();
        let charging_notify_active = charging_notify_active.clone();
        let auto_refresh_paused = auto_refresh_paused.clone();
        let refresh_count = refresh_count.clone();

        use_effect_with_deps(
            move |connected| {
                let interval_id: Rc<RefCell<Option<i32>>> = Rc::new(RefCell::new(None));

                if **connected {
                    if let Some(window) = web_sys::window() {
                        let battery_level = battery_level.clone();
                        let battery_range = battery_range.clone();
                        let charging_state = charging_state.clone();
                        let charge_limit_soc = charge_limit_soc.clone();
                        let charge_rate = charge_rate.clone();
                        let charger_power = charger_power.clone();
                        let time_to_full_charge = time_to_full_charge.clone();
                        let charge_energy_added = charge_energy_added.clone();
                        let uses_miles = uses_miles.clone();
                        let is_locked = is_locked.clone();
                        let inside_temp = inside_temp.clone();
                        let outside_temp = outside_temp.clone();
                        let is_climate_on = is_climate_on.clone();
                        let is_front_defroster_on = is_front_defroster_on.clone();
                        let is_rear_defroster_on = is_rear_defroster_on.clone();
                        let last_refresh_time = last_refresh_time.clone();
                        let last_refresh_epoch = last_refresh_epoch.clone();
                        let climate_notify_active = climate_notify_active.clone();
                        let charging_notify_active = charging_notify_active.clone();
                        let auto_refresh_paused = auto_refresh_paused.clone();
                        let refresh_count = refresh_count.clone();

                        let interval_callback = Closure::wrap(Box::new(move || {
                            // Don't refresh if paused
                            if *auto_refresh_paused {
                                return;
                            }

                            // Check if document is visible
                            let is_visible = web_sys::window()
                                .and_then(|w| w.document())
                                .map(|d| !d.hidden())
                                .unwrap_or(false);

                            if !is_visible {
                                return;
                            }

                            // Check refresh count - pause after 60 refreshes (30 min)
                            let count = *refresh_count + 1;
                            if count >= 60 {
                                auto_refresh_paused.set(true);
                                return;
                            }
                            refresh_count.set(count);

                            let battery_level = battery_level.clone();
                            let battery_range = battery_range.clone();
                            let charging_state = charging_state.clone();
                            let charge_limit_soc = charge_limit_soc.clone();
                            let charge_rate = charge_rate.clone();
                            let charger_power = charger_power.clone();
                            let time_to_full_charge = time_to_full_charge.clone();
                            let charge_energy_added = charge_energy_added.clone();
                            let uses_miles = uses_miles.clone();
                            let is_locked = is_locked.clone();
                            let inside_temp = inside_temp.clone();
                            let outside_temp = outside_temp.clone();
                            let is_climate_on = is_climate_on.clone();
                            let is_front_defroster_on = is_front_defroster_on.clone();
                            let is_rear_defroster_on = is_rear_defroster_on.clone();
                            let last_refresh_time = last_refresh_time.clone();
                            let last_refresh_epoch = last_refresh_epoch.clone();
                            let climate_notify_active = climate_notify_active.clone();
                            let charging_notify_active = charging_notify_active.clone();

                            spawn_local(async move {
                                // Silent refresh - no loading indicator
                                if let Ok(response) = Api::get("/api/tesla/battery-status").send().await {
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
                                            if let Some(limit) = data["charge_limit_soc"].as_i64() {
                                                charge_limit_soc.set(Some(limit as i32));
                                            }
                                            charge_rate.set(data["charge_rate"].as_f64());
                                            charger_power.set(data["charger_power"].as_i64().map(|p| p as i32));
                                            time_to_full_charge.set(data["time_to_full_charge"].as_f64());
                                            charge_energy_added.set(data["charge_energy_added"].as_f64());
                                            uses_miles.set(data["uses_miles"].as_bool().unwrap_or(true));
                                            if let Some(locked) = data["locked"].as_bool() {
                                                is_locked.set(Some(locked));
                                            }
                                            if let Some(temp) = data["inside_temp"].as_f64() {
                                                inside_temp.set(Some(temp));
                                            }
                                            if let Some(temp) = data["outside_temp"].as_f64() {
                                                outside_temp.set(Some(temp));
                                            }
                                            // For climate/defrost: if field is present, use it; if null, default to false
                                            is_climate_on.set(Some(data["is_climate_on"].as_bool().unwrap_or(false)));
                                            is_front_defroster_on.set(Some(data["is_front_defroster_on"].as_bool().unwrap_or(false)));
                                            is_rear_defroster_on.set(Some(data["is_rear_defroster_on"].as_bool().unwrap_or(false)));
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

                                // Also refresh notify statuses
                                if let Ok(response) = Api::get("/api/tesla/climate-notify/status").send().await {
                                    if response.ok() {
                                        if let Ok(data) = response.json::<serde_json::Value>().await {
                                            if let Some(active) = data["active"].as_bool() {
                                                climate_notify_active.set(active);
                                            }
                                        }
                                    }
                                }
                                if let Ok(response) = Api::get("/api/tesla/charging-notify/status").send().await {
                                    if response.ok() {
                                        if let Ok(data) = response.json::<serde_json::Value>().await {
                                            if let Some(active) = data["active"].as_bool() {
                                                charging_notify_active.set(active);
                                            }
                                        }
                                    }
                                }
                            });
                        }) as Box<dyn Fn()>);

                        if let Ok(id) = window.set_interval_with_callback_and_timeout_and_arguments_0(
                            interval_callback.as_ref().unchecked_ref(),
                            30000, // 30 seconds
                        ) {
                            *interval_id.borrow_mut() = Some(id);
                        }
                        interval_callback.forget();
                    }
                }

                // Cleanup
                let interval_id_cleanup = interval_id;
                move || {
                    if let Some(id) = *interval_id_cleanup.borrow() {
                        if let Some(window) = web_sys::window() {
                            window.clear_interval_with_handle(id);
                        }
                    }
                }
            },
            tesla_connected.clone(),
        );
    }

    // Listen for chat-sent event to trigger immediate refresh
    {
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
        let climate_notify_active = climate_notify_active.clone();
        let charging_notify_active = charging_notify_active.clone();
        let tesla_connected = tesla_connected.clone();

        use_effect_with_deps(
            move |_| {
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
                let climate_notify_active = climate_notify_active.clone();
                let charging_notify_active = charging_notify_active.clone();
                let tesla_connected = tesla_connected.clone();

                let closure = Closure::<dyn Fn()>::new(move || {
                    if !*tesla_connected {
                        return;
                    }
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
                    let climate_notify_active = climate_notify_active.clone();
                    let charging_notify_active = charging_notify_active.clone();

                    spawn_local(async move {
                        // Same refresh logic as auto-refresh
                        if let Ok(response) = Api::get("/api/tesla/battery-status").send().await {
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
                                    is_climate_on.set(Some(data["is_climate_on"].as_bool().unwrap_or(false)));
                                    is_front_defroster_on.set(Some(data["is_front_defroster_on"].as_bool().unwrap_or(false)));
                                    is_rear_defroster_on.set(Some(data["is_rear_defroster_on"].as_bool().unwrap_or(false)));
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

                        // Also refresh notify statuses
                        if let Ok(response) = Api::get("/api/tesla/climate-notify/status").send().await {
                            if response.ok() {
                                if let Ok(data) = response.json::<serde_json::Value>().await {
                                    if let Some(active) = data["active"].as_bool() {
                                        climate_notify_active.set(active);
                                    }
                                }
                            }
                        }
                        if let Ok(response) = Api::get("/api/tesla/charging-notify/status").send().await {
                            if response.ok() {
                                if let Ok(data) = response.json::<serde_json::Value>().await {
                                    if let Some(active) = data["active"].as_bool() {
                                        charging_notify_active.set(active);
                                    }
                                }
                            }
                        }
                    });
                });

                let window = web_sys::window().unwrap();
                let func = closure.as_ref().unchecked_ref::<js_sys::Function>().clone();
                let _ = window.add_event_listener_with_callback("lightfriend-chat-sent", &func);

                move || {
                    if let Some(window) = web_sys::window() {
                        let _ = window.remove_event_listener_with_callback("lightfriend-chat-sent", &func);
                    }
                    drop(closure);
                }
            },
            (),
        );
    }

    // Fetch granted scopes when connected for feature gating
    {
        let tesla_connected = tesla_connected.clone();
        let has_vehicle_device_data = has_vehicle_device_data.clone();
        let has_vehicle_cmds = has_vehicle_cmds.clone();
        let has_vehicle_charging_cmds = has_vehicle_charging_cmds.clone();

        use_effect_with_deps(
            move |connected| {
                if **connected {
                    spawn_local(async move {
                        match Api::get("/api/auth/tesla/scopes").send().await {
                            Ok(response) => {
                                if response.ok() {
                                    if let Ok(data) = response.json::<serde_json::Value>().await {
                                        if let Some(v) = data["has_vehicle_device_data"].as_bool() {
                                            has_vehicle_device_data.set(v);
                                        }
                                        if let Some(v) = data["has_vehicle_cmds"].as_bool() {
                                            has_vehicle_cmds.set(v);
                                        }
                                        if let Some(v) = data["has_vehicle_charging_cmds"].as_bool() {
                                            has_vehicle_charging_cmds.set(v);
                                        }
                                    }
                                }
                            }
                            Err(_) => {
                                // If fetch fails, default to allowing everything (backwards compat)
                            }
                        }
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

    // Handle climate with SSE streaming for status updates
    let handle_climate = {
        let climate_loading = climate_loading.clone();
        let command_result = command_result.clone();
        let status_message = status_message.clone();
        let is_climate_on = is_climate_on.clone();
        let is_front_defroster_on = is_front_defroster_on.clone();
        let is_rear_defroster_on = is_rear_defroster_on.clone();
        let climate_just_activated = climate_just_activated.clone();
        let defrost_was_off_before_climate = defrost_was_off_before_climate.clone();

        Callback::from(move |_: MouseEvent| {
            let climate_loading = climate_loading.clone();
            let command_result = command_result.clone();
            let status_message = status_message.clone();
            let is_climate_on = is_climate_on.clone();
            let is_front_defroster_on = is_front_defroster_on.clone();
            let is_rear_defroster_on = is_rear_defroster_on.clone();
            let climate_just_activated = climate_just_activated.clone();
            let defrost_was_off_before_climate = defrost_was_off_before_climate.clone();

            climate_loading.set(true);
            command_result.set(None);
            status_message.set(None);

            // Remember if defrost was off before we turn climate on
            let defrost_currently_off = !is_front_defroster_on.unwrap_or(false) && !is_rear_defroster_on.unwrap_or(false);

            let command = match *is_climate_on {
                Some(true) => "climate_off",
                Some(false) => "climate_on",
                None => "climate_on",
            };

            // Build the SSE URL (auth via cookies)
            let url = format!("{}/api/tesla/command-stream?command={}",
                config::get_backend_url(), command);

            // Create EventSource for SSE
            match EventSource::new(&url) {
                Ok(event_source) => {
                    let es = Rc::new(event_source);

                    // Clone for closures
                    let es_message = es.clone();
                    let es_error = es.clone();
                    let status_message_clone = status_message.clone();
                    let command_result_clone = command_result.clone();
                    let climate_loading_clone = climate_loading.clone();
                    let is_climate_on_clone = is_climate_on.clone();
                    let is_front_defroster_on_clone = is_front_defroster_on.clone();
                    let is_rear_defroster_on_clone = is_rear_defroster_on.clone();
                    let climate_just_activated_clone = climate_just_activated.clone();
                    let defrost_was_off_before_climate_clone = defrost_was_off_before_climate.clone();
                    let command_str = command.to_string();

                    // Handle incoming messages
                    let onmessage = Closure::wrap(Box::new(move |e: MessageEvent| {
                        if let Some(data) = e.data().as_string() {
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) {
                                let step = json.get("step").and_then(|s| s.as_str()).unwrap_or("");
                                let message = json.get("message").and_then(|m| m.as_str()).unwrap_or("");

                                match step {
                                    "done" => {
                                        // Success - update UI state
                                        status_message_clone.set(None);
                                        command_result_clone.set(Some(message.to_string()));
                                        climate_loading_clone.set(false);

                                        match command_str.as_str() {
                                            "climate_on" => {
                                                is_climate_on_clone.set(Some(true));
                                                climate_just_activated_clone.set(true);
                                                defrost_was_off_before_climate_clone.set(defrost_currently_off);
                                            }
                                            "climate_off" => {
                                                is_climate_on_clone.set(Some(false));
                                                is_front_defroster_on_clone.set(Some(false));
                                                is_rear_defroster_on_clone.set(Some(false));
                                                climate_just_activated_clone.set(false);
                                            }
                                            _ => {}
                                        }

                                        // Close the EventSource
                                        es_message.close();
                                    }
                                    "error" => {
                                        // Error - show error message
                                        status_message_clone.set(None);
                                        command_result_clone.set(Some(message.to_string()));
                                        climate_loading_clone.set(false);
                                        es_message.close();
                                    }
                                    _ => {
                                        // In-progress status update
                                        status_message_clone.set(Some(message.to_string()));
                                    }
                                }
                            }
                        }
                    }) as Box<dyn FnMut(_)>);

                    // Handle errors
                    let status_message_err = status_message.clone();
                    let command_result_err = command_result.clone();
                    let climate_loading_err = climate_loading.clone();

                    let onerror = Closure::wrap(Box::new(move |_: web_sys::Event| {
                        status_message_err.set(None);
                        command_result_err.set(Some("Connection error. Please try again.".to_string()));
                        climate_loading_err.set(false);
                        es_error.close();
                    }) as Box<dyn FnMut(_)>);

                    es.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
                    es.set_onerror(Some(onerror.as_ref().unchecked_ref()));

                    // Prevent closures from being dropped
                    onmessage.forget();
                    onerror.forget();
                }
                Err(_) => {
                    // Fallback to regular POST if EventSource fails
                    status_message.set(Some("Connecting...".to_string()));

                    spawn_local(async move {
                        let body = serde_json::json!({ "command": command });

                        let request = match Api::post("/api/tesla/command").json(&body) {
                            Ok(req) => req.send().await,
                            Err(e) => {
                                command_result.set(Some(format!("Failed: {}", e)));
                                status_message.set(None);
                                climate_loading.set(false);
                                return;
                            }
                        };

                        match request {
                            Ok(response) => {
                                if response.ok() {
                                    match command {
                                        "climate_on" => {
                                            is_climate_on.set(Some(true));
                                            climate_just_activated.set(true);
                                            defrost_was_off_before_climate.set(defrost_currently_off);
                                        }
                                        "climate_off" => {
                                            is_climate_on.set(Some(false));
                                            is_front_defroster_on.set(Some(false));
                                            is_rear_defroster_on.set(Some(false));
                                            climate_just_activated.set(false);
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
                        status_message.set(None);
                        climate_loading.set(false);
                    });
                }
            }
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
        let remote_start_countdown = remote_start_countdown.clone();

        Callback::from(move |_: MouseEvent| {
            let remote_start_loading = remote_start_loading.clone();
            let command_result = command_result.clone();
            let passkey_state = passkey_state.clone();
            let pending_passkey_options = pending_passkey_options.clone();
            let pending_command = pending_command.clone();
            let remote_start_countdown = remote_start_countdown.clone();

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
                                execute_tesla_command_with_countdown("remote_start", remote_start_loading, command_result, None, Some(remote_start_countdown)).await;
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

    // Handle precondition battery for fast charging
    let handle_precondition_battery = {
        let precondition_loading = precondition_loading.clone();
        let command_result = command_result.clone();

        Callback::from(move |_: MouseEvent| {
            let precondition_loading = precondition_loading.clone();
            let command_result = command_result.clone();

            precondition_loading.set(true);
            command_result.set(None);

            spawn_local(async move {
                let body = serde_json::json!({ "command": "precondition_battery" });
                let request = match Api::post("/api/tesla/command").json(&body) {
                    Ok(req) => req.send().await,
                    Err(e) => {
                        command_result.set(Some(format!("Failed to create request: {}", e)));
                        precondition_loading.set(false);
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
                            command_result.set(Some("Failed to start battery preconditioning".to_string()));
                        }
                    }
                    Err(e) => {
                        command_result.set(Some(format!("Network error: {}", e)));
                    }
                }
                precondition_loading.set(false);
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
        let auto_refresh_paused = auto_refresh_paused.clone();
        let refresh_count = refresh_count.clone();

        Callback::from(move |_: MouseEvent| {
            // Reset auto-refresh counter on manual refresh (user is active)
            refresh_count.set(0);
            if *auto_refresh_paused {
                auto_refresh_paused.set(false);
            }

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
        let remote_start_countdown = remote_start_countdown.clone();

        Callback::from(move |_: MouseEvent| {
            let passkey_state = passkey_state.clone();
            let pending_passkey_options = pending_passkey_options.clone();
            let pending_command = pending_command.clone();
            let lock_loading = lock_loading.clone();
            let remote_start_loading = remote_start_loading.clone();
            let command_result = command_result.clone();
            let is_locked = is_locked.clone();
            let remote_start_countdown = remote_start_countdown.clone();

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
                                    execute_tesla_command_with_countdown("remote_start", remote_start_loading, command_result, None, Some(remote_start_countdown)).await;
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
        let remote_start_countdown = remote_start_countdown.clone();

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
            let remote_start_countdown = remote_start_countdown.clone();

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
                                execute_tesla_command_with_countdown("remote_start", remote_start_loading, command_result, None, Some(remote_start_countdown)).await;
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


    // Handler to resume auto-refresh (also triggers immediate refresh)
    let handle_resume_refresh = {
        let auto_refresh_paused = auto_refresh_paused.clone();
        let refresh_count = refresh_count.clone();
        let battery_level = battery_level.clone();
        let battery_range = battery_range.clone();
        let charging_state = charging_state.clone();
        let charge_limit_soc = charge_limit_soc.clone();
        let charge_rate = charge_rate.clone();
        let charger_power = charger_power.clone();
        let time_to_full_charge = time_to_full_charge.clone();
        let charge_energy_added = charge_energy_added.clone();
        let uses_miles = uses_miles.clone();
        let is_locked = is_locked.clone();
        let inside_temp = inside_temp.clone();
        let outside_temp = outside_temp.clone();
        let is_climate_on = is_climate_on.clone();
        let is_front_defroster_on = is_front_defroster_on.clone();
        let is_rear_defroster_on = is_rear_defroster_on.clone();
        let last_refresh_time = last_refresh_time.clone();
        let last_refresh_epoch = last_refresh_epoch.clone();
        let battery_loading = battery_loading.clone();

        Callback::from(move |_: MouseEvent| {
            refresh_count.set(0);
            auto_refresh_paused.set(false);

            // Trigger immediate refresh
            let battery_level = battery_level.clone();
            let battery_range = battery_range.clone();
            let charging_state = charging_state.clone();
            let charge_limit_soc = charge_limit_soc.clone();
            let charge_rate = charge_rate.clone();
            let charger_power = charger_power.clone();
            let time_to_full_charge = time_to_full_charge.clone();
            let charge_energy_added = charge_energy_added.clone();
            let uses_miles = uses_miles.clone();
            let is_locked = is_locked.clone();
            let inside_temp = inside_temp.clone();
            let outside_temp = outside_temp.clone();
            let is_climate_on = is_climate_on.clone();
            let is_front_defroster_on = is_front_defroster_on.clone();
            let is_rear_defroster_on = is_rear_defroster_on.clone();
            let last_refresh_time = last_refresh_time.clone();
            let last_refresh_epoch = last_refresh_epoch.clone();
            let battery_loading = battery_loading.clone();

            spawn_local(async move {
                battery_loading.set(true);
                if let Ok(response) = Api::get("/api/tesla/battery-status").send().await {
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
                            if let Some(limit) = data["charge_limit_soc"].as_i64() {
                                charge_limit_soc.set(Some(limit as i32));
                            }
                            charge_rate.set(data["charge_rate"].as_f64());
                            charger_power.set(data["charger_power"].as_i64().map(|p| p as i32));
                            time_to_full_charge.set(data["time_to_full_charge"].as_f64());
                            charge_energy_added.set(data["charge_energy_added"].as_f64());
                            uses_miles.set(data["uses_miles"].as_bool().unwrap_or(true));
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
                battery_loading.set(false);
            });
        })
    };

    // Connected - show controls
    html! {
        <div class="tesla-controls-container">
            // Auto-refresh paused overlay
            {
                if *auto_refresh_paused {
                    html! {
                        <div class="refresh-paused-overlay" onclick={handle_resume_refresh}>
                            <div class="refresh-paused-content">
                                <i class="fas fa-pause-circle"></i>
                                <span>{"Auto-refresh paused"}</span>
                                <span class="refresh-paused-hint">{"Tap to resume"}</span>
                            </div>
                        </div>
                    }
                } else {
                    html! {}
                }
            }

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
                        let is_charging = (*charging_state).as_ref().map(|s| s == "Charging").unwrap_or(false);
                        html! {
                            <div class="status-row">
                                <i class={format!("fa-solid {}", icon_class)} style="font-size: 24px; color: #7EB2FF;"></i>
                                <div class="status-info">
                                    <span class="status-main">{format!("{}%", level)}</span>
                                    {
                                        if let Some(range) = *battery_range {
                                            let unit = if *uses_miles { "mi" } else { "km" };
                                            html! { <span class="status-sub">{format!("{:.0} {}", range, unit)}</span> }
                                        } else { html! {} }
                                    }
                                </div>
                                {
                                    // Only show "Charging" status, hide other states
                                    if is_charging {
                                        html! { <span class="charging-state">{"Charging"}</span> }
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

                // Charging details row (only when charging)
                {
                    if (*charging_state).as_ref().map(|s| s == "Charging").unwrap_or(false) {
                        let power_str = (*charger_power).map(|p| format!("{} kW", p)).unwrap_or_else(|| "-- kW".to_string());
                        let energy_str = (*charge_energy_added).map(|e| format!("{:.1} kWh", e));
                        let time_str = (*time_to_full_charge).map(|h| {
                            let hours = h.floor() as i32;
                            let mins = ((h - hours as f64) * 60.0).round() as i32;
                            if hours > 0 {
                                format!("{}h {}m", hours, mins)
                            } else {
                                format!("{}m", mins)
                            }
                        }).unwrap_or_else(|| "--".to_string());
                        let charging_info = if let Some(energy) = energy_str {
                            format!(" {} • {} • {} left", power_str, energy, time_str)
                        } else {
                            format!(" {} • {} left", power_str, time_str)
                        };

                        html! {
                            <div class="charging-details-row">
                                <span class="charging-info">
                                    <i class="fas fa-bolt"></i>
                                    {charging_info}
                                </span>
                                <button
                                    onclick={handle_charging_notify.clone()}
                                    disabled={*charging_notify_loading || !*has_vehicle_charging_cmds}
                                    class={format!("notify-btn-inline {}",
                                        if *charging_notify_active { "notify-btn-active" } else { "" }
                                    )}
                                    title={if !*has_vehicle_charging_cmds { "Requires Charging Commands permission" } else { "" }}
                                >
                                    {
                                        if *charging_notify_loading {
                                            html! { <i class="fas fa-spinner fa-spin"></i> }
                                        } else if *charging_notify_active {
                                            html! { <><i class="fas fa-bell-slash"></i>{" Cancel"}</> }
                                        } else {
                                            html! { <><i class="fas fa-bell"></i>{" Notify"}</> }
                                        }
                                    }
                                </button>
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }

                // Charging notify hint (only when notify is active)
                {
                    if *charging_notify_active {
                        html! {
                            <div class="charging-notify-hint">
                                {"Uses your default notification setting"}
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }

                // Temperature info
                {
                    if inside_temp.is_some() || outside_temp.is_some() {
                        html! {
                            <div class="temp-row">
                                {
                                    if let Some(temp) = *inside_temp {
                                        html! { <span class="temp-item"><i class="fas fa-car" title="Inside"></i>{format!(" {:.1}°C", temp)}</span> }
                                    } else { html! {} }
                                }
                                {
                                    if let Some(temp) = *outside_temp {
                                        html! { <span class="temp-item"><i class="fas fa-cloud" title="Outside"></i>{format!(" {:.1}°C", temp)}</span> }
                                    } else { html! {} }
                                }
                            </div>
                        }
                    } else { html! {} }
                }

                // Charge limit row
                {
                    if let Some(limit) = *charge_limit_soc {
                        html! {
                            <div class="charge-limit-row">
                                <span class="charge-limit-label">
                                    {format!("Limit: {}%", limit)}
                                </span>
                                {
                                    if *charge_limit_editing {
                                        html! {
                                            <div class="charge-limit-edit">
                                                <input
                                                    type="range"
                                                    min="50"
                                                    max="100"
                                                    value={(*charge_limit_input).to_string()}
                                                    class="charge-limit-slider"
                                                    oninput={{
                                                        let charge_limit_input = charge_limit_input.clone();
                                                        Callback::from(move |e: InputEvent| {
                                                            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                                                            if let Ok(val) = input.value().parse::<i32>() {
                                                                charge_limit_input.set(val);
                                                            }
                                                        })
                                                    }}
                                                />
                                                <span class="charge-limit-value">{format!("{}%", *charge_limit_input)}</span>
                                                <button
                                                    class="charge-limit-save-btn"
                                                    disabled={*charge_limit_loading}
                                                    onclick={{
                                                        let charge_limit_input = charge_limit_input.clone();
                                                        let charge_limit_soc = charge_limit_soc.clone();
                                                        let charge_limit_editing = charge_limit_editing.clone();
                                                        let charge_limit_loading = charge_limit_loading.clone();
                                                        let command_result = command_result.clone();
                                                        Callback::from(move |_: MouseEvent| {
                                                            let new_limit = *charge_limit_input;
                                                            let charge_limit_soc = charge_limit_soc.clone();
                                                            let charge_limit_editing = charge_limit_editing.clone();
                                                            let charge_limit_loading = charge_limit_loading.clone();
                                                            let command_result = command_result.clone();
                                                            spawn_local(async move {
                                                                charge_limit_loading.set(true);
                                                                let body = serde_json::json!({"percent": new_limit});
                                                                match Api::post("/api/tesla/charge-limit")
                                                                    .json(&body)
                                                                    .unwrap()
                                                                    .send()
                                                                    .await
                                                                {
                                                                    Ok(response) => {
                                                                        if response.ok() {
                                                                            charge_limit_soc.set(Some(new_limit));
                                                                            charge_limit_editing.set(false);
                                                                            command_result.set(Some(format!("Charge limit set to {}%", new_limit)));
                                                                        } else {
                                                                            command_result.set(Some("Failed to set charge limit".to_string()));
                                                                        }
                                                                    }
                                                                    Err(_) => {
                                                                        command_result.set(Some("Failed to set charge limit".to_string()));
                                                                    }
                                                                }
                                                                charge_limit_loading.set(false);
                                                            });
                                                        })
                                                    }}
                                                >
                                                    {
                                                        if *charge_limit_loading {
                                                            html! { <i class="fas fa-spinner fa-spin"></i> }
                                                        } else {
                                                            html! { <i class="fas fa-check"></i> }
                                                        }
                                                    }
                                                </button>
                                                <button
                                                    class="charge-limit-cancel-btn"
                                                    onclick={{
                                                        let charge_limit_editing = charge_limit_editing.clone();
                                                        Callback::from(move |_: MouseEvent| {
                                                            charge_limit_editing.set(false);
                                                        })
                                                    }}
                                                >
                                                    <i class="fas fa-times"></i>
                                                </button>
                                            </div>
                                        }
                                    } else {
                                        html! {
                                            <button
                                                class="charge-limit-edit-btn"
                                                disabled={!*has_vehicle_charging_cmds}
                                                title={if !*has_vehicle_charging_cmds { "Requires Charging Commands permission" } else { "" }}
                                                onclick={{
                                                    let charge_limit_editing = charge_limit_editing.clone();
                                                    let charge_limit_input = charge_limit_input.clone();
                                                    let limit = limit;
                                                    Callback::from(move |_: MouseEvent| {
                                                        charge_limit_input.set(limit);
                                                        charge_limit_editing.set(true);
                                                    })
                                                }}
                                            >
                                                <i class="fas fa-edit"></i>
                                            </button>
                                        }
                                    }
                                }
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }
            </div>

            // Control buttons
            <div class="tesla-control-buttons">
                <div class="control-btn-wrapper" title={if !*has_vehicle_cmds { "Requires Vehicle Commands permission. Reconnect Tesla to grant." } else { "" }}>
                    <button
                        onclick={handle_lock}
                        disabled={*lock_loading || *battery_loading || !*has_vehicle_cmds}
                        class={format!("control-btn {} {}",
                            if is_locked.map(|l| !l).unwrap_or(false) { "control-btn-attention" } else { "" },
                            if !*has_vehicle_cmds { "control-btn-disabled" } else { "" }
                        )}
                    >
                        {
                            if *lock_loading {
                                html! { <i class="fas fa-spinner fa-spin"></i> }
                            } else if *battery_loading && is_locked.is_none() {
                                // Show lock icon with spinner while loading initial state
                                html! { <><i class="fas fa-lock"></i><i class="fas fa-spinner fa-spin btn-status-spinner"></i></> }
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
                </div>

                // Climate button - splits into two when climate is on
                {
                    if is_climate_on.unwrap_or(false) {
                        html! {
                            <div class="climate-split-buttons">
                                <div class="control-btn-wrapper" title={if !*has_vehicle_cmds { "Requires Vehicle Commands permission" } else { "" }}>
                                    <button
                                        onclick={handle_climate.clone()}
                                        disabled={*climate_loading || !*has_vehicle_cmds}
                                        class={format!("control-btn control-btn-active control-btn-half {}", if !*has_vehicle_cmds { "control-btn-disabled" } else { "" })}
                                    >
                                        {
                                            if *climate_loading {
                                                html! { <i class="fas fa-spinner fa-spin"></i> }
                                            } else {
                                                html! { <><i class="fas fa-fan"></i>{" Off"}</> }
                                            }
                                        }
                                    </button>
                                </div>
                                <button
                                    onclick={handle_climate_notify.clone()}
                                    disabled={*climate_notify_loading}
                                    class={format!("control-btn control-btn-half {}", if *climate_notify_active { "control-btn-active" } else { "" })}
                                >
                                    {
                                        if *climate_notify_loading {
                                            html! { <i class="fas fa-spinner fa-spin"></i> }
                                        } else if *climate_notify_active {
                                            html! { <><i class="fas fa-bell-slash"></i>{" Cancel"}</> }
                                        } else {
                                            html! { <><i class="fas fa-bell"></i>{" Notify"}</> }
                                        }
                                    }
                                </button>
                            </div>
                        }
                    } else {
                        html! {
                            <div class="control-btn-wrapper" title={if !*has_vehicle_cmds { "Requires Vehicle Commands permission. Reconnect Tesla to grant." } else { "" }}>
                                <button
                                    onclick={handle_climate.clone()}
                                    disabled={*climate_loading || *battery_loading || !*has_vehicle_cmds}
                                    class={format!("control-btn {}", if !*has_vehicle_cmds { "control-btn-disabled" } else { "" })}
                                >
                                    {
                                        if *climate_loading {
                                            html! { <i class="fas fa-spinner fa-spin"></i> }
                                        } else if *battery_loading && is_climate_on.is_none() {
                                            html! { <><i class="fas fa-fan"></i><i class="fas fa-spinner fa-spin btn-status-spinner"></i></> }
                                        } else {
                                            html! { <><i class="fas fa-fan"></i>{" Climate"}</> }
                                        }
                                    }
                                </button>
                            </div>
                        }
                    }
                }

                <div class="control-btn-wrapper" title={if !*has_vehicle_cmds { "Requires Vehicle Commands permission. Reconnect Tesla to grant." } else { "" }}>
                    <button
                        onclick={handle_defrost}
                        disabled={*defrost_loading || *battery_loading || !*has_vehicle_cmds}
                        class={format!("control-btn {} {}",
                            if is_front_defroster_on.unwrap_or(false) || is_rear_defroster_on.unwrap_or(false) { "control-btn-active" } else { "" },
                            if !*has_vehicle_cmds { "control-btn-disabled" } else { "" }
                        )}
                    >
                        {
                            if *defrost_loading {
                                html! { <i class="fas fa-spinner fa-spin"></i> }
                            } else if *battery_loading && is_front_defroster_on.is_none() {
                                html! { <><i class="fas fa-snowflake"></i><i class="fas fa-spinner fa-spin btn-status-spinner"></i></> }
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
                </div>

                <div class="control-btn-wrapper" title={if !*has_vehicle_cmds { "Requires Vehicle Commands permission. Reconnect Tesla to grant." } else { "" }}>
                    <button
                        onclick={handle_remote_start}
                        disabled={*remote_start_loading || !*has_vehicle_cmds || remote_start_countdown.is_some()}
                        class={format!("control-btn {} {}",
                            if !*has_vehicle_cmds { "control-btn-disabled" } else { "" },
                            if remote_start_countdown.is_some() { "control-btn-countdown" } else { "" }
                        )}
                    >
                        {
                            if *remote_start_loading {
                                html! { <i class="fas fa-spinner fa-spin"></i> }
                            } else if let Some(secs) = *remote_start_countdown {
                                let mins = secs / 60;
                                let remaining_secs = secs % 60;
                                html! { <><i class="fas fa-car"></i>{format!(" {}:{:02}", mins, remaining_secs)}</> }
                            } else {
                                html! { <><i class="fas fa-key"></i>{" Start"}</> }
                            }
                        }
                    </button>
                </div>
            </div>

            // Cabin Overheat Protection section
            <div class="cabin-overheat-section">
                <h4 class="section-title">{"Cabin Overheat Protection"}</h4>
                <div class="cabin-overheat-buttons">
                    <div class="control-btn-wrapper" title={if !*has_vehicle_cmds { "Requires Vehicle Commands permission" } else { "" }}>
                        <button
                            onclick={
                                let cb = handle_cabin_overheat.clone();
                                Callback::from(move |_: MouseEvent| cb.emit("cabin_overheat_on".to_string()))
                            }
                            disabled={*cabin_overheat_loading || !*has_vehicle_cmds}
                            class={format!("control-btn {}", if !*has_vehicle_cmds { "control-btn-disabled" } else { "" })}
                        >
                            {
                                if *cabin_overheat_loading {
                                    html! { <i class="fas fa-spinner fa-spin"></i> }
                                } else {
                                    html! { <><i class="fas fa-temperature-high"></i>{" On (A/C)"}</> }
                                }
                            }
                        </button>
                    </div>
                    <div class="control-btn-wrapper" title={if !*has_vehicle_cmds { "Requires Vehicle Commands permission" } else { "" }}>
                        <button
                            onclick={
                                let cb = handle_cabin_overheat.clone();
                                Callback::from(move |_: MouseEvent| cb.emit("cabin_overheat_fan_only".to_string()))
                            }
                            disabled={*cabin_overheat_loading || !*has_vehicle_cmds}
                            class={format!("control-btn {}", if !*has_vehicle_cmds { "control-btn-disabled" } else { "" })}
                        >
                            {
                                if *cabin_overheat_loading {
                                    html! { <i class="fas fa-spinner fa-spin"></i> }
                                } else {
                                    html! { <><i class="fas fa-fan"></i>{" Fan Only"}</> }
                                }
                            }
                        </button>
                    </div>
                    <div class="control-btn-wrapper" title={if !*has_vehicle_cmds { "Requires Vehicle Commands permission" } else { "" }}>
                        <button
                            onclick={
                                let cb = handle_cabin_overheat.clone();
                                Callback::from(move |_: MouseEvent| cb.emit("cabin_overheat_off".to_string()))
                            }
                            disabled={*cabin_overheat_loading || !*has_vehicle_cmds}
                            class={format!("control-btn {}", if !*has_vehicle_cmds { "control-btn-disabled" } else { "" })}
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
            </div>

            // Battery Preconditioning section - for fast charging preparation
            <div class="battery-precondition-section">
                <h4 class="section-title">{"Fast Charge Prep"}</h4>
                <div class="control-btn-wrapper" title={if !*has_vehicle_cmds { "Requires Vehicle Commands permission" } else { "Warms battery for faster charging by setting nav to distant Supercharger" }}>
                    <button
                        onclick={handle_precondition_battery}
                        disabled={*precondition_loading || !*has_vehicle_cmds}
                        class={format!("control-btn control-btn-full {}", if !*has_vehicle_cmds { "control-btn-disabled" } else { "" })}
                    >
                        {
                            if *precondition_loading {
                                html! { <><i class="fas fa-spinner fa-spin"></i>{" Preconditioning..."}</> }
                            } else {
                                html! { <><i class="fas fa-bolt"></i>{" Precondition Battery"}</> }
                            }
                        }
                    </button>
                </div>
                <div class="precondition-hint">
                    {"Warms battery for fast charging (use ~30 min before charging)"}
                </div>
            </div>

            // Hint for climate notify when visible
            {
                if is_climate_on.unwrap_or(false) && !charging_state.as_ref().map(|s| s == "Charging").unwrap_or(false) {
                    html! {
                        <div class="notify-hint">
                            {"Climate notification won't send if you're in the vehicle"}
                        </div>
                    }
                } else {
                    html! {}
                }
            }

            // Status message (SSE streaming updates)
            {
                if let Some(status) = (*status_message).as_ref() {
                    html! {
                        <div class="status-message">
                            <i class="fas fa-spinner fa-spin"></i>
                            {" "}{status}
                        </div>
                    }
                } else { html! {} }
            }

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
            position: relative;
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
            gap: 1.25rem;
            margin-top: 0.75rem;
            padding-top: 0.75rem;
            border-top: 1px solid rgba(30, 144, 255, 0.1);
        }
        .temp-item {
            color: #999;
            font-size: 0.9rem;
            display: flex;
            align-items: center;
            gap: 4px;
        }
        .temp-item i {
            color: #7EB2FF;
            font-size: 14px;
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
        .control-btn-attention {
            background: rgba(255, 107, 107, 0.15);
            color: #ff6b6b;
            border-color: rgba(255, 107, 107, 0.4);
            animation: pulse-attention 2s infinite;
        }
        .control-btn-attention:hover:not(:disabled) {
            background: rgba(255, 107, 107, 0.25);
        }
        @keyframes pulse-attention {
            0%, 100% { box-shadow: 0 0 0 0 rgba(255, 107, 107, 0.4); }
            50% { box-shadow: 0 0 0 8px rgba(255, 107, 107, 0); }
        }
        .control-btn-active {
            background: rgba(255, 152, 0, 0.15);
            color: #FFB74D;
            border-color: rgba(255, 152, 0, 0.4);
        }
        .control-btn-active:hover:not(:disabled) {
            background: rgba(255, 152, 0, 0.25);
        }

        /* Countdown button for remote start timer */
        .control-btn-countdown {
            background: rgba(105, 240, 174, 0.15);
            color: #69f0ae;
            border-color: rgba(105, 240, 174, 0.4);
            font-family: monospace;
            font-size: 16px;
        }

        /* Disabled button for scope-gated features */
        .control-btn-wrapper {
            position: relative;
            display: contents;
        }
        .control-btn-disabled {
            opacity: 0.35 !important;
            cursor: not-allowed !important;
            background: rgba(128, 128, 128, 0.1) !important;
            color: #666 !important;
            border-color: rgba(128, 128, 128, 0.2) !important;
        }
        .control-btn-wrapper[title]:not([title=""]):hover::after {
            content: attr(title);
            position: absolute;
            bottom: calc(100% + 8px);
            left: 50%;
            transform: translateX(-50%);
            background: #333;
            color: #fff;
            padding: 8px 12px;
            border-radius: 6px;
            font-size: 12px;
            white-space: nowrap;
            z-index: 100;
            box-shadow: 0 2px 8px rgba(0,0,0,0.3);
            max-width: 250px;
            text-align: center;
        }

        .climate-split-buttons {
            display: flex;
            gap: 4px;
        }
        .control-btn-half {
            flex: 1;
            min-width: 0;
            padding: 14px 12px;
        }
        .control-btn-full {
            width: 100%;
        }
        /* Small spinner shown next to icon while loading state */
        .btn-status-spinner {
            margin-left: 6px;
            font-size: 12px;
            opacity: 0.7;
        }
        .charging-details-row {
            display: flex;
            align-items: center;
            justify-content: space-between;
            margin-top: 8px;
            padding-top: 8px;
            border-top: 1px solid rgba(30, 144, 255, 0.1);
        }
        .charging-info {
            color: #7EB2FF;
            font-size: 14px;
        }
        .notify-btn-inline {
            padding: 6px 12px;
            font-size: 12px;
            background: rgba(30, 144, 255, 0.15);
            color: #7EB2FF;
            border: 1px solid rgba(30, 144, 255, 0.3);
            border-radius: 6px;
            cursor: pointer;
            transition: all 0.2s ease;
        }
        .notify-btn-inline:hover:not(:disabled) {
            background: rgba(30, 144, 255, 0.25);
        }
        .notify-btn-inline:disabled {
            opacity: 0.5;
            cursor: not-allowed;
        }
        .notify-btn-inline.notify-btn-active {
            background: rgba(30, 144, 255, 0.3);
            border-color: rgba(30, 144, 255, 0.5);
        }
        .charging-notify-hint {
            font-size: 11px;
            color: #666;
            text-align: center;
            margin-top: 4px;
            padding: 4px 0;
        }
        .charge-limit-row {
            display: flex;
            align-items: center;
            justify-content: space-between;
            margin-top: 8px;
            padding-top: 8px;
            border-top: 1px solid rgba(30, 144, 255, 0.1);
        }
        .charge-limit-label {
            color: #999;
            font-size: 14px;
        }
        .charge-limit-edit {
            display: flex;
            align-items: center;
            gap: 8px;
        }
        .charge-limit-slider {
            width: 80px;
            -webkit-appearance: none;
            appearance: none;
            height: 6px;
            background: rgba(30, 144, 255, 0.3);
            border-radius: 3px;
            outline: none;
        }
        .charge-limit-slider::-webkit-slider-thumb {
            -webkit-appearance: none;
            appearance: none;
            width: 16px;
            height: 16px;
            background: #7EB2FF;
            border-radius: 50%;
            cursor: pointer;
        }
        .charge-limit-slider::-moz-range-thumb {
            width: 16px;
            height: 16px;
            background: #7EB2FF;
            border-radius: 50%;
            cursor: pointer;
            border: none;
        }
        .charge-limit-value {
            color: #7EB2FF;
            font-size: 14px;
            min-width: 35px;
        }
        .charge-limit-save-btn,
        .charge-limit-cancel-btn {
            padding: 4px 8px;
            font-size: 12px;
            background: rgba(30, 144, 255, 0.15);
            color: #7EB2FF;
            border: 1px solid rgba(30, 144, 255, 0.3);
            border-radius: 4px;
            cursor: pointer;
            transition: all 0.2s ease;
        }
        .charge-limit-save-btn:hover:not(:disabled) {
            background: rgba(105, 240, 174, 0.2);
            border-color: rgba(105, 240, 174, 0.4);
            color: #69f0ae;
        }
        .charge-limit-cancel-btn:hover {
            background: rgba(255, 100, 100, 0.2);
            border-color: rgba(255, 100, 100, 0.4);
            color: #ff6464;
        }
        .charge-limit-save-btn:disabled {
            opacity: 0.5;
            cursor: not-allowed;
        }
        .charge-limit-edit-btn {
            padding: 4px 8px;
            font-size: 12px;
            background: transparent;
            color: #666;
            border: none;
            cursor: pointer;
            transition: color 0.2s ease;
        }
        .charge-limit-edit-btn:hover:not(:disabled) {
            color: #7EB2FF;
        }
        .charge-limit-edit-btn:disabled {
            opacity: 0.3;
            cursor: not-allowed;
        }
        .status-message {
            margin-top: 1rem;
            padding: 10px;
            background: rgba(30, 144, 255, 0.1);
            color: #1e90ff;
            border-radius: 8px;
            font-size: 14px;
            border: 1px solid rgba(30, 144, 255, 0.2);
            text-align: center;
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

        /* Auto-refresh paused overlay */
        .refresh-paused-overlay {
            position: absolute;
            top: 0;
            left: 0;
            right: 0;
            bottom: 0;
            background: rgba(0, 0, 0, 0.85);
            display: flex;
            align-items: center;
            justify-content: center;
            z-index: 50;
            border-radius: 16px;
            cursor: pointer;
            transition: background 0.2s;
        }
        .refresh-paused-overlay:hover {
            background: rgba(0, 0, 0, 0.75);
        }
        .refresh-paused-content {
            display: flex;
            flex-direction: column;
            align-items: center;
            gap: 8px;
            color: #999;
        }
        .refresh-paused-content i {
            font-size: 32px;
            color: #7EB2FF;
        }
        .refresh-paused-content span {
            font-size: 14px;
        }
        .refresh-paused-hint {
            font-size: 12px !important;
            color: #666 !important;
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
        .cabin-overheat-buttons .control-btn {
            flex: 1;
            min-width: 90px;
        }
        .battery-precondition-section {
            margin-top: 1.5rem;
            padding-top: 1rem;
            border-top: 1px solid rgba(30, 144, 255, 0.1);
        }
        .battery-precondition-section .section-title {
            color: #7EB2FF;
            font-size: 14px;
            font-weight: 500;
            margin-bottom: 12px;
        }
        .precondition-hint {
            width: 100%;
            text-align: center;
            color: #666;
            font-size: 12px;
            margin-top: 8px;
            font-style: italic;
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
    execute_tesla_command_with_countdown(command, loading, command_result, is_locked, None).await;
}

// Helper function to execute Tesla command with optional countdown timer (for remote start)
async fn execute_tesla_command_with_countdown(
    command: &str,
    loading: UseStateHandle<bool>,
    command_result: UseStateHandle<Option<String>>,
    is_locked: Option<UseStateHandle<Option<bool>>>,
    countdown: Option<UseStateHandle<Option<u32>>>,
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
                // Start 2-minute countdown for remote_start
                if command == "remote_start" {
                    if let Some(countdown) = countdown {
                        countdown.set(Some(120)); // 2 minutes = 120 seconds
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
