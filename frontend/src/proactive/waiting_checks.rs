use yew::prelude::*;
use gloo_net::http::Request;
use log::{info, Level};
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, js_sys, HtmlInputElement, KeyboardEvent, InputEvent, Event};
use wasm_bindgen::JsValue;
use crate::config;
use serde_json::json;
use serde::{Deserialize, Serialize};


#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct WaitingCheck {
    pub content: String,
    pub service_type: String,
}

#[derive(Deserialize, Serialize)]
pub struct WaitingCheckRequest {
    content: String,
    service_type: String,
}
#[derive(Properties, PartialEq, Clone)]
pub struct WaitingChecksProps {
    pub service_type: String,
    pub checks: Vec<WaitingCheck>,
    pub on_change: Callback<Vec<WaitingCheck>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct PrioritySender {
    pub sender: String,
}


#[function_component(WaitingChecksSection)]
pub fn waiting_checks_section(props: &WaitingChecksProps) -> Html {
    let all_checks = props.checks.clone();
    let all_empty = all_checks.is_empty();
    let new_check = use_state(|| String::new());
    let selected_service = use_state(|| props.service_type.clone());
    let checks_local = use_state(|| props.checks.clone());
    let error_message = use_state(|| None::<String>);
    let show_info = use_state(|| false);

    let refresh_from_server = {
        let checks_local = checks_local.clone();
        let on_change = props.on_change.clone();
        Callback::from(move |_| {
            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                let checks_local = checks_local.clone();
                let on_change = on_change.clone();
                spawn_local(async move {
                    if let Ok(resp) = Request::get(&format!(
                        "{}/api/filters/waiting-checks",
                        config::get_backend_url(),
                    ))
                    .header("Authorization", &format!("Bearer {}", token))
                    .send()
                    .await
                    {
                        if let Ok(list) = resp.json::<Vec<WaitingCheck>>().await {
                            checks_local.set(list.clone());
                            on_change.emit(list);
                        }
                    }
                });
            }
        })
    };

    // Load checks when component mounts
    {
        let refresh_from_server = refresh_from_server.clone();
        use_effect_with_deps(
            move |_| {
                refresh_from_server.emit(());
                || ()
            },
            ()
        );
    }


    let add_waiting_check = {
        let new_check = new_check.clone();
        let refresh = refresh_from_server.clone();
        let selected_service = selected_service.clone();
        let checks_local = checks_local.clone();
        let error_message = error_message.clone();
        
        Callback::from(move |_| {
            let check = (*new_check).trim().to_string();
            if check.is_empty() { return; }
            let service_type = (*selected_service).clone();
            let error_message = error_message.clone();

            // Check if we've reached the maximum number of checks
            if (*checks_local).len() >= 5 {
                error_message.set(Some("Maximum of 5 waiting checks allowed".to_string()));
                return;
            }
            
            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                let new_check = new_check.clone();
                let refresh = refresh.clone();
                let service_type = service_type.clone();

                spawn_local(async move {
                    let _ = Request::post(&format!(
                        "{}/api/filters/waiting-check/{}",
                        config::get_backend_url(),
                        service_type
                    ))
                    .header("Authorization", &format!("Bearer {}", token))
                    .json(&WaitingCheckRequest {
                        content: check,
                        service_type: service_type.clone(),
                    })
                    .unwrap()
                    .send()
                    .await;

                    new_check.set(String::new());
                    error_message.set(None);
                    refresh.emit(());
                });
            }
        })
    };

    let delete_waiting_check = {
        let refresh = refresh_from_server.clone();
        
        Callback::from(move |(content, service_type): (String, String)| {
            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                let refresh = refresh.clone();
                
                spawn_local(async move {
                    let _ = Request::delete(&format!(
                        "{}/api/filters/waiting-check/{}/{}",
                        config::get_backend_url(),
                        service_type,
                        content
                    ))
                    .header("Authorization", &format!("Bearer {}", token))
                    .send()
                    .await;
                    
                    refresh.emit(());
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

                    .filter-title h3 {
                        margin: 0;
                        color: #F59E0B;
                        font-size: 1.2rem;
                    }

                    .status-badge {
                        background: rgba(245, 158, 11, 0.1);
                        color: #F59E0B;
                        padding: 0.25rem 0.75rem;
                        border-radius: 12px;
                        font-size: 0.8rem;
                    }

                    .status-badge.active {
                        background: rgba(52, 211, 153, 0.1);
                        color: #34D399;
                    }

                    .flow-description {
                        color: #999;
                        font-size: 0.9rem;
                    }

                    .waiting-check-input {
                        background: rgba(0, 0, 0, 0.2);
                        border: 1px solid rgba(245, 158, 11, 0.1);
                        border-radius: 12px;
                        padding: 1.5rem;
                        margin-bottom: 1.5rem;
                    }

                    .waiting-check-fields {
                        display: grid;
                        grid-template-columns: 1fr auto auto auto;
                        gap: 1rem;
                        align-items: center;
                        margin-bottom: 1rem;
                    }

                    @media (max-width: 768px) {
                        .waiting-check-fields {
                            grid-template-columns: 1fr;
                        }
                    }

                    .input-group {
                        display: flex;
                        gap: 0.5rem;
                        width: 100%;
                    }

                    .service-select {
                        padding: 0.75rem;
                        border-radius: 8px;
                        border: 1px solid rgba(245, 158, 11, 0.2);
                        background: rgba(0, 0, 0, 0.2);
                        color: #fff;
                        min-width: 140px;
                        cursor: pointer;
                    }

                    .service-select:focus {
                        outline: none;
                        border-color: #F59E0B;
                    }

                    .service-select option {
                        background: #1a1a1a;
                        color: #fff;
                        padding: 0.5rem;
                    }

                    .waiting-check-fields input[type="text"] {
                        padding: 0.75rem;
                        border-radius: 8px;
                        border: 1px solid rgba(245, 158, 11, 0.2);
                        background: rgba(0, 0, 0, 0.2);
                        color: #fff;
                        width: 100%;
                    }

                    .waiting-check-fields input[type="text"]:focus {
                        outline: none;
                        border-color: #F59E0B;
                    }

                    .date-label {
                        display: flex;
                        flex-direction: column;
                        gap: 0.25rem;
                    }

                    .date-label span {
                        font-size: 0.8rem;
                        color: #999;
                    }

                    .date-label input[type="date"] {
                        padding: 0.75rem;
                        border-radius: 8px;
                        border: 1px solid rgba(245, 158, 11, 0.2);
                        background: rgba(0, 0, 0, 0.2);
                        color: #fff;
                        min-width: 150px;
                    }

                    .waiting-check-fields label {
                        display: flex;
                        align-items: center;
                        gap: 0.5rem;
                        color: #999;
                        font-size: 0.9rem;
                    }

                    .waiting-check-fields input[type="checkbox"] {
                        width: 16px;
                        height: 16px;
                        border-radius: 4px;
                        border: 1px solid rgba(245, 158, 11, 0.2);
                        background: rgba(0, 0, 0, 0.2);
                        cursor: pointer;
                    }

                    .waiting-check-input button {
                        padding: 0.75rem 2rem;
                        border-radius: 8px;
                        border: none;
                        background: linear-gradient(45deg, #F59E0B, #D97706);
                        color: white;
                        cursor: pointer;
                        transition: all 0.3s ease;
                        font-weight: 500;
                    }

                    .waiting-check-input button:hover {
                        transform: translateY(-2px);
                        box-shadow: 0 4px 20px rgba(245, 158, 11, 0.3);
                    }

                    .filter-list {
                        list-style: none;
                        padding: 0;
                        margin: 0;
                        display: flex;
                        flex-direction: column;
                        gap: 0.75rem;
                    }

                    .filter-list li {
                        display: flex;
                        align-items: center;
                        gap: 1rem;
                        padding: 1rem;
                        background: rgba(0, 0, 0, 0.2);
                        border: 1px solid rgba(245, 158, 11, 0.1);
                        border-radius: 12px;
                        color: #fff;
                    }

                    .service-type-badge {
                        padding: 0.25rem 0.75rem;
                        border-radius: 8px;
                        font-size: 0.8rem;
                        background: rgba(0, 0, 0, 0.2);
                    }

                    .service-type-badge.email {
                        color: #1E90FF;
                        border: 1px solid rgba(245, 158, 11, 0.2);
                    }

                    .service-type-badge.messaging {
                        color: #25D366;
                        border: 1px solid rgba(236, 72, 153, 0.2);
                    }

                    .filter-list li:hover {
                        border-color: rgba(245, 158, 11, 0.2);
                        transform: translateY(-1px);
                        transition: all 0.3s ease;
                    }

                    .filter-list .delete-btn {
                        margin-left: auto;
                        background: none;
                        border: none;
                        color: #FF6347;
                        font-size: 1.2rem;
                        cursor: pointer;
                        padding: 0.5rem;
                        border-radius: 8px;
                        transition: all 0.3s ease;
                        display: flex;
                        align-items: center;
                        justify-content: center;
                        width: 32px;
                        height: 32px;
                    }

                    .filter-list .delete-btn:hover {
                        background: rgba(255, 99, 71, 0.1);
                        transform: scale(1.1);
                    }

                    .toggle-container {
                        display: flex;
                        align-items: center;
                        gap: 1rem;
                        margin-top: 1rem;
                    }

                    .toggle-label {
                        color: #999;
                        font-size: 0.9rem;
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

                    .error-message {
                        background: rgba(255, 99, 71, 0.1);
                        border: 1px solid rgba(255, 99, 71, 0.2);
                        color: #FF6347;
                        padding: 1rem;
                        border-radius: 8px;
                        margin-bottom: 1rem;
                        font-size: 0.9rem;
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

                    .info-subsection li:last-child {
                        margin-bottom: 0;
                    }
                "#}
            </style>
            <div class="filter-header">
                <div class="filter-title">
                    <i class="fas fa-hourglass-half" style="color: #4ECDC4;"></i>
                    <h3>{"Waiting Checks"}</h3>
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
                    {"Set up notifications for when you're waiting for responses or updates."}
                </div>
                <div class="info-section" style={if *show_info { "display: block" } else { "display: none" }}>
                    <h4>{"How It Works"}</h4>
                    <div class="info-subsection">
                        <ul>
                            <li>{"Lightfriend will notify you when it notices anything related to your waiting checks in messages"}</li>
                            <li>{"Checks are automatically removed once a match is found"}</li>
                            <li>{"You can set up to 5 waiting checks at a time"}</li>
                            <li>{"Waiting checks can also be set through SMS or voice calls with lightfriend"}</li>
                        </ul>
                    </div>
                </div>
            </div>

            {
                if let Some(error) = (*error_message).as_ref() {
                    html! {
                        <div class="error-message">
                            {error}
                        </div>
                    }
                } else {
                    html! {}
                }
            }

            <div class="waiting-check-input">
                <div class="waiting-check-fields">
                    <div class="input-group">
                        <select 
                            class="service-select"
                            value={(*selected_service).clone()}
                            onchange={Callback::from({
                                let selected_service = selected_service.clone();
                                move |e: Event| {
                                    let input: HtmlInputElement = e.target_unchecked_into();
                                    selected_service.set(input.value());
                                }
                            })}
                        >
                            <option value="email">{"Email"}</option>
                            <option value="messaging">{"Messaging Apps"}</option>
                        </select>
                        <input
                            type="text"
                            placeholder="Add waiting check phrase"
                            value={(*new_check).clone()}
                            oninput={Callback::from({
                                let new_check = new_check.clone();
                                move |e: InputEvent| {
                                    let input: HtmlInputElement = e.target_unchecked_into();
                                    new_check.set(input.value());
                                }
                            })}
                            onkeypress={Callback::from({
                                let add_waiting_check = add_waiting_check.clone();
                                move |e: KeyboardEvent| {
                                    if e.key() == "Enter" {
                                        add_waiting_check.emit(());
                                    }
                                }
                            })}
                    />
                </div>
                <button onclick={Callback::from(move |_| add_waiting_check.emit(()))}>{"Add"}</button>
            </div>

            <ul class="filter-list">
            {
                (*checks_local).iter().map(|check| {
                    let content = check.content.clone();
                    let service_type_class = if check.service_type == "email" { "email" } else { "messaging" };
                    let service_type_display = if check.service_type == "email" { "Email" } else { "Messaging" };
                    html! {
                        <li>
                            <span>{&check.content}</span>
                            <span class={classes!("service-type-badge", service_type_class)}>{service_type_display}</span>
                            <button class="delete-btn"
                                onclick={Callback::from({
                                    let content = content.clone();
                                    let service_type = check.service_type.clone();
                                    let delete_waiting_check = delete_waiting_check.clone();
                                    move |_| delete_waiting_check.emit((content.clone(), service_type.clone()))
                                })}
                            >{"×"}</button>
                        </li>
                    }
                }).collect::<Html>()
            }
            </ul>
        </div>
        </>
    }
}
