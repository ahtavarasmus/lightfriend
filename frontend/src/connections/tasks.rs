use yew::prelude::*;
use web_sys::{MouseEvent, js_sys::Date};
use serde_json::json;
use wasm_bindgen_futures::spawn_local;
use gloo_net::http::Request;
use crate::config;

#[derive(Properties, PartialEq)]
pub struct TasksConnectProps {
    pub user_id: i32,
    pub sub_tier: Option<String>,
    pub discount: bool,
}

#[function_component(TasksConnect)]
pub fn tasks_connect(props: &TasksConnectProps) -> Html {
    let error = use_state(|| None::<String>);
    let tasks_connected = use_state(|| false);
    let connecting_tasks = use_state(|| false);

    // Check connection status on component mount
    {
        let tasks_connected = tasks_connected.clone();
        use_effect_with_deps(
            move |_| {
                if let Some(window) = web_sys::window() {
                    if let Ok(Some(storage)) = window.local_storage() {
                        if let Ok(Some(token)) = storage.get_item("token") {
                            // Google Tasks status
                            let tasks_connected = tasks_connected.clone();
                            let token = token.clone();
                            spawn_local(async move {
                                let request = Request::get(&format!("{}/api/auth/google/tasks/status", config::get_backend_url()))
                                    .header("Authorization", &format!("Bearer {}", token))
                                    .send()
                                    .await;

                                if let Ok(response) = request {
                                    if response.ok() {
                                        if let Ok(data) = response.json::<serde_json::Value>().await {
                                            if let Some(connected) = data.get("connected").and_then(|v| v.as_bool()) {
                                                tasks_connected.set(connected);
                                            }
                                        }
                                    } else {
                                        web_sys::console::log_1(&"Failed to check tasks status".into());
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

    let onclick_tasks = {
        let connecting_tasks = connecting_tasks.clone();
        let error = error.clone();
        let tasks_connected = tasks_connected.clone();
        Callback::from(move |_: MouseEvent| {
            let connecting_tasks = connecting_tasks.clone();
            let error = error.clone();
            let tasks_connected = tasks_connected.clone();

            connecting_tasks.set(true);
            error.set(None);

            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    if let Ok(Some(token)) = storage.get_item("token") {
                        spawn_local(async move {
                            let request = Request::get(&format!("{}/api/auth/google/tasks/login", config::get_backend_url()))
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
                                        error.set(Some("Failed to initiate Google Tasks connection".to_string()));
                                    }
                                }
                                Err(e) => {
                                    error.set(Some(format!("Network error: {}", e)));
                                }
                            }
                            connecting_tasks.set(false);
                        });
                    }
                }
            }
        })
    };

    let onclick_delete_tasks = {
        let tasks_connected = tasks_connected.clone();
        let error = error.clone();
        Callback::from(move |_: MouseEvent| {
            let tasks_connected = tasks_connected.clone();
            let error = error.clone();

            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    if let Ok(Some(token)) = storage.get_item("token") {
                        spawn_local(async move {
                            let request = Request::delete(&format!("{}/api/auth/google/tasks/connection", config::get_backend_url()))
                                .header("Authorization", &format!("Bearer {}", token))
                                .send()
                                .await;

                            match request {
                                Ok(response) => {
                                    if response.status() == 200 {
                                        tasks_connected.set(false);
                                        error.set(None);
                                    } else {
                                        error.set(Some("Failed to disconnect Google Tasks".to_string()));
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

    let onclick_test_tasks = {
        let error = error.clone();
        Callback::from(move |_: MouseEvent| {
            let error = error.clone();
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    if let Ok(Some(token)) = storage.get_item("token") {
                        spawn_local(async move {
                            let request = Request::get(&format!("{}/api/tasks", config::get_backend_url()))
                                .header("Authorization", &format!("Bearer {}", token))
                                .send()
                                .await;

                            match request {
                                Ok(response) => {
                                    if response.status() == 200 {
                                        if let Ok(data) = response.json::<serde_json::Value>().await {
                                            web_sys::console::log_1(&format!("Tasks: {:?}", data).into());
                                        }
                                    } else {
                                        error.set(Some("Failed to fetch tasks".to_string()));
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
                <img src="https://upload.wikimedia.org/wikipedia/commons/5/5b/Google_Tasks_2021.svg" alt="Google Tasks"  width="24" height="24"/>
                {"Google Tasks"}
            </div>
            <button class="info-button" onclick={Callback::from(|_| {
                if let Some(element) = web_sys::window()
                    .and_then(|w| w.document())
                    .and_then(|d| d.get_element_by_id("tasks-info"))
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
            if *tasks_connected {
                <span class="service-status">{"Connected ✓"}</span>
            }
            </div>
            <p class="service-description">
                {"Create and manage tasks and ideas through SMS or voice calls. This integration creates a dedicated \"lightfriend\" list, keeping your existing task lists untouched. "}
                {"Perfect for quick note-taking or capturing ideas on the go."}
            </p>
                            <div id="tasks-info" class="info-section" style="display: none">
                                <h4>{"How It Works"}</h4>

                                <div class="info-subsection">
                                    <h5>{"SMS and Voice Call Tools"}</h5>
                                    <ul>
                                        <li>{"Create a Task: Add a new task with optional due date"}</li>
                                        <li>{"List Tasks: View your pending and completed tasks"}</li>
                                    </ul>
                                </div>

                                <div class="info-subsection">
                                    <h5>{"Task Management Features"}</h5>
                                    <ul>
                                        <li>{"Dedicated List: All tasks are stored in a \"lightfriend\" list"}</li>
                                        <li>{"Due Dates: Set deadlines for your tasks (note: times will be set to midnight)"}</li>
                                        <li>{"List Organization: Your existing Google Tasks lists remain untouched"}</li>
                                    </ul>
                                </div>

                                <div class="info-subsection security-notice">
                                    <h5>{"Security & Privacy"}</h5>
                                    <p>{"Your tasks data is protected through:"}</p>
                                    <ul>
                                        <li>{"OAuth 2.0: Secure authentication with storing only the encrypted access token"}</li>
                                        <li>{"Limited Scope: Access restricted to tasks management only"}</li>
                                        <li>{"Revocable Access: You can disconnect anytime through lightfriend or Google Account settings"}</li>
                                    </ul>
                                    <p class="security-recommendation">{"Note: Tasks are transmitted via SMS or voice calls. For sensitive task details, consider using Google Tasks directly."}</p>
                                </div>
                            </div>

                    {
                        if props.sub_tier.as_deref() == Some("tier 2") || props.discount {
                            html! {
                                <>
                            
                            if *tasks_connected {
                                <div class="tasks-controls">
                                    <button 
                                        onclick={onclick_delete_tasks}
                                        class="disconnect-button"
                                    >
                                        {"Disconnect"}
                                    </button>
                                    {
                                        if props.user_id == 1 {
                                            html! {
                                                <>
                                                    <button 
                                                        onclick={onclick_test_tasks}
                                                        class="test-button"
                                                    >
                                                        {"Test Tasks"}
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
                                                                                let request = Request::post(&format!("{}/api/tasks/create", config::get_backend_url()))
                                                                                    .header("Authorization", &format!("Bearer {}", token))
                                                                                    .header("Content-Type", "application/json")
                                                                                    .json(&json!({
                                                                                        "title": format!("Test task created at {}", Date::new_0().to_iso_string()),
                                                                                    }))
                                                                                    .unwrap()
                                                                                    .send()
                                                                                    .await;

                                                                                match request {
                                                                                    Ok(response) => {
                                                                                        if response.status() == 200 {
                                                                                            if let Ok(data) = response.json::<serde_json::Value>().await {
                                                                                                web_sys::console::log_1(&format!("Created task: {:?}", data).into());
                                                                                            }
                                                                                        } else {
                                                                                            error.set(Some("Failed to create task".to_string()));
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
                                                        {"Create Test Task"}
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
                                    onclick={onclick_tasks}
                                    class="connect-button"
                                >
                                    if *connecting_tasks {
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
                                <h3>{"Upgrade to Enable Tasks Integration"}</h3>
                                <p>{"Google Tasks integration is available for premium plan subscribers. Upgrade your plan to connect your tasks and manage them through SMS and voice calls."}</p>
                                <a href="/pricing" class="upgrade-button">
                                    {"View Pricing Plans"}
                                </a>
                            </div>
                        </div>
                        if *tasks_connected {
                                <div class="tasks-controls">
                                    <button 
                                        onclick={onclick_delete_tasks}
                                        class="disconnect-button"
                                    >
                                        {"Disconnect"}
                                    </button>
                                    {
                                        if props.user_id == 1 {
                                            html! {
                                                <>
                                                    <button 
                                                        onclick={onclick_test_tasks}
                                                        class="test-button"
                                                    >
                                                        {"Test Tasks"}
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
                                                                                let request = Request::post(&format!("{}/api/tasks/create", config::get_backend_url()))
                                                                                    .header("Authorization", &format!("Bearer {}", token))
                                                                                    .header("Content-Type", "application/json")
                                                                                    .json(&json!({
                                                                                        "title": format!("Test task created at {}", Date::new_0().to_iso_string()),
                                                                                    }))
                                                                                    .unwrap()
                                                                                    .send()
                                                                                    .await;

                                                                                match request {
                                                                                    Ok(response) => {
                                                                                        if response.status() == 200 {
                                                                                            if let Ok(data) = response.json::<serde_json::Value>().await {
                                                                                                web_sys::console::log_1(&format!("Created task: {:?}", data).into());
                                                                                            }
                                                                                        } else {
                                                                                            error.set(Some("Failed to create task".to_string()));
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
                                                        {"Create Test Task"}
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
            </style>
        </div>
    }
}

