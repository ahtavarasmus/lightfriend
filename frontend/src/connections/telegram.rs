use yew::prelude::*;
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use web_sys::{window, Event};
use wasm_bindgen::JsCast;
use crate::config;
use wasm_bindgen_futures::spawn_local;
use web_sys::js_sys;

#[derive(Deserialize, Clone, Debug)]
struct TelegramStatus {
    connected: bool,
    status: String,
    created_at: i32,
}

#[derive(Deserialize)]
struct TelegramConnectionResponse {
    login_url: String,
}

#[derive(Properties, PartialEq)]
pub struct TelegramProps {
    pub user_id: i32,
    pub sub_tier: Option<String>,
    pub discount: bool,
}

#[function_component(TelegramConnect)]
pub fn telegram_connect(props: &TelegramProps) -> Html {
    let connection_status = use_state(|| None::<TelegramStatus>);
    let login_link = use_state(|| None::<String>);
    let error = use_state(|| None::<String>);
    let is_connecting = use_state(|| false);

    // Function to fetch Telegram status
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
                    match Request::get(&format!("{}/api/auth/telegram/status", config::get_backend_url()))
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await
                    {
                        Ok(response) => {
                            match response.json::<TelegramStatus>().await {
                                Ok(status) => {
                                    connection_status.set(Some(status));
                                    error.set(None);
                                }
                                Err(_) => {
                                    error.set(Some("Failed to parse Telegram status".to_string()));
                                }
                            }
                        }
                        Err(_) => {
                            error.set(Some("Failed to fetch Telegram status".to_string()));
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

    // Function to start Telegram connection
    let start_connection = {
        let is_connecting = is_connecting.clone();
        let login_link = login_link.clone();
        let error = error.clone();
        let fetch_status = fetch_status.clone();

        Callback::from(move |_| {
            let is_connecting = is_connecting.clone();
            let login_link = login_link.clone();
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
                    match Request::get(&format!("{}/api/auth/telegram/connect", config::get_backend_url()))
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await
                    {
                        Ok(response) => {
                            match response.json::<TelegramConnectionResponse>().await {
                                Ok(connection_response) => {
                                    login_link.set(Some(connection_response.login_url));
                                    error.set(None);

                                    // Start polling for status
                                    let poll_interval = 5000; // 5 seconds
                                    let poll_duration = 300000; // 5 minutes
                                    let start_time = js_sys::Date::now();

                                    fn create_poll_fn(
                                        start_time: f64,
                                        poll_duration: i32,
                                        poll_interval: i32,
                                        is_connecting: UseStateHandle<bool>,
                                        login_link: UseStateHandle<Option<String>>,
                                        error: UseStateHandle<Option<String>>,
                                        fetch_status: Callback<()>,
                                    ) -> Box<dyn Fn()> {
                                        Box::new(move || {
                                            if js_sys::Date::now() - start_time > poll_duration as f64 {
                                                is_connecting.set(false);
                                                login_link.set(None);
                                                error.set(Some("Connection attempt timed out".to_string()));
                                                return;
                                            }

                                            fetch_status.emit(());

                                            let is_connecting = is_connecting.clone();
                                            let login_link = login_link.clone();
                                            let error = error.clone();
                                            let fetch_status = fetch_status.clone();

                                            let poll_fn = create_poll_fn(
                                                start_time,
                                                poll_duration,
                                                poll_interval,
                                                is_connecting,
                                                login_link,
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
                                        login_link.clone(),
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
                            error.set(Some("Failed to start Telegram connection".to_string()));
                        }
                    }
                });
            }
        })
    };

    // Function to disconnect Telegram
    let disconnect = {
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
                    match Request::delete(&format!("{}/api/auth/telegram/disconnect", config::get_backend_url()))
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await
                    {
                        Ok(_) => {
                            connection_status.set(Some(TelegramStatus {
                                connected: false,
                                status: "not_connected".to_string(),
                                created_at: (js_sys::Date::now() as i32),
                            }));
                            error.set(None);
                        }
                        Err(_) => {
                            error.set(Some("Failed to disconnect Telegram".to_string()));
                        }
                    }
                });
            }
        })
    };

    if props.user_id != 1 {
        return html! {
            <div class="telegram-connect">
                <div class="service-header">
                    <div class="service-name">
                        <img src="https://upload.wikimedia.org/wikipedia/commons/8/82/Telegram_logo.svg" style="width: 24px; height: 24px;" alt="Telegram"/>
                        {"Telegram"}
                    </div>
                </div>
                <div class="restricted-access">
                    <p>{"This feature is currently in beta testing and not yet available for general use."}</p>
                </div>
                <style>
                    {".restricted-access { 
                        padding: 1rem;
                        color: #999;
                        text-align: center;
                        margin-top: 1rem;
                        background: rgba(0, 0, 0, 0.2);
                        border-radius: 8px;
                    }"}
                </style>
            </div>
        };
    }

    html! {
        <div class="telegram-connect">
            <div class="service-header">
                <div class="service-name">
                    <img src="https://upload.wikimedia.org/wikipedia/commons/8/82/Telegram_logo.svg" alt="Telegram"/>
                    {"Telegram"}
                </div>
                if let Some(status) = (*connection_status).clone() {
                    if status.connected {
                        <span class="service-status">{"Connected ✓"}</span>
                    }
                }
                <button class="info-button" onclick={Callback::from(|_| {
                    if let Some(element) = web_sys::window()
                        .and_then(|w| w.document())
                        .and_then(|d| d.get_element_by_id("telegram-info"))
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

            <div id="telegram-info" class="info-section" style="display: none">
                <h4>{"How It Works"}</h4>

                <div class="info-subsection">
                    <h5>{"SMS and Voice Call Tools"}</h5>
                    <ul>
                        <li>{"Fetch Telegram Messages: Get recent Telegram messages from a specific time period"}</li>
                        <li>{"Fetch Chat Messages: Get messages from a specific Telegram chat or contact"}</li>
                        <li>{"Search Contacts: Search for Telegram contacts or chat rooms by name"}</li>
                        <li>{"Send Message: Send a Telegram message to a specific recipient (will ask for confirmation before sending)"}</li>
                    </ul>
                </div>

                <div class="info-subsection security-notice">
                    <h5>{"Security & Privacy"}</h5>
                    <p>{"Your security is our priority. Here's how we protect your messages:"}</p>
                    <ul>
                        <li>{"Your Telegram messages are end-to-end encrypted between Telegram and our Matrix server. We use the same trusted Matrix server and Telegram bridge technology as Beeper, with robust encryption and strict access controls to protect your data at every step."}</li>
                    </ul>
                    <p class="security-recommendation">{"Note: While we maintain high security standards, SMS and voice calls use standard cellular networks. For maximum privacy, use Telegram directly for sensitive communications."}</p>
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
                                    {"Send and receive Telegram messages through SMS or voice calls."}
                                </p>
                                <button onclick={disconnect} class="disconnect-button">
                                    {"Disconnect"}
                                </button>
                            </div>
                        </>
                    } else {
                        if *is_connecting {
                            if let Some(link) = (*login_link).clone() {
                                <div class="login-link-container">
                                    <p>{"Click the button below to connect your Telegram account:"}</p>
                                    <a href={link} target="_blank" class="telegram-login-button">
                                        {"Connect Telegram"}
                                    </a>
                                    <p class="instruction">{"1. Click the button above"}</p>
                                    <p class="instruction">{"2. Log in to your Telegram account"}</p>
                                    <p class="instruction">{"3. Authorize Lightfriend to access your messages"}</p>
                                </div>
                            } else {
                                <div class="loading-container">
                                    <p>{"Generating login link..."}</p>
                                    <div class="loading-spinner"></div>
                                </div>
                            }
                        } else {
                            <p class="service-description">
                                {"Send and receive Telegram messages through SMS or voice calls."}
                            </p>
                            <button onclick={start_connection} class="connect-button">
                                {"Start Auth"}
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
                    .telegram-connect {
                        background: rgba(0, 0, 0, 0.2);
                        border: 1px solid rgba(0, 136, 204, 0.2);
                        border-radius: 12px;
                        width: 100%;
                        padding: 1.5rem;
                        margin: 1rem 0;
                        transition: all 0.3s ease;
                    }

                    .telegram-connect:hover {
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

                    .login-link-container {
                        margin: 1.5rem 0;
                        text-align: center;
                    }

                    .telegram-login-button {
                        display: inline-block;
                        background: #0088cc;
                        color: white;
                        text-decoration: none;
                        padding: 1rem 2rem;
                        border-radius: 8px;
                        font-weight: bold;
                        margin: 1rem 0;
                        transition: all 0.3s ease;
                    }

                    .telegram-login-button:hover {
                        background: #0077b3;
                        transform: translateY(-2px);
                        box-shadow: 0 4px 12px rgba(0, 136, 204, 0.3);
                    }

                    .instruction {
                        color: #999;
                        margin-top: 0.5rem;
                        font-size: 0.9rem;
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
                "#}
            </style>
        </div>
    }
}

