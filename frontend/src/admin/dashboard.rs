use yew::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::window;
use wasm_bindgen::closure::Closure;
use crate::config;
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use yew_router::prelude::*;
use crate::Route;
use chrono::{Utc, TimeZone};
use crate::profile::billing_models::format_timestamp;
use serde_json::json;

#[derive(Serialize)]
struct BroadcastMessage {
    message: String,
}

#[derive(Serialize)]
struct EmailBroadcastMessage {
    subject: String,
    message: String,
}


#[derive(Deserialize, Clone, Debug)]
struct UserInfo {
    id: i32,
    email: String,
    phone_number: String,
    time_to_live: Option<i32>,
    verified: bool,
    credits: f32,
    notify: bool,
    preferred_number: Option<String>,
    sub_tier: Option<String>,
    msgs_left: i32,
    credits_left: f32,
    discount: bool,
    discount_tier: Option<String>,
}

#[derive(Clone, Debug)]
struct DeleteModalState {
    show: bool,
    user_id: Option<i32>,
    user_email: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct EmailJudgmentResponse {
    id: i32,
    email_timestamp: i32,
    processed_at: i32,
    should_notify: bool,
    score: i32,
    reason: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct UsageLog {
    id: i32,
    activity_type: String,
    timestamp: i32,
    sid: Option<String>,
    status: Option<String>,
    success: Option<bool>,
    credits: Option<f32>,
    time_consumed: Option<i32>,
    reason: Option<String>,
    recharge_threshold_timestamp: Option<i32>,
    zero_credits_timestamp: Option<i32>,
}




#[derive(Debug, Clone)]
struct ChatMessage {
    content: String,
    is_user: bool,
    timestamp: i64,
    image_url: Option<String>,
}

#[derive(Debug, Clone)]
struct ImagePreview {
    data_url: Option<String>,
}

#[function_component(AdminDashboard)]
pub fn admin_dashboard() -> Html {
    let users = use_state(|| Vec::new());
    let error = use_state(|| None::<String>);
    let usage_logs = use_state(|| Vec::<UsageLog>::new());
    let activity_filter = use_state(|| None::<String>);
    let selected_user_id = use_state(|| None::<i32>);
    let message = use_state(|| String::new());
    let email_subject = use_state(|| String::new());
    let email_message = use_state(|| String::new());
    let test_message = use_state(|| String::new());
    let chat_messages = use_state(|| Vec::<ChatMessage>::new());
    let image_preview = use_state(|| ImagePreview { data_url: None });
    let delete_modal = use_state(|| DeleteModalState {
        show: false,
        user_id: None,
        user_email: None,
    });

    let users_effect = users.clone();
    let error_effect = error.clone();

    // Fetch usage logs
    {
        let usage_logs = usage_logs.clone();
        let error = error.clone();
        let activity_filter = activity_filter.clone();

        use_effect_with_deps(move |_| {
            let usage_logs = usage_logs.clone();
            let error = error.clone();
            
            wasm_bindgen_futures::spawn_local(async move {
                if let Some(token) = window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|storage| storage.get_item("token").ok())
                    .flatten()
                {
                    // This endpoint doesn't exist yet - we'll implement it later
                    match Request::get(&format!("{}/api/admin/usage-logs", config::get_backend_url()))
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await
                    {
                        Ok(response) => {
                            if response.ok() {
                                match response.json::<Vec<UsageLog>>().await {
                                    Ok(logs) => {
                                        usage_logs.set(logs);
                                    }
                                    Err(_) => {
                                        error.set(Some("Failed to parse usage logs data".to_string()));
                                    }
                                }
                            } else {
                                error.set(Some("Failed to fetch usage logs".to_string()));
                            }
                        }
                        Err(_) => {
                            error.set(Some("Failed to fetch usage logs".to_string()));
                        }
                    }
                }
            });
            || ()
        }, [activity_filter]);
    }

    use_effect_with_deps(move |_| {
        let users = users_effect;
        let error = error_effect;
        wasm_bindgen_futures::spawn_local(async move {
            // Get token from localStorage
            let token = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|storage| storage.get_item("token").ok())
                .flatten();

            if let Some(token) = token {
                match Request::get(&format!("{}/api/admin/users", config::get_backend_url()))
                    .header("Authorization", &format!("Bearer {}", token))
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.ok() {
                            match response.json::<Vec<UserInfo>>().await {
                                Ok(data) => {
                                    users.set(data);
                                }
                                Err(_) => {
                                    error.set(Some("Failed to parse users data".to_string()));
                                }
                            }
                        } else {
                            error.set(Some("Not authorized to view this page".to_string()));
                        }
                    }
                    Err(_) => {
                        error.set(Some("Failed to fetch users".to_string()));
                    }
                }
            }
        });
        || ()
    }, ());

    let toggle_user_details = {
        let selected_user_id = selected_user_id.clone();
        Callback::from(move |user_id: i32| {
            selected_user_id.set(Some(match *selected_user_id {
                Some(current_id) if current_id == user_id => return selected_user_id.set(None),
                _ => user_id
            }));
        })
    };

    html! {
        <div class="dashboard-container">
            <div class="dashboard-panel">
                <div class="panel-header">
                    <h1 class="panel-title">{"Admin Dashboard"}</h1>
                    <Link<Route> to={Route::Home} classes="back-link">
                        {"Back to Home"}
                    </Link<Route>>
                </div>

                
                <div class="broadcast-section">
                    <h2>{"SMS Broadcast"}</h2>
                    <textarea
                        value={(*message).clone()}
                        onchange={{
                            let message = message.clone();
                            Callback::from(move |e: Event| {
                                let input: web_sys::HtmlTextAreaElement = e.target_unchecked_into();
                                message.set(input.value());
                            })
                        }}
                        placeholder="Enter SMS message to broadcast..."
                        class="broadcast-textarea"
                    />
                    <button
                        onclick={{
                            let message = message.clone();
                            let error = error.clone();
                            Callback::from(move |_| {
                                let message = message.clone();
                                let error = error.clone();
                                
                                if message.is_empty() {
                                    error.set(Some("Message cannot be empty".to_string()));
                                    return;
                                }
                                
                                wasm_bindgen_futures::spawn_local(async move {
                                    if let Some(token) = window()
                                        .and_then(|w| w.local_storage().ok())
                                        .flatten()
                                        .and_then(|storage| storage.get_item("token").ok())
                                        .flatten()
                                    {
                                        let broadcast_message = BroadcastMessage {
                                            message: (*message).clone(),
                                        };
                                        
                                        match Request::post(&format!("{}/api/admin/broadcast", config::get_backend_url()))
                                            .header("Authorization", &format!("Bearer {}", token))
                                            .json(&broadcast_message)
                                            .unwrap()
                                            .send()
                                            .await
                                        {
                                            Ok(response) => {
                                                if response.ok() {
                                                    message.set(String::new());
                                                    error.set(Some("SMS broadcast sent successfully".to_string()));
                                                } else {
                                                    error.set(Some("Failed to send SMS broadcast(hahaa you were brave enough to try!)".to_string()));
                                                }
                                            }
                                            Err(_) => {
                                                error.set(Some("Failed to send SMS broadcast request(hahaa you were brave enough to try!)".to_string()));
                                            }
                                        }
                                    }
                                });
                            })
                        }}
                        class="broadcast-button"
                    >
                        {"Send SMS Broadcast"}
                    </button>
                </div>

                <div class="broadcast-section email-broadcast">
                    <h2>{"Email Broadcast"}</h2>
                    <input
                        type="text"
                        value={(*email_subject).clone()}
                        onchange={{
                            let email_subject = email_subject.clone();
                            Callback::from(move |e: Event| {
                                let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                                email_subject.set(input.value());
                            })
                        }}
                        placeholder="Enter email subject..."
                        class="email-subject-input"
                    />
                    <textarea
                        value={(*email_message).clone()}
                        onchange={{
                            let email_message = email_message.clone();
                            Callback::from(move |e: Event| {
                                let input: web_sys::HtmlTextAreaElement = e.target_unchecked_into();
                                email_message.set(input.value());
                            })
                        }}
                        placeholder="Enter email message to broadcast..."
                        class="broadcast-textarea"
                    />
                    <button
                        onclick={{
                            let email_subject = email_subject.clone();
                            let email_message = email_message.clone();
                            let error = error.clone();
                            Callback::from(move |_| {
                                let email_subject = email_subject.clone();
                                let email_message = email_message.clone();
                                let error = error.clone();
                                
                                if email_subject.is_empty() || email_message.is_empty() {
                                    error.set(Some("Subject and message cannot be empty".to_string()));
                                    return;
                                }
                                
                                wasm_bindgen_futures::spawn_local(async move {
                                    if let Some(token) = window()
                                        .and_then(|w| w.local_storage().ok())
                                        .flatten()
                                        .and_then(|storage| storage.get_item("token").ok())
                                        .flatten()
                                    {
                                        let broadcast_message = EmailBroadcastMessage {
                                            subject: (*email_subject).clone(),
                                            message: (*email_message).clone(),
                                        };
                                        
                                        match Request::post(&format!("{}/api/admin/broadcast-email", config::get_backend_url()))
                                            .header("Authorization", &format!("Bearer {}", token))
                                            .json(&broadcast_message)
                                            .unwrap()
                                            .send()
                                            .await
                                        {
                                            Ok(response) => {
                                                if response.ok() {
                                                    email_subject.set(String::new());
                                                    email_message.set(String::new());
                                                    error.set(Some("Email broadcast sent successfully".to_string()));
                                                } else {
                                                    error.set(Some("Failed to send email broadcast(lol you thought you were him?)".to_string()));
                                                }
                                            }
                                            Err(_) => {
                                                error.set(Some("Failed to send email broadcast request(lol you thought you were him?)".to_string()));
                                            }
                                        }
                                    }
                                });
                            })
                        }}
                        class="broadcast-button email"
                    >
                        {"Send Email Broadcast"}
                    </button>
                </div>

                // Test SMS Chat Section
                <div class="test-chat-section">
                    <h2>{"Test SMS Processing (User ID: 1)"}</h2>
                    <div class="chat-window">
                        <div class="chat-messages">
                            {
                                (*chat_messages).iter().map(|msg| {
                                    let timestamp = chrono::DateTime::<chrono::Utc>::from_timestamp(msg.timestamp, 0)
                                        .map(|dt| dt.format("%H:%M:%S").to_string())
                                        .unwrap_or_else(|| "unknown time".to_string());
                                    
                                    html! {
                                        <div class={classes!("chat-message", if msg.is_user { "user" } else { "assistant" })}>
                                            if let Some(image_url) = &msg.image_url {
                                                <div class="message-image">
                                                    <img src={image_url.clone()} alt="Uploaded content" />
                                                </div>
                                            }
                                            <div class="message-content">{&msg.content}</div>
                                            <div class="message-timestamp">{timestamp}</div>
                                        </div>
                                    }
                                }).collect::<Html>()
                            }
                        </div>
                    <div class="chat-input">
                        {
                            if let Some(preview_url) = &(*image_preview).data_url {
                                html! {
                                    <div class="image-preview">
                                        <img src={preview_url.clone()} alt="Preview" />
                                        <button 
                                            onclick={{
                                                let image_preview = image_preview.clone();
                                                Callback::from(move |_| {
                                                    image_preview.set(ImagePreview { data_url: None });
                                                })
                                            }}
                                            class="remove-image"
                                        >
                                            {"×"}
                                        </button>
                                    </div>
                                }
                            } else {
                                html! {}
                            }
                        }
                        <div class="chat-input-wrapper">
                            <input
                                type="file"
                                accept="image/*"
                                id="image-upload"
                                class="image-upload"
                                onchange={{
                                    let test_message = test_message.clone();
                                    let chat_messages = chat_messages.clone();
                                    let error = error.clone();
                                    let image_preview_handle = image_preview.clone();
                                    
                                    Callback::from(move |e: Event| {
                                        let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                                        if let Some(files) = input.files() {
                                            if let Some(file) = files.get(0) {
                                                let test_message = test_message.clone();
                                                let chat_messages = chat_messages.clone();
                                                let error = error.clone();
                                                let image_preview = image_preview_handle.clone();
                                                
                                                // First create a preview of the image
                                                let file_reader = web_sys::FileReader::new().unwrap();
                                                let file_reader_clone = file_reader.clone();
                                                let image_preview_for_closure = image_preview.clone();
                                                
                                                let onload = Closure::wrap(Box::new(move || {
                                                    if let Ok(result) = file_reader_clone.result() {
                                                        if let Some(data_url) = result.as_string() {
                                                            image_preview_for_closure.set(ImagePreview {
                                                                data_url: Some(data_url),
                                                            });
                                                        }
                                                    }
                                                }) as Box<dyn FnMut()>);
                                                
                                                file_reader.set_onload(Some(onload.as_ref().unchecked_ref()));
                                                file_reader.read_as_data_url(&file).unwrap();
                                                onload.forget();

                                                wasm_bindgen_futures::spawn_local(async move {
                                                    if let Some(token) = window()
                                                        .and_then(|w| w.local_storage().ok())
                                                        .flatten()
                                                        .and_then(|storage| storage.get_item("token").ok())
                                                        .flatten()
                                                    {
                                                        let form_data = web_sys::FormData::new().unwrap();
                                                        form_data.append_with_blob("image", &file).unwrap();
                                                        form_data.append_with_str("message", &(*test_message)).unwrap();
                                                        
                                                        match Request::post(&format!("{}/api/admin/test-sms-with-image", config::get_backend_url()))
                                                            .header("Authorization", &format!("Bearer {}", token))
                                                            .body(form_data)
                                                            .send()
                                                            .await
                                                        {
                                                            Ok(response) => {
                                                                if response.ok() {
                                                                    match response.json::<serde_json::Value>().await {
                                                                        Ok(data) => {
                                                                            let mut new_messages = (*chat_messages).clone();
                                                                            
                                                                            // Add user message with image
                                                                            if let Some(image_url) = data.get("image_url").and_then(|u| u.as_str()) {
                                                                                new_messages.push(ChatMessage {
                                                                                    content: (*test_message).clone(),
                                                                                    is_user: true,
                                                                                    timestamp: chrono::Utc::now().timestamp(),
                                                                                    image_url: Some(image_url.to_string()),
                                                                                });
                                                                            }
                                                                            
                                                                            // Add AI response
                                                                            if let Some(reply) = data.get("message").and_then(|m| m.as_str()) {
                                                                                new_messages.push(ChatMessage {
                                                                                    content: reply.to_string(),
                                                                                    is_user: false,
                                                                                    timestamp: chrono::Utc::now().timestamp(),
                                                                                    image_url: None,
                                                                                });
                                                                            }
                                                                            
                                                                            chat_messages.set(new_messages);
                                                                            test_message.set(String::new());
                                                                        }
                                                                        Err(_) => {
                                                                            error.set(Some("Failed to parse response".to_string()));
                                                                        }
                                                                    }
                                                                } else {
                                                                    error.set(Some("Failed to process test message with image".to_string()));
                                                                }
                                                            }
                                                            Err(_) => {
                                                                error.set(Some("Failed to send test message with image".to_string()));
                                                            }
                                                        }
                                                    }
                                                });
                                            }
                                        }
                                    })
                                }}
                            />
                            <label for="image-upload" class="image-upload-label">
                                <i class="fas fa-image"></i>
                            </label>
                            </div>
                            <input
                                type="text"
                                value={(*test_message).clone()}
                                onchange={{
                                    let test_message = test_message.clone();
                                    Callback::from(move |e: Event| {
                                        let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                                        test_message.set(input.value());
                                    })
                                }}
                                onkeypress={{
                                    let test_message = test_message.clone();
                                    let chat_messages = chat_messages.clone();
                                    let error = error.clone();
                                    let image_preview_handle = image_preview.clone();
                                    
                                    Callback::from(move |e: KeyboardEvent| {
                                        if e.key() == "Enter" {
                                            let test_message = test_message.clone();
                                            let chat_messages = chat_messages.clone();
                                            let error = error.clone();
                                            let message_content = (*test_message).clone();
                                            let image_preview_data = (*image_preview_handle).data_url.clone();
                                            let image_preview = image_preview_handle.clone();
                                            
                                            if !message_content.is_empty() {
                                                wasm_bindgen_futures::spawn_local(async move {
                                                    if let Some(token) = window()
                                                        .and_then(|w| w.local_storage().ok())
                                                        .flatten()
                                                        .and_then(|storage| storage.get_item("token").ok())
                                                        .flatten()
                                                    {
                                                        // Add user message to chat
                                                                                                let mut new_messages = (*chat_messages).clone();
                                                                                                new_messages.push(ChatMessage {
                                                                                                    content: message_content.clone(),
                                                                                                    is_user: true,
                                                                                                    timestamp: chrono::Utc::now().timestamp(),
                                                                                                    image_url: image_preview_data,
                                                                                                });
                                                                                                chat_messages.set(new_messages);
                                                                                                // Clear the image preview after sending
                                                                                                image_preview.set(ImagePreview { data_url: None });
                                                        
                                                        // Clear input
                                                        test_message.set(String::new());
                                                        
                                                        // Send test message
                                                        match Request::post(&format!("{}/api/admin/test-sms", config::get_backend_url()))
                                                            .header("Authorization", &format!("Bearer {}", token))
                                                            .json(&json!({
                                                                "message": message_content,
                                                                "user_id": 1
                                                            }))
                                                            .unwrap()
                                                            .send()
                                                            .await
                                                        {
                                                            Ok(response) => {
                                                                if response.ok() {
                                                                                                    match response.json::<serde_json::Value>().await {
                                                                                                        Ok(data) => {
                                                                                                            if let Some(reply) = data.get("message").and_then(|m| m.as_str()) {
                                                                                                                let mut new_messages = (*chat_messages).clone();
                                                                new_messages.push(ChatMessage {
                                                                    content: reply.to_string(),
                                                                    is_user: false,
                                                                    timestamp: chrono::Utc::now().timestamp(),
                                                                    image_url: None,
                                                                });
                                                                                                                chat_messages.set(new_messages);
                                                                                                                // Clear the image preview after successful response
                                                                                                                image_preview.set(ImagePreview { data_url: None });
                                                                                                            }
                                                                        }
                                                                        Err(_) => {
                                                                            error.set(Some("Failed to parse response".to_string()));
                                                                        }
                                                                    }
                                                                } else {
                                                                    error.set(Some("Failed to process test message".to_string()));
                                                                }
                                                            }
                                                            Err(_) => {
                                                                error.set(Some("Failed to send test message".to_string()));
                                                            }
                                                        }
                                                    }
                                                });
                                            }
                                        }
                                    })
                                }}
                                placeholder="Type a message and press Enter..."
                                class="chat-input-field"
                            />
                        </div>
                    </div>
                </div>

                // Usage Logs Section
                <div class="filter-section">
                    <h3>{"Usage Logs"}</h3>
                    <div class="usage-filter">
                        <button 
                            class={classes!(
                                "filter-button",
                                (activity_filter.is_none()).then_some("active")
                            )}
                            onclick={
                                let activity_filter = activity_filter.clone();
                                Callback::from(move |_| activity_filter.set(None))
                            }
                        >
                            {"All"}
                        </button>
                        <button 
                            class={classes!(
                                "filter-button",
                                (activity_filter.as_deref() == Some("sms")).then_some("active")
                            )}
                            onclick={
                                let activity_filter = activity_filter.clone();
                                Callback::from(move |_| activity_filter.set(Some("sms".to_string())))
                            }
                        >
                            {"SMS"}
                        </button>
                        <button 
                            class={classes!(
                                "filter-button",
                                (activity_filter.as_deref() == Some("call")).then_some("active")
                            )}
                            onclick={
                                let activity_filter = activity_filter.clone();
                                Callback::from(move |_| activity_filter.set(Some("call".to_string())))
                            }
                        >
                            {"Calls"}
                        </button>
                        <button 
                            class={classes!(
                                "filter-button",
                                (activity_filter.as_deref() == Some("failed")).then_some("active")
                            )}
                            onclick={
                                let activity_filter = activity_filter.clone();
                                Callback::from(move |_| activity_filter.set(Some("failed".to_string())))
                            }
                        >
                            {"Failed"}
                        </button>
                    </div>

                    <div class="usage-logs">
                        {
                            (*usage_logs).iter()
                                .filter(|log| {
                    if let Some(filter) = (*activity_filter).as_ref() {
                        match filter.as_str() {
                            "failed" => !log.success.unwrap_or(true),
                            _ => log.activity_type == *filter
                        }
                    } else {
                        true
                    }
                                })
                                .map(|log| {
                                    html! {
                                        <div class={classes!("usage-log-item", log.activity_type.clone())}>
                                                <div class="usage-log-header">
                                                    <span class="usage-type">{&log.activity_type}</span>
                                                    <span class="usage-date">
                                                        {
                                                            // Format timestamp with date and time
                                                            if let Some(dt) = Utc.timestamp_opt(log.timestamp as i64, 0).single() {
                                                                dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
                                                            } else {
                                                                "Invalid timestamp".to_string()
                                                            }
                                                        }
                                                    </span>
                                                </div>
                                                <div class="usage-details">
                                                    {
                                                        if let Some(status) = &log.status {
                                                            html! {
                                                                <div>
                                                                    <span class="label">{"Status"}</span>
                                                                    <span class={classes!("value", if log.success.unwrap_or(false) { "success" } else { "failure" })}>
                                                                        {status}
                                                                    </span>
                                                                </div>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }
                                                    }
                                                    {
                                                        // Add success field display
                                                        if let Some(success) = log.success {
                                                            html! {
                                                                <div>
                                                                    <span class="label">{"Success"}</span>
                                                                    <span class={classes!("value", if success { "success" } else { "failure" })}>
                                                                        {if success { "Yes" } else { "No" }}
                                                                    </span>
                                                                </div>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }
                                                    }

                                                    {
                                                        if let Some(credits) = log.credits {
                                                            html! {
                                                                <div>
                                                                    <span class="label">{"Credits Used"}</span>
                                                                    <span class="value">{format!("{:.2}€", credits)}</span>
                                                                </div>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }

                                                    }
                                                    {
                                                        if let Some(time) = log.time_consumed {
                                                            html! {
                                                                <div>
                                                                    <span class="label">{"Duration"}</span>
                                                                    <span class="value">{format!("{}s", time)}</span>
                                                                </div>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }

                                                    }
                                                    {
                                                        if let Some(reason) = &log.reason {
                                                            html! {
                                                                <div class="usage-reason">
                                                                    <span class="label">{"Reason"}</span>
                                                                    <span class="value">{reason}</span>
                                                                </div>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }

                                                    }
                                                    {
                                                        if let Some(sid) = &log.sid {
                                                            html! {
                                                                <div class="usage-sid">
                                                                    <span class="label">{"SID"}</span>
                                                                    <span class="value">{sid}</span>
                                                                </div>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }

                                                    }
                                                    {
                                                        if let Some(threshold) = log.recharge_threshold_timestamp {
                                                            html! {
                                                                <div>
                                                                    <span class="label">{"Recharge Threshold"}</span>
                                                                    <span class="value">{format_timestamp(threshold)}</span>
                                                                </div>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }

                                                    }
                                                    {
                                                        if let Some(zero) = log.zero_credits_timestamp {
                                                            html! {
                                                                <div>
                                                                    <span class="label">{"Zero Credits At"}</span>
                                                                    <span class="value">{format_timestamp(zero)}</span>
                                                                </div>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }

                                                    }
                                                </div>
                                        </div>
                                    }
                                })
                                .collect::<Html>()
                        }
                    </div>
                </div>


                {
                    if let Some(error_msg) = (*error).as_ref() {
                        html! {
                            <div class="info-section error">
                                <span class="error-message">{error_msg}</span>
                            </div>
                        }
                    } else {
                        html! {
                            <div class="info-section">
                                <h2 class="section-title">{"Users List"}</h2>
                                <div class="users-table-container">
                                    <table class="users-table">
                                        <thead>
                                            <tr>
                                                <th>{"ID"}</th>
                                                <th>{"Email"}</th>
                                                <th>{"Phone"}</th>
                                                <th>{"Overage Credits"}</th>
                                                <th>{"Monthly Credits"}</th>
                                                <th>{"Notifications Left"}</th>
                                                <th>{"Tier"}</th>
                                                <th>{"Verified"}</th>
                                                <th>{"Notify"}</th>
                                                <th>{"Discount"}</th>
                                                <th>{"Joined"}</th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {
                                                users.iter().map(|user| {
                                                    let is_selected = selected_user_id.as_ref() == Some(&user.id);
                                                    let user_id = user.id;
                                                    let onclick = toggle_user_details.reform(move |_| user_id);
                                                    
                                                    html! {
                                                        <>
                                                            <tr onclick={onclick} key={user.id} class={classes!(
                                                                "user-row",
                                                                is_selected.then(|| "selected"),
                                                                match user.sub_tier.as_deref() {
                                                                    Some("tier 2") => "gold-user",
                                                                    Some("tier 1") => "silver-user",
                                                                    _ => ""
                                                                }
                                                            )}>
                                                                <td>{user.id}</td>
                                                                <td>
                                                                    <div class="user-email-container">
                                                                        {&user.email}
                                                                        {
                                                                            match user.sub_tier.as_deref() {
                                                                                Some("tier 2") => html! {
                                                                                    <span class="gold-badge">{"★"}</span>
                                                                                },
                                                                                Some("tier 1") => html! {
                                                                                    <span class="silver-badge">{"★"}</span>
                                                                                },
                                                                                _ => html! {}
                                                                            }
                                                                        }
                                                                        {
                                                                            match user.discount_tier.as_deref() {
                                                                                Some("msg") => html! {
                                                                                    <span class="discount-badge msg">{"msg✦"}</span>
                                                                                },
                                                                                Some("voice") => html! {
                                                                                    <span class="discount-badge voice">{"voice✧"}</span>
                                                                                },
                                                                                Some("full") => html! {
                                                                                    <span class="discount-badge full">{"full✶"}</span>
                                                                                },
                                                                                _ => html! {}
                                                                            }
                                                                        }
                                                                    </div>
                                                                </td>
                                                                <td>{&user.phone_number}</td>
                                                                <td>{format!("{:.2}€", user.credits)}</td>
                                                                <td>{format!("{:.2}€", user.credits_left)}</td>
                                                                <td>{user.msgs_left}</td>
                                                                <td>
                                                                    <span class={classes!(
                                                                        "tier-badge",
                                                                        match user.sub_tier.as_deref() {
                                                                            Some("tier 2") => "gold",
                                                                            Some("tier 1") => "silver",
                                                                            _ => "none"
                                                                        }
                                                                    )}>
                                                                        {user.sub_tier.clone().unwrap_or_else(|| "None".to_string())}
                                                                    </span>
                                                                </td>
                                                                <td>
                                                                    <span class={classes!(
                                                                        "status-badge",


                                                                        if user.discount { "enabled" } else { "disabled" },
                                                                        match user.discount_tier.as_deref() {
                                                                            Some("msg") => "discount-msg",
                                                                            Some("voice") => "discount-voice",
                                                                            Some("full") => "discount-full",
                                                                            _ => "disabled"
                                                                        }
                                                                    )}>
                                                                                                                                                                                                {if user.discount { "Yes" } else { "No" }}

                                                                        {match user.discount_tier.as_deref() {
                                                                            Some("msg") => "MSG",
                                                                            Some("voice") => "Voice",
                                                                            Some("full") => "Full",
                                                                            _ => "None"
                                                                        }}
                                                                    </span>
                                                                </td>
                                                                <td>
                                                                    <span class={classes!(
                                                                        "status-badge",
                                                                        if user.verified { "verified" } else { "unverified" }
                                                                    )}>
                                                                        {if user.verified { "Yes" } else { "No" }}
                                                                    </span>
                                                                </td>
                                                                <td>
                                                                    <span class={classes!(
                                                                        "status-badge",
                                                                        if user.notify { "enabled" } else { "disabled" }
                                                                    )}>
                                                                        {if user.notify { "Yes" } else { "No" }}
                                                                    </span>
                                                                </td>
                                                                <td>
                                                                    {
                                                                        user.time_to_live.map_or("N/A".to_string(), |ttl| {
                                                                            Utc.timestamp_opt(ttl as i64, 0)
                                                                                .single()
                                                                                .map(|dt| dt.format("%Y-%m-%d").to_string())
                                                                                .unwrap_or_else(|| "Invalid".to_string())
                                                                        })
                                                                    }
                                                                </td>
                                                            </tr>
                                                            if is_selected {
                                                                <tr class="details-row">
                                                                    <td colspan="4">
                                                                        <div class="user-details">
                                                                            <div class="preferred-number-section">
                                                                                <p><strong>{"Current Preferred Number: "}</strong>{user.preferred_number.clone().unwrap_or_else(|| "Not set".to_string())}</p>
                                                                            </div>
                                                                            
                                                                        <button 
                                                                            onclick={{
                                                                                let users = users.clone();
                                                                                let error = error.clone();
                                                                                let user_id = user.id;
                                                                                Callback::from(move |_| {
                                                                                    let users = users.clone();
                                                                                    let error = error.clone();
                                                                                    wasm_bindgen_futures::spawn_local(async move {
                                                                                        if let Some(token) = window()
                                                                                            .and_then(|w| w.local_storage().ok())
                                                                                            .flatten()
                                                                                            .and_then(|storage| storage.get_item("token").ok())
                                                                                            .flatten()
                                                                                        {
                                                                                            match Request::post(&format!("{}/api/billing/increase-credits/{}", config::get_backend_url(), user_id))
                                                                                                .header("Authorization", &format!("Bearer {}", token))
                                                                                                .send()
                                                                                                .await
                                                                                            {
                                                                                                Ok(response) => {
                                                                                                    if response.ok() {
                                                                                                        // Refresh the users list after increasing credits 
                                                                                                        if let Ok(response) = Request::get(&format!("{}/api/admin/users", config::get_backend_url()))
                                                                                                            .header("Authorization", &format!("Bearer {}", token))
                                                                                                            .send()
                                                                                                            .await
                                                                                                        {
                                                                                                            if let Ok(updated_users) = response.json::<Vec<UserInfo>>().await {
                                                                                                                users.set(updated_users);
                                                                                                            }
                                                                                                        }
                                                                                                    } else {
                                                                                                        error.set(Some("Failed to increase Credits".to_string()));
                                                                                                    }
                                                                                                }
                                                                                                Err(_) => {
                                                                                                    error.set(Some("Failed to send request".to_string()));
                                                                                                }
                                                                                            }
                                                                                        }
                                                                                    });
                                                                                })
                                                                            }}
                                                                            class="iq-button"
                                                                        >
                                                                            {"Add 1€ credits"}
                                                                        </button>
                                                                        <button 
                                                                            onclick={{
                                                                                let users = users.clone();
                                                                                let error = error.clone();
                                                                                let user_id = user.id;
                                                                                Callback::from(move |_| {
                                                                                    let users = users.clone();
                                                                                    let error = error.clone();
                                                                                    wasm_bindgen_futures::spawn_local(async move {
                                                                                        if let Some(token) = window()
                                                                                            .and_then(|w| w.local_storage().ok())
                                                                                            .flatten()
                                                                                            .and_then(|storage| storage.get_item("token").ok())
                                                                                            .flatten()
                                                                                        {
                                                                                            match Request::post(&format!("{}/api/billing/reset-credits/{}", config::get_backend_url(), user_id))
                                                                                                .header("Authorization", &format!("Bearer {}", token))
                                                                                                .send()
                                                                                                .await
                                                                                            {
                                                                                                Ok(response) => {
                                                                                                    if response.ok() {
                                                                                                        // Refresh the users list after resetting credits
                                                                                                        if let Ok(response) = Request::get(&format!("{}/api/admin/users", config::get_backend_url()))
                                                                                                            .header("Authorization", &format!("Bearer {}", token))
                                                                                                            .send()
                                                                                                            .await
                                                                                                        {
                                                                                                            if let Ok(updated_users) = response.json::<Vec<UserInfo>>().await {
                                                                                                                users.set(updated_users);
                                                                                                            }
                                                                                                        }
                                                                                                    } else {
                                                                                                        error.set(Some("Failed to reset credits".to_string()));
                                                                                                    }
                                                                                                }
                                                                                                Err(_) => {
                                                                                                    error.set(Some("Failed to send request".to_string()));
                                                                                                }
                                                                                            }
                                                                                        }
                                                                                    });
                                                                                })
                                                                            }}
                                                                            class="iq-button reset"
                                                                        >
                                                                            {"Reset credits"}
                                                                        </button>
                                                                        {
                                                                            if !user.verified {
                                                                                html! {
                                                                                    <button 
                                                                                        onclick={{
                                                                                            let users = users.clone();
                                                                                            let error = error.clone();
                                                                                            let user_id = user.id;
                                                                                            Callback::from(move |_| {
                                                                                                let users = users.clone();
                                                                                                let error = error.clone();
                                                                                                wasm_bindgen_futures::spawn_local(async move {
                                                                                                    if let Some(token) = window()
                                                                                                        .and_then(|w| w.local_storage().ok())
                                                                                                        .flatten()
                                                                                                        .and_then(|storage| storage.get_item("token").ok())
                                                                                                        .flatten()
                                                                                                    {
                                                                                                        match Request::post(&format!("{}/api/admin/verify/{}", config::get_backend_url(), user_id))
                                                                                                            .header("Authorization", &format!("Bearer {}", token))
                                                                                                            .send()
                                                                                                            .await
                                                                                                        {
                                                                                                            Ok(response) => {
                                                                                                                if response.ok() {
                                                                                                                    // Refresh the users list after verifying
                                                                                                                    if let Ok(response) = Request::get(&format!("{}/api/admin/users", config::get_backend_url()))
                                                                                                                        .header("Authorization", &format!("Bearer {}", token))
                                                                                                                        .send()
                                                                                                                        .await
                                                                                                                    {
                                                                                                                        if let Ok(updated_users) = response.json::<Vec<UserInfo>>().await {
                                                                                                                            users.set(updated_users);
                                                                                                                        }
                                                                                                                    }
                                                                                                                } else {
                                                                                                                    error.set(Some("Failed to verify user".to_string()));
                                                                                                                }
                                                                                                            }
                                                                                                            Err(_) => {
                                                                                                                error.set(Some("Failed to send verification request".to_string()));
                                                                                                            }
                                                                                                        }

                                                                                                    }
                                                                                                });
                                                                                            })
                                                                                        }}
                                                                                        class="iq-button"
                                                                                    >
                                                                                        {"Verify User"}
                                                                                    </button>
                                                                                }
                                                                            } else {
                                                                                html! {}
                                                                            }
                                                                        }
                                                                        <button 
                                                                            onclick={{
                                                                                let users = users.clone();
                                                                                let error = error.clone();
                                                                                let user_id = user.id;
                                                                                Callback::from(move |_| {
                                                                                    let users = users.clone();
                                                                                    let error = error.clone();
                                                                                    wasm_bindgen_futures::spawn_local(async move {
                                                                                        if let Some(token) = window()
                                                                                            .and_then(|w| w.local_storage().ok())
                                                                                            .flatten()
                                                                                            .and_then(|storage| storage.get_item("token").ok())
                                                                                            .flatten()
                                                                                        {
                                                                                            match Request::post(&format!("{}/api/admin/set-preferred-number-default/{}", config::get_backend_url(), user_id))
                                                                                                .header("Authorization", &format!("Bearer {}", token))
                                                                                                .send()
                                                                                                .await
                                                                                            {
                                                                                                Ok(response) => {
                                                                                                    if response.ok() {
                                                                                                        // Refresh the users list after setting preferred number
                                                                                                        if let Ok(response) = Request::get(&format!("{}/api/admin/users", config::get_backend_url()))
                                                                                                            .header("Authorization", &format!("Bearer {}", token))
                                                                                                            .send()
                                                                                                            .await
                                                                                                        {
                                                                                                            if let Ok(updated_users) = response.json::<Vec<UserInfo>>().await {
                                                                                                                users.set(updated_users);
                                                                                                                error.set(None);
                                                                                                            }
                                                                                                        }
                                                                                                    } else {
                                                                                                        error.set(Some("Failed to set preferred number".to_string()));
                                                                                                    }
                                                                                                }
                                                                                                Err(_) => {
                                                                                                    error.set(Some("Failed to send request".to_string()));
                                                                                                }
                                                                                            }
                                                                                        }
                                                                                    });
                                                                                })
                                                                            }}
                                                                            class="iq-button"
                                                                        >
                                                                            {"Set Default Number"}
                                                                        </button>
                                                                        <button 
                                                                            onclick={{
                                                                                let users = users.clone();
                                                                                let error = error.clone();
                                                                                let user_id = user.id;
                                                                                let current_discount_tier = user.discount_tier.clone();
                                                                                Callback::from(move |_| {
                                                                                    let users = users.clone();
                                                                                    let error = error.clone();
                                                                                    let new_tier = match current_discount_tier.as_deref() {
                                                                                        None => "msg",
                                                                                        Some("msg") => "voice",
                                                                                        Some("voice") => "full",
                                                                                        Some("full") | _ => "none",
                                                                                    };
                                                                                    
                                                                                    wasm_bindgen_futures::spawn_local(async move {
                                                                                        if let Some(token) = window()
                                                                                            .and_then(|w| w.local_storage().ok())
                                                                                            .flatten()
                                                                                            .and_then(|storage| storage.get_item("token").ok())
                                                                                            .flatten()
                                                                                        {
                                                                                            match Request::post(&format!("{}/api/admin/discount-tier/{}/{}", config::get_backend_url(), user_id, new_tier))
                                                                                                .header("Authorization", &format!("Bearer {}", token))
                                                                                                .send()
                                                                                                .await
                                                                                            {
                                                                                                Ok(response) => {
                                                                                                    if response.ok() {
                                                                                                        // Refresh the users list
                                                                                                        if let Ok(response) = Request::get(&format!("{}/api/admin/users", config::get_backend_url()))
                                                                                                            .header("Authorization", &format!("Bearer {}", token))
                                                                                                            .send()
                                                                                                            .await
                                                                                                        {
                                                                                                            if let Ok(updated_users) = response.json::<Vec<UserInfo>>().await {
                                                                                                                users.set(updated_users);
                                                                                                            }
                                                                                                        }
                                                                                                    } else {
                                                                                                        error.set(Some("Failed to update discount tier".to_string()));
                                                                                                    }
                                                                                                }
                                                                                                Err(_) => {
                                                                                                    error.set(Some("Failed to send request".to_string()));
                                                                                                }
                                                                                            }
                                                                                        }
                                                                                    });
                                                                                })
                                                                            }}
                                                                            class="iq-button discount-tier"
                                                                        >
                                                                            {match user.discount_tier.as_deref() {
                                                                                None => "Set MSG Discount",
                                                                                Some("msg") => "Set Voice Discount",
                                                                                Some("voice") => "Set Full Discount",
                                                                                Some("full") => "Remove Discount",
                                                                                _ => "Set MSG Discount",
                                                                            }}
                                                                        </button>
                                                                        <button 
                                                                            onclick={{
                                                                                let users = users.clone();
                                                                                let error = error.clone();
                                                                                let user_id = user.id;
                                                                                let current_tier = user.sub_tier.clone();
                                                                                Callback::from(move |_| {
                                                                                    let users = users.clone();
                                                                                    let error = error.clone();
                                                                                let new_tier = match current_tier.as_deref() {
                                                                                    None => "tier 2",
                                                                                    Some("tier 2") => "tier 1",
                                                                                    Some("tier 1") => "tier 0",
                                                                                    _ => "tier 0"
                                                                                };
                                                                                    
                                                                                    wasm_bindgen_futures::spawn_local(async move {
                                                                                        if let Some(token) = window()
                                                                                            .and_then(|w| w.local_storage().ok())
                                                                                            .flatten()
                                                                                            .and_then(|storage| storage.get_item("token").ok())
                                                                                            .flatten()
                                                                                        {
match Request::post(&format!("{}/api/admin/subscription/{}/{}", config::get_backend_url(), user_id, urlencoding::encode(new_tier).trim_end_matches('/')))
                                                                                                .header("Authorization", &format!("Bearer {}", token))
                                                                                                .send()
                                                                                                .await
                                                                                            {
                                                                                                Ok(response) => {
                                                                                                    if response.ok() {
                                                                                                        // Refresh the users list
                                                                                                        if let Ok(response) = Request::get(&format!("{}/api/admin/users", config::get_backend_url()))
                                                                                                            .header("Authorization", &format!("Bearer {}", token))
                                                                                                            .send()
                                                                                                            .await
                                                                                                        {
                                                                                                            if let Ok(updated_users) = response.json::<Vec<UserInfo>>().await {
                                                                                                                users.set(updated_users);

                                                                                                            }
                                                                                                        }
                                                                                                    } else {
                                                                                                        error.set(Some("Failed to update subscription tier".to_string()));
                                                                                                    }
                                                                                                }
                                                                                                Err(_) => {
                                                                                                    error.set(Some("Failed to send request".to_string()));
                                                                                                }
                                                                                            }
                                                                                        }
                                                                                    });
                                                                                })
                                                                            }}
                                                                            class="iq-button"
                                                                        >
                                                                            {match user.sub_tier.as_deref() {
                                                                                None => "Set Tier 2",
                                                                                Some("tier 2") => "Set Tier 1",
                                                                                Some("tier 1") => "Remove Subscription",
                                                                                _ => "Set Tier 2"
                                                                            }}
                                                                        </button>
                                                                        <button 
                                                                            onclick={{
                                                                                let users = users.clone();
                                                                                let error = error.clone();
                                                                                let user_id = user.id;
                                                                                Callback::from(move |_| {
                                                                                    let users = users.clone();
                                                                                    let error = error.clone();
                                                                                    wasm_bindgen_futures::spawn_local(async move {
                                                                                        if let Some(token) = window()
                                                                                            .and_then(|w| w.local_storage().ok())
                                                                                            .flatten()
                                                                                            .and_then(|storage| storage.get_item("token").ok())
                                                                                            .flatten()
                                                                                        {
                                                                                            match Request::post(&format!("{}/api/admin/messages/{}/10", config::get_backend_url(), user_id))
                                                                                                .header("Authorization", &format!("Bearer {}", token))
                                                                                                .send()
                                                                                                .await
                                                                                            {
                                                                                                Ok(response) => {
                                                                                                    if response.ok() {
                                                                                                        // Refresh the users list
                                                                                                        if let Ok(response) = Request::get(&format!("{}/api/admin/users", config::get_backend_url()))
                                                                                                            .header("Authorization", &format!("Bearer {}", token))
                                                                                                            .send()
                                                                                                            .await
                                                                                                        {
                                                                                                            if let Ok(updated_users) = response.json::<Vec<UserInfo>>().await {
                                                                                                                users.set(updated_users);
                                                                                                            }
                                                                                                        }
                                                                                                    } else {
                                                                                                        error.set(Some("Failed to add messages".to_string()));
                                                                                                    }
                                                                                                }
                                                                                                Err(_) => {
                                                                                                    error.set(Some("Failed to send request".to_string()));
                                                                                                }
                                                                                            }
                                                                                        }
                                                                                    });
                                                                                })
                                                                            }}
                                                                            class="iq-button"
                                                                        >
                                                                            {"Add 10 Messages"}
                                                                        </button>
                                                                        <button 
                                                                            onclick={{
                                                                                let users = users.clone();
                                                                                let error = error.clone();
                                                                                let user_id = user.id;
                                                                                Callback::from(move |_| {
                                                                                    let users = users.clone();
                                                                                    let error = error.clone();
                                                                                    wasm_bindgen_futures::spawn_local(async move {
                                                                                        if let Some(token) = window()
                                                                                            .and_then(|w| w.local_storage().ok())
                                                                                            .flatten()
                                                                                            .and_then(|storage| storage.get_item("token").ok())
                                                                                            .flatten()
                                                                                        {
                                                                                            match Request::post(&format!("{}/api/admin/messages/{}/{}", config::get_backend_url(), user_id, -10))
                                                                                                .header("Authorization", &format!("Bearer {}", token))
                                                                                                .send()
                                                                                                .await
                                                                                            {
                                                                                                Ok(response) => {
                                                                                                    if response.ok() {
                                                                                                        // Refresh the users list
                                                                                                        if let Ok(response) = Request::get(&format!("{}/api/admin/users", config::get_backend_url()))
                                                                                                            .header("Authorization", &format!("Bearer {}", token))
                                                                                                            .send()
                                                                                                            .await
                                                                                                        {
                                                                                                            if let Ok(updated_users) = response.json::<Vec<UserInfo>>().await {
                                                                                                                users.set(updated_users);
                                                                                                            }
                                                                                                        }
                                                                                                    } else {
                                                                                                        error.set(Some("Failed to remove messages".to_string()));
                                                                                                    }
                                                                                                }
                                                                                                Err(_) => {
                                                                                                    error.set(Some("Failed to send request".to_string()));
                                                                                                }
                                                                                            }
                                                                                        }
                                                                                    });
                                                                                })
                                                                            }}
                                                                            class="iq-button reset"
                                                                        >
                                                                            {"Remove 10 Messages"}
                                                                        </button>
                                                                        <button 
                                                                            onclick={{
                                                                                let users = users.clone();
                                                                                let error = error.clone();
                                                                                let user_id = user.id;
                                                                                Callback::from(move |_| {
                                                                                    let users = users.clone();
                                                                                    let error = error.clone();
                                                                                    wasm_bindgen_futures::spawn_local(async move {
                                                                                        if let Some(token) = window()
                                                                                            .and_then(|w| w.local_storage().ok())
                                                                                            .flatten()
                                                                                            .and_then(|storage| storage.get_item("token").ok())
                                                                                            .flatten()
                                                                                        {
                                                                                            match Request::post(&format!("{}/api/admin/monthly-credits/{}/{}", config::get_backend_url(), user_id, 10.0))
                                                                                                .header("Authorization", &format!("Bearer {}", token))
                                                                                                .send()
                                                                                                .await
                                                                                            {
                                                                                                Ok(response) => {
                                                                                                    if response.ok() {
                                                                                                        // Refresh the users list
                                                                                                        if let Ok(response) = Request::get(&format!("{}/api/admin/users", config::get_backend_url()))
                                                                                                            .header("Authorization", &format!("Bearer {}", token))
                                                                                                            .send()
                                                                                                            .await
                                                                                                        {
                                                                                                            if let Ok(updated_users) = response.json::<Vec<UserInfo>>().await {
                                                                                                                users.set(updated_users);
                                                                                                            }
                                                                                                        }
                                                                                                    } else {
                                                                                                        error.set(Some("Failed to add monthly credits".to_string()));
                                                                                                    }
                                                                                                }
                                                                                                Err(_) => {
                                                                                                    error.set(Some("Failed to send request".to_string()));
                                                                                                }
                                                                                            }
                                                                                        }
                                                                                    });
                                                                                })
                                                                            }}
                                                                            class="iq-button"
                                                                        >
                                                                            {"Add 10€ Monthly Credits"}
                                                                        </button>
                                                                        <button 
                                                                            onclick={{
                                                                                let users = users.clone();
                                                                                let error = error.clone();
                                                                                let user_id = user.id;
                                                                                Callback::from(move |_| {
                                                                                    let users = users.clone();
                                                                                    let error = error.clone();
                                                                                    wasm_bindgen_futures::spawn_local(async move {
                                                                                        if let Some(token) = window()
                                                                                            .and_then(|w| w.local_storage().ok())
                                                                                            .flatten()
                                                                                            .and_then(|storage| storage.get_item("token").ok())
                                                                                            .flatten()
                                                                                        {
                                                                                            match Request::post(&format!("{}/api/admin/monthly-credits/{}/{}", config::get_backend_url(), user_id, -10.0))
                                                                                                .header("Authorization", &format!("Bearer {}", token))
                                                                                                .send()
                                                                                                .await
                                                                                            {
                                                                                                Ok(response) => {
                                                                                                    if response.ok() {
                                                                                                        // Refresh the users list
                                                                                                        if let Ok(response) = Request::get(&format!("{}/api/admin/users", config::get_backend_url()))
                                                                                                            .header("Authorization", &format!("Bearer {}", token))
                                                                                                            .send()
                                                                                                            .await
                                                                                                        {
                                                                                                            if let Ok(updated_users) = response.json::<Vec<UserInfo>>().await {
                                                                                                                users.set(updated_users);
                                                                                                            }
                                                                                                        }
                                                                                                    } else {
                                                                                                        error.set(Some("Failed to remove monthly credits".to_string()));
                                                                                                    }
                                                                                                }
                                                                                                Err(_) => {
                                                                                                    error.set(Some("Failed to send request".to_string()));
                                                                                                }
                                                                                            }
                                                                                        }
                                                                                    });
                                                                                })
                                                                            }}
                                                                            class="iq-button reset"
                                                                        >
                                                                            {"Remove 10€ Monthly Credits"}
                                                                        </button>
                                                                        {
                                                                            // Only show migrate button for users with subscription
                                                                            if user.sub_tier.is_some() {
                                                                                html! {
                                                                                    <button 
                                                                                        onclick={{
                                                                                            let users = users.clone();
                                                                                            let error = error.clone();
                                                                                            let user_id = user.id;
                                                                                            Callback::from(move |_| {
                                                                                                let users = users.clone();
                                                                                                let error = error.clone();
                                                                                                wasm_bindgen_futures::spawn_local(async move {
                                                                                                    if let Some(token) = window()
                                                                                                        .and_then(|w| w.local_storage().ok())
                                                                                                        .flatten()
                                                                                                        .and_then(|storage| storage.get_item("token").ok())
                                                                                                        .flatten()
                                                                                                    {
                                                                                                        match Request::post(&format!("{}/api/profile/migrate-to-daily/{}", config::get_backend_url(), user_id))
                                                                                                            .header("Authorization", &format!("Bearer {}", token))
                                                                                                            .send()
                                                                                                            .await
                                                                                                        {
                                                                                                            Ok(response) => {
                                                                                                                if response.ok() {
                                                                                                                    match response.json::<serde_json::Value>().await {
                                                                                                                        Ok(data) => {
                                                                                                                            // Refresh the users list
                                                                                                                            if let Ok(response) = Request::get(&format!("{}/api/admin/users", config::get_backend_url()))
                                                                                                                                .header("Authorization", &format!("Bearer {}", token))
                                                                                                                                .send()
                                                                                                                                .await
                                                                                                                            {
                                                                                                                                if let Ok(updated_users) = response.json::<Vec<UserInfo>>().await {
                                                                                                                                    users.set(updated_users);
                                                                                                                                }
                                                                                                                            }
                                                                                                                            error.set(Some(format!("Successfully migrated user to daily reset plan. Country: {}", 
                                                                                                                                data["country"].as_str().unwrap_or("unknown"))));
                                                                                                                        }
                                                                                                                        Err(_) => {
                                                                                                                            error.set(Some("Failed to parse migration response".to_string()));
                                                                                                                        }
                                                                                                                    }
                                                                                                                } else {
                                                                                                                    error.set(Some("Failed to migrate user to daily reset plan".to_string()));
                                                                                                                }
                                                                                                            }
                                                                                                            Err(_) => {
                                                                                                                error.set(Some("Failed to send migration request".to_string()));
                                                                                                            }
                                                                                                        }
                                                                                                    }
                                                                                                });
                                                                                            })
                                                                                        }}
                                                                                        class="iq-button migrate"
                                                                                    >
                                                                                        {"Migrate to Daily Reset"}
                                                                                    </button>
                                                                                }
                                                                            } else {
                                                                                html! {}
                                                                            }
                                                                        }
                                                                        <button 
                                                                            onclick={{
                                                                                let delete_modal = delete_modal.clone();
                                                                                let user_id = user.id;
                                                                                let user_email = user.email.clone();
                                                                                Callback::from(move |_| {
                                                                                    delete_modal.set(DeleteModalState {
                                                                                        show: true,
                                                                                        user_id: Some(user_id),
                                                                                        user_email: Some(user_email.clone()),
                                                                                    });
                                                                                })
                                                                            }}
                                                                            class="iq-button delete"
                                                                        >
                                                                            {"Delete User"}
                                                                        </button>

                                                                        </div>
                                                                    </td>
                                                                </tr>
                                                            }
                                                        </>
                                                    }
                                                }).collect::<Html>()
                                            }
                                        </tbody>
                                    </table>
                                </div>
            {
                if (*delete_modal).show {
                    html! {
                        <div class="modal-overlay">
                            <div class="modal-content">
                                <h2>{"Confirm Delete"}</h2>
                                <p>{format!("Are you sure you want to delete user {}?", delete_modal.user_email.clone().unwrap_or_default())}</p>
                                <p class="warning">{"This action cannot be undone!"}</p>
                                <div class="modal-buttons">
                                    <button 
                                        onclick={{
                                            let delete_modal = delete_modal.clone();
                                            Callback::from(move |_| {
                                                delete_modal.set(DeleteModalState {
                                                    show: false,
                                                    user_id: None,
                                                    user_email: None,
                                                });
                                            })
                                        }}
                                        class="modal-button cancel"
                                    >
                                        {"Cancel"}
                                    </button>
                                    <button 
                                        onclick={{
                                            let delete_modal = delete_modal.clone();
                                            let users = users.clone();
                                            let error = error.clone();
                                            Callback::from(move |_| {
                                                let users = users.clone();
                                                let error = error.clone();
                                                let delete_modal = delete_modal.clone();
                                                let user_id = delete_modal.user_id.unwrap();
                                                
                                                wasm_bindgen_futures::spawn_local(async move {
                                                    if let Some(token) = window()
                                                        .and_then(|w| w.local_storage().ok())
                                                        .flatten()
                                                        .and_then(|storage| storage.get_item("token").ok())
                                                        .flatten()
                                                    {
                                                        match Request::delete(&format!("{}/api/profile/delete/{}", config::get_backend_url(), user_id))
                                                            .header("Authorization", &format!("Bearer {}", token))
                                                            .send()
                                                            .await
                                                        {
                                                            Ok(response) => {
                                                                if response.ok() {
                                                                    // Remove the deleted user from the users list
                                                                    users.set((*users).clone().into_iter().filter(|u| u.id != user_id).collect());
                                                                    delete_modal.set(DeleteModalState {
                                                                        show: false,
                                                                        user_id: None,
                                                                        user_email: None,
                                                                    });
                                                                    error.set(Some("User deleted successfully".to_string()));
                                                                } else {
                                                                    error.set(Some("Failed to delete user".to_string()));
                                                                }
                                                            }
                                                            Err(_) => {
                                                                error.set(Some("Failed to send delete request".to_string()));
                                                            }
                                                        }
                                                    }
                                                });
                                            })
                                        }}
                                        class="modal-button delete"
                                    >
                                        {"Delete"}
                                    </button>
                                </div>
                            </div>

                                                    </div>
                    }
                } else {
                    html! {}
                }
            }
        </div>
    }
}
                }
            </div>
            <style>
                {r#"
                .test-chat-section {
                    margin: 2rem 0;
                    padding: 1rem;
                    background: rgba(30, 30, 30, 0.7);
                    border-radius: 8px;
                }

                .chat-window {
                    display: flex;
                    flex-direction: column;
                    height: 400px;
                    border: 1px solid rgba(30, 144, 255, 0.2);
                    border-radius: 8px;
                    overflow: hidden;
                }

                .chat-messages {
                    flex-grow: 1;
                    overflow-y: auto;
                    padding: 1rem;
                    display: flex;
                    flex-direction: column;
                    gap: 0.5rem;
                }

                .chat-message {
                    max-width: 80%;
                    padding: 0.5rem 1rem;
                    border-radius: 8px;
                    margin: 0.25rem 0;
                }

                .chat-message.user {
                    align-self: flex-end;
                    background: rgba(30, 144, 255, 0.2);
                    color: #fff;
                }

                .chat-message.assistant {
                    align-self: flex-start;
                    background: rgba(76, 175, 80, 0.2);
                    color: #fff;
                }

                .message-timestamp {
                    font-size: 0.7rem;
                    color: #999;
                    margin-top: 0.25rem;
                }

                .chat-input {
                    padding: 1rem;
                    background: rgba(0, 0, 0, 0.2);
                }

                .chat-input-wrapper {
                    display: flex;
                    align-items: center;
                    gap: 0.5rem;
                }

                .image-upload {
                    display: none;
                }

                .image-upload-label {
                    cursor: pointer;
                    padding: 0.5rem;
                    background: rgba(30, 144, 255, 0.2);
                    border-radius: 4px;
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    transition: all 0.3s ease;
                }

                .image-upload-label:hover {
                    background: rgba(30, 144, 255, 0.3);
                }

                .message-image {
                    margin-bottom: 0.5rem;
                }

                .message-image img {
                    max-width: 100%;
                    max-height: 200px;
                    border-radius: 4px;
                }

                .chat-input-field {
                    width: 100%;
                    padding: 0.75rem;
                    border: 1px solid rgba(30, 144, 255, 0.2);
                    border-radius: 4px;
                    background: rgba(0, 0, 0, 0.3);
                    color: #fff;
                }

                .chat-input-field:focus {
                    outline: none;
                    border-color: #1E90FF;
                }

                .image-preview {
                    position: relative;
                    margin-bottom: 1rem;
                    padding: 0.5rem;
                    background: rgba(0, 0, 0, 0.2);
                    border-radius: 8px;
                    display: inline-block;
                }

                .image-preview img {
                    max-width: 200px;
                    max-height: 200px;
                    border-radius: 4px;
                }

                .remove-image {
                    position: absolute;
                    top: 0;
                    right: 0;
                    background: rgba(255, 0, 0, 0.7);
                    color: white;
                    border: none;
                    border-radius: 50%;
                    width: 24px;
                    height: 24px;
                    cursor: pointer;
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    font-size: 16px;
                    margin: 4px;
                }

                .remove-image:hover {
                    background: rgba(255, 0, 0, 0.9);
                }
                "#}
                {r#"
                .judgment-processed {
                    font-size: 0.8rem;
                    color: #666;
                }

                /* Usage Logs Styles */
                .usage-filter {
                    display: flex;
                    gap: 1rem;
                    margin-bottom: 1.5rem;
                }

                .filter-button {
                    padding: 0.5rem 1.5rem;
                    background: rgba(30, 144, 255, 0.1);
                    border: 1px solid rgba(30, 144, 255, 0.2);
                    border-radius: 20px;
                    color: #fff;
                    cursor: pointer;
                    transition: all 0.3s ease;
                }

                .filter-button:hover {
                    background: rgba(30, 144, 255, 0.2);
                }

                .filter-button.active {
                    background: #1E90FF;
                    border-color: #1E90FF;
                }

                .usage-logs {
                    display: flex;
                    flex-direction: column;
                    gap: 1rem;
                    max-height: 500px;
                    overflow-y: auto;
                    padding-right: 0.5rem;
                }

                .usage-log-item {
                    background: rgba(30, 30, 30, 0.7);
                    border: 1px solid rgba(30, 144, 255, 0.1);
                    border-radius: 8px;
                    padding: 1rem;
                    transition: all 0.3s ease;
                }

                .usage-log-item:hover {
                    transform: translateY(-2px);
                    box-shadow: 0 4px 20px rgba(30, 144, 255, 0.1);
                }

                .usage-log-item.sms {
                    border-left: 4px solid #4CAF50;
                }

                .usage-log-item.call {
                    border-left: 4px solid #FF9800;
                }

                .usage-log-header {
                    display: flex;
                    justify-content: space-between;
                    align-items: center;
                    margin-bottom: 1rem;
                }

                .usage-type {
                    font-size: 0.9rem;
                    padding: 0.3rem 0.8rem;
                    border-radius: 12px;
                    font-weight: 500;
                    text-transform: uppercase;
                }

                .usage-log-item.sms .usage-type {
                    background: rgba(76, 175, 80, 0.1);
                    color: #4CAF50;
                }

                .usage-log-item.call .usage-type {
                    background: rgba(255, 152, 0, 0.1);
                    color: #FF9800;
                }

                .usage-date {
                    color: #7EB2FF;
                    font-size: 0.9rem;
                }

                .usage-details {
                    display: grid;
                    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
                    gap: 0.8rem;
                }

                .usage-details > div {
                    display: flex;
                    flex-direction: column;
                    gap: 0.3rem;
                }

                .usage-details .label {
                    color: #999;
                    font-size: 0.8rem;
                }

                .usage-details .value {
                    color: #fff;
                    font-size: 0.9rem;
                }

                .usage-details .value.success {
                    color: #4CAF50;
                }

                .usage-details .value.failure {
                    color: #ff4757;
                }

                .usage-reason {
                    grid-column: 1 / -1;
                }

                .usage-reason .value {
                    font-style: italic;
                }

                .usage-sid {
                    grid-column: 1 / -1;
                }

                    .usage-sid .value {
                        font-family: monospace;
                        font-size: 0.8rem;
                        color: #7EB2FF;
                    }

                    .email-broadcast {
                        margin-top: 2rem;
                        border-top: 1px solid rgba(30, 144, 255, 0.2);
                        padding-top: 2rem;
                    }

                    .email-subject-input {
                        width: 100%;
                        padding: 0.75rem;
                        margin-bottom: 1rem;
                        border: 1px solid rgba(30, 144, 255, 0.2);
                        border-radius: 4px;
                        background: rgba(0, 0, 0, 0.3);
                        color: #fff;
                        font-size: 1rem;
                    }

                    .email-subject-input:focus {
                        outline: none;
                        border-color: #1E90FF;
                    }

                    .broadcast-button.email {
                        background: linear-gradient(45deg, #1E90FF, #4169E1);
                        color: white;
                    }

                    .broadcast-button.email:hover {
                        background: linear-gradient(45deg, #4169E1, #1E90FF);
                        box-shadow: 0 4px 15px rgba(30, 144, 255, 0.4);
                    }

                @media (max-width: 768px) {
                    .usage-filter {
                        flex-wrap: wrap;
                    }

                    .filter-button {
                        flex: 1;
                        text-align: center;
                    }

                    .usage-details {
                        grid-template-columns: 1fr;
                    }
                }
                    .iq-button {
                        background: linear-gradient(45deg, #FFD700, #FFA500);
                        color: #000;
                        border: none;
                        padding: 0.75rem 1.5rem;
                        border-radius: 8px;
                        font-size: 0.9rem;
                        font-weight: 600;
                        cursor: pointer;
                        transition: all 0.3s ease;
                        text-transform: uppercase;
                        letter-spacing: 0.5px;
                        display: inline-flex;
                        align-items: center;
                        justify-content: center;
                        gap: 0.5rem;
                        margin-left: 1rem;
                        position: relative;
                        overflow: hidden;
                        box-shadow: 0 2px 10px rgba(255, 215, 0, 0.2);
                    }

                    .iq-button:hover {
                        transform: translateY(-2px);
                        box-shadow: 0 4px 15px rgba(255, 215, 0, 0.4);
                        background: linear-gradient(45deg, #FFE44D, #FFB347);
                    }

                    .iq-button:active {
                        transform: translateY(0);
                    }

                    .iq-button::before {
                        content: '';
                        position: absolute;
                        top: 0;
                        left: 0;
                        width: 100%;
                        height: 100%;
                        background: linear-gradient(45deg, transparent, rgba(255, 255, 255, 0.2), transparent);
                        transform: translateX(-100%);
                        transition: transform 0.6s;
                    }

                    .iq-button:hover::before {
                        transform: translateX(100%);
                    }

                    .iq-button.reset {
                        background: linear-gradient(45deg, #FF6B6B, #FF4757);
                        color: white;
                        box-shadow: 0 2px 10px rgba(255, 107, 107, 0.2);
                    }

                    .iq-button.reset:hover {
                        background: linear-gradient(45deg, #FF8787, #FF6B6B);
                        box-shadow: 0 4px 15px rgba(255, 107, 107, 0.4);
                    }
                    .iq-button.delete {
                        background: linear-gradient(45deg, #FF6B6B, #FF4757);
                        color: white;
                    }

                    .iq-button.delete:hover {
                        background: linear-gradient(45deg, #FF8787, #FF6B6B);
                        box-shadow: 0 4px 15px rgba(255, 107, 107, 0.4);
                    }

                    .modal-overlay {
                        position: fixed;
                        top: 0;
                        left: 0;
                        right: 0;
                        bottom: 0;
                        background-color: rgba(0, 0, 0, 0.5);
                        display: flex;
                        justify-content: center;
                        align-items: center;
                        z-index: 1000;
                    }

                    .modal-content {
                        background-color: white;
                        padding: 2rem;
                        border-radius: 8px;
                        max-width: 500px;
                        width: 90%;
                    }

                    .modal-content h2 {
                        margin-top: 0;
                        color: #333;
                    }

                    .modal-content p {
                        margin: 1rem 0;
                    }

                    .modal-content p.warning {
                        color: #dc3545;
                        font-weight: bold;
                    }

                    .modal-buttons {
                        display: flex;
                        justify-content: flex-end;
                        gap: 1rem;
                        margin-top: 2rem;
                    }

                    .modal-button {
                        padding: 0.5rem 1rem;
                        border: none;
                        border-radius: 4px;
                        cursor: pointer;
                        font-weight: bold;
                    }

                    .modal-button.cancel {
                        background-color: #6c757d;
                        color: white;
                    }

                    .modal-button.delete {
                        background-color: #dc3545;
                        color: white;
                    }

                    .modal-button.cancel:hover {
                        background-color: #5a6268;
                    }

                    .modal-button.delete:hover {
                        background-color: #c82333;
                    }

                    .user-email-container {
                        display: flex;
                        align-items: center;
                        gap: 0.5rem;
                    }

                    .gold-badge {

                        font-size: 1.2rem;
                    }

                    .gold-badge {
                        color: #FFD700;
                    }

                    .silver-badge {
                        color: #C0C0C0;
                    }

                    .discount-badge {
                        font-size: 1.2rem;
                        margin-left: 0.2rem;
                    }

                    .discount-badge.msg {
                        color: #4CAF50;
                    }

                    .discount-badge.voice {
                        color: #FFC107;
                    }

                    .discount-badge.full {
                        color: #E91E63;
                    }

                    .gold-user {
                        background: linear-gradient(90deg, rgba(255, 215, 0, 0.05), transparent);
                        border-left: 3px solid #FFD700;
                    }

                    .silver-user {
                        background: linear-gradient(90deg, rgba(192, 192, 192, 0.05), transparent);
                        border-left: 3px solid #C0C0C0;
                    }

                    .tier-badge {
                        padding: 0.25rem 0.5rem;
                        border-radius: 4px;
                        font-size: 0.8rem;
                        font-weight: 500;
                    }

                    .tier-badge.gold {
                        background: rgba(255, 215, 0, 0.1);
                        color: #FFD700;
                        border: 1px solid rgba(255, 215, 0, 0.2);
                    }

                    .tier-badge.silver {
                        background: rgba(192, 192, 192, 0.1);
                        color: #C0C0C0;
                        border: 1px solid rgba(192, 192, 192, 0.2);
                    }

                    .tier-badge.none {
                        background: rgba(128, 128, 128, 0.1);
                        color: #808080;
                        border: 1px solid rgba(128, 128, 128, 0.2);
                    }

                    .status-badge {
                        padding: 0.25rem 0.5rem;
                        border-radius: 4px;
                        font-size: 0.8rem;
                        font-weight: 500;
                    }

                    .status-badge.verified {
                        background: rgba(76, 175, 80, 0.1);
                        color: #4CAF50;
                        border: 1px solid rgba(76, 175, 80, 0.2);
                    }

                    .status-badge.unverified {
                        background: rgba(255, 152, 0, 0.1);
                        color: #FF9800;
                        border: 1px solid rgba(255, 152, 0, 0.2);
                    }

                    .status-badge.enabled {
                        background: rgba(33, 150, 243, 0.1);
                        color: #2196F3;
                        border: 1px solid rgba(33, 150, 243, 0.2);
                    }

                    .status-badge.disabled {
                        background: rgba(158, 158, 158, 0.1);
                        color: #9E9E9E;
                        border: 1px solid rgba(158, 158, 158, 0.2);
                    }

                    .status-badge.discount-msg {
                        background: rgba(76, 175, 80, 0.1);
                        color: #4CAF50;
                        border: 1px solid rgba(76, 175, 80, 0.2);
                    }

                    .status-badge.discount-voice {
                        background: rgba(255, 193, 7, 0.1);
                        color: #FFC107;
                        border: 1px solid rgba(255, 193, 7, 0.2);
                    }

                    .status-badge.discount-full {
                        background: rgba(233, 30, 99, 0.1);
                        color: #E91E63;
                        border: 1px solid rgba(233, 30, 99, 0.2);
                    }

                    .iq-button.discount-tier {
                        background: linear-gradient(45deg, #4CAF50, #81C784);
                    }

                    .iq-button.discount-tier:hover {
                        background: linear-gradient(45deg, #81C784, #4CAF50);
                        box-shadow: 0 4px 15px rgba(76, 175, 80, 0.4);
                    }

                    .iq-button.migrate {
                        background: linear-gradient(45deg, #9C27B0, #673AB7);
                        color: white;
                    }

                    .iq-button.migrate:hover {
                        background: linear-gradient(45deg, #673AB7, #9C27B0);
                        box-shadow: 0 4px 15px rgba(156, 39, 176, 0.4);
                    }

                    .users-table th {
                        padding: 0.75rem;
                        text-align: left;
                        border-bottom: 2px solid rgba(30, 144, 255, 0.2);
                        color: #1E90FF;
                        font-weight: 600;
                        white-space: nowrap;
                    }

                    .users-table td {
                        padding: 0.75rem;
                        border-bottom: 1px solid rgba(30, 144, 255, 0.1);
                        white-space: nowrap;
                    }

                    .users-table-container {
                        overflow-x: auto;
                        margin: 1rem 0;
                    }

                "#}
            </style>
        </div>
    }
}
