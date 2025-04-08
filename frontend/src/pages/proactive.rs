use yew::prelude::*;
use yew::{Properties, function_component};
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use web_sys::{window, HtmlInputElement, KeyboardEvent, InputEvent};
use serde_json::Number;
use chrono::Date;
use wasm_bindgen::JsValue;
use crate::profile::imap_general_checks::ImapGeneralChecks;


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
struct ConnectedService {
    service_type: String,
    identifier: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct WaitingCheck {
    content: String,
    due_date: i32,
    remove_when_found: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct PrioritySender {
    sender: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ImportancePriority {
    threshold: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct EmailJudgmentResponse {
    id: i32,
    email_timestamp: i32,
    processed_at: i32,
    should_notify: bool,
    score: i32,
    reason: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct ImportancePriorityResponse {
    user_id: i32,
    threshold: i32,
    service_type: String,
}

impl ImportancePriority {
    fn new(threshold: i32) -> Self {
        Self { threshold }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct FilterSettings {
    keywords: Vec<String>,
    priority_senders: Vec<PrioritySender>,
    waiting_checks: Vec<WaitingCheck>,
    importance_priority: Option<ImportancePriority>,
}

#[derive(Debug, Clone)]
struct ServiceState {
    service_type: String,
    identifier: String,
    filter_settings: Option<FilterSettings>,
}



#[derive(Properties, PartialEq)]
pub struct Props {
    pub user_id: i32,
}

// Helper functions
fn get_service_display_name(service_type: &str) -> String {
    match service_type {
        "imap" => "IMAP Email",
        "calendar" => "Google Calendar",
        _ => service_type,
    }.to_string()
}
use web_sys::js_sys;

fn format_timestamp(timestamp: i32) -> String {
    let date = js_sys::Date::new(&js_sys::Number::from(timestamp as f64 * 1000.0));
    let options = js_sys::Object::new();
    js_sys::Reflect::set(&options, &JsValue::from_str("year"), &JsValue::from_str("numeric")).unwrap();
    js_sys::Reflect::set(&options, &JsValue::from_str("month"), &JsValue::from_str("long")).unwrap();
    js_sys::Reflect::set(&options, &JsValue::from_str("day"), &JsValue::from_str("numeric")).unwrap();
    date.to_locale_string("en-US", &options).into()
}

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
    let new_keyword = use_state(|| String::new());
    let new_priority_sender = use_state(|| String::new());
    let filter_settings = use_state(|| None::<FilterSettings>);
    let new_waiting_check_content = use_state(|| String::new());
    let new_waiting_check_due_date = use_state(|| 0);
    let new_waiting_check_remove = use_state(|| false);

    let importance_value = use_state(|| {
        if let Some(service_type) = (*selected_service).clone() {
            if let Some(service) = (*services_state).iter().find(|s| s.service_type == service_type) {
                if let Some(settings) = &service.filter_settings {
                    if let Some(importance) = &settings.importance_priority {
                        return importance.threshold;
                    }
                }
            }
        }
        7
    });

    // Effect to update importance value when selected service changes
    {
        let importance_value = importance_value.clone();
        let services_state = services_state.clone();
        let selected_service = selected_service.clone();
        
        use_effect_with_deps(
            move |selected_service| {
                if let Some(service_type) = (*selected_service).as_ref() {
                    if let Some(service) = (*services_state).iter().find(|s| s.service_type == *service_type) {
                        if let Some(settings) = &service.filter_settings {
                            if let Some(importance) = &settings.importance_priority {
                                importance_value.set(importance.threshold);
                                return;
                            }
                        }
                    }
                }
                importance_value.set(7); // Default value
            },
            selected_service,
        );
    }
    let is_modified = use_state(|| false);
    let email_judgments = use_state(|| None::<Vec<EmailJudgmentResponse>>);

    // Fetch email judgments on mount
    {
        let email_judgments = email_judgments.clone();
        let error = error.clone();

        use_effect_with_deps(move |_| {
            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|storage| storage.get_item("token").ok())
                .flatten()
            {
                spawn_local(async move {
                    if let Ok(response) = Request::get(&format!("{}/api/profile/email-judgments", config::get_backend_url()))
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await
                    {
                        if let Ok(judgments) = response.json::<Vec<EmailJudgmentResponse>>().await {
                            email_judgments.set(Some(judgments));
                        } else {
                            error.set(Some("Failed to parse email judgments".to_string()));
                        }
                    } else {
                        error.set(Some("Failed to fetch email judgments".to_string()));
                    }
                });
            }
            || ()
        }, ());
    }
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

    // Fetch connected services and their keywords on mount
    {
        let services_state = services_state.clone();
        let error = error.clone();
        let selected_service = selected_service.clone();
        let importance_value = importance_value.clone();

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
                        if let Ok(services) = response.json::<Vec<ConnectedService>>().await {
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
                                        importance_value.set(fetched_priority.threshold);
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
                        <i class={classes!(
                            "service-icon",
                            format!("{}-icon", service.service_type)
                        )}></i>
                        <h3>{get_service_display_name(&service.service_type)}</h3>
                        <p class="service-identifier">{&service.identifier}</p>
                    </div>
                }
            }).collect::<Html>()
        }
    };

    html! {
        <div class="proactive-container">
            <h2>{"Proactive messaging"}</h2>
            /*
            <div class="service-boxes-container">
                {render_service_grid()}
            </div>
            */
            
            {
                if let Some(selected) = (*selected_service).clone() {
                    if let Some(service) = (*services_state).iter().find(|s| s.service_type == selected) {
                        if service.service_type == "calendar" {
                            html! {
                                <div class="coming-soon-container">
                                    <div class="coming-soon-content">
                                        <h3>{"Google Calendar Proactive Coming Soon!"}</h3>
                                        <p>{"We're working hard to bring you smart notifications for your Google Calendar events."}</p>
                                    </div>
                                </div>
                            }
                        } else if let Some(settings) = &service.filter_settings {
                            html! {
                                <div class="filters-container">
                                    <div class="proactive-toggle-section">
                                        <div class="notify-toggle">
                                        <img src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 512 512'%3E%3Cpath fill='%234285f4' d='M48 64C21.5 64 0 85.5 0 112c0 15.1 7.1 29.3 19.2 38.4L236.8 313.6c11.4 8.5 27 8.5 38.4 0L492.8 150.4c12.1-9.1 19.2-23.3 19.2-38.4c0-26.5-21.5-48-48-48H48zM0 176V384c0 35.3 28.7 64 64 64H448c35.3 0 64-28.7 64-64V176L294.4 339.2c-22.8 17.1-54 17.1-76.8 0L0 176z'/%3E%3C/svg%3E" alt="IMAP"/>
                                            <span class="proactive-title">{"Proactive EMAIL"}</span>
                                            <span class="toggle-status">
                                                {if *is_proactive { "Active" } else { "Inactive" }}
                                            </span>
                                            <label class="switch">
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
                                            {"Enable proactive notifications for IMAP email based on your filters."}
                                        </p>
                                    </div>
                                    <div class="filter-section">
                                        <h3>{"Filter Activity Log"}</h3>
                                        <div class="judgment-list">
                                            {
                                                if let Some(judgments) = &*email_judgments {
                                                    judgments.iter().map(|judgment| {
                                                        let date = format_timestamp(judgment.email_timestamp);
                                                        let processed_date = format_timestamp(judgment.processed_at);
                                                        html! {
                                                            <div class={classes!(
                                                                "judgment-item",
                                                                if judgment.should_notify { "notify" } else { "no-notify" }
                                                            )}>
                                                                <div class="judgment-header">
                                                                    <span class="judgment-date">{date}</span>
                                                                    <span class={classes!(
                                                                        "judgment-status",
                                                                        if judgment.should_notify { "notify" } else { "no-notify" }
                                                                    )}>
                                                                        {if judgment.should_notify { "Notified" } else { "Skipped" }}
                                                                    </span>
                                                                </div>
                                                                <div class="judgment-score">
                                                                    <span class="score-label">{"Importance Score: "}</span>
                                                                    <span class="score-value">{judgment.score}{" / 10"}</span>
                                                                </div>
                                                                <div class="judgment-reason">
                                                                    <span class="reason-label">{"Reason: "}</span>
                                                                    <span class="reason-text">{&judgment.reason}</span>
                                                                </div>
                                                                <div class="judgment-processed">
                                                                    <span class="processed-label">{"Processed: "}</span>
                                                                    <span class="processed-date">{processed_date}</span>
                                                                </div>
                                                            </div>
                                                        }
                                                    }).collect::<Html>()
                                                } else {
                                                    html! {
                                                        <div class="loading-judgments">
                                                            {"Loading filter activity..."}
                                                        </div>
                                                    }
                                                }
                                            }
                                        </div>
                                    </div>

                                    <div class="filter-section">
                                        <h3>{"Keywords"}</h3>
                                        <div class="keyword-input">
                                        <input
                                            type="text"
                                            placeholder="Add new keyword"
                                            value={(*new_keyword).clone()}
                                            onchange={
                                                let new_keyword = new_keyword.clone();
                                                move |e: Event| {
                                                    let input: HtmlInputElement = e.target_unchecked_into();
                                                    new_keyword.set(input.value());
                                                }
                                            }
                                            onkeypress={
                                                let new_keyword = new_keyword.clone();
                                                let services_state = services_state.clone();
                                                let selected_service = selected_service.clone();
                                                Callback::from(move |e: KeyboardEvent| {
                                                    if e.key() == "Enter" {
                                                        e.prevent_default();
                                                        let keyword = (*new_keyword).clone();
                                                        if !keyword.is_empty() {
                                                            if let Some(service_type) = (*selected_service).clone() {
                                                                let services_state = services_state.clone();
                                                                let new_keyword = new_keyword.clone();
                                                                wasm_bindgen_futures::spawn_local(async move {
                                                                    if let Some(token) = window()
                                                                        .and_then(|w| w.local_storage().ok())
                                                                        .flatten()
                                                                        .and_then(|storage| storage.get_item("token").ok())
                                                                        .flatten()
                                                                    {
                                                                        let request = Request::post(&format!("{}/api/filters/keyword/{}", config::get_backend_url(), service_type))
                                                                            .header("Authorization", &format!("Bearer {}", token))
                                                                            .json(&json!({ "keyword": keyword, "service_type": service_type.clone() }))
                                                                            .expect("Failed to build request");

                                                                        if let Ok(_) = request.send().await {
                                                                            // Refresh the keywords list after adding
                                                                            if let Ok(keywords_response) = Request::get(&format!("{}/api/filters/keywords/{}", config::get_backend_url(), service_type))
                                                                                .header("Authorization", &format!("Bearer {}", token))
                                                                                .send()
                                                                                .await
                                                                            {
                                                                                if let Ok(keywords) = keywords_response.json::<Vec<String>>().await {
                                                                                    let mut updated_services = (*services_state).clone();
                                                                                    if let Some(service) = updated_services.iter_mut().find(|s| s.service_type == service_type) {
                                                                                service.filter_settings = Some(FilterSettings {
                                                                                    keywords,
                                                                                    priority_senders: Vec::new(),
                                                                                    waiting_checks: Vec::new(),
                                                                                    importance_priority: None,
                                                                                });
                                                                            }
                                                                            services_state.set(updated_services);
                                                                            new_keyword.set(String::new());
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                            });
                                                            }
                                                        }
                                                    }
                                                })
                                            }
                                        />
                                        <button onclick={
                                            let new_keyword = new_keyword.clone();
                                            let services_state = services_state.clone();
                                            let selected_service = selected_service.clone();
                                            Callback::from(move |_| {
                                                let keyword = (*new_keyword).clone();
                                                if !keyword.is_empty() {
                                                    if let Some(service_type) = (*selected_service).clone() {
                                                        let services_state = services_state.clone();
                                                        let new_keyword = new_keyword.clone();
                                                        wasm_bindgen_futures::spawn_local(async move {
                                                            if let Some(token) = window()
                                                                .and_then(|w| w.local_storage().ok())
                                                                .flatten()
                                                                .and_then(|storage| storage.get_item("token").ok())
                                                                .flatten()
                                                            {
                                                                let request = Request::post(&format!("{}/api/filters/keyword/{}", config::get_backend_url(), service_type))
                                                                    .header("Authorization", &format!("Bearer {}", token))
                                                                    .json(&json!({ "keyword": keyword, "service_type": service_type.clone() }))
                                                                    .expect("Failed to build request");

                                                                if let Ok(_) = request.send().await {
                                                                    // Refresh the keywords list after adding
                                                                    if let Ok(keywords_response) = Request::get(&format!("{}/api/filters/keywords/{}", config::get_backend_url(), service_type))
                                                                        .header("Authorization", &format!("Bearer {}", token))
                                                                        .send()
                                                                        .await
                                                                    {
                                                                        if let Ok(keywords) = keywords_response.json::<Vec<String>>().await {
                                                                            let mut updated_services = (*services_state).clone();
                                                                            if let Some(service) = updated_services.iter_mut().find(|s| s.service_type == service_type) {
                                                                            service.filter_settings = Some(FilterSettings {
                                                                                    keywords,
                                                                                    priority_senders: Vec::new(),
                                                                                    waiting_checks: Vec::new(),
                                                                                    importance_priority: None,
                                                                                });
                                                                            }
                                                                            services_state.set(updated_services);
                                                                            new_keyword.set(String::new());
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        });
                                                    }
                                                }
                                            })
                                        }>{"Add"}</button>
                                </div>

                                        <ul class="keyword-list">
                                            {
                                                settings.keywords.iter().map(|keyword| {
                                                    let keyword_clone = keyword.clone();
                                                    let services_state = services_state.clone();
                                                    let selected_service = selected_service.clone();
                                                    html! {
                                                        <li class="keyword-item">
                                                            <span>{keyword}</span>
                                                            <button class="delete-btn" onclick={
                                                                let keyword = keyword_clone.clone();
                                                                let services_state = services_state.clone();
                                                                let selected_service = selected_service.clone();
                                                                Callback::from(move |_| {
                                                                    let keyword = keyword.clone();
                                                                    let services_state = services_state.clone();
                                                                    let selected_service = selected_service.clone();
                                                                    
                                                                    if let Some(service_type) = (*selected_service).clone() {
                                                                        wasm_bindgen_futures::spawn_local(async move {
                                                                            if let Some(token) = window()
                                                                                .and_then(|w| w.local_storage().ok())
                                                                                .flatten()
                                                                                .and_then(|storage| storage.get_item("token").ok())
                                                                                .flatten()
                                                                            {
                                                                                let request = Request::delete(&format!("{}/api/filters/keyword/{}/{}", config::get_backend_url(), service_type, keyword))
                                                                                    .header("Authorization", &format!("Bearer {}", token))
                                                                                    .send()
                                                                                    .await;

                                                                                if let Ok(_) = request {
                                                                                    // Refresh the keywords list after deleting
                                                                                    if let Ok(keywords_response) = Request::get(&format!("{}/api/filters/keywords/{}", config::get_backend_url(), service_type))
                                                                                        .header("Authorization", &format!("Bearer {}", token))
                                                                                        .send()
                                                                                        .await
                                                                                    {
                                                                                        if let Ok(keywords) = keywords_response.json::<Vec<String>>().await {
                                                                                            let mut updated_services = (*services_state).clone();
                                                                                            if let Some(service) = updated_services.iter_mut().find(|s| s.service_type == service_type) {
                                                                                service.filter_settings = Some(FilterSettings {
                                                                                    keywords,
                                                                                    priority_senders: Vec::new(),
                                                                                    waiting_checks: Vec::new(),
                                                                                    importance_priority: None,
                                                                                });

                                                                                            }
                                                                                            services_state.set(updated_services);
                                                                                        }

                                                                                    }
                                                                                }
                                                                            }
                                                                        });
                                                                    }
                                                                })
                                                            }>{"×"}</button>
                                                        </li>
                                                    }
                                                }).collect::<Html>()
                                            }
                                        </ul>
                                    </div>

                                    <div class="filter-section">
                                        <h3>{"Priority Senders"}</h3>
                                        <div class="filter-input">
                                            <input
                                                type="text"
                                                placeholder="Add priority sender"
                                                value={(*new_priority_sender).clone()}
                                                onchange={
                                                    let new_priority_sender = new_priority_sender.clone();
                                                    move |e: Event| {
                                                        let input: HtmlInputElement = e.target_unchecked_into();
                                                        new_priority_sender.set(input.value());
                                                    }
                                                }
                                                onkeypress={
                                                    let new_priority_sender = new_priority_sender.clone();
                                                    let services_state = services_state.clone();
                                                    let selected_service = selected_service.clone();
                                                    Callback::from(move |e: KeyboardEvent| {
                                                        if e.key() == "Enter" {
                                                            e.prevent_default();
                                                            let sender = (*new_priority_sender).clone();
                                                            if !sender.is_empty() {
                                                                if let Some(service_type) = (*selected_service).clone() {
                                                                    let services_state = services_state.clone();
                                                                    let new_priority_sender = new_priority_sender.clone();
                                                                    wasm_bindgen_futures::spawn_local(async move {
                                                                        if let Some(token) = window()
                                                                            .and_then(|w| w.local_storage().ok())
                                                                            .flatten()
                                                                            .and_then(|storage| storage.get_item("token").ok())
                                                                            .flatten()
                                                                        {
                                                                            let request = Request::post(&format!("{}/api/filters/priority-sender/{}", config::get_backend_url(), service_type))
                                                                                .header("Authorization", &format!("Bearer {}", token))
                                                                                .json(&json!({ "sender": sender, "service_type": service_type.clone() }))
                                                                                .expect("Failed to build request");

                                                                            if let Ok(_) = request.send().await {
                                                                                // Refresh the priority senders list after adding
                                                                                if let Ok(senders_response) = Request::get(&format!("{}/api/filters/priority-senders/{}", config::get_backend_url(), service_type))
                                                                                    .header("Authorization", &format!("Bearer {}", token))
                                                                                    .send()
                                                                                    .await
                                                                                {
                                                                                    if let Ok(senders) = senders_response.json::<Vec<PrioritySender>>().await {
                                                                                        let mut updated_services = (*services_state).clone();
                                                                                        if let Some(service) = updated_services.iter_mut().find(|s| s.service_type == service_type) {
                                                                                            if let Some(settings) = &mut service.filter_settings {
                                                                                                settings.priority_senders = senders;
                                                                                            }
                                                                                        }
                                                                                        services_state.set(updated_services);
                                                                                        new_priority_sender.set(String::new());
                                                                                    }
                                                                                }
                                                                            }
                                                                        }
                                                                    });
                                                                }
                                                            }
                                                        }
                                                    })
                                                }
                                            />
                                            <button onclick={
                                                let new_priority_sender = new_priority_sender.clone();
                                                let services_state = services_state.clone();
                                                let selected_service = selected_service.clone();
                                                Callback::from(move |_| {
                                                    let sender = (*new_priority_sender).clone();
                                                    if !sender.is_empty() {
                                                        if let Some(service_type) = (*selected_service).clone() {
                                                            let services_state = services_state.clone();
                                                            let new_priority_sender = new_priority_sender.clone();
                                                            wasm_bindgen_futures::spawn_local(async move {
                                                                if let Some(token) = window()
                                                                    .and_then(|w| w.local_storage().ok())
                                                                    .flatten()
                                                                    .and_then(|storage| storage.get_item("token").ok())
                                                                    .flatten()
                                                                {
                                                                    let request = Request::post(&format!("{}/api/filters/priority-sender/{}", config::get_backend_url(), service_type))
                                                                        .header("Authorization", &format!("Bearer {}", token))
                                                                        .json(&json!({ "sender": sender, "service_type": service_type.clone() }))
                                                                        .expect("Failed to build request");

                                                                    if let Ok(_) = request.send().await {
                                                                        // Refresh the priority senders list after adding
                                                                        if let Ok(senders_response) = Request::get(&format!("{}/api/filters/priority-senders/{}", config::get_backend_url(), service_type))
                                                                            .header("Authorization", &format!("Bearer {}", token))
                                                                            .send()
                                                                            .await
                                                                        {
                                                                            if let Ok(senders) = senders_response.json::<Vec<PrioritySender>>().await {
                                                                                let mut updated_services = (*services_state).clone();
                                                                                if let Some(service) = updated_services.iter_mut().find(|s| s.service_type == service_type) {
                                                                                    if let Some(settings) = &mut service.filter_settings {
                                                                                        settings.priority_senders = senders;
                                                                                    }
                                                                                }
                                                                                services_state.set(updated_services);
                                                                                new_priority_sender.set(String::new());
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            });
                                                        }
                                                    }
                                                })
                                            }>{"Add"}</button>
                                        </div>
                                        <ul class="filter-list">
                                            {
                                                settings.priority_senders.iter().map(|sender| {
                                                    let sender_clone = sender.sender.clone();
                                                    let services_state = services_state.clone();
                                                    let selected_service = selected_service.clone();
                                                    html! {
                                                        <li class="filter-item">
                                                            <span>{&sender.sender}</span>
                                                            <button class="delete-btn" onclick={
                                                                let sender = sender_clone.clone();
                                                                let services_state = services_state.clone();
                                                                let selected_service = selected_service.clone();
                                                                Callback::from(move |_| {
                                                                    let sender = sender.clone();
                                                                    let services_state = services_state.clone();
                                                                    let selected_service = selected_service.clone();
                                                                    
                                                                    if let Some(service_type) = (*selected_service).clone() {
                                                                        wasm_bindgen_futures::spawn_local(async move {
                                                                            if let Some(token) = window()
                                                                                .and_then(|w| w.local_storage().ok())
                                                                                .flatten()
                                                                                .and_then(|storage| storage.get_item("token").ok())
                                                                                .flatten()
                                                                            {
                                                                                let request = Request::delete(&format!("{}/api/filters/priority-sender/{}/{}", config::get_backend_url(), service_type, sender))
                                                                                    .header("Authorization", &format!("Bearer {}", token))
                                                                                    .send()
                                                                                    .await;

                                                                                if let Ok(_) = request {
                                                                                    // Refresh the priority senders list after deleting
                                                                                    if let Ok(senders_response) = Request::get(&format!("{}/api/filters/priority-senders/{}", config::get_backend_url(), service_type))
                                                                                        .header("Authorization", &format!("Bearer {}", token))
                                                                                        .send()
                                                                                        .await
                                                                                    {
                                                                                        if let Ok(senders) = senders_response.json::<Vec<PrioritySender>>().await {
                                                                                            let mut updated_services = (*services_state).clone();
                                                                                            if let Some(service) = updated_services.iter_mut().find(|s| s.service_type == service_type) {
                                                                                                if let Some(settings) = &mut service.filter_settings {
                                                                                                    settings.priority_senders = senders;
                                                                                                }
                                                                                            }
                                                                                            services_state.set(updated_services);
                                                                                        }
                                                                                    }
                                                                                }
                                                                            }
                                                                        });
                                                                    }
                                                                })
                                                            }>{"×"}</button>
                                                        </li>
                                                    }
                                                }).collect::<Html>()
                                            }
                                        </ul>
                                    </div>

                                    <div class="filter-section">
                                        <h3>{"Waiting Checks"}</h3>
                                        <div class="waiting-check-input">
                                            <div class="waiting-check-fields">
                                                <input
                                                    type="text"
                                                    placeholder="Content to wait for"
                                                    value={(*new_waiting_check_content).clone()}
                                                    onchange={
                                                        let new_waiting_check_content = new_waiting_check_content.clone();
                                                        move |e: Event| {
                                                            let input: HtmlInputElement = e.target_unchecked_into();
                                                            new_waiting_check_content.set(input.value());
                                                        }
                                                    }
                                                />
                                                <label class="date-label">
                                                    <input
                                                        type="date"
                                                        value={format_date_for_input(*new_waiting_check_due_date)}
                                                    onchange={
                                                        let new_waiting_check_due_date = new_waiting_check_due_date.clone();
                                                        move |e: Event| {
                                                            let input: HtmlInputElement = e.target_unchecked_into();
                                                            if let Ok(timestamp) = parse_date_to_timestamp(&input.value()) {
                                                                new_waiting_check_due_date.set(timestamp);
                                                            }
                                                        }
                                                    }
                                                    />
                                                </label>
                                                <label>
                                                    <input 
                                                        type="checkbox"
                                                        checked={*new_waiting_check_remove}
                                                        onchange={
                                                            let new_waiting_check_remove = new_waiting_check_remove.clone();
                                                            move |e: Event| {
                                                                let input: HtmlInputElement = e.target_unchecked_into();
                                                                new_waiting_check_remove.set(input.checked());
                                                            }
                                                        }
                                                    />
                                                    {"Remove when found"}
                                                </label>
                                            </div>
                                            <button onclick={
                                                let new_waiting_check_content = new_waiting_check_content.clone();
                                                let new_waiting_check_due_date = new_waiting_check_due_date.clone();
                                                let new_waiting_check_remove = new_waiting_check_remove.clone();
                                                let services_state = services_state.clone();
                                                let selected_service = selected_service.clone();
                                                Callback::from(move |_| {
                                                    let content = (*new_waiting_check_content).clone();
                                                    if !content.is_empty() {
                                                        if let Some(service_type) = (*selected_service).clone() {
                                                            let services_state = services_state.clone();
                                                            let new_waiting_check_content = new_waiting_check_content.clone();
                                                            let new_waiting_check_due_date = new_waiting_check_due_date.clone();
                                                            let new_waiting_check_remove = new_waiting_check_remove.clone();
                                                            wasm_bindgen_futures::spawn_local(async move {
                                                                if let Some(token) = window()
                                                                    .and_then(|w| w.local_storage().ok())
                                                                    .flatten()
                                                                    .and_then(|storage| storage.get_item("token").ok())
                                                                    .flatten()
                                                                {
                                                                    let request = Request::post(&format!("{}/api/filters/waiting-check/{}", config::get_backend_url(), service_type))
                                                                        .header("Authorization", &format!("Bearer {}", token))
                                                                        .json(&json!({
                                                                            "waiting_type": "content",
                                                                            "content": content,
                                                                            "due_date": *new_waiting_check_due_date,
                                                                            "remove_when_found": *new_waiting_check_remove,
                                                                            "service_type": service_type.clone()
                                                                        }))
                                                                        .expect("Failed to build request");

                                                                    if let Ok(_) = request.send().await {
                                                                        // Refresh the waiting checks list after adding
                                                                        if let Ok(checks_response) = Request::get(&format!("{}/api/filters/waiting-checks/{}", config::get_backend_url(), service_type))
                                                                            .header("Authorization", &format!("Bearer {}", token))
                                                                            .send()
                                                                            .await
                                                                        {
                                                                            if let Ok(checks) = checks_response.json::<Vec<WaitingCheck>>().await {
                                                                                let mut updated_services = (*services_state).clone();
                                                                                if let Some(service) = updated_services.iter_mut().find(|s| s.service_type == service_type) {
                                                                                    if let Some(settings) = &mut service.filter_settings {
                                                                                        settings.waiting_checks = checks;
                                                                                    }
                                                                                }
                                                                                services_state.set(updated_services);
                                                                                new_waiting_check_content.set(String::new());
                                                                                new_waiting_check_due_date.set(0);
                                                                                new_waiting_check_remove.set(false);
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            });
                                                        }
                                                    }
                                                })
                                            }>{"Add"}</button>
                                        </div>
                                        <ul class="filter-list">
                                            {
                                                settings.waiting_checks.iter().map(|check| {
                                                    let content_clone = check.content.clone();
                                                    let services_state = services_state.clone();
                                                    let selected_service = selected_service.clone();
                                                    html! {
                                                        <li class="filter-item">
                                                            <span>{&check.content}</span>
                                                            <span class="due-date">{format_timestamp(check.due_date)}</span>
                                                            <span class="remove-when-found">{
                                                                if check.remove_when_found {
                                                                    "Remove when found"
                                                                } else {
                                                                    "Keep after found"
                                                                }
                                                            }</span>
                                                            <button class="delete-btn" onclick={
                                                                let content = content_clone.clone();
                                                                let services_state = services_state.clone();
                                                                let selected_service = selected_service.clone();
                                                                Callback::from(move |_| {
                                                                    let content = content.clone();
                                                                    let services_state = services_state.clone();
                                                                    let selected_service = selected_service.clone();
                                                                    
                                                                    if let Some(service_type) = (*selected_service).clone() {
                                                                        wasm_bindgen_futures::spawn_local(async move {
                                                                            if let Some(token) = window()
                                                                                .and_then(|w| w.local_storage().ok())
                                                                                .flatten()
                                                                                .and_then(|storage| storage.get_item("token").ok())
                                                                                .flatten()
                                                                            {
                                                                                let request = Request::delete(&format!("{}/api/filters/waiting-check/{}/{}", config::get_backend_url(), service_type, content))
                                                                                    .header("Authorization", &format!("Bearer {}", token))
                                                                                    .send()
                                                                                    .await;

                                                                                if let Ok(_) = request {
                                                                                    // Refresh the waiting checks list after deleting
                                                                                    if let Ok(checks_response) = Request::get(&format!("{}/api/filters/waiting-checks/{}", config::get_backend_url(), service_type))
                                                                                        .header("Authorization", &format!("Bearer {}", token))
                                                                                        .send()
                                                                                        .await
                                                                                    {
                                                                                        if let Ok(checks) = checks_response.json::<Vec<WaitingCheck>>().await {
                                                                                            let mut updated_services = (*services_state).clone();
                                                                                            if let Some(service) = updated_services.iter_mut().find(|s| s.service_type == service_type) {
                                                                                                if let Some(settings) = &mut service.filter_settings {
                                                                                                    settings.waiting_checks = checks;
                                                                                                }
                                                                                            }
                                                                                            services_state.set(updated_services);
                                                                                        }
                                                                                    }
                                                                                }
                                                                            }
                                                                        });
                                                                    }
                                                                })
                                                            }>{"×"}</button>
                                                        </li>
                                                    }
                                                }).collect::<Html>()
                                            }
                                        </ul>
                                    </div>

                                    <div class="filter-section">

                                        <h3>{"Importance Priority"}</h3>

                                        <div class="filter-input">
                                            <div class="importance-input-group">
                                                <input
                                                    type="number"
                                                    min="1"
                                                    max="10"
                                                    placeholder="Priority threshold (1-10)"
                                                    value={(*importance_value).to_string()}
                                                oninput={
                                                    let importance_value = importance_value.clone();
                                                    let is_modified = is_modified.clone();
                                                    Callback::from(move |e: InputEvent| {
                                                        let input: HtmlInputElement = e.target_unchecked_into();
                                                        let new_value = input.value().parse::<i32>().unwrap_or(7);
                                                        if new_value != *importance_value {
                                                            importance_value.set(new_value);
                                                            is_modified.set(true);
                                                        }
                                                    })
                                                }
                                                />
                                                <span class="priority-label">{"out of 10"}</span>
                                            </div>
                                            {
                                                if *is_modified {
                                                    html! {
                                                        <button
                                                            class="save-btn"
                                                            onclick={
                                                                let services_state = services_state.clone();
                                                                let selected_service = selected_service.clone();
                                                                let importance_value = importance_value.clone();
                                                                let is_modified = is_modified.clone();
                                                                Callback::from(move |_| {
                                                                    if let Some(service_type) = (*selected_service).clone() {
                                                                        let services_state = services_state.clone();
                                                                        let threshold = *importance_value;
                                                                        let importance_value = importance_value.clone();
                                                                        let is_modified = is_modified.clone();
                                                                        
                                                                        wasm_bindgen_futures::spawn_local(async move {
                                                                            if let Some(token) = window()
                                                                                .and_then(|w| w.local_storage().ok())
                                                                                .flatten()
                                                                                .and_then(|storage| storage.get_item("token").ok())
                                                                                .flatten()
                                                                            {
                                                                                let request = Request::post(&format!("{}/api/filters/importance-priority/{}", config::get_backend_url(), service_type))
                                                                                    .header("Authorization", &format!("Bearer {}", token))
                                                                                    .json(&json!({
                                                                                        "threshold": threshold,
                                                                                        "service_type": service_type.clone()
                                                                                    }))
                                                                                    .expect("Failed to build request");

                                                                                if let Ok(_) = request.send().await {
                                                                                    // Update the state
                                                                                    let mut updated_services = (*services_state).clone();
                                                                                    if let Some(service) = updated_services.iter_mut().find(|s| s.service_type == service_type) {
                                                                                        if let Some(settings) = &mut service.filter_settings {
                                                                                            settings.importance_priority = Some(ImportancePriority {
                                                                                                threshold,
                                                                                            });
                                                                                            // Update the importance value state to match the service
                                                                                            importance_value.set(threshold);
                                                                                        }
                                                                                    }
                                                                                    services_state.set(updated_services);
                                                                                    is_modified.set(false);
                                                                                }
                                                                            }
                                                                        });
                                                                    }
                                                                })
                                                            }
                                                        >
                                                            {"Save"}
                                                        </button>
                                                    }
                                                } else {
                                                    html! {}
                                                }
                                            }

                                            <style>
                                                {r#"
                                                .coming-soon-container {
                                                    background: rgba(30, 30, 30, 0.5);
                                                    border: 1px solid rgba(30, 144, 255, 0.1);
                                                    border-radius: 12px;
                                                    padding: 2rem;
                                                    text-align: center;
                                                    margin-top: 2rem;
                                                    backdrop-filter: blur(10px);
                                                }

                                                .coming-soon-content {
                                                    max-width: 600px;
                                                    margin: 0 auto;
                                                }

                                                .coming-soon-content h3 {
                                                    color: #7EB2FF;
                                                    font-size: 1.8rem;
                                                    margin-bottom: 1rem;
                                                }

                                                .coming-soon-content p {
                                                    color: rgba(255, 255, 255, 0.8);
                                                    font-size: 1.1rem;
                                                    line-height: 1.6;
                                                    margin-bottom: 2rem;
                                                }

                                                .features-preview {
                                                    background: rgba(30, 144, 255, 0.05);
                                                    border: 1px solid rgba(30, 144, 255, 0.1);
                                                    border-radius: 8px;
                                                    padding: 1.5rem;
                                                    text-align: left;
                                                }

                                                .features-preview h4 {
                                                    color: #7EB2FF;
                                                    font-size: 1.2rem;
                                                    margin-bottom: 1rem;
                                                }

                                                .features-preview ul {
                                                    list-style: none;
                                                    padding: 0;
                                                    margin: 0;
                                                }

                                                .features-preview li {
                                                    color: #fff;
                                                    font-size: 1rem;
                                                    margin-bottom: 0.8rem;
                                                    padding-left: 1.5rem;
                                                    position: relative;
                                                }

                                                .features-preview li:before {
                                                    content: "→";
                                                    position: absolute;
                                                    left: 0;
                                                    color: #7EB2FF;
                                                }

                                                @media (max-width: 768px) {
                                                    .coming-soon-container {
                                                        padding: 1.5rem;
                                                        margin: 1rem;
                                                    }

                                                    .coming-soon-content h3 {
                                                        font-size: 1.5rem;
                                                    }

                                                    .coming-soon-content p {
                                                        font-size: 1rem;
                                                    }
                                                }
                                                "#}
                                            </style>
                                        </div>
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
                    html! {}
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

            {
                if let Some(selected) = (*selected_service).clone() {
                    if selected == "imap" {
                        if let Some(service) = (*services_state).iter().find(|s| s.service_type == "imap") {
                            if let Some(settings) = &service.filter_settings {
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
                                    <ImapGeneralChecks 
                                        on_update={Callback::from(|_| {})}
                                        keywords={settings.keywords.clone()}
                                        priority_senders={priority_senders}
                                        waiting_checks={waiting_checks}
                                        threshold={threshold}
                                    />
                                }
                            } else {
                                html! {
                                    <ImapGeneralChecks 
                                        on_update={Callback::from(|_| {})}
                                        keywords={vec![]}
                                        priority_senders={vec![]}
                                        waiting_checks={vec![]}
                                        threshold={7}
                                    />
                                }
                            }
                        } else {
                            html! {
                                <ImapGeneralChecks 
                                    on_update={Callback::from(|_| {})}
                                    keywords={vec![]}
                                    priority_senders={vec![]}
                                    waiting_checks={vec![]}
                                    threshold={7}
                                />
                            }
                        }
                    } else {
                        html! {}
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

