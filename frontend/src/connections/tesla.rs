use yew::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use wasm_bindgen_futures::spawn_local;
use web_sys::window;
use crate::utils::api::Api;
use serde::Deserialize;

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

#[derive(Properties, PartialEq)]
pub struct TeslaConnectProps {
    pub user_id: i32,
    pub sub_tier: Option<String>,
}

#[function_component(TeslaConnect)]
pub fn tesla_connect(props: &TeslaConnectProps) -> Html {
    let error = use_state(|| None::<String>);
    let tesla_connected = use_state(|| false);
    let connecting = use_state(|| false);
    let pairing_link = use_state(|| None::<String>);
    let qr_code_url = use_state(|| None::<String>);
    let show_pairing = use_state(|| false);
    let command_result = use_state(|| None::<String>);

    // Vehicle selection state
    let available_vehicles = use_state(|| Vec::<VehicleInfo>::new());
    let selected_vehicle_name = use_state(|| None::<String>);
    let show_vehicle_selector = use_state(|| false);
    let vehicle_loading = use_state(|| false);

    // Per-vehicle pairing state
    let vehicle_pairing_vin = use_state(|| None::<String>); // VIN of vehicle whose pairing is shown
    let vehicle_pairing_link = use_state(|| None::<String>);
    let vehicle_qr_code_url = use_state(|| None::<String>);

    // Disconnect confirmation modal state
    let show_disconnect_modal = use_state(|| false);
    let is_disconnecting = use_state(|| false);

    // Scope picker modal state
    let show_scope_picker = use_state(|| false);
    let scope_vehicle_data = use_state(|| true);    // Default: checked
    let scope_vehicle_cmds = use_state(|| true);    // Default: checked
    let scope_charging_cmds = use_state(|| true);   // Default: checked

    // Check for OAuth callback error/success in URL on mount
    {
        let error = error.clone();
        let command_result = command_result.clone();
        use_effect_with_deps(
            move |_| {
                if let Some(window) = web_sys::window() {
                    if let Ok(search) = window.location().search() {
                        let params = web_sys::UrlSearchParams::new_with_str(&search).ok();
                        if let Some(params) = params {
                            // Check for error from OAuth callback
                            if let Some(tesla_status) = params.get("tesla") {
                                if tesla_status == "error" {
                                    if let Some(message) = params.get("message") {
                                        error.set(Some(message));
                                    } else {
                                        error.set(Some("Failed to connect Tesla. Please try again.".to_string()));
                                    }
                                } else if tesla_status == "success" {
                                    command_result.set(Some("Tesla connected successfully!".to_string()));
                                }
                                // Clear URL params after reading
                                let _ = window.history().and_then(|h| {
                                    h.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some("/connections"))
                                });
                            }
                        }
                    }
                }
                || ()
            },
            (),
        );
    }

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
                        10000, // 10 seconds
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
        let error = error.clone();
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
                        Err(e) => {
                            error.set(Some(format!("Failed to check Tesla status: {}", e)));
                        }
                    }
                });
                || ()
            },
            (),
        );
    }

    // Fetch virtual key pairing info when connected
    {
        let tesla_connected = tesla_connected.clone();
        let pairing_link = pairing_link.clone();
        let qr_code_url = qr_code_url.clone();
        let show_pairing = show_pairing.clone();
        let error = error.clone();

        use_effect_with_deps(
            move |connected| {
                if **connected {
                    spawn_local(async move {
                        match Api::get("/api/auth/tesla/virtual-key")
                            .send()
                            .await
                        {
                            Ok(response) => {
                                if response.ok() {
                                    if let Ok(data) = response.json::<serde_json::Value>().await {
                                        if let Some(link) = data["pairing_link"].as_str() {
                                            pairing_link.set(Some(link.to_string()));
                                        }
                                        if let Some(qr_url) = data["qr_code_url"].as_str() {
                                            qr_code_url.set(Some(qr_url.to_string()));
                                        }

                                        // Don't auto-show pairing instructions - user can click to see them
                                        show_pairing.set(false);
                                    }
                                }
                            }
                            Err(e) => {
                                error.set(Some(format!("Failed to fetch pairing info: {}", e)));
                            }
                        }
                    });
                }
                || ()
            },
            tesla_connected.clone(),
        );
    }

    // Fetch available vehicles when connected
    {
        let tesla_connected = tesla_connected.clone();
        let available_vehicles = available_vehicles.clone();
        let selected_vehicle_name = selected_vehicle_name.clone();

        use_effect_with_deps(
            move |connected| {
                if **connected {
                    spawn_local(async move {
                        match Api::get("/api/tesla/vehicles")
                            .send()
                            .await
                        {
                            Ok(response) => {
                                if response.ok() {
                                    if let Ok(data) = response.json::<serde_json::Value>().await {
                                        if let Some(vehicles_array) = data["vehicles"].as_array() {
                                            let vehicles: Vec<VehicleInfo> = vehicles_array
                                                .iter()
                                                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                                                .collect();

                                            // Find selected vehicle name
                                            let selected_name = vehicles.iter()
                                                .find(|v| v.selected)
                                                .map(|v| v.name.clone());

                                            available_vehicles.set(vehicles);
                                            selected_vehicle_name.set(selected_name);
                                        }
                                    }
                                }
                            }
                            Err(_) => {
                                // Silently fail - vehicles list is optional
                            }
                        }
                    });
                }
                || ()
            },
            tesla_connected.clone(),
        );
    }


    // Clear all state when disconnected (handles edge cases like external disconnects)
    {
        let tesla_connected = tesla_connected.clone();
        let pairing_link = pairing_link.clone();
        let qr_code_url = qr_code_url.clone();
        let show_pairing = show_pairing.clone();
        let available_vehicles = available_vehicles.clone();
        let selected_vehicle_name = selected_vehicle_name.clone();
        let show_vehicle_selector = show_vehicle_selector.clone();
        let vehicle_pairing_vin = vehicle_pairing_vin.clone();
        let vehicle_pairing_link = vehicle_pairing_link.clone();
        let vehicle_qr_code_url = vehicle_qr_code_url.clone();
        let command_result = command_result.clone();

        use_effect_with_deps(
            move |connected| {
                if !**connected {
                    // Clear all Tesla-related state when disconnected
                    pairing_link.set(None);
                    qr_code_url.set(None);
                    show_pairing.set(false);
                    available_vehicles.set(Vec::new());
                    selected_vehicle_name.set(None);
                    show_vehicle_selector.set(false);
                    vehicle_pairing_vin.set(None);
                    vehicle_pairing_link.set(None);
                    vehicle_qr_code_url.set(None);
                    command_result.set(None);
                }
                || ()
            },
            tesla_connected.clone(),
        );
    }

    // Handle connect button click - shows scope picker modal
    let onclick_connect = {
        let show_scope_picker = show_scope_picker.clone();
        Callback::from(move |_: MouseEvent| {
            show_scope_picker.set(true);
        })
    };

    // Handle proceeding to Tesla after scope selection
    let onclick_proceed_to_tesla = {
        let error = error.clone();
        let connecting = connecting.clone();
        let show_scope_picker = show_scope_picker.clone();
        let scope_vehicle_data = scope_vehicle_data.clone();
        let scope_vehicle_cmds = scope_vehicle_cmds.clone();
        let scope_charging_cmds = scope_charging_cmds.clone();
        Callback::from(move |_: MouseEvent| {
            let error = error.clone();
            let connecting = connecting.clone();
            let show_scope_picker = show_scope_picker.clone();

            // Build scopes string from selected checkboxes
            let mut scopes = Vec::new();
            if *scope_vehicle_data { scopes.push("vehicle_device_data"); }
            if *scope_vehicle_cmds { scopes.push("vehicle_cmds"); }
            if *scope_charging_cmds { scopes.push("vehicle_charging_cmds"); }

            // Require at least one scope
            if scopes.is_empty() {
                error.set(Some("Please select at least one permission".to_string()));
                return;
            }

            let scopes_param = scopes.join(",");
            show_scope_picker.set(false);
            connecting.set(true);

            spawn_local(async move {
                let url = format!("/api/auth/tesla/login?scopes={}", urlencoding::encode(&scopes_param));
                match Api::get(&url)
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.ok() {
                            if let Ok(data) = response.json::<serde_json::Value>().await {
                                if let Some(auth_url) = data["auth_url"].as_str() {
                                    if let Some(window) = window() {
                                        let _ = window.location().set_href(auth_url);
                                    }
                                }
                            }
                        } else {
                            if let Ok(error_data) = response.json::<serde_json::Value>().await {
                                if let Some(error_msg) = error_data["error"].as_str() {
                                    error.set(Some(error_msg.to_string()));
                                }
                            } else {
                                error.set(Some("Failed to initiate Tesla login".to_string()));
                            }
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Network error: {}", e)));
                    }
                }
                connecting.set(false);
            });
        })
    };

    // Handle cancel scope picker
    let onclick_cancel_scope_picker = {
        let show_scope_picker = show_scope_picker.clone();
        Callback::from(move |_: MouseEvent| {
            show_scope_picker.set(false);
        })
    };

    // Handle disconnect button click - shows confirmation modal
    let onclick_disconnect = {
        let show_disconnect_modal = show_disconnect_modal.clone();
        Callback::from(move |_: MouseEvent| {
            show_disconnect_modal.set(true);
        })
    };

    // Handle confirmed disconnect
    let handle_confirmed_disconnect = {
        let tesla_connected = tesla_connected.clone();
        let error = error.clone();
        let pairing_link = pairing_link.clone();
        let qr_code_url = qr_code_url.clone();
        let show_pairing = show_pairing.clone();
        let available_vehicles = available_vehicles.clone();
        let selected_vehicle_name = selected_vehicle_name.clone();
        let show_vehicle_selector = show_vehicle_selector.clone();
        let vehicle_pairing_vin = vehicle_pairing_vin.clone();
        let vehicle_pairing_link = vehicle_pairing_link.clone();
        let vehicle_qr_code_url = vehicle_qr_code_url.clone();
        let command_result = command_result.clone();
        let show_disconnect_modal = show_disconnect_modal.clone();
        let is_disconnecting = is_disconnecting.clone();

        Callback::from(move |_: MouseEvent| {
            let tesla_connected = tesla_connected.clone();
            let error = error.clone();
            let pairing_link = pairing_link.clone();
            let qr_code_url = qr_code_url.clone();
            let show_pairing = show_pairing.clone();
            let available_vehicles = available_vehicles.clone();
            let selected_vehicle_name = selected_vehicle_name.clone();
            let show_vehicle_selector = show_vehicle_selector.clone();
            let vehicle_pairing_vin = vehicle_pairing_vin.clone();
            let vehicle_pairing_link = vehicle_pairing_link.clone();
            let vehicle_qr_code_url = vehicle_qr_code_url.clone();
            let command_result = command_result.clone();
            let show_disconnect_modal = show_disconnect_modal.clone();
            let is_disconnecting = is_disconnecting.clone();

            is_disconnecting.set(true);

            spawn_local(async move {
                let request = Api::delete("/api/auth/tesla/connection")
                    .send()
                    .await;
                match request {
                    Ok(response) => {
                        if response.ok() {
                            // Clear all Tesla-related state
                            tesla_connected.set(false);
                            pairing_link.set(None);
                            qr_code_url.set(None);
                            show_pairing.set(false);
                            available_vehicles.set(Vec::new());
                            selected_vehicle_name.set(None);
                            show_vehicle_selector.set(false);
                            vehicle_pairing_vin.set(None);
                            vehicle_pairing_link.set(None);
                            vehicle_qr_code_url.set(None);
                            command_result.set(None);
                            show_disconnect_modal.set(false);
                            is_disconnecting.set(false);
                        } else {
                            if let Ok(error_data) = response.json::<serde_json::Value>().await {
                                if let Some(error_msg) = error_data.get("error").and_then(|e| e.as_str()) {
                                    error.set(Some(error_msg.to_string()));
                                } else {
                                    error.set(Some(format!("Failed to delete connection: {}", response.status())));
                                }
                            }
                            is_disconnecting.set(false);
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Network error: {}", e)));
                        is_disconnecting.set(false);
                    }
                }
            });
        })
    };

    // Handle vehicle selection
    let handle_vehicle_select = {
        let vehicle_loading = vehicle_loading.clone();
        let selected_vehicle_name = selected_vehicle_name.clone();
        let command_result = command_result.clone();
        let show_vehicle_selector = show_vehicle_selector.clone();
        let available_vehicles = available_vehicles.clone();

        Callback::from(move |vehicle: VehicleInfo| {
            let vehicle_loading = vehicle_loading.clone();
            let selected_vehicle_name = selected_vehicle_name.clone();
            let command_result = command_result.clone();
            let show_vehicle_selector = show_vehicle_selector.clone();
            let available_vehicles = available_vehicles.clone();
            let vehicle_clone = vehicle.clone();

            vehicle_loading.set(true);

            spawn_local(async move {
                let body = serde_json::json!({
                    "vin": vehicle_clone.vin,
                    "name": vehicle_clone.name,
                    "vehicle_id": vehicle_clone.vehicle_id,
                });

                let request = match Api::post("/api/tesla/select-vehicle")
                    .json(&body)
                {
                    Ok(req) => req.send().await,
                    Err(e) => {
                        command_result.set(Some(format!("Failed to select vehicle: {}", e)));
                        vehicle_loading.set(false);
                        return;
                    }
                };

                match request {
                    Ok(response) => {
                        if response.ok() {
                            // Update local state
                            selected_vehicle_name.set(Some(vehicle_clone.name.clone()));

                            // Update selected flag in vehicles list
                            let mut vehicles = (*available_vehicles).clone();
                            for v in vehicles.iter_mut() {
                                v.selected = v.vin == vehicle_clone.vin;
                            }
                            available_vehicles.set(vehicles);

                            // Close selector
                            show_vehicle_selector.set(false);

                            if let Ok(data) = response.json::<serde_json::Value>().await {
                                if let Some(msg) = data.get("message").and_then(|m| m.as_str()) {
                                    command_result.set(Some(msg.to_string()));
                                }
                            }
                        } else {
                            command_result.set(Some("Failed to select vehicle".to_string()));
                        }
                    }
                    Err(e) => {
                        command_result.set(Some(format!("Network error: {}", e)));
                    }
                }
                vehicle_loading.set(false);
            });
        })
    };

    // Handle showing vehicle-specific pairing QR code
    let handle_show_vehicle_pairing = {
        let vehicle_pairing_vin = vehicle_pairing_vin.clone();
        let vehicle_pairing_link = vehicle_pairing_link.clone();
        let vehicle_qr_code_url = vehicle_qr_code_url.clone();
        let command_result = command_result.clone();

        Callback::from(move |vin: String| {
            let vehicle_pairing_vin = vehicle_pairing_vin.clone();
            let vehicle_pairing_link = vehicle_pairing_link.clone();
            let vehicle_qr_code_url = vehicle_qr_code_url.clone();
            let command_result = command_result.clone();
            let vin_clone = vin.clone();

            spawn_local(async move {
                match Api::get(&format!("/api/auth/tesla/virtual-key?vin={}", urlencoding::encode(&vin_clone)))
                    .send()
                    .await
                {
                                Ok(response) => {
                                    if response.ok() {
                                        if let Ok(data) = response.json::<serde_json::Value>().await {
                                            if let Some(link) = data["pairing_link"].as_str() {
                                                vehicle_pairing_link.set(Some(link.to_string()));
                                            }
                                            if let Some(qr_url) = data["qr_code_url"].as_str() {
                                                vehicle_qr_code_url.set(Some(qr_url.to_string()));
                                            }
                                            vehicle_pairing_vin.set(Some(vin_clone));
                                        }
                                    } else {
                                        command_result.set(Some("Failed to fetch pairing info".to_string()));
                                    }
                                }
                    Err(e) => {
                        command_result.set(Some(format!("Failed to fetch pairing info: {}", e)));
                    }
                }
            })
        })
    };

    html! {
        <div class="service-item">
            <div class="service-header">
                <div class="service-name">
                    <img src="https://upload.wikimedia.org/wikipedia/commons/b/bb/Tesla_T_symbol.svg" alt="Tesla" width="24" height="24"/>
                    {"Tesla"}
                </div>
                <button class="info-button" onclick={Callback::from(|_| {
                    if let Some(element) = web_sys::window()
                        .and_then(|w| w.document())
                        .and_then(|d| d.get_element_by_id("tesla-info"))
                    {
                        let display = element.get_attribute("style")
                            .unwrap_or_else(|| "display: none".to_string());

                        if display.contains("none") {
                            let _ = element.set_attribute("style", "display: block");
                        } else {
                            let _ = element.set_attribute("style", "display: none");
                        }
                    }
                })}>
                    {"ⓘ"}
                </button>
                if *tesla_connected {
                    <span class="service-status">{"Connected ✓"}</span>
                }
            </div>
            <p class="service-description">
                {"Control your Tesla vehicle remotely through SMS or voice calls."}
            </p>
            <div id="tesla-info" class="info-section" style="display: none">
                <h4>{"How It Works"}</h4>
                <div class="info-subsection">
                    <h5>{"Available Commands"}</h5>
                    <ul>
                        <li>{"Lock/Unlock: Secure or open your vehicle remotely"}</li>
                        <li>{"Climate Control: Start or stop preconditioning"}</li>
                        <li>{"Remote Start: Enable keyless driving for 2 minutes"}</li>
                        <li>{"Charge Status: Check battery level and range"}</li>
                        <li>{"Cabin Overheat Protection: Keep cabin cool when parked (on/off/fan-only)"}</li>
                    </ul>
                </div>
                <div class="info-subsection">
                    <h5>{"Smart Notifications"}</h5>
                    <ul>
                        <li>{"Ask to be notified when climate is ready: \"Turn on climate and notify me when it's ready\""}</li>
                        <li>{"Ask to be notified when charging completes: \"Let me know when my car is done charging\""}</li>
                        <li>{"Notifications are automatically skipped if you're in the vehicle"}</li>
                    </ul>
                </div>
                <div class="info-subsection">
                    <h5>{"Example Commands"}</h5>
                    <ul>
                        <li>{"\"Lock my Tesla\""}</li>
                        <li>{"\"Start climate\" or \"Start climate and notify me when ready\""}</li>
                        <li>{"\"What's my Tesla's battery level?\""}</li>
                        <li>{"\"Notify me when charging is complete\""}</li>
                        <li>{"\"Turn on cabin overheat protection\" or \"Set cabin protection to fan only\""}</li>
                    </ul>
                </div>
                <p class="info-note">
                    {"Your Tesla credentials are encrypted and never stored in plain text. You can also control your vehicle and set up notifications from the Controls tab."}
                </p>
            </div>

            if let Some(error_msg) = (*error).as_ref() {
                <div class="error-message">
                    {error_msg}
                </div>
            }

            // Check subscription tier
            if props.sub_tier.is_some() {
                if !*tesla_connected {
                    <div class="tesla-connect-hint" style="
                        background: rgba(30, 144, 255, 0.08);
                        border: 1px solid rgba(30, 144, 255, 0.2);
                        border-radius: 10px;
                        padding: 1rem;
                        margin-bottom: 1rem;
                        font-size: 0.9rem;
                        color: #ccc;
                        line-height: 1.5;
                    ">
                        <p style="margin: 0 0 0.5rem 0;">
                            {"Once connected, you can control your Tesla via:"}
                        </p>
                        <ul style="margin: 0; padding-left: 1.25rem;">
                            <li>{"SMS and phone calls to your Lightfriend number"}</li>
                            <li>{"Type "}<code style="background: rgba(126, 178, 255, 0.2); padding: 0.15rem 0.4rem; border-radius: 4px; color: #7EB2FF;">{"@tesla"}</code>{" in the web chatbox for quick controls"}</li>
                        </ul>
                    </div>
                    <button
                        class="connect-button"
                        onclick={onclick_connect}
                        disabled={*connecting}
                    >
                        {if *connecting { "Connecting..." } else { "Connect Tesla" }}
                    </button>

                    // Scope picker modal
                    if *show_scope_picker {
                        <div class="modal-overlay">
                            <div class="scope-picker-modal">
                                <h3>{"Choose Tesla Permissions"}</h3>
                                <p class="scope-picker-subtitle">{"Select which features you want to enable. You can change these later by reconnecting."}</p>

                                <div class="scope-options">
                                    <label class="scope-checkbox">
                                        <input
                                            type="checkbox"
                                            checked={*scope_vehicle_data}
                                            onchange={{
                                                let scope_vehicle_data = scope_vehicle_data.clone();
                                                Callback::from(move |e: web_sys::Event| {
                                                    if let Some(target) = e.target() {
                                                        if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                                                            scope_vehicle_data.set(input.checked());
                                                        }
                                                    }
                                                })
                                            }}
                                        />
                                        <div class="scope-info">
                                            <strong>{"Vehicle Data"}</strong>
                                            <span>{"View battery level, range, temperature, and charging status"}</span>
                                        </div>
                                    </label>

                                    <label class="scope-checkbox">
                                        <input
                                            type="checkbox"
                                            checked={*scope_vehicle_cmds}
                                            onchange={{
                                                let scope_vehicle_cmds = scope_vehicle_cmds.clone();
                                                Callback::from(move |e: web_sys::Event| {
                                                    if let Some(target) = e.target() {
                                                        if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                                                            scope_vehicle_cmds.set(input.checked());
                                                        }
                                                    }
                                                })
                                            }}
                                        />
                                        <div class="scope-info">
                                            <strong>{"Vehicle Commands"}</strong>
                                            <span>{"Lock/unlock, climate control, defrost, remote start"}</span>
                                        </div>
                                    </label>

                                    <label class="scope-checkbox">
                                        <input
                                            type="checkbox"
                                            checked={*scope_charging_cmds}
                                            onchange={{
                                                let scope_charging_cmds = scope_charging_cmds.clone();
                                                Callback::from(move |e: web_sys::Event| {
                                                    if let Some(target) = e.target() {
                                                        if let Ok(input) = target.dyn_into::<web_sys::HtmlInputElement>() {
                                                            scope_charging_cmds.set(input.checked());
                                                        }
                                                    }
                                                })
                                            }}
                                        />
                                        <div class="scope-info">
                                            <strong>{"Charging Commands"}</strong>
                                            <span>{"Charging notifications, start/stop charging"}</span>
                                        </div>
                                    </label>
                                </div>

                                <div class="modal-buttons">
                                    <button onclick={onclick_cancel_scope_picker.clone()} class="cancel-button">
                                        {"Cancel"}
                                    </button>
                                    <button onclick={onclick_proceed_to_tesla.clone()} class="proceed-button">
                                        {"Continue to Tesla"}
                                    </button>
                                </div>
                            </div>
                        </div>
                    }
                } else {
                    <div class="connection-actions">
                        // Main Tesla section
                        <div style="
                            background: rgba(0, 0, 0, 0.2);
                            border: 1px solid rgba(30, 144, 255, 0.2);
                            border-radius: 12px;
                            padding: 1.5rem;
                            margin: 15px 0;
                        ">
                            // Simple vehicle selector row
                            <div style="
                                display: flex;
                                align-items: center;
                                gap: 12px;
                                margin-bottom: 15px;
                            ">
                                // Vehicle dropdown
                                {
                                    if available_vehicles.len() > 1 {
                                        let handle_vehicle_select = handle_vehicle_select.clone();
                                        let vehicles = (*available_vehicles).clone();
                                        html! {
                                            <select
                                                onchange={Callback::from(move |e: web_sys::Event| {
                                                    if let Some(target) = e.target() {
                                                        if let Ok(select) = target.dyn_into::<web_sys::HtmlSelectElement>() {
                                                            let vin = select.value();
                                                            if let Some(vehicle) = vehicles.iter().find(|v| v.vin == vin) {
                                                                handle_vehicle_select.emit(vehicle.clone());
                                                            }
                                                        }
                                                    }
                                                })}
                                                style="
                                                    flex: 1;
                                                    padding: 10px 12px;
                                                    background: rgba(30, 144, 255, 0.1);
                                                    color: #fff;
                                                    border: 1px solid rgba(30, 144, 255, 0.3);
                                                    border-radius: 8px;
                                                    font-size: 14px;
                                                    cursor: pointer;
                                                "
                                            >
                                                {
                                                    for (*available_vehicles).iter().map(|v| {
                                                        let is_selected = v.selected;
                                                        html! {
                                                            <option value={v.vin.clone()} selected={is_selected}>
                                                                {format!("{} {}", v.name.clone(), if v.paired { "✓" } else { "⚠️" })}
                                                            </option>
                                                        }
                                                    })
                                                }
                                            </select>
                                        }
                                    } else if !available_vehicles.is_empty() {
                                        let vehicle = available_vehicles.first().unwrap();
                                        html! {
                                            <span style="color: #fff; font-size: 14px; font-weight: 500;">
                                                {&vehicle.name}
                                                {if vehicle.paired {
                                                    html! { <span style="color: #69f0ae; margin-left: 8px;">{"✓ Paired"}</span> }
                                                } else {
                                                    html! { <span style="color: #ff9800; margin-left: 8px;">{"⚠️ Setup needed"}</span> }
                                                }}
                                            </span>
                                        }
                                    } else {
                                        html! { <span style="color: #999;">{"No vehicles found"}</span> }
                                    }
                                }

                                // Virtual Key button - always show, style based on paired status
                                {
                                    (*available_vehicles).iter()
                                        .find(|v| v.selected)
                                        .map(|vehicle| {
                                            let vin = vehicle.vin.clone();
                                            let is_paired = vehicle.paired;
                                            let handle_pairing = handle_show_vehicle_pairing.clone();
                                            if is_paired {
                                                html! {
                                                    <button
                                                        onclick={Callback::from(move |_| {
                                                            handle_pairing.emit(vin.clone());
                                                        })}
                                                        style="
                                                            padding: 6px 12px;
                                                            background: rgba(105, 240, 174, 0.15);
                                                            color: #69f0ae;
                                                            border: 1px solid rgba(105, 240, 174, 0.3);
                                                            border-radius: 6px;
                                                            font-size: 12px;
                                                            cursor: pointer;
                                                        "
                                                    >
                                                        {"🔑 Virtual Key ✓"}
                                                    </button>
                                                }
                                            } else {
                                                html! {
                                                    <button
                                                        onclick={Callback::from(move |_| {
                                                            handle_pairing.emit(vin.clone());
                                                        })}
                                                        style="
                                                            padding: 6px 12px;
                                                            background: rgba(255, 152, 0, 0.15);
                                                            color: #ff9800;
                                                            border: 1px solid rgba(255, 152, 0, 0.3);
                                                            border-radius: 6px;
                                                            font-size: 12px;
                                                            cursor: pointer;
                                                        "
                                                    >
                                                        {"⚠️ Setup Virtual Key"}
                                                    </button>
                                                }
                                            }
                                        })
                                        .unwrap_or(html! {})
                                }

                            </div>

                            // Vehicle pairing modal (shown when Setup Virtual Key is clicked)
                            {
                                if (*vehicle_pairing_vin).is_some() {
                                    html! {
                                        <div style="
                                            margin: 15px 0;
                                            padding: 20px;
                                            background: rgba(0, 0, 0, 0.4);
                                            border: 1px solid rgba(126, 178, 255, 0.3);
                                            border-radius: 8px;
                                        ">
                                            <div style="color: #7EB2FF; font-size: 14px; font-weight: 600; margin-bottom: 15px;">
                                                {"Virtual Key Setup"}
                                            </div>
                                            <div style="color: #ccc; font-size: 13px; line-height: 1.6; margin-bottom: 15px;">
                                                <ol style="margin: 10px 0; padding-left: 20px;">
                                                    <li>{"Open your Tesla mobile app"}</li>
                                                    <li>{"Scan the QR code below OR tap the button"}</li>
                                                    <li>{"Approve the pairing request"}</li>
                                                </ol>
                                            </div>
                                            {
                                                if let Some(qr_url) = (*vehicle_qr_code_url).as_ref() {
                                                    html! {
                                                        <div style="text-align: center; margin: 20px 0;">
                                                            <img
                                                                src={qr_url.clone()}
                                                                alt="Tesla Pairing QR Code"
                                                                style="max-width: 250px; width: 100%; height: auto; border-radius: 8px;"
                                                            />
                                                        </div>
                                                    }
                                                } else {
                                                    html! {}
                                                }
                                            }
                                            {
                                                if let Some(link) = (*vehicle_pairing_link).as_ref() {
                                                    html! {
                                                        <div style="text-align: center; margin-bottom: 15px;">
                                                            <a
                                                                href={link.clone()}
                                                                target="_blank"
                                                                style="
                                                                    display: inline-block;
                                                                    padding: 10px 20px;
                                                                    background: linear-gradient(135deg, #1e90ff 0%, #0066cc 100%);
                                                                    color: white;
                                                                    text-decoration: none;
                                                                    border-radius: 8px;
                                                                    font-weight: 600;
                                                                    font-size: 14px;
                                                                "
                                                            >
                                                                {"Open in Tesla App"}
                                                            </a>
                                                        </div>
                                                    }
                                                } else {
                                                    html! {}
                                                }
                                            }
                                            <div style="display: flex; gap: 10px; justify-content: center;">
                                                <button
                                                    onclick={{
                                                        let vehicle_pairing_vin = vehicle_pairing_vin.clone();
                                                        Callback::from(move |_| {
                                                            let vehicle_pairing_vin = vehicle_pairing_vin.clone();
                                                            spawn_local(async move {
                                                                match Api::post("/api/tesla/mark-paired")
                                                                    .json(&serde_json::json!({"paired": true}))
                                                                {
                                                                    Ok(req) => {
                                                                        match req.send().await {
                                                                            Ok(response) => {
                                                                                if response.ok() {
                                                                                    vehicle_pairing_vin.set(None);
                                                                                    if let Some(window) = web_sys::window() {
                                                                                        let _ = window.location().reload();
                                                                                    }
                                                                                }
                                                                            }
                                                                            Err(_) => { vehicle_pairing_vin.set(None); }
                                                                        }
                                                                    }
                                                                    Err(_) => { vehicle_pairing_vin.set(None); }
                                                                }
                                                            });
                                                        })
                                                    }}
                                                    style="
                                                        padding: 8px 16px;
                                                        background: rgba(105, 240, 174, 0.2);
                                                        color: #69f0ae;
                                                        border: 1px solid rgba(105, 240, 174, 0.3);
                                                        border-radius: 6px;
                                                        font-weight: 600;
                                                        cursor: pointer;
                                                    "
                                                >
                                                    {"✓ Done"}
                                                </button>
                                                <button
                                                    onclick={{
                                                        let vehicle_pairing_vin = vehicle_pairing_vin.clone();
                                                        Callback::from(move |_| {
                                                            vehicle_pairing_vin.set(None);
                                                        })
                                                    }}
                                                    style="
                                                        padding: 8px 16px;
                                                        background: rgba(0, 0, 0, 0.2);
                                                        color: #999;
                                                        border: 1px solid rgba(255, 255, 255, 0.1);
                                                        border-radius: 6px;
                                                        cursor: pointer;
                                                    "
                                                >
                                                    {"Cancel"}
                                                </button>
                                            </div>
                                        </div>
                                    }
                                } else {
                                    html! {}
                                }
                            }

                            // Command result feedback
                            if let Some(result) = (*command_result).as_ref() {
                                <div style="
                                    padding: 10px;
                                    background: rgba(105, 240, 174, 0.1);
                                    color: #69f0ae;
                                    border-radius: 8px;
                                    font-size: 14px;
                                    border: 1px solid rgba(105, 240, 174, 0.2);
                                ">
                                    {result}
                                </div>
                            }
                        </div>

                        <button
                            class="disconnect-button"
                            onclick={onclick_disconnect}
                        >
                            {"Disconnect"}
                        </button>

                        // Disconnect confirmation modal
                        if *show_disconnect_modal {
                            <div class="modal-overlay">
                                <div class="modal-content">
                                    <h3>{"Confirm Disconnection"}</h3>
                                    <p>{"Are you sure you want to disconnect Tesla? This will:"}</p>
                                    <ul>
                                        <li>{"Remove your Tesla OAuth tokens from our servers"}</li>
                                        <li>{"Delete your selected vehicle and pairing status"}</li>
                                        <li>{"Stop all Tesla vehicle control features"}</li>
                                        <li>{"Require reconnection to use Tesla features again"}</li>
                                    </ul>
                                    <p style="margin-top: 15px; color: #7EB2FF; font-size: 13px;">
                                        {"Note: To fully revoke access, you may also want to visit "}
                                        <a
                                            href="https://auth.tesla.com/user/revoke/consent"
                                            target="_blank"
                                            style="color: #69f0ae; text-decoration: underline;"
                                        >
                                            {"Tesla's consent management page"}
                                        </a>
                                        {" after disconnecting."}
                                    </p>
                                    if *is_disconnecting {
                                        <p class="disconnecting-message">{"Disconnecting Tesla... Please wait."}</p>
                                    }
                                    <div class="modal-buttons">
                                        <button onclick={
                                            let show_disconnect_modal = show_disconnect_modal.clone();
                                            Callback::from(move |_| show_disconnect_modal.set(false))
                                        } class="cancel-button" disabled={*is_disconnecting}>
                                            {"Cancel"}
                                        </button>
                                        <button onclick={handle_confirmed_disconnect.clone()}
                                            class="confirm-disconnect-button" disabled={*is_disconnecting}>
                                            if *is_disconnecting {
                                                <span class="button-spinner"></span> {"Disconnecting..."}
                                            } else {
                                                {"Yes, Disconnect"}
                                            }
                                        </button>
                                    </div>
                                </div>
                            </div>
                        }
                    </div>
                }
            } else {
                <div class="subscription-notice">
                    <p>{"Tesla integration requires a paid subscription."}</p>
                    <a href="/profile" class="upgrade-link">{"Upgrade Now"}</a>
                </div>
            }

            <style>
                {r#"
                    .modal-overlay {
                        position: fixed;
                        top: 0;
                        left: 0;
                        right: 0;
                        bottom: 0;
                        background: rgba(0, 0, 0, 0.85);
                        display: flex;
                        justify-content: center;
                        align-items: center;
                        z-index: 1000;
                    }
                    .modal-content {
                        background: #1a1a1a;
                        border: 1px solid rgba(30, 144, 255, 0.2);
                        border-radius: 12px;
                        padding: 2rem;
                        max-width: 500px;
                        width: 90%;
                        box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
                    }
                    .modal-content h3 {
                        color: #FF6347;
                        margin-bottom: 1rem;
                    }
                    .modal-content p {
                        color: #CCC;
                        margin-bottom: 1rem;
                    }
                    .modal-content ul {
                        margin-bottom: 2rem;
                        padding-left: 1.5rem;
                    }
                    .modal-content li {
                        color: #999;
                        margin-bottom: 0.5rem;
                    }
                    .modal-buttons {
                        display: flex;
                        gap: 1rem;
                        justify-content: flex-end;
                    }
                    .cancel-button {
                        background: transparent;
                        border: 1px solid rgba(204, 204, 204, 0.3);
                        color: #CCC;
                        padding: 0.8rem 1.5rem;
                        border-radius: 8px;
                        cursor: pointer;
                        transition: all 0.3s ease;
                    }
                    .cancel-button:hover {
                        background: rgba(204, 204, 204, 0.1);
                        transform: translateY(-2px);
                    }
                    .cancel-button:disabled {
                        opacity: 0.5;
                        cursor: not-allowed;
                    }
                    .confirm-disconnect-button {
                        background: linear-gradient(45deg, #FF6347, #FF4500);
                        color: white;
                        border: none;
                        padding: 0.8rem 1.5rem;
                        border-radius: 8px;
                        cursor: pointer;
                        transition: all 0.3s ease;
                        display: flex;
                        align-items: center;
                        gap: 0.5rem;
                    }
                    .confirm-disconnect-button:hover:not(:disabled) {
                        transform: translateY(-2px);
                        box-shadow: 0 4px 12px rgba(255, 99, 71, 0.3);
                    }
                    .confirm-disconnect-button:disabled {
                        opacity: 0.6;
                        cursor: not-allowed;
                    }
                    .button-spinner {
                        display: inline-block;
                        width: 14px;
                        height: 14px;
                        border: 2px solid rgba(255, 255, 255, 0.3);
                        border-radius: 50%;
                        border-top-color: white;
                        animation: spin 1s ease-in-out infinite;
                    }
                    .disconnecting-message {
                        color: #7EB2FF;
                        font-style: italic;
                        text-align: center;
                        margin: 1rem 0;
                    }
                    @keyframes spin {
                        to { transform: rotate(360deg); }
                    }

                    /* Scope picker modal styles */
                    .scope-picker-modal {
                        background: #1a1a1a;
                        border: 1px solid rgba(30, 144, 255, 0.3);
                        border-radius: 16px;
                        padding: 2rem;
                        max-width: 480px;
                        width: 90%;
                        box-shadow: 0 4px 30px rgba(0, 0, 0, 0.4);
                    }
                    .scope-picker-modal h3 {
                        color: #fff;
                        margin-bottom: 0.5rem;
                        font-size: 1.3rem;
                    }
                    .scope-picker-subtitle {
                        color: #999;
                        font-size: 0.9rem;
                        margin-bottom: 1.5rem;
                    }
                    .scope-options {
                        display: flex;
                        flex-direction: column;
                        gap: 12px;
                        margin-bottom: 1.5rem;
                    }
                    .scope-checkbox {
                        display: flex;
                        align-items: flex-start;
                        gap: 12px;
                        padding: 16px;
                        background: rgba(30, 144, 255, 0.05);
                        border: 1px solid rgba(30, 144, 255, 0.15);
                        border-radius: 10px;
                        cursor: pointer;
                        transition: all 0.2s ease;
                    }
                    .scope-checkbox:hover {
                        background: rgba(30, 144, 255, 0.1);
                        border-color: rgba(30, 144, 255, 0.3);
                    }
                    .scope-checkbox input[type="checkbox"] {
                        width: 20px;
                        height: 20px;
                        margin-top: 2px;
                        accent-color: #1E90FF;
                        cursor: pointer;
                    }
                    .scope-info {
                        display: flex;
                        flex-direction: column;
                        gap: 4px;
                    }
                    .scope-info strong {
                        color: #fff;
                        font-size: 0.95rem;
                    }
                    .scope-info span {
                        color: #888;
                        font-size: 0.85rem;
                        line-height: 1.4;
                    }
                    .proceed-button {
                        background: linear-gradient(45deg, #1E90FF, #4169E1);
                        color: white;
                        border: none;
                        padding: 0.8rem 1.5rem;
                        border-radius: 8px;
                        cursor: pointer;
                        font-size: 0.95rem;
                        transition: all 0.3s ease;
                    }
                    .proceed-button:hover {
                        transform: translateY(-2px);
                        box-shadow: 0 4px 15px rgba(30, 144, 255, 0.3);
                    }
                "#}
            </style>
        </div>
    }
}
