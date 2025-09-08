use yew::prelude::*;
use gloo_net::http::Request;
use log::info;
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, Event, HtmlInputElement};
use wasm_bindgen::JsCast;
use serde::{Deserialize, Serialize};
use crate::config;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CriticalResponse {
    enabled: Option<String>,
    average_critical_per_day: f32,
    estimated_monthly_price: f32,
    call_notify: bool,
    message_critical_mode: String,
    family_no_followup: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpdateCriticalRequest {
    enabled: Option<Option<String>>,
    call_notify: Option<bool>,
    message_critical_mode: Option<String>,
    family_no_followup: Option<bool>,
}

#[derive(Properties, PartialEq)]
pub struct CriticalSectionProps {
    pub phone_number: String,
}

#[function_component(CriticalSection)]
pub fn critical_section(props: &CriticalSectionProps) -> Html {
    let critical_enabled = use_state(|| None::<String>);
    let average_critical = use_state(|| 0.0);
    let estimated_price = use_state(|| 0.0);
    let call_notify = use_state(|| true);
    let is_saving = use_state(|| false);
    let message_mode = use_state(|| "all".to_string());
    let family_no_followup = use_state(|| false);

    // States for info toggles
    let show_message_info = use_state(|| false);
    let show_action_info = use_state(|| false);

    // Load critical notification settings when component mounts
    {
        let critical_enabled = critical_enabled.clone();
        let average_critical = average_critical.clone();
        let estimated_price = estimated_price.clone();
        let call_notify = call_notify.clone();
        let message_mode = message_mode.clone();
        let family_no_followup = family_no_followup.clone();
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
                            "{}/api/profile/critical",
                            config::get_backend_url(),
                        ))
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await
                        {
                            if let Ok(critical) = resp.json::<CriticalResponse>().await {
                                info!("Received critical settings from backend: {:?}", critical);
                                critical_enabled.set(critical.enabled);
                                average_critical.set(critical.average_critical_per_day);
                                estimated_price.set(critical.estimated_monthly_price);
                                call_notify.set(critical.call_notify);
                                message_mode.set(critical.message_critical_mode);
                                family_no_followup.set(critical.family_no_followup);
                            }
                        }
                    });
                }
                || ()
            },
            (),
        );
    }

    let handle_option_change = {
        let critical_enabled = critical_enabled.clone();
        let is_saving = is_saving.clone();
        Callback::from(move |new_value: Option<String>| {
            let is_saving = is_saving.clone();
            critical_enabled.set(new_value.clone());
            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                is_saving.set(true);
                spawn_local(async move {
                    let request = UpdateCriticalRequest {
                        enabled: Some(new_value),
                        call_notify: None,
                        message_critical_mode: None,
                        family_no_followup: None,
                    };
                    let result = Request::post(&format!(
                        "{}/api/profile/critical",
                        config::get_backend_url(),
                    ))
                    .header("Authorization", &format!("Bearer {}", token))
                    .json(&request)
                    .unwrap()
                    .send()
                    .await;
                    is_saving.set(false);
                });
            }
        })
    };

    let handle_call_notify_change = {
        let call_notify = call_notify.clone();
        let is_saving = is_saving.clone();
        Callback::from(move |new_value: bool| {
            let is_saving = is_saving.clone();
            call_notify.set(new_value);
            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                is_saving.set(true);
                spawn_local(async move {
                    let request = UpdateCriticalRequest {
                        enabled: None,
                        call_notify: Some(new_value),
                        message_critical_mode: None,
                        family_no_followup: None,
                    };
                    let result = Request::post(&format!(
                        "{}/api/profile/critical",
                        config::get_backend_url(),
                    ))
                    .header("Authorization", &format!("Bearer {}", token))
                    .json(&request)
                    .unwrap()
                    .send()
                    .await;
                    is_saving.set(false);
                });
            }
        })
    };

    let handle_message_mode_change = {
        let message_mode = message_mode.clone();
        let is_saving = is_saving.clone();
        Callback::from(move |new_value: String| {
            let is_saving = is_saving.clone();
            message_mode.set(new_value.clone());
            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                is_saving.set(true);
                spawn_local(async move {
                    let request = UpdateCriticalRequest {
                        enabled: None,
                        call_notify: None,
                        message_critical_mode: Some(new_value),
                        family_no_followup: None,
                    };
                    let result = Request::post(&format!(
                        "{}/api/profile/critical",
                        config::get_backend_url(),
                    ))
                    .header("Authorization", &format!("Bearer {}", token))
                    .json(&request)
                    .unwrap()
                    .send()
                    .await;
                    is_saving.set(false);
                });
            }
        })
    };

    let handle_family_no_followup_change = {
        let family_no_followup = family_no_followup.clone();
        let is_saving = is_saving.clone();
        Callback::from(move |new_value: bool| {
            let is_saving = is_saving.clone();
            family_no_followup.set(new_value);
            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                is_saving.set(true);
                spawn_local(async move {
                    let request = UpdateCriticalRequest {
                        enabled: None,
                        call_notify: None,
                        message_critical_mode: None,
                        family_no_followup: Some(new_value),
                    };
                    let result = Request::post(&format!(
                        "{}/api/profile/critical",
                        config::get_backend_url(),
                    ))
                    .header("Authorization", &format!("Bearer {}", token))
                    .json(&request)
                    .unwrap()
                    .send()
                    .await;
                    is_saving.set(false);
                });
            }
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
    let sms_extra: Html = match country {
        "US" => html! { <span>{" (1/2 Message)"}</span> },
        "FI" => html! { <span>{format!(" (€{:.2} per message)", 0.15)}</span> },
        "NL" => html! { <span>{format!(" (€{:.2} per message)", 0.15)}</span> },
        "UK" => html! { <span>{format!(" (£{:.2} per message)", 0.15)}</span> },
        "AU" => html! { <span>{format!(" (${:.2} per message)", 0.15)}</span> },
        "Other" => html! { <>{" ("}<a href="/bring-own-number">{"see pricing"}</a>{")"}</> },
        _ => html! {},
    };
    let call_extra: Html = match country {
        "US" => html! { <span>{" (1/2 Message)"}</span> },
        "FI" => html! { <span>{format!(" (€{:.2} per call)", 0.70)}</span> },
        "NL" => html! { <span>{format!(" (€{:.2} per call)", 0.70)}</span> },
        "UK" => html! { <span>{format!(" (£{:.2} per call)", 0.15)}</span> },
        "AU" => html! { <span>{format!(" (${:.2} per call)", 0.15)}</span> },
        "Other" => html! { <>{" ("}<a href="/bring-own-number">{"see pricing"}</a>{")"}</> },
        _ => html! {},
    };

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
                "#}
            </style>
            <div class="filter-header">
                <div class="filter-title critical">
                    <h3>{"Critical Notifications"}</h3>
                </div>
                <div class="flow-description">
                    {if country == "US" {
                        format!(
                            "Instant alerts for urgent items. Usage: ~{:.1}/day, est. cost: {:.2} Messages/month",
                            *average_critical, *estimated_price / 0.5
                        )
                    } else {
                        format!(
                            "Instant alerts for urgent items. Usage: ~{:.1}/day, est. cost: {}{:.2}/month",
                            *average_critical, currency, *estimated_price
                        )
                    }}
                </div>
            </div>
            <div class="critical-option">
                <span class="critical-label">{"Notification Method"}</span>
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
                            checked={*critical_enabled == Some("sms".to_string())}
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
                            checked={*critical_enabled == Some("call".to_string())}
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
                <span class="critical-label">{"What is Critical?"}</span>
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
                                        <div class="radio-description">{"Alert for calls immediately"}</div>
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
                    <span class="critical-label">{"Action on Critical Message (this feature is coming soon, you will be notified by email when ready! also you can tell me your thoughts about it rasmus@ahtava.com)"}</span>
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
                                <li>{"Family Only: Alert only if sender is in your family contacts."}</li>
                                <li>{"Ask Sender: Lightfriend asks sender the following: \"Hi, I'm Lightfriend, your friend's AI assistant. This message looks time-sensitive—since they're not currently on their computer, would you like me to send them a notification about it? Reply \"yes\" or \"no.\""}</li>
                                <li>{"Always Notify Family: For family senders, notify without follow-up question (only when 'Ask Sender' is selected)."}</li>
                            </ul>
                        </div>
                    }
                } else {
                    html! {}
                }}
                <div class="radio-group">
                    <label class="radio-option" onclick={
                        let handle_message_mode_change = handle_message_mode_change.clone();
                        Callback::from(move |_| handle_message_mode_change.emit("all".to_string()))
                    }>
                        <input
                            type="radio"
                            name="message-critical-mode"
                            checked={*message_mode == "all"}
                        />
                        <div class="radio-label">
                            {"Notify All"}
                        </div>
                    </label>
                    <label class="radio-option" onclick={
                        let handle_message_mode_change = handle_message_mode_change.clone();
                        Callback::from(move |_| handle_message_mode_change.emit("family".to_string()))
                    }>
                        <input
                            type="radio"
                            name="message-critical-mode"
                            checked={*message_mode == "family"}
                        />
                        <div class="radio-label">
                            {"Family Only"}
                        </div>
                    </label>
                    <label class="radio-option" onclick={
                        let handle_message_mode_change = handle_message_mode_change.clone();
                        Callback::from(move |_| handle_message_mode_change.emit("none".to_string()))
                    }>
                        <input
                            type="radio"
                            name="message-critical-mode"
                            checked={*message_mode == "none"}
                        />
                        <div class="radio-label">
                            {"Ask Sender"}
                        </div>
                    </label>
                    <label style="display: flex; align-items: center; gap: 0.75rem; margin-left: 2.5rem; margin-top: 0.5rem;">
                        <input
                            type="checkbox"
                            checked={*family_no_followup}
                            disabled={*message_mode != "none"}
                            onchange={Callback::from({
                                let handle_family_no_followup_change = handle_family_no_followup_change.clone();
                                let message_mode = message_mode.clone();
                                move |e: Event| {
                                    if *message_mode == "none" {
                                        if let Some(input) = e.target().and_then(|t| t.dyn_into::<HtmlInputElement>().ok()) {
                                            handle_family_no_followup_change.emit(input.checked());
                                        }
                                    }
                                }
                            })}
                        />
                        <span class="radio-label">{"Always Notify Family (No Follow-up)"}</span>
                    </label>
                </div>
            </div>
        </>
    }
}
