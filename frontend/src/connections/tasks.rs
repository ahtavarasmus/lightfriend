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

