use yew::prelude::*;
use web_sys::{MouseEvent, HtmlInputElement};
use serde_json::json;
use wasm_bindgen_futures::spawn_local;
use wasm_bindgen::JsValue;
use crate::config;
use gloo_net::http::Request;
use web_sys::UrlSearchParams;
use web_sys::js_sys::Date;
use crate::connections::whatsapp::WhatsappConnect;
use crate::connections::calendar::CalendarConnect;
use crate::connections::email::EmailConnect;
use crate::connections::tasks::TasksConnect;
use crate::connections::telegram::TelegramConnect;
use crate::connections::uber::UberConnect;
use crate::connections::signal::SignalConnect;

#[derive(Properties, PartialEq)]
pub struct ConnectProps {
    pub user_id: i32,
    pub sub_tier: Option<String>,
    pub discount: bool,
    pub phone_number: String,
    pub estimated_monitoring_cost: f32,
}
/*
pub struct Connect {
    pub user_id: i32,
}
*/

#[derive(Clone, PartialEq)]
struct ServiceGroupState {
    expanded: bool,
    service_count: usize,
    connected_count: usize,
}


#[function_component(Connect)]
pub fn connect(props: &ConnectProps) -> Html {
    let error = use_state(|| None::<String>);
    let connecting = use_state(|| false);
    let calendar_connected = use_state(|| false);
    let memory_connected = use_state(|| false);
    let email_connected = use_state(|| false);
    let whatsapp_connected = use_state(|| false);
    let telegram_connected = use_state(|| false);

    {
        let calendar_connected = calendar_connected.clone();
        let memory_connected = memory_connected.clone();
        let email_connected = email_connected.clone();
        let whatsapp_connected = whatsapp_connected.clone();
        let telegram_connected= telegram_connected.clone();
        use_effect_with_deps(
            move |_| {
                if let Some(window) = web_sys::window() {
                    if let Ok(Some(storage)) = window.local_storage() {
                        if let Ok(Some(token)) = storage.get_item("token") {
                            // Calendar status check
                            spawn_local({
                                let calendar_connected = calendar_connected.clone();
                                let token = token.clone();
                                async move {
                                    if let Ok(response) = Request::get(&format!("{}/api/auth/google/calendar/status", config::get_backend_url()))
                                        .header("Authorization", &format!("Bearer {}", token))
                                        .send()
                                        .await
                                    {
                                        if let Ok(data) = response.json::<serde_json::Value>().await {
                                            if let Some(connected) = data.get("connected").and_then(|v| v.as_bool()) {
                                                calendar_connected.set(connected);
                                            }
                                        }
                                    }
                                }
                            });

                            // Memory (Tasks) status check
                            spawn_local({
                                let memory_connected = memory_connected.clone();
                                let token = token.clone();
                                async move {
                                    if let Ok(response) = Request::get(&format!("{}/api/auth/google/tasks/status", config::get_backend_url()))
                                        .header("Authorization", &format!("Bearer {}", token))
                                        .send()
                                        .await
                                    {
                                        if let Ok(data) = response.json::<serde_json::Value>().await {
                                            if let Some(connected) = data.get("connected").and_then(|v| v.as_bool()) {
                                                memory_connected.set(connected);
                                            }
                                        }
                                    }
                                }
                            });

                            // Email status check
                            spawn_local({
                                let email_connected = email_connected.clone();
                                let token = token.clone();
                                async move {
                                    if let Ok(response) = Request::get(&format!("{}/api/auth/imap/status", config::get_backend_url()))
                                        .header("Authorization", &format!("Bearer {}", token))
                                        .send()
                                        .await
                                    {
                                        if let Ok(data) = response.json::<serde_json::Value>().await {
                                            if let Some(connected) = data.get("connected").and_then(|v| v.as_bool()) {
                                                email_connected.set(connected);
                                            }
                                        }
                                    }
                                }
                            });
                            // whatsapp status check
                            spawn_local({
                                let whatsapp_connected = whatsapp_connected.clone();
                                let token = token.clone();
                                async move {
                                    if let Ok(response) = Request::get(&format!("{}/api/auth/whatsapp/status", config::get_backend_url()))
                                        .header("Authorization", &format!("Bearer {}", token))
                                        .send()
                                        .await
                                    {
                                        if let Ok(data) = response.json::<serde_json::Value>().await {
                                            if let Some(connected) = data.get("connected").and_then(|v| v.as_bool()) {
                                                whatsapp_connected.set(connected);
                                            }
                                        }
                                    }
                                }
                            });
                            // telegram status check
                            spawn_local({
                                let telegram_connected = telegram_connected.clone();
                                let token = token.clone();
                                async move {
                                    if let Ok(response) = Request::get(&format!("{}/api/auth/telegram/status", config::get_backend_url()))
                                        .header("Authorization", &format!("Bearer {}", token))
                                        .send()
                                        .await
                                    {
                                        if let Ok(data) = response.json::<serde_json::Value>().await {
                                            if let Some(connected) = data.get("connected").and_then(|v| v.as_bool()) {
                                                telegram_connected.set(connected);
                                            }
                                        }
                                    }
                                }
                            });
                        }
                    }
                }
                || ()
            },
            (),
        );
    }
    

    let group_states = use_state(|| {
        let mut map = std::collections::HashMap::new();
        map.insert("tools", ServiceGroupState { expanded: false, service_count: 4, connected_count: 2 });
        map.insert("apps", ServiceGroupState { expanded: false, service_count: 4, connected_count: 0 });
        map.insert("proactive", ServiceGroupState { expanded: false, service_count: 4, connected_count: 0 });
        map
    });


    // Predefined providers (you can expand this list)
    let providers = vec![
        ("gmail", "Gmail", "imap.gmail.com", "993"),
        ("privateemail", "PrivateEmail", "mail.privateemail.com", "993"),
        ("outlook", "Outlook", "imap-mail.outlook.com", "993"),
        ("custom", "Custom", "", ""), // Custom option with empty defaults
    ];

    
    // Check token on component mount
    use_effect_with_deps(
        |_| {
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    if let Ok(Some(token)) = storage.get_item("token") {
                        web_sys::console::log_1(&format!("Token found in localStorage: {}", token).into());
                    } else {
                        web_sys::console::log_1(&"No token found in localStorage".into());
                    }
                }
            }
            || ()
        },
        (),
    );

    // Clean URL parameters if present (post-callback)
    use_effect_with_deps(
        move |_| {
            if let Some(window) = web_sys::window() {
                if let Ok(search) = window.location().search() {
                    if !search.is_empty() {
                        let params = UrlSearchParams::new_with_str(&search).unwrap();
                        if params.get("code").is_some() || params.get("state").is_some() {
                            web_sys::console::log_1(&"Detected callback parameters, cleaning URL".into());
                            if let Ok(history) = window.history() {
                                let _ = history.push_state_with_url(
                                    &JsValue::NULL,
                                    "",
                                    Some(&window.location().pathname().unwrap_or_default()),
                                );
                            }
                        }
                    }
                }
            }
            || ()
        },
        (),
    );

    let currency = if props.phone_number.starts_with("+1") || props.phone_number.starts_with("+61") {
        "$"
    } else if props.phone_number.starts_with("+358") || props.phone_number.starts_with("+44") {
        "€"
    } else {
        "$"
    };
            html! {
                <div class="connect-section">

                    // Apps 
                    <div class={classes!("service-group", 
                        if props.sub_tier.as_deref() != Some("tier 2") && props.sub_tier.as_deref() != Some("tier 1.5") && props.sub_tier.as_deref() != Some("self_hosted") && !props.discount { "apps-disabled" } else { "" }
                    )}>
                        <h3 class="service-group-title"
                            onclick={
                                let group_states = group_states.clone();
                                let can_expand = props.sub_tier.as_deref() == Some("tier 2") || props.sub_tier.as_deref() == Some("self_hosted") || props.discount;
                                Callback::from(move |_| {
                                    if can_expand {
                                        let mut new_states = (*group_states).clone();
                                        if let Some(state) = new_states.get_mut("apps") {
                                            state.expanded = !state.expanded;
                                        }
                                        group_states.set(new_states);
                                    }
                                })
                            }
                        >
                            <i class="fa-solid fa-plug"></i>
                            {"Apps"}
                            <div class="group-summary">
                                <span class="service-count">
                                    {format!("{}/5 Connected", 
                                        (if *calendar_connected { 1 } else { 0 }) +
                                        (if *memory_connected { 1 } else { 0 }) +
                                        (if *email_connected { 1 } else { 0 }) +
                                        (if *whatsapp_connected { 1 } else { 0 }) +
                                        (if *telegram_connected { 1 } else { 0 })
                                    )}
                                </span>
                                <i class={if group_states.get("apps").map(|s| s.expanded).unwrap_or(false) {
                                    "fas fa-chevron-up"
                                } else {
                                    "fas fa-chevron-down"
                                }}></i>
                            </div>
                        </h3>
                        <div class={classes!(
                            "service-list",
                            if group_states.get("apps").map(|s| s.expanded).unwrap_or(false) {
                                "expanded"
                            } else {
                                "collapsed"
                            }
                        )}>
                            <CalendarConnect 
                                user_id={props.user_id} 
                                sub_tier={props.sub_tier.clone()} 
                                discount={props.discount}
                                on_connection_change={{
                                    let group_states = group_states.clone();
                                    Some(Callback::from(move |connected: bool| {
                                        let mut new_states = (*group_states).clone();
                                        if let Some(state) = new_states.get_mut("calendar") {
                                            state.connected_count = if connected { 1 } else { 0 };
                                        }
                                        group_states.set(new_states);
                                    }))
                                }}
                            />

                            <TasksConnect 
                                user_id={props.user_id}
                                sub_tier={props.sub_tier.clone()}
                                discount={props.discount}
                            />

                            <EmailConnect 
                                user_id={props.user_id}
                                sub_tier={props.sub_tier.clone()}
                                discount={props.discount}
                            />
                            <WhatsappConnect 
                                user_id={props.user_id} 
                                sub_tier={props.sub_tier.clone()} 
                                discount={props.discount}
                            />
                            <TelegramConnect 
                                user_id={props.user_id} 
                                sub_tier={props.sub_tier.clone()} 
                                discount={props.discount}
                            />
                            <SignalConnect 
                                user_id={props.user_id} 
                                sub_tier={props.sub_tier.clone()} 
                                discount={props.discount}
                            />
                            <UberConnect
                                user_id={props.user_id} 
                                sub_tier={props.sub_tier.clone()} 
                                discount={props.discount}
                            />
                        </div>
                    </div>

                    // Proactive Services
                    <div class={classes!("service-group", 
                        if props.sub_tier.as_deref() != Some("tier 2") && props.sub_tier.as_deref() != Some("self_hosted") { "monitoring-disabled" } else { "" }
                    )}>
                    <h3 class="service-group-title"
                        onclick={let group_states = group_states.clone();
                            Callback::from(move |_| {
                                let mut new_states = (*group_states).clone();
                                if let Some(state) = new_states.get_mut("proactive") {
                                    state.expanded = !state.expanded;
                                }
                                group_states.set(new_states);
                            })
                        }
                    >
                        <i class="fa-solid fa-robot"></i>
                        {"Monitoring"}
                        <div class="group-summary">
                            <span class="service-count">
                                {format!("{}/5 Available",
                                    if *email_connected || *calendar_connected || *whatsapp_connected || *telegram_connected { 5 } else { 0 }
                                )}
                            </span>
                            <span class="monitoring-cost">
                                {
                                    if props.phone_number.starts_with("+1") {
                                        html! {
                                            format!("Est. {:.2} Messages/mo", props.estimated_monitoring_cost)
                                        }
                                    } else if props.phone_number.starts_with("+358") || props.phone_number.starts_with("+44") || props.phone_number.starts_with("+61") {
                                        html! {
                                            format!("Est. {}{:.2}/mo", currency, props.estimated_monitoring_cost)
                                        }
                                    } else {
                                        html! {}
                                    }
                                }
                            </span>
                            <i class={if group_states.get("proactive").map(|s| s.expanded).unwrap_or(false) {
                                "fas fa-chevron-up"
                            } else {
                                "fas fa-chevron-down"
                            }}></i>
                        </div>
                    </h3>
                        <div class={classes!(
                            "service-list",
                            if group_states.get("proactive").map(|s| s.expanded).unwrap_or(false) {
                                "expanded"
                            } else {
                                "collapsed"
                            }
                        )}>
                        // Proactive Agent Section
                        <div class={classes!(
                            "service-item",
                            if !(*email_connected || *calendar_connected || *whatsapp_connected || *telegram_connected) { "inactive" } else { "" }
                        )}>
                            {
                                if !(*email_connected || *calendar_connected || *whatsapp_connected || *telegram_connected) {
                                    html! {
                                        <div class="feature-overlay">
                                            <div class="overlay-content" style="color: #999;">
                                                <i class="fas fa-lock"></i>
                                                <p>{"Connect Email, Calendar, or WhatsApp to use Proactive Agent"}</p>
                                            </div>
                                        </div>
                                    }
                                } else {
                                    html! {
                                        <crate::proactive::agent_on::ProactiveAgentSection/>
                                    }
                                }
                            }
                        </div>
                            // Critical Section
                            <div class={classes!(
                                "service-item",
                                if !(*email_connected || *calendar_connected || *whatsapp_connected || *telegram_connected) { "inactive" } else { "" }
                            )}>
                                {
                                    if !(*email_connected || *calendar_connected || *whatsapp_connected || *telegram_connected) {
                                        html! {
                                            <div class="feature-overlay">
                                                <div class="overlay-content" style="color: #999;">
                                                    <i class="fas fa-lock"></i>
                                                    <p>{"Connect Email, Calendar, or WhatsApp to use Critical Notifications"}</p>
                                                </div>
                                            </div>
                                        }
                                    } else {
                                        html! {
                                            <crate::proactive::critical::CriticalSection
                                                phone_number={props.phone_number.clone()}
                                                />
                                        }
                                    }
                                }
                            </div>

                            // Digest Section
                            <div class={classes!(
                                "service-item",
                                if !(*email_connected || *calendar_connected || *whatsapp_connected || *telegram_connected) { "inactive" } else { "" }
                            )}>
                                {
                                    if !(*email_connected || *calendar_connected || *whatsapp_connected || *telegram_connected) {
                                        html! {
                                            <div class="feature-overlay">
                                                <div class="overlay-content" style="color: #999;">
                                                    <i class="fas fa-lock"></i>
                                                    <p>{"Connect Email, Calendar, or WhatsApp to use Digest"}</p>
                                                </div>
                                            </div>
                                        }
                                    } else {
                                        html! {
                                            <crate::proactive::digest::DigestSection
                                                phone_number={props.phone_number.clone()}
                                                />
                                        }
                                    }
                                }
                            </div>

                            // Waiting Checks Section
                            <div class={classes!(
                                "service-item",
                                if !(*email_connected || *whatsapp_connected || *telegram_connected) { "inactive" } else { "" }
                            )}>
                                {
                                    if !(*email_connected || *whatsapp_connected || *telegram_connected) {
                                        html! {
                                            <div class="feature-overlay">
                                                <div class="overlay-content" style="color: #999;">
                                                    <i class="fas fa-lock"></i>
                                                    <p>{"Connect Email or some messaging app like WhatsApp to use Waiting Checks"}</p>
                                                </div>
                                            </div>
                                        }
                                    } else {
                                        html! {
                                            <crate::proactive::waiting_checks::WaitingChecksSection
                                                service_type={"messaging".to_string()}
                                                checks={Vec::new()}
                                                on_change={Callback::from(|_| ())}
                                                phone_number={props.phone_number.clone()}
                                            />
                                        }
                                    }
                                }
                            </div>

                            // Monitored Contacts Section
                            <div class={classes!(
                                "service-item",
                                if !(*email_connected || *whatsapp_connected || *telegram_connected) { "inactive" } else { "" }
                            )}>
                                {
                                    if !(*email_connected || *whatsapp_connected || *telegram_connected) {
                                        html! {
                                            <div class="feature-overlay">
                                                <div class="overlay-content" style="color: #999;">
                                                    <i class="fas fa-lock"></i>
                                                    <p>{"Connect Some Messaging App or Email to use Monitored Contacts"}</p>
                                                </div>
                                            </div>
                                        }
                                    } else {
                                        html! {
                                            <crate::proactive::constant_monitoring::MonitoredContactsSection
                                                service_type={"email".to_string()}
                                                contacts={Vec::new()}
                                                on_change={Callback::from(|_| ())}
                                                phone_number={props.phone_number.clone()}
                                            />
                                        }
                                    }
                                }
                            </div>
                        </div>
                    </div>

                    // Tools 
                    <div class="service-group">
                        <h3 class="service-group-title" 
                            onclick={let group_states = group_states.clone();
                                Callback::from(move |_| {
                                    let mut new_states = (*group_states).clone();
                                    if let Some(state) = new_states.get_mut("tools") {
                                        state.expanded = !state.expanded;
                                    }
                                    group_states.set(new_states);
                                })
                            }

                        >
                            <i class="fa-solid fa-hammer"></i>
                            {"Tools"}
                            <div class="group-summary">
                                <span class="service-count">
                                {
                                    if props.sub_tier.as_deref() != Some("self_hosted") && props.sub_tier.as_deref() != Some("tier 2") { {"5 tools ready!"} } else { "7 tools ready!" }
                                }
                                </span>
                                <i class={if group_states.get("tools").map(|s| s.expanded).unwrap_or(false) {
                                    "fas fa-chevron-up"
                                } else {
                                    "fas fa-chevron-down"
                                }}></i>
                            </div>
                        </h3>
<div class={classes!(
                            "service-list",
                            if group_states.get("tools").map(|s| s.expanded).unwrap_or(false) {
                                "expanded"
                            } else {
                                "collapsed"
                            }
                        )}>
                            // Perplexity
                            <div class="service-item">
                                <div class="service-header">
                                    <div class="service-name">
                                        <img src="https://www.perplexity.ai/favicon.ico" alt="Perplexity"  width="24" height="24"/>
                                        {"Perplexity AI"}
                                    </div>
                                </div>
                                <p class="service-description">
                                    {"Ask any question and get accurate, AI-powered answers through SMS or voice calls. Perplexity helps you find information, solve problems, and learn new things."}
                                </p>
                            </div>

                            // Weather
                            <div class="service-item">
                                <div class="service-header">
                                    <div class="service-name">
                                        {"☀️ Weather"}
                                    </div>
                                </div>
                                <p class="service-description">
                                    {"Get instant weather updates and forecasts for any location through SMS or voice calls. Provides current conditions."}
                                </p>
                            </div>

                            // Directions
                            <div class="service-item">
                                <div class="service-header">
                                    <div class="service-name">
                                        <i class="fas fa-directions" style="color: #1E90FF; font-size: 24px; margin-right: 8px;"></i>
                                        {"Get Directions"}
                                    </div>
                                </div>
                                <p class="service-description">
                                    {"Get step-by-step directions between any two addresses through SMS or voice calls. Includes estimated travel time, distance, and turn-by-turn navigation for walking, biking, driving, or public transit."}
                                </p>
                            </div>

                            // QR Code Scanner
                            <div class="service-item">
                                <div class="service-header">
                                    <div class="service-name">
                                        <i class="fas fa-qrcode" style="color: #1E90FF; font-size: 24px; margin-right: 8px;"></i>
                                        {"QR Code Scanner"}

                                    </div>
                                    <button class="info-button" onclick={Callback::from(|_| {
                                        if let Some(element) = web_sys::window()
                                            .and_then(|w| w.document())
                                            .and_then(|d| d.get_element_by_id("qr-scanner-info"))
                                        {
                                            let display = element.get_attribute("style")
                                                .unwrap_or_else(|| "display: none".to_string());
                                            
                                            if display.contains("none") {
                                                let _ = element.set_attribute("style", "display: block");
                                            } else {
                                                let _ = element.set_attribute("style", "display: none");
                                            }
                                        }
                                    })}>
                                        {"ⓘ"}
                                    </button>
                                </div>
                                <p class="service-description">
                                    {"Send a photo of any QR code through SMS and receive its contents instantly. For URLs, you can then either type them manually or have them automatically forwarded to your email if you're using The Light Phone. Note: Photo messaging (MMS) is only available in countries where Twilio supports MMS, including the US and Australia."}
                                </p>
                                <div id="qr-scanner-info" class="info-section" style="display: none">
                                    <h4>{"How It Works"}</h4>
                                    <div class="info-subsection">
                                        <ul>
                                            <li>{"1. Take a photo of any QR code"}</li>
                                            <li>{"2. Send the photo to lightfriend via SMS"}</li>
                                            <li>{"3. Receive the decoded content in seconds"}</li>
                                            <li>{"4. For URLs: The Light Phone users get them automatically forwarded to email"}</li>
                                        </ul>
                                    </div>
                                </div>
                            </div>
                            // Photo Translation
                            <div class="service-item">
                                <div class="service-header">
                                    <div class="service-name">
                                        {"🔤 Photo Translation"}

                                    </div>
                                    <button class="info-button" onclick={Callback::from(|_| {
                                        if let Some(element) = web_sys::window()
                                            .and_then(|w| w.document())
                                            .and_then(|d| d.get_element_by_id("photo-translation-info"))
                                        {
                                            let display = element.get_attribute("style")
                                                .unwrap_or_else(|| "display: none".to_string());
                                            
                                            if display.contains("none") {
                                                let _ = element.set_attribute("style", "display: block");
                                            } else {
                                                let _ = element.set_attribute("style", "display: none");
                                            }
                                        }
                                    })}>
                                        {"ⓘ"}
                                    </button>
                                </div>
                                <p class="service-description">
                                    {"Send a photo of text in any language and receive its translation instantly via SMS. Perfect for menus, signs, documents, or any text you need to understand quickly. Note: Photo messaging (MMS) is only available in countries where Twilio supports MMS, including the US and Australia."}
                                </p>
                                <div id="photo-translation-info" class="info-section" style="display: none">
                                    <h4>{"How It Works"}</h4>
                                    <div class="info-subsection">
                                        <ul>
                                            <li>{"1. Send a photo containing text to lightfriend"}</li>
                                            <li>{"2. Specify the target language (or it will default to English)"}</li>
                                            <li>{"3. Receive the translated text via SMS within seconds"}</li>
                                        </ul>
                                    </div>
                                </div>
                            </div>

                            // Waiting Checks
                            <div class={classes!("service-item",
                                if props.sub_tier.as_deref() != Some("tier 2") && props.sub_tier.as_deref() != Some("self_hosted") && !props.discount { "disabled" } else { "" }
                            )}>
                                {
                                    if props.sub_tier.as_deref() != Some("tier 2") && props.sub_tier.as_deref() != Some("self_hosted") && !props.discount {
                                        html! {
                                            <div class="feature-overlay">
                                                <div class="overlay-content" style="color: #999;">
                                                    <i class="fas fa-lock"></i>
                                                    <p>{"Upgrade Hosted Plan to use Waiting Checks here"}</p>
                                                </div>
                                            </div>
                                        }
                                    } else {
                                        html! {
                                            <>
                                                <div class="service-header">
                                                    <div class="service-name">
                                                        {"⏰ Waiting Checks"}
                                                    </div>
                                                    <button class="info-button" onclick={Callback::from(|_| {
                                                        if let Some(element) = web_sys::window()
                                                            .and_then(|w| w.document())
                                                            .and_then(|d| d.get_element_by_id("waiting-checks-info"))
                                                        {
                                                            let display = element.get_attribute("style")
                                                                .unwrap_or_else(|| "display: none".to_string());
                                                            
                                                            if display.contains("none") {
                                                                let _ = element.set_attribute("style", "display: block");
                                                            } else {
                                                                let _ = element.set_attribute("style", "display: none");
                                                            }
                                                        }
                                                    })}>
                                                        {"ⓘ"}
                                                    </button>
                                                </div>
                                                <p class="service-description">
                                                    {"Set up notifications for when you're waiting for something from emails or messaging apps. Get a message when it's time to check on what you're waiting for."}
                                                </p>
                                                <div id="waiting-checks-info" class="info-section" style="display: none">
                                                    <h4>{"How It Works"}</h4>
                                                    <div class="info-subsection">
                                                        <ul>
                                                            <li>{"1. Tell lightfriend what you're waiting for and from where (Messaging apps or email)"}</li>
                                                            <li>{"2. When lightfriend notices the event, it sends you a text and removes the waiting check"}</li>
                                                        </ul>
                                                    </div>
                                                </div>
                                            </>
                                        }
                                    }
                                }
                            </div>

                            // SMS During Calls
                            <div class="service-item">
                                <div class="service-header">
                                    <div class="service-name">
                                        {"📱 SMS During Calls"}
                                    </div>
                                    <button class="info-button" onclick={Callback::from(|_| {
                                        if let Some(element) = web_sys::window()
                                            .and_then(|w| w.document())
                                            .and_then(|d| d.get_element_by_id("sms-during-calls-info"))
                                        {
                                            let display = element.get_attribute("style")
                                                .unwrap_or_else(|| "display: none".to_string());
                                            
                                            if display.contains("none") {
                                                let _ = element.set_attribute("style", "display: block");
                                            } else {
                                                let _ = element.set_attribute("style", "display: none");
                                            }
                                        }
                                    })}>
                                        {"ⓘ"}
                                    </button>
                                </div>
                                <p class="service-description">
                                    {"Send information via SMS while you're on a voice call with lightfriend. Perfect for getting details you need to write down or remember."}
                                </p>
                                <div id="sms-during-calls-info" class="info-section" style="display: none">
                                    <h4>{"How It Works"}</h4>
                                    <div class="info-subsection">
                                        <ul>
                                            <li>{"1. During any voice call with lightfriend"}</li>
                                            <li>{"2. Ask for information to be sent via SMS"}</li>
                                            <li>{"3. Continue your conversation while receiving the info"}</li>
                                            <li>{"4. Check your messages after the call for the details"}</li>
                                        </ul>
                                    </div>
                                </div>
                            </div>
                        </div>
                    </div>

                    if let Some(err) = (*error).as_ref() {
                        <div class="error-message">
                            {err}
                        </div>
                    }
<style>
        {r#"
.group-summary {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 1rem;
    font-size: 0.9rem;
    color: #999;
}

.service-count {

    padding: 0.25rem 0.75rem;
    border-radius: 12px;
    font-size: 0.8rem;
}

/* Apps */
.service-group:nth-child(1) .service-count {
    background: rgba(96, 165, 250, 0.1);
    color: #60A5FA;
}

/* Monitoring */
.service-group:nth-child(2) .service-count {
    background: rgba(52, 211, 153, 0.1);
    color: #34D399;
}

/* Tools */
.service-group:nth-child(3) .service-count {
    background: rgba(169, 169, 169, 0.1);
    color: #A9A9A9;
}

.monitoring-cost {
    padding: 0.25rem 0.75rem;
    border-radius: 12px;
    font-size: 0.8rem;
    background: rgba(52, 211, 153, 0.1);
    color: #34D399;
}


.service-group-title {
    cursor: pointer;
    user-select: none;
    transition: all 0.3s ease;
}

.service-group-title:hover {
    color: #1E90FF;
}

.service-group-title i.fa-chevron-up,
.service-group-title i.fa-chevron-down {
    font-size: 0.8rem;
    transition: transform 0.3s ease;
}

.service-group-title:hover i.fa-chevron-up,
.service-group-title:hover i.fa-chevron-down {
    transform: translateY(-2px);
}

.service-list {
    transition: all 0.3s ease-in-out;
    overflow: hidden;
}

.service-list.collapsed {
    max-height: 0;
    opacity: 0;
    margin: 0;
    padding: 0;
}

.service-list.expanded {
    max-height: 5000px;
    opacity: 1;
    margin-top: 1.5rem;
}

.service-group {
    margin-bottom: 2rem;
    background: rgba(30, 30, 30, 0.7);
    border: 1px solid rgba(30, 144, 255, 0.1);
    border-radius: 16px;
    padding: 1.5rem;
    backdrop-filter: blur(10px);
    width: 100%;
    box-sizing: border-box;
    position: relative;
}


.service-group.apps-disabled::after {
    content: "Upgrade to Hosted Plan to access Apps here";
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    color: #999;
    font-size: 1.1rem;
    border-radius: 16px;
    backdrop-filter: blur(4px);
}

.service-group.monitoring-disabled::after {
    content: "Upgrade to Hosted Plan to access Monitoring here";
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    color: #999;
    font-size: 1.1rem;
    border-radius: 16px;
    backdrop-filter: blur(4px);
}

.service-group-title {
    font-size: 1.2rem;

    margin: 0;
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.5rem;
    border-radius: 8px;
    cursor: pointer;
    transition: all 0.3s ease;
}

/* Apps - Blue */
.service-group:nth-child(1) .service-group-title {
    color: #60A5FA;
}
.service-group:nth-child(1) .service-group-title:hover {
    background: rgba(96, 165, 250, 0.1);
}

/* Monitoring - Green */
.service-group:nth-child(2) .service-group-title {
    color: #34D399; /* Adjusted to a green shade to match the hover rgba(52, 211, 153, 0.1) */
}
.service-group:nth-child(2) .service-group-title:hover {
    background: rgba(52, 211, 153, 0.1);
}

/* Tools - Silver */
.service-group:nth-child(3) .service-group-title {
    color: #A9A9A9;
}
.service-group:nth-child(3) .service-group-title:hover {
    background: rgba(169, 169, 169, 0.1);
}


.service-group-title:hover {
    background: rgba(30, 144, 255, 0.1);
}

.service-item {
    background: rgba(0, 0, 0, 0.2);
    border: 1px solid rgba(30, 144, 255, 0.1);
    border-radius: 12px;
    padding: 1.5rem;
    margin-bottom: 1rem;
    transition: all 0.3s ease;
}

.service-item:last-child {
    margin-bottom: 0;
}

.service-item:hover {
    transform: translateY(-2px);
    border-color: rgba(30, 144, 255, 0.2);
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.2);
}

.service-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 1rem;
}

.service-name {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    font-size: 1.1rem;
    color: #fff;
}

.service-description {
    color: #999;
    font-size: 0.95rem;
    line-height: 1.5;
    margin-bottom: 1rem;
}
{"
.connect-section {
    max-width: 800px;
    margin: 0;
    padding: 0;
    width: 100%;
    box-sizing: border-box;
}

.service-group {
    margin-bottom: 2.5rem;
    background: rgba(30, 30, 30, 0.7);
    border: 1px solid rgba(30, 144, 255, 0.1);
    border-radius: 16px;
    padding: 2rem;
    backdrop-filter: blur(10px);
    width: 100%;
    box-sizing: border-box;
}

@media (max-width: 768px) {
    .service-group {
        padding: 1rem;
        margin-bottom: 1.5rem;
    }
    
    .service-item {
        padding: 1rem;
    }
    
    .service-header {
        flex-direction: column;
        align-items: flex-start;
        gap: 0.5rem;
    }
    
    .service-status-container {
        width: 100%;
        display: flex;
        flex-direction: column;
        gap: 0.25rem;
    }
    
    .imap-form input,
    .imap-form select {
        width: 100%;
        box-sizing: border-box;
    }
}

.info-button:hover {
    background: rgba(30, 144, 255, 0.1);
    transform: scale(1.1);
}

.info-section {
    background: rgba(30, 144, 255, 0.05);
    border-radius: 8px;
    margin-top: 1rem;
    border: 1px solid rgba(30, 144, 255, 0.1);
}

.info-section p {
    color: #CCC;
    margin: 0 0 0.5rem 0;
}

.info-section ul {
    margin: 0;
    color: #999;
}

.info-section li {
    margin: 0.5rem 0;
}

.fas.fa-cloud-sun,
.fas.fa-qrcode,
.fas.fa-search {
    display: inline-block;
    width: 24px;
    text-align: center;
}

.service-group-title {
    font-size: 1.4rem;
    color: #7EB2FF;
    margin-bottom: 1.5rem;
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding-bottom: 1rem;
    border-bottom: 1px solid rgba(30, 144, 255, 0.1);
}

.service-list {
    display: grid;
    gap: 1.5rem;
    width: 100%;
    box-sizing: border-box;
}

.service-item {
    background: rgba(0, 0, 0, 0.2);
    border: 1px solid rgba(30, 144, 255, 0.2);
    border-radius: 12px;
    padding: 1.5rem;
    transition: all 0.3s ease;
    width: 100%;
    box-sizing: border-box;
    overflow-wrap: break-word;
    word-wrap: break-word;
    word-break: break-word;
}

.service-item:hover {
    transform: translateY(-2px);
    border-color: rgba(30, 144, 255, 0.4);
    box-shadow: 0 4px 20px rgba(30, 144, 255, 0.1);
}

.service-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 1rem;
}

.service-name {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    font-size: 1.1rem;
    color: #fff;
}

.service-name img {
    width: 20px !important;
    height: 20px !important;
    object-fit: contain;
    vertical-align: middle;
}

.service-status {
    font-size: 0.9rem;
    color: #7EB2FF;
    display: flex;
    align-items: center;
    gap: 0.5rem;
}

.service-description {
    color: #999;
    font-size: 0.95rem;
    line-height: 1.5;
    margin-bottom: 1.5rem;
}

.connect-button, .disconnect-button {
    width: 100%;
    padding: 0.75rem;
    border-radius: 8px;
    font-size: 0.95rem;
    cursor: pointer;
    transition: all 0.3s ease;
    text-align: center;
    border: none;
}

.connect-button {
    background: linear-gradient(45deg, #1E90FF, #4169E1);
    color: white;
}

.connect-button:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 20px rgba(30, 144, 255, 0.3);
}

.disconnect-button {
    background: transparent;
    border: 1px solid rgba(255, 99, 71, 0.3);
    color: #FF6347;
}

.disconnect-button:hover {
    background: rgba(255, 99, 71, 0.1);
    border-color: rgba(255, 99, 71, 0.5);
}

.error-message {
    color: #FF6347;
    background: rgba(255, 99, 71, 0.1);
    border: 1px solid rgba(255, 99, 71, 0.2);
    padding: 1rem;
    border-radius: 8px;
    margin-top: 1rem;
    font-size: 0.9rem;
}

.coming-soon {
    opacity: 0.5;
    pointer-events: none;
}

.coming-soon-tag {
    background: rgba(30, 144, 255, 0.1);
    color: #1E90FF;
    font-size: 0.8rem;
    padding: 0.25rem 0.75rem;
    border-radius: 12px;
    margin-left: 0.75rem;
}

.pro-tag {
    background: linear-gradient(45deg, #FFD700, #FFA500);
    color: #000;
    font-size: 0.8rem;
    padding: 0.25rem 0.75rem;
    border-radius: 12px;
    margin-left: 0.75rem;
    font-weight: bold;
    text-shadow: 0 1px 1px rgba(255, 255, 255, 0.5);
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
}

.test-button {
    background: rgba(76, 175, 80, 0.2);
    color: #4CAF50;
    border: 1px solid rgba(76, 175, 80, 0.3);
    padding: 0.5rem 1rem;
    border-radius: 6px;
    margin-top: 0.75rem;
    cursor: pointer;
    transition: all 0.3s ease;
}

.test-button:hover {
    background: rgba(76, 175, 80, 0.3);
    border-color: rgba(76, 175, 80, 0.4);
}

.calendar-connect-options {
                            display: flex;
                            flex-direction: column;
                            gap: 10px;
                            margin-top: 10px;
                        }
                        .calendar-checkbox {
                            display: flex;
                            align-items: center;
                            gap: 8px;
                            font-size: 14px;
                            color: #666;
                            cursor: pointer;
                        }
                        .calendar-checkbox input[type='checkbox'] {
                            width: 16px;
                            height: 16px;
                            cursor: pointer;
                        }
                        .service-status-container {
                            display: flex;
                            align-items: center;
                            gap: 8px;
                        }
                        .connected-email {
                            font-size: 0.9em;
                            color: #666;
                            font-style: italic;
                        }
                        .gmail-controls {
                            display: flex;
                            gap: 10px;
                            margin-top: 10px;
                        }
                        .test-button {
                            background-color: #4CAF50;
                            color: white;
                            padding: 8px 16px;
                            border: none;
                            border-radius: 4px;
                            cursor: pointer;
                            margin-left: 10px;
                            font-size: 14px;
                        }
                        .test-button:hover {
                            background-color: #45a049;
                        }

                        .service-group {
                            margin-bottom: 2rem;
                        }

                        .service-group:last-child {
                            margin-bottom: 0;
                        }

                        .service-group-title {
                            color: #7EB2FF;
                            font-size: 1.2rem;
                            margin-bottom: 1rem;
                            display: flex;
                            align-items: center;
                            gap: 0.5rem;
                        }

                        .service-group-title i {
                            font-size: 1.1rem;
                        }

                        .service-list {
                            display: grid;
                            gap: 1rem;
                        }

                        .service-item {
                            background: rgba(0, 0, 0, 0.2);
                            border: 1px solid rgba(30, 144, 255, 0.2);
                            border-radius: 8px;
                            padding: 1.5rem;
                            transition: all 0.3s ease;
                        }

                        .service-item:hover {
                            border-color: rgba(30, 144, 255, 0.4);
                            transform: translateY(-2px);
                        }

                        .service-header {
                            display: flex;
                            align-items: center;
                            justify-content: space-between;
                            margin-bottom: 1rem;
                        }

                        .service-name {
                            display: flex;
                            align-items: center;
                            gap: 0.75rem;
                            color: #fff;
                            font-size: 1.1rem;
                        }

                        .service-name img {
                            width: 24px;
                            height: 24px;
                        }

                        .service-status {
                            font-size: 0.9rem;
                            color: #666;
                        }

                        .service-description {
                            color: #999;
                            font-size: 0.9rem;
                            margin-bottom: 1.5rem;
                            line-height: 1.4;
                        }

                        .connect-button {
                            background: linear-gradient(45deg, #1E90FF, #4169E1);
                            color: white;
                            border: none;
                            padding: 0.75rem 1.5rem;
                            border-radius: 6px;
                            font-size: 0.9rem;
                            cursor: pointer;
                            transition: all 0.3s ease;
                            display: inline-flex;
                            align-items: center;
                            gap: 0.5rem;
                            width: 100%;
                            justify-content: center;
                        }

                        .connect-button:hover {
                            transform: translateY(-2px);
                            box-shadow: 0 4px 20px rgba(30, 144, 255, 0.3);
                        }

                        .connect-button.connected {
                            background: rgba(30, 144, 255, 0.1);
                            border: 1px solid rgba(30, 144, 255, 0.3);
                            color: #1E90FF;
                        }

                        .connect-button.connected:hover {
                            background: rgba(30, 144, 255, 0.15);
                        }

                        .disconnect-button {
                            background: transparent;
                            border: 1px solid rgba(255, 99, 71, 0.3);
                            color: #FF6347;
                            padding: 0.75rem 1.5rem;
                            border-radius: 6px;
                            font-size: 0.9rem;
                            cursor: pointer;
                            transition: all 0.3s ease;
                            margin-top: 0.5rem;
                            width: 100%;
                        }

                        .disconnect-button:hover {
                            background: rgba(255, 99, 71, 0.1);
                            border-color: rgba(255, 99, 71, 0.5);
                        }

                        .coming-soon {
                            opacity: 0.5;
                            pointer-events: none;
                        }

                        .coming-soon-tag {
                            background: rgba(30, 144, 255, 0.1);
                            color: #1E90FF;
                            font-size: 0.8rem;
                            padding: 0.25rem 0.5rem;
                            border-radius: 4px;
                            margin-left: 0.5rem;
                        }

                        .error-message {
                            color: #FF6347;
                            font-size: 0.9rem;
                            margin-top: 1rem;
                            padding: 0.75rem;
                            background: rgba(255, 99, 71, 0.1);
                            border-radius: 6px;
                            border: 1px solid rgba(255, 99, 71, 0.2);
                        }

                        /* Waiting Checks Section Styles */
                        .filter-section {
                            background: rgba(0, 0, 0, 0.2);
                            border: 1px solid rgba(30, 144, 255, 0.2);
                            border-radius: 12px;
                            padding: 1.5rem;
                            margin-bottom: 1rem;
                        }

                        .filter-section.inactive {
                            opacity: 0.7;
                        }

                        .filter-header {
                            display: flex;
                            align-items: center;
                            justify-content: space-between;
                            margin-bottom: 1rem;
                        }

                        .filter-header h3 {
                            margin: 0;
                            color: #F59E0B;
                            font-size: 1.1rem;
                        }

                        .waiting-check-input {
                            display: flex;
                            gap: 1rem;
                            margin-bottom: 1rem;
                        }

                        .waiting-check-fields {
                            flex: 1;
                            display: flex;
                            gap: 1rem;
                            align-items: center;
                        }

                        .waiting-check-fields input[type="text"] {
                            flex: 1;
                            padding: 0.75rem;
                            border-radius: 8px;
                            border: 1px solid rgba(30, 144, 255, 0.2);
                            background: rgba(0, 0, 0, 0.2);
                            color: #fff;
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
                            border: 1px solid rgba(30, 144, 255, 0.2);
                            background: rgba(0, 0, 0, 0.2);
                            color: #fff;
                        }

                        .waiting-check-input button {
                            padding: 0.75rem 1.5rem;
                            border-radius: 8px;
                            border: none;
                            background: linear-gradient(45deg, #F59E0B, #D97706);
                            color: white;
                            cursor: pointer;
                            transition: all 0.3s ease;
                        }

                        .waiting-check-input button:hover {
                            transform: translateY(-2px);
                            box-shadow: 0 4px 20px rgba(245, 158, 11, 0.3);
                        }

                        .filter-list {
                            list-style: none;
                            padding: 0;
                            margin: 0;
                        }

                        .filter-list li {
                            display: flex;
                            align-items: center;
                            justify-content: space-between;
                            padding: 0.75rem;
                            background: rgba(0, 0, 0, 0.2);
                            border: 1px solid rgba(30, 144, 255, 0.1);
                            border-radius: 8px;
                            margin-bottom: 0.5rem;
                            color: #fff;
                        }

                        .filter-list li:last-child {
                            margin-bottom: 0;
                        }

                        .filter-list .due-date {
                            font-size: 0.9rem;
                            color: #999;
                            margin-left: 1rem;
                        }

                        .filter-list .remove-when-found {
                            font-size: 0.8rem;
                            color: #F59E0B;
                            margin-left: 1rem;
                        }

                        .filter-list .delete-btn {
                            background: none;
                            border: none;
                            color: #FF6347;
                            font-size: 1.2rem;
                            cursor: pointer;
                            padding: 0.25rem 0.5rem;
                            border-radius: 4px;
                            transition: all 0.3s ease;
                        }

                        .filter-list .delete-btn:hover {
                            background: rgba(255, 99, 71, 0.1);
                        }

                        .toggle-container {
                            display: flex;
                            align-items: center;
                            gap: 0.75rem;
                        }

                        .toggle-label {
                            font-size: 0.9rem;
                            color: #999;
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
                            background-color: rgba(0, 0, 0, 0.2);
                            transition: .4s;
                            border: 1px solid rgba(30, 144, 255, 0.2);
                        }

                        .slider:before {
                            position: absolute;
                            content: "";
                            height: 16px;
                            width: 16px;
                            left: 4px;
                            bottom: 3px;
                            background-color: white;
                            transition: .4s;
                        }

                        input:checked + .slider {
                            background-color: #F59E0B;
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

                        /* Feature Section Styles */
                        .feature-section {
                            position: relative;
                            background: rgba(30, 30, 30, 0.7);
                            border: 1px solid rgba(30, 144, 255, 0.1);
                            border-radius: 16px;
                            padding: 2rem;
                            margin-bottom: 2rem;
                            backdrop-filter: blur(10px);
                            transition: all 0.3s ease;
                        }

                        .feature-section.inactive {
                            opacity: 0.7;
                            filter: grayscale(50%);
                        }

                        .feature-overlay {
                            position: absolute;
                            top: 0;
                            left: 0;
                            right: 0;
                            bottom: 0;
                            background: rgba(0, 0, 0, 0.7);
                            backdrop-filter: blur(4px);
                            border-radius: 16px;
                            color: #999;
                            font-size: 0.9rem;
                            display: flex;
                            align-items: center;
                            justify-content: center;
                            z-index: 10;
                        }

                        .overlay-content {
                            text-align: center;
                            color: #999;
                            font-size: 0.9rem;
                            padding: 2rem;
                        }

                        .overlay-content i {
                            font-size: 2rem;
                            color: #999 !important;
                            margin-bottom: 1rem;
                        }

                        .overlay-content p {
                            font-size: 1.1rem;
                            margin: 0;
                            color: #999 !important;
                        }

                        @media (max-width: 768px) {
                            .connect-section {
                                padding: 0;
                                margin: 0;
                            }

                            .service-list {
                                grid-template-columns: 1fr;
                            }

                            .service-item {
                                padding: 1rem;
                            }

                            .feature-section {
                                padding: 1rem;
                            }

                            .overlay-content {
                                padding: 1rem;
                            }

                            .overlay-content i {
                                font-size: 1.5rem;
                            }

                            .overlay-content p {
                                font-size: 1rem;
                            }
                        }


                        "#}
                    </style>
                </div>
            }
}
