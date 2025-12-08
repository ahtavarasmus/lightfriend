use yew::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlInputElement, KeyboardEvent, InputEvent, Event};
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
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct WaitingCheck {
    pub content: String,
    pub service_type: String,
    pub noti_type: Option<String>,
}
#[derive(Deserialize, Serialize)]
pub struct WaitingCheckRequest {
    content: String,
    service_type: String,
    noti_type: Option<String>,
}
#[derive(Properties, PartialEq, Clone)]
pub struct WaitingChecksProps {
    pub service_type: String,
    pub checks: Vec<WaitingCheck>,
    pub on_change: Callback<Vec<WaitingCheck>>,
    pub phone_number: String,
    #[prop_or(false)]
    pub critical_disabled: bool,
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
    let selected_noti_type = use_state(|| "sms".to_string());
    let checks_local = use_state(|| props.checks.clone());
    let error_message = use_state(|| None::<String>);
    let show_info = use_state(|| false);
    let add_save_state = use_state(|| FieldSaveState::Idle);
    let refresh_from_server = {
        let checks_local = checks_local.clone();
        let on_change = props.on_change.clone();
        Callback::from(move |_| {
            let checks_local = checks_local.clone();
            let on_change = on_change.clone();
            spawn_local(async move {
                if let Ok(resp) = Api::get("/api/filters/waiting-checks")
                    .send()
                    .await
                {
                    if let Ok(list) = resp.json::<Vec<WaitingCheck>>().await {
                        checks_local.set(list.clone());
                        on_change.emit(list);
                    }
                }
            });
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
        let selected_noti_type = selected_noti_type.clone();
        let checks_local = checks_local.clone();
        let error_message = error_message.clone();
        let save_state = add_save_state.clone();

        Callback::from(move |_| {
            let check = (*new_check).trim().to_string();
            if check.is_empty() { return; }
            let service_type = (*selected_service).clone();
            let error_message = error_message.clone();
            let save_state = save_state.clone();
            // Check if we've reached the maximum number of checks
            if (*checks_local).len() >= 5 {
                error_message.set(Some("Maximum of 5 waiting checks allowed".to_string()));
                save_state.set(FieldSaveState::Error("Max limit reached".to_string()));
                return;
            }

            let new_check = new_check.clone();
            let refresh = refresh.clone();
            let service_type = service_type.clone();
            let noti_type = (*selected_noti_type).clone();
            save_state.set(FieldSaveState::Saving);
            spawn_local(async move {
                match Api::post(&format!(
                    "/api/filters/waiting-check/{}",
                    service_type
                ))
                .json(&WaitingCheckRequest {
                    content: check,
                    service_type: service_type.clone(),
                    noti_type: Some(noti_type),
                })
                .unwrap()
                .send()
                .await
                {
                    Ok(response) if response.ok() => {
                        new_check.set(String::new());
                        error_message.set(None);
                        save_state.set(FieldSaveState::Success);
                        let save_state_clone = save_state.clone();
                        spawn_local(async move {
                            gloo_timers::future::TimeoutFuture::new(2000).await;
                            save_state_clone.set(FieldSaveState::Idle);
                        });
                        refresh.emit(());
                    },
                    Ok(_) => {
                        save_state.set(FieldSaveState::Error("Failed to add".to_string()));
                    },
                    Err(_) => {
                        save_state.set(FieldSaveState::Error("Network error".to_string()));
                    }
                }
            });
        })
    };
    let delete_waiting_check = {
        let refresh = refresh_from_server.clone();
      
        Callback::from(move |(content, service_type): (String, String)| {
            let refresh = refresh.clone();

            spawn_local(async move {
                let _ = Api::delete(&format!(
                    "/api/filters/waiting-check/{}/{}",
                    service_type,
                    content
                ))
                .send()
                .await;

                refresh.emit(());
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
    let sms_extra: Html = html! {};
    let call_extra: Html = html! {};
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
                        grid-template-columns: 1fr auto;
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
                    .noti-select {
                        padding: 0.75rem;
                        border-radius: 8px;
                        border: 1px solid rgba(245, 158, 11, 0.2);
                        background: rgba(0, 0, 0, 0.2);
                        color: #fff;
                        min-width: 100px;
                        cursor: pointer;
                    }
                    .noti-select:focus {
                        outline: none;
                        border-color: #F59E0B;
                    }
                    .noti-select option {
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
                    .noti-type-badge {
                        padding: 0.25rem 0.75rem;
                        border-radius: 8px;
                        font-size: 0.8rem;
                        background: rgba(0, 0, 0, 0.2);
                    }
                    .noti-type-badge.sms {
                        color: #4ECDC4;
                        border: 1px solid rgba(78, 205, 196, 0.2);
                    }
                    .noti-type-badge.call {
                        color: #FF6347;
                        border: 1px solid rgba(255, 99, 71, 0.2);
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
                    .price-note {
                        color: #999;
                        font-size: 0.8rem;
                        white-space: nowrap;
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
                    .add-button-row {
                        display: flex;
                        align-items: center;
                        gap: 0.5rem;
                    }
                    .section-disabled {
                        opacity: 0.5;
                    }
                    .disabled-hint {
                        font-size: 0.75rem;
                        color: #666;
                        font-style: italic;
                        margin-left: 0.5rem;
                    }
                "#}
            </style>
            <div class={classes!(if props.critical_disabled { "section-disabled" } else { "" })}>
            <div class="filter-header">
                <div class="filter-title">
                    <i class="fas fa-hourglass-half" style="color: #4ECDC4;"></i>
                    <h3>{"Waiting Checks"}</h3>
                    {if props.critical_disabled {
                        html! { <span class="disabled-hint">{"(not active)"}</span> }
                    } else {
                        html! {}
                    }}
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
                            <li>{"Lightfriend will notify you when it notices anything related to your waiting checks in messages or emails"}</li>
                            <li>{"Notifications are sent via SMS or Call depending on your choice"}</li>
                            <li>{"Checks are automatically removed once a match is found"}</li>
                            <li>{"You can set up to 5 waiting checks at a time"}</li>
                            <li>{"Waiting checks can also be set through SMS or voice calls with lightfriend. Just ask lightfriend to keep an eye out for something!"}</li>
                        </ul>
                    </div>
                    <h4>{"Best Practices for Waiting Check Phrases"}</h4>
                    <div class="info-subsection">
                        <p>{"Enter a phrase describing what to watch for in incoming messages. For best results:"}</p>
                        <ul>
                            <li><strong>{"Keep it short (≤5 words)"}</strong>{" for exact keyword matches: e.g., 'meeting rescheduled' or 'order shipped'. The message must contain all these words (case-insensitive)."}</li>
                            <li><strong>{"Use longer descriptions (>5 words)"}</strong>{" for smarter, context-aware matching: e.g., 'Any update from Rasmus about the new phone model, including synonyms like smartphone or device'. This allows paraphrases and related ideas."}</li>
                            <li><strong>{"Include sender if important"}</strong>{": e.g., 'Message from @rasmus containing phone details' – otherwise, sender alone won't trigger a match."}</li>
                            <li><strong>{"Be specific and unambiguous"}</strong>{": Avoid vague terms; include conditions like 'must include a link' or 'related to travel plans'."}</li>
                            <li><strong>{"Non-English?"}</strong>{" The AI handles translations internally."}</li>
                            <li>{"Examples:"}
                                <ul>
                                    <li>{"Short: 'flight delayed'"}</li>
                                    <li>{"Long: 'Notification from bank about unusual activity on my account'"}</li>
                                    <li>{"With sender: 'Email from support@company.com with resolution to ticket #123'"}</li>
                                </ul>
                            </li>
                        </ul>
                        <p>{"Notifications will only trigger on clear, definitive matches."}</p>
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
                        <select
                            class="noti-select"
                            value={(*selected_noti_type).clone()}
                            onchange={Callback::from({
                                let selected_noti_type = selected_noti_type.clone();
                                move |e: Event| {
                                    let input: HtmlInputElement = e.target_unchecked_into();
                                    selected_noti_type.set(input.value());
                                }
                            })}
                        >
                            <option value="call">{"Call"}</option>
                            <option value="sms">{"SMS"}</option>
                        </select>
                    </div>
                    <div class="add-button-row">
                        <button onclick={Callback::from(move |_| add_waiting_check.emit(()))}>{"Add"}</button>
                        {render_save_indicator(&*add_save_state)}
                    </div>
                </div>
            </div>
            <ul class="filter-list">
            {
                (*checks_local).iter().map(|check| {
                    let content = check.content.clone();
                    let service_type_class = if check.service_type == "email" { "email" } else { "messaging" };
                    let service_type_display = if check.service_type == "email" { "Email" } else { "Messaging" };
                    let noti_type_display = check.noti_type.as_ref().map(|s| s.as_str()).unwrap_or("sms");
                    let noti_type_class = if noti_type_display == "call" { "call" } else { "sms" };
                    let extra = if noti_type_display == "sms" { sms_extra.clone() } else { call_extra.clone() };
                    html! {
                        <li>
                            <span>{&check.content}</span>
                            <span class={classes!("service-type-badge", service_type_class)}>{service_type_display}</span>
                            <span class={classes!("noti-type-badge", noti_type_class)}>{noti_type_display.to_uppercase()}</span>
                            {extra}
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
