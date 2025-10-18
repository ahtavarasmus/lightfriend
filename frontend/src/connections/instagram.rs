use yew::prelude::*;
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use web_sys::{window, Event};
use wasm_bindgen::JsCast;
use crate::config;
use wasm_bindgen_futures::spawn_local;
use web_sys::js_sys;

#[derive(Deserialize, Clone, Debug)]
struct InstagramStatus {
    connected: bool,
    status: String,
    created_at: i32,
}

#[derive(Serialize)]
struct InstagramLoginRequest {
    curl_paste: String,
}

#[derive(Deserialize, Clone, Debug)]
struct InstagramConnectionResponse {
    message: String,
}

#[derive(Properties, PartialEq)]
pub struct InstagramProps {
    pub user_id: i32,
}

#[function_component(InstagramConnect)]
pub fn instagram_connect(props: &InstagramProps) -> Html {
    let connection_status = use_state(|| None::<InstagramStatus>);
    let error = use_state(|| None::<String>);
    let is_connecting = use_state(|| false);
    let show_disconnect_modal = use_state(|| false);
    let is_disconnecting = use_state(|| false);
    let show_auth_form = use_state(|| false);
    let curl_paste = use_state(|| String::new());

    let fetch_status = {
        let connection_status = connection_status.clone();
        let error = error.clone();
        Callback::from(move |_| {
            let connection_status = connection_status.clone();
            let error = error.clone();
            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|storage| storage.get_item("token").ok())
                .flatten()
            {
                spawn_local(async move {
                    match Request::get(&format!("{}/api/auth/instagram/status", config::get_backend_url()))
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await
                    {
                        Ok(response) => {
                            let status_code = response.status();
                            let response_text = response.text().await.unwrap_or_else(|_| "".to_string());
                            if status_code == 200 {
                                match serde_json::from_str::<InstagramStatus>(&response_text) {
                                    Ok(status) => {
                                        connection_status.set(Some(status));
                                        error.set(None);
                                    }
                                    Err(parse_err) => {
                                        error.set(Some(format!("Failed to parse Instagram status: {} (text: {})", parse_err, response_text)));
                                    }
                                }
                            } else {
                                if !response_text.is_empty() {
                                    match serde_json::from_str::<serde_json::Value>(&response_text) {
                                        Ok(data) => {
                                            if let Some(err_msg) = data.get("error").and_then(|v| v.as_str()) {
                                                error.set(Some(err_msg.to_string()));
                                            } else {
                                                error.set(Some(format!("Server error {}: {}", status_code, response_text)));
                                            }
                                        }
                                        Err(_) => {
                                            error.set(Some(format!("Server error {}: {}", status_code, response_text)));
                                        }
                                    }
                                } else {
                                    error.set(Some(format!("Server error {}: no response body", status_code)));
                                }
                            }
                        }
                        Err(send_err) => {
                            error.set(Some(format!("Failed to fetch Instagram status: {}", send_err)));
                        }
                    }
                });
            }
        })
    };

    {
        let fetch_status = fetch_status.clone();
        use_effect_with_deps(move |_| {
            fetch_status.emit(());
            || ()
        }, ());
    }

    let start_auth = {
        let show_auth_form = show_auth_form.clone();
        Callback::from(move |_| {
            show_auth_form.set(true);
        })
    };

    let submit_curl = {
        let is_connecting = is_connecting.clone();
        let error = error.clone();
        let fetch_status = fetch_status.clone();
        let curl_paste = curl_paste.clone();
        let show_auth_form = show_auth_form.clone();
        Callback::from(move |_| {
            let is_connecting = is_connecting.clone();
            let error = error.clone();
            let fetch_status = fetch_status.clone();
            let curl_paste_value = (*curl_paste).clone();
            let show_auth_form = show_auth_form.clone();
            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|storage| storage.get_item("token").ok())
                .flatten()
            {
                is_connecting.set(true);
                spawn_local(async move {
                    let request_body = InstagramLoginRequest {
                        curl_paste: curl_paste_value.clone(),
                    };
                    match Request::post(&format!("{}/api/auth/instagram/connect", config::get_backend_url()))
                        .header("Authorization", &format!("Bearer {}", token))
                        .header("Content-Type", "application/json")
                        .body(serde_json::to_string(&request_body).unwrap())
                        .send()
                        .await
                    {
                        Ok(response) => {
                            let status_code = response.status();
                            let response_text = response.text().await.unwrap_or_else(|_| "".to_string());
                            web_sys::console::log_1(&format!("Instagram connect response status: {}, body: {}", status_code, response_text).into());
                            if status_code == 200 {
                                match serde_json::from_str::<serde_json::Value>(&response_text) {
                                    Ok(data) => {
                                        if data.get("message").is_some() {
                                            error.set(None);
                                            show_auth_form.set(false);
                                            let poll_interval = 5000;
                                            let poll_duration = 300000;
                                            let start_time = js_sys::Date::now();
                                            fn create_poll_fn(
                                                start_time: f64,
                                                poll_duration: i32,
                                                poll_interval: i32,
                                                is_connecting: UseStateHandle<bool>,
                                                error: UseStateHandle<Option<String>>,
                                                fetch_status: Callback<()>,
                                            ) -> Box<dyn Fn()> {
                                                Box::new(move || {
                                                    if js_sys::Date::now() - start_time > poll_duration as f64 {
                                                        is_connecting.set(false);
                                                        error.set(Some("Connection attempt timed out".to_string()));
                                                        return;
                                                    }
                                                    fetch_status.emit(());
                                                    let is_connecting = is_connecting.clone();
                                                    let error = error.clone();
                                                    let fetch_status = fetch_status.clone();
                                                    let poll_fn = create_poll_fn(
                                                        start_time,
                                                        poll_duration,
                                                        poll_interval,
                                                        is_connecting,
                                                        error,
                                                        fetch_status,
                                                    );
                                                    let handle = gloo_timers::callback::Timeout::new(
                                                        poll_interval as u32,
                                                        move || poll_fn(),
                                                    );
                                                    handle.forget();
                                                })
                                            }
                                            let poll_fn = create_poll_fn(
                                                start_time,
                                                poll_duration,
                                                poll_interval,
                                                is_connecting.clone(),
                                                error.clone(),
                                                fetch_status.clone(),
                                            );
                                            poll_fn();
                                        } else {
                                            is_connecting.set(false);
                                            error.set(Some(format!("Unexpected successful response: {}", response_text)));
                                        }
                                    }
                                    Err(parse_err) => {
                                        is_connecting.set(false);
                                        error.set(Some(format!("Failed to parse success response: {} (text: {})", parse_err, response_text)));
                                    }
                                }
                            } else {
                                if !response_text.is_empty() {
                                    match serde_json::from_str::<serde_json::Value>(&response_text) {
                                        Ok(data) => {
                                            if let Some(err_msg) = data.get("error").and_then(|v| v.as_str()) {
                                                is_connecting.set(false);
                                                error.set(Some(err_msg.to_string()));
                                            } else {
                                                is_connecting.set(false);
                                                error.set(Some(format!("Server error {}: {}", status_code, response_text)));
                                            }
                                        }
                                        Err(_) => {
                                            is_connecting.set(false);
                                            error.set(Some(format!("Server error {}: {}", status_code, response_text)));
                                        }
                                    }
                                } else {
                                    is_connecting.set(false);
                                    error.set(Some(format!("Server error {}: no response body", status_code)));
                                }
                            }
                        }
                        Err(send_err) => {
                            web_sys::console::error_1(&format!("Failed to send Instagram connect request: {}", send_err).into());
                            is_connecting.set(false);
                            error.set(Some(format!("Failed to start Instagram connection: {}", send_err)));
                        }
                    }
                });
            }
        })
    };

    let disconnect = {
        let connection_status = connection_status.clone();
        let error = error.clone();
        let is_disconnecting = is_disconnecting.clone();
        let show_disconnect_modal = show_disconnect_modal.clone();
        Callback::from(move |_| {
            let connection_status = connection_status.clone();
            let error = error.clone();
            let is_disconnecting = is_disconnecting.clone();
            let show_disconnect_modal = show_disconnect_modal.clone();
            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|storage| storage.get_item("token").ok())
                .flatten()
            {
                is_disconnecting.set(true);
                spawn_local(async move {
                    match Request::delete(&format!("{}/api/auth/instagram/disconnect", config::get_backend_url()))
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await
                    {
                        Ok(response) => {
                            if response.ok() {
                                connection_status.set(Some(InstagramStatus {
                                    connected: false,
                                    status: "not_connected".to_string(),
                                    created_at: (js_sys::Date::now() as i32),
                                }));
                                error.set(None);
                            } else {
                                let text = response.text().await.unwrap_or_default();
                                error.set(Some(format!("Failed to disconnect Instagram: status {}, {}", response.status(), text)));
                            }
                        }
                        Err(send_err) => {
                            error.set(Some(format!("Failed to send disconnect request: {}", send_err)));
                        }
                    }
                    is_disconnecting.set(false);
                    show_disconnect_modal.set(false);
                });
            } else {
                show_disconnect_modal.set(false);
            }
        })
    };

    html! {
        <div class="instagram-connect">
            <div class="service-header">
                <div class="service-name">
                    <img src="https://upload.wikimedia.org/wikipedia/commons/e/e7/Instagram_logo_2016.svg" alt="Instagram Logo" />
                    {"Instagram"}
                </div>
                if let Some(status) = (*connection_status).clone() {
                    if status.connected {
                        <span class="service-status">{"Connected ✓"}</span>
                    }
                }
                <button class="info-button" onclick={Callback::from(|_| {
                    if let Some(element) = web_sys::window()
                        .and_then(|w| w.document())
                        .and_then(|d| d.get_element_by_id("instagram-info"))
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
            <div id="instagram-info" class="info-section" style="display: none">
                <h4>{"How It Works"}</h4>
                <div class="info-subsection">
                    <h5>{"SMS and Voice Call Tools"}</h5>
                    <ul>
                        <li>{"Fetch Instagram Messages: Get recent Instagram messages from a specific time period"}</li>
                        <li>{"Fetch Chat Messages: Get messages from a specific Instagram chat or contact"}</li>
                        <li>{"Search Contacts: Search for Instagram contacts or chat rooms by name"}</li>
                        <li>{"Send Message: Send a Instagram message to a specific recipient (will ask for confirmation before sending)"}</li>
                    </ul>
                </div>
                <div class="info-subsection security-notice">
                    <h5>{"Security & Privacy"}</h5>
                    <p>{"Your security is our priority. Here's how we protect your messages:"}</p>
                    <ul>
                        <li>{"Your Instagram messages are end-to-end encrypted between Instagram and our Matrix server. We use the same trusted Matrix server and Instagram bridge technology as Beeper, with robust encryption and strict access controls to protect your data at every step."}</li>
                    </ul>
                    <p class="security-recommendation">{"Note: While we maintain high security standards, SMS and voice calls use standard cellular networks. For maximum privacy, use Instagram directly for sensitive communications."}</p>
                </div>
            </div>
            if let Some(status) = (*connection_status).clone() {
                <div class="connection-status">
                    if status.connected {
                        <>
                            {
                                if js_sys::Date::now() - (status.created_at as f64 * 1000.0) <= 900000.0 {
                                    html! {
                                        <div class="sync-indicator">
                                            <div class="sync-spinner"></div>
                                            <p>{"Building the connection bridge... This may take up to 10 minutes. Message history will not be fetched. Lightfriend can only fetch messages from current time onwards."}</p>
                                        </div>
                                    }
                                } else {
                                    html! {}
                                }
                            }
                            <div class="button-group">
                                <p class="service-description">
                                    {"Send and receive Instagram messages through SMS or voice calls."}
                                </p>
                                <button onclick={
                                    let show_disconnect_modal = show_disconnect_modal.clone();
                                    Callback::from(move |_| show_disconnect_modal.set(true))
                                } class="disconnect-button">
                                    {"Disconnect"}
                                </button>
                                if *show_disconnect_modal {
                                    <div class="modal-overlay">
                                        <div class="modal-content">
                                            <h3>{"Confirm Disconnection"}</h3>
                                            <p>{"Are you sure you want to disconnect Instagram? This will:"}</p>
                                            <ul>
                                                <li>{"Stop all Instagram message forwarding"}</li>
                                                <li>{"Delete all your Instagram data from our servers"}</li>
                                                <li>{"Require reconnection to use Instagram features again"}</li>
                                            </ul>
                                            if *is_disconnecting {
                                                <p class="disconnecting-message">{"Disconnecting Instagram... Please wait."}</p>
                                            }
                                            <div class="modal-buttons">
                                                <button onclick={
                                                    let show_disconnect_modal = show_disconnect_modal.clone();
                                                    Callback::from(move |_| show_disconnect_modal.set(false))
                                                } class="cancel-button" disabled={*is_disconnecting}>
                                                    {"Cancel"}
                                                </button>
                                                <button onclick={
                                                    let disconnect = disconnect.clone();
                                                    Callback::from(move |_| {
                                                        disconnect.emit(());
                                                    })
                                                } class="confirm-disconnect-button" disabled={*is_disconnecting}>
                                                    if *is_disconnecting {
                                                        <span class="button-spinner"></span> {"Disconnecting..."}
                                                    } else {
                                                        {"Yes, Disconnect"}
                                                    }
                                                </button>
                                            </div>
                                        </div>
                                    </div>
                                }
                                {
                                    html! {
                                        <button onclick={{
                                            let fetch_status = fetch_status.clone();
                                            Callback::from(move |_| {
                                                let fetch_status = fetch_status.clone();
                                                if let Some(token) = window()
                                                    .and_then(|w| w.local_storage().ok())
                                                    .flatten()
                                                    .and_then(|storage| storage.get_item("token").ok())
                                                    .flatten()
                                                {
                                                    spawn_local(async move {
                                                        match Request::post(&format!("{}/api/auth/instagram/resync", config::get_backend_url()))
                                                            .header("Authorization", &format!("Bearer {}", token))
                                                            .send()
                                                            .await
                                                        {
                                                            Ok(response) => {
                                                                if response.ok() {
                                                                    web_sys::console::log_1(&"Instagram resync initiated".into());
                                                                    fetch_status.emit(());
                                                                } else {
                                                                    let text = response.text().await.unwrap_or_default();
                                                                    web_sys::console::error_1(&format!("Failed to resync Instagram: status {}, {}", response.status(), text).into());
                                                                }
                                                            }
                                                            Err(e) => {
                                                                web_sys::console::error_1(&format!("Failed to send resync request: {}", e).into());
                                                            }
                                                        }
                                                    });
                                                }
                                            })
                                        }} class="resync-button">
                                            {"Resync Instagram"}
                                        </button>
                                    }
                                }
                            </div>
                            {
                                html! {
                                    <>
                                        <button onclick={{
                                            Callback::from(move |_| {
                                                if let Some(token) = window()
                                                    .and_then(|w| w.local_storage().ok())
                                                    .flatten()
                                                    .and_then(|storage| storage.get_item("token").ok())
                                                    .flatten()
                                                {
                                                    spawn_local(async move {
                                                        match Request::get(&format!("{}/api/instagram/test-messages", config::get_backend_url()))
                                                            .header("Authorization", &format!("Bearer {}", token))
                                                            .send()
                                                            .await
                                                        {
                                                            Ok(response) => {
                                                                web_sys::console::log_1(&format!("Response status: {}", response.status()).into());
                                                                match response.text().await {
                                                                    Ok(text) => {
                                                                        web_sys::console::log_1(&format!("Raw response: {}", text).into());
                                                                        match serde_json::from_str::<serde_json::Value>(&text) {
                                                                            Ok(data) => {
                                                                                web_sys::console::log_1(&format!("Messages: {:?}", data).into());
                                                                            }
                                                                            Err(e) => {
                                                                                web_sys::console::error_1(&format!("Failed to parse JSON: {}", e).into());
                                                                            }
                                                                        }
                                                                    }
                                                                    Err(e) => {
                                                                        web_sys::console::error_1(&format!("Failed to get response text: {}", e).into());
                                                                    }
                                                                }
                                                            }
                                                            Err(e) => {
                                                                web_sys::console::error_1(&format!("Failed to fetch messages: {}", e).into());
                                                            }
                                                        }
                                                    });
                                                }
                                            })
                                        }} class="test-button">
                                            {"Test Fetch Messages"}
                                        </button>
                                        <button onclick={{
                                            Callback::from(move |_| {
                                                if let Some(token) = window()
                                                    .and_then(|w| w.local_storage().ok())
                                                    .flatten()
                                                    .and_then(|storage| storage.get_item("token").ok())
                                                    .flatten()
                                                {
                                                    spawn_local(async move {
                                                        let request_body = serde_json::json!({
                                                            "chat_name": "test",
                                                            "message": "testing"
                                                        });
                                                        match Request::post(&format!("{}/api/instagram/send", config::get_backend_url()))
                                                            .header("Authorization", &format!("Bearer {}", token))
                                                            .header("Content-Type", "application/json")
                                                            .body(serde_json::to_string(&request_body).unwrap())
                                                            .send()
                                                            .await
                                                        {
                                                            Ok(response) => {
                                                                web_sys::console::log_1(&format!("Send message response status: {}", response.status()).into());
                                                                match response.text().await {
                                                                    Ok(text) => {
                                                                        web_sys::console::log_1(&format!("Send message response: {}", text).into());
                                                                    }
                                                                    Err(e) => {
                                                                        web_sys::console::error_1(&format!("Failed to get send message response text: {}", e).into());
                                                                    }
                                                                }
                                                            }
                                                            Err(e) => {
                                                                web_sys::console::error_1(&format!("Failed to send test message: {}", e).into());
                                                            }
                                                        }
                                                    });
                                                }
                                            })
                                        }} class="test-button test-send-button">
                                            {"Test Send Message"}
                                        </button>
                                        <button onclick={{
                                            Callback::from(move |_| {
                                                if let Some(token) = window()
                                                    .and_then(|w| w.local_storage().ok())
                                                    .flatten()
                                                    .and_then(|storage| storage.get_item("token").ok())
                                                    .flatten()
                                                {
                                                    spawn_local(async move {
                                                        let request_body = serde_json::json!({
                                                            "search_term": "test"
                                                        });
                                                        match Request::post(&format!("{}/api/instagram/search-rooms", config::get_backend_url()))
                                                            .header("Authorization", &format!("Bearer {}", token))
                                                            .header("Content-Type", "application/json")
                                                            .body(serde_json::to_string(&request_body).unwrap())
                                                            .send()
                                                            .await
                                                        {
                                                            Ok(response) => {
                                                                web_sys::console::log_1(&format!("Search rooms response status: {}", response.status()).into());
                                                                match response.text().await {
                                                                    Ok(text) => {
                                                                        web_sys::console::log_1(&format!("Search rooms response: {}", text).into());
                                                                    }
                                                                    Err(e) => {
                                                                        web_sys::console::error_1(&format!("Failed to get search rooms response text: {}", e).into());
                                                                    }
                                                                }
                                                            }
                                                            Err(e) => {
                                                                web_sys::console::error_1(&format!("Failed to search rooms: {}", e).into());
                                                            }
                                                        }
                                                    });
                                                }
                                            })
                                        }} class="test-button test-search-button">
                                            {"Test Search Rooms"}
                                        </button>
                                    </>
                                }
                            }
                        </>
                    } else {
                        if *is_connecting {
                            <div class="loading-container">
                                <p class="connect-instruction">{"Authenticating... This may take a moment."}</p>
                                <div class="loading-spinner"></div>
                            </div>
                        } else if *show_auth_form {
                            <div class="auth-form-container">
                                <p class="connect-instruction">{"Follow these steps to connect Instagram:"}</p>
                                <ol class="auth-instructions">
                                    <li>{"Open Instagram.com in a private/incognito browser window."}</li>
                                    <li>{"Open browser devtools (F12) and go to the Network tab."}</li>
                                    <li>{"Filter for XHR requests and search for 'graphql'."}</li>
                                    <li>{"Log in to your Instagram account normally."}</li>
                                    <li>{"After login, right-click one of the graphql requests, choose 'Copy as cURL' (use POSIX version on Windows)."}</li>
                                    <li>{"Paste the cURL command below and submit."}</li>
                                </ol>
                                <textarea
                                    class="curl-textarea"
                                    placeholder="Paste cURL here..."
                                    value={(*curl_paste).clone()}
                                    oninput={let curl_paste = curl_paste.clone(); Callback::from(move |e: InputEvent| {
                                        let value = e.target_unchecked_into::<web_sys::HtmlTextAreaElement>().value();
                                        curl_paste.set(value);
                                    })}
                                />
                                <button onclick={submit_curl.clone()} class="submit-button">
                                    {"Submit"}
                                </button>
                                <p class="auth-note">{"Note: Meta may flag suspicious activity. Enable 2FA to reduce risks. If blocked, complete Meta's tasks."}</p>
                            </div>
                        } else {
                            <p class="service-description">
                                {"Send and receive Instagram messages through SMS or voice calls."}
                            </p>
                            <button onclick={start_auth} class="connect-button">
                                {"Connect Instagram"}
                            </button>
                        }
                    }
                </div>
            } else {
                <p>{"Loading connection status..."}</p>
            }
            if let Some(error_msg) = (*error).clone() {
                <div class="error-message">
                    {error_msg}
                </div>
            }
            <style>
                {r#"
                    .instagram-connect {
                        background: rgba(0, 0, 0, 0.2);
                        border: 1px solid rgba(0, 136, 204, 0.2);
                        border-radius: 12px;
                        width: 100%;
                        padding: 1.5rem;
                        margin: 1rem 0;
                        transition: all 0.3s ease;
                        color: #fff;
                    }
                    .instagram-connect:hover {
                        transform: translateY(-2px);
                        border-color: rgba(0, 136, 204, 0.4);
                        box-shadow: 0 4px 20px rgba(0, 136, 204, 0.1);
                    }
                    .service-header {
                        display: flex;
                        align-items: center;
                        gap: 1rem;
                        flex-wrap: wrap;
                    }
                    .service-name {
                        flex: 1;
                        min-width: 150px;
                    }
                    .service-status {
                        white-space: nowrap;
                    }
                    .service-name {
                        display: flex;
                        align-items: center;
                        gap: 0.5rem;
                    }
                    .service-name img {
                        width: 24px !important;
                        height: 24px !important;
                    }
                    .service-status {
                        color: #4CAF50;
                        font-weight: 500;
                    }
                    .info-button {
                        background: none;
                        border: none;
                        color: #0088cc;
                        font-size: 1.2rem;
                        cursor: pointer;
                        padding: 0.5rem;
                        border-radius: 50%;
                        width: 2rem;
                        height: 2rem;
                        display: flex;
                        align-items: center;
                        justify-content: center;
                        transition: all 0.3s ease;
                        margin-left: auto;
                    }
                    .info-button:hover {
                        background: rgba(0, 136, 204, 0.1);
                        transform: scale(1.1);
                    }
                    .auth-form-container {
                        margin: 1.5rem 0;
                    }
                    .auth-instructions {
                        color: #CCC;
                        margin-bottom: 1rem;
                        padding-left: 1.5rem;
                    }
                    .auth-instructions li {
                        margin-bottom: 0.5rem;
                    }
                    .curl-textarea {
                        width: 100%;
                        height: 150px;
                        background: rgba(0, 0, 0, 0.3);
                        border: 1px solid rgba(255, 255, 255, 0.1);
                        border-radius: 8px;
                        color: #fff;
                        padding: 1rem;
                        font-family: monospace;
                        margin-bottom: 1rem;
                    }
                    .submit-button {
                        background: #0088cc;
                        color: white;
                        border: none;
                        padding: 0.8rem 1.5rem;
                        border-radius: 8px;
                        cursor: pointer;
                        transition: all 0.3s ease;
                        width: 100%;
                    }
                    .submit-button:hover {
                        background: #0077b3;
                        transform: translateY(-2px);
                        box-shadow: 0 4px 12px rgba(0, 136, 204, 0.3);
                    }
                    .auth-note {
                        color: #999;
                        font-size: 0.9rem;
                        margin-top: 1rem;
                    }
                    .loading-container {
                        text-align: center;
                        margin: 2rem 0;
                    }
                    .loading-spinner {
                        display: inline-block;
                        width: 40px;
                        height: 40px;
                        border: 4px solid rgba(0, 136, 204, 0.1);
                        border-radius: 50%;
                        border-top-color: #0088cc;
                        animation: spin 1s ease-in-out infinite;
                        margin: 1rem auto;
                    }
                    .button-group {
                        display: flex;
                        flex-direction: column;
                        gap: 1rem;
                        margin-bottom: 1rem;
                    }
                    @media (min-width: 768px) {
                        .button-group {
                            flex-direction: row;
                        }
                    }
                    .resync-button {
                        background: linear-gradient(45deg, #0088cc, #0099dd);
                        color: white;
                        border: none;
                        padding: 0.8rem 1.5rem;
                        border-radius: 8px;
                        cursor: pointer;
                        transition: all 0.3s ease;
                        flex: 1;
                    }
                    .resync-button:hover {
                        transform: translateY(-2px);
                        box-shadow: 0 4px 20px rgba(0, 136, 204, 0.3);
                    }
                    .connect-button, .disconnect-button {
                        background: #0088cc;
                        color: white;
                        border: none;
                        padding: 0.8rem 1.5rem;
                        border-radius: 8px;
                        cursor: pointer;
                        transition: all 0.3s ease;
                        margin-top: 1rem;
                    }
                    .disconnect-button {
                        background: transparent;
                        border: 1px solid rgba(255, 99, 71, 0.3);
                        color: #FF6347;
                    }
                    .disconnect-button:hover {
                        background: rgba(255, 99, 71, 0.1);
                        border-color: rgba(255, 99, 71, 0.5);
                        transform: translateY(-2px);
                    }
                    .connect-button:hover {
                        background: #0077b3;
                        transform: translateY(-2px);
                        box-shadow: 0 4px 12px rgba(0, 136, 204, 0.3);
                    }
                    .error-message {
                        color: #FF4B4B;
                        background: rgba(255, 75, 75, 0.1);
                        border: 1px solid rgba(255, 75, 75, 0.2);
                        border-radius: 8px;
                        padding: 1rem;
                        margin-top: 1rem;
                    }
                    .sync-indicator {
                        display: flex;
                        align-items: center;
                        background: rgba(0, 136, 204, 0.1);
                        border-radius: 8px;
                        padding: 1rem;
                        margin-bottom: 1rem;
                        color: #0088cc;
                    }
                    .sync-spinner {
                        display: inline-block;
                        width: 20px;
                        height: 20px;
                        border: 3px solid rgba(0, 136, 204, 0.1);
                        border-radius: 50%;
                        border-top-color: #0088cc;
                        animation: spin 1s ease-in-out infinite;
                        margin-right: 10px;
                    }
                    .test-button {
                        background: linear-gradient(45deg, #4CAF50, #45a049);
                        color: white;
                        border: none;
                        width: 100%;
                        padding: 1rem;
                        border-radius: 8px;
                        font-size: 1rem;
                        cursor: pointer;
                        transition: all 0.3s ease;
                        margin-top: 1rem;
                    }
                    .test-button:hover {
                        transform: translateY(-2px);
                        box-shadow: 0 4px 20px rgba(76, 175, 80, 0.3);
                    }
                    .test-send-button {
                        background: linear-gradient(45deg, #FF8C00, #FFA500);
                        margin-top: 0.5rem;
                    }
                    .test-send-button:hover {
                        box-shadow: 0 4px 20px rgba(255, 140, 0, 0.3);
                    }
                    .test-search-button {
                        background: linear-gradient(45deg, #9C27B0, #BA68C8);
                        margin-top: 0.5rem;
                    }
                    .test-search-button:hover {
                        box-shadow: 0 4px 20px rgba(156, 39, 176, 0.3);
                    }
                    .upgrade-prompt {
                        background: rgba(0, 136, 204, 0.05);
                        border: 1px solid rgba(0, 136, 204, 0.1);
                        border-radius: 12px;
                        padding: 1.8rem;
                        text-align: center;
                        margin: 0.8rem 0;
                    }
                    .upgrade-content h3 {
                        color: #0088cc;
                        margin-bottom: 1rem;
                        font-size: 1.2rem;
                    }
                    .upgrade-button {
                        display: inline-block;
                        background: #0088cc;
                        color: white;
                        text-decoration: none;
                        padding: 1rem 2rem;
                        border-radius: 8px;
                        font-weight: bold;
                        transition: all 0.3s ease;
                        margin-top: 1rem;
                    }
                    .upgrade-button:hover {
                        background: #0077b3;
                        transform: translateY(-2px);
                        box-shadow: 0 4px 12px rgba(0, 136, 204, 0.3);
                    }
                    @keyframes spin {
                        to { transform: rotate(360deg); }
                    }
                    .security-notice {
                        background: rgba(0, 136, 204, 0.1);
                        padding: 1.2rem;
                        border-radius: 8px;
                        border: 1px solid rgba(0, 136, 204, 0.2);
                    }
                    .security-notice p {
                        margin: 0 0 1rem 0;
                        color: #CCC;
                    }
                    .security-recommendation {
                        font-style: italic;
                        color: #999 !important;
                        margin-top: 1rem !important;
                        font-size: 0.9rem;
                        padding-top: 1rem;
                        border-top: 1px solid rgba(0, 136, 204, 0.1);
                    }
                    .modal-overlay {
                        position: fixed;
                        top: 0;
                        left: 0;
                        right: 0;
                        bottom: 0;
                        background: rgba(0, 0, 0, 0.85);
                        display: flex;
                        justify-content: center;
                        align-items: center;
                        z-index: 1000;
                    }
                    .modal-content {
                        background: #1a1a1a;
                        border: 1px solid rgba(0, 136, 204, 0.2);
                        border-radius: 12px;
                        padding: 2rem;
                        max-width: 500px;
                        width: 90%;
                        box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
                    }
                    .modal-content h3 {
                        color: #FF6347;
                        margin-bottom: 1rem;
                    }
                    .modal-content p {
                        color: #CCC;
                        margin-bottom: 1rem;
                    }
                    .modal-content ul {
                        margin-bottom: 2rem;
                        padding-left: 1.5rem;
                    }
                    .modal-content li {
                        color: #999;
                        margin-bottom: 0.5rem;
                    }
                    .modal-buttons {
                        display: flex;
                        gap: 1rem;
                        justify-content: flex-end;
                    }
                    .cancel-button {
                        background: transparent;
                        border: 1px solid rgba(204, 204, 204, 0.3);
                        color: #CCC;
                        padding: 0.8rem 1.5rem;
                        border-radius: 8px;
                        cursor: pointer;
                        transition: all 0.3s ease;
                    }
                    .cancel-button:hover {
                        background: rgba(204, 204, 204, 0.1);
                        transform: translateY(-2px);
                    }
                    .confirm-disconnect-button {
                        background: linear-gradient(45deg, #FF6347, #FF4500);
                        color: white;
                        border: none;
                        padding: 0.8rem 1.5rem;
                        border-radius: 8px;
                        cursor: pointer;
                        transition: all 0.3s ease;
                    }
                    .confirm-disconnect-button:hover {
                        transform: translateY(-2px);
                        box-shadow: 0 4px 12px rgba(255, 99, 71, 0.3);
                    }
                    .button-spinner {
                        display: inline-block;
                        width: 16px;
                        height: 16px;
                        border: 2px solid rgba(255, 255, 255, 0.3);
                        border-radius: 50%;
                        border-top-color: #fff;
                        animation: spin 1s ease-in-out infinite;
                        margin-right: 8px;
                        vertical-align: middle;
                    }
                    .disconnecting-message {
                        color: #0088cc;
                        margin: 1rem 0;
                        font-weight: bold;
                    }
                "#}
            </style>
        </div>
    }
}
