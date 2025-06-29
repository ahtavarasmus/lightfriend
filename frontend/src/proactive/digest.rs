use yew::prelude::*;
use gloo_net::http::Request;
use log::info;
use wasm_bindgen_futures::spawn_local;
use web_sys::window;
use serde::{Deserialize, Serialize};
use crate::config;
use chrono::{DateTime, NaiveTime, Utc, TimeZone, Local};
use chrono_tz::Tz;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DigestsResponse {
    morning_digest_time: Option<String>,  // RFC3339 time string or None
    day_digest_time: Option<String>,      // RFC3339 time string or None
    evening_digest_time: Option<String>,  // RFC3339 time string or None
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpdateDigestsRequest {
    morning_digest_time: Option<String>,  // RFC3339 time string or None
    day_digest_time: Option<String>,      // RFC3339 time string or None
    evening_digest_time: Option<String>,  // RFC3339 time string or None
}

#[function_component(DigestSection)]
pub fn digest_section() -> Html {
    let morning_digest_time = use_state(|| None::<String>);
    let day_digest_time = use_state(|| None::<String>);
    let evening_digest_time = use_state(|| None::<String>);
    let show_info = use_state(|| false);
    let is_saving = use_state(|| false);
    let has_unsaved_changes = use_state(|| false);

    // Load digest settings when component mounts
    {
        let morning_digest_time = morning_digest_time.clone();
        let day_digest_time = day_digest_time.clone();
        let evening_digest_time = evening_digest_time.clone();

        use_effect_with_deps(
            move |_| {
                if let Some(token) = window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|s| s.get_item("token").ok())
                    .flatten()
                {
                    spawn_local(async move {
                        if let Ok(resp) = Request::get(&format!(
                            "{}/api/profile/digests",
                            config::get_backend_url(),
                        ))
                        .header("Authorization", &format!("Bearer {}", token))
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
                }
                || ()
            },
            (),
        );
    }

    let update_digests = {
        let morning_digest_time = morning_digest_time.clone();
        let day_digest_time = day_digest_time.clone();
        let evening_digest_time = evening_digest_time.clone();
        let is_saving = is_saving.clone();
        let has_unsaved_changes = has_unsaved_changes.clone();

        Callback::from(move |_| {
            let morning = (*morning_digest_time).clone();
            let day = (*day_digest_time).clone();
            let evening = (*evening_digest_time).clone();

            let is_saving = is_saving.clone();
            let has_unsaved_changes = has_unsaved_changes.clone();


            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                is_saving.set(true);
                spawn_local(async move {
                    let request = UpdateDigestsRequest {
                        morning_digest_time: morning,
                        day_digest_time: day,
                        evening_digest_time: evening,
                    };

                    let result = Request::post(&format!(
                        "{}/api/profile/digests",
                        config::get_backend_url(),
                    ))
                    .header("Authorization", &format!("Bearer {}", token))
                    .json(&request)
                    .unwrap()
                    .send()
                    .await;

                    is_saving.set(false);
                    if result.is_ok() {
                        has_unsaved_changes.set(false);
                    }
                });
            }
        })
    };

    let handle_time_change = {
        let has_unsaved_changes = has_unsaved_changes.clone();
        
        Callback::from(move |_| {
            has_unsaved_changes.set(true);
        })
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
            "#}
        </style>
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
                {"Get summarized updates about your messages and upcoming events at specific times of the day"}
            </div>
            <div class="info-section" style={if *show_info { "display: block" } else { "display: none" }}>
                <h4>{"How It Works"}</h4>
                <div class="info-subsection">
                    <ul>
                        <li>{"Morning Digest: Get a summary of messages received overnight and about upcoming events"}</li>
                        <li>{"Day Digest: Receive a midday update of important messages and about upcoming events"}</li>
                        <li>{"Evening Digest: Review a summary of the day's communications and next days events"}</li>
                    </ul>
                </div>
            </div>
        </div>

        <div class="digest-options">
            <div class={classes!("digest-option", if morning_digest_time.is_some() { "active" } else { "inactive" })}>
                <span class="digest-label">{"Morning Digest"}</span>
                <div class="digest-time">
                    <select
                        class="time-input"
                        value={morning_digest_time.as_ref().cloned().unwrap_or("none".to_string())}
                        onchange={Callback::from({
                            let morning_digest_time = morning_digest_time.clone();
                            let handle_time_change = handle_time_change.clone();
                            move |e: Event| {
                                let select: web_sys::HtmlSelectElement = e.target_unchecked_into();
                                let time = select.value();
                                morning_digest_time.set(if time == "none" { None } else { Some(time) });
                                handle_time_change.emit(());
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
                <span class="digest-label">{"Day Digest"}</span>
                <div class="digest-time">
                    <select
                        class="time-input"
                        value={day_digest_time.as_ref().cloned().unwrap_or("none".to_string())}
                        onchange={Callback::from({
                            let day_digest_time = day_digest_time.clone();
                            let handle_time_change = handle_time_change.clone();
                            move |e: Event| {
                                let select: web_sys::HtmlSelectElement = e.target_unchecked_into();
                                let time = select.value();
                                day_digest_time.set(if time == "none" { None } else { Some(time) });
                                handle_time_change.emit(());
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
                <span class="digest-label">{"Evening Digest"}</span>
                <div class="digest-time">
                    <select
                        class="time-input"
                        value={evening_digest_time.as_ref().cloned().unwrap_or("none".to_string())}
                        onchange={Callback::from({
                            let evening_digest_time = evening_digest_time.clone();
                            let handle_time_change = handle_time_change.clone();
                            move |e: Event| {
                                let select: web_sys::HtmlSelectElement = e.target_unchecked_into();
                                let time = select.value();
                                evening_digest_time.set(if time == "none" { None } else { Some(time) });
                                handle_time_change.emit(());
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

        <button
            class={classes!("save-button", if *is_saving { "saving" } else { "" })}
            onclick={update_digests}
            disabled={*is_saving || !*has_unsaved_changes}
        >
            {if *is_saving {
                html! {
                    <>
                        <div class="spinner" />
                        {"Saving..."}
                    </>
                }
            } else {
                html! {
                    {"Save Changes"}
                }
            }}
        </button>
        </>
    }
}

