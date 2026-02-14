use yew::prelude::*;
use web_sys::HtmlInputElement;
use wasm_bindgen_futures::spawn_local;
use serde_json::{json, Value};
use crate::utils::api::Api;
use crate::dashboard::media_panel::{MediaPanel, MediaItem, extract_video_id};
use crate::dashboard::tesla_quick_panel::TeslaQuickPanel;
use crate::dashboard::youtube_quick_panel::YouTubeQuickPanel;
use super::timeline_view::UpcomingTask;

// @mention system - available mentions
const MENTION_OPTIONS: &[(&str, &str, &str)] = &[
    ("tesla", "Tesla Controls", "fa-car"),
    ("youtube", "YouTube", "fa-youtube"),
    // Future: ("calendar", "Calendar", "fa-calendar"),
    // Future: ("weather", "Weather", "fa-cloud"),
];

const CHAT_STYLES: &str = r#"
.chat-section {
    background: rgba(30, 30, 30, 0.6);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 12px;
    padding: 1rem;
}
.chat-messages {
    min-height: 0;
    max-height: 200px;
    overflow-y: auto;
    margin-bottom: 0.75rem;
}
.chat-messages:empty {
    display: none;
    margin-bottom: 0;
}
.chat-msg {
    padding: 0.6rem 0.9rem;
    border-radius: 12px;
    margin-bottom: 0.5rem;
    max-width: 85%;
    line-height: 1.4;
    font-size: 0.9rem;
}
.chat-msg.user {
    background: rgba(30, 144, 255, 0.15);
    color: #9ecfff;
    margin-left: auto;
    border-bottom-right-radius: 4px;
}
.chat-msg.assistant {
    background: rgba(255, 255, 255, 0.08);
    color: #ddd;
    margin-right: auto;
    border-bottom-left-radius: 4px;
}
.chat-msg.loading {
    opacity: 0.6;
}
.chat-error {
    color: #ff6b6b;
    font-size: 0.85rem;
    padding: 0.5rem;
    margin-bottom: 0.5rem;
}
.chat-image-preview {
    position: relative;
    display: inline-block;
    margin-bottom: 0.75rem;
}
.chat-image-preview img {
    max-width: 120px;
    max-height: 80px;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.1);
}
.chat-image-preview .remove-btn {
    position: absolute;
    top: -8px;
    right: -8px;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: #ff4444;
    color: white;
    border: none;
    cursor: pointer;
    font-size: 12px;
    display: flex;
    align-items: center;
    justify-content: center;
}
.chat-input-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    width: 100%;
    box-sizing: border-box;
}
.chat-input-row input[type="text"] {
    flex: 1 1 0;
    min-width: 50px;
    background: rgba(255, 255, 255, 0.06) !important;
    border: 1px solid rgba(255, 255, 255, 0.12) !important;
    border-radius: 8px !important;
    padding: 0.6rem 0.9rem !important;
    color: #fff !important;
    font-size: 0.95rem !important;
    outline: none;
    box-sizing: border-box;
}
.chat-input-row input[type="text"]:focus {
    border-color: rgba(30, 144, 255, 0.5);
    background: rgba(255, 255, 255, 0.08);
}
.chat-input-row input[type="text"]::placeholder {
    color: #666;
}
.chat-btn {
    flex-shrink: 0;
    border-radius: 8px;
    padding: 0.5rem 0.75rem;
    cursor: pointer;
    font-size: 0.85rem;
    display: flex;
    align-items: center;
    gap: 0.4rem;
    white-space: nowrap;
    transition: all 0.2s;
}
.chat-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
}
.chat-btn.attach {
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.12);
    color: #888;
    width: 38px;
    height: 38px;
    padding: 0;
    justify-content: center;
}
.chat-btn.attach:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.12);
    color: #aaa;
}
.chat-btn.digest {
    background: rgba(30, 144, 255, 0.1);
    border: 1px solid rgba(30, 144, 255, 0.25);
    color: #7EB2FF;
}
.chat-btn.digest:hover:not(:disabled) {
    background: rgba(30, 144, 255, 0.18);
    border-color: rgba(30, 144, 255, 0.4);
}
.chat-btn.call {
    background: rgba(76, 175, 80, 0.12);
    border: 1px solid rgba(76, 175, 80, 0.3);
    color: #81C784;
}
.chat-btn.call:hover:not(:disabled) {
    background: rgba(76, 175, 80, 0.2);
}
.chat-btn.call.active {
    background: rgba(244, 67, 54, 0.15);
    border-color: rgba(244, 67, 54, 0.4);
    color: #ef9a9a;
}
.chat-btn.send {
    background: linear-gradient(135deg, #1E90FF, #4169E1);
    border: none;
    color: white;
    padding: 0.6rem 1rem;
    font-weight: 500;
}
.chat-btn.send:hover:not(:disabled) {
    box-shadow: 0 4px 12px rgba(30, 144, 255, 0.3);
}
.chat-price {
    font-size: 0.7rem;
    color: #666;
    background: rgba(0, 0, 0, 0.3);
    padding: 0.15rem 0.35rem;
    border-radius: 4px;
}
.chat-btn.send .chat-price {
    background: rgba(255, 255, 255, 0.2);
    color: rgba(255, 255, 255, 0.8);
}
/* Task preview panel */
.task-preview-panel {
    background: rgba(30, 144, 255, 0.1);
    border: 1px solid rgba(30, 144, 255, 0.3);
    border-radius: 12px;
    padding: 0.75rem;
    margin-top: 0.5rem;
}
.task-preview-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.5rem;
}
.task-preview-label {
    color: #7eb2ff;
    font-size: 0.8rem;
    font-weight: 500;
    display: flex;
    align-items: center;
    gap: 0.4rem;
}
.task-preview-close {
    background: transparent;
    border: none;
    color: #666;
    cursor: pointer;
    font-size: 1rem;
    padding: 0.25rem 0.5rem;
    border-radius: 4px;
    transition: all 0.2s;
}
.task-preview-close:hover {
    color: #999;
    background: rgba(255, 255, 255, 0.05);
}
.task-preview-content {
    cursor: pointer;
    padding: 0.5rem;
    border-radius: 8px;
    transition: background 0.2s;
}
.task-preview-content:hover {
    background: rgba(30, 144, 255, 0.1);
}
.task-preview-time {
    color: #fff;
    font-size: 0.9rem;
    font-weight: 500;
    display: flex;
    align-items: center;
    gap: 0.4rem;
}
.task-preview-time i {
    color: #7eb2ff;
}
.task-preview-date {
    color: #888;
    font-weight: 400;
}
.task-preview-desc {
    color: #ccc;
    font-size: 0.85rem;
    margin-top: 0.25rem;
    line-height: 1.4;
}
.task-preview-source {
    color: #7eb2ff;
    font-size: 0.75rem;
    margin-top: 0.15rem;
    opacity: 0.8;
}
.task-preview-condition {
    color: #e8a838;
    font-size: 0.75rem;
    margin-top: 0.15rem;
    font-style: italic;
}
.task-preview-hint {
    color: #666;
    font-size: 0.75rem;
    margin-top: 0.5rem;
}
.chat-shortcut-row {
    display: flex;
    gap: 0.4rem;
    margin-top: 0.4rem;
}
.chat-shortcut-btn {
    display: flex;
    align-items: center;
    gap: 0.3rem;
    background: transparent;
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 14px;
    padding: 0.2rem 0.6rem;
    color: #888;
    font-size: 0.75rem;
    cursor: pointer;
    transition: all 0.2s;
}
.chat-shortcut-btn:hover {
    background: rgba(255, 255, 255, 0.08);
    border-color: rgba(255, 255, 255, 0.2);
    color: #bbb;
}
.chat-shortcut-btn i {
    font-size: 0.7rem;
}
"#;

#[derive(Properties, PartialEq, Clone)]
pub struct ChatBoxProps {
    pub on_usage_change: Callback<()>,
    #[prop_or(false)]
    pub youtube_connected: bool,
    #[prop_or(false)]
    pub tesla_connected: bool,
    #[prop_or_default]
    pub focused_task: Option<UpcomingTask>,
    #[prop_or_default]
    pub on_task_cleared: Callback<()>,
    /// Callback when a task is created via chat - passes the task ID
    #[prop_or_default]
    pub on_task_created: Callback<i32>,
    /// Task preview (shown after creation, before entering edit mode)
    #[prop_or_default]
    pub preview_task: Option<UpcomingTask>,
    /// Callback when user clicks preview task to edit it
    #[prop_or_default]
    pub on_preview_click: Callback<UpcomingTask>,
    /// Callback to close task preview
    #[prop_or_default]
    pub on_preview_close: Callback<()>,
}

#[function_component(ChatBox)]
pub fn chat_box(props: &ChatBoxProps) -> Html {
    // Web chat state - only stores the most recent exchange (user msg, bot reply)
    let chat_user_msg = use_state(|| None::<String>);
    let chat_bot_reply = use_state(|| None::<String>);
    let chat_input = use_state(|| String::new());
    let chat_loading = use_state(|| false);
    let chat_error = use_state(|| None::<String>);
    let chat_input_ref = use_node_ref();

    // Image attachment state for web chat (paste from clipboard only)
    let chat_image: UseStateHandle<Option<web_sys::File>> = use_state(|| None);
    let chat_image_preview: UseStateHandle<Option<String>> = use_state(|| None);

    // Web call state
    let call_active = use_state(|| false);
    let call_connecting = use_state(|| false);
    let call_duration = use_state(|| 0i32);
    let call_error = use_state(|| None::<String>);

    // Media panel state for detected URLs and AI search results
    let detected_media: UseStateHandle<Vec<MediaItem>> = use_state(|| Vec::new());
    let media_playing = use_state(|| false);
    let media_playing_index = use_state(|| 0usize);
    let media_from_yt_panel = use_mut_ref(|| false);

    // @mention system state
    let active_mention = use_state(|| None::<String>);

    // Update call duration every second when call is active
    {
        let call_active = call_active.clone();
        let call_duration = call_duration.clone();
        let is_active_dep = (*call_active).clone();
        use_effect_with_deps(
            move |is_active: &bool| {
                let call_duration = call_duration.clone();
                let interval_handle: Option<gloo_timers::callback::Interval> = if *is_active {
                    Some(gloo_timers::callback::Interval::new(1000, move || {
                        let duration = crate::utils::elevenlabs_web::get_elevenlabs_call_duration();
                        call_duration.set(duration);
                    }))
                } else {
                    None
                };
                move || {
                    drop(interval_handle);
                }
            },
            is_active_dep,
        );
    }

    // Auto-focus chat input on mount
    {
        let chat_input_ref = chat_input_ref.clone();
        use_effect_with_deps(
            move |_| {
                // Small delay to ensure DOM is updated
                let chat_input_ref = chat_input_ref.clone();
                gloo_timers::callback::Timeout::new(100, move || {
                    if let Some(input) = chat_input_ref.cast::<HtmlInputElement>() {
                        let _ = input.focus();
                    }
                }).forget();
                || ()
            },
            (),
        );
    }

    // Clear chat history when a task is selected for editing
    {
        let chat_user_msg = chat_user_msg.clone();
        let chat_bot_reply = chat_bot_reply.clone();
        let chat_error = chat_error.clone();
        let focused_task_id = props.focused_task.as_ref().and_then(|t| t.task_id);
        use_effect_with_deps(
            move |task_id: &Option<i32>| {
                if task_id.is_some() {
                    // Clear chat when task is selected
                    chat_user_msg.set(None);
                    chat_bot_reply.set(None);
                    chat_error.set(None);
                }
                || ()
            },
            focused_task_id,
        );
    }

    let on_send = {
        let chat_input = chat_input.clone();
        let chat_user_msg = chat_user_msg.clone();
        let chat_bot_reply = chat_bot_reply.clone();
        let chat_loading = chat_loading.clone();
        let chat_error = chat_error.clone();
        let refetch_usage = props.on_usage_change.clone();
        let chat_image = chat_image.clone();
        let chat_image_preview = chat_image_preview.clone();
        let detected_media_send = detected_media.clone();
        let media_playing_send = media_playing.clone();
        let focused_task = props.focused_task.clone();
        let on_task_cleared = props.on_task_cleared.clone();
        let on_task_created = props.on_task_created.clone();
        let chat_input_ref = chat_input_ref.clone();

        Callback::from(move |_| {
            let message = (*chat_input).clone();
            let has_image = (*chat_image).is_some();

            // Allow send if there's text OR an image
            if message.trim().is_empty() && !has_image {
                return;
            }

            let chat_input = chat_input.clone();
            let chat_user_msg = chat_user_msg.clone();
            let chat_bot_reply = chat_bot_reply.clone();
            let chat_loading = chat_loading.clone();
            let chat_error = chat_error.clone();
            let refetch_usage = refetch_usage.clone();
            let chat_image = chat_image.clone();
            let chat_image_preview = chat_image_preview.clone();
            let image_file = (*chat_image).clone();
            let detected_media = detected_media_send.clone();
            let media_playing = media_playing_send.clone();
            let focused_task = focused_task.clone();
            let on_task_cleared = on_task_cleared.clone();
            let on_task_created = on_task_created.clone();
            let chat_input_ref = chat_input_ref.clone();

            // Set user message and clear previous reply (only for regular chat, not task editing)
            let is_task_edit = focused_task.is_some();
            if !is_task_edit {
                let display_msg = if has_image {
                    if message.trim().is_empty() {
                        "[Image]".to_string()
                    } else {
                        format!("[Image] {}", message)
                    }
                } else {
                    message.clone()
                };
                chat_user_msg.set(Some(display_msg));
                chat_bot_reply.set(None);
            }
            chat_loading.set(true);
            chat_error.set(None);
            chat_input.set(String::new());

            spawn_local(async move {
                // Check if we're in task edit mode
                let result = if let Some(task) = &focused_task {
                    // Task edit mode - call edit endpoint
                    if let Some(task_id) = task.task_id {
                        Api::post(&format!("/api/tasks/{}/edit-ai", task_id))
                            .json(&json!({ "instruction": message }))
                            .unwrap()
                            .send()
                            .await
                    } else {
                        Err(gloo_net::Error::GlooError("Task has no ID".to_string()))
                    }
                } else if let Some(file) = image_file {
                    // Send with image
                    let array_buffer = wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await;
                    if let Ok(buffer) = array_buffer {
                        let uint8_array = js_sys::Uint8Array::new(&buffer);
                        let bytes: Vec<u8> = uint8_array.to_vec();
                        let base64_image = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
                        let content_type = file.type_();

                        Api::post("/api/chat/web")
                            .json(&json!({
                                "message": message,
                                "image": base64_image,
                                "image_type": content_type
                            }))
                            .unwrap()
                            .send()
                            .await
                    } else {
                        Err(gloo_net::Error::GlooError("Failed to read image".to_string()))
                    }
                } else {
                    // Send text only
                    Api::post("/api/chat/web")
                        .json(&json!({ "message": message }))
                        .unwrap()
                        .send()
                        .await
                };

                // Clear image after sending
                chat_image.set(None);
                chat_image_preview.set(None);

                match result {
                    Ok(response) => {
                        if response.ok() {
                            match response.json::<Value>().await {
                                Ok(data) => {
                                    let reply = data["message"].as_str().unwrap_or("No response").to_string();

                                    // For task edits, show the response and refresh
                                    if focused_task.is_some() {
                                        // Show the AI's response/explanation
                                        chat_bot_reply.set(Some(reply));

                                        // Dispatch event to refresh dashboard (updates task details)
                                        if let Some(window) = web_sys::window() {
                                            let event = web_sys::CustomEvent::new("lightfriend-chat-sent").unwrap();
                                            let _ = window.dispatch_event(&event);
                                        }
                                        // Refocus the input for continued editing (with delay to ensure re-render completes)
                                        let chat_input_ref = chat_input_ref.clone();
                                        gloo_timers::callback::Timeout::new(100, move || {
                                            if let Some(input) = chat_input_ref.cast::<HtmlInputElement>() {
                                                let _ = input.focus();
                                            }
                                        }).forget();
                                        // Stay in task edit mode - don't call on_task_cleared
                                    } else {
                                        // Regular chat - show in message history
                                        chat_bot_reply.set(Some(reply));
                                        refetch_usage.emit(());

                                        // Check for media results from AI tool calls
                                        if let Some(media_arr) = data["media"].as_array() {
                                            let media_items: Vec<MediaItem> = media_arr.iter().filter_map(|m| {
                                                Some(MediaItem {
                                                    platform: m["platform"].as_str()?.to_string(),
                                                    video_id: m["video_id"].as_str()?.to_string(),
                                                    title: m["title"].as_str().unwrap_or("").to_string(),
                                                    thumbnail: m["thumbnail"].as_str().unwrap_or("").to_string(),
                                                    duration: m["duration"].as_str().map(|s| s.to_string()),
                                                    channel: m["channel"].as_str().map(|s| s.to_string()),
                                                    original_url: None, // AI search results don't have original URLs
                                                })
                                            }).collect();
                                            if !media_items.is_empty() {
                                                detected_media.set(media_items);
                                                media_playing.set(false);
                                            }
                                        }

                                        // Check if a task was created - trigger preview
                                        if let Some(task_id) = data["created_task_id"].as_i64() {
                                            on_task_created.emit(task_id as i32);
                                        }

                                        // Dispatch event for other components
                                        if let Some(window) = web_sys::window() {
                                            let event = web_sys::CustomEvent::new("lightfriend-chat-sent").unwrap();
                                            let _ = window.dispatch_event(&event);
                                        }
                                    }
                                }
                                Err(_) => {
                                    chat_error.set(Some("Failed to parse response".to_string()));
                                }
                            }
                        } else {
                            let status = response.status();
                            match response.json::<Value>().await {
                                Ok(data) => {
                                    let err = data["error"].as_str().unwrap_or("Request failed").to_string();
                                    chat_error.set(Some(err));
                                }
                                Err(e) => {
                                    chat_error.set(Some(format!("Request failed ({}): {}", status, e)));
                                }
                            }
                        }
                    }
                    Err(_) => {
                        chat_error.set(Some("Network error".to_string()));
                    }
                }
                chat_loading.set(false);
            });
        })
    };

    // Handler for starting a web call
    let on_start_call = {
        let call_active = call_active.clone();
        let call_connecting = call_connecting.clone();
        let call_duration = call_duration.clone();
        let call_error = call_error.clone();

        Callback::from(move |_| {
            let call_active = call_active.clone();
            let call_connecting = call_connecting.clone();
            let call_duration = call_duration.clone();
            let call_error = call_error.clone();

            call_connecting.set(true);
            call_error.set(None);

            spawn_local(async move {
                match Api::get("/api/call/web-signed-url").send().await {
                    Ok(response) => {
                        if response.ok() {
                            match response.json::<Value>().await {
                                Ok(data) => {
                                    if let Some(signed_url) = data["signed_url"].as_str() {
                                        let overrides = data
                                            .get("agent_overrides")
                                            .map(|v| serde_wasm_bindgen::to_value(v).unwrap_or(wasm_bindgen::JsValue::NULL))
                                            .unwrap_or(wasm_bindgen::JsValue::NULL);
                                        let result = crate::utils::elevenlabs_web::start_elevenlabs_call(signed_url, overrides).await;
                                        if result.is_truthy() {
                                            call_active.set(true);
                                            call_duration.set(0);
                                        } else {
                                            call_error.set(Some("Failed to start call. Check microphone permissions.".to_string()));
                                        }
                                    } else {
                                        call_error.set(Some("Invalid response from server".to_string()));
                                    }
                                }
                                Err(_) => {
                                    call_error.set(Some("Failed to parse server response".to_string()));
                                }
                            }
                        } else {
                            match response.json::<Value>().await {
                                Ok(data) => {
                                    let err = data["error"].as_str().unwrap_or("Failed to start call").to_string();
                                    call_error.set(Some(err));
                                }
                                Err(_) => {
                                    call_error.set(Some("Failed to start call".to_string()));
                                }
                            }
                        }
                    }
                    Err(_) => {
                        call_error.set(Some("Network error".to_string()));
                    }
                }
                call_connecting.set(false);
            });
        })
    };

    // Handler for ending a web call
    let on_end_call = {
        let call_active = call_active.clone();
        let call_duration = call_duration.clone();
        let refetch_usage = props.on_usage_change.clone();

        Callback::from(move |_| {
            let call_active = call_active.clone();
            let call_duration = call_duration.clone();
            let refetch_usage = refetch_usage.clone();

            spawn_local(async move {
                let _duration = crate::utils::elevenlabs_web::end_elevenlabs_call().await;
                call_active.set(false);
                if let Some(window) = web_sys::window() {
                    let event = web_sys::CustomEvent::new("lightfriend-chat-sent").unwrap();
                    let _ = window.dispatch_event(&event);
                    refetch_usage.emit(());
                }
                call_duration.set(0);
            });
        })
    };

    // Connection shortcut icon callbacks
    let show_shortcuts = props.focused_task.is_none()
        && (props.tesla_connected || props.youtube_connected);
    let tesla_shortcut_click = {
        let chat_input = chat_input.clone();
        let active_mention = active_mention.clone();
        let chat_input_ref = chat_input_ref.clone();
        Callback::from(move |_: MouseEvent| {
            chat_input.set("@tesla ".to_string());
            active_mention.set(Some("tesla".to_string()));
            let chat_input_ref = chat_input_ref.clone();
            gloo_timers::callback::Timeout::new(50, move || {
                if let Some(input) = chat_input_ref.cast::<HtmlInputElement>() {
                    let _ = input.focus();
                }
            }).forget();
        })
    };
    let youtube_shortcut_click = {
        let chat_input = chat_input.clone();
        let active_mention = active_mention.clone();
        let chat_input_ref = chat_input_ref.clone();
        Callback::from(move |_: MouseEvent| {
            chat_input.set("@youtube ".to_string());
            active_mention.set(Some("youtube".to_string()));
            let chat_input_ref = chat_input_ref.clone();
            gloo_timers::callback::Timeout::new(50, move || {
                if let Some(input) = chat_input_ref.cast::<HtmlInputElement>() {
                    let _ = input.focus();
                }
            }).forget();
        })
    };

    html! {
        <>
            <style>{CHAT_STYLES}</style>
            <div class="chat-section">
                <div class="chat-messages">
                    {
                        match ((*chat_user_msg).clone(), (*chat_bot_reply).clone(), *chat_loading) {
                            // Task edit mode: only show bot reply (must come before wildcard)
                            (None, Some(bot_reply), false) => html! {
                                <div class="chat-msg assistant">{bot_reply}</div>
                            },
                            // Loading state in task edit mode (no user message shown)
                            (None, None, true) => html! {
                                <div class="chat-msg assistant loading">{"..."}</div>
                            },
                            // No messages, not loading - show nothing
                            (None, None, false) => html! {},
                            // Regular chat: user message with loading indicator
                            (Some(user_msg), None, true) => html! {
                                <>
                                    <div class="chat-msg user">{user_msg}</div>
                                    <div class="chat-msg assistant loading">{"..."}</div>
                                </>
                            },
                            // Regular chat: both messages
                            (Some(user_msg), Some(bot_reply), _) => html! {
                                <>
                                    <div class="chat-msg user">{user_msg}</div>
                                    <div class="chat-msg assistant">{bot_reply}</div>
                                </>
                            },
                            // Regular chat: user message, no response yet
                            (Some(user_msg), None, false) => html! {
                                <div class="chat-msg user">{user_msg}</div>
                            },
                            // Task edit mode: loading with existing reply
                            (None, Some(_), true) => html! {
                                <div class="chat-msg assistant loading">{"..."}</div>
                            },
                        }
                    }
                </div>
                {
                    if let Some(err) = (*chat_error).as_ref() {
                        html! { <div class="chat-error">{err}</div> }
                    } else {
                        html! {}
                    }
                }
                {
                    if let Some(err) = (*call_error).as_ref() {
                        html! { <div class="chat-error">{err}</div> }
                    } else {
                        html! {}
                    }
                }
                {
                    if let Some(preview_url) = (*chat_image_preview).clone() {
                        let chat_image_clear = chat_image.clone();
                        let chat_image_preview_clear = chat_image_preview.clone();
                        html! {
                            <div class="chat-image-preview">
                                <img src={preview_url} alt="Attached image" />
                                <button
                                    class="remove-btn"
                                    onclick={Callback::from(move |_: MouseEvent| {
                                        chat_image_clear.set(None);
                                        chat_image_preview_clear.set(None);
                                    })}
                                    title="Remove image"
                                >
                                    {"x"}
                                </button>
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }
                <div class="chat-input-row">
                    {
                        // Hide call button when in task edit mode
                        if props.focused_task.is_some() {
                            html! {}
                        } else if *call_active {
                            let duration = *call_duration;
                            let mins = duration / 60;
                            let secs = duration % 60;
                            html! {
                                <button
                                    class="chat-btn call active"
                                    onclick={{
                                        let on_end_call = on_end_call.clone();
                                        Callback::from(move |_: MouseEvent| {
                                            on_end_call.emit(());
                                        })
                                    }}
                                    title="End the call"
                                >
                                    {format!("End {mins}:{secs:02}")}
                                </button>
                            }
                        } else if *call_connecting {
                            html! {
                                <button class="chat-btn call" disabled=true title="Connecting...">
                                    {"..."}
                                </button>
                            }
                        } else {
                            html! {
                                <button
                                    class="chat-btn call"
                                    onclick={{
                                        let on_start_call = on_start_call.clone();
                                        Callback::from(move |_: MouseEvent| {
                                            on_start_call.emit(());
                                        })
                                    }}
                                    disabled={*chat_loading}
                                    title="Start voice call"
                                >
                                    {"Call"}
                                </button>
                            }
                        }
                    }
                    <input
                        type="text"
                        class="chat-text-input"
                        style="flex: 1 1 0; min-width: 100px;"
                        ref={chat_input_ref.clone()}
                        value={(*chat_input).clone()}
                        placeholder={if props.focused_task.is_some() { "Describe an edit to the task..." } else { "Ask your assistant..." }}
                        disabled={*chat_loading || *call_active}
                        oninput={{
                            let chat_input = chat_input.clone();
                            let detected_media = detected_media.clone();
                            let media_playing = media_playing.clone();
                            let active_mention = active_mention.clone();
                            Callback::from(move |e: InputEvent| {
                                let input: HtmlInputElement = e.target_unchecked_into();
                                let value = input.value();
                                chat_input.set(value.clone());

                                // @mention detection - check for @word pattern at end of input
                                let mention_regex = regex::Regex::new(r"@(\w*)$").ok();
                                if let Some(re) = mention_regex {
                                    if let Some(cap) = re.captures(&value) {
                                        let query = cap.get(1).map(|m| m.as_str().to_lowercase()).unwrap_or_default();

                                        // Find matching mentions
                                        let matches: Vec<_> = MENTION_OPTIONS.iter()
                                            .filter(|(name, _, _)| name.starts_with(&query))
                                            .collect();

                                        // If exactly one match, show its panel
                                        if matches.len() == 1 && !query.is_empty() {
                                            let (name, _, _) = matches[0];
                                            active_mention.set(Some(name.to_string()));
                                        } else if query.is_empty() {
                                            // Just typed @ - could show dropdown here in future
                                            // For now, don't change active_mention
                                        }
                                    } else {
                                        // No @mention pattern found - check if we should clear
                                        // Only clear if user deleted the @ or mention text
                                        let has_mention = MENTION_OPTIONS.iter()
                                            .any(|(name, _, _)| value.to_lowercase().contains(&format!("@{}", name)));
                                        if !has_mention && (*active_mention).is_some() {
                                            // Check if there's still a partial @mention
                                            let has_at = value.contains('@');
                                            if !has_at {
                                                active_mention.set(None);
                                            }
                                        }
                                    }
                                }

                                // Also check for completed @mentions in text
                                for (name, _, _) in MENTION_OPTIONS.iter() {
                                    if value.to_lowercase().contains(&format!("@{}", name)) {
                                        active_mention.set(Some(name.to_string()));
                                        break;
                                    }
                                }

                                // Detect video URLs from all supported platforms
                                let url_regex = regex::Regex::new(r"https?://[^\s]+").ok();
                                if let Some(re) = url_regex {
                                    let mut new_media = Vec::new();
                                    for cap in re.find_iter(&value) {
                                        let url = cap.as_str();
                                        // Use the unified extract_video_id function
                                        if let Some((platform, video_id)) = extract_video_id(url) {
                                            use crate::dashboard::media_panel::MediaPlatform;
                                            let platform_name = match platform {
                                                MediaPlatform::YouTube => "youtube",
                                                MediaPlatform::TikTok => "tiktok",
                                                MediaPlatform::Instagram => "instagram",
                                                MediaPlatform::Twitter => "twitter",
                                                MediaPlatform::Vimeo => "vimeo",
                                                MediaPlatform::Rumble => "rumble",
                                                MediaPlatform::Dailymotion => "dailymotion",
                                                MediaPlatform::Reddit => "reddit",
                                                MediaPlatform::Streamable => "streamable",
                                                MediaPlatform::Spotify => "spotify",
                                                _ => continue,
                                            };
                                            let thumbnail = match platform_name {
                                                "youtube" => format!("https://img.youtube.com/vi/{}/mqdefault.jpg", video_id),
                                                _ => String::new(),
                                            };
                                            let title = match platform_name {
                                                "youtube" => format!("YouTube Video: {}", video_id),
                                                "tiktok" => format!("TikTok Video: {}", video_id),
                                                "instagram" => format!("Instagram Reel: {}", video_id),
                                                "twitter" => format!("Twitter Video: {}", video_id),
                                                "vimeo" => format!("Vimeo Video: {}", video_id),
                                                "rumble" => format!("Rumble Video: {}", video_id),
                                                "dailymotion" => format!("Dailymotion Video: {}", video_id),
                                                "reddit" => format!("Reddit Post: {}", video_id),
                                                "streamable" => format!("Streamable Video: {}", video_id),
                                                "spotify" => format!("Spotify Track: {}", video_id),
                                                _ => format!("Video: {}", video_id),
                                            };
                                            new_media.push(MediaItem {
                                                platform: platform_name.to_string(),
                                                video_id: video_id.clone(),
                                                title,
                                                thumbnail,
                                                duration: None,
                                                channel: None,
                                                original_url: Some(url.to_string()),
                                            });
                                        }
                                    }
                                    if !new_media.is_empty() {
                                        detected_media.set(new_media);
                                        media_playing.set(false);
                                    } else if !(*detected_media).is_empty() {
                                        // Only clear if there were previously detected media from URL detection
                                        // Don't clear AI search results
                                        let current = (*detected_media).clone();
                                        // If current media was from URL detection (no channel info), clear it
                                        if current.iter().all(|m| m.channel.is_none()) {
                                            detected_media.set(Vec::new());
                                        }
                                    }
                                }
                            })
                        }}
                        onkeypress={{
                            let on_send = on_send.clone();
                            Callback::from(move |e: KeyboardEvent| {
                                if e.key() == "Enter" {
                                    on_send.emit(());
                                }
                            })
                        }}
                        onpaste={{
                            let chat_image = chat_image.clone();
                            let chat_image_preview = chat_image_preview.clone();
                            let chat_error = chat_error.clone();
                            Callback::from(move |e: Event| {
                                use wasm_bindgen::JsCast;
                                if let Some(clipboard_event) = e.dyn_ref::<web_sys::ClipboardEvent>() {
                                    if let Some(clipboard_data) = clipboard_event.clipboard_data() {
                                        if let Some(items) = clipboard_data.files() {
                                            for i in 0..items.length() {
                                                if let Some(file) = items.get(i) {
                                                    if file.type_().starts_with("image/") {
                                                        e.prevent_default();
                                                        if file.size() > 10.0 * 1024.0 * 1024.0 {
                                                            chat_error.set(Some("Image must be less than 10MB".to_string()));
                                                            return;
                                                        }
                                                        let chat_image = chat_image.clone();
                                                        let chat_image_preview = chat_image_preview.clone();
                                                        let file_clone = file.clone();
                                                        wasm_bindgen_futures::spawn_local(async move {
                                                            let array_buffer = wasm_bindgen_futures::JsFuture::from(file_clone.array_buffer()).await;
                                                            if let Ok(buffer) = array_buffer {
                                                                let uint8_array = js_sys::Uint8Array::new(&buffer);
                                                                let bytes: Vec<u8> = uint8_array.to_vec();
                                                                let base64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
                                                                let content_type = file_clone.type_();
                                                                let data_url = format!("data:{};base64,{}", content_type, base64);
                                                                chat_image_preview.set(Some(data_url));
                                                                chat_image.set(Some(file_clone));
                                                            }
                                                        });
                                                        break;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            })
                        }}
                    />
                    <button
                        class="chat-btn send"
                        onclick={{
                            let on_send = on_send.clone();
                            Callback::from(move |_: MouseEvent| {
                                on_send.emit(());
                            })
                        }}
                        disabled={*chat_loading || *call_active}
                    >
                        {"Send"}
                    </button>
                </div>
                // Connection shortcut icons below the input
                if show_shortcuts {
                    <div class="chat-shortcut-row">
                        if props.tesla_connected {
                            <button class="chat-shortcut-btn" onclick={tesla_shortcut_click}>
                                <i class="fa-solid fa-car"></i>
                                {"Tesla"}
                            </button>
                        }
                        if props.youtube_connected {
                            <button class="chat-shortcut-btn" onclick={youtube_shortcut_click}>
                                <i class="fa-brands fa-youtube"></i>
                                {"YouTube"}
                            </button>
                        }
                    </div>
                }
                // Media panel for detected URLs and AI search results
                // Hide media panel when editing a task (will reappear when task editing ends)
                {
                    if !(*detected_media).is_empty() && props.focused_task.is_none() {
                        let on_media_close = {
                            let detected_media = detected_media.clone();
                            let media_playing = media_playing.clone();
                            let media_from_yt_panel = media_from_yt_panel.clone();
                            Callback::from(move |_: ()| {
                                detected_media.set(Vec::new());
                                media_playing.set(false);
                                *media_from_yt_panel.borrow_mut() = false;
                            })
                        };
                        let on_media_select = {
                            let media_playing = media_playing.clone();
                            let media_playing_index = media_playing_index.clone();
                            Callback::from(move |idx: usize| {
                                media_playing_index.set(idx);
                                media_playing.set(true);
                            })
                        };
                        let from_yt_render = *media_from_yt_panel.borrow();
                        let on_media_back = if from_yt_render || (*detected_media).len() > 1 {
                            let media_playing = media_playing.clone();
                            let detected_media = detected_media.clone();
                            let active_mention = active_mention.clone();
                            let media_from_yt_panel = media_from_yt_panel.clone();
                            Some(Callback::from(move |_: ()| {
                                let from_yt = *media_from_yt_panel.borrow();
                                if from_yt {
                                    // Go back to YouTube subscription feed
                                    *media_from_yt_panel.borrow_mut() = false;
                                    detected_media.set(Vec::new());
                                    media_playing.set(false);
                                    active_mention.set(Some("youtube".to_string()));
                                } else {
                                    media_playing.set(false);
                                }
                            }))
                        } else {
                            None
                        };
                        html! {
                            <MediaPanel
                                media_items={(*detected_media).clone()}
                                playing={*media_playing}
                                playing_index={*media_playing_index}
                                on_close={on_media_close}
                                on_select={on_media_select}
                                on_back={on_media_back}
                                youtube_connected={props.youtube_connected}
                            />
                        }
                    } else {
                        html! {}
                    }
                }
                // @mention control panels
                {
                    match (*active_mention).as_deref() {
                        Some("tesla") if props.focused_task.is_none() => {
                            let on_close = {
                                let active_mention = active_mention.clone();
                                Callback::from(move |_: ()| active_mention.set(None))
                            };
                            html! { <TeslaQuickPanel on_close={on_close} /> }
                        }
                        Some("youtube") if props.focused_task.is_none() => {
                            let on_close = {
                                let active_mention = active_mention.clone();
                                Callback::from(move |_: ()| active_mention.set(None))
                            };
                            let on_video_select = {
                                let detected_media = detected_media.clone();
                                let media_playing = media_playing.clone();
                                let media_playing_index = media_playing_index.clone();
                                let active_mention = active_mention.clone();
                                let media_from_yt_panel = media_from_yt_panel.clone();
                                Callback::from(move |item: MediaItem| {
                                    *media_from_yt_panel.borrow_mut() = true;
                                    detected_media.set(vec![item]);
                                    media_playing_index.set(0);
                                    media_playing.set(true);
                                    active_mention.set(None);
                                })
                            };
                            html! { <YouTubeQuickPanel on_close={on_close} on_video_select={on_video_select} /> }
                        }
                        // Future: Some("calendar") => html! { <CalendarPanel on_close={...} /> }
                        _ => html! {}
                    }
                }
                // Task preview panel (shown after task creation)
                {
                    if let Some(task) = &props.preview_task {
                        let task_for_click = task.clone();
                        let on_click = props.on_preview_click.clone();
                        let on_close = props.on_preview_close.clone();
                        let is_recurring = task.trigger_type == "recurring_email" || task.trigger_type == "recurring_messaging";
                        html! {
                            <div class="task-preview-panel">
                                <div class="task-preview-header">
                                    <span class="task-preview-label">{if is_recurring { "Monitoring active" } else { "Task scheduled" }}</span>
                                    <button class="task-preview-close" onclick={Callback::from(move |_: MouseEvent| on_close.emit(()))}>{"x"}</button>
                                </div>
                                <div class="task-preview-content" onclick={Callback::from(move |_: MouseEvent| on_click.emit(task_for_click.clone()))}>
                                    <div class="task-preview-time">
                                        {if is_recurring {
                                            html! { <i class="fa-solid fa-eye"></i> }
                                        } else {
                                            html! { <i class="fa-regular fa-clock"></i> }
                                        }}
                                        {&task.time_display}
                                        {if !task.date_display.is_empty() {
                                            html! { <span class="task-preview-date">{format!(" - {}", &task.date_display)}</span> }
                                        } else {
                                            html! {}
                                        }}
                                    </div>
                                    if let Some(ref src) = task.sources_display {
                                        <div class="task-preview-source">{format!("Check: {}", src)}</div>
                                    }
                                    if let Some(ref cond) = task.condition {
                                        <div class="task-preview-condition">{format!("Condition: {}", cond)}</div>
                                    }
                                    <div class="task-preview-desc">
                                        {if task.condition.is_some() || task.sources_display.is_some() {
                                            format!("Then: {}", &task.description)
                                        } else {
                                            task.description.clone()
                                        }}
                                    </div>
                                    <div class="task-preview-hint">{"Click to edit"}</div>
                                </div>
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }
            </div>
        </>
    }
}
