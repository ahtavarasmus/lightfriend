use yew::prelude::*;
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use web_sys::{window, Event};
use wasm_bindgen::JsCast;
use crate::config;
use wasm_bindgen_futures::spawn_local;
use web_sys::js_sys;
#[derive(Deserialize, Clone, Debug)]
struct WhatsappStatus {
    connected: bool,
    status: String,
    created_at: i32,
}
#[derive(Deserialize)]
struct WhatsappConnectionResponse {
    pairing_code: String,
}
#[derive(Properties, PartialEq)]
pub struct WhatsappProps {
    pub user_id: i32,
}
#[function_component(WhatsappConnect)]
pub fn whatsapp_connect(props: &WhatsappProps) -> Html {
    let connection_status = use_state(|| None::<WhatsappStatus>);
    let qr_code = use_state(|| None::<String>);
    let error = use_state(|| None::<String>);
    let is_connecting = use_state(|| false);
    let show_disconnect_modal = use_state(|| false);
    let is_disconnecting = use_state(|| false);
    // Function to fetch WhatsApp status
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
                    match Request::get(&format!("{}/api/auth/whatsapp/status", config::get_backend_url()))
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await
                    {
                        Ok(response) => {
                            match response.json::<WhatsappStatus>().await {
                                Ok(status) => {
                                    connection_status.set(Some(status));
                                    error.set(None);
                                }
                                Err(_) => {
                                    error.set(Some("Failed to parse WhatsApp status".to_string()));
                                }
                            }
                        }
                        Err(_) => {
                            error.set(Some("Failed to fetch WhatsApp status".to_string()));
                        }
                    }
                });
            }
        })
    };
    // Effect to fetch initial status
    {
        let fetch_status = fetch_status.clone();
        use_effect_with_deps(move |_| {
            fetch_status.emit(());
            || ()
        }, ());
    }
    // Function to start WhatsApp connection
    let start_connection = {
        let is_connecting = is_connecting.clone();
        let qr_code = qr_code.clone();
        let error = error.clone();
        let fetch_status = fetch_status.clone();
        Callback::from(move |_| {
            let is_connecting = is_connecting.clone();
            let qr_code = qr_code.clone();
            let error = error.clone();
            let fetch_status = fetch_status.clone();
            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|storage| storage.get_item("token").ok())
                .flatten()
            {
                is_connecting.set(true);
                spawn_local(async move {
                    match Request::get(&format!("{}/api/auth/whatsapp/connect", config::get_backend_url()))
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await
                    {
                        Ok(response) => {
                            // Debug: Log the response status
                            web_sys::console::log_1(&format!("Response status: {}", response.status()).into());
                          
                            match response.json::<WhatsappConnectionResponse>().await {
                                Ok(connection_response) => {
                                    // Debug: Log that we received the verification code
                                    web_sys::console::log_1(&format!("Received verification code: {}", &connection_response.pairing_code).into());
                                  
                                    qr_code.set(Some(connection_response.pairing_code));
                                    error.set(None);
                                    // Start polling for status
                                    let poll_interval = 5000; // 5 seconds
                                    let poll_duration = 300000; // 5 minutes
                                    let start_time = js_sys::Date::now();
                                    // Create a recursive polling function
                                    fn create_poll_fn(
                                        start_time: f64,
                                        poll_duration: i32,
                                        poll_interval: i32,
                                        is_connecting: UseStateHandle<bool>,
                                        qr_code: UseStateHandle<Option<String>>,
                                        error: UseStateHandle<Option<String>>,
                                        fetch_status: Callback<()>,
                                    ) -> Box<dyn Fn()> {
                                        Box::new(move || {
                                            if js_sys::Date::now() - start_time > poll_duration as f64 {
                                                is_connecting.set(false);
                                                qr_code.set(None);
                                                error.set(Some("Connection attempt timed out".to_string()));
                                                return;
                                            }
                                            fetch_status.emit(());
                                            // Clone all necessary values for the next iteration
                                            let is_connecting = is_connecting.clone();
                                            let qr_code = qr_code.clone();
                                            let error = error.clone();
                                            let fetch_status = fetch_status.clone();
                                            // Schedule next poll
                                            let poll_fn = create_poll_fn(
                                                start_time,
                                                poll_duration,
                                                poll_interval,
                                                is_connecting,
                                                qr_code,
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
                                    // Start the polling
                                    let poll_fn = create_poll_fn(
                                        start_time,
                                        poll_duration,
                                        poll_interval,
                                        is_connecting.clone(),
                                        qr_code.clone(),
                                        error.clone(),
                                        fetch_status.clone(),
                                    );
                                    poll_fn();
                                }
                                Err(_) => {
                                    is_connecting.set(false);
                                    error.set(Some("Failed to parse connection response".to_string()));
                                }
                            }
                        }
                        Err(_) => {
                            is_connecting.set(false);
                            error.set(Some("Failed to start WhatsApp connection".to_string()));
                        }
                    }
                });
            }
        })
    };
    // Function to disconnect WhatsApp
    let disconnect = {
        let connection_status = connection_status.clone();
        let error = error.clone();
        let is_disconnecting = is_disconnecting.clone(); // Clone the new state
        let show_disconnect_modal = show_disconnect_modal.clone(); // Clone to close modal later
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
                is_disconnecting.set(true); // Indicate disconnection is starting
                spawn_local(async move {
                    match Request::delete(&format!("{}/api/auth/whatsapp/disconnect", config::get_backend_url()))
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await
                    {
                        Ok(_) => {
                            connection_status.set(Some(WhatsappStatus {
                                connected: false,
                                status: "not_connected".to_string(),
                                created_at: (js_sys::Date::now() as i32),
                            }));
                            error.set(None);
                        }
                        Err(_) => {
                            error.set(Some("Failed to disconnect WhatsApp".to_string()));
                        }
                    }
                    is_disconnecting.set(false); // Disconnection complete
                    show_disconnect_modal.set(false); // Close the modal
                });
            } else {
                show_disconnect_modal.set(false); // Close modal if no token
            }
        })
    };
    html! {
        <div class="whatsapp-connect">
            <div class="service-header">
                <div class="service-name">
                    <img src="https://upload.wikimedia.org/wikipedia/commons/6/6b/WhatsApp.svg" alt="WhatsApp" width="24" height="24"/>
                    {"WhatsApp"}
                </div>
                if let Some(status) = (*connection_status).clone() {
                    if status.connected {
                        <span class="service-status">{"Connected ✓"}</span>
                    }
                }
                <button class="info-button" onclick={Callback::from(|_| {
                    if let Some(element) = web_sys::window()
                        .and_then(|w| w.document())
                        .and_then(|d| d.get_element_by_id("whatsapp-info"))
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
            <div id="whatsapp-info" class="info-section" style="display: none">
                <h4>{"How It Works"}</h4>
                <div class="info-subsection">
                    <h5>{"SMS and Voice Call Tools"}</h5>
                    <ul>
                        <li>{"Fetch WhatsApp Messages: Get recent WhatsApp messages from a specific time period"}</li>
                        <li>{"Fetch Chat Messages: Get messages from a specific WhatsApp chat or contact"}</li>
                        <li>{"Search Contacts: Search for WhatsApp contacts or chat rooms by name"}</li>
                        <li>{"Send Message: Give platform, message content and recipient name and lightfriend will send the message. Message will only be sent 60 seconds later so if you or assistant made a mistake just type 'C' with sms or say 'cancel the message' with voice calls to discard the sent event."}</li>
                    </ul>
                </div>
                <div class="info-subsection security-notice">
                    <h5>{"Security & Privacy"}</h5>
                    <p>{"Your security is our priority. We use the same trusted Matrix server and WhatsApp bridge technology as Beeper Cloud, with robust encryption and strict access controls to protect your data at every step. When you disconnect your WhatsApp account, all your WhatsApp data will be automatically deleted from our servers."}</p>
                    <p class="security-recommendation">{"Note: While we maintain high security standards, SMS and voice calls use standard cellular networks. For maximum privacy, use WhatsApp directly for sensitive communications."}</p>
                </div>
            </div>
          
            if let Some(status) = (*connection_status).clone() {
                <div class="connection-status">
                    if status.connected {
                        <>
                            {
                                // Show sync indicator for 10 minutes after connection
                                if js_sys::Date::now() - (status.created_at as f64 * 1000.0) <= 300000.0 { // 10 minutes in milliseconds
                                    html! {
                                        <div class="sync-indicator">
                                            <div class="sync-spinner"></div>
                                            <p>{"Building the bridge... This may take up to 5 minutes. Only future messages will be visible. To send messages, contacts may need to message you first."}</p>
                                        </div>
                                    }
                                } else {
                                    html! {}
                                }
                            }
                            <div class="button-group">
                                <p class="service-description">
                                    {"Send and receive WhatsApp messages through SMS or voice calls."}
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
                                            <p>{"Are you sure you want to disconnect WhatsApp? This will:"}</p>
                                            <ul>
                                                <li>{"Stop all WhatsApp message forwarding"}</li>
                                                <li>{"Delete all your WhatsApp data from our servers"}</li>
                                                <li>{"Require reconnection to use WhatsApp features again"}</li>
                                            </ul>
                                            if *is_disconnecting {
                                                <p class="disconnecting-message">{"Disconnecting WhatsApp... Please wait."}</p>
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
                                    if props.user_id == 1 {
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
                                                            match Request::post(&format!("{}/api/auth/whatsapp/resync", config::get_backend_url()))
                                                                .header("Authorization", &format!("Bearer {}", token))
                                                                .send()
                                                                .await
                                                            {
                                                                Ok(_) => {
                                                                    web_sys::console::log_1(&"WhatsApp resync initiated".into());
                                                                    // Refresh status after resync
                                                                    fetch_status.emit(());
                                                                }
                                                                Err(e) => {
                                                                    web_sys::console::error_1(&format!("Failed to resync WhatsApp: {}", e).into());
                                                                }
                                                            }
                                                        });
                                                    }
                                                })
                                            }} class="resync-button">
                                                {"Resync WhatsApp"}
                                            </button>
                                        }
                                    } else {
                                        html! {}
                                    }
                                }
                            </div>
                            {
                                if props.user_id == 1 {
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
                                                            match Request::get(&format!("{}/api/whatsapp/test-messages", config::get_backend_url()))
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
                                                                "chat_name": "Rasmus Ähtävä",
                                                                "message": "rasmus testing matrix, sorry:)"
                                                            });
                                                            match Request::post(&format!("{}/api/whatsapp/send", config::get_backend_url()))
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
                                                                "search_term": "leevi"
                                                            });
                                                            match Request::post(&format!("{}/api/whatsapp/search-rooms", config::get_backend_url()))
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
                                } else {
                                    html! {}
                                }
                            }
                        </>
                    } else {
                            if *is_connecting {
                                if let Some(pairing_code) = (*qr_code).clone() {
                                    <div class="verification-code-container">
                                        <button onclick={{
                                            let error = error.clone();
                                            Callback::from(move |_| {
                                                let error = error.clone();
                                                if let Some(token) = window()
                                                    .and_then(|w| w.local_storage().ok())
                                                    .flatten()
                                                    .and_then(|storage| storage.get_item("token").ok())
                                                    .flatten()
                                                {
                                                    spawn_local(async move {
                                                        match Request::post(&format!("{}/api/auth/whatsapp/reset", config::get_backend_url()))
                                                            .header("Authorization", &format!("Bearer {}", token))
                                                            .send()
                                                            .await
                                                        {
                                                            Ok(_) => {
                                                                // Reload the page to restart the connection process
                                                                if let Some(window) = web_sys::window() {
                                                                    let location = window.location();
                                                                    let _ = location.reload();
                                                                }
                                                            }
                                                            Err(_) => {
                                                                error.set(Some("Failed to reset WhatsApp connection".to_string()));
                                                            }
                                                        }
                                                    });
                                                }
                                            })
                                        }} class="reset-button">
                                            {"Having trouble? Reset Connection"}
                                        </button>
                                        <p class="code-prompt">{"Enter this code in WhatsApp to connect:"}</p>
                                        <div class="verification-code">
                                            {pairing_code}
                                        </div>
                                        <p class="instruction-note">{"Note: Connects to the WhatsApp account linked to your current phone number. For a different account, update your number in Settings first, connect, then revert."}</p>
                                        <ol class="instruction-list">
                                            <li>{"Open WhatsApp on your phone"}</li>
                                            <li>{"Go to Settings > Linked Devices"}</li>
                                            <li>{"Tap 'Link a Device'"}</li>
                                            <li>{"Enter the code when prompted"}</li>
                                            <li>{"Wait a few minutes for the connection"}</li>
                                        </ol>
                                    </div>
                                } else {
                                    <div class="loading-container">
                                        <p>{"Generating connection code..."}</p>
                                        <div class="loading-spinner"></div>
                                    </div>
                                }
                            } else {
                                <p class="service-description">
                                    {"Send and receive WhatsApp messages through SMS or voice calls."}
                                </p>
                                <button onclick={start_connection} class="connect-button">
                                    {"Connect WhatsApp"}
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
                        color: #1E90FF;
                        margin: 1rem 0;
                        font-weight: bold;
                    }
                    .action-button:hover {
                        transform: translateY(-2px);
                        box-shadow: 0 4px 20px rgba(30, 144, 255, 0.3);
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
                        background: linear-gradient(45deg, #2196F3, #03A9F4);
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
                        box-shadow: 0 4px 20px rgba(33, 150, 243, 0.3);
                    }
                    .disconnect-button {
                        flex: 1;
                    }
                    .loading-container {
                        text-align: center;
                        margin: 2rem 0;
                    }
                    .loading-spinner {
                        display: inline-block;
                        width: 40px;
                        height: 40px;
                        border: 4px solid rgba(30, 144, 255, 0.1);
                        border-radius: 50%;
                        border-top-color: #1E90FF;
                        animation: spin 1s ease-in-out infinite;
                        margin: 1rem auto;
                    }
                    @keyframes spin {
                        to { transform: rotate(360deg); }
                    }
                    .whatsapp-connect {
                        background: rgba(0, 0, 0, 0.2);
                        border: 1px solid rgba(30, 144, 255, 0.2);
                        border-radius: 12px;
                        padding: 1.5rem;
                        margin: 1rem 0;
                        transition: all 0.3s ease;
                    }
                    .whatsapp-connect:hover {
                        transform: translateY(-2px);
                        border-color: rgba(30, 144, 255, 0.4);
                        box-shadow: 0 4px 20px rgba(30, 144, 255, 0.1);
                    }
                    .whatsapp-connect h3 {
                        color: #7EB2FF;
                        margin-bottom: 1rem;
                    }
                    .connection-status {
                        margin: 1rem 0;
                    }
                    .status {
                        font-weight: bold;
                    }
                    .status.connected {
                        color: #4CAF50;
                    }
                    .status.disconnected {
                        color: #999;
                    }
                    .verification-code-container {
                        margin: 1.5rem 0;
                        text-align: center;
                    }
                    .verification-code {
                        font-family: monospace;
                        font-size: 2.5rem;
                        font-weight: bold;
                        letter-spacing: 4px;
                        color: #1E90FF;
                        background: rgba(30, 144, 255, 0.1);
                        padding: 1rem 2rem;
                        margin: 1rem auto;
                        border-radius: 8px;
                        display: inline-block;
                        border: 2px solid rgba(30, 144, 255, 0.2);
                    }
                    .code-prompt {
                        color: #DDD;
                        font-size: 1.1rem;
                        margin-bottom: 0.5rem;
                    }
                    .instruction-note {
                        color: #BBB;
                        font-size: 0.95rem;
                        margin: 1.5rem 0 1rem;
                        font-style: italic;
                    }
                    .instruction-list {
                        color: #DDD;
                        font-size: 1rem;
                        padding-left: 1.5rem;
                        margin: 0 auto;
                        max-width: 400px;
                        text-align: left;
                    }
                    .instruction-list li {
                        margin-bottom: 0.75rem;
                        line-height: 1.4;
                    }
                    .connect-button, .disconnect-button {
                        background: linear-gradient(45deg, #1E90FF, #4169E1);
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
                        transform: translateY(-2px);
                        box-shadow: 0 4px 12px rgba(30, 144, 255, 0.3);
                    }
                    .error-message {
                        color: #FF4B4B;
                        background: rgba(255, 75, 75, 0.1);
                        border: 1px solid rgba(255, 75, 75, 0.2);
                        border-radius: 8px;
                        padding: 1rem;
                        margin-top: 1rem;
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
                        border: 1px solid rgba(30, 144, 255, 0.2);
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
                    .sync-spinner {
                        display: inline-block;
                        width: 24px;
                        height: 24px;
                        border: 2px solid rgba(30, 144, 255, 0.1);
                        border-radius: 50%;
                        border-top-color: #1E90FF;
                        animation: spin 1s ease-in-out infinite;
                        margin-right: 10px;
                        box-sizing: border-box;
                    }
                    .sync-indicator {
                        display: flex;
                        align-items: center;
                        background: rgba(30, 144, 255, 0.1);
                        border-radius: 8px;
                        padding: 1rem;
                        margin-bottom: 1rem;
                        color: #1E90FF;
                    }
                    .sync-indicator p {
                        margin: 0;
                        font-size: 0.9rem;
                    }
                    .info-button {
                        background: none;
                        border: none;
                        color: #1E90FF;
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
                        background: rgba(30, 144, 255, 0.1);
                        transform: scale(1.1);
                    }
                    #whatsapp-info {
                        max-height: 400px;
                        overflow-y: auto;
                        scrollbar-width: thin;
                        scrollbar-color: rgba(30, 144, 255, 0.5) rgba(30, 144, 255, 0.1);
                    }
                    #whatsapp-info::-webkit-scrollbar {
                        width: 8px;
                    }
                    #whatsapp-info::-webkit-scrollbar-track {
                        background: rgba(30, 144, 255, 0.1);
                        border-radius: 4px;
                    }
                    #whatsapp-info::-webkit-scrollbar-thumb {
                        background: rgba(30, 144, 255, 0.5);
                        border-radius: 4px;
                    }
                    #whatsapp-info::-webkit-scrollbar-thumb:hover {
                        background: rgba(30, 144, 255, 0.7);
                    }
                    .security-notice {
                        background: rgba(30, 144, 255, 0.1);
                        padding: 1.2rem;
                        border-radius: 8px;
                        border: 1px solid rgba(30, 144, 255, 0.2);
                    }
                    .security-notice p {
                        margin: 0 0 1rem 0;
                        color: #CCC;
                    }
                    .security-notice p:last-child {
                        margin-bottom: 0;
                    }
                    .security-recommendation {
                        font-style: italic;
                        color: #BBB !important;
                        margin-top: 1rem !important;
                        font-size: 0.9rem;
                        padding-top: 1rem;
                        border-top: 1px solid rgba(30, 144, 255, 0.1);
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
                    .reset-button {
                        background: linear-gradient(45deg, #FF8C00, #FFA500);
                        color: white;
                        border: none;
                        padding: 0.8rem 1.5rem;
                        border-radius: 8px;
                        cursor: pointer;
                        transition: all 0.3s ease;
                        margin-bottom: 1rem;
                        width: 100%;
                        font-weight: bold;
                    }
                    .reset-button:hover {
                        transform: translateY(-2px);
                        box-shadow: 0 4px 12px rgba(255, 140, 0, 0.3);
                        background: linear-gradient(45deg, #FFA500, #FFB700);
                    }
                    @keyframes spin {
                        to { transform: rotate(360deg); }
                    }
                    .service-description {
                        color: #DDD;
                    }
                "#}
            </style>
        </div>
    }
}
