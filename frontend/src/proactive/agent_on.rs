use yew::prelude::*;

use gloo_net::http::Request;

use log::info;
use wasm_bindgen_futures::spawn_local;
use web_sys::window;
use serde::{Deserialize, Serialize};
use crate::config;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProactiveResponse {
    enabled: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpdateProactiveRequest {
    enabled: bool,
}

#[function_component(ProactiveAgentSection)]
pub fn proactive_agent_section() -> Html {
    let proactive_enabled = use_state(|| true);
    let show_info = use_state(|| false);
    let is_saving = use_state(|| false);

    // Load proactive agent settings when component mounts
    {
        let proactive_enabled = proactive_enabled.clone();
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
                            "{}/api/profile/proactive-agent",
                            config::get_backend_url(),
                        ))
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await
                        {
                            if let Ok(proactive) = resp.json::<ProactiveResponse>().await {
                                info!("Received proactive settings from backend: {:?}", proactive);
                                proactive_enabled.set(proactive.enabled);
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
        let proactive_enabled = proactive_enabled.clone();
        let is_saving = is_saving.clone();
        Callback::from(move |new_value: bool| {
            let is_saving = is_saving.clone();
            proactive_enabled.set(new_value.clone());
            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                is_saving.set(true);
                spawn_local(async move {
                    let request = UpdateProactiveRequest {
                        enabled: new_value,
                    };
                    let result = Request::post(&format!(
                        "{}/api/profile/proactive-agent",
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
                    .filter-title.proactive h3 {
                        margin: 0;
                        color: white;
                        text-decoration: none;
                        font-weight: 600;
                        background: linear-gradient(45deg, #fff, #34D399);
                        -webkit-background-clip: text;
                        -webkit-text-fill-color: transparent;
                        transition: opacity 0.3s ease;
                        font-size: 1.2rem;
                    }
                    .info-button {
                        background: none;
                        border: none;
                        color: #34D399;
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
                        background: rgba(52, 211, 153, 0.1);
                        transform: scale(1.1);
                    }
                    .flow-description {
                        color: #999;
                        font-size: 0.9rem;
                    }
                    .info-section {
                        background: rgba(0, 0, 0, 0.2);
                        border: 1px solid rgba(52, 211, 153, 0.1);
                        border-radius: 12px;
                        padding: 1.5rem;
                        margin-top: 1rem;
                    }
                    .info-section h4 {
                        color: #34D399;
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
                    .proactive-option {
                        display: flex;
                        flex-direction: column;
                        align-items: flex-start;
                        gap: 1rem;
                        padding: 1rem;
                        background: rgba(0, 0, 0, 0.2);
                        border: 1px solid rgba(52, 211, 153, 0.1);
                        border-radius: 12px;
                        margin-top: 1rem;
                    }
                    .proactive-label {
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
                        .proactive-option {
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
                        background: rgba(52, 211, 153, 0.05);
                    }
                    .radio-option input[type="radio"] {
                        appearance: none;
                        width: 18px;
                        height: 18px;
                        border: 2px solid rgba(52, 211, 153, 0.3);
                        border-radius: 50%;
                        background: transparent;
                        cursor: pointer;
                        position: relative;
                        transition: all 0.2s ease;
                    }
                    .radio-option input[type="radio"]:checked {
                        border-color: #34D399;
                        background: rgba(52, 211, 153, 0.1);
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
                        background: #34D399;
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
                <div class="filter-title proactive">
                    <h3>{"Notifications Status"}</h3>
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
                    {"Easily toggle all notifications on and off."}
                </div>
                <div class="info-section" style={if *show_info { "display: block" } else { "display: none" }}>
                    <h4>{"How It Works"}</h4>
                    <div class="info-subsection">
                        <ul>
                            <li>{"When disabled, you won't receive any notifications."}</li>
                            <li>{"When enabled, notifications follow the rules set below."}</li>
                            <li>{"You can toggle this setting on the fly by calling or texting lightfriend to turn notifications off/on."}</li>
                            <li>{"This is useful when you only want notifications in specific situations. For example, you can enable notifications when going for a run, and disable it when you're return home."}</li>
                        </ul>
                    </div>
                </div>
            </div>
            <div class="proactive-option">
                <div class="radio-group">
                    <label class="radio-option" onclick={
                        let handle_option_change = handle_option_change.clone();
                        Callback::from(move |_| handle_option_change.emit(false))
                    }>
                        <input
                            type="radio"
                            name="proactive-agent"
                            checked={*proactive_enabled == false}
                        />
                        <div class="radio-label">
                            {"Disabled"}
                        </div>
                    </label>
                    <label class="radio-option" onclick={
                        let handle_option_change = handle_option_change.clone();
                        Callback::from(move |_| handle_option_change.emit(true))
                    }>
                        <input
                            type="radio"
                            name="proactive-agent"
                            checked={*proactive_enabled == true}
                        />
                        <div class="radio-label">
                            {"Enabled"}
                        </div>
                    </label>
                </div>
            </div>
        </>
    }
}
