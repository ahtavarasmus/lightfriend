use yew::prelude::*;
use web_sys::{MouseEvent, Event, HtmlSelectElement, EventTarget};
use wasm_bindgen::JsCast;
use serde_json::json;
use wasm_bindgen_futures::spawn_local;
use gloo_net::http::Request;
use crate::config;

#[derive(Properties, PartialEq)]
pub struct UberConnectProps {
    pub user_id: i32,
    pub sub_tier: Option<String>,
    pub discount: bool,
}

#[function_component(UberConnect)]
pub fn uber_connect(props: &UberConnectProps) -> Html {
    let error = use_state(|| None::<String>);
    let uber_connected = use_state(|| false);
    let connecting_uber = use_state(|| false);
    let selected_status = use_state(|| String::new());

    // Check connection status on component mount
    {
        let uber_connected = uber_connected.clone();
        use_effect_with_deps(
            move |_| {
                if let Some(window) = web_sys::window() {
                    if let Ok(Some(storage)) = window.local_storage() {
                        if let Ok(Some(token)) = storage.get_item("token") {
                            // Uber status
                            let uber_connected = uber_connected.clone();
                            let token = token.clone();
                            spawn_local(async move {
                                let request = Request::get(&format!("{}/api/auth/uber/status", config::get_backend_url()))
                                    .header("Authorization", &format!("Bearer {}", token))
                                    .send()
                                    .await;
                                if let Ok(response) = request {
                                    if response.ok() {
                                        if let Ok(data) = response.json::<serde_json::Value>().await {
                                            if let Some(connected) = data.get("connected").and_then(|v| v.as_bool()) {
                                                uber_connected.set(connected);
                                            }
                                        }
                                    } else {
                                        web_sys::console::log_1(&"Failed to check uber status".into());
                                    }
                                }
                            });
                        }
                    }
                }
            },
            () // Empty tuple as dependencies since we want this to run only once on mount
        )
    }

    let onclick_uber = {
        let connecting_uber = connecting_uber.clone();
        let error = error.clone();
        let uber_connected = uber_connected.clone();
        Callback::from(move |_: MouseEvent| {
            let connecting_uber = connecting_uber.clone();
            let error = error.clone();
            let uber_connected = uber_connected.clone();
            connecting_uber.set(true);
            error.set(None);
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    if let Ok(Some(token)) = storage.get_item("token") {
                        spawn_local(async move {
                            let request = Request::get(&format!("{}/api/auth/uber/login", config::get_backend_url()))
                                .header("Authorization", &format!("Bearer {}", token))
                                .send()
                                .await;
                            match request {
                                Ok(response) => {
                                    if response.status() == 200 {
                                        if let Ok(data) = response.json::<serde_json::Value>().await {
                                            if let Some(auth_url) = data.get("auth_url").and_then(|u| u.as_str()) {
                                                if let Some(window) = web_sys::window() {
                                                    let _ = window.location().set_href(auth_url);
                                                }
                                            } else {
                                                error.set(Some("Invalid response format".to_string()));
                                            }
                                        }
                                    } else {
                                        error.set(Some("Failed to initiate Uber connection".to_string()));
                                    }
                                }
                                Err(e) => {
                                    error.set(Some(format!("Network error: {}", e)));
                                }
                            }
                            connecting_uber.set(false);
                        });
                    }
                }
            }
        })
    };

    let onclick_delete_uber = {
        let uber_connected = uber_connected.clone();
        let error = error.clone();
        Callback::from(move |_: MouseEvent| {
            let uber_connected = uber_connected.clone();
            let error = error.clone();
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    if let Ok(Some(token)) = storage.get_item("token") {
                        spawn_local(async move {
                            let request = Request::delete(&format!("{}/api/auth/uber/connection", config::get_backend_url()))
                                .header("Authorization", &format!("Bearer {}", token))
                                .send()
                                .await;
                            match request {
                                Ok(response) => {
                                    if response.status() == 200 {
                                        uber_connected.set(false);
                                        error.set(None);
                                    } else {
                                        error.set(Some("Failed to disconnect Uber".to_string()));
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

    let onclick_test_uber = {
        let error = error.clone();
        Callback::from(move |_: MouseEvent| {
            let error = error.clone();
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    if let Ok(Some(token)) = storage.get_item("token") {
                        spawn_local(async move {
                            let request = Request::get(&format!("{}/api/uber", config::get_backend_url()))
                                .header("Authorization", &format!("Bearer {}", token))
                                .send()
                                .await;
                            match request {
                                Ok(response) => {
                                    if response.status() == 200 {
                                        if let Ok(data) = response.json::<serde_json::Value>().await {
                                            web_sys::console::log_1(&format!("Uber data: {:?}", data).into());
                                        }
                                    } else {
                                        error.set(Some("Failed to fetch uber data".to_string()));
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

    let onchange_status = {
        let selected_status = selected_status.clone();
        Callback::from(move |e: Event| {
            if let Some(target) = e.target() {
                if let Ok(select) = target.dyn_into::<HtmlSelectElement>() {
                    selected_status.set(select.value());
                }
            }
        })
    };

    let onclick_update_status = {
        let selected_status = selected_status.clone();
        let error = error.clone();
        Callback::from(move |_: MouseEvent| {
            let status = (*selected_status).clone();
            let selected_status = selected_status.clone();
            let error = error.clone();
            if !status.is_empty() {
                if let Some(window) = web_sys::window() {
                    if let Ok(Some(storage)) = window.local_storage() {
                        if let Ok(Some(token)) = storage.get_item("token") {
                            spawn_local(async move {
                                let request = Request::post(&format!("{}/api/uber/ride/status", config::get_backend_url()))
                                    .header("Authorization", &format!("Bearer {}", token))
                                    .header("Content-Type", "application/json")
                                    .json(&json!({
                                        "status": status,
                                    }))
                                    .unwrap()
                                    .send()
                                    .await;
                                match request {
                                    Ok(response) => {
                                        if response.status() == 200 {
                                            if let Ok(data) = response.json::<serde_json::Value>().await {
                                                web_sys::console::log_1(&format!("Updated status: {:?}", data).into());
                                                selected_status.set(String::new());
                                            }
                                        } else {
                                            error.set(Some("Failed to update status".to_string()));
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
            }
        })
    };

    html! {
        <div class="service-item">
            <div class="service-header">
            <div class="service-name">
                <img src="https://upload.wikimedia.org/wikipedia/commons/c/cc/Uber_logo_2018.svg" alt="Uber" width="24" height="24"/>
            </div>
            <button class="info-button" onclick={Callback::from(|_| {
                if let Some(element) = web_sys::window()
                    .and_then(|w| w.document())
                    .and_then(|d| d.get_element_by_id("uber-info"))
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
            if *uber_connected {
                <span class="service-status">{"Connected ✓"}</span>
            }
            </div>
            <p class="service-description">
                {"Request and manage Uber rides through SMS or voice calls. This integration connects to your Uber account, allowing you to book rides and check status on the go."}
            </p>
            <div id="uber-info" class="info-section" style="display: none">
                <h4>{"How It Works"}</h4>
                <div class="info-subsection">
                    <h5>{"SMS and Voice Call Tools"}</h5>
                    <ul>
                        <li>{"Request Ride: Book an Uber to your location with optional destination"}</li>
                        <li>{"Check Status: View your current ride status"}</li>
                    </ul>
                </div>
                <div class="info-subsection">
                    <h5>{"Ride Management Features"}</h5>
                    <ul>
                        <li>{"Account Integration: Connect your Uber account securely"}</li>
                        <li>{"Ride Requests: Book rides with specified pickup and dropoff"}</li>
                        <li>{"Status Updates: Get real-time updates via SMS"}</li>
                    </ul>
                </div>
                <div class="info-subsection security-notice">
                    <h5>{"Security & Privacy"}</h5>
                    <p>{"Your Uber data is protected through:"}</p>
                    <ul>
                        <li>{"OAuth 2.0: Secure authentication with storing only the encrypted access token"}</li>
                        <li>{"Limited Scope: Access restricted to ride management only"}</li>
                        <li>{"Revocable Access: You can disconnect anytime through the app or Uber settings"}</li>
                    </ul>
                    <p class="security-recommendation">{"Note: Ride details are transmitted via SMS or voice calls. For sensitive information, consider using Uber app directly."}</p>
                </div>
            </div>
            {
                if props.sub_tier.as_deref() == Some("tier 2") || props.discount {
                    html! {
                        <>
                            if *uber_connected {
                                <div class="uber-controls">
                                    <button
                                        onclick={onclick_delete_uber}
                                        class="disconnect-button"
                                    >
                                        {"Disconnect"}
                                    </button>
                                    {
                                        if props.user_id == 1 {
                                            html! {
                                                <>
                                                    <button
                                                        onclick={onclick_test_uber}
                                                        class="test-button"
                                                    >
                                                        {"Test Uber"}
                                                    </button>
                                                    <select class="test-select" onchange={onchange_status}>
                                                        <option value="" selected={(*selected_status).is_empty()}>{"Change Ride Status"}</option>
                                                        <option value="processing" selected={*selected_status == "processing"}>{"Processing"}</option>
                                                        <option value="accepted" selected={*selected_status == "accepted"}>{"Accepted"}</option>
                                                        <option value="arriving" selected={*selected_status == "arriving"}>{"Arriving"}</option>
                                                        <option value="in_progress" selected={*selected_status == "in_progress"}>{"In Progress"}</option>
                                                        <option value="completed" selected={*selected_status == "completed"}>{"Completed"}</option>
                                                    </select>
                                                    <button
                                                        onclick={onclick_update_status}
                                                        class="test-button"
                                                    >
                                                        {"Update Status"}
                                                    </button>
                                                </>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }
                                </div>
                            } else {
                                <button
                                    onclick={onclick_uber}
                                    class="connect-button"
                                >
                                    if *connecting_uber {
                                        {"Connecting..."}
                                    } else {
                                        {"Connect"}
                                    }
                                </button>
                            }
                            if let Some(err) = (*error).as_ref() {
                                <div class="error-message">
                                    {err}
                                </div>
                            }
                        </>
                    }
                } else {
                    html! {
                        <>
                        <div class="upgrade-prompt">
                            <div class="upgrade-content">
                                <h3>{"Upgrade to Enable Uber Integration"}</h3>
                                <p>{"Uber integration is available for premium plan subscribers. Upgrade your plan to connect your Uber account and manage rides through SMS and voice calls."}</p>
                                <a href="/pricing" class="upgrade-button">
                                    {"View Pricing Plans"}
                                </a>
                            </div>
                        </div>
                        if *uber_connected {
                            <div class="uber-controls">
                                <button
                                    onclick={onclick_delete_uber}
                                    class="disconnect-button"
                                >
                                    {"Disconnect"}
                                </button>
                                {
                                    if props.user_id == 1 {
                                        html! {
                                            <>
                                                <button
                                                    onclick={onclick_test_uber}
                                                    class="test-button"
                                                >
                                                    {"Test Uber"}
                                                </button>
                                                <select class="test-select" onchange={onchange_status}>
                                                    <option value="" selected={(*selected_status).is_empty()}>{"Change Ride Status"}</option>
                                                    <option value="processing" selected={*selected_status == "processing"}>{"Processing"}</option>
                                                    <option value="accepted" selected={*selected_status == "accepted"}>{"Accepted"}</option>
                                                    <option value="arriving" selected={*selected_status == "arriving"}>{"Arriving"}</option>
                                                    <option value="in_progress" selected={*selected_status == "in_progress"}>{"In Progress"}</option>
                                                    <option value="completed" selected={*selected_status == "completed"}>{"Completed"}</option>
                                                </select>
                                                <button
                                                    onclick={onclick_update_status}
                                                    class="test-button"
                                                >
                                                    {"Update Status"}
                                                </button>
                                            </>
                                        }
                                    } else {
                                        html! {}
                                    }
                                }
                            </div>
                        }
                        </>
                    }
                }
            }
            <style>
                {r#"
                    .upgrade-prompt {
                        padding: 20px;
                        text-align: center;
                        margin-top: 1rem;
                    }
                    .upgrade-content {
                        max-width: 500px;
                        margin: 0 auto;
                    }
                    .upgrade-content h3 {
                        color: #1E90FF;
                        margin-bottom: 1rem;
                    }
                    .upgrade-content p {
                        color: #CCC;
                        margin-bottom: 1.5rem;
                    }
                    .upgrade-button {
                        display: inline-block;
                        padding: 10px 20px;
                        background-color: #1E90FF;
                        color: white;
                        text-decoration: none;
                        border-radius: 5px;
                        transition: background-color 0.3s;
                    }
                    .upgrade-button:hover {
                        background-color: #1873CC;
                    }
                "#}
                {r#"
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
                        border-radius: 12px;
                        margin-top: 1rem;
                        font-size: 0.95rem;
                        line-height: 1.6;
                    }
                    .info-section::-webkit-scrollbar {
                        width: 8px;
                    }
                    .info-section::-webkit-scrollbar-track {
                        background: rgba(30, 144, 255, 0.1);
                        border-radius: 4px;
                    }
                    .info-section::-webkit-scrollbar-thumb {
                        background: rgba(30, 144, 255, 0.5);
                        border-radius: 4px;
                    }
                    .info-section::-webkit-scrollbar-thumb:hover {
                        background: rgba(30, 144, 255, 0.7);
                    }
                    .info-section h4 {
                        color: #1E90FF;
                        margin: 0 0 1.5rem 0;
                        font-size: 1.3rem;
                        font-weight: 600;
                    }
                    .info-subsection {
                        margin-bottom: 2rem;
                        border-radius: 8px;
                    }
                    .info-subsection:last-child {
                        margin-bottom: 0;
                    }
                    .info-subsection h5 {
                        color: #1E90FF;
                        margin: 0 0 1rem 0;
                        font-size: 1.1rem;
                        font-weight: 500;
                    }
                    .info-subsection ul {
                        margin: 0;
                        list-style-type: none;
                    }
                    .info-subsection li {
                        margin-bottom: 0.8rem;
                        color: #CCC;
                        position: relative;
                    }
                    .info-subsection li:before {
                        content: "•";
                        color: #1E90FF;
                        position: absolute;
                        left: -1.2rem;
                    }
                    .info-subsection li:last-child {
                        margin-bottom: 0;
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
                        color: #999 !important;
                        margin-top: 1rem !important;
                        font-size: 0.9rem;
                        padding-top: 1rem;
                        border-top: 1px solid rgba(30, 144, 255, 0.1);
                    }
                "#}
                {r#"
                    .test-select {
                        margin-left: 10px;
                        padding: 5px;
                        background-color: #f0f0f0;
                        border: 1px solid #ccc;
                        border-radius: 4px;
                    }
                "#}
            </style>
        </div>
    }
}
