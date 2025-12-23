use yew::prelude::*;
use serde::Deserialize;
use crate::utils::api::Api;
use wasm_bindgen_futures::spawn_local;
use web_sys::js_sys;
use gloo_timers::future::TimeoutFuture;

// Auth type enum
#[derive(Clone, PartialEq)]
pub enum AuthType {
    QrCode,      // WhatsApp, Signal - displays image
    LoginLink,   // Telegram - displays clickable link
}

// Bridge configuration - only truly different values
#[derive(Clone, PartialEq)]
pub struct BridgeConfig {
    pub name: &'static str,
    pub id: &'static str,
    pub logo_url: &'static str,
    pub auth_type: AuthType,
    pub instructions: &'static [&'static str],
    pub info_features: &'static [&'static str],
}

// SHARED CONSTANTS - same for all bridges
const POLL_INTERVAL_MS: i32 = 5000;        // 5 seconds
const POLL_DURATION_MS: i32 = 300000;      // 5 minutes
const SYNC_INDICATOR_DURATION_MS: f64 = 300000.0;  // 5 minutes

// Bridge configs
pub const WHATSAPP_CONFIG: BridgeConfig = BridgeConfig {
    name: "WhatsApp",
    id: "whatsapp",
    logo_url: "https://upload.wikimedia.org/wikipedia/commons/6/6b/WhatsApp.svg",
    auth_type: AuthType::QrCode,
    instructions: &[
        "Open WhatsApp on your phone",
        "Go to Settings > Linked Devices",
        "Tap 'Link a Device' and scan this QR code",
    ],
    info_features: &[
        "Fetch WhatsApp Messages: Get recent WhatsApp messages from a specific time period",
        "Fetch Chat Messages: Get messages from a specific WhatsApp chat or contact",
        "Search Contacts: Search for WhatsApp contacts or chat rooms by name",
        "Send Message: Give platform, message content and recipient name and lightfriend will send the message. Message will only be sent 60 seconds later so if you or assistant made a mistake just type 'C' with sms or say 'cancel the message' with voice calls to discard the sent event.",
    ],
};

pub const SIGNAL_CONFIG: BridgeConfig = BridgeConfig {
    name: "Signal",
    id: "signal",
    logo_url: "https://upload.wikimedia.org/wikipedia/commons/8/8d/Signal-Logo.svg",
    auth_type: AuthType::QrCode,
    instructions: &[
        "Open Signal on your phone",
        "Go to Settings > Linked Devices",
        "Tap '+' and scan this QR code",
    ],
    info_features: &[
        "Fetch Signal Messages: Get recent Signal messages from a specific time period",
        "Fetch Chat Messages: Get messages from a specific Signal chat or contact",
        "Search Contacts: Search for Signal contacts or chat rooms by name",
        "Send Message: Give platform, message content and recipient name and lightfriend will send the message. Message will only be sent 60 seconds later so if you or assistant made a mistake just type 'C' with sms or say 'cancel the message' with voice calls to discard the sent event.",
    ],
};

pub const TELEGRAM_CONFIG: BridgeConfig = BridgeConfig {
    name: "Telegram",
    id: "telegram",
    logo_url: "https://upload.wikimedia.org/wikipedia/commons/8/82/Telegram_logo.svg",
    auth_type: AuthType::LoginLink,
    instructions: &[
        "Click the button above",
        "Log in to your Telegram account",
        "Authorize Lightfriend",
    ],
    info_features: &[
        "Fetch Telegram Messages: Get recent Telegram messages from a specific time period",
        "Fetch Chat Messages: Get messages from a specific Telegram chat or contact",
        "Search Contacts: Search for Telegram contacts or chat rooms by name",
        "Send Message: Give platform, message content and recipient name and lightfriend will send the message. Message will only be sent 60 seconds later so if you or assistant made a mistake just type 'C' with sms or say 'cancel the message' with voice calls to discard the sent event.",
    ],
};

// Generic response - use serde alias to handle both field names
#[derive(Deserialize)]
struct ConnectionResponse {
    #[serde(alias = "qr_code_url", alias = "login_url")]
    auth_data: String,
}

#[derive(Deserialize, Clone, Debug)]
struct BridgeStatus {
    connected: bool,
    #[allow(dead_code)]
    status: String,
    created_at: i32,
}

#[derive(Properties, PartialEq)]
pub struct BridgeConnectProps {
    pub user_id: i32,
    pub sub_tier: Option<String>,
    pub discount: bool,
    pub config: BridgeConfig,
}

#[function_component(BridgeConnect)]
pub fn bridge_connect(props: &BridgeConnectProps) -> Html {
    let config = props.config.clone();
    let bridge_id = config.id;
    let bridge_name = config.name;

    // State - identical for all bridges
    let connection_status = use_state(|| None::<BridgeStatus>);
    let auth_data = use_state(|| None::<String>);
    let error = use_state(|| None::<String>);
    let success_message = use_state(|| None::<String>);
    let is_connecting = use_state(|| false);
    let was_connecting = use_state(|| false);
    let show_disconnect_modal = use_state(|| false);

    // Function to fetch status
    let fetch_status = {
        let connection_status = connection_status.clone();
        let error = error.clone();
        let success_message = success_message.clone();
        let was_connecting = was_connecting.clone();
        let is_connecting = is_connecting.clone();
        let auth_data = auth_data.clone();
        let bridge_id = bridge_id.to_string();
        let bridge_name = bridge_name.to_string();
        Callback::from(move |_| {
            let connection_status = connection_status.clone();
            let error = error.clone();
            let success_message = success_message.clone();
            let was_connecting = was_connecting.clone();
            let is_connecting = is_connecting.clone();
            let auth_data = auth_data.clone();
            let bridge_id = bridge_id.clone();
            let bridge_name = bridge_name.clone();
            spawn_local(async move {
                let url = format!("/api/auth/{}/status", bridge_id);
                match Api::get(&url).send().await {
                    Ok(response) => {
                        match response.json::<BridgeStatus>().await {
                            Ok(status) => {
                                // Check if we just connected (was connecting and now connected)
                                if *was_connecting && status.connected {
                                    was_connecting.set(false);
                                    is_connecting.set(false);
                                    auth_data.set(None);
                                    success_message.set(Some(format!("{} connected successfully!", bridge_name)));
                                    // Auto-hide success message after 3 seconds
                                    let success_message_clone = success_message.clone();
                                    spawn_local(async move {
                                        TimeoutFuture::new(3_000).await;
                                        success_message_clone.set(None);
                                    });
                                }
                                // Reset connecting state if disconnected
                                if !status.connected {
                                    is_connecting.set(false);
                                    was_connecting.set(false);
                                    auth_data.set(None);
                                }
                                connection_status.set(Some(status));
                                error.set(None);
                            }
                            Err(_) => {
                                error.set(Some(format!("Failed to parse {} status", bridge_name)));
                            }
                        }
                    }
                    Err(_) => {
                        error.set(Some(format!("Failed to fetch {} status", bridge_name)));
                    }
                }
            });
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

    // Function to start connection
    let start_connection = {
        let is_connecting = is_connecting.clone();
        let was_connecting = was_connecting.clone();
        let auth_data = auth_data.clone();
        let error = error.clone();
        let fetch_status = fetch_status.clone();
        let bridge_id = bridge_id.to_string();
        let bridge_name = bridge_name.to_string();
        Callback::from(move |_| {
            let is_connecting = is_connecting.clone();
            let was_connecting = was_connecting.clone();
            let auth_data = auth_data.clone();
            let error = error.clone();
            let fetch_status = fetch_status.clone();
            let bridge_id = bridge_id.clone();
            let bridge_name = bridge_name.clone();
            is_connecting.set(true);
            was_connecting.set(true);
            spawn_local(async move {
                let url = format!("/api/auth/{}/connect", bridge_id);
                match Api::get(&url).send().await {
                    Ok(response) => {
                        match response.json::<ConnectionResponse>().await {
                            Ok(connection_response) => {
                                auth_data.set(Some(connection_response.auth_data));
                                error.set(None);
                                // Start polling for status
                                let start_time = js_sys::Date::now();
                                // Create a recursive polling function
                                fn create_poll_fn(
                                    start_time: f64,
                                    poll_duration: i32,
                                    poll_interval: i32,
                                    is_connecting: UseStateHandle<bool>,
                                    auth_data: UseStateHandle<Option<String>>,
                                    error: UseStateHandle<Option<String>>,
                                    fetch_status: Callback<()>,
                                ) -> Box<dyn Fn()> {
                                    Box::new(move || {
                                        if js_sys::Date::now() - start_time > poll_duration as f64 {
                                            is_connecting.set(false);
                                            auth_data.set(None);
                                            error.set(Some("Connection attempt timed out".to_string()));
                                            return;
                                        }
                                        fetch_status.emit(());
                                        // Clone all necessary values for the next iteration
                                        let is_connecting = is_connecting.clone();
                                        let auth_data = auth_data.clone();
                                        let error = error.clone();
                                        let fetch_status = fetch_status.clone();
                                        // Schedule next poll
                                        let poll_fn = create_poll_fn(
                                            start_time,
                                            poll_duration,
                                            poll_interval,
                                            is_connecting,
                                            auth_data,
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
                                    POLL_DURATION_MS,
                                    POLL_INTERVAL_MS,
                                    is_connecting.clone(),
                                    auth_data.clone(),
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
                        error.set(Some(format!("Failed to start {} connection", bridge_name)));
                    }
                }
            });
        })
    };

    // Function to disconnect - instant response, cleanup in background
    let disconnect = {
        let connection_status = connection_status.clone();
        let error = error.clone();
        let show_disconnect_modal = show_disconnect_modal.clone();
        let bridge_id = bridge_id.to_string();
        let bridge_name = bridge_name.to_string();
        Callback::from(move |_| {
            let connection_status = connection_status.clone();
            let error = error.clone();
            let show_disconnect_modal = show_disconnect_modal.clone();
            let bridge_id = bridge_id.clone();
            let bridge_name = bridge_name.clone();
            // Immediately update UI - don't wait for backend
            connection_status.set(Some(BridgeStatus {
                connected: false,
                status: "not_connected".to_string(),
                created_at: (js_sys::Date::now() / 1000.0) as i32,
            }));
            show_disconnect_modal.set(false);
            // Fire and forget - backend cleanup happens in background
            spawn_local(async move {
                let url = format!("/api/auth/{}/disconnect", bridge_id);
                if let Err(_) = Api::delete(&url).send().await {
                    // Log error but don't show to user - UI already updated
                    web_sys::console::error_1(&format!("Background {} disconnect failed", bridge_name).into());
                }
            });
            error.set(None);
        })
    };

    // Generate info section id
    let info_id = format!("{}-info", bridge_id);
    let info_id_clone = info_id.clone();

    html! {
        <div class="bridge-connect">
            if let Some(msg) = (*success_message).as_ref() {
                <div class="success-banner">
                    {msg}
                </div>
            }
            <div class="service-header">
                <div class="service-name">
                    <img src={config.logo_url} alt={bridge_name} width="24" height="24"/>
                    {bridge_name}
                </div>
                if let Some(status) = (*connection_status).clone() {
                    if status.connected {
                        <span class="service-status">{"Connected ✓"}</span>
                    }
                }
                <button class="info-button" onclick={
                    let info_id = info_id.clone();
                    Callback::from(move |_| {
                        if let Some(element) = web_sys::window()
                            .and_then(|w| w.document())
                            .and_then(|d| d.get_element_by_id(&info_id))
                        {
                            let display = element.get_attribute("style")
                                .unwrap_or_else(|| "display: none".to_string());
                            if display.contains("none") {
                                let _ = element.set_attribute("style", "display: block");
                            } else {
                                let _ = element.set_attribute("style", "display: none");
                            }
                        }
                    })
                }>
                    {"ⓘ"}
                </button>
            </div>
            <div id={info_id_clone} class="info-section" style="display: none">
                <h4>{"How It Works"}</h4>
                <div class="info-subsection">
                    <h5>{"SMS and Voice Call Tools"}</h5>
                    <ul>
                        { for config.info_features.iter().map(|feature| html! { <li>{feature}</li> }) }
                    </ul>
                </div>
                <div class="info-subsection security-notice">
                    <h5>{"Security & Privacy"}</h5>
                    <p>{format!("Your security is our priority. We use the same trusted Matrix server and {} bridge technology as Beeper Cloud, with robust encryption and strict access controls to protect your data at every step. When you disconnect your {} account, all your {} data will be automatically deleted from our servers.", bridge_name, bridge_name, bridge_name)}</p>
                    <p class="security-recommendation">{format!("Note: While we maintain high security standards, SMS and voice calls use standard cellular networks. For maximum privacy, use {} directly for sensitive communications.", bridge_name)}</p>
                </div>
            </div>

            if let Some(status) = (*connection_status).clone() {
                <div class="connection-status">
                    if status.connected {
                        <>
                            {
                                // Show sync indicator for 5 minutes after connection
                                if js_sys::Date::now() - (status.created_at as f64 * 1000.0) <= SYNC_INDICATOR_DURATION_MS {
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
                                    {format!("Send and receive {} messages through SMS or voice calls.", bridge_name)}
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
                                            <p>{format!("Are you sure you want to disconnect {}? This will:", bridge_name)}</p>
                                            <ul>
                                                <li>{format!("Stop all {} message forwarding", bridge_name)}</li>
                                                <li>{format!("Delete all your {} data from our servers", bridge_name)}</li>
                                                <li>{format!("Require reconnection to use {} features again", bridge_name)}</li>
                                            </ul>
                                            <div class="modal-buttons">
                                                <button onclick={
                                                    let show_disconnect_modal = show_disconnect_modal.clone();
                                                    Callback::from(move |_| show_disconnect_modal.set(false))
                                                } class="cancel-button">
                                                    {"Cancel"}
                                                </button>
                                                <button onclick={
                                                    let disconnect = disconnect.clone();
                                                    Callback::from(move |_| {
                                                        disconnect.emit(());
                                                    })
                                                } class="confirm-disconnect-button">
                                                    {"Yes, Disconnect"}
                                                </button>
                                            </div>
                                        </div>
                                    </div>
                                }
                            </div>
                        </>
                    } else {
                        if *is_connecting {
                            if let Some(data) = (*auth_data).clone() {
                                <div class="login-link-container">
                                    {
                                        match config.auth_type {
                                            AuthType::QrCode => html! {
                                                <>
                                                    <p class="connect-instruction">{format!("Scan the QR code below with your {} app:", bridge_name)}</p>
                                                    <img src={data} alt={format!("{} QR Code", bridge_name)} class="qr-code" />
                                                </>
                                            },
                                            AuthType::LoginLink => html! {
                                                <>
                                                    <p class="connect-instruction">{format!("Click the button below to connect your {} account:", bridge_name)}</p>
                                                    <a href={data} target="_blank" class="telegram-login-button">
                                                        {format!("Connect {}", bridge_name)}
                                                    </a>
                                                </>
                                            },
                                        }
                                    }
                                    { for config.instructions.iter().enumerate().map(|(i, instruction)| {
                                        html! { <p class="instruction">{format!("{}. {}", i + 1, instruction)}</p> }
                                    })}
                                </div>
                            } else {
                                <div class="loading-container">
                                    <p class="connect-instruction">
                                        {
                                            match config.auth_type {
                                                AuthType::QrCode => "Generating QR code...",
                                                AuthType::LoginLink => "Generating login link...",
                                            }
                                        }
                                    </p>
                                    <div class="loading-spinner"></div>
                                </div>
                            }
                        } else {
                            if props.sub_tier.as_deref() == Some("tier 2") || props.discount {
                                <p class="service-description">
                                    {format!("Send and receive {} messages through SMS or voice calls.", bridge_name)}
                                </p>
                                <button onclick={start_connection} class="connect-button">
                                    {format!("Connect {}", bridge_name)}
                                </button>
                            } else {
                                <div class="upgrade-prompt">
                                    <div class="upgrade-content">
                                        <h3>{format!("Upgrade to Enable {} Integration", bridge_name)}</h3>
                                        <a href="/pricing" class="upgrade-button">
                                            {"View Pricing Plans"}
                                        </a>
                                    </div>
                                </div>
                            }
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
                    .bridge-connect {
                        background: rgba(0, 0, 0, 0.2);
                        border: 1px solid rgba(30, 144, 255, 0.2);
                        border-radius: 12px;
                        padding: 1.5rem;
                        margin: 1rem 0;
                        transition: all 0.3s ease;
                    }
                    .bridge-connect:hover {
                        transform: translateY(-2px);
                        border-color: rgba(30, 144, 255, 0.4);
                        box-shadow: 0 4px 20px rgba(30, 144, 255, 0.1);
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
                        display: flex;
                        align-items: center;
                        gap: 0.5rem;
                        font-weight: bold;
                        color: #fff;
                    }
                    .service-status {
                        white-space: nowrap;
                        color: #4CAF50;
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
                    .info-section {
                        max-height: 400px;
                        overflow-y: auto;
                        scrollbar-width: thin;
                        scrollbar-color: rgba(30, 144, 255, 0.5) rgba(30, 144, 255, 0.1);
                        margin-top: 1rem;
                        padding: 1rem;
                        background: rgba(30, 144, 255, 0.05);
                        border-radius: 8px;
                    }
                    .info-section h4 {
                        color: #7EB2FF;
                        margin-bottom: 1rem;
                    }
                    .info-section h5 {
                        color: #1E90FF;
                        margin-bottom: 0.5rem;
                    }
                    .info-section ul {
                        color: #CCC;
                        padding-left: 1.5rem;
                    }
                    .info-section li {
                        margin-bottom: 0.5rem;
                    }
                    .security-notice {
                        background: rgba(30, 144, 255, 0.1);
                        padding: 1.2rem;
                        border-radius: 8px;
                        border: 1px solid rgba(30, 144, 255, 0.2);
                        margin-top: 1rem;
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
                    .connection-status {
                        margin: 1rem 0;
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
                        flex-shrink: 0;
                    }
                    .button-group {
                        display: flex;
                        flex-direction: column;
                        gap: 1rem;
                        margin-bottom: 1rem;
                    }
                    .service-description {
                        color: #DDD;
                        margin: 0;
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
                    .connect-button:hover {
                        transform: translateY(-2px);
                        box-shadow: 0 4px 12px rgba(30, 144, 255, 0.3);
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
                    .login-link-container {
                        margin: 1.5rem 0;
                        text-align: center;
                    }
                    .qr-code {
                        width: 200px;
                        height: 200px;
                        margin: 1rem auto;
                        display: block;
                    }
                    .telegram-login-button {
                        display: inline-block;
                        background: linear-gradient(45deg, #1E90FF, #4169E1);
                        color: white;
                        text-decoration: none;
                        padding: 1rem 2rem;
                        border-radius: 8px;
                        font-weight: bold;
                        transition: all 0.3s ease;
                        margin: 1rem 0;
                    }
                    .telegram-login-button:hover {
                        transform: translateY(-2px);
                        box-shadow: 0 4px 12px rgba(30, 144, 255, 0.3);
                    }
                    .connect-instruction {
                        color: #fff;
                        margin-bottom: 1rem;
                        font-size: 1rem;
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
                        border: 4px solid rgba(30, 144, 255, 0.1);
                        border-radius: 50%;
                        border-top-color: #1E90FF;
                        animation: spin 1s ease-in-out infinite;
                        margin: 1rem auto;
                    }
                    .upgrade-prompt {
                        background: rgba(30, 144, 255, 0.05);
                        border: 1px solid rgba(30, 144, 255, 0.1);
                        border-radius: 12px;
                        padding: 2rem;
                        text-align: center;
                        margin: 1rem 0;
                    }
                    .upgrade-content {
                        max-width: 400px;
                        margin: 0 auto;
                    }
                    .upgrade-content h3 {
                        color: #1E90FF;
                        margin-bottom: 1rem;
                        font-size: 1.5rem;
                    }
                    .upgrade-button {
                        display: inline-block;
                        background: linear-gradient(45deg, #1E90FF, #4169E1);
                        color: white;
                        text-decoration: none;
                        padding: 1rem 2rem;
                        border-radius: 8px;
                        font-weight: bold;
                        transition: all 0.3s ease;
                        margin-top: 1rem;
                    }
                    .upgrade-button:hover {
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
                    .success-banner {
                        color: #4CAF50;
                        background: rgba(76, 175, 80, 0.1);
                        border: 1px solid rgba(76, 175, 80, 0.3);
                        border-radius: 8px;
                        padding: 1rem;
                        margin-bottom: 1rem;
                        text-align: center;
                        font-weight: 500;
                    }
                    @keyframes spin {
                        to { transform: rotate(360deg); }
                    }
                "#}
            </style>
        </div>
    }
}
