use yew::prelude::*;
use gloo_net::http::Request;
use log::info;
use wasm_bindgen_futures::spawn_local;
use web_sys::window;
use serde::{Deserialize, Serialize};
use crate::config;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CriticalResponse {
    enabled: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpdateCriticalRequest {
    enabled: bool,
}

#[function_component(CriticalSection)]
pub fn critical_section() -> Html {
    let critical_enabled = use_state(|| false);
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


    let handle_toggle = {
        let critical_enabled = critical_enabled.clone();
        let is_saving = is_saving.clone();
        
        Callback::from(move |_| {
            let new_value = !*critical_enabled;
            let is_saving = is_saving.clone();
            critical_enabled.set(new_value);
            
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
                        align-items: center;
                        justify-content: space-between;
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

                    .critical-controls {
                        display: flex;
                        align-items: center;
                        gap: 1rem;
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

                        .critical-controls {
                            width: 100%;
                            justify-content: space-between;
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
                        <ul>
                            <li>{"Receive instant notifications for urgent messages"}</li>
                            <li>{"Never miss time-sensitive information"}</li>
                            <li>{"Get alerted about critical updates immediately"}</li>
                        </ul>
                    </div>
                </div>
            </div>

            <div class="critical-option">
                <span class="critical-label">{"Enable Critical Notifications"}</span>
                <div class="critical-controls">
                    <label class="switch">
                        <input 
                            type="checkbox"
                            checked={*critical_enabled}
                            onchange={handle_toggle}
                        />
                        <span class="slider"></span>
                    </label>
                    <span class={classes!("status-text", if *critical_enabled { "active" } else { "inactive" })}>
                        {if *critical_enabled { "Active" } else { "Inactive" }}
                    </span>
                </div>
            </div>
        </>
    }
}

