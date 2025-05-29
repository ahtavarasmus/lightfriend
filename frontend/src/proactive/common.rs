use yew::prelude::*;
use gloo_net::http::Request;
use log::{info, Level};
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, js_sys, HtmlInputElement, KeyboardEvent, InputEvent, Event};
use wasm_bindgen::JsValue;
use crate::config;
use crate::pages::proactive::{PrioritySender, ImportancePriority, WaitingCheck};
use crate::profile::imap_general_checks::ImapGeneralChecks;
use crate::proactive::whatsapp_general_checks::WhatsappGeneralChecks;
use serde_json::json;

use crate::pages::proactive::format_date_for_input;

pub(crate) fn format_timestamp(ts: i32) -> String {
    let date = js_sys::Date::new(&js_sys::Number::from(ts as f64 * 1000.0));
    let opts = js_sys::Object::new();
    js_sys::Reflect::set(&opts, &JsValue::from_str("year"),  &JsValue::from_str("numeric")).unwrap();
    js_sys::Reflect::set(&opts, &JsValue::from_str("month"), &JsValue::from_str("long")).unwrap();
    js_sys::Reflect::set(&opts, &JsValue::from_str("day"),   &JsValue::from_str("numeric")).unwrap();
    date.to_locale_string("en-US", &opts).into()
}

#[derive(Properties, PartialEq, Clone)]
pub struct KeywordsProps {
    pub service_type: String,
    pub keywords: Vec<String>,
    pub on_change: Callback<Vec<String>>,
}

#[derive(Deserialize, Serialize, Debug)]
struct FilterSettings {
    keywords_active: bool,
    priority_senders_active: bool,
    waiting_checks_active: bool,
    general_importance_active: bool,
}

#[derive(Deserialize, Serialize)]
pub struct WaitingCheckRequest {
    content: String,
    due_date: i32,
    remove_when_found: bool,
}

#[function_component(KeywordsSection)]
pub fn keywords_section(props: &KeywordsProps) -> Html {
    let all_keywords = props.keywords.clone();
    let all_empty = all_keywords.is_empty();
    let new_kw = use_state(|| String::new());
    let keywords_local = use_state(|| props.keywords.clone());
    let is_active = use_state(|| false);
    let error_message = use_state(|| None::<String>);

    // Fetch initial active state
    {
        let is_active = is_active.clone();
        let error_message = error_message.clone();
        let service_type = props.service_type.clone();

        use_effect_with_deps(move |_| {
            if service_type == "whatsapp" || service_type == "imap" {
                if let Some(token) = window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|s| s.get_item("token").ok())
                    .flatten()
                {
                    let service_type = service_type.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        let endpoint = if service_type == "whatsapp" {
                            format!("{}/api/filters/whatsapp/settings", config::get_backend_url())
                        } else {
                            format!("{}/api/filters/imap/settings", config::get_backend_url())
                        };

                        match Request::get(&endpoint)
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await
                        {
                            Ok(response) => {
                                match response.json::<FilterSettings>().await {
                                    Ok(settings) => {
                                        info!("Filter settings received: {:#?}", settings);
                                        is_active.set(settings.keywords_active);
                                    },
                                    Err(e) => {
                                        error_message.set(Some(format!("Failed to parse filter settings: {}", e)));
                                    }
                                }
                            }
                            Err(e) => {
                                error_message.set(Some(format!("Failed to fetch filter settings: {}", e)));
                            }
                        }
                    });
                }
            }
            || ()
        }, (props.service_type.clone(),));
    }

    let toggle_active = {
        let is_active = is_active.clone();
        let error_message = error_message.clone();
        let service_type = props.service_type.clone();

        Callback::from(move |_| {
            if service_type == "whatsapp" || service_type == "imap" {
                if let Some(token) = window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|s| s.get_item("token").ok())
                    .flatten()
                {
                    let is_active = is_active.clone();
                    let error_message = error_message.clone();
                    let new_state = !*is_active;
                    let service_type = service_type.clone();


                    wasm_bindgen_futures::spawn_local(async move {
                        let endpoint = format!("{}/api/filters/{}/keywords/toggle", 
                            config::get_backend_url(),
                            service_type
                        );

                        match Request::post(&endpoint)
                        .header("Authorization", &format!("Bearer {}", token))
                        .json(&json!({ "active": new_state }))
                        .expect("Failed to create request")
                        .send()
                        .await
                        {
                            Ok(_) => {
                                is_active.set(new_state);
                                error_message.set(None);
                            }
                            Err(_) => {
                                error_message.set(Some("Failed to toggle keywords filter".to_string()));
                            }
                        }
                    });
                }
            }
        })
    };

    {
        let keywords_local = keywords_local.clone();
        let props_keywords = props.keywords.clone();
        use_effect_with_deps(
            move |_| { keywords_local.set(props_keywords); || () },
            props.keywords.clone(),
        );
    }

    let refresh_from_server = {
        let stype = props.service_type.clone();
        let kw_loc = keywords_local.clone();
        let on_par = props.on_change.clone();
        Callback::from(move |_| {
            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                let stype = stype.clone();
                let kw_loc = kw_loc.clone();
                let on_par = on_par.clone();
                spawn_local(async move {
                    if let Ok(resp) = Request::get(&format!(
                        "{}/api/filters/keywords/{}",
                        crate::config::get_backend_url(),
                        stype
                    ))
                    .header("Authorization", &format!("Bearer {}", token))
                    .send()
                    .await
                    {
                        if let Ok(list) = resp.json::<Vec<String>>().await {
                            kw_loc.set(list.clone());
                            on_par.emit(list);
                        }
                    }
                });
            }
        })
    };

    let add_keyword = {
        let stype = props.service_type.clone();
        let new_kw = new_kw.clone();
        let reload = refresh_from_server.clone();
        Callback::from(move |_| {
            let kw = (*new_kw).trim().to_string();
            if kw.is_empty() { return; }
            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                let stype = stype.clone();
                let reload = reload.clone();
                let new_kw = new_kw.clone();
                spawn_local(async move {
                    let _ = Request::post(&format!(
                            "{}/api/filters/keyword/{}",
                            crate::config::get_backend_url(), stype
                        ))
                        .header("Authorization", &format!("Bearer {}", token))
                        .json(&json!({ "keyword": kw, "service_type": stype }))
                        .unwrap()
                        .send()
                        .await;
                    new_kw.set(String::new());
                    reload.emit(());
                });
            }
        })
    };

    let del_keyword = {
        let stype = props.service_type.clone();
        let reload = refresh_from_server.clone();
        Callback::from(move |kw_to_del: String| {
            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                let stype = stype.clone();
                let reload = reload.clone();
                spawn_local(async move {
                    let _ = Request::delete(&format!(
                            "{}/api/filters/keyword/{}/{}",
                            crate::config::get_backend_url(), stype, kw_to_del
                        ))
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await;
                    reload.emit(());
                });
            }
        })
    };

    html! {
        <>
        <div class={classes!("filter-section", "main-filter",
                ((props.service_type == "whatsapp" || props.service_type == "imap") && !*is_active).then(|| "inactive"))}>
            <div class="filter-header">
                <h3>{"1. Keywords"}</h3>
                <div class="flow-step-status">
                    <div class={classes!("step-status", "keywords-status", (*is_active).then(|| "active"))}>
                        {if !*is_active { "SKIP" } else { "CHECK" }}
                    </div>
                    <div class="flow-description">
                        {
                            if all_empty {
                                "No keywords configured - this check is skipped"
                            } else {
                                "If message contains any keyword → NOTIFY"
                            }
                        }
                    </div>
                </div>
                    {
                            if props.service_type == "whatsapp" || props.service_type == "imap" {
                                html! {
                                    <div class="toggle-container">
                                        <span class="toggle-label">
                                            {if *is_active { "Active" } else { "Inactive" }}
                                        </span>
                                        <label class="switch">
                                            <input
                                                type="checkbox"
                                                checked={*is_active}
                                                onchange={toggle_active}
                                            />
                                            <span class="slider round"></span>
                                        </label>
                                    </div>
                                }
                            } else {
                                html! {}
                            }

                    }
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

            <div class="keyword-input">
                <input
                    type="text"
                    placeholder="Add new keyword"
                    value={(*new_kw).clone()}
                    oninput={Callback::from({
                        let new_kw = new_kw.clone();
                        move |e: InputEvent| {
                            let input: HtmlInputElement = e.target_unchecked_into();
                            new_kw.set(input.value());
                        }
                    })}
                    onkeypress={Callback::from({
                        let add_keyword = add_keyword.clone();
                        move |e: KeyboardEvent| {
                            if e.key() == "Enter" { add_keyword.emit(()); }
                        }
                    })}
                />
                <button
                    onclick={Callback::from({
                        let add_keyword = add_keyword.clone();
                        move |_| add_keyword.emit(())
                    })}
                >
                {"Add"}
                </button>
            </div>

            <ul class="keyword-list">
            {
                (*keywords_local).iter().map(|kw| {
                    let kw_clone = kw.clone();
                    html! {
                        <li class="keyword-item">
                            <span>{kw}</span>
                            <button class="delete-btn"
                                    onclick={Callback::from({
                                        let kw_clone = kw_clone.clone();
                                        let del_keyword = del_keyword.clone();
                                        move |_| del_keyword.emit(kw_clone.clone())
                                    })}>
                                {"×"}
                            </button>
                        </li>
                    }
                }).collect::<Html>()
            }
            </ul>
        </div>
        <style>
        {r#"
            .filter-section {
                position: relative;
                transition: opacity 0.3s ease;
            }

            .filter-section.inactive {
                opacity: 0.7;
            }

            .filter-section.main-filter.inactive::before {
                content: '';
                position: absolute;
                top: 0;
                left: 0;
                right: 0;
                bottom: 0;
                background: rgba(20, 20, 20, 0.1);
                pointer-events: none;
                z-index: 1;
                border-radius: 8px;
            }

            .filter-section.main-filter::after {
                position: absolute;
                top: 5%;
                right: 0;
                transform: translateY(-50%);
                font-size: 100px;
                text-shadow: 0 0 10px rgba(0, 0, 0, 0.3);
                z-index: 2;
            }

            .filter-section.main-filter.inactive::after {
                content: '⤵';
                transform: translateY(-50%) rotate(-45deg);
                color: #FFC107;
                text-shadow: 0 0 10px rgba(255, 193, 7, 0.3);
                animation: bounce 2s infinite;
            }

            .filter-section.main-filter:not(.inactive)::after {
                content: '➜';
                color: #4CAF50;
                text-shadow: 0 0 10px rgba(76, 175, 80, 0.3);
                animation: pulse 2s infinite;
            }

            @keyframes pulse {
                0%, 100% {
                    transform: translateY(-50%) scale(1);
                }
                50% {
                    transform: translateY(-50%) scale(1.1);
                }
            }

            @keyframes bounce {
                0%, 100% {
                    transform: translateY(-50%) rotate(-45deg) translateX(0);
                }
                50% {
                    transform: translateY(-50%) rotate(-45deg) translateX(5px);
                }
            }

            .filter-section.inactive .filter-header,
            .filter-section.inactive .filter-input,
            .filter-section.inactive .filter-list {
                filter: grayscale(20%);
            }
        
                .filter-header {
                    display: flex;
                    flex-direction: column;
                    gap: 0.5rem;
                    margin-bottom: 1rem;
                }

                .filter-header > div:first-child {
                    display: flex;
                    justify-content: space-between;
                    align-items: center;
                }

                .flow-step-status {
                    display: flex;
                    align-items: center;
                    gap: 1rem;
                    padding: 0.5rem;
                    background: rgba(30, 30, 30, 0.3);
                    border-radius: 8px;
                    margin-top: 0.5rem;
                }

                .flow-description {
                    color: rgba(255, 255, 255, 0.9);
                    font-size: 0.9rem;
                    flex: 1;
                }

                .step-status {
                    padding: 0.3rem 0.8rem;
                    border-radius: 12px;
                    font-size: 0.8rem;
                    font-weight: 500;
                    text-transform: uppercase;
                    letter-spacing: 0.5px;
                    min-width: 70px;
                    text-align: center;
                }

                .step-status.active {
                    background: rgba(76, 175, 80, 0.1);
                    color: #4CAF50;
                    border: 1px solid rgba(76, 175, 80, 0.2);
                }

                .step-status.keywords-status.active,
                .step-status.senders-status.active,
                .step-status.waiting-status.active {
                    background: rgba(76, 175, 80, 0.1);
                    color: #4CAF50;
                    border: 1px solid rgba(76, 175, 80, 0.2);
                }

                .step-status.keywords-status:not(.active),
                .step-status.senders-status:not(.active),
                .step-status.waiting-status:not(.active) {
                    background: rgba(255, 193, 7, 0.1);
                    color: #FFC107;
                    border: 1px solid rgba(255, 193, 7, 0.2);
                }

                .step-status.ai-status {
                    background: rgba(255, 193, 7, 0.1);
                    color: #FFC107;
                    border: 1px solid rgba(255, 193, 7, 0.2);
                }

                .threshold-value {
                    color: #7EB2FF;
                    font-weight: bold;
                    background: rgba(30, 144, 255, 0.1);
                    padding: 0.2rem 0.5rem;
                    border-radius: 4px;
                    border: 1px solid rgba(30, 144, 255, 0.2);
                }

                .toggle-container {
                    display: flex;
                    align-items: center;
                    gap: 0.5rem;
                }

                .toggle-label {
                    color: #7EB2FF;
                    font-size: 0.9rem;
                }

                /* Switch styles */
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
                    background-color: rgba(30, 30, 30, 0.7);
                    transition: .4s;
                    border: 1px solid rgba(30, 144, 255, 0.2);
                }

                .slider:before {
                    position: absolute;
                    content: "";
                    height: 18px;
                    width: 18px;
                    left: 3px;
                    bottom: 2px;
                    background-color: white;
                    transition: .4s;
                }

                input:checked + .slider {
                    background-color: #1E90FF;
                }

                input:checked + .slider:before {
                    transform: translateX(24px);
                }

                .slider.round {
                    border-radius: 24px;
                }

                .slider.round:before {
                    border-radius: 50%;
                }

                .error-message {
                    color: #ff4757;
                    background: rgba(255, 71, 87, 0.1);
                    border: 1px solid rgba(255, 71, 87, 0.2);
                    padding: 0.5rem;
                    border-radius: 4px;
                    margin-bottom: 1rem;
                    font-size: 0.9rem;
                }
        "#}
        </style>
            </>
    }
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WhatsAppRoom {
    pub room_id: String,
    pub display_name: String,
    pub last_activity: i64,
    pub last_activity_formatted: String,
}

#[derive(Properties, PartialEq, Clone)]
pub struct WaitingChecksProps {
    pub service_type: String,
    pub checks: Vec<WaitingCheck>,
    pub on_change: Callback<Vec<WaitingCheck>>,
}

#[function_component(WaitingChecksSection)]
pub fn waiting_checks_section(props: &WaitingChecksProps) -> Html {
    let all_checks = props.checks.clone();
    let all_empty = all_checks.is_empty();
    let new_check = use_state(|| String::new());
    let new_due_date = use_state(|| String::new());
    let remove_when_found = use_state(|| false);
    let checks_local = use_state(|| props.checks.clone());
    let is_active = use_state(|| false);
    let error_message = use_state(|| None::<String>);

    // Fetch initial active state
    {
        let is_active = is_active.clone();
        let error_message = error_message.clone();
        let service_type = props.service_type.clone();

        use_effect_with_deps(move |_| {
            if service_type == "whatsapp" || service_type == "imap" {
                if let Some(token) = window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|s| s.get_item("token").ok())
                    .flatten()
                {
                    let service_type = service_type.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        let endpoint = format!("{}/api/filters/{}/settings", 
                            config::get_backend_url(),
                            service_type
                        );

                        match Request::get(&endpoint)
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await
                        {
                            Ok(response) => {
                                if let Ok(settings) = response.json::<FilterSettings>().await {
                                    is_active.set(settings.waiting_checks_active);
                                }
                            }
                            Err(_) => {
                                error_message.set(Some("Failed to fetch filter settings".to_string()));
                            }
                        }
                    });
                }
            }
            || ()
        }, (props.service_type.clone(),));
    }

    let toggle_active = {
        let is_active = is_active.clone();
        let error_message = error_message.clone();
        let service_type = props.service_type.clone();

        Callback::from(move |_| {
            if service_type == "whatsapp" || service_type == "imap" {
                if let Some(token) = window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|s| s.get_item("token").ok())
                    .flatten()
                {
                    let is_active = is_active.clone();
                    let error_message = error_message.clone();
                    let new_state = !*is_active;
                    let service_type = service_type.clone();

                    wasm_bindgen_futures::spawn_local(async move {
                        let endpoint = format!("{}/api/filters/{}/waiting-checks/toggle", 
                            config::get_backend_url(),
                            service_type
                        );

                        match Request::post(&endpoint)
                            .header("Authorization", &format!("Bearer {}", token))
                            .json(&json!({ "active": new_state }))
                            .expect("Failed to create request")
                            .send()
                            .await
                        {
                            Ok(_) => {
                                is_active.set(new_state);
                                error_message.set(None);
                            }
                            Err(_) => {
                                error_message.set(Some("Failed to toggle waiting checks filter".to_string()));
                            }
                        }
                    });
                }
            }
        })
    };

    let refresh_from_server = {
        let stype = props.service_type.clone();
        let checks_local = checks_local.clone();
        let on_change = props.on_change.clone();
        Callback::from(move |_| {
            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                let stype = stype.clone();
                let checks_local = checks_local.clone();
                let on_change = on_change.clone();
                spawn_local(async move {
                    if let Ok(resp) = Request::get(&format!(
                        "{}/api/filters/waiting-checks/{}",
                        config::get_backend_url(),
                        stype
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

    let add_waiting_check = {
        let stype = props.service_type.clone();
        let new_check = new_check.clone();
        let new_due_date = new_due_date.clone();
        let remove_when_found = remove_when_found.clone();
        let refresh = refresh_from_server.clone();
        
        Callback::from(move |_| {
            let check = (*new_check).trim().to_string();
            if check.is_empty() { return; }
            
            let due_date = match (*new_due_date).trim().parse::<i32>() {
                Ok(date) => date,
                Err(_) => return,
            };

            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                let stype = stype.clone();
                let new_check = new_check.clone();
                let new_due_date = new_due_date.clone();
                let refresh = refresh.clone();
                let remove = *remove_when_found;

                spawn_local(async move {
                    let _ = Request::post(&format!(
                        "{}/api/filters/waiting-check/{}",
                        config::get_backend_url(),
                        stype
                    ))
                    .header("Authorization", &format!("Bearer {}", token))
                    .json(&WaitingCheckRequest {
                        content: check,
                        due_date,
                        remove_when_found: remove,
                    })
                    .unwrap()
                    .send()
                    .await;

                    new_check.set(String::new());
                    new_due_date.set(String::new());
                    refresh.emit(());
                });
            }
        })
    };

    let delete_waiting_check = {
        let stype = props.service_type.clone();
        let refresh = refresh_from_server.clone();
        
        Callback::from(move |content: String| {
            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                let stype = stype.clone();
                let refresh = refresh.clone();
                
                spawn_local(async move {
                    let _ = Request::delete(&format!(
                        "{}/api/filters/waiting-check/{}/{}",
                        config::get_backend_url(),
                        stype,
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
        <div class={classes!("filter-section", "main-filter", (!*is_active).then(|| "inactive"))}>
            <div class="filter-header">
                <h3>{"3. Waiting Checks"}</h3>
                <div class="flow-step-status">
                    <div class={classes!("step-status", "waiting-status", (*is_active).then(|| "active"))}>
                        {if !*is_active { "SKIP" } else { "CHECK" }}
                    </div>
                    <div class="flow-description">
                        {
                            if all_empty {
                                "No waiting checks configured - this check is skipped"
                            } else {
                                "If message contains any waiting check phrase → NOTIFY"
                            }
                        }
                    </div>
                </div>
                {
                    if props.service_type == "whatsapp" || props.service_type == "imap" {
                        html! {
                            <div class="toggle-container">
                                <span class="toggle-label">
                                    {if *is_active { "Active" } else { "Inactive" }}
                                </span>
                                <label class="switch">
                                    <input
                                        type="checkbox"
                                        checked={*is_active}
                                        onchange={toggle_active}
                                    />
                                    <span class="slider round"></span>
                                </label>
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }
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
                    <div class="date-label">
                        <span>{"Due Date"}</span>
                        <input
                            type="date"
                            value={(*new_due_date).clone()}
                            oninput={Callback::from({
                                let new_due_date = new_due_date.clone();
                                move |e: InputEvent| {
                                    let input: HtmlInputElement = e.target_unchecked_into();
                                    new_due_date.set(input.value());
                                }
                            })}
                        />
                    </div>
                    <label>
                        <input
                            type="checkbox"
                            checked={*remove_when_found}
                            onchange={Callback::from({
                                let remove_when_found = remove_when_found.clone();
                                move |e: Event| {
                                    let input: HtmlInputElement = e.target_unchecked_into();
                                    remove_when_found.set(input.checked());
                                }
                            })}
                        />
                        {"Remove when found"}
                    </label>
                </div>
                <button onclick={Callback::from(move |_| add_waiting_check.emit(()))}>{"Add"}</button>
            </div>

            <ul class="filter-list">
            {
                (*checks_local).iter().map(|check| {
                    let content = check.content.clone();
                    html! {
                        <li>
                            <span>{&check.content}</span>
                            {
                                if check.due_date > 0 {
                                    html! {
                                        <span class="due-date">
                                            {"Due: "}{format_date_for_input(check.due_date)}
                                        </span>
                                    }
                                } else {
                                    html! {}
                                }
                            }
                            {
                                if check.remove_when_found {
                                    html! {
                                        <span class="remove-when-found">
                                            {"(Remove when found)"}
                                        </span>
                                    }
                                } else {
                                    html! {}
                                }
                            }
                            <button class="delete-btn"
                                onclick={Callback::from({
                                    let content = content.clone();
                                    let delete_waiting_check = delete_waiting_check.clone();
                                    move |_| delete_waiting_check.emit(content.clone())
                                })}
                            >{"×"}</button>
                        </li>
                    }
                }).collect::<Html>()
            }
            </ul>
        </div>
    }
}

#[derive(Properties, PartialEq, Clone)]
pub struct PrioritySendersProps {
    pub service_type: String,
    pub senders: Vec<PrioritySender>,
    pub on_change: Callback<Vec<PrioritySender>>,
}

#[function_component(PrioritySendersSection)]
pub fn priority_senders_section(props: &PrioritySendersProps) -> Html {
    let all_senders = props.senders.clone();
    let all_empty = all_senders.is_empty();
    let new_sender = use_state(|| String::new());
    let senders_local = use_state(|| props.senders.clone());
    let search_results = use_state(|| Vec::<WhatsAppRoom>::new());
    let show_suggestions = use_state(|| false);
    let is_searching = use_state(|| false);
    let is_active = use_state(|| false);
    let error_message = use_state(|| None::<String>);

    // Fetch initial active state
    {
        let is_active = is_active.clone();
        let error_message = error_message.clone();
        let service_type = props.service_type.clone();

        use_effect_with_deps(move |_| {
            if service_type == "whatsapp" || service_type == "imap" {
                if let Some(token) = window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|s| s.get_item("token").ok())
                    .flatten()
                {
                    let service_type = service_type.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        let endpoint = format!("{}/api/filters/{}/settings", 
                            config::get_backend_url(),
                            service_type
                        );

                        match Request::get(&endpoint)
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await
                        {
                            Ok(response) => {
                                if let Ok(settings) = response.json::<FilterSettings>().await {
                                    is_active.set(settings.priority_senders_active);
                                }
                            }
                            Err(_) => {
                                error_message.set(Some("Failed to fetch filter settings".to_string()));
                            }
                        }
                    });
                }
            }
            || ()
        }, (props.service_type.clone(),));
    }

    let toggle_active = {
        let is_active = is_active.clone();
        let error_message = error_message.clone();
        let service_type = props.service_type.clone();

        Callback::from(move |_| {
            if service_type == "whatsapp" || service_type == "imap" {
                if let Some(token) = window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|s| s.get_item("token").ok())
                    .flatten()
                {
                    let is_active = is_active.clone();
                    let error_message = error_message.clone();
                    let new_state = !*is_active;
                    let service_type = service_type.clone();

                    wasm_bindgen_futures::spawn_local(async move {
                        let endpoint = format!("{}/api/filters/{}/priority-senders/toggle", 
                            config::get_backend_url(),
                            service_type
                        );

                        match Request::post(&endpoint)
                        .header("Authorization", &format!("Bearer {}", token))
                        .json(&json!({ "active": new_state }))
                        .expect("Failed to create request")
                        .send()
                        .await
                        {
                            Ok(_) => {
                                is_active.set(new_state);
                                error_message.set(None);
                            }
                            Err(_) => {
                                error_message.set(Some("Failed to toggle priority senders filter".to_string()));
                            }
                        }
                    });
                }
            }
        })
    };

    {
        let senders_local = senders_local.clone();
        let parent_copy = props.senders.clone();
        use_effect_with_deps(
            move |_| { senders_local.set(parent_copy); || () },
            props.senders.clone(),
        );
    }

    let refresh = {
        let stype = props.service_type.clone();
        let loc = senders_local.clone();
        let par = props.on_change.clone();
        Callback::from(move |_| {
            if let Some(tok) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                let stype = stype.clone();
                let loc = loc.clone();
                let par = par.clone();
                spawn_local(async move {
                    if let Ok(r) = Request::get(&format!(
                        "{}/api/filters/priority-senders/{}",
                        crate::config::get_backend_url(), stype
                    ))
                    .header("Authorization", &format!("Bearer {}", tok))
                    .send()
                    .await
                    {
                        if let Ok(list) = r.json::<Vec<PrioritySender>>().await {
                            loc.set(list.clone());
                            par.emit(list);
                        }
                    }
                });
            }
        })
    };

    let search_whatsapp_rooms = {
        let search_results = search_results.clone();
        let show_suggestions = show_suggestions.clone();
        let is_searching = is_searching.clone();
        Callback::from(move |search_term: String| {
            if search_term.trim().is_empty() {
                search_results.set(Vec::new());
                show_suggestions.set(false);
                return;
            }

            if let Some(tok) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                let search_results = search_results.clone();
                let show_suggestions = show_suggestions.clone();
                let is_searching = is_searching.clone();
                is_searching.set(true);
                
                spawn_local(async move {
                    match Request::get(&format!(
                        "{}/api/whatsapp/search-rooms?search={}",
                        crate::config::get_backend_url(),
                        urlencoding::encode(&search_term)
                    ))
                    .header("Authorization", &format!("Bearer {}", tok))
                    .send()
                    .await
                    {
                        Ok(response) => {
                            if let Ok(rooms) = response.json::<Vec<WhatsAppRoom>>().await {
                                search_results.set(rooms);
                                show_suggestions.set(true);
                            }
                        }
                        Err(e) => {
                            web_sys::console::log_1(&format!("Search error: {}", e).into());
                        }
                    }
                    is_searching.set(false);
                });
            }
        })
    };

    let add_sender = {
        let stype = props.service_type.clone();
        let new_s = new_sender.clone();
        let reload = refresh.clone();
        let show_suggestions = show_suggestions.clone();
        Callback::from(move |_| {
            let s = (*new_s).trim().to_string();
            if s.is_empty() { return; }
            if let Some(tok) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                let stype = stype.clone();
                let new_s = new_s.clone();
                let reload = reload.clone();
                let show_suggestions = show_suggestions.clone();
                spawn_local(async move {
                    let _ = Request::post(&format!(
                            "{}/api/filters/priority-sender/{}",
                            crate::config::get_backend_url(), stype
                        ))
                        .header("Authorization", &format!("Bearer {}", tok))
                        .json(&json!({ "sender": s, "service_type": stype }))
                        .unwrap()
                        .send()
                        .await;
                    new_s.set(String::new());
                    show_suggestions.set(false);
                    reload.emit(());
                });
            }
        })
    };

    let select_suggestion = {
        let new_sender = new_sender.clone();
        let show_suggestions = show_suggestions.clone();
        Callback::from(move |room_name: String| {
            // Extract clean name from display name (remove " (WA)" suffix)
            let clean_name = room_name
                .split(" (WA)")
                .next()
                .unwrap_or(&room_name)
                .trim()
                .to_string();
            new_sender.set(clean_name);
            show_suggestions.set(false);
        })
    };

    let del_sender = {
        let stype = props.service_type.clone();
        let reload = refresh.clone();
        Callback::from(move |who: String| {
            if let Some(tok) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                let stype = stype.clone();
                let reload = reload.clone();
                spawn_local(async move {
                    let _ = Request::delete(&format!(
                            "{}/api/filters/priority-sender/{}/{}",
                            crate::config::get_backend_url(), stype, who
                        ))
                        .header("Authorization", &format!("Bearer {}", tok))
                        .send()
                        .await;
                    reload.emit(());
                });
            }
        })
    };

    let hide_suggestions = {
        let show_suggestions = show_suggestions.clone();
        Callback::from(move |_| {
            show_suggestions.set(false);
        })
    };

    html! {
        <div class={classes!("filter-section", "main-filter", (!*is_active).then(|| "inactive"))}>
            <div class="filter-header">
                <h3>{"2. Priority Senders"}</h3>
                <div class="flow-step-status">
                    <div class={classes!("step-status", "senders-status", (*is_active).then(|| "active"))}>
                        {if !*is_active { "SKIP" } else { "CHECK" }}
                    </div>
                    <div class="flow-description">
                        {
                            if all_empty {
                                "No priority senders configured - this check is skipped"
                            } else {
                                "If message is from any priority sender → NOTIFY"
                            }
                        }
                    </div>
                </div>
                {
                    if props.service_type == "whatsapp" || props.service_type == "imap" {
                        html! {
                            <div class="toggle-container">
                                <span class="toggle-label">
                                    {if *is_active { "Active" } else { "Inactive" }}
                                </span>
                                <label class="switch">
                                    <input
                                        type="checkbox"
                                        checked={*is_active}
                                        onchange={toggle_active}
                                    />
                                    <span class="slider round"></span>
                                </label>
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }
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

            <div class="filter-input-container">
                <div class="filter-input">
                    {
                        if props.service_type == "whatsapp" {
                            html! {
                                <div class="whatsapp-search-container">
                                    <input
                                        type="text"
                                        placeholder="Search WhatsApp chats or add manually"
                                        value={(*new_sender).clone()}
                                        oninput={Callback::from({
                                            let new_sender = new_sender.clone();
                                            let search_whatsapp_rooms = search_whatsapp_rooms.clone();
                                            move |e: InputEvent| {
                                                let el: HtmlInputElement = e.target_unchecked_into();
                                                let value = el.value();
                                                new_sender.set(value.clone());
                                                search_whatsapp_rooms.emit(value);
                                            }
                                        })}
                                        onkeypress={Callback::from({
                                            let add_sender = add_sender.clone();
                                            move |e: KeyboardEvent| if e.key() == "Enter" { add_sender.emit(()) }
                                        })}
                                        onblur={Callback::from({
                                            let hide_suggestions = hide_suggestions.clone();
                                            move |_| {
                                                // Delay hiding to allow click on suggestions
                                                let hide_suggestions = hide_suggestions.clone();
                                                spawn_local(async move {
                                                    gloo_timers::future::TimeoutFuture::new(200).await;
                                                    hide_suggestions.emit(());
                                                });
                                            }
                                        })}
                                    />
                                    {
                                        if *is_searching {
                                            html! {
                                                <div class="search-loading">
                                                    <span>{"🔍 Searching..."}</span>
                                                </div>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }
                                    {
                                        if *show_suggestions && !(*search_results).is_empty() {
                                            html! {
                                                <div class="suggestions-dropdown">
                                                    {
                                                        (*search_results).iter().map(|room| {
                                                            let room_name = room.display_name.clone();
                                                            let clean_name = room_name
                                                                .split(" (WA)")
                                                                .next()
                                                                .unwrap_or(&room_name)
                                                                .trim()
                                                                .to_string();
                                                            html! {
                                                                <div 
                                                                    class="suggestion-item"
                                                                    onmousedown={Callback::from({
                                                                        let select_suggestion = select_suggestion.clone();
                                                                        let room_name = room_name.clone();
                                                                        move |_| select_suggestion.emit(room_name.clone())
                                                                    })}
                                                                >
                                                                    <div class="suggestion-name">{clean_name}</div>
                                                                    <div class="suggestion-activity">{&room.last_activity_formatted}</div>
                                                                </div>
                                                            }
                                                        }).collect::<Html>()
                                                    }
                                                </div>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }
                                </div>
                            }
                        } else {
                            html! {
                                <input
                                    type="text"
                                    placeholder="Add priority sender"
                                    value={(*new_sender).clone()}
                                    oninput={Callback::from({
                                        let new_sender = new_sender.clone();
                                        move |e: InputEvent| {
                                            let el: HtmlInputElement = e.target_unchecked_into();
                                            new_sender.set(el.value());
                                        }
                                    })}
                                    onkeypress={Callback::from({
                                        let add_sender = add_sender.clone();
                                        move |e: KeyboardEvent| if e.key() == "Enter" { add_sender.emit(()) }
                                    })}
                                />
                            }
                        }
                    }
                    <button
                        onclick={Callback::from({
                            let add_sender = add_sender.clone();
                            move |_| add_sender.emit(())
                        })}
                    >
                        {"Add"}
                    </button>
                </div>
            </div>

            <ul class="filter-list">
            {
                (*senders_local).iter().map(|ps| {
                    let who = ps.sender.clone();
                    html! {
                        <li class="filter-item">
                            <span>{&ps.sender}</span>
                            <button class="delete-btn"
                                onclick={Callback::from({
                                    let who = who.clone();
                                    let del_sender = del_sender.clone();
                                    move |_| del_sender.emit(who.clone())
                                })}
                            >{"×"}</button>
                        </li>
                    }
                }).collect::<Html>()
            }
            </ul>
        </div>
    }
}


#[derive(Properties, PartialEq, Clone)]
pub struct ImportanceProps {
    pub service_type: String,
    pub current_threshold: i32,
    pub on_change: Callback<i32>,
    pub keywords: Vec<String>,
    pub priority_senders: Vec<String>,
    pub waiting_checks: Vec<String>,
    pub threshold: i32,
    pub is_active: bool,
}

#[function_component(ImportancePrioritySection)]
pub fn importance_priority_section(props: &ImportanceProps) -> Html {
    let threshold = props.current_threshold;
    let value = use_state(|| props.current_threshold);
    let is_modified = use_state(|| false);
    let is_active = use_state(|| false);
    let error_message = use_state(|| None::<String>);

    // Fetch initial active state
    {
        let is_active = is_active.clone();
        let error_message = error_message.clone();
        let service_type = props.service_type.clone();

        use_effect_with_deps(move |_| {
            if service_type == "whatsapp" || service_type == "imap" {
                if let Some(token) = window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|s| s.get_item("token").ok())
                    .flatten()
                {
                    let service_type = service_type.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        let endpoint = format!("{}/api/filters/{}/settings", 
                            config::get_backend_url(),
                            service_type
                        );

                        match Request::get(&endpoint)
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await
                        {
                            Ok(response) => {
                                if let Ok(settings) = response.json::<FilterSettings>().await {
                                    is_active.set(settings.general_importance_active);
                                }
                            }
                            Err(_) => {
                                error_message.set(Some("Failed to fetch filter settings".to_string()));
                            }
                        }
                    });
                }
            }
            || ()
        }, (props.service_type.clone(),));
    }

    let toggle_active = {
        let is_active = is_active.clone();
        let error_message = error_message.clone();
        let service_type = props.service_type.clone();

        Callback::from(move |_| {
            if service_type == "whatsapp" || service_type == "imap" {
                if let Some(token) = window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|s| s.get_item("token").ok())
                    .flatten()
                {
                    let is_active = is_active.clone();
                    let error_message = error_message.clone();
                    let new_state = !*is_active;
                    let service_type = service_type.clone();

                    wasm_bindgen_futures::spawn_local(async move {
                        let endpoint = format!("{}/api/filters/{}/general-importance/toggle", 
                            config::get_backend_url(),
                            service_type
                        );

                        match Request::post(&endpoint)
                            .header("Authorization", &format!("Bearer {}", token))
                            .json(&json!({ "active": new_state }))
                            .expect("Failed to create request")
                            .send()
                            .await
                        {
                            Ok(_) => {
                                is_active.set(new_state);
                                error_message.set(None);
                            }
                            Err(_) => {
                                error_message.set(Some("Failed to toggle general importance filter".to_string()));
                            }
                        }
                    });
                }
            }
        })
    };

    {
        let value = value.clone();
        let is_modified = is_modified.clone();
        use_effect_with_deps(
            move |new_prop| {
                if *value != new_prop.current_threshold {
                    value.set(new_prop.current_threshold);
                    is_modified.set(false);
                }
                || ()
            },
            props.clone(),
        );
    }

    let save_threshold = {
        let stype = props.service_type.clone();
        let val = value.clone();
        let is_mod = is_modified.clone();
        let notify = props.on_change.clone();
        Callback::from(move |_| {
            let threshold = *val;
            if let Some(tok) = web_sys::window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                let stype = stype.clone(); let is_mod = is_mod.clone(); let notify = notify.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    let _ = gloo_net::http::Request::post(&format!(
                            "{}/api/filters/importance-priority/{}",
                            crate::config::get_backend_url(), stype
                        ))
                        .header("Authorization", &format!("Bearer {}", tok))
                        .json(&serde_json::json!({ "threshold": threshold, "service_type": stype }))
                        .unwrap()
                        .send()
                        .await;
                    is_mod.set(false);
                    notify.emit(threshold);
                });
            }
        })
    };

    html! {
        <div class={classes!("filter-section", "main-filter", (!*is_active).then(|| "inactive"))}>
            <div class="filter-header">
                <h3>{"4. AI Analysis"}</h3>
                <div class="flow-step-status">
                    <div class={classes!("step-status", "ai-status", (*is_active).then(|| "active"))}>
                        {if !*is_active { "SKIP" } else { "CHECK" }}
                    </div>
                    <div class="flow-description">
                        {"If none of the above matched, AI analyzes and scores the message (1-10). "}
                        {"If score ≥ "}<span class="threshold-value">{threshold}</span>{" → NOTIFY"}
                    </div>
                </div>
                {
                    if props.service_type == "whatsapp" || props.service_type == "imap" {
                        html! {
                            <div class="toggle-container">
                                <span class="toggle-label">
                                    {if *is_active { "Active" } else { "Inactive" }}
                                </span>
                                <label class="switch">
                                    <input
                                        type="checkbox"
                                        checked={*is_active}
                                        onchange={toggle_active}
                                    />
                                    <span class="slider round"></span>
                                </label>
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }
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
            <div class="filter-input">
                <div class="importance-input-group">
                    <input
                        type="number"
                        min="1" max="10"
                        value={value.to_string()}
                        oninput={Callback::from({
                            let value = value.clone();
                            let is_mod = is_modified.clone();
                            move |e: web_sys::InputEvent| {
                                let el: web_sys::HtmlInputElement = e.target_unchecked_into();
                                let new_val = el.value().parse::<i32>().unwrap_or(7);
                                if new_val != *value {
                                    value.set(new_val);
                                    is_mod.set(true);
                                }
                            }
                        })}
                    />
                    <span class="priority-label">{"out of 10"}</span>
                </div>

                if *is_modified {
                    <button class="save-btn"
                            onclick={Callback::from({
                                let save = save_threshold.clone();
                                move |_| save.emit(())
                            })}
                    >{"Save"}</button>
                }
            </div>

            {
                if props.service_type == "whatsapp" {
                    html! {
                        <WhatsappGeneralChecks 
                            on_update={Callback::from(|_| {})}
                            keywords={props.keywords.clone()}
                            priority_senders={props.priority_senders.clone()}
                            waiting_checks={props.waiting_checks.clone()}
                            threshold={threshold}
                        />
                    }
                } else if props.service_type == "imap" {
                    html! {
                        <ImapGeneralChecks 
                            on_update={Callback::from(|_| {})}
                            keywords={props.keywords.clone()}
                            priority_senders={props.priority_senders.clone()}
                            waiting_checks={props.waiting_checks.clone()}
                            threshold={threshold}
                        />
                    }
                } else {
                    html! {}
                }
                
            }
        </div>
    }
}

