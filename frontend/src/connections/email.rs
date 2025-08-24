use yew::prelude::*;
use web_sys::{MouseEvent, HtmlInputElement, Event};
use serde_json::json;
use wasm_bindgen_futures::spawn_local;
use gloo_net::http::Request;
use crate::config;
#[derive(Properties, PartialEq)]
pub struct EmailProps {
    pub user_id: i32,
    pub sub_tier: Option<String>,
    pub discount: bool,
}
#[function_component(EmailConnect)]
pub fn email_connect(props: &EmailProps) -> Html {
    let error = use_state(|| None::<String>);
    let imap_connected = use_state(|| false);
    let imap_email = use_state(|| String::new());
    let imap_password = use_state(|| String::new());
    let imap_provider = use_state(|| "gmail".to_string()); // Default to Gmail
    let imap_server = use_state(|| String::new()); // For custom provider
    let imap_port = use_state(|| String::new()); // For custom provider
    let connected_email = use_state(|| None::<String>);
    // Predefined providers
    let providers = vec![
        ("gmail", "Gmail", "imap.gmail.com", "993"),
        ("privateemail", "PrivateEmail", "mail.privateemail.com", "993"),
        ("outlook", "Outlook", "imap-mail.outlook.com", "993"),
        ("custom", "Custom", "", ""), // Custom option with empty defaults
    ];
    // Check connection status on component mount
    {
        let imap_connected = imap_connected.clone();
        let connected_email = connected_email.clone();
        use_effect_with_deps(
            move |_| {
                if let Some(window) = web_sys::window() {
                    if let Ok(Some(storage)) = window.local_storage() {
                        if let Ok(Some(token)) = storage.get_item("token") {
                            let imap_connected = imap_connected.clone();
                            let connected_email = connected_email.clone();
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
    html! {
        <div class="service-item">
            <div class="service-header">
                <div class="service-name">
                    <img src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 512 512'%3E%3Cpath fill='%234285f4' d='M48 64C21.5 64 0 85.5 0 112c0 15.1 7.1 29.3 19.2 38.4L236.8 313.6c11.4 8.5 27 8.5 38.4 0L492.8 150.4c12.1-9.1 19.2-23.3 19.2-38.4c0-26.5-21.5-48-48-48H48zM0 176V384c0 35.3 28.7 64 64 64H448c35.3 0 64-28.7 64-64V176L294.4 339.2c-22.8 17.1-54 17.1-76.8 0L0 176z'/%3E%3C/svg%3E" alt="IMAP" width="24" height="24"/>
                    {"IMAP Email"}
                </div>
                <button class="info-button" onclick={Callback::from(|_| {
                    if let Some(element) = web_sys::window()
                        .and_then(|w| w.document())
                        .and_then(|d| d.get_element_by_id("email-info"))
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
            <div id="email-info" class="info-section" style="display: none">
                <h4>{"How It Works"}</h4>
                <div class="info-subsection">
                    <h5>{"SMS and Voice Call Tools"}</h5>
                    <ul>
                        <li>{"Fetch specific number of Email Previews: Fetches a given number of latest emails previews from your inbox."}</li>
                        <li>{"Search specific Email: Searches for specific email based on a given query(sender, subject, content, time, etc)."}</li>
                    </ul>
                </div>
                <div class="info-subsection">
                    <h5>{"Provider Support"}</h5>
                    <ul>
                        <li>{"Gmail: Full support with App Password (2FA enabled requirement)"}</li>
                        <li>{"Outlook: Native IMAP support"}</li>
                        <li>{"PrivateEmail: Direct IMAP integration"}</li>
                        <li>{"Custom: Support for any IMAP-enabled email provider"}</li>
                    </ul>
                </div>
                <div class="info-subsection security-notice">
                    <h5>{"Security & Privacy"}</h5>
                    <p>{"Your email security is our top priority. Here's how we protect your data:"}</p>
                    <ul>
                        <li>{"Secure IMAP Connection: All email communications use TLS-encrypted IMAP connections (port 993)"}</li>
                        <li>{"Credentials Protection: Your email credentials are encrypted and stored securely"}</li>
                        <li>{"Limited Access: We only access emails when you specifically request them"}</li>
                        <li>{"No Email Storage: We don't store your emails - we fetch them on demand when you need them"}</li>
                    </ul>
                    <p class="security-recommendation">{"Note: For Gmail users, we recommend using App Passwords instead of your main account password. This provides an extra layer of security and control over access."}</p>
                </div>
            </div>
            <p class="service-description">
                {"Connect your email account using IMAP access your emails through SMS or voice calls. For Gmail, create an app password "}
                <a class="nice-link" href="https://myaccount.google.com/apppasswords" target="_blank">{"here"}</a>
                {" (requires 2FA)."}
            </p>
            if props.sub_tier.as_deref() == Some("tier 2") || props.discount {
                if *imap_connected {
                    <div class="imap-controls">
                        <button
                            onclick={onclick_imap_disconnect}
                            class="disconnect-button"
                        >
                            {"Disconnect"}
                        </button>
                        // Test buttons for admin
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
                                <button
                                    onclick={
                                        let error = error.clone();
                                        Callback::from(move |_: MouseEvent| {
                                            let error = error.clone();
                                            if let Some(window) = web_sys::window() {
                                                if let Ok(Some(storage)) = window.local_storage() {
                                                    if let Ok(Some(token)) = storage.get_item("token") {
                                                        spawn_local(async move {
                                                            // First fetch previews to get the latest email ID
                                                            let previews_request = Request::get(&format!("{}/api/imap/previews?limit=1", config::get_backend_url()))
                                                                .header("Authorization", &format!("Bearer {}", token))
                                                                .send()
                                                                .await;
                                                            match previews_request {
                                                                Ok(response) => {
                                                                    if response.status() == 200 {
                                                                        if let Ok(data) = response.json::<serde_json::Value>().await {
                                                                            if let Some(previews) = data.get("previews").and_then(|p| p.as_array()) {
                                                                                if let Some(latest_email) = previews.first() {
                                                                                    if let Some(id) = latest_email.get("id").and_then(|i| i.as_str()) {
                                                                                        // Now send a test reply
                                                                                        let reply_payload = json!({
                                                                                            "email_id": id,
                                                                                            "response_text": "This is a test reply from LightFriend!"
                                                                                        });
                                                                                        let reply_request = Request::post(&format!("{}/api/imap/reply", config::get_backend_url()))
                                                                                            .header("Authorization", &format!("Bearer {}", token))
                                                                                            .header("Content-Type", "application/json")
                                                                                            .json(&reply_payload)
                                                                                            .unwrap()
                                                                                            .send()
                                                                                            .await;
                                                                                        match reply_request {
                                                                                            Ok(reply_response) => {
                                                                                                if reply_response.status() == 200 {
                                                                                                    web_sys::console::log_1(&"Successfully sent test reply".into());
                                                                                                } else {
                                                                                                    if let Ok(error_data) = reply_response.json::<serde_json::Value>().await {
                                                                                                        error.set(Some(format!("Failed to send reply: {}",
                                                                                                            error_data.get("error").and_then(|e| e.as_str()).unwrap_or("Unknown error"))));
                                                                                                    }
                                                                                                }
                                                                                            }
                                                                                            Err(e) => {
                                                                                                error.set(Some(format!("Network error while sending reply: {}", e)));
                                                                                            }
                                                                                        }
                                                                                    }
                                                                                }
                                                                            }
                                                                        }
                                                                    } else {
                                                                        error.set(Some("Failed to fetch latest email".to_string()));
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
                                    {"Test Reply to Latest"}
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
                                                            let payload = json!({
                                                                "to": "rasmus@ahtava.com",
                                                                "subject": "test email subject",
                                                                "body": "testing body here"
                                                            });
                                                            let request = Request::post(&format!("{}/api/imap/send", config::get_backend_url()))
                                                                .header("Authorization", &format!("Bearer {}", token))
                                                                .header("Content-Type", "application/json")
                                                                .json(&payload)
                                                                .unwrap()
                                                                .send()
                                                                .await;
                                                            match request {
                                                                Ok(response) => {
                                                                    if response.status() == 200 {
                                                                        web_sys::console::log_1(&"Successfully sent test email".into());
                                                                    } else {
                                                                        if let Ok(error_data) = response.json::<serde_json::Value>().await {
                                                                            error.set(Some(format!("Failed to send email: {}",
                                                                                error_data.get("error").and_then(|e| e.as_str()).unwrap_or("Unknown error"))));
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
                                    {"Test Send Email"}
                                </button>
                            </>
                        }
                    </div>
                } else {
                    <div class="imap-form" style="display: flex; flex-wrap: wrap; gap: 10px; align-items: center;">
                        <select
                            onchange={onchange_imap_provider}
                            style="flex: 1 1 100px; padding: 8px; border-radius: 4px; background-color: #2a2a2a; color: #ccc; border: 1px solid #444; appearance: none;"
                        >
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
                            style="flex: 2 1 200px; padding: 8px; border-radius: 4px; background-color: #2a2a2a; color: #ccc; border: 1px solid #444;"
                        />
                        <input
                            type="password"
                            placeholder="Password or App Password"
                            value={(*imap_password).clone()}
                            onchange={onchange_imap_password}
                            style="flex: 2 1 200px; padding: 8px; border-radius: 4px; background-color: #2a2a2a; color: #ccc; border: 1px solid #444;"
                        />
                        if *imap_provider == "custom" {
                            <>
                                <input
                                    type="text"
                                    placeholder="IMAP Server (e.g., mail.privateemail.com)"
                                    value={(*imap_server).clone()}
                                    onchange={onchange_imap_server}
                                    style="flex: 2 1 200px; padding: 8px; border-radius: 4px; background-color: #2a2a2a; color: #ccc; border: 1px solid #444;"
                                />
                                <input
                                    type="number"
                                    placeholder="IMAP Port (e.g., 993)"
                                    value={(*imap_port).clone()}
                                    onchange={onchange_imap_port}
                                    style="flex: 1 1 100px; padding: 8px; border-radius: 4px; background-color: #2a2a2a; color: #ccc; border: 1px solid #444;"
                                />
                            </>
                        }
                    </div>
                    <button
                        onclick={onclick_imap_connect}
                        class="connect-button"
                        style="margin-top: 10px; padding: 8px 16px; background-color: #3b82f6; color: white; border: none; border-radius: 4px; cursor: pointer;"
                    >
                        {"Connect"}
                    </button>
                }
            if let Some(err) = (*error).as_ref() {
                <div class="error-message">
                    {err}
                </div>
            }
        } else {
            <div class="upgrade-prompt">
                <div class="upgrade-content">
                    <h3>{"Upgrade to Enable Email Integration"}</h3>
                    <p>{"Email integration is available for premium plan subscribers. Upgrade your plan to connect your email account and manage your emails through SMS and voice calls."}</p>
                    <a href="/pricing" class="upgrade-button">
                        {"View Pricing Plans"}
                    </a>
                </div>
            </div>
            {
                if *imap_connected {
                    html! {
                    <div class="imap-controls">
                        <button
                            onclick={onclick_imap_disconnect}
                            class="disconnect-button"
                        >
                            {"Disconnect previous connection"}
                        </button>
                    </div>
                    }
                } else {
                    html! {}
                }
            }
        }
        </div>
    }
}
