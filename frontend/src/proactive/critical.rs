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
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpdateCriticalRequest {
    enabled: Option<String>,
}

#[function_component(CriticalSection)]
pub fn critical_section() -> Html {
    let critical_enabled = use_state(|| None::<String>);
    let show_info = use_state(|| false);
    let is_saving = use_state(|| false);

    // Load critical notification settings when component mounts
    {
        let critical_enabled = critical_enabled.clone();
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
                    {"Get instant notifications for critical messages."}
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
                            <div class="radio-description">{"Receive critical alerts via SMS"}</div>
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
                            <div class="radio-description">{"Receive critical alerts via phone call"}</div>
                        </div>
                    </label>
                </div>
            </div>
        </>
    }
}
