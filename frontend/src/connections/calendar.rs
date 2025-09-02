use yew::prelude::*;
use gloo_net::http::Request;
use serde_json::json;
use wasm_bindgen_futures::spawn_local;
use web_sys::{MouseEvent, Event, HtmlInputElement};
use crate::config;
#[derive(Properties, PartialEq)]
pub struct CalendarProps {
    pub user_id: i32,
    pub sub_tier: Option<String>,
    pub discount: bool,
    #[prop_or_default]
    pub on_connection_change: Option<Callback<bool>>,
}
#[function_component(CalendarConnect)]
pub fn calendar_connect(props: &CalendarProps) -> Html {
    let error = use_state(|| None::<String>);
    let connecting = use_state(|| false);
    let calendar_connected = use_state(|| false);
    let all_calendars = use_state(|| false);
    // Check connection status on component mount
    {
        let calendar_connected = calendar_connected.clone();
        let on_connection_change = props.on_connection_change.clone();
        use_effect_with_deps(
            move |_| {
                if let Some(window) = web_sys::window() {
                    if let Ok(Some(storage)) = window.local_storage() {
                        if let Ok(Some(token)) = storage.get_item("token") {
                            // Check Google Calendar status
                            let calendar_connected = calendar_connected.clone();
                            let on_connection_change = on_connection_change.clone();
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
                                                if let Some(callback) = on_connection_change {
                                                    callback.emit(connected);
                                                }
                                            }
                                        }
                                    } else {
                                        web_sys::console::log_1(&"Failed to check calendar status".into());
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
        let on_connection_change = props.on_connection_change.clone();
        Callback::from(move |_: MouseEvent| {
            let calendar_connected = calendar_connected.clone();
            let error = error.clone();
            let on_connection_change = on_connection_change.clone();
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
                                        if let Some(callback) = on_connection_change {
                                            callback.emit(false);
                                        }
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
        <div class="service-item">
            <div class="service-header">
                <div class="service-name">
                    <img src="https://upload.wikimedia.org/wikipedia/commons/a/a5/Google_Calendar_icon_%282020%29.svg" alt="Google Calendar" width="24" height="24"/>
                    {"Google Calendar"}
                </div>
                <button class="info-button" onclick={Callback::from(|_| {
                    if let Some(element) = web_sys::window()
                        .and_then(|w| w.document())
                        .and_then(|d| d.get_element_by_id("calendar-info"))
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
                if *calendar_connected {
                    <span class="service-status">{"Connected ✓"}</span>
                }
            </div>
            <p class="service-description">
                {"Access your Google Calendar events through SMS or voice calls."}
            </p>
            <div id="calendar-info" class="info-section" style="display: none">
                <h4>{"How It Works"}</h4>
                <div class="info-subsection">
                    <h5>{"SMS and Voice Call Tools"}</h5>
                    <ul>
                        <li>{"Fetch Specific Timeframe: Fetch your calendar events between start and end time"}</li>
                        <li>{"Create New Event: Give event start time, duration and content and lightfriend will create the event."}</li>
                    </ul>
                </div>
                <div class="info-subsection">
                    <h5>{"Calendar Access Options"}</h5>
                    <ul>
                        <li>{"Primary Calendar: Default access to your main Google Calendar"}</li>
                        <li>{"All Calendars: Optional access to all your calendars, including shared ones"}</li>
                    </ul>
                </div>
                <div class="info-subsection security-notice">
                    <h5>{"Security & Privacy"}</h5>
                    <p>{"Your calendar data is protected through:"}</p>
                    <ul>
                        <li>{"OAuth 2.0: Secure authentication with storing only the encrypted access token"}</li>
                        <li>{"Limited Scope: Access restricted to calendar data only"}</li>
                        <li>{"Revocable Access: You can disconnect anytime through lightfriend or Google Account settings"}</li>
                    </ul>
                    <p class="security-recommendation">{"Note: Calendar events are transmitted via SMS or voice calls. For sensitive event details, consider using Google Calendar directly."}</p>
                </div>
            </div>
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
                                                // Get current time for the test event
                                                let now = web_sys::js_sys::Date::new_0();
                                                let start_time = now.to_iso_string().as_string().unwrap();
                                               
                                                let test_event = json!({
                                                    "start_time": start_time,
                                                    "duration_minutes": 30,
                                                    "summary": "Test Event",
                                                    "description": "This is a test event created by the test button",
                                                    "add_notification": true
                                                });
                                                spawn_local(async move {
                                                    match Request::post(&format!("{}/api/calendar/create", config::get_backend_url()))
                                                        .header("Authorization", &format!("Bearer {}", token))
                                                        .json(&test_event)
                                                        .unwrap()
                                                        .send()
                                                        .await {
                                                        Ok(response) => {
                                                            if response.status() == 200 {
                                                                web_sys::console::log_1(&"Test event created successfully".into());
                                                            } else {
                                                                error.set(Some("Failed to create test event".to_string()));
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
                                <button
                                    onclick={onclick_test}
                                    class="test-button"
                                >
                                    {"Create Test Event"}
                                </button>
                            }
                            } else {
                                html! {}
                            }
                        }
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
                                                let now = web_sys::js_sys::Date::new_0();
                                                let today_start = web_sys::js_sys::Date::new_0();
                                                let today_end = web_sys::js_sys::Date::new_0();
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
                    </div>
                } else {
                    if props.sub_tier.as_deref() == Some("tier 2") || props.discount {
                        <div class="calendar-connect-options">
                            <label class="calendar-checkbox">
                                <input
                                    type="checkbox"
                                    checked={*all_calendars}
                                    onchange={
                                        let all_calendars = all_calendars.clone();
                                        Callback::from(move |e: Event| {
                                            let input: HtmlInputElement = e.target_unchecked_into();
                                            all_calendars.set(input.checked());
                                        })
                                    }
                                />
                                {"Access all calendars (including shared)"}
                            </label>
                            <button
                                onclick={onclick_calendar}
                                class="connect-button"
                            >
                                if *connecting {
                                    {"Connecting..."}
                                } else {
                                    {"Connect"}
                                }
                            </button>
                        </div>
                    } else {
                        <div class="upgrade-prompt">
                            <div class="upgrade-content">
                                <h3>{"Upgrade to Enable Calendar Integration"}</h3>
                                <a href="/pricing" class="upgrade-button">
                                    {"View Pricing Plans"}
                                </a>
                            </div>
                        </div>
                    }
                }
            if let Some(err) = (*error).as_ref() {
                <div class="error-message">
                    {err}
                </div>
            }
            <style>
                {r#"
                    .test-button {
                        background-color: #4CAF50;
                        color: white;
                        padding: 8px 16px;
                        border: none;
                        border-radius: 4px;
                        cursor: pointer;
                        margin-left: 10px;
                        font-size: 14px;
                        transition: background-color 0.3s;
                    }
                    .test-button:hover {
                        background-color: #45a049;
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
                        border-radius: 12px;
                        margin-top: 1rem;
                        font-size: 0.95rem;
                        line-height: 1.6;
                    }
                    .info-section h4 {
                        color: #1E90FF;
                        margin: 0 0 1.5rem 0;
                        font-size: 1.3rem;
                        font-weight: 600;
                    }
                    #calendar-info {
                        max-height: 400px;
                        overflow-y: auto;
                        scrollbar-width: thin;
                        scrollbar-color: rgba(30, 144, 255, 0.5) rgba(30, 144, 255, 0.1);
                        border-radius: 12px;
                        margin-top: 1rem;
                        font-size: 0.95rem;
                        line-height: 1.6;
                    }
                    #calendar-info::-webkit-scrollbar {
                        width: 8px;
                    }
                    #calendar-info::-webkit-scrollbar-track {
                        background: rgba(30, 144, 255, 0.1);
                        border-radius: 4px;
                    }
                    #calendar-info::-webkit-scrollbar-thumb {
                        background: rgba(30, 144, 255, 0.5);
                        border-radius: 4px;
                    }
                    #calendar-info::-webkit-scrollbar-thumb:hover {
                        background: rgba(30, 144, 255, 0.7);
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
                    .upgrade-prompt {
                        background: rgba(30, 144, 255, 0.1);
                        padding: 1.5rem;
                        border-radius: 8px;
                        text-align: center;
                        margin-top: 1rem;
                    }
                    .upgrade-content h3 {
                        color: #1E90FF;
                        margin: 0 0 1rem 0;
                        font-size: 1.2rem;
                    }
                    .upgrade-content p {
                        color: #CCC;
                        margin-bottom: 1.5rem;
                        line-height: 1.5;
                    }
                    .upgrade-button {
                        display: inline-block;
                        background-color: #1E90FF;
                        color: white;
                        padding: 0.8rem 1.5rem;
                        border-radius: 4px;
                        text-decoration: none;
                        transition: background-color 0.3s;
                    }
                    .upgrade-button:hover {
                        background-color: #1873CC;
                    }
                    .calendar-connect-options {
                        display: flex;
                        align-items: center;
                        justify-content: space-between;
                        margin-top: 1rem;
                    }
                    .calendar-checkbox {
                        display: flex;
                        align-items: center;
                        color: #FFFFFF;
                        font-size: 0.9rem;
                        cursor: pointer;
                        user-select: none;
                    }
                    .calendar-checkbox input {
                        width: 16px;
                        height: 16px;
                        margin-right: 0.5rem;
                        accent-color: #1E90FF;
                        cursor: pointer;
                    }
                    .connect-button {
                        background-color: #1E90FF;
                        color: white;
                        padding: 0.5rem 1rem;
                        border: none;
                        border-radius: 4px;
                        cursor: pointer;
                        font-size: 0.9rem;
                        transition: background-color 0.3s;
                        min-width: 80px;
                        text-align: center;
                    }
                    .connect-button:hover {
                        background-color: #1873CC;
                    }
                    .calendar-controls {
                        display: flex;
                        align-items: center;
                        margin-top: 1rem;
                    }
                    .disconnect-button {
                        background-color: #f44336;
                        color: white;
                        padding: 0.5rem 1rem;
                        border: none;
                        border-radius: 4px;
                        cursor: pointer;
                        font-size: 0.9rem;
                        transition: background-color 0.3s;
                    }
                    .disconnect-button:hover {
                        background-color: #d32f2f;
                    }
                    .service-status {
                        color: #4CAF50;
                        font-weight: 500;
                        margin-left: auto;
                        padding-left: 1rem;
                    }
                    .error-message {
                        color: #f44336;
                        margin-top: 1rem;
                        font-size: 0.9rem;
                    }
                    .service-item {
                        background: rgba(0, 0, 0, 0.2);
                        border: 1px solid rgba(0, 136, 204, 0.2);
                        border-radius: 12px;
                        width: 100%;
                        padding: 1.5rem;
                        margin: 1rem 0;
                        transition: all 0.3s ease;
                        color: #fff;
                    }
                    .service-item:hover {
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
