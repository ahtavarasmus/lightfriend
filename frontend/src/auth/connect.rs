use yew::prelude::*;
use web_sys::{MouseEvent, HtmlInputElement};
use serde_json::json;
use wasm_bindgen_futures::spawn_local;
use wasm_bindgen::JsValue;
use crate::config;
use gloo_net::http::Request;
use web_sys::UrlSearchParams;
use web_sys::js_sys::Date;
use crate::auth::whatsapp::WhatsappConnect;

#[derive(Properties, PartialEq)]
pub struct ConnectProps {
    pub user_id: i32,
}

#[function_component(Connect)]
pub fn connect(props: &ConnectProps) -> Html {
    let error = use_state(|| None::<String>);
    let connecting = use_state(|| false);
    let whatsapp_connected = use_state(|| false);
    let whatsapp_connecting = use_state(|| false);
    let whatsapp_qr_code = use_state(|| None::<String>);
    let calendar_connected = use_state(|| false);
    let gmail_connected = use_state(|| false);
    let imap_connected = use_state(|| false);
    let all_calendars = use_state(|| false);
    let imap_email = use_state(|| String::new());
    let imap_password = use_state(|| String::new());
    let imap_provider = use_state(|| "gmail".to_string()); // Default to Gmail
    let imap_server = use_state(|| String::new()); // For custom provider
    let imap_port = use_state(|| String::new());   // For custom provider
    let connected_email = use_state(|| None::<String>);

    // Predefined providers (you can expand this list)
    let providers = vec![
        ("gmail", "Gmail", "imap.gmail.com", "993"),
        ("privateemail", "PrivateEmail", "mail.privateemail.com", "993"),
        ("outlook", "Outlook", "imap-mail.outlook.com", "993"),
        ("custom", "Custom", "", ""), // Custom option with empty defaults
    ];

    // Check connection status on component mount
    {
        let calendar_connected = calendar_connected.clone();
        let gmail_connected = gmail_connected.clone();
        let imap_connected = imap_connected.clone();
        let connected_email = connected_email.clone();
        use_effect_with_deps(
            move |_| {
                if let Some(window) = web_sys::window() {
                    if let Ok(Some(storage)) = window.local_storage() {
                        if let Ok(Some(token)) = storage.get_item("token") {
                            // Check Google Calendar status
                            {
                                let calendar_connected = calendar_connected.clone();
                                let token = token.clone();
                                spawn_local(async move {
                                    let request = Request::get(&format!("{}/api/auth/google/calendar/status", config::get_backend_url()))
                                        .header("Authorization", &format!("Bearer {}", token))
                                        .send()
                                        .await;

                                    if let Ok(response) = request {
                                if response.ok() {
                                            if let Ok(data) = response.json::<serde_json::Value>().await {
                                                if let Some(connected) = data.get("connected").and_then(|v| v.as_bool()) {
                                                    calendar_connected.set(connected);
                                                }
                                            }
                                        } else {
                                            web_sys::console::log_1(&"Failed to check calendar status".into());
                                        }
                                    }
                                });
                            }

                            // imap status

                            {
                                let imap_connected = imap_connected.clone();
                                let connected_email = connected_email.clone();
                                let token = token.clone();
                                spawn_local(async move {
                                    let request = Request::get(&format!("{}/api/auth/imap/status", config::get_backend_url()))
                                        .header("Authorization", &format!("Bearer {}", token))
                                        .send()
                                        .await;

                                    if let Ok(response) = request {
                                        if response.ok() {
                                            if let Ok(data) = response.json::<serde_json::Value>().await {
                                                if let Some(connected) = data.get("connected").and_then(|v| v.as_bool()) {
                                                    imap_connected.set(connected);
                                                    if connected {
                                                        connected_email.set(data.get("email").and_then(|e| e.as_str()).map(String::from));
                                                    } else {
                                                        connected_email.set(None);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                });
                            }
                                                        
                        }
                    }
                }
                || ()
            },
            (),
        );
    }

    // Handlers for input changes
    let onchange_imap_email = {
        let imap_email = imap_email.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            imap_email.set(input.value());
        })
    };

    let onchange_imap_password = {
        let imap_password = imap_password.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            imap_password.set(input.value());
        })
    };

    let onchange_imap_provider = {
        let imap_provider = imap_provider.clone();
        let imap_server = imap_server.clone();
        let imap_port = imap_port.clone();
        let providers = providers.clone();
        Callback::from(move |e: Event| {
            let select: web_sys::HtmlSelectElement = e.target_unchecked_into();
            let value = select.value();
            imap_provider.set(value.clone());
            // Auto-fill server and port for predefined providers
            if let Some((_, _, server, port)) = providers.iter().find(|(id, _, _, _)| *id == value) {
                imap_server.set(server.to_string());
                imap_port.set(port.to_string());
            } else {
                imap_server.set(String::new());
                imap_port.set(String::new());
            }
        })
    };

    let onchange_imap_server = {
        let imap_server = imap_server.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            imap_server.set(input.value());
        })
    };

    let onchange_imap_port = {
        let imap_port = imap_port.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            imap_port.set(input.value());
        })
    };

    // Handler for connecting IMAP
    let onclick_imap_connect = {
        let imap_email_value = imap_email.clone();
        let imap_password_value = imap_password.clone();
        let imap_provider_value = imap_provider.clone();
        let imap_server_value = imap_server.clone();
        let imap_port_value = imap_port.clone();
        let imap_connected = imap_connected.clone();
        let error = error.clone();
        let imap_email_setter = imap_email.clone();
        let imap_password_setter = imap_password.clone();
        let connected_email = connected_email.clone();

        Callback::from(move |_: MouseEvent| {
            let email = (*imap_email_value).clone();
            let password = (*imap_password_value).clone();
            let provider = (*imap_provider_value).clone();
            let server = (*imap_server_value).clone();
            let port = (*imap_port_value).clone();
            let imap_connected = imap_connected.clone();
            let error = error.clone();
            let imap_email_setter = imap_email_setter.clone();
            let imap_password_setter = imap_password_setter.clone();
            let connected_email = connected_email.clone();

            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    if let Ok(Some(token)) = storage.get_item("token") {
                        spawn_local(async move {
                            let mut payload = json!({
                                "email": email,
                                "password": password,
                            });

                            // Include server and port only for custom provider or if overridden
                            if provider == "custom" || (!server.is_empty() && !port.is_empty()) {
                                payload["imap_server"] = json!(server);
                                payload["imap_port"] = json!(port.parse::<u16>().unwrap_or(993));
                            }

                            let request = Request::post(&format!("{}/api/auth/imap/login", config::get_backend_url()))
                                .header("Authorization", &format!("Bearer {}", token))
                                .header("Content-Type", "application/json")
                                .json(&payload)
                                .unwrap();

                            match request.send().await {
                                Ok(response) => {
                                    if response.ok() {
                                        imap_connected.set(true);
                                        imap_email_setter.set(String::new());
                                        imap_password_setter.set(String::new());
                                        error.set(None);
                                        connected_email.set(Some(email));
                                    } else {
                                        if let Ok(error_data) = response.json::<serde_json::Value>().await {
                                            if let Some(error_msg) = error_data.get("error").and_then(|e| e.as_str()) {
                                                error.set(Some(error_msg.to_string()));
                                            } else {
                                                error.set(Some(format!("Failed to connect: {}", response.status())));
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    error.set(Some(format!("Network error: {}", e)));
                                }
                            }
                        });
                    }
                }
            }
        })
    };

    // Handler for disconnecting IMAP
    // Check WhatsApp connection status on component mount
    {
        let whatsapp_connected = whatsapp_connected.clone();
        use_effect_with_deps(
            move |_| {
                if let Some(window) = web_sys::window() {
                    if let Ok(Some(storage)) = window.local_storage() {
                        if let Ok(Some(token)) = storage.get_item("token") {
                            spawn_local(async move {
                                let request = Request::get(&format!("{}/api/auth/whatsapp/status", config::get_backend_url()))
                                    .header("Authorization", &format!("Bearer {}", token))
                                    .send()
                                    .await;

                                if let Ok(response) = request {
                                    if response.ok() {
                                        if let Ok(data) = response.json::<serde_json::Value>().await {
                                            if let Some(connected) = data.get("connected").and_then(|v| v.as_bool()) {
                                                whatsapp_connected.set(connected);
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

    let onclick_whatsapp_connect = {
        let whatsapp_connecting = whatsapp_connecting.clone();
        let whatsapp_qr_code = whatsapp_qr_code.clone();
        let error = error.clone();
        
        Callback::from(move |_| {
            let whatsapp_connecting = whatsapp_connecting.clone();
            let whatsapp_qr_code = whatsapp_qr_code.clone();
            let error = error.clone();

            whatsapp_connecting.set(true);
            error.set(None);

            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    if let Ok(Some(token)) = storage.get_item("token") {
                        spawn_local(async move {
                            let request = Request::get(&format!("{}/api/auth/whatsapp/connect", config::get_backend_url()))
                                .header("Authorization", &format!("Bearer {}", token))
                                .send()
                                .await;

                            match request {
                                Ok(response) => {
                                    if response.ok() {
                                        if let Ok(data) = response.json::<serde_json::Value>().await {
                                            if let Some(qr_url) = data.get("qr_code_url").and_then(|u| u.as_str()) {
                                                whatsapp_qr_code.set(Some(qr_url.to_string()));
                                            } else {
                                                error.set(Some("Failed to get QR code".to_string()));
                                            }
                                        }
                                    } else {
                                        if let Ok(error_data) = response.json::<serde_json::Value>().await {
                                            if let Some(error_msg) = error_data.get("error").and_then(|e| e.as_str()) {
                                                error.set(Some(error_msg.to_string()));
                                            } else {
                                                error.set(Some(format!("Failed to connect: {}", response.status())));
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    error.set(Some(format!("Network error: {}", e)));
                                }
                            }
                            whatsapp_connecting.set(false);
                        });
                    }
                }
            }
        })
    };

    let onclick_whatsapp_disconnect = {
        let whatsapp_connected = whatsapp_connected.clone();
        let error = error.clone();
        
        Callback::from(move |_| {
            let whatsapp_connected = whatsapp_connected.clone();
            let error = error.clone();

            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    if let Ok(Some(token)) = storage.get_item("token") {
                        spawn_local(async move {
                            let request = Request::delete(&format!("{}/api/auth/whatsapp/disconnect", config::get_backend_url()))
                                .header("Authorization", &format!("Bearer {}", token))
                                .send()
                                .await;

                            match request {
                                Ok(response) => {
                                    if response.ok() {
                                        whatsapp_connected.set(false);
                                    } else {
                                        if let Ok(error_data) = response.json::<serde_json::Value>().await {
                                            if let Some(error_msg) = error_data.get("error").and_then(|e| e.as_str()) {
                                                error.set(Some(error_msg.to_string()));
                                            } else {
                                                error.set(Some(format!("Failed to disconnect: {}", response.status())));
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    error.set(Some(format!("Network error: {}", e)));
                                }
                            }
                        });
                    }
                }
            }
        })
    };

    let onclick_close_qr_modal = {
        let whatsapp_qr_code = whatsapp_qr_code.clone();
        Callback::from(move |_| {
            whatsapp_qr_code.set(None);
        })
    };

    let onclick_imap_disconnect = {
        let imap_connected = imap_connected.clone();
        let error = error.clone();
        let connected_email = connected_email.clone();
        Callback::from(move |_: MouseEvent| {
            let imap_connected = imap_connected.clone();
            let error = error.clone();
            let connected_email = connected_email.clone();

            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    if let Ok(Some(token)) = storage.get_item("token") {
                        spawn_local(async move {
                            let request = Request::delete(&format!("{}/api/auth/imap/disconnect", config::get_backend_url()))
                                .header("Authorization", &format!("Bearer {}", token))
                                .send()
                                .await;

                            match request {
                                Ok(response) => {
                                    if response.ok() {
                                        imap_connected.set(false);
                                        connected_email.set(None);
                                        error.set(None);
                                    } else {
                                        if let Ok(error_data) = response.json::<serde_json::Value>().await {
                                            if let Some(error_msg) = error_data.get("error").and_then(|e| e.as_str()) {
                                                error.set(Some(error_msg.to_string()));
                                            } else {
                                                error.set(Some(format!("Failed to disconnect: {}", response.status())));
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    error.set(Some(format!("Network error: {}", e)));
                                }
                            }
                        });
                    }
                }
            }
        })
    };

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
    let onclick_calendar = {
        let connecting = connecting.clone();
        let error = error.clone();
        let all_calendars = all_calendars.clone();
        Callback::from(move |_: MouseEvent| {
            let connecting = connecting.clone();
            let error = error.clone();
            let calendar_access_type = if *all_calendars { "all" } else { "primary" };

            connecting.set(true);
            error.set(None);

            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    if let Ok(Some(token)) = storage.get_item("token") {
                        web_sys::console::log_1(&format!("Initiating OAuth flow with token: {}", token).into());
                        spawn_local(async move {
                            let request = Request::get(&format!("{}/api/auth/google/calendar/login?calendar_access_type={}", 
                                config::get_backend_url(), 
                                calendar_access_type))
                                .header("Authorization", &format!("Bearer {}", token))
                                .header("Content-Type", "application/json");

                                match request.send().await {
                                    Ok(response) => {
                                        if (200..300).contains(&response.status()) {
                                            match response.json::<serde_json::Value>().await {
                                                Ok(data) => {
                                                    if let Some(auth_url) = data.get("auth_url").and_then(|u| u.as_str()) {
                                                        web_sys::console::log_1(&format!("Redirecting to auth_url: {}", auth_url).into());
                                                        if let Some(window) = web_sys::window() {
                                                            let _ = window.location().set_href(auth_url);
                                                        }
                                                    } else {
                                                        web_sys::console::log_1(&"Missing auth_url in response".into());
                                                        error.set(Some("Invalid response format: missing auth_url".to_string()));
                                                    }
                                                }
                                                Err(e) => {
                                                    web_sys::console::log_1(&format!("Failed to parse response: {}", e).into());
                                                    error.set(Some(format!("Failed to parse response: {}", e)));
                                                }
                                            }
                                        } else {
                                            match response.json::<serde_json::Value>().await {
                                                Ok(error_data) => {
                                                    if let Some(error_msg) = error_data.get("error").and_then(|e| e.as_str()) {
                                                        web_sys::console::log_1(&format!("Server error: {}", error_msg).into());
                                                        error.set(Some(error_msg.to_string()));
                                                    } else {
                                                        web_sys::console::log_1(&format!("Server error: Status {}", response.status()).into());
                                                        error.set(Some(format!("Server error: {}", response.status())));
                                                    }
                                                }
                                                Err(_) => {
                                                    web_sys::console::log_1(&format!("Server error: Status {}", response.status()).into());
                                                    error.set(Some(format!("Server error: {}", response.status())));
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        web_sys::console::log_1(&format!("Network error: {}", e).into());
                                        error.set(Some(format!("Network error: {}", e)));
                                    }
                                }
                                connecting.set(false);
                            });
                        } else {
                            web_sys::console::log_1(&"No token found in localStorage".into());
                            error.set(Some("Not authenticated".to_string()));
                            connecting.set(false);
                        }
                    }
                }
            })
        };

        let onclick_delete_calendar = {
                let calendar_connected = calendar_connected.clone();
                let error = error.clone();
                Callback::from(move |_: MouseEvent| {
                    let calendar_connected = calendar_connected.clone();
                    let error = error.clone();

                    if let Some(window) = web_sys::window() {
                        if let Ok(Some(storage)) = window.local_storage() {
                            if let Ok(Some(token)) = storage.get_item("token") {
                                spawn_local(async move {
                                    let request = Request::delete(&format!("{}/api/auth/google/calendar/connection", config::get_backend_url()))
                                        .header("Authorization", &format!("Bearer {}", token))
                                        .send()
                                        .await;

                                    match request {
                                        Ok(response) => {
                                            if response.ok() {
                                                calendar_connected.set(false);
                                            } else {
                                                if let Ok(error_data) = response.json::<serde_json::Value>().await {
                                                    if let Some(error_msg) = error_data.get("error").and_then(|e| e.as_str()) {
                                                        error.set(Some(error_msg.to_string()));
                                                    } else {
                                                        error.set(Some(format!("Failed to delete connection: {}", response.status())));
                                                    }
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            error.set(Some(format!("Network error: {}", e)));
                                        }
                                    }
                                });
                            }
                        }
                    }
                })
            };

            html! {
                <div class="connect-section">
                    // Calendar Services
                    <div class="service-group">
                        <h3 class="service-group-title">
                            <i class="fas fa-calendar"></i>
                            {"Calendar Services"}
                        </h3>
                        <div class="service-list">
                            // Google Calendar
                            <div class="service-item">
                                <div class="service-header">
                                    <div class="service-name">
                                        <img src="https://upload.wikimedia.org/wikipedia/commons/a/a5/Google_Calendar_icon_%282020%29.svg" alt="Google Calendar"/>
                                        {"Google Calendar"}
                                    </div>
                                    if *calendar_connected {
                                        <span class="service-status">{"Connected ✓"}</span>
                                    }
                                </div>
                                <p class="service-description">
                                    {"Access and manage your Google Calendar events through SMS or voice calls."}
                                </p>
                                if *calendar_connected {
                                    <div class="calendar-controls">
                                        <button 
                                            onclick={onclick_delete_calendar}
                                            class="disconnect-button"
                                        >
                                            {"Disconnect"}
                                        </button>
                                        {
                                            if props.user_id == 1 {
                                                let onclick_test = {
                                                    let error = error.clone();
                                                    Callback::from(move |_: MouseEvent| {
                                                        let error = error.clone();
                                                        if let Some(window) = web_sys::window() {
                                                            if let Ok(Some(storage)) = window.local_storage() {
                                                                if let Ok(Some(token)) = storage.get_item("token") {
                                                                    // Get today's start and end times in RFC3339 format
                                                                    let now = Date::new_0();
                                                                    let today_start = Date::new_0();
                                                                    let today_end = Date::new_0();
                                                                    today_start.set_hours(0);
                                                                    today_start.set_minutes(0);
                                                                    today_start.set_seconds(0);
                                                                    today_start.set_milliseconds(0);
                                                                    
                                                                    today_end.set_hours(23);
                                                                    today_end.set_minutes(59);
                                                                    today_end.set_seconds(59);
                                                                    today_end.set_milliseconds(999);
                                                                    
                                                                    let start_time = today_start.to_iso_string().as_string().unwrap();
                                                                    let end_time = today_end.to_iso_string().as_string().unwrap();
                                                                    
                                                                    spawn_local(async move {
                                                                        let url = format!(
                                                                            "{}/api/calendar/events?start={}&end={}", 
                                                                            config::get_backend_url(),
                                                                            start_time,
                                                                            end_time
                                                                        );
                                                                        
                                                                        match Request::get(&url)
                                                                            .header("Authorization", &format!("Bearer {}", token))
                                                                            .send()
                                                                            .await {
                                                                            Ok(response) => {
                                                                                if response.status() == 200 {
                                                                                    if let Ok(data) = response.json::<serde_json::Value>().await {
                                                                                        web_sys::console::log_1(&format!("Calendar events: {:?}", data).into());
                                                                                        // You could also show this in the UI instead of just console
                                                                                    }
                                                                                } else {
                                                                                    error.set(Some("Failed to fetch calendar events".to_string()));
                                                                                }
                                                                            }
                                                                            Err(e) => {
                                                                                error.set(Some(format!("Network error: {}", e)));
                                                                            }
                                                                        }
                                                                    });
                                                                }
                                                            }
                                                        }
                                                    })
                                                };
                                                {
                                                    if props.user_id == 1 {
                                                        html! {
                                                            <button 
                                                                onclick={onclick_test}
                                                                class="test-button"
                                                            >
                                                                {"Test Calendar"}
                                                            </button>
                                                        }
                                                    } else {
                                                        html! {}
                                                    }
                                                }
                                                

                                            } else {
                                                html! {}
                                            }
                                        }
                                    </div>
                                } else {
                                    <div class="calendar-connect-options">
                                        <label class="calendar-checkbox">
                                            <input 
                                                type="checkbox"
                                                checked={*all_calendars}
                                                onchange={
                                                    let all_calendars = all_calendars.clone();
                                                    Callback::from(move |e: Event| {
                                                        let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                                                        all_calendars.set(input.checked());
                                                    })
                                                }
                                            />
                                            {"Access all calendars (including shared)"}
                                        </label>
                                        <button 
                                            onclick={
                                                let all_calendars = all_calendars.clone();
                                                let onclick_calendar = onclick_calendar.clone();
                                                Callback::from(move |e: MouseEvent| {
                                                    let all_calendars = *all_calendars;
                                                    if let Some(window) = web_sys::window() {
                                                        if let Ok(Some(storage)) = window.local_storage() {
                                                            let _ = storage.set_item("calendar_access_type", if all_calendars { "all" } else { "primary" });
                                                        }
                                                    }
                                                    onclick_calendar.emit(e);
                                                })
                                            }
                                            class="connect-button"
                                        >
                                            if *connecting {
                                                {"Connecting..."}
                                            } else {
                                                {"Connect"}
                                            }
                                        </button>
                                    </div>
                                }
                            </div>

                        </div>
                    </div>

                    // Email Services
                    <div class="service-group">
                        <h3 class="service-group-title">
                            <i class="fas fa-envelope"></i>
                            {"Email Services"}
                        </h3>
                        <div class="service-list">

                            // Generic IMAP section
                            <div class="service-item">
                                <div class="service-header">
                                    <div class="service-name">
                                        <img src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 512 512'%3E%3Cpath fill='%234285f4' d='M48 64C21.5 64 0 85.5 0 112c0 15.1 7.1 29.3 19.2 38.4L236.8 313.6c11.4 8.5 27 8.5 38.4 0L492.8 150.4c12.1-9.1 19.2-23.3 19.2-38.4c0-26.5-21.5-48-48-48H48zM0 176V384c0 35.3 28.7 64 64 64H448c35.3 0 64-28.7 64-64V176L294.4 339.2c-22.8 17.1-54 17.1-76.8 0L0 176z'/%3E%3C/svg%3E" alt="IMAP"/>
                                        {"IMAP Email"}
                                    </div>
                                    if *imap_connected {
                                        <div class="service-status-container">
                                            <span class="service-status">{"Connected ✓"}</span>
                                            <span class="connected-email">
                                                {
                                                    if let Some(email) = &*connected_email {
                                                        format!(" ({})", email)
                                                    } else {
                                                        "".to_string()
                                                    }
                                                }
                                            </span>
                                        </div>
                                    }
                                </div>
                                <p class="service-description">
                                    {"Connect your email account using IMAP to send and receive emails through SMS or voice calls. "}
                                    {"For Gmail, create an app password "}
                                    <a class="nice-link" href="https://myaccount.google.com/apppasswords" target="_blank">{"here"}</a>
                                    {" (requires 2FA)."}
                                </p>
                                if *imap_connected {
                                    <div class="imap-controls">
                                        <button 
                                            onclick={onclick_imap_disconnect}
                                            class="disconnect-button"
                                        >
                                            {"Disconnect"}
                                        </button>
                                        // Test buttons (updated endpoints)
                                        if props.user_id == 1 {
                                            <>
                                                <button
                                                    onclick={
                                                        let error = error.clone();
                                                        Callback::from(move |_: MouseEvent| {
                                                            let error = error.clone();
                                                            if let Some(window) = web_sys::window() {
                                                                if let Ok(Some(storage)) = window.local_storage() {
                                                                    if let Ok(Some(token)) = storage.get_item("token") {
                                                                        spawn_local(async move {
                                                                            let request = Request::get(&format!("{}/api/imap/previews", config::get_backend_url()))
                                                                                .header("Authorization", &format!("Bearer {}", token))
                                                                                .send()
                                                                                .await;

                                                                            match request {
                                                                                Ok(response) => {
                                                                                    if response.status() == 200 {
                                                                                        if let Ok(data) = response.json::<serde_json::Value>().await {
                                                                                            web_sys::console::log_1(&format!("IMAP previews: {:?}", data).into());
                                                                                        }
                                                                                    } else {
                                                                                        error.set(Some("Failed to fetch IMAP previews".to_string()));
                                                                                    }
                                                                                }
                                                                                Err(e) => {
                                                                                    error.set(Some(format!("Network error: {}", e)));
                                                                                }
                                                                            }
                                                                        });
                                                                    }
                                                                }
                                                            }
                                                        })
                                                    }
                                                    class="test-button"
                                                >
                                                    {"Test IMAP Previews"}
                                                </button>
                                                <button
                                                    onclick={
                                                        let error = error.clone();
                                                        Callback::from(move |_: MouseEvent| {
                                                            let error = error.clone();
                                                            if let Some(window) = web_sys::window() {
                                                                if let Ok(Some(storage)) = window.local_storage() {
                                                                    if let Ok(Some(token)) = storage.get_item("token") {
                                                                        spawn_local(async move {
                                                                            let request = Request::get(&format!("{}/api/imap/full_emails", config::get_backend_url()))
                                                                                .header("Authorization", &format!("Bearer {}", token))
                                                                                .send()
                                                                                .await;

                                                                            match request {
                                                                                Ok(response) => {
                                                                                    if response.status() == 200 {
                                                                                        if let Ok(data) = response.json::<serde_json::Value>().await {
                                                                                            web_sys::console::log_1(&format!("IMAP full emails: {:?}", data).into());
                                                                                        }
                                                                                    } else {
                                                                                        error.set(Some("Failed to fetch full IMAP emails".to_string()));
                                                                                    }
                                                                                }
                                                                                Err(e) => {
                                                                                    error.set(Some(format!("Network error: {}", e)));
                                                                                }
                                                                            }
                                                                        });
                                                                    }
                                                                }
                                                            }
                                                        })
                                                    }
                                                    class="test-button"
                                                >
                                                    {"Test Full Emails"}
                                                </button>
                                                <button
                                                    onclick={
                                                        let error = error.clone();
                                                        Callback::from(move |_: MouseEvent| {
                                                            let error = error.clone();
                                                            if let Some(window) = web_sys::window() {
                                                                if let Ok(Some(storage)) = window.local_storage() {
                                                                    if let Ok(Some(token)) = storage.get_item("token") {
                                                                        spawn_local(async move {
                                                                            let previews_request = Request::get(&format!("{}/api/imap/previews", config::get_backend_url()))
                                                                                .header("Authorization", &format!("Bearer {}", token))
                                                                                .send()
                                                                                .await;

                                                                            match previews_request {
                                                                                Ok(response) => {
                                                                                    if response.status() == 200 {
                                                                                        if let Ok(data) = response.json::<serde_json::Value>().await {
                                                                                            if let Some(previews) = data.get("previews").and_then(|p| p.as_array()) {
                                                                                                if let Some(first_message) = previews.first() {
                                                                                                    if let Some(id) = first_message.get("id").and_then(|i| i.as_str()) {
                                                                                                        let message_request = Request::get(&format!("{}/api/imap/message/{}", config::get_backend_url(), id))
                                                                                                            .header("Authorization", &format!("Bearer {}", token))
                                                                                                            .send()
                                                                                                            .await;

                                                                                                        match message_request {
                                                                                                            Ok(msg_response) => {
                                                                                                                if msg_response.status() == 200 {
                                                                                                                    if let Ok(msg_data) = msg_response.json::<serde_json::Value>().await {
                                                                                                                        web_sys::console::log_1(&format!("IMAP single message: {:?}", msg_data).into());
                                                                                                                    }
                                                                                                                } else {
                                                                                                                    error.set(Some("Failed to fetch single IMAP message".to_string()));
                                                                                                                }
                                                                                                            }
                                                                                                            Err(e) => {
                                                                                                                error.set(Some(format!("Network error: {}", e)));
                                                                                                            }
                                                                                                        }
                                                                                                    }
                                                                                                }
                                                                                            }
                                                                                        }
                                                                                    }
                                                                                }
                                                                                Err(e) => {
                                                                                    error.set(Some(format!("Network error: {}", e)));
                                                                                }
                                                                            }
                                                                        });
                                                                    }
                                                                }
                                                            }
                                                        })
                                                    }
                                                    class="test-button"
                                                >
                                                    {"Test Single Message"}
                                                </button>
                                            </>
                                        }
                                    </div>
                                } else {
                                    <div class="imap-form">
                                        <select onchange={onchange_imap_provider}>
                                            { for providers.iter().map(|(id, name, _, _)| {
                                                html! {
                                                    <option value={id.to_string()} selected={*imap_provider == *id}>
                                                        {name}
                                                    </option>
                                                }
                                            })}
                                        </select>
                                        <input
                                            type="email"
                                            placeholder="Email address"
                                            value={(*imap_email).clone()}
                                            onchange={onchange_imap_email}
                                        />
                                        <input
                                            type="password"
                                            placeholder="Password or App Password"
                                            value={(*imap_password).clone()}
                                            onchange={onchange_imap_password}
                                        />
                                        // Show custom fields only if "custom" is selected
                                        if *imap_provider == "custom" {
                                            <>
                                                <input
                                                    type="text"
                                                    placeholder="IMAP Server (e.g., mail.privateemail.com)"
                                                    value={(*imap_server).clone()}
                                                    onchange={onchange_imap_server}
                                                />
                                                <input
                                                    type="number"
                                                    placeholder="IMAP Port (e.g., 993)"
                                                    value={(*imap_port).clone()}
                                                    onchange={onchange_imap_port}
                                                />
                                            </>
                                        }
                                        <button 
                                            onclick={onclick_imap_connect}
                                            class="connect-button"
                                        >
                                            {"Connect"}
                                        </button>
                                    </div>
                                }
                            </div>
                        </div>
                    </div>

                    // Messaging Services (Coming Soon)
                    <div class="service-group">
                        <h3 class="service-group-title">
                            <i class="fas fa-comments"></i>
                            {"Messaging Services"}
                            <span class="coming-soon-tag">{"Coming Soon"}</span>
                        </h3>
                        <div class="service-list">

                            <WhatsappConnect user_id={props.user_id} />
                            <div class="service-item coming-soon">
                                <div class="service-header">
                                    <div class="service-name">
                                        <img src="https://upload.wikimedia.org/wikipedia/commons/8/82/Telegram_logo.svg" alt="Telegram"/>
                                        {"Telegram"}
                                    </div>
                                </div>
                                <p class="service-description">
                                    {"Send and receive Telegram messages through SMS or voice calls."}
                                </p>
                                <button class="connect-button" disabled=true>
                                    {"Connect"}
                                </button>
                            </div>

                            <div class="service-item">
                                <div class="service-header">
                                    <div class="service-name">
                                        <img src="https://upload.wikimedia.org/wikipedia/commons/6/6b/WhatsApp.svg" alt="WhatsApp"/>
                                        {"WhatsApp"}
                                    </div>
                                    if *whatsapp_connected {
                                        <span class="service-status">{"Connected ✓"}</span>
                                    }
                                </div>
                                <p class="service-description">
                                    {"Send and receive WhatsApp messages through SMS or voice calls."}
                                </p>
                                if *whatsapp_connected {
                                    <button 
                                        onclick={onclick_whatsapp_disconnect}
                                        class="disconnect-button"
                                    >
                                        {"Disconnect"}
                                    </button>
                                } else {
                                    <button 
                                        onclick={onclick_whatsapp_connect}
                                        class="connect-button"
                                        disabled={*whatsapp_connecting}
                                    >
                                        if *whatsapp_connecting {
                                            {"Connecting..."}
                                        } else {
                                            {"Connect"}
                                        }
                                    </button>
                                }
                                if let Some(qr_code) = &*whatsapp_qr_code {
                                    <div class="qr-code-modal">
                                        <div class="qr-code-content">
                                            <h3>{"Scan QR Code with WhatsApp"}</h3>
                                            <img src={qr_code.clone()} alt="WhatsApp QR Code" />
                                            <p>{"Open WhatsApp on your phone and scan this QR code to connect."}</p>
                                            <button onclick={onclick_close_qr_modal} class="close-button">
                                                {"Close"}
                                            </button>
                                        </div>
                                    </div>
                                }
                            </div>
                        </div>

                        // Outlook Calendar (Coming Soon)
                        <div class="service-item coming-soon">
                            <div class="service-header">
                                <div class="service-name">
                                    <img src="https://upload.wikimedia.org/wikipedia/commons/d/df/Microsoft_Office_Outlook_%282018%E2%80%93present%29.svg" alt="Outlook Calendar"/>
                                    {"Outlook Calendar"}
                                    <span class="coming-soon-tag">{"Coming Soon"}</span>
                                </div>
                            </div>
                            <p class="service-description">
                                {"Manage your Outlook Calendar events through SMS or voice calls."}
                            </p>
                            <button class="connect-button" disabled=true>
                                {"Connect"}
                            </button>
                        </div>
                    </div>

                    if let Some(err) = (*error).as_ref() {
                        <div class="error-message">
                            {err}
                        </div>
                    }
<style>
{".connect-section {
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
    width: 24px;
    height: 24px;
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

.imap-form {
    display: flex;
    flex-direction: column;
    gap: 1rem;
}

.imap-form input, .imap-form select {
    padding: 0.75rem;
    border-radius: 8px;
    border: 1px solid rgba(30, 144, 255, 0.2);
    background: rgba(0, 0, 0, 0.2);
    color: #fff;
    font-size: 0.95rem;
}

.imap-form input:focus, .imap-form select:focus {
    border-color: rgba(30, 144, 255, 0.4);
    outline: none;
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
                        }"}
                        {".service-status-container {
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
                        }



                        "}
                    </style>
                </div>
            }

}
