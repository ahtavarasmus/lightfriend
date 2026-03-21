use yew::prelude::*;
use web_sys::MouseEvent;
use wasm_bindgen_futures::spawn_local;
use wasm_bindgen::JsValue;
use web_sys::UrlSearchParams;
use crate::utils::api::Api;
use crate::connections::whatsapp::WhatsappConnect;
use crate::connections::email::EmailConnect;
use crate::connections::telegram::TelegramConnect;
use crate::connections::signal::SignalConnect;
use crate::connections::tesla::TeslaConnect;
use crate::connections::youtube::YouTubeConnect;
use crate::connections::mcp::McpConnect;
use serde_json::Value;

#[derive(Properties, PartialEq)]
pub struct ConnectProps {
    pub user_id: i32,
    pub sub_tier: Option<String>,
    pub phone_number: String,
    pub estimated_monitoring_cost: f32,
}
#[derive(Clone, PartialEq)]
struct ServiceGroupState {
    expanded: bool,
    service_count: usize,
    connected_count: usize,
}
// MonitoringTab removed - Tasks, People, and Digests are now in Settings Panel
#[function_component(Connect)]
pub fn connect(props: &ConnectProps) -> Html {
    let error = use_state(|| None::<String>);
    let email_connected = use_state(|| false);
    let whatsapp_connected = use_state(|| false);
    let telegram_connected = use_state(|| false);
    let signal_connected = use_state(|| false);
    let tesla_connected = use_state(|| false);
    let youtube_connected = use_state(|| false);
    let mcp_server_count = use_state(|| 0_usize);
    let selected_app = use_state(|| None::<String>);

    {
        let email_connected = email_connected.clone();
        let whatsapp_connected = whatsapp_connected.clone();
        let telegram_connected= telegram_connected.clone();
        let signal_connected= signal_connected.clone();
        let tesla_connected = tesla_connected.clone();
        let youtube_connected = youtube_connected.clone();
        let mcp_server_count = mcp_server_count.clone();
        use_effect_with_deps(
            move |_| {
                // Auth handled by cookies - check all connection statuses
                // Email status check
                spawn_local({
                    let email_connected = email_connected.clone();
                    async move {
                        if let Ok(response) = Api::get("/api/auth/imap/status")
                            .send()
                            .await
                        {
                            if let Ok(data) = response.json::<Value>().await {
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
                    async move {
                        if let Ok(response) = Api::get("/api/auth/whatsapp/status")
                            .send()
                            .await
                        {
                            if let Ok(data) = response.json::<Value>().await {
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
                    async move {
                        if let Ok(response) = Api::get("/api/auth/telegram/status")
                            .send()
                            .await
                        {
                            if let Ok(data) = response.json::<Value>().await {
                                if let Some(connected) = data.get("connected").and_then(|v| v.as_bool()) {
                                    telegram_connected.set(connected);
                                }
                            }
                        }
                    }
                });
                // signal status check
                spawn_local({
                    let signal_connected = signal_connected.clone();
                    async move {
                        if let Ok(response) = Api::get("/api/auth/signal/status")
                            .send()
                            .await
                        {
                            if let Ok(data) = response.json::<Value>().await {
                                if let Some(connected) = data.get("connected").and_then(|v| v.as_bool()) {
                                    signal_connected.set(connected);
                                }
                            }
                        }
                    }
                });
                // tesla status check
                spawn_local({
                    let tesla_connected = tesla_connected.clone();
                    async move {
                        if let Ok(response) = Api::get("/api/auth/tesla/status")
                            .send()
                            .await
                        {
                            if let Ok(data) = response.json::<Value>().await {
                                if let Some(has_tesla) = data.get("has_tesla").and_then(|v| v.as_bool()) {
                                    tesla_connected.set(has_tesla);
                                }
                            }
                        }
                    }
                });
                // youtube status check
                spawn_local({
                    let youtube_connected = youtube_connected.clone();
                    async move {
                        if let Ok(response) = Api::get("/api/auth/youtube/status")
                            .send()
                            .await
                        {
                            if let Ok(data) = response.json::<Value>().await {
                                if let Some(connected) = data.get("connected").and_then(|v| v.as_bool()) {
                                    youtube_connected.set(connected);
                                }
                            }
                        }
                    }
                });
                // MCP servers count check
                spawn_local({
                    let mcp_server_count = mcp_server_count.clone();
                    async move {
                        if let Ok(response) = Api::get("/api/mcp/servers")
                            .send()
                            .await
                        {
                            if let Ok(data) = response.json::<Value>().await {
                                if let Some(servers) = data.as_array() {
                                    mcp_server_count.set(servers.len());
                                }
                            }
                        }
                    }
                });
                || ()
            },
            (),
        );
    }
    let _group_states = use_state(|| {
        let mut map = std::collections::HashMap::new();
        map.insert("tools", ServiceGroupState { expanded: false, service_count: 4, connected_count: 0 });
        map.insert("proactive", ServiceGroupState { expanded: false, service_count: 4, connected_count: 0 });
        map
    });
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
    let details = if let Some(app) = &*selected_app {
        match app.as_str() {
            "email" => html! { <EmailConnect user_id={props.user_id} sub_tier={props.sub_tier.clone()} /> },
            "whatsapp" => html! { <WhatsappConnect user_id={props.user_id} sub_tier={props.sub_tier.clone()} /> },
            "telegram" => html! { <TelegramConnect user_id={props.user_id} sub_tier={props.sub_tier.clone()} /> },
            "signal" => html! { <SignalConnect user_id={props.user_id} sub_tier={props.sub_tier.clone()} /> },
            "tesla" => html! { <TeslaConnect user_id={props.user_id} sub_tier={props.sub_tier.clone()} /> },
            "youtube" => html! { <YouTubeConnect user_id={props.user_id} sub_tier={props.sub_tier.clone()} /> },
            "mcp" => html! { <McpConnect user_id={props.user_id} /> },
            "perplexity" => html! { <div class="builtin-detail"><p>{"AI-powered web search for real-time information, research, and fact-checking."}</p></div> },
            "weather" => html! { <div class="builtin-detail"><p>{"Weather updates and forecasts. Uses your location from Settings > Account."}</p></div> },
            "photo" => html! { <div class="builtin-detail"><p>{"Send a photo to scan QR codes, translate text, or describe what you see."}</p></div> },
            "sms_calls" => html! { <div class="builtin-detail"><p>{"Send follow-up info via SMS while on a voice call."}</p></div> },
            _ => html! {},
        }
    } else {
        html! {}
    };
    html! {
                <div class="connect-section">
                    // Apps
                    <div class="apps-icons-row">
                        <button
                            class={classes!("app-icon", if *email_connected { "connected" } else { "" }, if selected_app.as_ref().map_or(false, |s| s == "email") { "selected" } else { "" })}
                            onclick={let selected_app = selected_app.clone(); Callback::from(move |_: MouseEvent| {
                                selected_app.set(if *selected_app == Some("email".to_string()) { None } else { Some("email".to_string()) });
                            })}
                        >
                            <img src="data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 512 512'%3E%3Cpath fill='%234285f4' d='M48 64C21.5 64 0 85.5 0 112c0 15.1 7.1 29.3 19.2 38.4L236.8 313.6c11.4 8.5 27 8.5 38.4 0L492.8 150.4c12.1-9.1 19.2-23.3 19.2-38.4c0-26.5-21.5-48-48-48H48zM0 176V384c0 35.3 28.7 64 64 64H448c35.3 0 64-28.7 64-64V176L294.4 339.2c-22.8 17.1-54 17.1-76.8 0L0 176z'/%3E%3C/svg%3E" alt="IMAP" width="24" height="24"/>
                        </button>
                        <button
                            class={classes!("app-icon", if *whatsapp_connected { "connected" } else { "" }, if selected_app.as_ref().map_or(false, |s| s == "whatsapp") { "selected" } else { "" })}
                            onclick={let selected_app = selected_app.clone(); Callback::from(move |_: MouseEvent| {
                                selected_app.set(if *selected_app == Some("whatsapp".to_string()) { None } else { Some("whatsapp".to_string()) });
                            })}
                        >
                            <img src="https://upload.wikimedia.org/wikipedia/commons/6/6b/WhatsApp.svg" alt="WhatsApp" width="24" height="24"/>
                        </button>
                        <button
                            class={classes!("app-icon", if *telegram_connected { "connected" } else { "" }, if selected_app.as_ref().map_or(false, |s| s == "telegram") { "selected" } else { "" })}
                            onclick={let selected_app = selected_app.clone(); Callback::from(move |_: MouseEvent| {
                                selected_app.set(if *selected_app == Some("telegram".to_string()) { None } else { Some("telegram".to_string()) });
                            })}
                        >
                            <img src="https://upload.wikimedia.org/wikipedia/commons/8/82/Telegram_logo.svg" alt="Telegram" width="24" height="24"/>
                        </button>
                        <button
                            class={classes!("app-icon", if *signal_connected { "connected" } else { "" }, if selected_app.as_ref().map_or(false, |s| s == "signal") { "selected" } else { "" })}
                            onclick={let selected_app = selected_app.clone(); Callback::from(move |_: MouseEvent| {
                                selected_app.set(if *selected_app == Some("signal".to_string()) { None } else { Some("signal".to_string()) });
                            })}
                        >
                            <img src="https://upload.wikimedia.org/wikipedia/commons/6/60/Signal-Logo-Ultramarine_%282024%29.svg" alt="Signal Logo" width="24" height="24"/>
                        </button>
                        <button
                            class={classes!("app-icon", if *tesla_connected { "connected" } else { "" }, if selected_app.as_ref().map_or(false, |s| s == "tesla") { "selected" } else { "" })}
                            onclick={let selected_app = selected_app.clone(); Callback::from(move |_: MouseEvent| {
                                selected_app.set(if *selected_app == Some("tesla".to_string()) { None } else { Some("tesla".to_string()) });
                            })}
                        >
                            <img src="https://upload.wikimedia.org/wikipedia/commons/b/bb/Tesla_T_symbol.svg" alt="Tesla" width="24" height="24"/>
                        </button>
                        <button
                            class={classes!("app-icon", if *youtube_connected { "connected" } else { "" }, if selected_app.as_ref().map_or(false, |s| s == "youtube") { "selected" } else { "" })}
                            onclick={let selected_app = selected_app.clone(); Callback::from(move |_: MouseEvent| {
                                selected_app.set(if *selected_app == Some("youtube".to_string()) { None } else { Some("youtube".to_string()) });
                            })}
                        >
                            <img src="https://upload.wikimedia.org/wikipedia/commons/0/09/YouTube_full-color_icon_%282017%29.svg" alt="YouTube" width="24" height="24"/>
                        </button>
                        <button
                            class={classes!("app-icon", "mcp-icon", if *mcp_server_count > 0 { "connected" } else { "" }, if selected_app.as_ref().map_or(false, |s| s == "mcp") { "selected" } else { "" })}
                            onclick={let selected_app = selected_app.clone(); Callback::from(move |_: MouseEvent| {
                                selected_app.set(if *selected_app == Some("mcp".to_string()) { None } else { Some("mcp".to_string()) });
                            })}
                            title="MCP Servers - Add custom tools (beta)"
                        >
                            <i class="fa-solid fa-plug"></i>
                        </button>
                    </div>
                    <div class="app-details">
                        { details }
                    </div>
                    // Built-in tools row - same style as capabilities above
                    <div class="builtin-tools-label">{"Built-in"}</div>
                    <div class="apps-icons-row builtin-row">
                        <button
                            class={classes!("app-icon", "connected", "builtin-tool", if selected_app.as_ref().map_or(false, |s| s == "perplexity") { "selected" } else { "" })}
                            title="Perplexity AI - AI-powered search and answers"
                            onclick={let selected_app = selected_app.clone(); Callback::from(move |_: MouseEvent| {
                                selected_app.set(if *selected_app == Some("perplexity".to_string()) { None } else { Some("perplexity".to_string()) });
                            })}
                        >
                            <img src="https://upload.wikimedia.org/wikipedia/commons/1/1d/Perplexity_AI_logo.svg" alt="Perplexity" width="24" height="24"/>
                        </button>
                        <button
                            class={classes!("app-icon", "connected", "builtin-tool", if selected_app.as_ref().map_or(false, |s| s == "weather") { "selected" } else { "" })}
                            title="Weather - Weather updates and forecasts"
                            onclick={let selected_app = selected_app.clone(); Callback::from(move |_: MouseEvent| {
                                selected_app.set(if *selected_app == Some("weather".to_string()) { None } else { Some("weather".to_string()) });
                            })}
                        >
                            <i class="fa-solid fa-sun"></i>
                        </button>
                        <button
                            class={classes!("app-icon", "connected", "builtin-tool", if selected_app.as_ref().map_or(false, |s| s == "photo") { "selected" } else { "" })}
                            title="Photo - QR codes and text translation"
                            onclick={let selected_app = selected_app.clone(); Callback::from(move |_: MouseEvent| {
                                selected_app.set(if *selected_app == Some("photo".to_string()) { None } else { Some("photo".to_string()) });
                            })}
                        >
                            <i class="fa-solid fa-camera"></i>
                        </button>
                        <button
                            class={classes!("app-icon", "connected", "builtin-tool", if selected_app.as_ref().map_or(false, |s| s == "sms_calls") { "selected" } else { "" })}
                            title="SMS During Calls - Send info via SMS while on voice calls"
                            onclick={let selected_app = selected_app.clone(); Callback::from(move |_: MouseEvent| {
                                selected_app.set(if *selected_app == Some("sms_calls".to_string()) { None } else { Some("sms_calls".to_string()) });
                            })}
                        >
                            <i class="fa-solid fa-sms"></i>
                        </button>
                    </div>
                    if let Some(err) = (*error).as_ref() {
                        <div class="error-message">
                            {err}
                        </div>
                    }
<style>
        {r#"
/* Built-in tools - same row style as capabilities */
.builtin-tools-label {
    color: #666;
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    padding: 0 1.5rem;
    margin: 0;
}
.apps-icons-row.builtin-row {
    padding-top: 0.5rem;
    margin-top: 0;
}
.app-icon.builtin-tool {
    color: #34D399;
    font-size: 1.5rem;
    cursor: pointer;
}
.app-icon.builtin-tool.selected {
    background: rgba(52, 211, 153, 0.25);
    box-shadow: 0 0 12px rgba(52, 211, 153, 0.5);
}
.app-icon.builtin-tool img {
    filter: brightness(0) saturate(100%) invert(68%) sepia(52%) saturate(434%) hue-rotate(106deg) brightness(96%) contrast(92%);
}
.builtin-detail {
    padding: 0.75rem 1rem;
    color: #aaa;
    font-size: 0.85rem;
    line-height: 1.5;
}
.builtin-detail p {
    margin: 0;
}
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
/* Monitoring */
.service-group:nth-child(1) .service-count {
    background: rgba(52, 211, 153, 0.1);
    color: #34D399;
}
/* Tools */
.service-group:nth-child(2) .service-count {
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
.usage-link {
    padding: 0.25rem 0.75rem;
    border-radius: 12px;
    font-size: 0.8rem;
    background: rgba(126, 178, 255, 0.15);
    color: #7EB2FF;
    text-decoration: none;
    transition: all 0.2s ease;
}
.usage-link:hover {
    background: rgba(126, 178, 255, 0.25);
    color: #9AC4FF;
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
.service-list, .monitoring-content {
    transition: all 0.3s ease-in-out;
}
.service-list.collapsed, .monitoring-content.collapsed {
    max-height: 0;
    opacity: 0;
    margin: 0;
    padding: 0;
    overflow: hidden;
}
.service-list.expanded, .monitoring-content.expanded {
    max-height: 5000px;
    opacity: 1;
    margin-top: 1.5rem;
    overflow: visible;
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
.service-group-title {
    font-size: 1.2rem;
    margin: 0;
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.5rem;
    border-radius: 8px;
    transition: all 0.3s ease;
}
/* Monitoring - Green */
.service-group:nth-child(1) .service-group-title {
    color: #34D399;
}
.service-group:nth-child(1) .service-group-title:hover {
    background: rgba(52, 211, 153, 0.1);
}
/* Tools - Silver */
.service-group:nth-child(2) .service-group-title {
    color: #A9A9A9;
}
.service-group:nth-child(2) .service-group-title:hover {
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
    position: relative;
}
.service-item:last-child {
    margin-bottom: 0;
}
.service-item:hover {
    transform: translateY(-2px);
    border-color: rgba(30, 144, 255, 0.2);
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.2);
}
/* Prevent transform on service-item containing modals - transform breaks position:fixed */
.service-item:has(.modal-overlay):hover {
    transform: none;
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
.flow-title {
    font-size: 1.2rem;
    color: #34D399;
    text-align: center;
    margin-bottom: 1rem;
}
.flow-description {
    text-align: center;
    color: #999;
    font-style: italic;
    margin: 1.5rem 0;
}
.flow-step:not(:last-of-type)::after {
    content: '↓';
    position: absolute;
    left: 50%;
    bottom: -2rem;
    transform: translateX(-50%);
    font-size: 3rem;
    color: #fff;
    opacity: 0.5;
}
.apps-icons-row {
    display: flex;
    justify-content: flex-start;
    align-items: center;
    gap: 1.5rem;
    padding: 1.5rem;
    margin: 1.5rem;
    flex-wrap: wrap;
}
.app-icon {
    background: none;
    border: none;
    cursor: pointer;
    padding: 0.5rem;
    border-radius: 50%;
    transition: all 0.3s ease;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 1.5rem;
    color: #fff;
}
.app-icon:hover {
    background: rgba(30, 144, 255, 0.1);
}
.app-icon.selected {
    background: rgba(30, 144, 255, 0.2);
    box-shadow: 0 0 10px rgba(30, 144, 255, 0.3);
}
.app-icon.connected {
    background: rgba(52, 211, 153, 0.2);
    box-shadow: 0 0 10px rgba(52, 211, 153, 0.5);
}
.app-icon.mcp-icon {
    color: #A78BFA;
}
.app-icon.mcp-icon.connected {
    background: rgba(139, 92, 246, 0.2);
    box-shadow: 0 0 10px rgba(139, 92, 246, 0.5);
}
.app-icon.mcp-icon:hover {
    background: rgba(139, 92, 246, 0.1);
}
.app-details {
    width: 100%;
}
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
.info-button {
    background: transparent;
    border: none;
    color: #666;
    font-size: 1rem;
    cursor: pointer;
    padding: 0.25rem 0.5rem;
    border-radius: 50%;
    transition: all 0.2s;
    line-height: 1;
}
.info-button:hover {
    background: rgba(30, 144, 255, 0.1);
    color: #7eb2ff;
    transform: scale(1.1);
}
.info-section {
    background: rgba(30, 144, 255, 0.05);
    border-radius: 8px;
    margin-top: 1rem;
    padding: 1rem;
    border: 1px solid rgba(30, 144, 255, 0.1);
}
.info-section h4 {
    color: #fff;
    font-size: 1.1rem;
    margin: 0 0 0.75rem 0;
    font-weight: 600;
}
.info-section h5 {
    color: #7eb2ff;
    font-size: 0.95rem;
    margin: 0 0 0.5rem 0;
    font-weight: 500;
}
.info-section p {
    color: #bbb;
    margin: 0 0 0.5rem 0;
    line-height: 1.5;
}
.info-section ul {
    margin: 0;
    padding-left: 1.25rem;
    color: #bbb;
}
.info-section li {
    margin: 0.5rem 0;
    line-height: 1.4;
}
.info-subsection {
    margin-top: 1rem;
    padding-top: 0.75rem;
    border-top: 1px solid rgba(255, 255, 255, 0.05);
}
.info-subsection:first-child {
    margin-top: 0;
    padding-top: 0;
    border-top: none;
}
.info-subsection.security-notice {
    background: rgba(105, 240, 174, 0.05);
    border: 1px solid rgba(105, 240, 174, 0.1);
    border-radius: 6px;
    padding: 0.75rem;
    margin-top: 1rem;
}
.info-subsection.security-notice h5 {
    color: #69f0ae;
}
.fas.fa-qrcode {
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
.monitoring-tabs {
    display: flex;
    gap: 1rem;
    margin-bottom: 2rem;
    border-bottom: 1px solid rgba(30, 144, 255, 0.1);
    padding-bottom: 1rem;
    flex-wrap: wrap;
}
.tab-button {
    background: transparent;
    border: none;
    color: #999;
    padding: 0.5rem 1rem;
    cursor: pointer;
    font-size: 1rem;
    transition: all 0.3s ease;
    position: relative;
    white-space: nowrap;
    flex: 1;
    min-width: fit-content;
}
.tab-button::after {
    content: '';
    position: absolute;
    bottom: -1rem;
    left: 0;
    width: 100%;
    height: 2px;
    background: transparent;
    transition: background-color 0.3s ease;
}
.tab-button.active {
    color: white;
}
.tab-button.active::after {
    background: #1E90FF;
}
.tab-button:hover {
    color: #7EB2FF;
}
/* Notifications header with toggle */
.notifications-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
}
.notifications-header .header-left {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    flex: 1;
    cursor: pointer;
}
.notifications-header .header-toggle {
    flex-shrink: 0;
}
.toggle-status-hint {
    font-size: 0.8rem;
    color: #888;
    font-weight: normal;
}
.toggle-switch {
    position: relative;
    width: 52px;
    height: 28px;
    cursor: pointer;
}
.toggle-switch input {
    opacity: 0;
    width: 0;
    height: 0;
}
.toggle-slider {
    position: absolute;
    cursor: pointer;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background-color: rgba(100, 100, 100, 0.5);
    transition: 0.3s;
    border-radius: 28px;
}
.toggle-slider:before {
    position: absolute;
    content: "";
    height: 22px;
    width: 22px;
    left: 3px;
    bottom: 3px;
    background-color: white;
    transition: 0.3s;
    border-radius: 50%;
}
.toggle-switch input:checked + .toggle-slider {
    background: linear-gradient(45deg, #F59E0B, #D97706);
}
.toggle-switch input:checked + .toggle-slider:before {
    transform: translateX(24px);
}
.save-indicator {
    min-width: 20px;
    height: 20px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    margin-left: 0.5rem;
}
.save-spinner {
    width: 14px;
    height: 14px;
    border: 2px solid rgba(245, 158, 11, 0.3);
    border-top-color: #F59E0B;
    border-radius: 50%;
    animation: spin 1s linear infinite;
}
@keyframes spin {
    to { transform: rotate(360deg); }
}
.save-success {
    color: #22C55E;
    font-size: 16px;
}
.save-error {
    color: #EF4444;
    cursor: help;
    font-size: 16px;
}
.service-list.disabled {
    opacity: 0.5;
    pointer-events: none;
}
/* Simplified Notifications Toggle */
.notifications-toggle-group {
    padding: 1.5rem;
}
.notifications-toggle-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1rem;
}
.notifications-toggle-info {
    display: flex;
    align-items: center;
    gap: 0.75rem;
}
.notifications-toggle-info i {
    color: #34D399;
    font-size: 1.5rem;
}
.notifications-toggle-text h3 {
    color: #34D399;
    font-size: 1.1rem;
    margin: 0;
}
.notifications-toggle-text .toggle-status-hint {
    display: block;
    margin-top: 0.25rem;
}
.notifications-toggle-controls {
    display: flex;
    align-items: center;
    gap: 0.5rem;
}
.notifications-hint {
    color: #666;
    font-size: 0.85rem;
    margin-top: 1rem;
    margin-bottom: 0;
    padding-top: 1rem;
    border-top: 1px solid rgba(255, 255, 255, 0.05);
}
"#}
                    </style>
                </div>
            }
}
