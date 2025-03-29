use yew::prelude::*;
use web_sys::MouseEvent;
use yew_hooks::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, Window, History};
use wasm_bindgen::JsValue;
use crate::config;
use gloo_net::http::Request;
use web_sys::UrlSearchParams;
use web_sys::js_sys::Date;

#[derive(Properties, PartialEq)]
pub struct ConnectProps {
    pub user_id: i32,
}

#[function_component(Connect)]
pub fn connect(props: &ConnectProps) -> Html {
    let error = use_state(|| None::<String>);
    let connecting = use_state(|| false);
    let calendar_connected = use_state(|| false);
    let gmail_connected = use_state(|| false);
    let imap_connected = use_state(|| false);
    let imap_email = use_state(|| String::new());
    let imap_password = use_state(|| String::new());

    // Check connection status on component mount
    {
        let calendar_connected = calendar_connected.clone();
        let gmail_connected = gmail_connected.clone();
        let imap_connected = imap_connected.clone();
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

                            // Check Gmail status
                            {
                                let gmail_connected = gmail_connected.clone();
                                let token = token.clone();
                                spawn_local(async move {
                                    let request = Request::get(&format!("{}/api/auth/google/gmail/status", config::get_backend_url()))
                                        .header("Authorization", &format!("Bearer {}", token))
                                        .send()
                                        .await;

                                    if let Ok(response) = request {
                                        if (200..300).contains(&response.status()) {
                                            if let Ok(data) = response.json::<serde_json::Value>().await {
                                                if let Some(connected) = data.get("connected").and_then(|v| v.as_bool()) {
                                                    gmail_connected.set(connected);
                                                }
                                            }
                                        } else {
                                            web_sys::console::log_1(&"Failed to check Gmail status".into());
                                        }
                                    }
                                });
                            }
                            // Check Gmail IMAP status
                            {
                                let imap_connected = imap_connected.clone();
                                let token = token.clone();
                                spawn_local(async move {
                                    let request = Request::get(&format!("{}/api/auth/gmail/imap/status", config::get_backend_url()))
                                        .header("Authorization", &format!("Bearer {}", token))
                                        .send()
                                        .await;

                                    if let Ok(response) = request {
                                        if response.ok() {
                                            if let Ok(data) = response.json::<serde_json::Value>().await {
                                                if let Some(connected) = data.get("connected").and_then(|v| v.as_bool()) {
                                                    imap_connected.set(connected);
                                                }
                                            }
                                        } else {
                                            web_sys::console::log_1(&"Failed to check IMAP status".into());
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
            Callback::from(move |_: MouseEvent| {
                let connecting = connecting.clone();
                let error = error.clone();

                connecting.set(true);
                error.set(None);

                if let Some(window) = web_sys::window() {
                    if let Ok(Some(storage)) = window.local_storage() {
                        if let Ok(Some(token)) = storage.get_item("token") {
                            web_sys::console::log_1(&format!("Initiating OAuth flow with token: {}", token).into());
                            spawn_local(async move {
                                let request = Request::get(&format!("{}/api/auth/google/calendar/login", config::get_backend_url()))
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

        let onclick_gmail = {
            let connecting = connecting.clone();
            let error = error.clone();
            Callback::from(move |_: MouseEvent| {
                let connecting = connecting.clone();
                let error = error.clone();

                connecting.set(true);
                error.set(None);

                if let Some(window) = web_sys::window() {
                    if let Ok(Some(storage)) = window.local_storage() {
                        if let Ok(Some(token)) = storage.get_item("token") {
                            spawn_local(async move {
                                let request = Request::get(&format!("{}/api/auth/google/gmail/login", config::get_backend_url()))
                                    .header("Authorization", &format!("Bearer {}", token))
                                    .header("Content-Type", "application/json");

                                match request.send().await {
                                    Ok(response) => {
                                    if (200..300).contains(&response.status()) {
                                            match response.json::<serde_json::Value>().await {
                                                Ok(data) => {
                                                    if let Some(auth_url) = data.get("auth_url").and_then(|u| u.as_str()) {
                                                        if let Some(window) = web_sys::window() {
                                                            let _ = window.location().set_href(auth_url);
                                                        }
                                                    } else {
                                                        error.set(Some("Invalid response format: missing auth_url".to_string()));
                                                    }
                                                }
                                                Err(e) => {
                                                    error.set(Some(format!("Failed to parse response: {}", e)));
                                                }
                                            }
                                        } else {
                                            match response.json::<serde_json::Value>().await {
                                                Ok(error_data) => {
                                                    if let Some(error_msg) = error_data.get("error").and_then(|e| e.as_str()) {
                                                        error.set(Some(error_msg.to_string()));
                                                    } else {
                                                        error.set(Some(format!("Server error: {}", response.status())));
                                                    }
                                                }
                                                Err(_) => {
                                                    error.set(Some(format!("Server error: {}", response.status())));
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        error.set(Some(format!("Network error: {}", e)));
                                    }
                                }
                                connecting.set(false);
                            });
                        } else {
                            error.set(Some("Not authenticated".to_string()));
                            connecting.set(false);
                        }
                    }
                }
            })
        };

        let onclick_delete_gmail = {
            let gmail_connected = gmail_connected.clone();
            let error = error.clone();
            Callback::from(move |_: MouseEvent| {
                let gmail_connected = gmail_connected.clone();
                let error = error.clone();

                if let Some(window) = web_sys::window() {
                    if let Ok(Some(storage)) = window.local_storage() {
                        if let Ok(Some(token)) = storage.get_item("token") {
                            spawn_local(async move {
                                let request = Request::delete(&format!("{}/api/auth/google/gmail/delete_connection", config::get_backend_url()))
                                    .header("Authorization", &format!("Bearer {}", token))
                                    .send()
                                    .await;

                                match request {
                                    Ok(response) => {
                                        if response.ok() {
                                            gmail_connected.set(false);
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

        // New handlers for IMAP email and password input
    let onchange_imap_email = {
        let imap_email = imap_email.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            imap_email.set(input.value());
        })
    };

    let onchange_imap_password = {
        let imap_password = imap_password.clone();
        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            imap_password.set(input.value());
        })
    };

    // Handler for connecting IMAP
    let onclick_imap_connect = {
        let imap_email_value = imap_email.clone();
        let imap_password_value = imap_password.clone();
        let imap_connected = imap_connected.clone();
        let error = error.clone();
        let imap_email_setter = imap_email.clone();
        let imap_password_setter = imap_password.clone();
        
        Callback::from(move |_: MouseEvent| {
            let email = (*imap_email_value).clone();
            let password = (*imap_password_value).clone();
            let imap_connected = imap_connected.clone();
            let error = error.clone();
            let imap_email_setter = imap_email_setter.clone();
            let imap_password_setter = imap_password_setter.clone();

            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    if let Ok(Some(token)) = storage.get_item("token") {
                        spawn_local(async move {
                            let payload = serde_json::json!({
                                "email": email,
                                "password": password,
                            });
                            let request = Request::post(&format!("{}/api/auth/gmail/imap/login", config::get_backend_url()))
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
                                        error.set(None); // Clear error on success
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
            let onclick_imap_disconnect = {
                let imap_connected = imap_connected.clone();
                let error = error.clone();
                Callback::from(move |_: MouseEvent| {
                    let imap_connected = imap_connected.clone();
                    let error = error.clone();

                    if let Some(window) = web_sys::window() {
                        if let Ok(Some(storage)) = window.local_storage() {
                            if let Ok(Some(token)) = storage.get_item("token") {
                                spawn_local(async move {
                                    let request = Request::delete(&format!("{}/api/auth/gmail/imap/disconnect", config::get_backend_url()))
                                        .header("Authorization", &format!("Bearer {}", token))
                                        .send()
                                        .await;

                                    match request {
                                        Ok(response) => {
                                            if response.ok() {
                                                imap_connected.set(false);
                                                error.set(None); // Clear error on success
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
                                                html! {
                                                /*
                                                    <button 
                                                        onclick={onclick_test}
                                                        class="test-button"
                                                    >
                                                        {"Test Calendar"}
                                                    </button>
                                                */
                                                }

                                            } else {
                                                html! {}
                                            }
                                        }
                                    </div>
                                } else {
                                    <button 
                                        onclick={onclick_calendar.clone()} 
                                        class="connect-button"
                                    >
                                        if *connecting {
                                            {"Connecting..."}
                                        } else {
                                            {"Connect"}
                                        }
                                    </button>
                                }
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
                    </div>

                    // Email Services
                    <div class="service-group">
                        <h3 class="service-group-title">
                            <i class="fas fa-envelope"></i>
                            {"Email Services"}
                        </h3>
                        <div class="service-list">
                            // Gmail via IMAP section
                            <div class="service-item">
                                <div class="service-header">
                                    <div class="service-name">
                                        <img src="https://upload.wikimedia.org/wikipedia/commons/7/7e/Gmail_icon_%282020%29.svg" alt="Gmail via IMAP"/>
                                        {"Gmail via IMAP"}
                                    </div>
                                    if *imap_connected {
                                        <span class="service-status">{"Connected ✓"}</span>
                                    }
                                </div>
                                <p class="service-description">
                                    {"Connect your Gmail account using IMAP to send and receive emails through SMS or voice calls. "}
                                    <strong>{"You can create an app password "}</strong>
                                    <a class="nice-link" href="https://support.google.com/accounts/answer/185833" target="_blank">{"here."}</a>
                                    <strong>{" Regular passwords won't work. "}</strong>
                                </p>
                                if *imap_connected {
                                    <div class="gmail-controls">
                                        <button 
                                            onclick={onclick_imap_disconnect}
                                            class="disconnect-button"
                                        >
                                            {"Disconnect"}
                                        </button>
                                        {
                                            if props.user_id == 1 {
                                                html! {
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
                                                                                    let request = Request::get(&format!("{}/api/gmail/imap/previews", config::get_backend_url()))
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
                                                                                    let request = Request::get(&format!("{}/api/gmail/imap/full_emails", config::get_backend_url()))
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
                                                                                    // Fetch first message from previews to test single message fetch
                                                                                    let previews_request = Request::get(&format!("{}/api/gmail/imap/previews", config::get_backend_url()))
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
                                                                                                                let message_request = Request::get(&format!("{}/api/gmail/imap/message/{}", config::get_backend_url(), id))
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
                                            } else {
                                                html! {}
                                            }
                                        }
                                    </div>
                                } else {
                                    <div class="imap-form">
                                        <input
                                            type="email"
                                            placeholder="Gmail address"
                                            value={(*imap_email).clone()}
                                            onchange={onchange_imap_email}
                                        />
                                        <input
                                            type="password"
                                            placeholder="Password or App Password"
                                            value={(*imap_password).clone()}
                                            onchange={onchange_imap_password}
                                        />
                                        <button 
                                            onclick={onclick_imap_connect}
                                            class="connect-button"
                                        >
                                            {"Connect"}
                                        </button>
                                    </div>
                                }
                            </div>

                            // Gmail (Commented out in favor of IMAP implementation)
                            /*
                            <div class="service-item coming-soon">
                                <div class="service-header">
                                    <div class="service-name">
                                        <img src="https://upload.wikimedia.org/wikipedia/commons/7/7e/Gmail_icon_%282020%29.svg" alt="Gmail"/>
                                        {"Gmail"}
                                        <span class="coming-soon-tag">{"Coming Soon"}</span>
                                    </div>

                                </div>
                                <p class="service-description">
                                    {"Send and receive Gmail messages through SMS or voice calls."}
                                </p>
                                <button class="connect-button" disabled=true>
                                    {"Connect"}
                                </button>
                            </div>
                            */

                            // Outlook (Coming Soon)
                            <div class="service-item coming-soon">
                                <div class="service-header">
                                    <div class="service-name">
                                        <img src="https://upload.wikimedia.org/wikipedia/commons/d/df/Microsoft_Office_Outlook_%282018%E2%80%93present%29.svg" alt="Outlook"/>
                                        {"Outlook"}
                                        <span class="coming-soon-tag">{"Coming Soon"}</span>
                                    </div>
                                </div>
                                <p class="service-description">
                                    {"Send and receive Outlook emails through SMS or voice calls."}
                                </p>
                                <button class="connect-button" disabled=true>
                                    {"Connect"}
                                </button>
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

                            <div class="service-item coming-soon">
                                <div class="service-header">
                                    <div class="service-name">
                                        <img src="https://upload.wikimedia.org/wikipedia/commons/6/6b/WhatsApp.svg" alt="WhatsApp"/>
                                        {"WhatsApp"}
                                    </div>
                                </div>
                                <p class="service-description">
                                    {"Send and receive WhatsApp messages through SMS or voice calls."}
                                </p>
                                <button class="connect-button" disabled=true>
                                    {"Connect"}
                                </button>
                            </div>
                        </div>
                    </div>

                    if let Some(err) = (*error).as_ref() {
                        <div class="error-message">
                            {err}
                        </div>
                    }
                    <style>
                        {".gmail-controls {
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
                        }"}
                    </style>
                </div>
            }

}
