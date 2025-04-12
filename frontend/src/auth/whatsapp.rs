use yew::prelude::*;
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use web_sys::{window, Event};
use wasm_bindgen::JsCast;
use crate::config;
use wasm_bindgen_futures::spawn_local;
use web_sys::js_sys;

#[derive(Deserialize, Clone)]
struct WhatsappStatus {
    connected: bool,
    status: String,
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
                    match Request::delete(&format!("{}/api/auth/whatsapp/disconnect", config::get_backend_url()))
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await
                    {
                        Ok(_) => {
                            connection_status.set(Some(WhatsappStatus {
                                connected: false,
                                status: "not_connected".to_string(),
                            }));
                            error.set(None);
                        }
                        Err(_) => {
                            error.set(Some("Failed to disconnect WhatsApp".to_string()));
                        }
                    }
                });
            }
        })
    };

    html! {
        <div class="whatsapp-connect">
            <div class="service-header">
                <div class="service-name">
                    <img src="https://upload.wikimedia.org/wikipedia/commons/6/6b/WhatsApp.svg" alt="WhatsApp"/>
                    {"WhatsApp"}
                </div>
                if let Some(status) = (*connection_status).clone() {
                    if status.connected {
                        <span class="service-status">{"Connected ✓"}</span>
                    }
                }
            </div>
            
            if let Some(status) = (*connection_status).clone() {
                <div class="connection-status">
                    if status.connected {
                        <button onclick={disconnect} class="disconnect-button">
                            {"Disconnect WhatsApp"}
                        </button>
                    } else {
                        if *is_connecting {
                            if let Some(pairing_code) = (*qr_code).clone() {
                                <div class="verification-code-container">
                                    <p>{"Enter this code in WhatsApp to connect:"}</p>
                                    <div class="verification-code">
                                        {pairing_code}
                                    </div>
                                    <p class="instruction">{"1. Open WhatsApp on your phone"}</p>
                                    <p class="instruction">{"2. Go to Settings > Linked Devices"}</p>
                                    <p class="instruction">{"3. Tap 'Link a Device'"}</p>
                                    <p class="instruction">{"4. When prompted, enter this code"}</p>
                                </div>
                            } else {
                                <div class="loading-container">
                                    <p>{"Generating connection code..."}</p>
                                    <div class="loading-spinner"></div>
                                </div>
                            }
                        } else {
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
                        border: 1px solid rgba(30, 144, 255, 0.1);
                        border-radius: 12px;
                        padding: 1.5rem;
                        margin: 1rem 0;
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

                    .instruction {
                        color: #999;
                        margin-top: 1rem;
                        font-size: 0.9rem;
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
                        background: linear-gradient(45deg, #FF4B4B, #FF6B6B);
                    }

                    .connect-button:hover, .disconnect-button:hover {
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
                "#}
            </style>
        </div>
    }
}

