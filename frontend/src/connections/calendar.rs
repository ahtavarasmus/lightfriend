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
                    <img src="https://upload.wikimedia.org/wikipedia/commons/a/a5/Google_Calendar_icon_%282020%29.svg" alt="Google Calendar"/>
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
                        <li>{"Create New Event: Give event start time, duration and content and lightfriend will confirm with you the event to be created. Reply with yes or no to add or discard the suggestion. Voice call will also confirm the suggestion through SMS first."}</li>
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
                        border-radius: 12px;
                        padding: 1.5rem;
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

                    .info-subsection {
                        margin-bottom: 2rem;
                        padding: 1.2rem;
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
                        padding-left: 1.2rem;
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
            </style>
        </div>
    }
}

