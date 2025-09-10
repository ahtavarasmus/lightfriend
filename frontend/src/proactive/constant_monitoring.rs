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
pub struct Room {
    pub display_name: String,
    pub last_activity_formatted: String,
}
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct MonitoredContact {
    pub sender: String,
    pub service_type: String,
    pub noti_type: Option<String>,
    pub noti_mode: Option<String>,
}
#[derive(Deserialize, Serialize)]
pub struct MonitoredContactRequest {
    sender: String,
    service_type: String,
    noti_type: Option<String>,
    noti_mode: String,
}
#[derive(Properties, PartialEq, Clone)]
pub struct MonitoredContactsProps {
    pub service_type: String,
    pub contacts: Vec<MonitoredContact>,
    pub on_change: Callback<Vec<MonitoredContact>>,
    pub phone_number: String,
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
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MonitoredContactsResponse {
    contacts: Vec<MonitoredContact>,
    average_per_day: f32,
    estimated_monthly_price: f32,
}
#[function_component(MonitoredContactsSection)]
pub fn monitored_contacts_section(props: &MonitoredContactsProps) -> Html {
    let all_contacts = props.contacts.clone();
    let all_empty = all_contacts.is_empty();
    let new_contact = use_state(|| String::new());
    let selected_service = use_state(|| "".to_string());
    let selected_noti_type = use_state(|| "sms".to_string());
    let is_all_mode = use_state(|| false);
    let contacts_local = use_state(|| props.contacts.clone());
    let error_message = use_state(|| None::<String>);
    let show_info = use_state(|| false);
    let search_results = use_state(|| Vec::<Room>::new());
    let show_suggestions = use_state(|| false);
    let is_searching = use_state(|| false);
    let average_per_day = use_state(|| 0.0);
    let estimated_monthly_price = use_state(|| 0.0);
    let current_tab = use_state(|| "whatsapp".to_string());
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
    let search_rooms = {
        let search_results = search_results.clone();
        let show_suggestions = show_suggestions.clone();
        let is_searching = is_searching.clone();
        let selected_service = selected_service.clone();
        Callback::from(move |search_term: String| {
            if search_term.trim().is_empty() {
                search_results.set(Vec::new());
                show_suggestions.set(false);
                return;
            }
            let service = (*selected_service).clone();
            if service == "imap" {
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
                        "{}/api/{}/search-rooms?search={}",
                        crate::config::get_backend_url(),
                        service,
                        urlencoding::encode(&search_term)
                    ))
                    .header("Authorization", &format!("Bearer {}", tok))
                    .send()
                    .await
                    {
                        Ok(response) => {
                            if let Ok(rooms) = response.json::<Vec<Room>>().await {
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
        let average_per_day = average_per_day.clone();
        let estimated_monthly_price = estimated_monthly_price.clone();
        let on_change = props.on_change.clone();
        Callback::from(move |_| {
            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                let contacts_local = contacts_local.clone();
                let average_per_day = average_per_day.clone();
                let estimated_monthly_price = estimated_monthly_price.clone();
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
                        if let Ok(response) = resp.json::<MonitoredContactsResponse>().await {
                            info!("Received contacts: {:?}", response.contacts);
                            contacts_local.set(response.contacts.clone());
                            average_per_day.set(response.average_per_day);
                            estimated_monthly_price.set(response.estimated_monthly_price);
                            on_change.emit(response.contacts);
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
        let selected_noti_type = selected_noti_type.clone();
        let is_all_mode = is_all_mode.clone();
        let contacts_local = contacts_local.clone();
        let error_message = error_message.clone();
        let current_tab = current_tab.clone();
        Callback::from(move |_| {
            let identifier = (*new_contact).trim().to_string();
            if identifier.is_empty() { return; }
            let service_type = (*selected_service).clone();
            if service_type.is_empty() { return; }
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
                let noti_mode = if *is_all_mode { "all".to_string() } else { "focus".to_string() };
                let noti_type = if *is_all_mode {
                    Some((*selected_noti_type).clone())
                } else {
                    Some("sms".to_string())
                };
                let current_tab = current_tab.clone();
                let error_message = error_message.clone();
                let is_all_mode = is_all_mode.clone();
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
                        noti_type,
                        noti_mode: noti_mode,
                    })
                    .unwrap()
                    .send()
                    .await;
                    new_contact.set(String::new());
                    is_all_mode.set(false);
                    error_message.set(None);
                    refresh.emit(());
                    current_tab.set(service_type);
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
        "US" => "",
        _ => "€",
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
                        flex-wrap: wrap;
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
                    .checkbox-label {
                        display: flex;
                        align-items: center;
                        gap: 0.5rem;
                        color: #fff;
                        font-size: 0.9rem;
                        cursor: pointer;
                    }
                    .checkbox-label input[type="checkbox"] {
                        width: 16px;
                        height: 16px;
                        cursor: pointer;
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
                        color: #D3D3D3;
                        border: 1px solid rgba(245, 158, 11, 0.2);
                    }
                    .service-type-badge.whatsapp {
                        color: #25D366;
                        border: 1px solid rgba(236, 72, 153, 0.2);
                    }
                    .service-type-badge.telegram {
                        color: #0088cc;
                        border: 1px solid rgba(0, 136, 204, 0.2);
                    }
                    .service-type-badge.signal {
                        color: #3A76F0;
                        border: 1px solid rgba(58, 118, 240, 0.2);
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
                    .mode-badge {
                        padding: 0.25rem 0.75rem;
                        border-radius: 8px;
                        font-size: 0.8rem;
                        background: rgba(0, 0, 0, 0.2);
                    }
                    .mode-badge.all {
                        color: #34D399;
                        border: 1px solid rgba(52, 211, 153, 0.2);
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
                    .notification-cost {
                        color: #999;
                        font-size: 0.8rem;
                        margin-top: 1rem;
                    }
                    .tabs {
                        display: flex;
                        gap: 0.5rem;
                        margin-bottom: 1rem;
                    }
                    .tab-button {
                        padding: 0.5rem 1rem;
                        border-radius: 8px;
                        background: rgba(0,0,0,0.2);
                        border: 1px solid rgba(245,158,11,0.1);
                        color: #fff;
                        cursor: pointer;
                        transition: all 0.3s ease;
                    }
                    .tab-button.active {
                        border: none;
                    }
                    .tab-button:hover {
                        opacity: 0.9;
                    }
                "#}
            </style>
            <div class="filter-header">
                <div class="filter-title">
                    <i class="fas fa-user-check" style="color: #4ECDC4;"></i>
                    <h3>{"Special Contacts"}</h3>
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
                    {"Define important contacts and lightfriend will keep an extra eye out for them."}
                </div>
                    /*
                <div class="notification-cost">
                    {if *estimated_monthly_price == 0.0 {
                        "Not enough data to estimate cost yet".to_string()
                    } else if country == "US" {
                        format!(
                            "Estimated monthly cost: {:.2} Messages (based on {:.1} notifications per day)",
                            *estimated_monthly_price / 0.15, *average_per_day
                        )
                    } else {
                        format!(
                            "Estimated monthly cost: {}{:.2} (based on {:.1} notifications per day)",
                            currency, *estimated_monthly_price, *average_per_day
                        )
                    }}
                </div>
                    */
                <div class="info-section" style={if *show_info { "display: block" } else { "display: none" }}>
                    <h4>{"How It Works"}</h4>
                    <div class="info-subsection">
                        <ul>
                            <li>{"Special contacts will be prioritized in digest messages and can be set to have special behaviour with critical notitications"}</li>
                            <li>{"To get notified about every new message from certain contact, check the checkbox and choose notification method"}</li>
                            <li>{"For WhatsApp, Telegram, Signal, enter the contact's name or phone number"}</li>
                            <li>{"For Email, enter the contact's email address"}</li>
                        </ul>
                    </div>
                    <h4>{"Notification Modes"}</h4>
                    <div class="info-subsection">
                        <p>{"By default, contacts are in \"focus\" mode, which prioritizes them in the family group for digest messages and critical notifications. Check the \"Notify about all messages\" box when adding a contact to receive every message from them."}</p>
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
                                let new_contact = new_contact.clone();
                                move |e: Event| {
                                    let input: HtmlInputElement = e.target_unchecked_into();
                                    let val = input.value();
                                    selected_service.set(val.clone());
                                    if val.is_empty() {
                                        new_contact.set(String::new());
                                    }
                                }
                            })}
                        >
                            <option value="whatsapp">{"WhatsApp"}</option>
                            <option value="telegram">{"Telegram"}</option>
                            <option value="signal">{"Signal"}</option>
                            <option value="imap">{"Email"}</option>
                            <option value="">{"Select Service"}</option>
                        </select>
                        {
                            if !(*selected_service).is_empty() {
                                html! {
                                    <div class="input-with-suggestions">
                                        <input
                                            type={if *selected_service == "imap" { "email" } else { "text" }}
                                            autocomplete={if *selected_service == "imap" { "email" } else { "off" }}
                                            placeholder={match (*selected_service).as_str() {
                                                "imap" => "Enter email address",
                                                "whatsapp" => "Search WhatsApp chats or add manually",
                                                "telegram" => "Search Telegram chats or add manually",
                                                "signal" => "Search Signal chats or add manually",
                                                _ => "Select app first from the left",
                                            }}
                                            value={(*new_contact).clone()}
                                            oninput={Callback::from({
                                                let new_contact = new_contact.clone();
                                                let search_rooms = search_rooms.clone();
                                                let selected_service = selected_service.clone();
                                                move |e: InputEvent| {
                                                    let input: HtmlInputElement = e.target_unchecked_into();
                                                    let value = input.value();
                                                    new_contact.set(value.clone());
                                                    if *selected_service != "imap" {
                                                        search_rooms.emit(value);
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
                                                                let clean_name = if *selected_service == "whatsapp" {
                                                                    room_name.split(" (WA)").next().unwrap_or(&room_name).trim().to_string()
                                                                } else if *selected_service == "telegram" {
                                                                    room_name.split(" (TG)").next().unwrap_or(&room_name).trim().to_string()
                                                                } else {
                                                                    room_name.clone()
                                                                };
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
                                html! {}
                            }
                        }
                        {
                            if !(*selected_service).is_empty() {
                                html! {
                                    <label class="checkbox-label">
                                        <input
                                            type="checkbox"
                                            checked={*is_all_mode}
                                            onchange={Callback::from({
                                                let is_all_mode = is_all_mode.clone();
                                                move |e: Event| {
                                                    let input: HtmlInputElement = e.target_unchecked_into();
                                                    is_all_mode.set(input.checked());
                                                }
                                            })}
                                        />
                                        {"Notify about all messages from this sender"}
                                    </label>
                                }
                            } else {
                                html! {}
                            }
                        }
                        {
                            if !(*selected_service).is_empty() && *is_all_mode {
                                html! {
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
                                }
                            } else {
                                html! {}
                            }
                        }
                    </div>
                    {
                        if !(*selected_service).is_empty() {
                            html! { <button onclick={Callback::from(move |_| add_monitored_contact.emit(()))}>{"Add"}</button> }
                        } else {
                            html! {}
                        }
                    }
                </div>
            </div>
            <div class="tabs">
            {
                vec!["whatsapp", "telegram", "signal", "imap"].iter().map(|&tab| {
                    let display = match tab {
                        "whatsapp" => "WhatsApp",
                        "telegram" => "Telegram",
                        "signal" => "Signal",
                        "imap" => "Email",
                        _ => ""
                    };
                    html! {
                        <button
                            class={classes!("tab-button", if *current_tab == tab { "active" } else { "" })}
                            onclick={Callback::from({
                                let current_tab = current_tab.clone();
                                let tab = tab.to_string();
                                move |_| current_tab.set(tab.clone())
                            })}
                        >
                            {display}
                        </button>
                    }
                }).collect::<Html>()
            }
            </div>
            <ul class="filter-list">
            {
                (*contacts_local).iter().filter(|contact| contact.service_type == *current_tab).map(|contact| {
                    let identifier = contact.sender.clone();
                    let service_type_class = match contact.service_type.as_str() {
                        "imap" => "email",
                        "telegram" => "telegram",
                        "signal" => "signal",
                        "whatsapp" => "whatsapp",
                        _ => "messaging",
                    };
                    let service_type_display = match contact.service_type.as_str() {
                        "imap" => " Email",
                        "whatsapp" => " WhatsApp",
                        "telegram" => " Telegram",
                        "signal" => " Signal",
                        _ => "Unknown",
                    };
                    let icon_class = match contact.service_type.as_str() {
                        "imap" => "fas fa-envelope",
                        "whatsapp" => "fab fa-whatsapp",
                        "telegram" => "fab fa-telegram",
                        "signal" => "fab fa-signal-messenger",
                        _ => "",
                    };
                    let noti_type_display = contact.noti_type.as_ref().map(|s| s.as_str()).unwrap_or("sms");
                    let noti_type_class = match noti_type_display {
                        "call" => "call",
                        _ => "sms",
                    };
                    let noti_mode_str = contact.noti_mode.as_ref().cloned().unwrap_or("focus".to_string());
                    html! {
                        <li>
                            <span>{identifier.clone()}</span>
                            <span class={classes!("service-type-badge", service_type_class)}>
                            if !icon_class.is_empty() {
                                <i class={icon_class}></i>
                            }
                            {service_type_display}
                            </span>
                            {
                                if noti_mode_str == "all" {
                                    html! { <span class={classes!("noti-type-badge", noti_type_class)}>{noti_type_display.to_uppercase()}</span> }
                                } else {
                                    html! {}
                                }
                            }
                            {
                                if noti_mode_str == "all" {
                                    html! { <span class={classes!("mode-badge", "all")}>{"ALL"}</span> }
                                } else {
                                    html! {}
                                }
                            }
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
        </>
    }
}
