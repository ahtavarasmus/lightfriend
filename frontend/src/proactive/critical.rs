use yew::prelude::*;
use gloo_net::http::Request;
use log::info;
use wasm_bindgen_futures::spawn_local;
use web_sys::window;
use serde::{Deserialize, Serialize};
use crate::config;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CriticalResponse {
    enabled: Option<String>,
    average_critical_per_day: f32,
    estimated_monthly_price: f32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpdateCriticalRequest {
    enabled: Option<String>,
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
    let show_info = use_state(|| false);
    let is_saving = use_state(|| false);

    // Load critical notification settings when component mounts
    {
        let critical_enabled = critical_enabled.clone();
        let average_critical = average_critical.clone();
        let estimated_price = estimated_price.clone();
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
                        enabled: new_value,
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
                "#}
            </style>
            <div class="filter-header">
                <div class="filter-title critical">
                    <h3>{"Critical Notifications"}</h3>
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
                    {if country == "US" {
                        format!(
                            "Get instant notifications for critical messages. Based on your usage (~{:.1} critical notifications/day), estimated cost: {:.2} Messages/month",
                            *average_critical, *estimated_price / 0.5
                        )
                    } else {
                        format!(
                            "Get instant notifications for critical messages. Based on your usage (~{:.1} critical notifications/day), estimated cost: {}{:.2}/month",
                            *average_critical, currency, *estimated_price
                        )
                    }}
                </div>
                <div class="info-section" style={if *show_info { "display: block" } else { "display: none" }}>
                    <h4>{"How It Works"}</h4>
                    <div class="info-subsection">
                        <p>{"We handle incoming communications as follows to ensure you never miss truly urgent matters:"}</p>
                        <ul>
                            <li>
                                <i class="fa-solid fa-gears"></i>{" Incoming calls on any messaging platform: You will be notified every time about the call. (Rule-based)"}
                            </li>
                            <li>
                                <i class="fa-solid fa-hat-wizard"></i>{" Incoming messages: Analyzed by AI to determine if critical. The AI looks for situations where delaying action beyond 2 hours risks:"}
                                <ul>
                                    <li>{"Direct harm to people"}</li>
                                    <li>{"Severe data loss or major financial loss"}</li>
                                    <li>{"Production system outage or security breach"}</li>
                                    <li>{"Hard legal/compliance deadline expiring in 2 hours or less"}</li>
                                    <li>{"The sender explicitly indicates it must be handled immediately (e.g., “ASAP”, “emergency”, “right now”) or gives a deadline of 2 hours or less"}</li>
                                </ul>
                                {"Everything else — like vague urgency, routine updates, or unclear requests — is not critical and will be handled in your next scheduled summary."}
                            </li>
                        </ul>
                    </div>
                </div>
            </div>
            <div class="critical-option">
                <span class="critical-label">{"Critical Notification Method"}</span>
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
                            <div class="radio-description">{"No critical notifications"}</div>
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
                            {"SMS Notifications"}
                            <div class="radio-description">
                                {"Receive critical alerts via SMS"}
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
                            {"Phone Call Notifications"}
                            <div class="radio-description">
                                {"Receive critical alerts via phone call"}
                                {call_extra}
                            </div>
                        </div>
                    </label>
                </div>
            </div>
        </>
    }
}
