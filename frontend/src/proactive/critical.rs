use yew::prelude::*;
use crate::utils::api::Api;
use crate::proactive::constant_monitoring::MonitoredContactsSection;
use crate::proactive::waiting_checks::WaitingChecksSection;
use log::info;
use wasm_bindgen_futures::spawn_local;
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq)]
pub enum FieldSaveState {
    Idle,
    Saving,
    Success,
    Error(String),
}

fn render_save_indicator(state: &FieldSaveState) -> Html {
    match state {
        FieldSaveState::Idle => html! {},
        FieldSaveState::Saving => html! {
            <span class="save-indicator">
                <span class="save-spinner"></span>
            </span>
        },
        FieldSaveState::Success => html! {
            <span class="save-indicator save-success">{"✓"}</span>
        },
        FieldSaveState::Error(msg) => html! {
            <span class="save-indicator save-error" title={msg.clone()}>{"✗"}</span>
        },
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CriticalResponse {
    pub enabled: Option<String>,
    pub average_critical_per_day: f32,
    pub estimated_monthly_price: f32,
    pub call_notify: bool,
    pub action_on_critical_message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpdateCriticalRequest {
    enabled: Option<Option<String>>,
    call_notify: Option<bool>,
    action_on_critical_message: Option<Option<String>>,
}

#[derive(Properties, PartialEq)]
pub struct CriticalSectionProps {
    pub phone_number: String,
    #[prop_or(false)]
    pub proactive_disabled: bool,
}

#[function_component(CriticalSection)]
pub fn critical_section(props: &CriticalSectionProps) -> Html {
    // CriticalSection owns the critical_enabled state (single source of truth)
    let critical_enabled = use_state(|| None::<String>);
    let average_critical = use_state(|| 0.0);
    let estimated_price = use_state(|| 0.0);
    let call_notify = use_state(|| true);
    let mode = use_state(|| "all".to_string());
    // Per-field save states for visual feedback
    let enabled_save_state = use_state(|| FieldSaveState::Idle);
    let call_notify_save_state = use_state(|| FieldSaveState::Idle);
    let mode_save_state = use_state(|| FieldSaveState::Idle);
    // States for info toggles
    let show_message_info = use_state(|| false);
    let show_action_info = use_state(|| false);
    // Load critical notification settings when component mounts
    {
        let critical_enabled = critical_enabled.clone();
        let average_critical = average_critical.clone();
        let estimated_price = estimated_price.clone();
        let call_notify = call_notify.clone();
        let mode = mode.clone();
        use_effect_with_deps(
            move |_| {
                // Auth handled by cookies
                // Fetch critical settings
                spawn_local(async move {
                    if let Ok(resp) = Api::get("/api/profile/critical")
                    .send()
                    .await
                    {
                        if let Ok(critical) = resp.json::<CriticalResponse>().await {
                            info!("Received critical settings from backend: {:?}", critical);
                            critical_enabled.set(critical.enabled);
                            average_critical.set(critical.average_critical_per_day);
                            estimated_price.set(critical.estimated_monthly_price);
                            call_notify.set(critical.call_notify);
                            match critical.action_on_critical_message {
                                Some(ref val) if val == "notify_family" => {
                                    mode.set("family".to_string());
                                },
                                _ => {
                                    mode.set("all".to_string());
                                },
                            }
                        }
                    }
                });
                || ()
            },
            (),
        );
    }
    let handle_option_change = {
        let save_state = enabled_save_state.clone();
        let critical_enabled = critical_enabled.clone();
        Callback::from(move |new_value: Option<String>| {
            let save_state = save_state.clone();
            let critical_enabled = critical_enabled.clone();
            save_state.set(FieldSaveState::Saving);
            spawn_local(async move {
                let request = UpdateCriticalRequest {
                    enabled: Some(new_value.clone()),
                    call_notify: None,
                    action_on_critical_message: None,
                };
                info!("Sending update request: {:?}", request);
                match Api::post("/api/profile/critical")
                .json(&request)
                .unwrap()
                .send()
                .await
                {
                    Ok(response) if response.ok() => {
                        // Update internal state directly
                        critical_enabled.set(new_value);
                        info!("Successfully updated critical notification method");
                        save_state.set(FieldSaveState::Success);
                        let save_state_clone = save_state.clone();
                        spawn_local(async move {
                            gloo_timers::future::TimeoutFuture::new(2000).await;
                            save_state_clone.set(FieldSaveState::Idle);
                        });
                    },
                    Ok(response) => {
                        info!("Failed to update critical notification method: {}", response.status());
                        save_state.set(FieldSaveState::Error("Failed to save".to_string()));
                    },
                    Err(e) => {
                        info!("Error updating critical notification method: {:?}", e);
                        save_state.set(FieldSaveState::Error("Network error".to_string()));
                    }
                }
            });
        })
    };
    let handle_call_notify_change = {
        let call_notify = call_notify.clone();
        let save_state = call_notify_save_state.clone();
        Callback::from(move |new_value: bool| {
            let save_state = save_state.clone();
            let call_notify = call_notify.clone();
            save_state.set(FieldSaveState::Saving);
            spawn_local(async move {
                let request = UpdateCriticalRequest {
                    enabled: None,
                    call_notify: Some(new_value),
                    action_on_critical_message: None,
                };
                match Api::post("/api/profile/critical")
                .json(&request)
                .unwrap()
                .send()
                .await
                {
                    Ok(response) if response.ok() => {
                        call_notify.set(new_value);
                        info!("Successfully updated call notify setting");
                        save_state.set(FieldSaveState::Success);
                        let save_state_clone = save_state.clone();
                        spawn_local(async move {
                            gloo_timers::future::TimeoutFuture::new(2000).await;
                            save_state_clone.set(FieldSaveState::Idle);
                        });
                    },
                    Ok(response) => {
                        info!("Failed to update call notify setting: {}", response.status());
                        save_state.set(FieldSaveState::Error("Failed to save".to_string()));
                    },
                    Err(e) => {
                        info!("Error updating call notify setting: {:?}", e);
                        save_state.set(FieldSaveState::Error("Network error".to_string()));
                    }
                }
            });
        })
    };
    let handle_mode_change = {
        let mode = mode.clone();
        let save_state = mode_save_state.clone();
        Callback::from(move |new_mode: String| {
            let save_state = save_state.clone();
            let mode = mode.clone();
            save_state.set(FieldSaveState::Saving);
            let new_value: Option<String> = match new_mode.as_str() {
                "all" => None,
                "family" => Some("notify_family".to_string()),
                _ => None,
            };
            spawn_local(async move {
                let request = UpdateCriticalRequest {
                    enabled: None,
                    call_notify: None,
                    action_on_critical_message: Some(new_value),
                };
                match Api::post("/api/profile/critical")
                .json(&request)
                .unwrap()
                .send()
                .await
                {
                    Ok(response) if response.ok() => {
                        mode.set(new_mode);
                        info!("Successfully updated action on critical message");
                        save_state.set(FieldSaveState::Success);
                        let save_state_clone = save_state.clone();
                        spawn_local(async move {
                            gloo_timers::future::TimeoutFuture::new(2000).await;
                            save_state_clone.set(FieldSaveState::Idle);
                        });
                    },
                    Ok(response) => {
                        info!("Failed to update action on critical message: {}", response.status());
                        save_state.set(FieldSaveState::Error("Failed to save".to_string()));
                    },
                    Err(e) => {
                        info!("Error updating action on critical message: {:?}", e);
                        save_state.set(FieldSaveState::Error("Network error".to_string()));
                    }
                }
            });
        })
    };
    let phone_number = props.phone_number.clone();
    let country = if phone_number.starts_with("+1") {
        "US"
    } else if phone_number.starts_with("+358") {
        "FI"
    } else if phone_number.starts_with("+31") {
        "NL"
    } else if phone_number.starts_with("+44") {
        "UK"
    } else if phone_number.starts_with("+61") {
        "AU"
    } else {
        "Other"
    };
    let currency = match country {
        "US" => "", // No currency symbol for US (Messages will be used)
        "FI" => "€",
        "NL" => "€",
        "UK" => "£",
        "AU" => "$",
        _ => "$",
    };
    let sms_extra: Html = html! {};
    let call_extra: Html = html! {};
    let toggle_message_info = {
        let show_message_info = show_message_info.clone();
        Callback::from(move |_| show_message_info.set(!*show_message_info))
    };
    let toggle_action_info = {
        let show_action_info = show_action_info.clone();
        Callback::from(move |_| show_action_info.set(!*show_action_info))
    };
    html! {
        <>
            <style>
                {r#"
                    .filter-header {
                        display: flex;
                        flex-direction: column;
                        gap: 0.5rem;
                        margin-bottom: 1.5rem;
                    }
                    .filter-title {
                        display: flex;
                        align-items: center;
                        gap: 1rem;
                    }
                    .filter-title.critical h3 {
                        margin: 0;
                        color: white;
                        text-decoration: none;
                        font-weight: 600;
                        background: linear-gradient(45deg, #fff, #F59E0B);
                        -webkit-background-clip: text;
                        -webkit-text-fill-color: transparent;
                        transition: opacity 0.3s ease;
                        font-size: 1.2rem;
                    }
                    .info-button {
                        background: none;
                        border: none;
                        color: #F59E0B;
                        font-size: 1.2rem;
                        cursor: pointer;
                        padding: 0.5rem;
                        border-radius: 50%;
                        width: 32px;
                        height: 32px;
                        display: flex;
                        align-items: center;
                        justify-content: center;
                        transition: all 0.3s ease;
                    }
                    .info-button:hover {
                        background: rgba(245, 158, 11, 0.1);
                        transform: scale(1.1);
                    }
                    .flow-description {
                        color: #999;
                        font-size: 0.9rem;
                    }
                    .info-section {
                        background: rgba(0, 0, 0, 0.2);
                        border: 1px solid rgba(245, 158, 11, 0.1);
                        border-radius: 12px;
                        padding: 1.5rem;
                        margin-top: 1rem;
                    }
                    .info-section h4 {
                        color: #F59E0B;
                        margin: 0 0 1rem 0;
                        font-size: 1rem;
                    }
                    .info-subsection {
                        color: #999;
                        font-size: 0.9rem;
                    }
                    .info-subsection ul {
                        margin: 0;
                        padding-left: 1.5rem;
                    }
                    .info-subsection li {
                        margin-bottom: 0.5rem;
                    }
                    .critical-option {
                        display: flex;
                        flex-direction: column;
                        align-items: flex-start;
                        gap: 1rem;
                        padding: 1rem;
                        background: rgba(0, 0, 0, 0.2);
                        border: 1px solid rgba(245, 158, 11, 0.1);
                        border-radius: 12px;
                        margin-top: 1rem;
                    }
                    .critical-label {
                        color: #fff;
                        font-size: 0.9rem;
                    }
                    .estimated-price {
                        color: #F59E0B;
                        font-size: 0.9rem;
                        margin-top: 0.5rem;
                    }
                    @media (max-width: 480px) {
                        .filter-header {
                            margin-bottom: 1rem;
                        }
                        .filter-title h3 {
                            font-size: 1.1rem;
                        }
                        .flow-description {
                            font-size: 0.85rem;
                        }
                        .critical-option {
                            flex-direction: column;
                            align-items: flex-start;
                            gap: 0.75rem;
                            padding: 0.75rem;
                        }
                        .info-section {
                            padding: 1rem;
                        }
                        .info-section h4 {
                            font-size: 0.95rem;
                        }
                        .info-subsection {
                            font-size: 0.85rem;
                        }
                        .info-subsection ul {
                            padding-left: 1.2rem;
                        }
                        .estimated-price {
                            font-size: 0.85rem;
                        }
                    }
                    .radio-group {
                        display: flex;
                        flex-direction: column;
                        gap: 0.75rem;
                    }
                    .radio-option {
                        display: flex;
                        align-items: center;
                        gap: 0.75rem;
                        cursor: pointer;
                        padding: 0.5rem;
                        border-radius: 8px;
                        transition: background-color 0.2s ease;
                    }
                    .radio-option:hover {
                        background: rgba(245, 158, 11, 0.05);
                    }
                    .radio-option input[type="radio"] {
                        appearance: none;
                        width: 18px;
                        height: 18px;
                        border: 2px solid rgba(245, 158, 11, 0.3);
                        border-radius: 50%;
                        background: transparent;
                        cursor: pointer;
                        position: relative;
                        transition: all 0.2s ease;
                    }
                    .radio-option input[type="radio"]:checked {
                        border-color: #F59E0B;
                        background: rgba(245, 158, 11, 0.1);
                    }
                    .radio-option input[type="radio"]:checked::after {
                        content: '';
                        position: absolute;
                        top: 50%;
                        left: 50%;
                        transform: translate(-50%, -50%);
                        width: 8px;
                        height: 8px;
                        border-radius: 50%;
                        background: #F59E0B;
                    }
                    .radio-label {
                        color: #fff;
                        font-size: 0.9rem;
                        cursor: pointer;
                        flex: 1;
                    }
                    .radio-description {
                        color: #999;
                        font-size: 0.8rem;
                        margin-top: 0.25rem;
                    }
                    .info-details {
                        color: #999;
                        font-size: 0.85rem;
                        margin-top: 0.5rem;
                        padding: 0.75rem;
                        background: rgba(245, 158, 11, 0.05);
                        border-radius: 8px;
                        display: none;
                    }
                    .info-details.visible {
                        display: block;
                    }
                    .info-details ul {
                        margin: 0;
                        padding-left: 1.2rem;
                    }
                    .info-details li {
                        margin-bottom: 0.5rem;
                    }
                    .save-indicator {
                        min-width: 24px;
                        height: 24px;
                        display: inline-flex;
                        align-items: center;
                        justify-content: center;
                        margin-left: 8px;
                    }
                    .save-spinner {
                        width: 16px;
                        height: 16px;
                        border: 2px solid rgba(245, 158, 11, 0.3);
                        border-top-color: #F59E0B;
                        border-radius: 50%;
                        animation: spin 1s linear infinite;
                    }
                    @keyframes spin {
                        to { transform: rotate(360deg); }
                    }
                    .save-success {
                        color: #22C55E;
                        font-size: 18px;
                    }
                    .save-error {
                        color: #EF4444;
                        cursor: help;
                        font-size: 18px;
                    }
                    .critical-label-row {
                        display: flex;
                        align-items: center;
                    }
                    .disabled-hint {
                        font-size: 0.75rem;
                        color: #666;
                        font-style: italic;
                        margin-left: 0.5rem;
                    }
                    .section-disabled {
                        opacity: 0.5;
                    }
                    .internal-flow-step {
                        position: relative;
                        margin-bottom: 2rem;
                    }
                    .internal-flow-step::after {
                        content: '↓';
                        position: absolute;
                        left: 50%;
                        bottom: -1.5rem;
                        transform: translateX(-50%);
                        font-size: 2rem;
                        color: #fff;
                        opacity: 0.5;
                    }
                "#}
            </style>
            // MonitoredContactsSection (grayed only when master toggle is off)
            <div class={classes!("internal-flow-step", if props.proactive_disabled { "section-disabled" } else { "" })}>
                <MonitoredContactsSection
                    service_type={"email".to_string()}
                    contacts={Vec::new()}
                    on_change={Callback::from(|_| ())}
                    phone_number={props.phone_number.clone()}
                    critical_disabled={props.proactive_disabled}
                />
            </div>
            // WaitingChecksSection (grayed only when master toggle is off)
            <div class={classes!("internal-flow-step", if props.proactive_disabled { "section-disabled" } else { "" })}>
                <WaitingChecksSection
                    service_type={"messaging".to_string()}
                    checks={Vec::new()}
                    on_change={Callback::from(|_| ())}
                    phone_number={props.phone_number.clone()}
                    critical_disabled={props.proactive_disabled}
                />
            </div>
            // Critical Notifications content (grayed only when master toggle is off)
            <div class={classes!(if props.proactive_disabled { "section-disabled" } else { "" })}>
            <div class="filter-header">
                <div class="filter-title critical">
                    <h3>{"Critical Notifications"}</h3>
                    {if props.proactive_disabled {
                        html! { <span class="disabled-hint">{"(not active)"}</span> }
                    } else {
                        html! {}
                    }}
                </div>
                <div class="flow-description">
                    {"Instant alerts for urgent items."}
                </div>
            </div>
            <div class="critical-option">
                <div class="critical-label-row">
                    <span class="critical-label">{"Notification Method"}</span>
                    {render_save_indicator(&*enabled_save_state)}
                </div>
                <div class="radio-group">
                    <label class="radio-option" onclick={
                        let handle_option_change = handle_option_change.clone();
                        Callback::from(move |_| handle_option_change.emit(None))
                    }>
                        <input
                            type="radio"
                            name="critical-notifications"
                            checked={critical_enabled.is_none()}
                        />
                        <div class="radio-label">
                            {"Disabled"}
                            <div class="radio-description">{"No alerts"}</div>
                        </div>
                    </label>
                    <label class="radio-option" onclick={
                        let handle_option_change = handle_option_change.clone();
                        Callback::from(move |_| handle_option_change.emit(Some("sms".to_string())))
                    }>
                        <input
                            type="radio"
                            name="critical-notifications"
                            checked={matches!(&*critical_enabled, Some(s) if s == "sms")}
                        />
                        <div class="radio-label">
                            {"SMS"}
                            <div class="radio-description">
                                {"Text alerts"}
                                {sms_extra}
                            </div>
                        </div>
                    </label>
                    <label class="radio-option" onclick={
                        let handle_option_change = handle_option_change.clone();
                        Callback::from(move |_| handle_option_change.emit(Some("call".to_string())))
                    }>
                        <input
                            type="radio"
                            name="critical-notifications"
                            checked={matches!(&*critical_enabled, Some(s) if s == "call")}
                        />
                        <div class="radio-label">
                            {"Phone Call"}
                            <div class="radio-description">
                                {"Voice alerts"}
                                {call_extra}
                            </div>
                        </div>
                    </label>
                </div>
            </div>
            <div class="critical-option">
                <div class="critical-label-row">
                    <span class="critical-label">{"What is Critical?"}</span>
                    {render_save_indicator(&*call_notify_save_state)}
                </div>
                <div class="info-subsection">
                    <ul>
                        <li>
                            <i class="fa-solid fa-gears"></i>{" Incoming Calls: "}
                            <div class="radio-group">
                                <label class="radio-option" onclick={
                                    let handle_call_notify_change = handle_call_notify_change.clone();
                                    Callback::from(move |_| handle_call_notify_change.emit(true))
                                }>
                                    <input
                                        type="radio"
                                        name="call-notifications"
                                        checked={*call_notify}
                                    />
                                    <div class="radio-label">
                                        {"Notify Now"}
                                        <div class="radio-description">{"Alert for calls always immediately"}</div>
                                    </div>
                                </label>
                                <label class="radio-option" onclick={
                                    let handle_call_notify_change = handle_call_notify_change.clone();
                                    Callback::from(move |_| handle_call_notify_change.emit(false))
                                }>
                                    <input
                                        type="radio"
                                        name="call-notifications"
                                        checked={!*call_notify}
                                    />
                                    <div class="radio-label">
                                        {"In Summary"}
                                        <div class="radio-description">{"Handle in next summary"}</div>
                                    </div>
                                </label>
                            </div>
                        </li>
                        <li>
                            <i class="fa-solid fa-hat-wizard"></i>{" Messages: AI checks for urgency (can't wait >2hrs). Group chats: only @mentions."}
                            <button class="info-button" onclick={toggle_message_info.clone()}>
                                {"ⓘ"}
                            </button>
                            {if *show_message_info {
                                html! {
                                    <div class="info-details visible">
                                        {"Examples of critical: Someone getting hurt, losing important stuff/money, computers breaking/hacked, missing rules/laws in ≤2hrs, explicit emergencies (“ASAP”, “emergency”, “right now”) or deadlines ≤2hrs."}
                                        <br />
                                        {"Non-critical (wait for summary): Normal updates, vague asks."}
                                    </div>
                                }
                            } else {
                                html! {}
                            }}
                        </li>
                    </ul>
                </div>
            </div>
            <div class="critical-option">
                <div style="display: flex; align-items: center; gap: 0.5rem;">
                    <span class="critical-label">{"Action on Critical Message"}</span>
                    {render_save_indicator(&*mode_save_state)}
                    <button class="info-button" onclick={toggle_action_info.clone()}>
                        {"ⓘ"}
                    </button>
                </div>
                {if *show_action_info {
                    html! {
                        <div class="info-details visible">
                            {"Critical: Can't wait 2hrs (e.g., emergencies, lunch invites)."}
                            <ul>
                                <li>{"Notify All: Alert for any critical message, regardless of sender."}</li>
                                <li>{"Special Contacts Only: Alert only if sender is in your special contacts (specified 2 sections up). Does not apply to email which will always follow the notify all setting."}</li>
                            </ul>
                        </div>
                    }
                } else {
                    html! {}
                }}
                <div class="radio-group">
                    <label class="radio-option" onclick={
                        let handle_mode_change = handle_mode_change.clone();
                        Callback::from(move |_| handle_mode_change.emit("all".to_string()))
                    }>
                        <input
                            type="radio"
                            name="message-critical-mode"
                            checked={*mode == "all"}
                        />
                        <div class="radio-label">
                            {"Notify All"}
                        </div>
                    </label>
                    <label class="radio-option" onclick={
                        let handle_mode_change = handle_mode_change.clone();
                        Callback::from(move |_| handle_mode_change.emit("family".to_string()))
                    }>
                        <input
                            type="radio"
                            name="message-critical-mode"
                            checked={*mode == "family"}
                        />
                        <div class="radio-label">
                            {"Special Contacts Only"}
                        </div>
                    </label>
                </div>
            </div>
            </div>
        </>
    }
}
