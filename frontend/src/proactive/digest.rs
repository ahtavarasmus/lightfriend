use yew::prelude::*;
use log::info;
use wasm_bindgen_futures::spawn_local;
use serde::{Deserialize, Serialize};
use crate::utils::api::Api;

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
pub struct DigestsResponse {
    morning_digest_time: Option<String>, // RFC3339 time string or None
    day_digest_time: Option<String>, // RFC3339 time string or None
    evening_digest_time: Option<String>, // RFC3339 time string or None
}
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpdateDigestsRequest {
    morning_digest_time: Option<String>, // RFC3339 time string or None
    day_digest_time: Option<String>, // RFC3339 time string or None
    evening_digest_time: Option<String>, // RFC3339 time string or None
}
#[derive(Properties, PartialEq)]
pub struct DigestSectionProps {
    pub phone_number: String,
    #[prop_or(false)]
    pub disabled: bool,
}
#[function_component(DigestSection)]
pub fn digest_section(props: &DigestSectionProps) -> Html {
    let morning_digest_time = use_state(|| None::<String>);
    let day_digest_time = use_state(|| None::<String>);
    let evening_digest_time = use_state(|| None::<String>);
    let show_info = use_state(|| false);
    // Per-field save states for visual feedback
    let morning_save_state = use_state(|| FieldSaveState::Idle);
    let day_save_state = use_state(|| FieldSaveState::Idle);
    let evening_save_state = use_state(|| FieldSaveState::Idle);
    // Load digest settings when component mounts
    {
        let morning_digest_time = morning_digest_time.clone();
        let day_digest_time = day_digest_time.clone();
        let evening_digest_time = evening_digest_time.clone();
        use_effect_with_deps(
            move |_| {
                spawn_local(async move {
                    if let Ok(resp) = Api::get("/api/profile/digests")
                        .send()
                        .await
                    {
                        if let Ok(digests) = resp.json::<DigestsResponse>().await {
                            info!("Received digests from backend: {:?}", digests);
                            morning_digest_time.set(digests.morning_digest_time);
                            day_digest_time.set(digests.day_digest_time);
                            evening_digest_time.set(digests.evening_digest_time);
                        }
                    }
                });
                || ()
            },
            (),
        );
    }
    // Create save callback for morning digest
    let save_morning = {
        let morning_digest_time = morning_digest_time.clone();
        let day_digest_time = day_digest_time.clone();
        let evening_digest_time = evening_digest_time.clone();
        let save_state = morning_save_state.clone();
        Callback::from(move |new_time: Option<String>| {
            let morning = new_time.clone();
            let day = (*day_digest_time).clone();
            let evening = (*evening_digest_time).clone();
            let save_state = save_state.clone();
            let morning_digest_time = morning_digest_time.clone();
            save_state.set(FieldSaveState::Saving);
            spawn_local(async move {
                let request = UpdateDigestsRequest {
                    morning_digest_time: morning.clone(),
                    day_digest_time: day,
                    evening_digest_time: evening,
                };
                match Api::post("/api/profile/digests")
                    .json(&request)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(response) if response.ok() => {
                        morning_digest_time.set(morning);
                        info!("Successfully updated morning digest time");
                        save_state.set(FieldSaveState::Success);
                        let save_state_clone = save_state.clone();
                        spawn_local(async move {
                            gloo_timers::future::TimeoutFuture::new(2000).await;
                            save_state_clone.set(FieldSaveState::Idle);
                        });
                    },
                    Ok(_) => {
                        save_state.set(FieldSaveState::Error("Failed to save".to_string()));
                    },
                    Err(_) => {
                        save_state.set(FieldSaveState::Error("Network error".to_string()));
                    }
                }
            });
        })
    };

    // Create save callback for day digest
    let save_day = {
        let morning_digest_time = morning_digest_time.clone();
        let day_digest_time = day_digest_time.clone();
        let evening_digest_time = evening_digest_time.clone();
        let save_state = day_save_state.clone();
        Callback::from(move |new_time: Option<String>| {
            let morning = (*morning_digest_time).clone();
            let day = new_time.clone();
            let evening = (*evening_digest_time).clone();
            let save_state = save_state.clone();
            let day_digest_time = day_digest_time.clone();
            save_state.set(FieldSaveState::Saving);
            spawn_local(async move {
                let request = UpdateDigestsRequest {
                    morning_digest_time: morning,
                    day_digest_time: day.clone(),
                    evening_digest_time: evening,
                };
                match Api::post("/api/profile/digests")
                    .json(&request)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(response) if response.ok() => {
                        day_digest_time.set(day);
                        info!("Successfully updated day digest time");
                        save_state.set(FieldSaveState::Success);
                        let save_state_clone = save_state.clone();
                        spawn_local(async move {
                            gloo_timers::future::TimeoutFuture::new(2000).await;
                            save_state_clone.set(FieldSaveState::Idle);
                        });
                    },
                    Ok(_) => {
                        save_state.set(FieldSaveState::Error("Failed to save".to_string()));
                    },
                    Err(_) => {
                        save_state.set(FieldSaveState::Error("Network error".to_string()));
                    }
                }
            });
        })
    };

    // Create save callback for evening digest
    let save_evening = {
        let morning_digest_time = morning_digest_time.clone();
        let day_digest_time = day_digest_time.clone();
        let evening_digest_time = evening_digest_time.clone();
        let save_state = evening_save_state.clone();
        Callback::from(move |new_time: Option<String>| {
            let morning = (*morning_digest_time).clone();
            let day = (*day_digest_time).clone();
            let evening = new_time.clone();
            let save_state = save_state.clone();
            let evening_digest_time = evening_digest_time.clone();
            save_state.set(FieldSaveState::Saving);
            spawn_local(async move {
                let request = UpdateDigestsRequest {
                    morning_digest_time: morning,
                    day_digest_time: day,
                    evening_digest_time: evening.clone(),
                };
                match Api::post("/api/profile/digests")
                    .json(&request)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(response) if response.ok() => {
                        evening_digest_time.set(evening);
                        info!("Successfully updated evening digest time");
                        save_state.set(FieldSaveState::Success);
                        let save_state_clone = save_state.clone();
                        spawn_local(async move {
                            gloo_timers::future::TimeoutFuture::new(2000).await;
                            save_state_clone.set(FieldSaveState::Idle);
                        });
                    },
                    Ok(_) => {
                        save_state.set(FieldSaveState::Error("Failed to save".to_string()));
                    },
                    Err(_) => {
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
    let digest_extra: Html = html! {};
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
                .filter-title h3 {
                    margin: 0;
                    color: #F59E0B;
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
                .cost-description {
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
                .digest-options {
                    display: flex;
                    flex-direction: column;
                    gap: 1rem;
                    margin-top: 1rem;
                }
                .digest-option {
                    display: flex;
                    align-items: center;
                    justify-content: space-between;
                    padding: 1rem;
                    background: rgba(0, 0, 0, 0.2);
                    border: 1px solid rgba(245, 158, 11, 0.1);
                    border-radius: 12px;
                }
                .digest-label {
                    color: #fff;
                    font-size: 0.9rem;
                }
                /* Mobile responsiveness */
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
                    .digest-option {
                        flex-direction: column;
                        align-items: flex-start;
                        gap: 0.75rem;
                        padding: 0.75rem;
                    }
                    .digest-time {
                        width: 100%;
                        justify-content: space-between;
                    }
                    .time-input {
                        width: 150px;
                        padding: 0.4rem;
                        font-size: 0.9rem;
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
                    .status-text {
                        font-size: 0.75rem;
                    }
                }
                .switch {
                    position: relative;
                    display: inline-block;
                    width: 48px;
                    height: 24px;
                }
                .switch input {
                    opacity: 0;
                    width: 0;
                    height: 0;
                }
                .slider {
                    position: absolute;
                    cursor: pointer;
                    top: 0;
                    left: 0;
                    right: 0;
                    bottom: 0;
                    background: rgba(0, 0, 0, 0.2);
                    border: 1px solid rgba(245, 158, 11, 0.2);
                    transition: .4s;
                    border-radius: 24px;
                }
                .slider:before {
                    position: absolute;
                    content: "";
                    height: 16px;
                    width: 16px;
                    left: 4px;
                    bottom: 3px;
                    background-color: #fff;
                    transition: .4s;
                    border-radius: 50%;
                }
                input:checked + .slider {
                    background: #F59E0B;
                    border-color: #F59E0B;
                }
                input:checked + .slider:before {
                    transform: translateX(24px);
                }
                .digest-time {
                    display: flex;
                    align-items: center;
                    gap: 1rem;
                }
                .time-input {
                    background: rgba(30, 30, 30, 0.9);
                    color: #fff;
                    padding: 0.5rem;
                    font-size: 1rem;
                    width: 150px;
                    cursor: pointer;
                    border: 1px solid rgba(245, 158, 11, 0.3);
                    border-radius: 8px;
                    appearance: none;
                    -webkit-appearance: none;
                }
                /* Improve visibility of the time picker */
                .time-input::-webkit-calendar-picker-indicator {
                    background-color: rgba(245, 158, 11, 0.8);
                    padding: 4px;
                    cursor: pointer;
                    border-radius: 4px;
                }
                .time-input:hover {
                    border-color: #F59E0B;
                }
                .time-input:focus {
                    outline: none;
                    border-color: #F59E0B;
                    box-shadow: 0 0 0 2px rgba(245, 158, 11, 0.2);
                }
                /* Force 24-hour format */
                .time-input::-webkit-datetime-edit-ampm-field {
                    display: none;
                }
               
                .time-input::-moz-time-select {
                    -moz-appearance: textfield;
                }
                .digest-option.active {
                    border-color: rgba(34, 197, 94, 0.4);
                    background: rgba(34, 197, 94, 0.1);
                }
                .digest-option.inactive {
                    border-color: rgba(245, 158, 11, 0.1);
                    background: rgba(0, 0, 0, 0.2);
                }
                .status-text {
                    font-size: 0.8rem;
                    margin-left: 1rem;
                }
                .status-text.active {
                    color: #22C55E;
                }
                .status-text.inactive {
                    color: #999;
                }
                .save-button {
                    background: #F59E0B;
                    color: #000;
                    border: none;
                    border-radius: 8px;
                    padding: 0.5rem 1rem;
                    font-size: 0.9rem;
                    cursor: pointer;
                    transition: all 0.3s ease;
                    margin-top: 1rem;
                    width: 100%;
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    gap: 0.5rem;
                }
                .save-button:hover {
                    background: #D97706;
                }
                .save-button:disabled {
                    background: #666;
                    cursor: not-allowed;
                }
                .save-button.saving {
                    opacity: 0.7;
                    cursor: wait;
                }
                @keyframes spin {
                    0% { transform: rotate(0deg); }
                    100% { transform: rotate(360deg); }
                }
                .spinner {
                    border: 2px solid #000;
                    border-top: 2px solid transparent;
                    border-radius: 50%;
                    width: 16px;
                    height: 16px;
                    animation: spin 1s linear infinite;
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
                .save-success {
                    color: #22C55E;
                    font-size: 18px;
                }
                .save-error {
                    color: #EF4444;
                    cursor: help;
                    font-size: 18px;
                }
                .digest-label-row {
                    display: flex;
                    align-items: center;
                }
                .warning-modal {
                    position: fixed;
                    top: 0;
                    left: 0;
                    width: 100%;
                    height: 100%;
                    background: rgba(0, 0, 0, 0.7);
                    display: flex;
                    justify-content: center;
                    align-items: center;
                    z-index: 1000;
                }
                .warning-content {
                    background: #1A1A1A;
                    border: 1px solid rgba(245, 158, 11, 0.2);
                    border-radius: 12px;
                    padding: 1.5rem;
                    max-width: 90%;
                    width: 400px;
                }
                .warning-header {
                    color: #F59E0B;
                    font-size: 1.1rem;
                    margin-bottom: 1rem;
                }
                .warning-message {
                    color: #999;
                    font-size: 0.9rem;
                    margin-bottom: 1.5rem;
                    line-height: 1.5;
                }
                .warning-buttons {
                    display: flex;
                    gap: 1rem;
                    justify-content: flex-end;
                }
                .warning-button {
                    padding: 0.5rem 1rem;
                    border-radius: 6px;
                    font-size: 0.9rem;
                    cursor: pointer;
                    transition: all 0.3s ease;
                }
                .warning-button.confirm {
                    background: #F59E0B;
                    color: #000;
                    border: none;
                }
                .warning-button.confirm:hover {
                    background: #D97706;
                }
                .warning-button.cancel {
                    background: transparent;
                    color: #999;
                    border: 1px solid #666;
                }
                .warning-button.cancel:hover {
                    border-color: #999;
                    color: #fff;
                }
                .digest-section-wrapper {
                    transition: opacity 0.3s ease;
                }
                .digest-section-wrapper.disabled {
                    opacity: 0.5;
                    pointer-events: none;
                }
            "#}
        </style>
        <div class={classes!("digest-section-wrapper", if props.disabled { "disabled" } else { "" })}>
        <div class="filter-header">
            <div class="filter-title">
                <i class="fas fa-newspaper" style="color: #4ECDC4;"></i>
                <h3>{"Daily Digests"}</h3>
                <button
                    class="info-button"
                    onclick={Callback::from({
                        let show_info = show_info.clone();
                        move |_| show_info.set(!*show_info)
                    })}
                >
                    {"ⓘ"}
                </button>
            </div>
            <div class="flow-description">
                {"Get summarized updates about messages you might have missed and upcoming events at specific times of the day"}
            </div>
            <div class="cost-description">
                {digest_extra}
            </div>
            <div class="info-section" style={if *show_info { "display: block" } else { "display: none" }}>
                <h4>{"How It Works"}</h4>
                <div class="info-subsection">
                    <ul>
                        <li>{"Morning Digest: Get a summary of messages received overnight and about upcoming events"}</li>
                        <li>{"Day Digest: Receive a midday update of possible unread important messages and about upcoming events"}</li>
                        <li>{"Evening Digest: Receive an evening update of unread important messages since midday and events you have scheduled for the next day."}</li>
                    </ul>
                    <p style="margin-top: 1rem; font-style: italic;">{"Note: Digests are only sent when there are new unread messages or upcoming events to report. If you don't receive a digest at your scheduled time, it means there was nothing new to summarize."}</p>
                </div>
            </div>
        </div>
        <div class="digest-options">
            <div class={classes!("digest-option", if morning_digest_time.is_some() { "active" } else { "inactive" })}>
                <div class="digest-label-row">
                    <span class="digest-label">{"Morning Digest"}</span>
                    {render_save_indicator(&*morning_save_state)}
                </div>
                <div class="digest-time">
                    <select
                        class="time-input"
                        value={morning_digest_time.as_ref().cloned().unwrap_or("none".to_string())}
                        onchange={Callback::from({
                            let save_morning = save_morning.clone();
                            move |e: Event| {
                                let select: web_sys::HtmlSelectElement = e.target_unchecked_into();
                                let time = select.value();
                                save_morning.emit(if time == "none" { None } else { Some(time) });
                            }
                        })}
                    >
                        <option value="none" selected={morning_digest_time.is_none()}>{"Inactive"}</option>
                        <option value="00:00" selected={morning_digest_time.as_ref() == Some(&"00:00".to_string())}>{"00:00 (12:00 AM)"}</option>
                        <option value="01:00" selected={morning_digest_time.as_ref() == Some(&"01:00".to_string())}>{"01:00 (1:00 AM)"}</option>
                        <option value="02:00" selected={morning_digest_time.as_ref() == Some(&"02:00".to_string())}>{"02:00 (2:00 AM)"}</option>
                        <option value="03:00" selected={morning_digest_time.as_ref() == Some(&"03:00".to_string())}>{"03:00 (3:00 AM)"}</option>
                        <option value="04:00" selected={morning_digest_time.as_ref() == Some(&"04:00".to_string())}>{"04:00 (4:00 AM)"}</option>
                        <option value="05:00" selected={morning_digest_time.as_ref() == Some(&"05:00".to_string())}>{"05:00 (5:00 AM)"}</option>
                        <option value="06:00" selected={morning_digest_time.as_ref() == Some(&"06:00".to_string())}>{"06:00 (6:00 AM)"}</option>
                        <option value="07:00" selected={morning_digest_time.as_ref() == Some(&"07:00".to_string())}>{"07:00 (7:00 AM)"}</option>
                        <option value="08:00" selected={morning_digest_time.as_ref() == Some(&"08:00".to_string())}>{"08:00 (8:00 AM)"}</option>
                        <option value="09:00" selected={morning_digest_time.as_ref() == Some(&"09:00".to_string())}>{"09:00 (9:00 AM)"}</option>
                        <option value="10:00" selected={morning_digest_time.as_ref() == Some(&"10:00".to_string())}>{"10:00 (10:00 AM)"}</option>
                        <option value="11:00" selected={morning_digest_time.as_ref() == Some(&"11:00".to_string())}>{"11:00 (11:00 AM)"}</option>
                        <option value="12:00" selected={morning_digest_time.as_ref() == Some(&"12:00".to_string())}>{"12:00 (12:00 PM)"}</option>
                        <option value="13:00" selected={morning_digest_time.as_ref() == Some(&"13:00".to_string())}>{"13:00 (1:00 PM)"}</option>
                        <option value="14:00" selected={morning_digest_time.as_ref() == Some(&"14:00".to_string())}>{"14:00 (2:00 PM)"}</option>
                        <option value="15:00" selected={morning_digest_time.as_ref() == Some(&"15:00".to_string())}>{"15:00 (3:00 PM)"}</option>
                        <option value="16:00" selected={morning_digest_time.as_ref() == Some(&"16:00".to_string())}>{"16:00 (4:00 PM)"}</option>
                        <option value="17:00" selected={morning_digest_time.as_ref() == Some(&"17:00".to_string())}>{"17:00 (5:00 PM)"}</option>
                        <option value="18:00" selected={morning_digest_time.as_ref() == Some(&"18:00".to_string())}>{"18:00 (6:00 PM)"}</option>
                        <option value="19:00" selected={morning_digest_time.as_ref() == Some(&"19:00".to_string())}>{"19:00 (7:00 PM)"}</option>
                        <option value="20:00" selected={morning_digest_time.as_ref() == Some(&"20:00".to_string())}>{"20:00 (8:00 PM)"}</option>
                        <option value="21:00" selected={morning_digest_time.as_ref() == Some(&"21:00".to_string())}>{"21:00 (9:00 PM)"}</option>
                        <option value="22:00" selected={morning_digest_time.as_ref() == Some(&"22:00".to_string())}>{"22:00 (10:00 PM)"}</option>
                        <option value="23:00" selected={morning_digest_time.as_ref() == Some(&"23:00".to_string())}>{"23:00 (11:00 PM)"}</option>
                    </select>
                </div>
            </div>
            <div class={classes!("digest-option", if day_digest_time.is_some() { "active" } else { "inactive" })}>
                <div class="digest-label-row">
                    <span class="digest-label">{"Day Digest"}</span>
                    {render_save_indicator(&*day_save_state)}
                </div>
                <div class="digest-time">
                    <select
                        class="time-input"
                        value={day_digest_time.as_ref().cloned().unwrap_or("none".to_string())}
                        onchange={Callback::from({
                            let save_day = save_day.clone();
                            move |e: Event| {
                                let select: web_sys::HtmlSelectElement = e.target_unchecked_into();
                                let time = select.value();
                                save_day.emit(if time == "none" { None } else { Some(time) });
                            }
                        })}
                    >
                        <option value="none" selected={day_digest_time.is_none()}>{"Inactive"}</option>
                        <option value="00:00" selected={day_digest_time.as_ref() == Some(&"00:00".to_string())}>{"00:00 (12:00 AM)"}</option>
                        <option value="01:00" selected={day_digest_time.as_ref() == Some(&"01:00".to_string())}>{"01:00 (1:00 AM)"}</option>
                        <option value="02:00" selected={day_digest_time.as_ref() == Some(&"02:00".to_string())}>{"02:00 (2:00 AM)"}</option>
                        <option value="03:00" selected={day_digest_time.as_ref() == Some(&"03:00".to_string())}>{"03:00 (3:00 AM)"}</option>
                        <option value="04:00" selected={day_digest_time.as_ref() == Some(&"04:00".to_string())}>{"04:00 (4:00 AM)"}</option>
                        <option value="05:00" selected={day_digest_time.as_ref() == Some(&"05:00".to_string())}>{"05:00 (5:00 AM)"}</option>
                        <option value="06:00" selected={day_digest_time.as_ref() == Some(&"06:00".to_string())}>{"06:00 (6:00 AM)"}</option>
                        <option value="07:00" selected={day_digest_time.as_ref() == Some(&"07:00".to_string())}>{"07:00 (7:00 AM)"}</option>
                        <option value="08:00" selected={day_digest_time.as_ref() == Some(&"08:00".to_string())}>{"08:00 (8:00 AM)"}</option>
                        <option value="09:00" selected={day_digest_time.as_ref() == Some(&"09:00".to_string())}>{"09:00 (9:00 AM)"}</option>
                        <option value="10:00" selected={day_digest_time.as_ref() == Some(&"10:00".to_string())}>{"10:00 (10:00 AM)"}</option>
                        <option value="11:00" selected={day_digest_time.as_ref() == Some(&"11:00".to_string())}>{"11:00 (11:00 AM)"}</option>
                        <option value="12:00" selected={day_digest_time.as_ref() == Some(&"12:00".to_string())}>{"12:00 (12:00 PM)"}</option>
                        <option value="13:00" selected={day_digest_time.as_ref() == Some(&"13:00".to_string())}>{"13:00 (1:00 PM)"}</option>
                        <option value="14:00" selected={day_digest_time.as_ref() == Some(&"14:00".to_string())}>{"14:00 (2:00 PM)"}</option>
                        <option value="15:00" selected={day_digest_time.as_ref() == Some(&"15:00".to_string())}>{"15:00 (3:00 PM)"}</option>
                        <option value="16:00" selected={day_digest_time.as_ref() == Some(&"16:00".to_string())}>{"16:00 (4:00 PM)"}</option>
                        <option value="17:00" selected={day_digest_time.as_ref() == Some(&"17:00".to_string())}>{"17:00 (5:00 PM)"}</option>
                        <option value="18:00" selected={day_digest_time.as_ref() == Some(&"18:00".to_string())}>{"18:00 (6:00 PM)"}</option>
                        <option value="19:00" selected={day_digest_time.as_ref() == Some(&"19:00".to_string())}>{"19:00 (7:00 PM)"}</option>
                        <option value="20:00" selected={day_digest_time.as_ref() == Some(&"20:00".to_string())}>{"20:00 (8:00 PM)"}</option>
                        <option value="21:00" selected={day_digest_time.as_ref() == Some(&"21:00".to_string())}>{"21:00 (9:00 PM)"}</option>
                        <option value="22:00" selected={day_digest_time.as_ref() == Some(&"22:00".to_string())}>{"22:00 (10:00 PM)"}</option>
                        <option value="23:00" selected={day_digest_time.as_ref() == Some(&"23:00".to_string())}>{"23:00 (11:00 PM)"}</option>
                    </select>
                </div>
            </div>
            <div class={classes!("digest-option", if evening_digest_time.is_some() { "active" } else { "inactive" })}>
                <div class="digest-label-row">
                    <span class="digest-label">{"Evening Digest"}</span>
                    {render_save_indicator(&*evening_save_state)}
                </div>
                <div class="digest-time">
                    <select
                        class="time-input"
                        value={evening_digest_time.as_ref().cloned().unwrap_or("none".to_string())}
                        onchange={Callback::from({
                            let save_evening = save_evening.clone();
                            move |e: Event| {
                                let select: web_sys::HtmlSelectElement = e.target_unchecked_into();
                                let time = select.value();
                                save_evening.emit(if time == "none" { None } else { Some(time) });
                            }
                        })}
                    >
                        <option value="none" selected={evening_digest_time.is_none()}>{"Inactive"}</option>
                        <option value="00:00" selected={evening_digest_time.as_ref() == Some(&"00:00".to_string())}>{"00:00 (12:00 AM)"}</option>
                        <option value="01:00" selected={evening_digest_time.as_ref() == Some(&"01:00".to_string())}>{"01:00 (1:00 AM)"}</option>
                        <option value="02:00" selected={evening_digest_time.as_ref() == Some(&"02:00".to_string())}>{"02:00 (2:00 AM)"}</option>
                        <option value="03:00" selected={evening_digest_time.as_ref() == Some(&"03:00".to_string())}>{"03:00 (3:00 AM)"}</option>
                        <option value="04:00" selected={evening_digest_time.as_ref() == Some(&"04:00".to_string())}>{"04:00 (4:00 AM)"}</option>
                        <option value="05:00" selected={evening_digest_time.as_ref() == Some(&"05:00".to_string())}>{"05:00 (5:00 AM)"}</option>
                        <option value="06:00" selected={evening_digest_time.as_ref() == Some(&"06:00".to_string())}>{"06:00 (6:00 AM)"}</option>
                        <option value="07:00" selected={evening_digest_time.as_ref() == Some(&"07:00".to_string())}>{"07:00 (7:00 AM)"}</option>
                        <option value="08:00" selected={evening_digest_time.as_ref() == Some(&"08:00".to_string())}>{"08:00 (8:00 AM)"}</option>
                        <option value="09:00" selected={evening_digest_time.as_ref() == Some(&"09:00".to_string())}>{"09:00 (9:00 AM)"}</option>
                        <option value="10:00" selected={evening_digest_time.as_ref() == Some(&"10:00".to_string())}>{"10:00 (10:00 AM)"}</option>
                        <option value="11:00" selected={evening_digest_time.as_ref() == Some(&"11:00".to_string())}>{"11:00 (11:00 AM)"}</option>
                        <option value="12:00" selected={evening_digest_time.as_ref() == Some(&"12:00".to_string())}>{"12:00 (12:00 PM)"}</option>
                        <option value="13:00" selected={evening_digest_time.as_ref() == Some(&"13:00".to_string())}>{"13:00 (1:00 PM)"}</option>
                        <option value="14:00" selected={evening_digest_time.as_ref() == Some(&"14:00".to_string())}>{"14:00 (2:00 PM)"}</option>
                        <option value="15:00" selected={evening_digest_time.as_ref() == Some(&"15:00".to_string())}>{"15:00 (3:00 PM)"}</option>
                        <option value="16:00" selected={evening_digest_time.as_ref() == Some(&"16:00".to_string())}>{"16:00 (4:00 PM)"}</option>
                        <option value="17:00" selected={evening_digest_time.as_ref() == Some(&"17:00".to_string())}>{"17:00 (5:00 PM)"}</option>
                        <option value="18:00" selected={evening_digest_time.as_ref() == Some(&"18:00".to_string())}>{"18:00 (6:00 PM)"}</option>
                        <option value="19:00" selected={evening_digest_time.as_ref() == Some(&"19:00".to_string())}>{"19:00 (7:00 PM)"}</option>
                        <option value="20:00" selected={evening_digest_time.as_ref() == Some(&"20:00".to_string())}>{"20:00 (8:00 PM)"}</option>
                        <option value="21:00" selected={evening_digest_time.as_ref() == Some(&"21:00".to_string())}>{"21:00 (9:00 PM)"}</option>
                        <option value="22:00" selected={evening_digest_time.as_ref() == Some(&"22:00".to_string())}>{"22:00 (10:00 PM)"}</option>
                        <option value="23:00" selected={evening_digest_time.as_ref() == Some(&"23:00".to_string())}>{"23:00 (11:00 PM)"}</option>
                    </select>
                </div>
            </div>
        </div>
        </div> // Close digest-section-wrapper
        </>
    }
}
