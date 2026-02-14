use yew::prelude::*;
use wasm_bindgen_futures::spawn_local;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;
use serde::Deserialize;
use crate::utils::api::Api;
use std::rc::Rc;
use std::cell::RefCell;

const TESLA_PANEL_STYLES: &str = r#"
.tesla-quick-panel {
    background: rgba(30, 30, 30, 0.8);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 12px;
    padding: 0.75rem;
    margin-top: 0.5rem;
}
.tesla-quick-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.5rem;
}
.tesla-quick-info {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    color: #fff;
    font-size: 0.85rem;
    flex-wrap: wrap;
}
.tesla-quick-info .vehicle-name {
    font-weight: 500;
}
.tesla-quick-info .battery-level {
    color: #69f0ae;
    font-weight: 600;
}
.tesla-quick-info .battery-level.low {
    color: #ff9800;
}
.tesla-quick-info .battery-level.critical {
    color: #ff5252;
}
.tesla-quick-info .battery-range {
    color: #888;
    font-size: 0.8rem;
}
.tesla-quick-info .charging-state {
    font-size: 0.75rem;
    padding: 0.15rem 0.4rem;
    border-radius: 4px;
    font-weight: 500;
}
.tesla-quick-info .charging-state.charging {
    background: rgba(105, 240, 174, 0.2);
    color: #69f0ae;
}
.tesla-quick-info .charging-state.complete {
    background: rgba(30, 144, 255, 0.2);
    color: #7eb2ff;
}
.tesla-quick-info .charging-state.stopped {
    background: rgba(255, 152, 0, 0.2);
    color: #ff9800;
}
.tesla-quick-close {
    background: transparent;
    border: none;
    color: #666;
    cursor: pointer;
    font-size: 1rem;
    padding: 0.25rem 0.5rem;
    border-radius: 4px;
    transition: all 0.2s;
}
.tesla-quick-close:hover {
    color: #999;
    background: rgba(255, 255, 255, 0.05);
}
.tesla-quick-controls {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
}
.tesla-quick-btn {
    background: rgba(30, 144, 255, 0.2);
    border: 1px solid rgba(30, 144, 255, 0.3);
    color: #7eb2ff;
    padding: 0.5rem 0.75rem;
    border-radius: 8px;
    cursor: pointer;
    font-size: 0.85rem;
    display: flex;
    align-items: center;
    gap: 0.35rem;
    transition: all 0.2s;
}
.tesla-quick-btn:hover:not(:disabled) {
    background: rgba(30, 144, 255, 0.3);
    border-color: #1e90ff;
}
.tesla-quick-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
}
.tesla-quick-btn.active {
    background: rgba(30, 144, 255, 0.4);
    border-color: #1e90ff;
    color: #fff;
}
.tesla-quick-btn.locked {
    background: rgba(105, 240, 174, 0.15);
    border-color: rgba(105, 240, 174, 0.3);
    color: #69f0ae;
}
.tesla-quick-btn.danger {
    background: rgba(255, 152, 0, 0.15);
    border-color: rgba(255, 152, 0, 0.3);
    color: #ff9800;
}
.tesla-quick-btn.danger.active {
    background: rgba(255, 152, 0, 0.3);
    border-color: #ff9800;
}
.tesla-quick-btn .btn-spinner {
    width: 12px;
    height: 12px;
    border: 2px solid rgba(126, 178, 255, 0.3);
    border-radius: 50%;
    border-top-color: #7eb2ff;
    animation: spin 1s linear infinite;
}
@keyframes spin {
    to { transform: rotate(360deg); }
}
.tesla-quick-loading {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    color: #888;
    font-size: 0.85rem;
    padding: 0.5rem 0;
}
.tesla-quick-loading .spinner {
    width: 16px;
    height: 16px;
    border: 2px solid rgba(126, 178, 255, 0.3);
    border-radius: 50%;
    border-top-color: #7eb2ff;
    animation: spin 1s linear infinite;
}
.tesla-quick-error {
    color: #ff6b6b;
    font-size: 0.85rem;
    padding: 0.5rem 0;
}
.tesla-not-connected {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.5rem 0;
}
.tesla-not-connected span {
    color: #888;
    font-size: 0.85rem;
}
.tesla-not-connected a {
    color: #7eb2ff;
    text-decoration: none;
    font-size: 0.85rem;
}
.tesla-not-connected a:hover {
    text-decoration: underline;
}
.tesla-preview-banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: rgba(30, 144, 255, 0.1);
    border: 1px solid rgba(30, 144, 255, 0.2);
    border-radius: 8px;
    padding: 0.5rem 0.75rem;
    margin-bottom: 0.75rem;
}
.tesla-preview-banner span {
    color: #888;
    font-size: 0.8rem;
}
.tesla-preview-banner a {
    color: #7eb2ff;
    text-decoration: none;
    font-size: 0.8rem;
    font-weight: 500;
}
.tesla-preview-banner a:hover {
    text-decoration: underline;
}
.tesla-quick-panel.preview .tesla-quick-info {
    opacity: 0.6;
}
.tesla-quick-panel.preview .tesla-quick-controls {
    opacity: 0.7;
}
.tesla-quick-panel.preview .tesla-quick-btn {
    cursor: default;
}
.tesla-quick-panel.preview .tesla-quick-row2 {
    opacity: 0.7;
}
.tesla-temps {
    display: flex;
    align-items: center;
    gap: 0.2rem;
}
.tesla-climate-temp {
    color: #888;
    font-size: 0.8rem;
}
.tesla-climate-temp.warm {
    color: #ff9800;
}
.tesla-climate-temp.cold {
    color: #64b5f6;
}
.temp-separator {
    color: #555;
    font-size: 0.75rem;
}
.tesla-quick-row2 {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
    margin-top: 0.5rem;
    padding-top: 0.5rem;
    border-top: 1px solid rgba(255, 255, 255, 0.05);
}
.tesla-charge-limit {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.8rem;
    color: #888;
}
.tesla-charge-limit input[type="range"] {
    width: 80px;
    height: 4px;
    accent-color: #7eb2ff;
}
.tesla-charge-limit .limit-value {
    color: #7eb2ff;
    font-weight: 500;
    min-width: 32px;
}
.tesla-charge-limit button {
    background: rgba(30, 144, 255, 0.2);
    border: 1px solid rgba(30, 144, 255, 0.3);
    color: #7eb2ff;
    padding: 0.25rem 0.5rem;
    border-radius: 4px;
    cursor: pointer;
    font-size: 0.75rem;
}
.tesla-charge-limit button:hover {
    background: rgba(30, 144, 255, 0.3);
}
.tesla-charge-limit button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
}
"#;

#[derive(Clone, PartialEq, Deserialize, Debug)]
pub struct TeslaBatteryStatus {
    pub battery_level: Option<i32>,
    pub battery_range: Option<f64>,
    pub charging_state: Option<String>,
    pub charge_limit_soc: Option<i32>,
    pub is_climate_on: Option<bool>,
    pub inside_temp: Option<f64>,
    pub outside_temp: Option<f64>,
    pub locked: Option<bool>,
    pub uses_miles: Option<bool>,
    pub cabin_overheat_protection: Option<bool>,
}

#[derive(Properties, Clone, PartialEq)]
pub struct TeslaQuickPanelProps {
    pub on_close: Callback<()>,
}

#[function_component(TeslaQuickPanel)]
pub fn tesla_quick_panel(props: &TeslaQuickPanelProps) -> Html {
    let connected = use_state(|| None::<bool>);
    let status = use_state(|| None::<TeslaBatteryStatus>);
    let loading = use_state(|| true);
    let error = use_state(|| None::<String>);

    // Command loading states
    let lock_loading = use_state(|| false);
    let climate_loading = use_state(|| false);
    let defrost_loading = use_state(|| false);
    let start_loading = use_state(|| false);
    let cop_loading = use_state(|| false); // Cabin overheat protection

    // Charge limit editing
    let charge_limit_input = use_state(|| 80i32);
    let charge_limit_editing = use_state(|| false);
    let charge_limit_loading = use_state(|| false);

    // Remote start countdown (seconds remaining)
    let remote_start_countdown = use_state(|| None::<u32>);

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
                                1000,
                            ) {
                                *interval_id.borrow_mut() = Some(id);
                            }
                            interval_callback.forget();
                        }
                    }
                }

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

    // Check connection status and fetch battery data on mount
    {
        let connected = connected.clone();
        let status = status.clone();
        let loading = loading.clone();
        let error = error.clone();
        let charge_limit_input = charge_limit_input.clone();

        use_effect_with_deps(
            move |_| {
                spawn_local(async move {
                    match Api::get("/api/auth/tesla/status").send().await {
                        Ok(response) => {
                            if response.ok() {
                                if let Ok(data) = response.json::<serde_json::Value>().await {
                                    let has_tesla = data["has_tesla"].as_bool().unwrap_or(false);
                                    connected.set(Some(has_tesla));

                                    if has_tesla {
                                        match Api::get("/api/tesla/battery-status").send().await {
                                            Ok(resp) => {
                                                if resp.ok() {
                                                    if let Ok(battery_data) = resp.json::<TeslaBatteryStatus>().await {
                                                        // Initialize charge limit input
                                                        if let Some(limit) = battery_data.charge_limit_soc {
                                                            charge_limit_input.set(limit);
                                                        }
                                                        status.set(Some(battery_data));
                                                    } else {
                                                        error.set(Some("Failed to parse status".to_string()));
                                                    }
                                                } else {
                                                    if let Ok(err_data) = resp.json::<serde_json::Value>().await {
                                                        let msg = err_data["error"].as_str().unwrap_or("Failed to fetch status");
                                                        error.set(Some(msg.to_string()));
                                                    } else {
                                                        error.set(Some("Failed to fetch status".to_string()));
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                error.set(Some(format!("Network error: {}", e)));
                                            }
                                        }
                                    }
                                }
                            } else {
                                connected.set(Some(false));
                            }
                        }
                        Err(e) => {
                            error.set(Some(format!("Network error: {}", e)));
                            connected.set(Some(false));
                        }
                    }
                    loading.set(false);
                });
                || ()
            },
            (),
        );
    }

    // Send command helper
    let send_command = {
        let status = status.clone();
        let error = error.clone();
        let charge_limit_input = charge_limit_input.clone();

        move |command: String, loading_state: UseStateHandle<bool>| {
            let status = status.clone();
            let error = error.clone();
            let charge_limit_input = charge_limit_input.clone();
            let loading_state = loading_state.clone();

            spawn_local(async move {
                loading_state.set(true);
                error.set(None);

                let body = serde_json::json!({ "command": command });
                match Api::post("/api/tesla/command").json(&body) {
                    Ok(req) => {
                        match req.send().await {
                            Ok(resp) => {
                                if resp.ok() {
                                    // Refresh status after command
                                    if let Ok(refresh_resp) = Api::get("/api/tesla/battery-status").send().await {
                                        if refresh_resp.ok() {
                                            if let Ok(new_status) = refresh_resp.json::<TeslaBatteryStatus>().await {
                                                if let Some(limit) = new_status.charge_limit_soc {
                                                    charge_limit_input.set(limit);
                                                }
                                                status.set(Some(new_status));
                                            }
                                        }
                                    }
                                } else {
                                    if let Ok(err_data) = resp.json::<serde_json::Value>().await {
                                        let msg = err_data["error"].as_str()
                                            .or(err_data["message"].as_str())
                                            .unwrap_or("Command failed");
                                        error.set(Some(msg.to_string()));
                                    }
                                }
                            }
                            Err(e) => {
                                error.set(Some(format!("Network error: {}", e)));
                            }
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Failed to create request: {}", e)));
                    }
                }
                loading_state.set(false);
            });
        }
    };

    // Handlers for buttons
    let on_lock_toggle = {
        let status = status.clone();
        let lock_loading = lock_loading.clone();
        let send_command = send_command.clone();

        Callback::from(move |_: MouseEvent| {
            let is_locked = status.as_ref().and_then(|s| s.locked).unwrap_or(true);
            let command = if is_locked { "unlock" } else { "lock" };
            send_command(command.to_string(), lock_loading.clone());
        })
    };

    let on_climate_toggle = {
        let status = status.clone();
        let climate_loading = climate_loading.clone();
        let send_command = send_command.clone();

        Callback::from(move |_: MouseEvent| {
            let is_on = status.as_ref().and_then(|s| s.is_climate_on).unwrap_or(false);
            let command = if is_on { "climate_off" } else { "climate_on" };
            send_command(command.to_string(), climate_loading.clone());
        })
    };

    let on_defrost = {
        let defrost_loading = defrost_loading.clone();
        let send_command = send_command.clone();

        Callback::from(move |_: MouseEvent| {
            send_command("defrost".to_string(), defrost_loading.clone());
        })
    };

    let on_cop_toggle = {
        let status = status.clone();
        let cop_loading = cop_loading.clone();
        let send_command = send_command.clone();

        Callback::from(move |_: MouseEvent| {
            let is_on = status.as_ref().and_then(|s| s.cabin_overheat_protection).unwrap_or(false);
            let command = if is_on { "cop_off" } else { "cop_on" };
            send_command(command.to_string(), cop_loading.clone());
        })
    };

    let on_remote_start = {
        let start_loading = start_loading.clone();
        let remote_start_countdown = remote_start_countdown.clone();
        let error = error.clone();

        Callback::from(move |_: MouseEvent| {
            let start_loading = start_loading.clone();
            let remote_start_countdown = remote_start_countdown.clone();
            let error = error.clone();

            spawn_local(async move {
                start_loading.set(true);
                error.set(None);

                let body = serde_json::json!({ "command": "remote_start" });
                match Api::post("/api/tesla/command").json(&body) {
                    Ok(req) => {
                        match req.send().await {
                            Ok(resp) => {
                                if resp.ok() {
                                    // Start 2-minute countdown
                                    remote_start_countdown.set(Some(120));
                                } else {
                                    if let Ok(err_data) = resp.json::<serde_json::Value>().await {
                                        let msg = err_data["error"].as_str()
                                            .or(err_data["message"].as_str())
                                            .unwrap_or("Remote start failed");
                                        error.set(Some(msg.to_string()));
                                    }
                                }
                            }
                            Err(e) => {
                                error.set(Some(format!("Network error: {}", e)));
                            }
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Failed to create request: {}", e)));
                    }
                }
                start_loading.set(false);
            });
        })
    };

    let on_charge_limit_save = {
        let charge_limit_input = charge_limit_input.clone();
        let charge_limit_loading = charge_limit_loading.clone();
        let charge_limit_editing = charge_limit_editing.clone();
        let status = status.clone();
        let error = error.clone();

        Callback::from(move |_: MouseEvent| {
            let new_limit = *charge_limit_input;
            let charge_limit_loading = charge_limit_loading.clone();
            let charge_limit_editing = charge_limit_editing.clone();
            let status = status.clone();
            let error = error.clone();

            spawn_local(async move {
                charge_limit_loading.set(true);

                let body = serde_json::json!({ "percent": new_limit });
                match Api::post("/api/tesla/set-charge-limit").json(&body) {
                    Ok(req) => {
                        match req.send().await {
                            Ok(resp) => {
                                if resp.ok() {
                                    // Update local status
                                    if let Some(mut s) = (*status).clone() {
                                        s.charge_limit_soc = Some(new_limit);
                                        status.set(Some(s));
                                    }
                                    charge_limit_editing.set(false);
                                } else {
                                    if let Ok(err_data) = resp.json::<serde_json::Value>().await {
                                        let msg = err_data["error"].as_str().unwrap_or("Failed to set charge limit");
                                        error.set(Some(msg.to_string()));
                                    }
                                }
                            }
                            Err(e) => {
                                error.set(Some(format!("Network error: {}", e)));
                            }
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Failed to create request: {}", e)));
                    }
                }
                charge_limit_loading.set(false);
            });
        })
    };

    let on_close = {
        let on_close = props.on_close.clone();
        Callback::from(move |_: MouseEvent| {
            on_close.emit(());
        })
    };

    // Determine if we're in preview mode (not connected)
    let is_preview = matches!(*connected, Some(false));

    html! {
        <>
            <style>{TESLA_PANEL_STYLES}</style>
            <div class={classes!("tesla-quick-panel", is_preview.then(|| "preview"))}>
                {
                    if *loading {
                        html! {
                            <div class="tesla-quick-loading">
                                <div class="spinner"></div>
                                <span>{"Loading Tesla..."}</span>
                            </div>
                        }
                    } else if let Some(err) = (*error).as_ref() {
                        html! {
                            <>
                                <div class="tesla-quick-header">
                                    <div class="tesla-quick-error">{err}</div>
                                    <button class="tesla-quick-close" onclick={on_close}>{"x"}</button>
                                </div>
                            </>
                        }
                    } else if is_preview || (*status).is_some() {
                        // Use real data if connected, mock data if preview
                        let s = (*status).as_ref();
                        let battery = s.and_then(|x| x.battery_level).unwrap_or(78);
                        let battery_class = if battery <= 20 { "critical" } else if battery <= 40 { "low" } else { "" };
                        let is_locked = s.and_then(|x| x.locked).unwrap_or(true);
                        let is_climate_on = s.and_then(|x| x.is_climate_on).unwrap_or(false);
                        let inside_temp = s.and_then(|x| x.inside_temp).or(if is_preview { Some(21.0) } else { None });
                        let outside_temp = s.and_then(|x| x.outside_temp).or(if is_preview { Some(8.0) } else { None });
                        let battery_range = s.and_then(|x| x.battery_range).or(if is_preview { Some(245.0) } else { None });
                        let uses_miles = s.and_then(|x| x.uses_miles).unwrap_or(true);
                        let charging_state = s.and_then(|x| x.charging_state.clone());
                        let charge_limit = s.and_then(|x| x.charge_limit_soc).or(if is_preview { Some(80) } else { None });
                        let is_cop_on = s.and_then(|x| x.cabin_overheat_protection).unwrap_or(if is_preview { true } else { false });
                        let countdown = *remote_start_countdown;

                        html! {
                            <>
                                // Preview banner when not connected
                                {
                                    if is_preview {
                                        html! {
                                            <div class="tesla-preview-banner">
                                                <span>{"Connect your Tesla to use these controls"}</span>
                                                <a href="/?settings=capabilities">{"Connect"}</a>
                                            </div>
                                        }
                                    } else {
                                        html! {}
                                    }
                                }
                                <div class="tesla-quick-header">
                                    <div class="tesla-quick-info">
                                        <span class="vehicle-name">{"Tesla"}</span>
                                        <span class={classes!("battery-level", battery_class)}>
                                            {format!("{}%", battery)}
                                        </span>
                                        {
                                            if let Some(range) = battery_range {
                                                let unit = if uses_miles { "mi" } else { "km" };
                                                html! {
                                                    <span class="battery-range">
                                                        {format!("{:.0} {}", range, unit)}
                                                    </span>
                                                }
                                            } else {
                                                html! {}
                                            }
                                        }
                                        {
                                            if let Some(ref state) = charging_state {
                                                let (class, label) = match state.to_lowercase().as_str() {
                                                    "charging" => ("charging", "Charging"),
                                                    "complete" => ("complete", "Complete"),
                                                    "stopped" => ("stopped", "Stopped"),
                                                    _ => ("", ""),
                                                };
                                                if !label.is_empty() {
                                                    html! {
                                                        <span class={classes!("charging-state", class)}>
                                                            {label}
                                                        </span>
                                                    }
                                                } else {
                                                    html! {}
                                                }
                                            } else {
                                                html! {}
                                            }
                                        }
                                        {
                                            // Show temps: inside / outside
                                            match (inside_temp, outside_temp) {
                                                (Some(inside), Some(outside)) => {
                                                    let inside_class = if inside > 25.0 { "warm" } else if inside < 15.0 { "cold" } else { "" };
                                                    let outside_class = if outside > 25.0 { "warm" } else if outside < 10.0 { "cold" } else { "" };
                                                    html! {
                                                        <span class="tesla-temps">
                                                            <span class={classes!("tesla-climate-temp", inside_class)} title="Inside">
                                                                {format!("{:.0}C", inside)}
                                                            </span>
                                                            <span class="temp-separator">{"/"}</span>
                                                            <span class={classes!("tesla-climate-temp", outside_class)} title="Outside">
                                                                {format!("{:.0}C", outside)}
                                                            </span>
                                                        </span>
                                                    }
                                                }
                                                (Some(inside), None) => {
                                                    let temp_class = if inside > 25.0 { "warm" } else if inside < 15.0 { "cold" } else { "" };
                                                    html! {
                                                        <span class={classes!("tesla-climate-temp", temp_class)}>
                                                            {format!("{:.0}C", inside)}
                                                        </span>
                                                    }
                                                }
                                                _ => html! {}
                                            }
                                        }
                                    </div>
                                    <button class="tesla-quick-close" onclick={on_close}>{"x"}</button>
                                </div>
                                <div class="tesla-quick-controls">
                                    // Lock/Unlock button
                                    <button
                                        class={classes!("tesla-quick-btn", is_locked.then(|| "locked"))}
                                        onclick={on_lock_toggle.clone()}
                                        disabled={is_preview || *lock_loading}
                                    >
                                        {
                                            if *lock_loading {
                                                html! { <div class="btn-spinner"></div> }
                                            } else if is_locked {
                                                html! { <><i class="fa fa-lock"></i>{"Unlock"}</> }
                                            } else {
                                                html! { <><i class="fa fa-unlock"></i>{"Lock"}</> }
                                            }
                                        }
                                    </button>

                                    // Climate button
                                    <button
                                        class={classes!("tesla-quick-btn", is_climate_on.then(|| "active"))}
                                        onclick={on_climate_toggle.clone()}
                                        disabled={is_preview || *climate_loading}
                                    >
                                        {
                                            if *climate_loading {
                                                html! { <div class="btn-spinner"></div> }
                                            } else if is_climate_on {
                                                html! { <><i class="fa fa-snowflake"></i>{"Climate Off"}</> }
                                            } else {
                                                html! { <><i class="fa fa-snowflake"></i>{"Climate"}</> }
                                            }
                                        }
                                    </button>

                                    // Defrost button
                                    <button
                                        class="tesla-quick-btn"
                                        onclick={on_defrost.clone()}
                                        disabled={is_preview || *defrost_loading}
                                    >
                                        {
                                            if *defrost_loading {
                                                html! { <div class="btn-spinner"></div> }
                                            } else {
                                                html! { <><i class="fa fa-sun"></i>{"Defrost"}</> }
                                            }
                                        }
                                    </button>

                                    // Cabin Overheat Protection button
                                    <button
                                        class={classes!("tesla-quick-btn", is_cop_on.then(|| "active"))}
                                        onclick={on_cop_toggle.clone()}
                                        disabled={is_preview || *cop_loading}
                                        title="Cabin Overheat Protection"
                                    >
                                        {
                                            if *cop_loading {
                                                html! { <div class="btn-spinner"></div> }
                                            } else if is_cop_on {
                                                html! { <><i class="fa fa-thermometer-full"></i>{"COP Off"}</> }
                                            } else {
                                                html! { <><i class="fa fa-thermometer-full"></i>{"COP"}</> }
                                            }
                                        }
                                    </button>

                                    // Remote Start button
                                    <button
                                        class={classes!("tesla-quick-btn", "danger", countdown.is_some().then(|| "active"))}
                                        onclick={on_remote_start.clone()}
                                        disabled={is_preview || *start_loading || countdown.is_some()}
                                    >
                                        {
                                            if *start_loading {
                                                html! { <div class="btn-spinner"></div> }
                                            } else if let Some(secs) = countdown {
                                                let mins = secs / 60;
                                                let secs = secs % 60;
                                                html! { <>{format!("{}:{:02}", mins, secs)}</> }
                                            } else {
                                                html! { <><i class="fa fa-car"></i>{"Start"}</> }
                                            }
                                        }
                                    </button>
                                </div>

                                // Charge limit row (only show if we have charge limit data)
                                {
                                    if let Some(limit) = charge_limit {
                                        html! {
                                            <div class="tesla-quick-row2">
                                                <div class="tesla-charge-limit">
                                                    <span>{"Charge limit:"}</span>
                                                    {
                                                        if *charge_limit_editing {
                                                            let charge_limit_input_clone = charge_limit_input.clone();
                                                            html! {
                                                                <>
                                                                    <input
                                                                        type="range"
                                                                        min="50"
                                                                        max="100"
                                                                        value={(*charge_limit_input).to_string()}
                                                                        oninput={Callback::from(move |e: InputEvent| {
                                                                            if let Some(input) = e.target_dyn_into::<web_sys::HtmlInputElement>() {
                                                                                if let Ok(val) = input.value().parse::<i32>() {
                                                                                    charge_limit_input_clone.set(val);
                                                                                }
                                                                            }
                                                                        })}
                                                                    />
                                                                    <span class="limit-value">{format!("{}%", *charge_limit_input)}</span>
                                                                    <button
                                                                        onclick={on_charge_limit_save}
                                                                        disabled={*charge_limit_loading}
                                                                    >
                                                                        {if *charge_limit_loading { "..." } else { "Save" }}
                                                                    </button>
                                                                    <button onclick={{
                                                                        let charge_limit_editing = charge_limit_editing.clone();
                                                                        let charge_limit_input = charge_limit_input.clone();
                                                                        Callback::from(move |_: MouseEvent| {
                                                                            charge_limit_input.set(limit);
                                                                            charge_limit_editing.set(false);
                                                                        })
                                                                    }}>
                                                                        {"Cancel"}
                                                                    </button>
                                                                </>
                                                            }
                                                        } else {
                                                            let charge_limit_editing = charge_limit_editing.clone();
                                                            html! {
                                                                <>
                                                                    <span class="limit-value">{format!("{}%", limit)}</span>
                                                                    <button
                                                                        disabled={is_preview}
                                                                        onclick={Callback::from(move |_: MouseEvent| {
                                                                            charge_limit_editing.set(true);
                                                                        })}
                                                                    >
                                                                        {"Edit"}
                                                                    </button>
                                                                </>
                                                            }
                                                        }
                                                    }
                                                </div>
                                            </div>
                                        }
                                    } else {
                                        html! {}
                                    }
                                }
                            </>
                        }
                    } else {
                        html! {
                            <div class="tesla-quick-header">
                                <div class="tesla-quick-loading">
                                    <span>{"Unable to load Tesla status"}</span>
                                </div>
                                <button class="tesla-quick-close" onclick={on_close}>{"x"}</button>
                            </div>
                        }
                    }
                }
            </div>
        </>
    }
}
