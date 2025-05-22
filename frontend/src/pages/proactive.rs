use yew::prelude::*;
use yew::{Properties, function_component};
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use web_sys::{window, HtmlInputElement, KeyboardEvent, InputEvent};
use serde_json::Number;
use chrono::Date;
use wasm_bindgen::JsValue;
use crate::profile::imap_general_checks::ImapGeneralChecks;

use crate::proactive::{
    email::{FilterActivityLog, WaitingChecksSection},
    common::{ImportancePrioritySection, PrioritySendersSection, KeywordsSection},
};

trait PadStart {
    fn pad_start_with_character(&self, width: usize, padding: char) -> String;
}

impl PadStart for String {
    fn pad_start_with_character(&self, width: usize, padding: char) -> String {
        if self.len() >= width {
            return self.clone();
        }
        let padding_width = width - self.len();
        let padding_string: String = std::iter::repeat(padding).take(padding_width).collect();
        format!("{}{}", padding_string, self)
    }
}
use wasm_bindgen::JsCast;
use crate::config;

use serde_json::json;
use wasm_bindgen_futures::spawn_local;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConnectedService {
    pub service_type: String,
    pub identifier: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct WaitingCheck {
    pub content: String,
    pub due_date: i32,
    pub remove_when_found: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct PrioritySender {
    pub sender: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ImportancePriority {
    pub threshold: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EmailJudgmentResponse {
    pub id: i32,
    pub email_timestamp: i32,
    pub processed_at: i32,
    pub should_notify: bool,
    pub score: i32,
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ImportancePriorityResponse {
    pub user_id: i32,
    pub threshold: i32,
    pub service_type: String,
}

impl ImportancePriority {
    pub fn new(threshold: i32) -> Self {
        Self { threshold }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FilterSettings {
    pub keywords: Vec<String>,
    pub priority_senders: Vec<PrioritySender>,
    pub waiting_checks: Vec<WaitingCheck>,
    pub importance_priority: Option<ImportancePriority>,
}

#[derive(Debug, Clone)]
pub struct ServiceState {
    pub service_type: String,
    pub identifier: String,
    pub filter_settings: Option<FilterSettings>,
}


#[derive(Properties, PartialEq)]
pub struct Props {
    pub user_id: i32,
}

// Helper functions
fn get_service_display_name(service_type: &str) -> String {
    match service_type {
        "imap" => "Email",
        "calendar" => "Calendar",
        "whatsapp" => "WhatsApp",
        _ => service_type,
    }.to_string()
}
use web_sys::js_sys;


fn format_date_for_input(timestamp: i32) -> String {
    if timestamp == 0 {
        return String::new();
    }
    let date = js_sys::Date::new(&js_sys::Number::from(timestamp as f64 * 1000.0));
    let year = date.get_full_year();
    let month = (date.get_month() + 1).to_string().pad_start_with_character(2, '0');
    let day = date.get_date().to_string().pad_start_with_character(2, '0');
    format!("{}-{}-{}", year, month, day)
}

fn parse_date_to_timestamp(date_str: &str) -> Result<i32, &'static str> {
    if date_str.is_empty() {
        return Ok(0);
    }
    let date = js_sys::Date::new(&JsValue::from_str(date_str));
    if date.get_time().is_nan() {
        return Err("Invalid date");
    }
    Ok((date.get_time() / 1000.0) as i32)
}


#[function_component(ConnectedServices)]
pub fn connected_services(props: &Props) -> Html {
    let services_state = use_state(|| Vec::<ServiceState>::new());
    let error = use_state(|| None::<String>);
    let selected_service = use_state(|| None::<String>);
    let is_proactive = use_state(|| false);
    let filter_settings = use_state(|| None::<FilterSettings>);
    let is_calendar_proactive = use_state(|| false);
    let is_whatsapp_proactive = use_state(|| false);

    // Function to fetch keywords for a specific service
    let fetch_keywords = {
        let services_state = services_state.clone();
        let selected_service = selected_service.clone();
        let error = error.clone();
        
        Callback::from(move |service_type: String| {
            let services_state = services_state.clone();
            let error = error.clone();
            
            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|storage| storage.get_item("token").ok())
                .flatten()
            {
                spawn_local(async move {
                    if let Ok(keywords_response) = Request::get(&format!("{}/api/filters/keywords/{}", config::get_backend_url(), service_type))
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await
                    {
                        if let Ok(keywords) = keywords_response.json::<Vec<String>>().await {
                            let mut updated_services = (*services_state).clone();
                            if let Some(service) = updated_services.iter_mut().find(|s| s.service_type == service_type) {
                                if let Some(settings) = &mut service.filter_settings {
                                    settings.keywords = keywords;
                                }
                            }
                            services_state.set(updated_services);
                        } else {
                            error.set(Some("Failed to parse keywords".to_string()));
                        }
                    } else {
                        error.set(Some("Failed to fetch keywords".to_string()));
                    }
                });
            }
        })
    };

    // Fetch IMAP proactive state on mount
    {
        let is_proactive = is_proactive.clone();
        let error = error.clone();

        use_effect_with_deps(move |_| {
            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|storage| storage.get_item("token").ok())
                .flatten()
            {
                spawn_local(async move {
                    if let Ok(response) = Request::get(&format!("{}/api/profile/imap-proactive", config::get_backend_url()))
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await
                    {
                        if let Ok(data) = response.json::<serde_json::Value>().await {
                            if let Some(proactive) = data.get("proactive").and_then(|v| v.as_bool()) {
                                is_proactive.set(proactive);
                            }
                        } else {
                            error.set(Some("Failed to parse proactive state".to_string()));
                        }
                    } else {
                        error.set(Some("Failed to fetch proactive state".to_string()));
                    }
                });
            }
            || ()
        }, ());
    }

    // Fetch CALENDAR proactive state on mount
    {
        let is_calendar_proactive = is_calendar_proactive.clone();
        let error = error.clone();

        use_effect_with_deps(move |_| {
            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                spawn_local(async move {
                    if let Ok(resp) = Request::get(&format!(
                        "{}/api/profile/calendar-proactive", config::get_backend_url()
                    ))
                    .header("Authorization", &format!("Bearer {}", token))
                    .send()
                    .await
                    {
                        if let Ok(json) = resp.json::<serde_json::Value>().await {
                            if let Some(val) = json.get("proactive").and_then(|v| v.as_bool()) {
                                is_calendar_proactive.set(val);
                            }
                        }
                    } else {
                        error.set(Some("Failed to fetch calendar state".into()));
                    }
                });
            }
            || ()
        }, ());
    }

    // Fetch WHATSAPP proactive state on mount
    {
        let is_whatsapp_proactive = is_whatsapp_proactive.clone();
        let error = error.clone();

        use_effect_with_deps(move |_| {
            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                spawn_local(async move {
                    if let Ok(resp) = Request::get(&format!(
                        "{}/api/profile/whatsapp-proactive", config::get_backend_url()
                    ))
                    .header("Authorization", &format!("Bearer {}", token))
                    .send()
                    .await
                    {
                        if let Ok(json) = resp.json::<serde_json::Value>().await {
                            if let Some(val) = json.get("proactive").and_then(|v| v.as_bool()) {
                                is_whatsapp_proactive.set(val);
                            }
                        }
                    } else {
                        error.set(Some("Failed to fetch WhatsApp state".into()));
                    }
                });
            }
            || ()
        }, ());
    }


    // Fetch connected services and their keywords on mount
    {
        let services_state = services_state.clone();
        let error = error.clone();
        let selected_service = selected_service.clone();

        use_effect_with_deps(move |_| {
            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|storage| storage.get_item("token").ok())
                .flatten()
            {
                spawn_local(async move {
                    // Fetch connected services
                    if let Ok(response) = Request::get(&format!("{}/api/filters/connected-services", config::get_backend_url()))
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await
                    {
                    if let Ok(mut services) = response.json::<Vec<ConnectedService>>().await {
                            
                            let mut service_states = Vec::new();

                            // For each service, fetch its keywords
                            for service in services {
                                let mut keywords = Vec::new();
                                let mut priority_senders = Vec::new();
                                let mut waiting_checks = Vec::new();

                                // Fetch keywords
                                if let Ok(keywords_response) = Request::get(&format!("{}/api/filters/keywords/{}", config::get_backend_url(), service.service_type))
                                    .header("Authorization", &format!("Bearer {}", token))
                                    .send()
                                    .await
                                {
                                    if let Ok(fetched_keywords) = keywords_response.json::<Vec<String>>().await {
                                        keywords = fetched_keywords;
                                    }
                                }

                                // Fetch priority senders
                                if let Ok(senders_response) = Request::get(&format!("{}/api/filters/priority-senders/{}", config::get_backend_url(), service.service_type))
                                    .header("Authorization", &format!("Bearer {}", token))
                                    .send()
                                    .await
                                {
                                    if let Ok(fetched_senders) = senders_response.json::<Vec<PrioritySender>>().await {
                                        priority_senders = fetched_senders;
                                    }
                                }

                                // Fetch waiting checks
                                if let Ok(checks_response) = Request::get(&format!("{}/api/filters/waiting-checks/{}", config::get_backend_url(), service.service_type))
                                    .header("Authorization", &format!("Bearer {}", token))
                                    .send()
                                    .await
                                {
                                    if let Ok(fetched_checks) = checks_response.json::<Vec<WaitingCheck>>().await {
                                        waiting_checks = fetched_checks;
                                    }
                                }

                                // Fetch importance priority
                                let mut importance_priority = None;
                                if let Ok(priority_response) = Request::get(&format!("{}/api/filters/importance-priority/{}", config::get_backend_url(), service.service_type))
                                    .header("Authorization", &format!("Bearer {}", token))
                                    .send()
                                    .await
                                {
                                    if let Ok(fetched_priority) = priority_response.json::<ImportancePriorityResponse>().await {
                                        importance_priority = Some(ImportancePriority::new(fetched_priority.threshold));
                                    }
                                }

                                service_states.push(ServiceState {
                                    service_type: service.service_type.clone(),
                                    identifier: service.identifier.clone(),
                                    filter_settings: Some(FilterSettings {
                                        keywords,
                                        priority_senders,
                                        waiting_checks,
                                        importance_priority,
                                    }),
                                });
                            }

                            // Set initial selected service
                            if let Some(first_service) = service_states.first() {
                                selected_service.set(Some(first_service.service_type.clone()));
                            }

                            services_state.set(service_states);
                        } else {
                            error.set(Some("Failed to parse connected services".to_string()));
                        }
                    } else {
                        error.set(Some("Failed to fetch connected services".to_string()));
                    }
                });
            }
            || ()
        }, ());
    }

    // Event handlers
    let on_service_click = {
        let selected_service = selected_service.clone();
        let fetch_keywords = fetch_keywords.clone();
        
        Callback::from(move |service_type: String| {
            let service_type_clone = service_type.clone();
            if Some(service_type.clone()) == *selected_service {
                selected_service.set(None);
            } else {
                selected_service.set(Some(service_type));
                fetch_keywords.emit(service_type_clone);
            }
        })
    };    

    // Render function for service grid
    let render_service_grid = {
        let services_state = services_state.clone();
        let selected_service = selected_service.clone();
        let on_service_click = on_service_click.clone();
        let is_calendar_proactive = is_calendar_proactive.clone();
        let is_whatsapp_proactive = is_whatsapp_proactive.clone();
        let is_proactive = is_proactive.clone();

        move || {
            (*services_state).iter().map(|service| {
                let service_type = service.service_type.clone();
                let is_selected = Some(service_type.clone()) == *selected_service;
                let onclick = {
                    let service_type = service_type.clone();
                    let on_service_click = on_service_click.clone();
                    Callback::from(move |_| {
                        on_service_click.emit(service_type.clone());
                    })
                };

                html! {
                    <div 
                        class={classes!(
                            "service-box",
                            "connected",
                            is_selected.then(|| "selected")
                        )}
                        onclick={onclick}
                    >
                        <div class="service-status">
                            {
                                if service.service_type == "imap" {
                                    if *is_proactive {
                                        html! { <span class="status-dot active" title="Active"></span> }
                                    } else {
                                        html! { <span class="status-dot" title="Inactive"></span> }
                                    }
                                } else if service.service_type == "calendar" {
                                    if *is_calendar_proactive {
                                        html! { <span class="status-dot active" title="Active"></span> }
                                    } else {
                                        html! { <span class="status-dot" title="Inactive"></span> }
                                    }
                                } else if service.service_type == "whatsapp" {
                                    if *is_whatsapp_proactive {
                                        html! { <span class="status-dot active" title="Active"></span> }
                                    } else {
                                        html! { <span class="status-dot" title="Inactive"></span> }
                                    }
                                } else {
                                    html! { <span class="status-dot" title="Inactive"></span> }
                                }
                            }
                        </div>
                        {
                            if service.service_type == "imap" {
                                html! {
                                    <img src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 512 512'%3E%3Cpath fill='%234285f4' d='M48 64C21.5 64 0 85.5 0 112c0 15.1 7.1 29.3 19.2 38.4L236.8 313.6c11.4 8.5 27 8.5 38.4 0L492.8 150.4c12.1-9.1 19.2-23.3 19.2-38.4c0-26.5-21.5-48-48-48H48zM0 176V384c0 35.3 28.7 64 64 64H448c35.3 0 64-28.7 64-64V176L294.4 339.2c-22.8 17.1-54 17.1-76.8 0L0 176z'/%3E%3C/svg%3E" alt="IMAP"/>
                                }
                            } else if service.service_type == "whatsapp" {
                                html! {
                                    <img src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 448 512'%3E%3Cpath fill='%234285f4' d='M380.9 97.1C339 55.1 283.2 32 223.9 32c-122.4 0-222 99.6-222 222 0 39.1 10.2 77.3 29.6 111L0 480l117.7-30.9c32.4 17.7 68.9 27 106.1 27h.1c122.3 0 224.1-99.6 224.1-222 0-59.3-25.2-115-67.1-157zm-157 341.6c-33.2 0-65.7-8.9-94-25.7l-6.7-4-69.8 18.3L72 359.2l-4.4-7c-18.5-29.4-28.2-63.3-28.2-98.2 0-101.7 82.8-184.5 184.6-184.5 49.3 0 95.6 19.2 130.4 54.1 34.8 34.9 56.2 81.2 56.1 130.5 0 101.8-84.9 184.6-186.6 184.6zm101.2-138.2c-5.5-2.8-32.8-16.2-37.9-18-5.1-1.9-8.8-2.8-12.5 2.8-3.7 5.6-14.3 18-17.6 21.8-3.2 3.7-6.5 4.2-12 1.4-32.6-16.3-54-29.1-75.5-66-5.7-9.8 5.7-9.1 16.3-30.3 1.8-3.7.9-6.9-.5-9.7-1.4-2.8-12.5-30.1-17.1-41.2-4.5-10.8-9.1-9.3-12.5-9.5-3.2-.2-6.9-.2-10.6-.2-3.7 0-9.7 1.4-14.8 6.9-5.1 5.6-19.4 19-19.4 46.3 0 27.3 19.9 53.7 22.6 57.4 2.8 3.7 39.1 59.7 94.8 83.8 35.2 15.2 49 16.5 66.6 13.9 10.7-1.6 32.8-13.4 37.4-26.4 4.6-13 4.6-24.1 3.2-26.4-1.3-2.5-5-3.9-10.5-6.6z'/%3E%3C/svg%3E" alt="WhatsApp"/>
                                }
                            } else {
                                html! {
                                    <img src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 448 512'%3E%3Cpath fill='%234285f4' d='M152 24c0-13.3-10.7-24-24-24s-24 10.7-24 24V64H64C28.7 64 0 92.7 0 128v16 48V448c0 35.3 28.7 64 64 64H384c35.3 0 64-28.7 64-64V192 144 128c0-35.3-28.7-64-64-64H344V24c0-13.3-10.7-24-24-24s-24 10.7-24 24V64H152V24zM48 192H400V448c0 8.8-7.2 16-16 16H64c-8.8 0-16-7.2-16-16V192z'/%3E%3C/svg%3E" alt="Calendar"/>
                                }
                            }
                        }
                        <h3>{get_service_display_name(&service.service_type)}</h3>
                        //<p class="service-identifier">{&service.identifier}</p>
                    </div>
                }
            }).collect::<Html>()
        }
    };

    html! {
        <div class="proactive-container">
            <h2>{"Proactive Notifications"}</h2>
            <div class="service-boxes-container">
                {render_service_grid()}
            </div>
            
            {
                if let Some(selected) = (*selected_service).clone() {
                    if let Some(service) = (*services_state).iter().find(|s| s.service_type == selected) {
                    if service.service_type == "whatsapp" {
                        if let Some(settings) = &service.filter_settings {
                            html! {
                                <div class="filters-container">
                                    <div class="proactive-toggle-section">
                                        <div class="notify-toggle">
                                            <img src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 448 512'%3E%3Cpath fill='%234285f4' d='M380.9 97.1C339 55.1 283.2 32 223.9 32c-122.4 0-222 99.6-222 222 0 39.1 10.2 77.3 29.6 111L0 480l117.7-30.9c32.4 17.7 68.9 27 106.1 27h.1c122.3 0 224.1-99.6 224.1-222 0-59.3-25.2-115-67.1-157zm-157 341.6c-33.2 0-65.7-8.9-94-25.7l-6.7-4-69.8 18.3L72 359.2l-4.4-7c-18.5-29.4-28.2-63.3-28.2-98.2 0-101.7 82.8-184.5 184.6-184.5 49.3 0 95.6 19.2 130.4 54.1 34.8 34.9 56.2 81.2 56.1 130.5 0 101.8-84.9 184.6-186.6 184.6zm101.2-138.2c-5.5-2.8-32.8-16.2-37.9-18-5.1-1.9-8.8-2.8-12.5 2.8-3.7 5.6-14.3 18-17.6 21.8-3.2 3.7-6.5 4.2-12 1.4-32.6-16.3-54-29.1-75.5-66-5.7-9.8 5.7-9.1 16.3-30.3 1.8-3.7.9-6.9-.5-9.7-1.4-2.8-12.5-30.1-17.1-41.2-4.5-10.8-9.1-9.3-12.5-9.5-3.2-.2-6.9-.2-10.6-.2-3.7 0-9.7 1.4-14.8 6.9-5.1 5.6-19.4 19-19.4 46.3 0 27.3 19.9 53.7 22.6 57.4 2.8 3.7 39.1 59.7 94.8 83.8 35.2 15.2 49 16.5 66.6 13.9 10.7-1.6 32.8-13.4 37.4-26.4 4.6-13 4.6-24.1 3.2-26.4-1.3-2.5-5-3.9-10.5-6.6z'/%3E%3C/svg%3E" alt="WhatsApp"/>
                                            <span class="proactive-title">{"WHATSAPP NOTIFICATIONS"}</span>
                                            <span class="toggle-status">
                                                {if *is_whatsapp_proactive { "Active" } else { "Inactive" }}
                                            </span>
                                            <label class="switch"
                                                   onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                                                <input
                                                    type="checkbox"
                                                    checked={*is_whatsapp_proactive}
                                                    onchange={Callback::from({
                                                        let flag = is_whatsapp_proactive.clone();
                                                        let error = error.clone();
                                                        move |e: Event| {
                                                            let el: HtmlInputElement = e.target_unchecked_into();
                                                            let val = el.checked();
                                                            let handle = flag.clone();
                                                            let error_handle = error.clone();
                                                            if let Some(tok) = window()
                                                                .and_then(|w| w.local_storage().ok())
                                                                .flatten()
                                                                .and_then(|s| s.get_item("token").ok())
                                                                .flatten()
                                                            {
                                                                spawn_local(async move {
                                                                    match Request::post(&format!(
                                                                        "{}/api/profile/whatsapp-proactive",
                                                                        config::get_backend_url()
                                                                    ))
                                                                    .header("Authorization", &format!("Bearer {}", tok))
                                                                    .json(&json!({ "proactive": val }))
                                                                    .expect("Failed to create request")
                                                                    .send()
                                                                    .await {
                                                                        Ok(response) => {
                                                                            if response.ok() {
                                                                                handle.set(val);
                                                                                error_handle.set(None);
                                                                            } else {
                                                                                error_handle.set(Some("Failed to update WhatsApp proactive state".to_string()));
                                                                            }
                                                                        }
                                                                        Err(_) => {
                                                                            error_handle.set(Some("Network error while updating WhatsApp proactive state".to_string()));
                                                                        }
                                                                    }
                                                                });
                                                            }
                                                        }
                                                    })}
                                                />
                                                <span class="slider round"></span>
                                            </label>
                                        </div>
                                        <p class="notification-description">
                                            {"Enable notifications for your WhatsApp messages. When enabled, you will receive SMS notifications for new important WhatsApp messages. Lightfriend processes them based on your notification preferences below."}
                                        </p>
                                    </div>
                                    {
                                        if *is_whatsapp_proactive {
                                            html! {
                                                <>
                                                <KeywordsSection
                                                    service_type={service.service_type.clone()}
                                                    keywords={settings.keywords.clone()}
                                                    on_change={Callback::from({
                                                        let services_state   = services_state.clone();
                                                        let service_type     = service.service_type.clone();
                                                        move |new_list: Vec<String>| {
                                                            let mut services = (*services_state).clone();
                                                            if let Some(svc) = services.iter_mut().find(|s| s.service_type == service_type) {
                                                                if let Some(fs) = &mut svc.filter_settings {
                                                                    fs.keywords = new_list;
                                                                }
                                                            }
                                                            services_state.set(services);
                                                        }
                                                    })}
                                                />
                                                <PrioritySendersSection
                                                    service_type={service.service_type.clone()}
                                                    senders={settings.priority_senders.clone()}
                                                    on_change={Callback::from({
                                                        let services_state = services_state.clone();
                                                        let stype          = service.service_type.clone();
                                                        move |list: Vec<PrioritySender>| {
                                                            let mut svcs = (*services_state).clone();
                                                            if let Some(svc) = svcs.iter_mut().find(|s| s.service_type == stype) {
                                                                if let Some(fs) = &mut svc.filter_settings {
                                                                    fs.priority_senders = list;
                                                                }
                                                            }
                                                            services_state.set(svcs);
                                                        }
                                                    })}
                                                />
                                                <WaitingChecksSection
                                                    service_type={service.service_type.clone()}
                                                    checks={settings.waiting_checks.clone()}
                                                    on_change={Callback::from({
                                                        let services_state = services_state.clone();
                                                        let stype          = service.service_type.clone();
                                                        move |list: Vec<WaitingCheck>| {
                                                            let mut svcs = (*services_state).clone();
                                                            if let Some(svc) = svcs.iter_mut().find(|s| s.service_type == stype) {
                                                                if let Some(fs) = &mut svc.filter_settings {
                                                                    fs.waiting_checks = list;
                                                                }
                                                            }
                                                            services_state.set(svcs);
                                                        }
                                                    })}
                                                />
                                                <ImportancePrioritySection
                                                    service_type={service.service_type.clone()}
                                                    current_threshold={
                                                        settings.importance_priority
                                                                .as_ref()
                                                                .map(|ip| ip.threshold)
                                                                .unwrap_or(7)
                                                    }
                                                    on_change={Callback::from({
                                                        let services_state = services_state.clone();
                                                        let stype          = service.service_type.clone();
                                                        move |new_thr: i32| {
                                                            let mut svcs = (*services_state).clone();
                                                            if let Some(svc) = svcs.iter_mut().find(|s| s.service_type == stype) {
                                                                if let Some(fs) = &mut svc.filter_settings {
                                                                    fs.importance_priority = Some(ImportancePriority { threshold: new_thr });
                                                                }
                                                            }
                                                            services_state.set(svcs);
                                                        }
                                                    })}
                                                />
                                                </>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }
                                </div>
                            }
                        } else {
                            html! {
                                <div class="filters-container">
                                    <div class="proactive-toggle-section">
                                        <div class="notify-toggle">
                                            <img src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 448 512'%3E%3Cpath fill='%234285f4' d='M380.9 97.1C339 55.1 283.2 32 223.9 32c-122.4 0-222 99.6-222 222 0 39.1 10.2 77.3 29.6 111L0 480l117.7-30.9c32.4 17.7 68.9 27 106.1 27h.1c122.3 0 224.1-99.6 224.1-222 0-59.3-25.2-115-67.1-157zm-157 341.6c-33.2 0-65.7-8.9-94-25.7l-6.7-4-69.8 18.3L72 359.2l-4.4-7c-18.5-29.4-28.2-63.3-28.2-98.2 0-101.7 82.8-184.5 184.6-184.5 49.3 0 95.6 19.2 130.4 54.1 34.8 34.9 56.2 81.2 56.1 130.5 0 101.8-84.9 184.6-186.6 184.6zm101.2-138.2c-5.5-2.8-32.8-16.2-37.9-18-5.1-1.9-8.8-2.8-12.5 2.8-3.7 5.6-14.3 18-17.6 21.8-3.2 3.7-6.5 4.2-12 1.4-32.6-16.3-54-29.1-75.5-66-5.7-9.8 5.7-9.1 16.3-30.3 1.8-3.7.9-6.9-.5-9.7-1.4-2.8-12.5-30.1-17.1-41.2-4.5-10.8-9.1-9.3-12.5-9.5-3.2-.2-6.9-.2-10.6-.2-3.7 0-9.7 1.4-14.8 6.9-5.1 5.6-19.4 19-19.4 46.3 0 27.3 19.9 53.7 22.6 57.4 2.8 3.7 39.1 59.7 94.8 83.8 35.2 15.2 49 16.5 66.6 13.9 10.7-1.6 32.8-13.4 37.4-26.4 4.6-13 4.6-24.1 3.2-26.4-1.3-2.5-5-3.9-10.5-6.6z'/%3E%3C/svg%3E" alt="WhatsApp"/>
                                            <span class="proactive-title">{"WHATSAPP NOTIFICATIONS(under contstruction)"}</span>
                                            /*
                                            <span class="toggle-status">
                                                {if *is_whatsapp_proactive { "Active" } else { "Inactive" }}
                                            </span>
                                            <label class="switch"
                                                   onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                                                <input
                                                    type="checkbox"
                                                    checked={*is_whatsapp_proactive}
                                                    onchange={Callback::from({
                                                        let flag = is_whatsapp_proactive.clone();
                                                        let error = error.clone();
                                                        move |e: Event| {
                                                            let el: HtmlInputElement = e.target_unchecked_into();
                                                            let val = el.checked();
                                                            let handle = flag.clone();
                                                            let error_handle = error.clone();
                                                            if let Some(tok) = window()
                                                                .and_then(|w| w.local_storage().ok())
                                                                .flatten()
                                                                .and_then(|s| s.get_item("token").ok())
                                                                .flatten()
                                                            {
                                                                spawn_local(async move {
                                                                    match Request::post(&format!(
                                                                        "{}/api/profile/whatsapp-proactive",
                                                                        config::get_backend_url()
                                                                    ))
                                                                    .header("Authorization", &format!("Bearer {}", tok))
                                                                    .json(&json!({ "proactive": val }))
                                                                    .expect("Failed to create request")
                                                                    .send()
                                                                    .await {
                                                                        Ok(response) => {
                                                                            if response.ok() {
                                                                                handle.set(val);
                                                                                error_handle.set(None);
                                                                            } else {
                                                                                error_handle.set(Some("Failed to update WhatsApp proactive state".to_string()));
                                                                            }
                                                                        }
                                                                        Err(_) => {
                                                                            error_handle.set(Some("Network error while updating WhatsApp proactive state".to_string()));
                                                                        }
                                                                    }
                                                                });
                                                            }
                                                        }
                                                    })}
                                                />
                                                <span class="slider round"></span>
                                            </label>
                                            */
                                        </div>
                                        <p class="notification-description">
                                            {"(this feature does not work yet it is under construction)Enable notifications for your WhatsApp messages. When enabled, you will receive SMS notifications for new important WhatsApp messages. Lightfriend processes them based on your notification preferences."}
                                        </p>
                                    </div>
                                </div>
                            }
                        }
                    } else if service.service_type == "calendar" {
                        html! {
                            <div class="filters-container">
                                    <div class="proactive-toggle-section">
                                        <div class="notify-toggle">
                                            <img src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 448 512'%3E%3Cpath fill='%234285f4' d='M152 24c0-13.3-10.7-24-24-24s-24 10.7-24 24V64H64C28.7 64 0 92.7 0 128v16 48V448c0 35.3 28.7 64 64 64H384c35.3 0 64-28.7 64-64V192 144 128c0-35.3-28.7-64-64-64H344V24c0-13.3-10.7-24-24-24s-24 10.7-24 24V64H152V24zM48 192H400V448c0 8.8-7.2 16-16 16H64c-8.8 0-16-7.2-16-16V192z'/%3E%3C/svg%3E" alt="Calendar"/>
                                            <span class="proactive-title">{"CALENDAR EVENT NOTIFICATIONS"}</span>
                                            <span class="toggle-status">
                                                {if *is_calendar_proactive { "Active" } else { "Inactive" }}
                                            </span>
                                            <label class="switch"
                                                   onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                                                <input
                                                    type="checkbox"
                                                    checked={*is_calendar_proactive}
                                                    onchange={Callback::from({
                                                        let flag = is_calendar_proactive.clone();
                                                        let error = error.clone();
                                                        move |e: Event| {
                                                            let el: HtmlInputElement = e.target_unchecked_into();
                                                            let val = el.checked();
                                                            let handle = flag.clone();
                                                            let error_handle = error.clone();
                                                            if let Some(tok) = window()
                                                                .and_then(|w| w.local_storage().ok())
                                                                .flatten()
                                                                .and_then(|s| s.get_item("token").ok())
                                                                .flatten()
                                                            {
                                                                spawn_local(async move {
                                                                    match Request::post(&format!(
                                                                        "{}/api/profile/calendar-proactive",
                                                                        config::get_backend_url()
                                                                    ))
                                                                    .header("Authorization", &format!("Bearer {}", tok))
                                                                    .json(&json!({ "proactive": val }))
                                                                    .expect("Failed to create request")
                                                                    .send()
                                                                    .await {
                                                                        Ok(response) => {
                                                                            if response.ok() {
                                                                                handle.set(val);
                                                                                error_handle.set(None);
                                                                            } else {
                                                                                error_handle.set(Some("Failed to update calendar proactive state".to_string()));
                                                                            }
                                                                        }
                                                                        Err(_) => {
                                                                            error_handle.set(Some("Network error while updating calendar proactive state".to_string()));
                                                                        }
                                                                    }
                                                                });
                                                            }
                                                        }
                                                    })}
                                                />
                                                <span class="slider round"></span>
                                            </label>
                                        </div>
                                        <p class="notification-description">
                                            {"Enable notifications for your Google Calendar events. When your calendar event has a reminder set, you will be reminded about it through SMS. Lightfriend fetches upcoming event reminders from google every five minutes. No calendar event data is stored on lightfriend's servers."}
                                        </p>
                                    </div>
                                </div>
                            }
                        } else if let Some(settings) = &service.filter_settings {
                            html! {
                                <div class="filters-container">
                                    <div class="proactive-toggle-section">
                                        <div class="notify-toggle">
                                            <img src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 512 512'%3E%3Cpath fill='%234285f4' d='M48 64C21.5 64 0 85.5 0 112c0 15.1 7.1 29.3 19.2 38.4L236.8 313.6c11.4 8.5 27 8.5 38.4 0L492.8 150.4c12.1-9.1 19.2-23.3 19.2-38.4c0-26.5-21.5-48-48-48H48zM0 176V384c0 35.3 28.7 64 64 64H448c35.3 0 64-28.7 64-64V176L294.4 339.2c-22.8 17.1-54 17.1-76.8 0L0 176z'/%3E%3C/svg%3E" alt="IMAP"/>
                                            <span class="proactive-title">{"IMPORTANT EMAIL NOTIFICATIONS"}</span>
                                            <span class="toggle-status">
                                                {if *is_proactive { "Active" } else { "Inactive" }}
                                            </span>
                                            <label class="switch"
                                                   onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>

                                                <input 
                                                    type="checkbox" 
                                                    checked={*is_proactive}
                                                    onchange={
                                                        let is_proactive = is_proactive.clone();
                                                        Callback::from(move |e: Event| {
                                                            let input: HtmlInputElement = e.target_unchecked_into();
                                                            let proactive = input.checked();
                                                            let is_proactive_clone = is_proactive.clone();
                                                            
                                                            if let Some(token) = window()
                                                                .and_then(|w| w.local_storage().ok())
                                                                .flatten()
                                                                .and_then(|storage| storage.get_item("token").ok())
                                                                .flatten()
                                                            {
                                                                spawn_local(async move {
                                                                    if let Ok(_) = Request::post(&format!("{}/api/profile/imap-proactive", config::get_backend_url()))
                                                                        .header("Authorization", &format!("Bearer {}", token))
                                                                        .header("Content-Type", "application/json")
                                                                        .json(&json!({"proactive": proactive}))
                                                                        .expect("Failed to serialize proactive request")
                                                                        .send()
                                                                        .await
                                                                    {
                                                                        is_proactive_clone.set(proactive);
                                                                    }
                                                                });
                                                            }
                                                        })
                                                    }
                                                />
                                                <span class="slider round"></span>
                                            </label>
                                        </div>
                                        <p class="notification-description">
                                            {"Enable notifications for specific emails based on your filters. Lightfriend fetches emails from your email server every minute and processes new emails based on the importance criteria below. If some criteria is met, you will be notified about the email through SMS. No email data will be stored on lightfriend's servers expect email ids to prevent processing same email twice."}
                                        </p>
                                    </div> // end proactive-toggle-section
                                    {
                                        if *is_proactive {
                                            let priority_senders: Vec<String> = settings.priority_senders.iter()
                                                .map(|sender| sender.sender.clone())
                                                .collect();
                                            
                                            let waiting_checks: Vec<String> = settings.waiting_checks.iter()
                                                .map(|check| check.content.clone())
                                                .collect();

                                            let threshold = settings.importance_priority
                                                .as_ref()
                                                .map(|ip| ip.threshold)
                                                .unwrap_or(7);

                                            html! {
                                                <>
                                                <FilterActivityLog />

                                                <KeywordsSection
                                                    service_type={service.service_type.clone()}
                                                    keywords={settings.keywords.clone()}
                                                    on_change={Callback::from({
                                                        let services_state   = services_state.clone();
                                                        let service_type     = service.service_type.clone();
                                                        move |new_list: Vec<String>| {
                                                            let mut services = (*services_state).clone();
                                                            if let Some(svc) = services.iter_mut().find(|s| s.service_type == service_type) {
                                                                if let Some(fs) = &mut svc.filter_settings {
                                                                    fs.keywords = new_list;
                                                                }
                                                            }
                                                            services_state.set(services);
                                                        }
                                                    })}
                                                />
                                                <PrioritySendersSection
                                                    service_type={service.service_type.clone()}
                                                    senders={settings.priority_senders.clone()}
                                                    on_change={Callback::from({
                                                        let services_state = services_state.clone();
                                                        let stype          = service.service_type.clone();
                                                        move |list: Vec<PrioritySender>| {
                                                            let mut svcs = (*services_state).clone();
                                                            if let Some(svc) = svcs.iter_mut().find(|s| s.service_type == stype) {
                                                                if let Some(fs) = &mut svc.filter_settings {
                                                                    fs.priority_senders = list;
                                                                }
                                                            }
                                                            services_state.set(svcs);
                                                        }
                                                    })}
                                                />

                                                <WaitingChecksSection
                                                    service_type={service.service_type.clone()}
                                                    checks={settings.waiting_checks.clone()}
                                                    on_change={Callback::from({
                                                        let services_state = services_state.clone();
                                                        let stype          = service.service_type.clone();
                                                        move |list: Vec<WaitingCheck>| {
                                                            let mut svcs = (*services_state).clone();
                                                            if let Some(svc) = svcs.iter_mut().find(|s| s.service_type == stype) {
                                                                if let Some(fs) = &mut svc.filter_settings {
                                                                    fs.waiting_checks = list;
                                                                }
                                                            }
                                                            services_state.set(svcs);
                                                        }
                                                    })}
                                                />
                                                <ImportancePrioritySection
                                                    service_type={service.service_type.clone()}
                                                    current_threshold={
                                                        settings.importance_priority
                                                                .as_ref()
                                                                .map(|ip| ip.threshold)
                                                                .unwrap_or(7)
                                                    }
                                                    on_change={Callback::from({
                                                        let services_state = services_state.clone();
                                                        let stype          = service.service_type.clone();
                                                        move |new_thr: i32| {
                                                            let mut svcs = (*services_state).clone();
                                                            if let Some(svc) = svcs.iter_mut().find(|s| s.service_type == stype) {
                                                                if let Some(fs) = &mut svc.filter_settings {
                                                                    fs.importance_priority = Some(ImportancePriority { threshold: new_thr });
                                                                }
                                                            }
                                                            services_state.set(svcs);
                                                        }
                                                    })}
                                                />

                                                <ImapGeneralChecks 
                                                    on_update={Callback::from(|_| {})}
                                                    keywords={settings.keywords.clone()}
                                                    priority_senders={priority_senders}
                                                    waiting_checks={waiting_checks}
                                                    threshold={threshold}
                                                />

                                                </>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }
                                </div>
                            }

                        } else if selected == "calendar_push" {
                            html! {
                                <div class="filters-container">
                                    <div class="proactive-toggle-section">
                                        <div class="notify-toggle">
                                            <i class="service-icon calendar-icon"></i>
                                            <span class="proactive-title">{"Proactive CALENDAR"}</span>
                                            <span class="toggle-status">
                                                {if *is_calendar_proactive { "Active" } else { "Inactive" }}
                                            </span>
                                            <label class="switch">
                                                <input
                                                    type="checkbox"
                                                    checked={*is_calendar_proactive}
                                                    onchange={Callback::from({
                                                        let flag = is_calendar_proactive.clone();
                                                        move |e: Event| {
                                                            let el: HtmlInputElement = e.target_unchecked_into();
                                                            let val = el.checked();
                                                            let handle = flag.clone();
                                                            if let Some(tok) = window()
                                                                .and_then(|w| w.local_storage().ok())
                                                                .flatten()
                                                                .and_then(|s| s.get_item("token").ok())
                                                                .flatten()
                                                            {
                                                                spawn_local(async move {
                                                                    let _ = Request::post(&format!(
                                                                        "{}/api/profile/calendar-proactive",
                                                                        config::get_backend_url()
                                                                    ))
                                                                    .header("Authorization", &format!("Bearer {}", tok))
                                                                    .json(&json!({ "proactive": val }))
                                                                    .unwrap()
                                                                    .send()
                                                                    .await;
                                                                    handle.set(val);
                                                                });
                                                            }
                                                        }
                                                    })}
                                                />
                                                <span class="slider round"></span>
                                            </label>
                                        </div>
                                        <p class="notification-description">
                                            {"Turn on calendar push notifications (more settings coming soon)."}
                                        </p>
                                    </div>
                                </div>
                            }
                        } else if service.service_type == "whatsapp" {
                            html! {
                                <div class="filters-container">
                                    <div class="proactive-toggle-section">
                                        <div class="notify-toggle">
                                            <img src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 448 512'%3E%3Cpath fill='%234285f4' d='M380.9 97.1C339 55.1 283.2 32 223.9 32c-122.4 0-222 99.6-222 222 0 39.1 10.2 77.3 29.6 111L0 480l117.7-30.9c32.4 17.7 68.9 27 106.1 27h.1c122.3 0 224.1-99.6 224.1-222 0-59.3-25.2-115-67.1-157zm-157 341.6c-33.2 0-65.7-8.9-94-25.7l-6.7-4-69.8 18.3L72 359.2l-4.4-7c-18.5-29.4-28.2-63.3-28.2-98.2 0-101.7 82.8-184.5 184.6-184.5 49.3 0 95.6 19.2 130.4 54.1 34.8 34.9 56.2 81.2 56.1 130.5 0 101.8-84.9 184.6-186.6 184.6zm101.2-138.2c-5.5-2.8-32.8-16.2-37.9-18-5.1-1.9-8.8-2.8-12.5 2.8-3.7 5.6-14.3 18-17.6 21.8-3.2 3.7-6.5 4.2-12 1.4-32.6-16.3-54-29.1-75.5-66-5.7-9.8 5.7-9.1 16.3-30.3 1.8-3.7.9-6.9-.5-9.7-1.4-2.8-12.5-30.1-17.1-41.2-4.5-10.8-9.1-9.3-12.5-9.5-3.2-.2-6.9-.2-10.6-.2-3.7 0-9.7 1.4-14.8 6.9-5.1 5.6-19.4 19-19.4 46.3 0 27.3 19.9 53.7 22.6 57.4 2.8 3.7 39.1 59.7 94.8 83.8 35.2 15.2 49 16.5 66.6 13.9 10.7-1.6 32.8-13.4 37.4-26.4 4.6-13 4.6-24.1 3.2-26.4-1.3-2.5-5-3.9-10.5-6.6z'/%3E%3C/svg%3E" alt="WhatsApp"/>
                                            <span class="proactive-title">{"WHATSAPP NOTIFICATIONS"}</span>
                                            <span class="toggle-status">
                                                {if *is_whatsapp_proactive { "Active" } else { "Inactive" }}
                                            </span>
                                            <label class="switch"
                                                   onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                                                <input
                                                    type="checkbox"
                                                    checked={*is_whatsapp_proactive}
                                                    onchange={Callback::from({
                                                        let flag = is_whatsapp_proactive.clone();
                                                        let error = error.clone();
                                                        move |e: Event| {
                                                            let el: HtmlInputElement = e.target_unchecked_into();
                                                            let val = el.checked();
                                                            let handle = flag.clone();
                                                            let error_handle = error.clone();
                                                            if let Some(tok) = window()
                                                                .and_then(|w| w.local_storage().ok())
                                                                .flatten()
                                                                .and_then(|s| s.get_item("token").ok())
                                                                .flatten()
                                                            {
                                                                spawn_local(async move {
                                                                    match Request::post(&format!(
                                                                        "{}/api/profile/whatsapp-proactive",
                                                                        config::get_backend_url()
                                                                    ))
                                                                    .header("Authorization", &format!("Bearer {}", tok))
                                                                    .json(&json!({ "proactive": val }))
                                                                    .expect("Failed to create request")
                                                                    .send()
                                                                    .await {
                                                                        Ok(response) => {
                                                                            if response.ok() {
                                                                                handle.set(val);
                                                                                error_handle.set(None);
                                                                            } else {
                                                                                error_handle.set(Some("Failed to update WhatsApp proactive state".to_string()));
                                                                            }
                                                                        }
                                                                        Err(_) => {
                                                                            error_handle.set(Some("Network error while updating WhatsApp proactive state".to_string()));
                                                                        }
                                                                    }
                                                                });
                                                            }
                                                        }
                                                    })}
                                                />
                                                <span class="slider round"></span>
                                            </label>
                                        </div>
                                        <p class="notification-description">
                                            {"Enable notifications for your WhatsApp messages. When enabled, you will receive SMS notifications for new WhatsApp messages. Lightfriend checks for new messages every minute and processes them based on your notification preferences. No message content is stored on lightfriend's servers."}
                                        </p>
                                    </div>
                                </div>
                            }
                        } else {
                            html! {}
                        }
                    } else {
                        html! {}
                    }
                } else {
                    html! {
                        <p>{"Add some connections and they will appear here."}</p>
                    }
                }
            }

            {
                if let Some(err) = (*error).as_ref() {
                    html! {
                        <div class="error-message">{err}</div>
                    }
                } else {
                    html! {}
                }
            }



            <style>
                {r#"
                .proactive-container {
                    padding: 0;
                    max-width: 800px;
                    margin: 0 auto;
                }

                .proactive-container h2 {
                    color: #7EB2FF;
                    font-size: 1.5rem;
                    margin-bottom: 2rem;
                    text-align: left;
                }

                .service-boxes-container {
                    display: flex;
                    gap: 1rem;
                    flex-wrap: wrap;
                    margin-bottom: 1rem;
                }

                .service-box {
                    background: rgba(30, 144, 255, 0.05);
                    border: 1px solid rgba(30, 144, 255, 0.1);
                    border-radius: 12px;
                    padding: 1rem;
                    cursor: pointer;
                    transition: all 0.3s ease;
                    display: flex;
                    flex-direction: column;
                    align-items: center;
                    gap: 0.3rem;
                    min-width: 120px;
                    max-width: 150px;
                    position: relative;
                }

                .service-status {
                    position: absolute;
                    top: 8px;
                    right: 8px;
                }

                .status-dot {
                    width: 10px;
                    height: 10px;
                    background-color: rgba(255, 255, 255, 0.3);
                    border-radius: 50%;
                    display: inline-block;
                    transition: all 0.3s ease;
                }

                .status-dot.active {
                    background-color: #4CAF50;
                    box-shadow: 0 0 8px rgba(76, 175, 80, 0.5);
                }

                .service-box img {
                    width: 24px;
                    height: 24px;
                    margin-bottom: 0.5rem;
                }

                .service-box:hover {
                    transform: translateY(-2px);
                    background: rgba(30, 144, 255, 0.08);
                    border-color: rgba(30, 144, 255, 0.2);
                    box-shadow: 0 4px 20px rgba(30, 144, 255, 0.15);
                }

                .service-box.selected {
                    background: rgba(30, 144, 255, 0.1);
                    border-color: rgba(30, 144, 255, 0.3);
                    box-shadow: 0 0 15px rgba(30, 144, 255, 0.2);
                }

                .service-box h3 {
                    color: #1E90FF;
                    font-size: 1rem;
                    margin: 0;
                    text-align: center;
                }

                .service-identifier {
                    color: #999;
                    font-size: 0.8rem;
                    margin: 0;
                    text-align: center;
                    word-break: break-all;
                }

                .service-icon {
                    font-size: 1.5rem;
                    color: #1E90FF;
                    margin-bottom: 0.5rem;
                }

                .keyword-section {
                    background: rgba(30, 30, 30, 0.7);
                    border: 1px solid rgba(30, 144, 255, 0.1);
                    border-radius: 12px;
                    padding: 1.5rem;
                    margin-top: 2rem;
                    backdrop-filter: blur(10px);
                }

                .keyword-input,
                .filter-input {
                    display: flex;
                    gap: 1rem;
                    margin-bottom: 1.5rem;
                }

                .waiting-check-input {
                    display: flex;
                    gap: 1rem;
                    margin-bottom: 1.5rem;
                }

                .waiting-check-fields {
                    display: flex;
                    flex-direction: column;
                    gap: 1rem;
                    flex: 1;
                    align-items: start;
                    flex-wrap: wrap;
                }

                .waiting-check-fields input[type="text"],
                .waiting-check-fields input[type="date"] {
                    padding: 0.8rem 1rem;
                    background: rgba(30, 144, 255, 0.05);
                    border: 1px solid rgba(30, 144, 255, 0.1);
                    border-radius: 8px;
                    color: #fff;
                    font-size: 0.9rem;
                    transition: all 0.3s ease;
                    min-width: 0;
                }

                .waiting-check-fields input[type="text"] {
                    flex: 2;
                    width: 100%;
                }

                .waiting-check-fields input[type="date"] {
                    flex: 1;
                    width: 100%;
                }

                .waiting-check-fields .date-label {
                    display: flex;
                    flex-direction: column;
                    gap: 0.5rem;
                    width: 100%;
                }

                .waiting-check-fields .date-label span {
                    color: #fff;
                    font-size: 0.9rem;
                }

                .waiting-check-fields input[type="text"]:focus,
                .waiting-check-fields input[type="date"]:focus {
                    outline: none;
                    border-color: rgba(30, 144, 255, 0.3);
                    background: rgba(30, 144, 255, 0.08);
                    box-shadow: 0 0 10px rgba(30, 144, 255, 0.1);
                }

                .waiting-check-fields label {
                    display: flex;
                    align-items: center;
                    color: #fff;
                    font-size: 0.9rem;
                    gap: 0.5rem;
                    padding: 0.5rem 0;
                    white-space: nowrap;
                }

                .waiting-check-fields input[type="checkbox"] {
                    width: 18px;
                    height: 18px;
                    accent-color: #1E90FF;
                    cursor: pointer;
                }

                .waiting-check-input button {
                    align-self: flex-start;
                    margin-top: 0.5rem;
                    padding: 0.8rem 1.5rem;
                    background: linear-gradient(45deg, #1E90FF, #4169E1);
                    color: white;
                    border: none;
                    border-radius: 8px;
                    cursor: pointer;
                    font-size: 0.9rem;
                    transition: all 0.3s ease;
                    text-transform: uppercase;
                    letter-spacing: 0.5px;
                    white-space: nowrap;
                    align-self: center;
                }

                .waiting-check-input button:hover {
                    transform: translateY(-2px);
                    box-shadow: 0 4px 20px rgba(30, 144, 255, 0.3);
                }

                @media (max-width: 768px) {
                    .waiting-check-input {
                        flex-direction: column;
                    }

                    .waiting-check-fields {
                        flex-direction: column;
                    }

                    .waiting-check-fields input[type="text"],
                    .waiting-check-fields input[type="date"] {
                        width: 100%;
                    }

                    .waiting-check-input button {
                        width: 100%;
                    }
                }

                .keyword-input input,
                .filter-input input[type="text"],
                .filter-input input[type="number"],
                .filter-input input[type="date"],
                .waiting-check-fields input[type="text"],
                .waiting-check-fields input[type="date"] {
                    flex: 1;
                    padding: 0.8rem 1rem;
                    background: rgba(30, 144, 255, 0.05);
                    border: 1px solid rgba(30, 144, 255, 0.1);
                    border-radius: 8px;
                    color: #fff;
                    font-size: 0.9rem;
                    transition: all 0.3s ease;
                    min-width: 0; /* Prevents input from overflowing container */
                }

                .keyword-input input:focus,
                .filter-input input[type="text"]:focus,
                .filter-input input[type="number"]:focus,
                .filter-input input[type="date"]:focus {
                    outline: none;
                    border-color: rgba(30, 144, 255, 0.3);
                    background: rgba(30, 144, 255, 0.08);
                    box-shadow: 0 0 10px rgba(30, 144, 255, 0.1);
                }

                .keyword-input input::placeholder,
                .filter-input input::placeholder {
                    color: rgba(255, 255, 255, 0.5);
                }

                .keyword-input button,
                .filter-input button {
                    padding: 0.8rem 1.5rem;
                    background: linear-gradient(45deg, #1E90FF, #4169E1);
                    color: white;
                    border: none;
                    border-radius: 8px;
                    cursor: pointer;
                    font-size: 0.9rem;
                    transition: all 0.3s ease;
                    text-transform: uppercase;
                    letter-spacing: 0.5px;
                    white-space: nowrap; /* Prevents button text from wrapping */
                }

                .keyword-input button:hover,
                .filter-input button:hover {
                    transform: translateY(-2px);
                    box-shadow: 0 4px 20px rgba(30, 144, 255, 0.3);
                }
                .priority-list li .remove-when-found {
                    margin-left: 1rem;
                    color: #999;
                    font-style: italic;
                }

                .filter-input input[type="date"] {
                    color-scheme: dark;
                }

                .filter-input input[type="number"] {
                    width: 80px;
                    text-align: center;
                    -moz-appearance: textfield;
                }

                .filter-input input[type="number"]::-webkit-outer-spin-button,
                .filter-input input[type="number"]::-webkit-inner-spin-button {
                    -webkit-appearance: none;
                    margin: 0;
                }

                .importance-input-group {
                    display: flex;
                    align-items: center;
                    gap: 0.5rem;
                }

                .priority-label {
                    color: #7EB2FF;
                    margin-left: 10px;
                    font-size: 0.9rem;
                }

                .save-btn {
                    padding: 0.5rem 1rem;
                    background: linear-gradient(45deg, #1E90FF, #4169E1);
                    color: white;
                    border: none;
                    border-radius: 6px;
                    cursor: pointer;
                    font-size: 0.9rem;
                    transition: all 0.3s ease;
                    text-transform: uppercase;
                    letter-spacing: 0.5px;
                }

                .save-btn:hover {
                    transform: translateY(-2px);
                    box-shadow: 0 4px 15px rgba(30, 144, 255, 0.3);
                }

                .save-btn:active {
                    transform: translateY(0);
                    box-shadow: 0 2px 10px rgba(30, 144, 255, 0.2);
                }

                .filter-input {
                    display: flex;
                    align-items: center;
                    gap: 1rem;
                }

                .keyword-list {
                    list-style: none;
                    padding: 0;
                    margin: 0;
                    display: flex;
                    flex-wrap: wrap;
                    gap: 1rem;
                }

                .keyword-item {
                    display: flex;

                    align-items: center;
                    gap: 0.5rem;
                    padding: 0.5rem 1rem;
                    background: rgba(30, 144, 255, 0.1);
                    border: 1px solid rgba(30, 144, 255, 0.2);
                    border-radius: 20px;
                    transition: all 0.3s ease;
                }

                .keyword-item:hover {
                    background: rgba(30, 144, 255, 0.15);
                    transform: translateY(-1px);
                }

                .keyword-item span {
                    color: #fff;
                    font-size: 0.9rem;
                }

                .delete-btn {
                    background: none;
                    border: none;
                    color: rgba(255, 255, 255, 0.7);
                    cursor: pointer;
                    font-size: 1.2rem;
                    padding: 0;
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    transition: all 0.3s ease;
                }

                .delete-btn:hover {
                    color: #ff4757;
                    transform: scale(1.1);
                }

                .error-message {
                    color: #ff4757;
                    background: rgba(255, 71, 87, 0.1);
                    border: 1px solid rgba(255, 71, 87, 0.2);
                    padding: 1rem;
                    border-radius: 8px;
                    margin-top: 1rem;
                    text-align: center;
                    font-size: 0.9rem;
                }

                .filters-container {
                    display: flex;
                    flex-direction: column;
                }

                .proactive-toggle-section {
                    background: rgba(30, 30, 30, 0.5);
                    border: 1px solid rgba(30, 144, 255, 0.1);
                    border-radius: 8px;
                    padding: 1.5rem;
                    margin-bottom: 1rem;
                }

                .notify-toggle {
                    display: flex;
                    align-items: center;
                    gap: 1rem;
                    margin-bottom: 0.8rem;
                }

                .notify-toggle span:first-child {
                    font-size: 1.1rem;
                    font-weight: 500;
                    color: #7EB2FF;
                    text-transform: uppercase;
                    letter-spacing: 0.5px;
                    background: linear-gradient(45deg, #7EB2FF, #4169E1);
                    -webkit-background-clip: text;
                    -webkit-text-fill-color: transparent;
                    text-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
                }

                .toggle-status {
                    color: #7EB2FF;
                    font-size: 0.9rem;
                    padding: 0.3rem 0.8rem;
                    background: rgba(30, 144, 255, 0.1);
                    border-radius: 12px;
                    border: 1px solid rgba(30, 144, 255, 0.2);
                    transition: all 0.3s ease;
                }

                .notification-description {
                    color: rgba(255, 255, 255, 0.7);
                    font-size: 0.9rem;
                    margin: 0;
                    line-height: 1.4;
                    padding-left: 0.2rem;
                }

                .inactive-message {
                    background: rgba(30, 30, 30, 0.5);
                    border: 1px solid rgba(30, 144, 255, 0.1);
                    border-radius: 8px;
                    padding: 1.5rem;
                    margin-top: 1rem;
                    text-align: center;
                }

                .inactive-message p {
                    color: rgba(255, 255, 255, 0.7);
                    font-size: 1rem;
                    margin: 0;
                    line-height: 1.5;
                }

                .proactive-title {
                    display: flex;
                    align-items: center;
                    gap: 0.8rem;
                    font-size: 1.1rem;
                    font-weight: 600;
                    text-transform: uppercase;
                    letter-spacing: 0.8px;
                    color: #7EB2FF;
                    text-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
                    background: linear-gradient(135deg, #7EB2FF 0%, #4169E1 100%);
                    -webkit-background-clip: text;
                    -webkit-text-fill-color: transparent;
                    padding: 0.2rem 0;
                    position: relative;
                    transition: all 0.3s ease;
                }

                .proactive-title::after {
                    content: '';
                    position: absolute;
                    bottom: 0;
                    left: 0;
                    width: 100%;
                    height: 2px;
                    background: linear-gradient(90deg, #7EB2FF 0%, transparent 100%);
                    opacity: 0.3;
                }

                .proactive-title:hover {
                    transform: translateY(-1px);
                    text-shadow: 0 4px 8px rgba(0, 0, 0, 0.3);
                }
                .notify-toggle img {
                    width: 24px;
                    height: 24px;
                }

                /* Switch styles */
                .switch {
                    position: relative;
                    display: inline-block;
                    width: 60px;
                    height: 34px;
                    margin-left: auto;
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
                    height: 26px;
                    width: 26px;
                    left: 4px;
                    bottom: 3px;
                    background-color: white;
                    transition: .4s;
                }

                input:checked + .slider {
                    background-color: #1E90FF;
                }

                input:checked + .slider:before {
                    transform: translateX(26px);
                }

                .slider.round {
                    border-radius: 34px;
                }

                .slider.round:before {
                    border-radius: 50%;
                }

                .filter-section {
                    background: rgba(30, 30, 30, 0.5);
                    border: 1px solid rgba(30, 144, 255, 0.1);
                    border-radius: 8px;
                    padding: 1.5rem;
                    margin-top: 0;
                }

                .filter-section h3 {
                    color: #7EB2FF;
                    font-size: 1.2rem;
                    margin-bottom: 1rem;
                }

                /* Judgment list styles */
                .judgment-list {
                    display: flex;
                    flex-direction: column;
                    gap: 1rem;
                    max-height: 500px;
                    overflow-y: auto;
                    padding-right: 0.5rem;
                }

                .judgment-list::-webkit-scrollbar {
                    width: 8px;
                }

                .judgment-list::-webkit-scrollbar-track {
                    background: rgba(30, 30, 30, 0.5);
                    border-radius: 4px;
                }

                .judgment-list::-webkit-scrollbar-thumb {
                    background: rgba(30, 144, 255, 0.3);
                    border-radius: 4px;
                }

                .judgment-list::-webkit-scrollbar-thumb:hover {
                    background: rgba(30, 144, 255, 0.5);
                }

                .judgment-item {
                    background: rgba(30, 30, 30, 0.7);
                    border: 1px solid rgba(30, 144, 255, 0.1);
                    border-radius: 8px;
                    padding: 1rem;
                    transition: all 0.3s ease;
                }

                .judgment-item:hover {
                    transform: translateY(-2px);
                    box-shadow: 0 4px 20px rgba(30, 144, 255, 0.1);
                }

                .judgment-item.notify {
                    border-left: 4px solid #4CAF50;
                }

                .judgment-item.no-notify {
                    border-left: 4px solid #ff4757;
                }

                .judgment-header {
                    display: flex;
                    justify-content: space-between;
                    align-items: center;
                    margin-bottom: 0.5rem;
                }

                .judgment-date {
                    color: #7EB2FF;
                    font-size: 0.9rem;
                }

                .judgment-status {
                    font-size: 0.8rem;
                    padding: 0.3rem 0.8rem;
                    border-radius: 12px;
                    font-weight: 500;
                }

                .judgment-status.notify {
                    background: rgba(76, 175, 80, 0.1);
                    color: #4CAF50;
                }

                .judgment-status.no-notify {
                    background: rgba(255, 71, 87, 0.1);
                    color: #ff4757;
                }

                .judgment-score {
                    margin-bottom: 0.5rem;
                }

                .score-label {
                    color: #999;
                    font-size: 0.9rem;
                }

                .score-value {
                    color: #fff;
                    font-size: 0.9rem;
                    font-weight: 500;
                }

                .judgment-reason {
                    margin-bottom: 0.5rem;
                }

                .reason-label {
                    color: #999;
                    font-size: 0.9rem;
                }

                .reason-text {
                    color: #fff;
                    font-size: 0.9rem;
                    display: block;
                    margin-top: 0.2rem;
                    line-height: 1.4;
                }

                .judgment-processed {
                    font-size: 0.8rem;
                    color: #666;
                }

                .processed-label {
                    color: #999;
                }

                .processed-date {
                    color: #666;
                }

                .loading-judgments {
                    text-align: center;
                    padding: 2rem;
                    color: #999;
                    font-style: italic;
                }

                .filter-input {
                    display: flex;
                    gap: 1rem;
                    margin-bottom: 1rem;
                    flex-wrap: wrap;
                    align-items: center;
                }

                .filter-input label {
                    display: flex;
                    align-items: center;
                    color: #fff;
                    font-size: 0.9rem;
                    gap: 0.5rem;
                    padding: 0.5rem 0;
                }

                .filter-input input[type="checkbox"] {
                    width: 18px;
                    height: 18px;
                    accent-color: #1E90FF;
                    cursor: pointer;
                }

                .filter-input input[type="checkbox"] {
                    margin-right: 0.5rem;
                }

                .filter-input label {
                    display: flex;
                    align-items: center;
                    color: #fff;
                    font-size: 0.9rem;
                }

                .filter-list {
                    list-style: none;
                    padding: 0;
                    margin: 0;
                    display: flex;
                    flex-wrap: wrap;
                    gap: 0.5rem;
                }

                .filter-list li {
                    display: flex;
                    align-items: center;
                    gap: 0.5rem;
                    padding: 0.5rem 1rem;
                    background: rgba(30, 144, 255, 0.1);
                    border: 1px solid rgba(30, 144, 255, 0.2);
                    border-radius: 20px;
                }

                .filter-list li span {
                    color: #fff;
                    font-size: 0.9rem;
                }

                .filter-list li .due-date {
                    margin-left: 1rem;
                    color: #7EB2FF;
                }

                .filter-list li .remove-when-found {
                    margin-left: 1rem;
                    color: #999;
                    font-style: italic;
                }

                .filter-input input[type="date"] {
                    color-scheme: dark;
                }

                @media (max-width: 768px) {
                    .filter-input {
                        flex-direction: column;
                    }

                    .filter-input input[type="text"],
                    .filter-input input[type="number"],
                    .filter-input input[type="date"],
                    .filter-input button {
                        width: 100%;
                    }

                    .filter-list li {
                        width: 100%;
                    }
                }

                /* Service icons */
                .calendar-icon::before {
                    content: "📅";
                }

                .imap-icon::before {
                    content: "📧";
                }

                /* Responsive design */
                @media (max-width: 768px) {
                    .proactive-container {
                        padding: 0.5rem;
                        max-width: 100%;
                    }

                    .filter-section {
                        padding: 1rem;
                        margin: 0.5rem 0;
                        border-radius: 6px;
                    }

                    .proactive-toggle-section {
                        padding: 1rem;
                        margin: 0.5rem 0;
                        border-radius: 6px;
                    }

                    .notify-toggle {
                        flex-wrap: wrap;
                        gap: 0.5rem;
                    }

                    .proactive-title {
                        font-size: 1rem;
                        flex: 1 1 auto;
                    }

                    .toggle-status {
                        font-size: 0.8rem;
                        padding: 0.2rem 0.6rem;
                    }

                    .notification-description {
                        font-size: 0.85rem;
                        margin-top: 0.5rem;
                    }

                    .keyword-input,
                    .filter-input {
                        flex-direction: column;
                        gap: 0.5rem;
                        margin-bottom: 1rem;
                    }

                    .keyword-input button,
                    .filter-input button {
                        width: 100%;
                        padding: 0.6rem 1rem;
                    }

                    .keyword-list,
                    .filter-list {
                        gap: 0.5rem;
                    }

                    .keyword-item,
                    .filter-item {
                        width: 100%;
                        justify-content: space-between;
                        padding: 0.4rem 0.8rem;
                        font-size: 0.9rem;
                    }

                    .judgment-list {
                        max-height: 400px;
                        padding-right: 0;
                    }

                    .judgment-item {
                        padding: 0.8rem;
                        margin-bottom: 0.5rem;
                    }

                    .judgment-header {
                        flex-wrap: wrap;
                        gap: 0.5rem;
                    }

                    .judgment-date,
                    .judgment-status {
                        font-size: 0.8rem;
                    }

                    .judgment-reason,
                    .judgment-score,
                    .judgment-processed {
                        font-size: 0.85rem;
                        margin-bottom: 0.4rem;
                    }

                    .importance-input-group {
                        width: 100%;
                    }

                    .filter-input input[type="number"] {
                        width: 100%;
                        text-align: left;
                    }

                    .priority-label {
                        font-size: 0.8rem;
                    }

                    .save-btn {
                        width: 100%;
                        margin-top: 0.5rem;
                    }

                    /* Adjust switch size for mobile */
                    .switch {
                        width: 50px;
                        height: 28px;
                    }

                    .slider:before {
                        height: 20px;
                        width: 20px;
                        left: 4px;
                        bottom: 3px;
                    }

                    input:checked + .slider:before {
                        transform: translateX(22px);
                    }

                    /* Adjust waiting check inputs for mobile */
                    .waiting-check-fields {
                        width: 100%;
                    }

                    .waiting-check-fields input[type="text"],
                    .waiting-check-fields input[type="date"] {
                        width: 100%;
                        padding: 0.6rem 0.8rem;
                    }

                    .waiting-check-fields label {
                        font-size: 0.85rem;
                    }

                    /* Improve scrollbar for mobile */
                    .judgment-list::-webkit-scrollbar {
                        width: 4px;
                    }

                    /* Add some breathing room between sections */
                    .filters-container > * {
                        margin-bottom: 0.75rem;
                    }

                    /* Make headings more compact */
                    .filter-section h3 {
                        font-size: 1.1rem;
                        margin-bottom: 0.75rem;
                    }

                    /* Adjust the main container padding */
                    .proactive-container h2 {
                        font-size: 1.3rem;
                        margin: 0.5rem 0 1rem 0;
                    }
                }

                /* Additional breakpoint for very small screens */
                @media (max-width: 380px) {
                    .proactive-container {
                        padding: 0.25rem;
                    }

                    .filter-section,
                    .proactive-toggle-section {
                        padding: 0.75rem;
                    }

                    .proactive-title {
                        font-size: 0.9rem;
                    }

                    .notification-description {
                        font-size: 0.8rem;
                    }

                    .judgment-item {
                        padding: 0.6rem;
                    }
                }
                "#}
            </style>
        </div>
    }
}

#[derive(Properties, PartialEq)]
pub struct ProactiveProps {
    pub user_id: i32,
}

#[function_component(Proactive)]
pub fn proactive(props: &ProactiveProps) -> Html {
    html! {
        <ConnectedServices user_id={props.user_id} />
    }
}


