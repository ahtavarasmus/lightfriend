
use yew::prelude::*;
use gloo_net::http::Request;
use log::{info, Level};
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, js_sys, HtmlInputElement, KeyboardEvent, InputEvent, Event};
use wasm_bindgen::JsValue;
use crate::config;
use serde_json::json;
use serde::{Deserialize, Serialize};
use gloo_timers::future::TimeoutFuture;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WhatsAppRoom {
    pub display_name: String,
    pub last_activity_formatted: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct MonitoredContact {
    pub sender: String,
    pub service_type: String,
}

#[derive(Deserialize, Serialize)]
pub struct MonitoredContactRequest {
    sender: String,
    service_type: String,
}

#[derive(Properties, PartialEq, Clone)]
pub struct MonitoredContactsProps {
    pub service_type: String,
    pub contacts: Vec<MonitoredContact>,
    pub on_change: Callback<Vec<MonitoredContact>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct PrioritySender {
    pub sender: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ImportancePriorityResponse {
    pub user_id: i32,
    pub threshold: i32,
    pub service_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ImportancePriority {
    pub threshold: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FilterSettings {
    pub keywords: Vec<String>,
    pub priority_senders: Vec<PrioritySender>,
    pub monitored_contacts: Vec<MonitoredContact>,
    pub importance_priority: Option<ImportancePriority>,
}

#[function_component(MonitoredContactsSection)]
pub fn monitored_contacts_section(props: &MonitoredContactsProps) -> Html {
    let all_contacts = props.contacts.clone();
    let all_empty = all_contacts.is_empty();
    let new_contact = use_state(|| String::new());
    let selected_service = use_state(|| props.service_type.clone());
    let contacts_local = use_state(|| props.contacts.clone());
    let error_message = use_state(|| None::<String>);
    let show_info = use_state(|| false);
    let search_results = use_state(|| Vec::<WhatsAppRoom>::new());
    let show_suggestions = use_state(|| false);
    let is_searching = use_state(|| false);

    let hide_suggestions = {
        let show_suggestions = show_suggestions.clone();
        Callback::from(move |_| {
            show_suggestions.set(false);
        })
    };

    let select_suggestion = {
        let new_contact = new_contact.clone();
        let show_suggestions = show_suggestions.clone();
        Callback::from(move |room_name: String| {
            new_contact.set(room_name);
            show_suggestions.set(false);
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

    let refresh_from_server = {
        let contacts_local = contacts_local.clone();
        let on_change = props.on_change.clone();
        Callback::from(move |_| {
            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                let contacts_local = contacts_local.clone();
                let on_change = on_change.clone();
                spawn_local(async move {
                    if let Ok(resp) = Request::get(&format!(
                        "{}/api/filters/monitored-contacts",
                        config::get_backend_url(),
                    ))
                    .header("Authorization", &format!("Bearer {}", token))
                    .send()
                    .await
                    {
                        info!("Response status: {}", resp.status());
                        if let Ok(list) = resp.json::<Vec<MonitoredContact>>().await {
                            info!("Received contacts: {:?}", list);
                            contacts_local.set(list.clone());
                            on_change.emit(list);
                        } else {
                            info!("Failed to parse contacts response as JSON");
                        }
                    } else {
                        info!("Failed to fetch contacts");
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


    let add_monitored_contact = {
        let new_contact = new_contact.clone();
        let refresh = refresh_from_server.clone();
        let selected_service = selected_service.clone();
        let contacts_local = contacts_local.clone();
        let error_message = error_message.clone();
        
        Callback::from(move |_| {
            let identifier = (*new_contact).trim().to_string();
            if identifier.is_empty() { return; }
            let service_type = (*selected_service).clone();
            let error_message = error_message.clone();

            // Check if we've reached the maximum number of monitored contacts
            if (*contacts_local).len() >= 10 {
                error_message.set(Some("Maximum of 10 monitored contacts allowed".to_string()));
                return;
            }

            // Validate email format for IMAP service type
            if service_type == "imap" && !identifier.contains('@') {
                error_message.set(Some("Please enter a valid email address".to_string()));
                return;
            }
            
            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                let new_contact = new_contact.clone();
                let refresh = refresh.clone();
                let service_type = service_type.clone();

                spawn_local(async move {
                    let _ = Request::post(&format!(
                        "{}/api/filters/monitored-contact/{}",
                        config::get_backend_url(),
                        service_type
                    ))
                    .header("Authorization", &format!("Bearer {}", token))
                    .json(&MonitoredContactRequest {
                        sender: identifier,
                        service_type: service_type.clone(),
                    })
                    .unwrap()
                    .send()
                    .await;

                    new_contact.set(String::new());
                    error_message.set(None);
                    refresh.emit(());
                });
            }
        })
    };

    let delete_monitored_contact = {
        let refresh = refresh_from_server.clone();
        
        Callback::from(move |(identifier, service_type): (String, String)| {
            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                let refresh = refresh.clone();
                
                spawn_local(async move {
                    let _ = Request::delete(&format!(
                        "{}/api/filters/monitored-contact/{}/{}",
                        config::get_backend_url(),
                        service_type,
                        identifier
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
                        color: white;
                        text-decoration: none;
                        font-size: 1.2rem;
                        font-weight: 600;
                        background: linear-gradient(45deg, #fff, #34D399);
                        -webkit-background-clip: text;
                        -webkit-text-fill-color: transparent;
                        transition: opacity 0.3s ease;
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

                        .waiting-check-fields button {
                            width: 100%;
                        }
                    }

                    .input-group {
                        display: flex;
                        gap: 0.5rem;
                        width: 100%;
                    }

                    @media (max-width: 480px) {
                        .input-group {
                            flex-direction: column;
                        }

                        .service-select {
                            width: 100%;
                        }
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
                        overflow: hidden;
                    }

                    @media (max-width: 480px) {
                        .filter-list li {
                            flex-direction: column;
                            align-items: flex-start;
                            gap: 0.5rem;
                        }

                        .filter-list li .delete-btn {
                            position: absolute;
                            top: 0.5rem;
                            right: 0.5rem;
                        }

                        .filter-list li {
                            position: relative;
                            padding: 2rem 1rem 1rem;
                        }

                        .filter-list li span:first-child {
                            word-break: break-all;
                        }
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

                    .input-with-suggestions {
                        position: relative;
                        flex: 1;
                    }

                    .suggestions-dropdown {
                        position: absolute;
                        top: 100%;
                        left: 0;
                        right: 0;
                        background: rgba(30, 30, 30, 0.95);
                        border: 1px solid rgba(245, 158, 11, 0.2);
                        border-radius: 8px;
                        margin-top: 4px;
                        max-height: 300px;
                        overflow-y: auto;
                        z-index: 1000;
                        backdrop-filter: blur(10px);
                    }

                    .suggestion-item {
                        padding: 0.75rem 1rem;
                        cursor: pointer;
                        transition: all 0.2s ease;
                        display: flex;
                        justify-content: space-between;
                        align-items: center;
                        gap: 1rem;
                    }

                    .suggestion-item:hover {
                        background: rgba(245, 158, 11, 0.1);
                    }

                    .suggestion-name {
                        color: #fff;
                        font-size: 0.9rem;
                    }

                    .suggestion-activity {
                        color: #999;
                        font-size: 0.8rem;
                    }

                    .search-loading {
                        position: absolute;
                        top: 50%;
                        right: 1rem;
                        transform: translateY(-50%);
                        color: #999;
                        font-size: 0.9rem;
                        display: flex;
                        align-items: center;
                        gap: 0.5rem;
                    }
                "#}
            </style>
            <div class="filter-header">
                <div class="filter-title">
                    <i class="fas fa-user-check" style="color: #4ECDC4;"></i>
                    <h3>{"Monitored Contacts"}</h3>
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
                    {
                        "Get notified about messages from your monitored contacts. Note: Check info for notification quotas."
                    }
                </div>
                <div class="info-section" style={if *show_info { "display: block" } else { "display: none" }}>
                    <h4>{"How It Works"}</h4>
                    <div class="info-subsection">
                        <ul>
                            <li>{"Lightfriend will notify you about all messages from your monitored contacts"}</li>
                            <li>{"For WhatsApp, enter the contact's name or phone number"}</li>
                            <li>{"For Email, enter the contact's email address"}</li>
                            <li>{"Note: Priority sender notifications will use Messages with rate 1 Message = 3 notifications. If Messages are depleted for the month you can continue with overage credits."}</li>
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
                            <option value="imap">{"Email"}</option>
                            <option value="whatsapp">{"WhatsApp"}</option>
                        </select>
                        <div class="input-with-suggestions">
                            <input
                                type={if *selected_service == "whatsapp" { "text" } else { "email" }}
                                autocomplete={if *selected_service == "whatsapp" { "off" } else { "email" }}
                                placeholder={if *selected_service == "whatsapp" { "Search WhatsApp chats or add manually" } else { "Enter email address" }}
                                value={(*new_contact).clone()}
                                oninput={Callback::from({
                                    let new_contact = new_contact.clone();
                                    let search_whatsapp_rooms = search_whatsapp_rooms.clone();
                                    let selected_service = selected_service.clone();
                                    move |e: InputEvent| {
                                        let input: HtmlInputElement = e.target_unchecked_into();
                                        let value = input.value();
                                        new_contact.set(value.clone());
                                        if *selected_service == "whatsapp" {
                                            search_whatsapp_rooms.emit(value);
                                        }
                                    }
                                })}
                                onkeypress={Callback::from({
                                    let add_monitored_contact = add_monitored_contact.clone();
                                    move |e: KeyboardEvent| {
                                        if e.key() == "Enter" {
                                            add_monitored_contact.emit(());
                                        }
                                    }
                                })}
                                onblur={Callback::from({
                                    let hide_suggestions = hide_suggestions.clone();
                                    move |_| {
                                        // Delay hiding to allow click on suggestions
                                        let hide_suggestions = hide_suggestions.clone();
                                        spawn_local(async move {
                                            TimeoutFuture::new(200).await;
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
                </div>
                <button onclick={Callback::from(move |_| add_monitored_contact.emit(()))}>{"Add"}</button>
            </div>

            <ul class="filter-list">
            {
                (*contacts_local).iter().map(|contact| {
                    let identifier = contact.sender.clone();
                    let service_type_class = if contact.service_type == "imap" { "email" } else { "messaging" };
                    let service_type_display = if contact.service_type == "imap" { "Email" } else { "WhatsApp" };
                    html! {
                        <li>
                            <span>{identifier.clone()}</span>
                            <span class={classes!("service-type-badge", service_type_class)}>{service_type_display}</span>
                            <button class="delete-btn"
                                onclick={Callback::from({
                                    let identifier = identifier.clone();
                                    let service_type = contact.service_type.clone();
                                    let delete_monitored_contact = delete_monitored_contact.clone();
                                    move |_| delete_monitored_contact.emit((identifier.clone(), service_type.clone()))
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
