use yew::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use serde::Deserialize;
use crate::utils::api::Api;
use crate::profile::billing_models::UserProfile;

use super::chat_box::ChatBox;
use super::triage_indicator::{TriageIndicator, AttentionItem as TriageAttentionItem};
use super::timeline_view::{TimelineView, UpcomingTask, UpcomingDigest};
use super::dashboard_footer::{DashboardFooter, NextDigestInfo};
use super::settings_panel::{SettingsPanel, SettingsTab};
use super::activity_panel::ActivityPanel;
use super::quiet_mode::QuietModeStatus;

const DASHBOARD_STYLES: &str = r#"
.peace-dashboard {
    display: flex;
    flex-direction: column;
    gap: 2rem;
    max-width: 600px;
    margin: 0 auto;
    padding: 0;
    position: relative;
}
.task-focus-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.3);
    z-index: 100;
    cursor: pointer;
}
.task-edit-container {
    position: relative;
    z-index: 101;
}
.peace-main {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
    transition: all 0.2s ease;
}
.peace-main.task-focused {
    filter: blur(4px);
    opacity: 0.6;
}
.peace-separator {
    height: 1px;
    background: linear-gradient(to right, transparent, rgba(255, 255, 255, 0.1), transparent);
    margin: 1rem 0;
}
.task-detail-bar {
    background: rgba(30, 30, 46, 0.95);
    border: 1px solid rgba(126, 178, 255, 0.3);
    border-radius: 12px;
    padding: 0.75rem 1rem;
    margin-top: 0.75rem;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
}
.task-detail-info {
    flex: 1;
    min-width: 0;
}
.task-detail-time {
    font-size: 1rem;
    font-weight: 600;
    color: #7EB2FF;
}
.task-detail-desc {
    font-size: 0.85rem;
    color: #999;
    white-space: pre-wrap;
    word-wrap: break-word;
    line-height: 1.4;
}
.task-detail-source {
    font-size: 0.75rem;
    color: #7eb2ff;
    margin-top: 0.15rem;
    opacity: 0.8;
}
.task-detail-condition {
    font-size: 0.75rem;
    color: #e8a838;
    margin-top: 0.15rem;
    font-style: italic;
}
.task-detail-note {
    font-size: 0.7rem;
    color: #666;
    margin-top: 0.25rem;
    font-style: italic;
}
.task-btn-delete {
    background: rgba(255, 68, 68, 0.15);
    border: 1px solid rgba(255, 68, 68, 0.4);
    color: #ff6b6b;
    padding: 0.5rem 0.75rem;
    border-radius: 8px;
    cursor: pointer;
    font-size: 0.85rem;
    transition: all 0.2s;
}
.task-btn-delete:hover {
    background: rgba(255, 68, 68, 0.25);
}
.task-btn-close {
    background: transparent;
    border: 1px solid rgba(255, 255, 255, 0.2);
    color: #888;
    padding: 0.5rem 0.75rem;
    border-radius: 8px;
    cursor: pointer;
    font-size: 0.85rem;
}
.task-btn-close:hover {
    color: #fff;
    background: rgba(255, 255, 255, 0.1);
}
.section-label {
    display: flex;
    align-items: center;
    gap: 0.4rem;
}
.section-label span {
    font-size: 0.75rem;
    color: #666;
    text-transform: uppercase;
    letter-spacing: 0.05em;
}
.info-icon-btn {
    background: transparent;
    border: none;
    color: #555;
    font-size: 0.75rem;
    cursor: pointer;
    padding: 0.1rem 0.25rem;
    transition: color 0.2s;
}
.info-icon-btn:hover {
    color: #7EB2FF;
}
.info-modal-overlay {
    position: fixed;
    top: 0; left: 0; right: 0; bottom: 0;
    background: rgba(0,0,0,0.8);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 9999;
}
.info-modal-box {
    background: #1e1e2f;
    border: 1px solid rgba(255,255,255,0.1);
    border-radius: 12px;
    padding: 1.5rem;
    max-width: 440px;
    width: 90%;
    max-height: 80vh;
    overflow-y: auto;
    color: #ddd;
}
.info-modal-box h3 {
    margin: 0 0 0.75rem 0;
    font-size: 1.1rem;
    color: #fff;
}
.info-modal-box h4 {
    margin: 1rem 0 0.35rem 0;
    font-size: 0.9rem;
    color: #7EB2FF;
}
.info-modal-box ul {
    margin: 0;
    padding-left: 1.25rem;
}
.info-modal-box li {
    font-size: 0.8rem;
    color: #aaa;
    margin-bottom: 0.25rem;
    line-height: 1.4;
}
.info-modal-hint {
    font-size: 0.8rem;
    color: #888;
    margin-bottom: 0.75rem;
}
.info-modal-limits {
    margin-top: 1rem;
    padding-top: 0.75rem;
    border-top: 1px solid rgba(255,255,255,0.08);
}
.info-modal-limits p {
    font-size: 0.75rem;
    color: #666;
    margin: 0.2rem 0;
}
.info-modal-section {
    margin-bottom: 0.75rem;
}
.info-modal-section p {
    font-size: 0.8rem;
    color: #aaa;
    margin: 0.25rem 0;
    line-height: 1.4;
}
.info-modal-section strong {
    color: #ccc;
}
.info-modal-divider {
    height: 1px;
    background: rgba(255,255,255,0.08);
    margin: 1rem 0;
}
.info-modal-close {
    display: block;
    margin: 1.25rem auto 0;
    background: transparent;
    border: 1px solid rgba(255,255,255,0.15);
    color: #999;
    padding: 0.4rem 1.25rem;
    border-radius: 6px;
    cursor: pointer;
    font-size: 0.85rem;
}
.info-modal-close:hover {
    color: #ccc;
}
"#;

/// API response types matching backend
#[derive(Clone, PartialEq, Deserialize)]
struct DashboardSummaryResponse {
    attention_count: i32,
    attention_items: Vec<AttentionItemResponse>,
    next_scheduled: Option<ScheduledItemResponse>,
    upcoming_tasks: Vec<UpcomingTaskResponse>,
    #[serde(default)]
    upcoming_digests: Vec<UpcomingDigestResponse>,
    watched_contacts: Vec<WatchedContactResponse>,
    next_digest: Option<NextDigestResponse>,
    quiet_mode: QuietModeResponse,
    sunrise_hour: Option<f32>,
    sunset_hour: Option<f32>,
    /// Tasks beyond the current timeline range (for extend button preview)
    #[serde(default)]
    tasks_beyond: Vec<UpcomingTaskResponse>,
    /// Total count of tasks beyond the timeline range
    #[serde(default)]
    tasks_beyond_count: i32,
}

#[derive(Clone, PartialEq, Deserialize, Default)]
struct QuietModeResponse {
    is_quiet: bool,
    until: Option<i32>,
    until_display: Option<String>,
}

#[derive(Clone, PartialEq, Deserialize)]
struct AttentionItemResponse {
    id: i32,
    item_type: String,
    summary: String,
    timestamp: i32,
    source: Option<String>,
}

#[derive(Clone, PartialEq, Deserialize)]
struct ScheduledItemResponse {
    time_display: String,
    description: String,
    task_id: Option<i32>,
}

#[derive(Clone, PartialEq, Deserialize)]
struct UpcomingTaskResponse {
    task_id: Option<i32>,
    timestamp: i32,
    #[serde(default)]
    trigger_type: String,
    time_display: String,
    description: String,
    #[serde(default)]
    date_display: String,
    #[serde(default)]
    relative_display: String,
    #[serde(default)]
    condition: Option<String>,
    #[serde(default)]
    sources_display: Option<String>,
}

#[derive(Clone, PartialEq, Deserialize)]
struct WatchedContactResponse {
    nickname: String,
    notification_mode: String,
}

#[derive(Clone, PartialEq, Deserialize)]
struct NextDigestResponse {
    time_display: String,
}

#[derive(Clone, PartialEq, Deserialize)]
struct UpcomingDigestResponse {
    #[serde(default)]
    task_id: Option<i32>,
    timestamp: i32,
    time_display: String,
    sources: Option<String>,
}

#[derive(Properties, PartialEq, Clone)]
pub struct DashboardViewProps {
    pub user_profile: UserProfile,
    pub on_profile_update: Callback<UserProfile>,
}

#[function_component(DashboardView)]
pub fn dashboard_view(props: &DashboardViewProps) -> Html {
    // Dashboard summary state
    let summary = use_state(|| None::<DashboardSummaryResponse>);
    let summary_loading = use_state(|| true);

    // YouTube connection state for media panel comments
    let youtube_connected = use_state(|| false);

    // Tesla connection state for shortcut icons
    let tesla_connected = use_state(|| false);

    // Panel visibility state
    let settings_open = use_state(|| false);
    let activity_open = use_state(|| false);
    let settings_initial_tab = use_state(|| SettingsTab::Capabilities);

    // Handle URL parameters for opening settings panel with specific tab
    {
        let settings_open = settings_open.clone();
        let settings_initial_tab = settings_initial_tab.clone();
        use_effect_with_deps(
            move |_| {
                if let Some(window) = web_sys::window() {
                    if let Ok(search) = window.location().search() {
                        if let Ok(params) = web_sys::UrlSearchParams::new_with_str(&search) {
                            // Check for ?settings=capabilities (or other tab names)
                            if let Some(tab) = params.get("settings") {
                                let tab_enum = match tab.to_lowercase().as_str() {
                                    "capabilities" | "connections" => Some(SettingsTab::Capabilities),
                                    "account" => Some(SettingsTab::Account),
                                    "billing" => Some(SettingsTab::Billing),
                                    _ => None,
                                };
                                if let Some(tab) = tab_enum {
                                    settings_initial_tab.set(tab);
                                    settings_open.set(true);
                                    // Clean URL
                                    if let Ok(history) = window.history() {
                                        let _ = history.replace_state_with_url(
                                            &wasm_bindgen::JsValue::NULL,
                                            "",
                                            Some("/"),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                || ()
            },
            (),
        );
    }

    // Listen for custom event from nav Settings button
    {
        let settings_open = settings_open.clone();
        let settings_initial_tab = settings_initial_tab.clone();
        use_effect_with_deps(
            move |_| {
                let settings_open = settings_open.clone();
                let settings_initial_tab = settings_initial_tab.clone();
                let callback = wasm_bindgen::closure::Closure::wrap(Box::new(move || {
                    settings_initial_tab.set(SettingsTab::Account);
                    settings_open.set(true);
                }) as Box<dyn FnMut()>);

                if let Some(window) = web_sys::window() {
                    let _ = window.add_event_listener_with_callback(
                        "open-settings",
                        callback.as_ref().unchecked_ref(),
                    );
                }

                // Return cleanup function
                let cleanup_callback = callback;
                move || {
                    if let Some(window) = web_sys::window() {
                        let _ = window.remove_event_listener_with_callback(
                            "open-settings",
                            cleanup_callback.as_ref().unchecked_ref(),
                        );
                    }
                }
            },
            (),
        );
    }

    // Info modal state
    let show_tasks_info = use_state(|| false);

    // Task detail modal state
    let selected_task = use_state(|| None::<UpcomingTask>);

    // Task preview state (shown below chatbox after creation, before entering edit mode)
    let preview_task = use_state(|| None::<UpcomingTask>);

    // Timeline end timestamp state (default: now + 90 days)
    let now_ts_init = (js_sys::Date::now() / 1000.0) as i32;
    let ninety_days_secs = 90 * 24 * 60 * 60;
    let timeline_end_ts = use_state(move || now_ts_init + ninety_days_secs);

    // Fetch YouTube connection status
    {
        let youtube_connected = youtube_connected.clone();
        use_effect_with_deps(move |_| {
            spawn_local(async move {
                match Api::get("/api/auth/youtube/status").send().await {
                    Ok(response) => {
                        if let Ok(data) = response.json::<serde_json::Value>().await {
                            if let Some(connected) = data.get("connected").and_then(|v| v.as_bool()) {
                                youtube_connected.set(connected);
                            }
                        }
                    }
                    Err(_) => {}
                }
            });
            || ()
        }, ());
    }

    // Fetch Tesla connection status
    {
        let tesla_connected = tesla_connected.clone();
        use_effect_with_deps(move |_| {
            spawn_local(async move {
                match Api::get("/api/auth/tesla/status").send().await {
                    Ok(response) => {
                        if let Ok(data) = response.json::<serde_json::Value>().await {
                            if let Some(connected) = data.get("has_tesla").and_then(|v| v.as_bool()) {
                                tesla_connected.set(connected);
                            }
                        }
                    }
                    Err(_) => {}
                }
            });
            || ()
        }, ());
    }

    // Fetch dashboard summary
    let fetch_summary = {
        let summary = summary.clone();
        let summary_loading = summary_loading.clone();
        let timeline_end_ts = timeline_end_ts.clone();
        Callback::from(move |_: ()| {
            let summary = summary.clone();
            let summary_loading = summary_loading.clone();
            let until = *timeline_end_ts;

            spawn_local(async move {
                let url = format!("/api/dashboard/summary?until={}", until);
                match Api::get(&url).send().await {
                    Ok(response) => {
                        if response.ok() {
                            if let Ok(data) = response.json::<DashboardSummaryResponse>().await {
                                summary.set(Some(data));
                            }
                        }
                    }
                    Err(_) => {}
                }
                summary_loading.set(false);
            });
        })
    };

    // Fetch on mount and after chat
    {
        let fetch_summary = fetch_summary.clone();
        use_effect_with_deps(
            move |_| {
                fetch_summary.emit(());
                || ()
            },
            (),
        );
    }

    // Listen for chat events to refresh summary
    {
        let fetch_summary = fetch_summary.clone();
        use_effect_with_deps(
            move |_| {
                use wasm_bindgen::closure::Closure;
                use wasm_bindgen::JsCast;

                let callback = Closure::wrap(Box::new(move || {
                    fetch_summary.emit(());
                }) as Box<dyn Fn()>);

                if let Some(window) = web_sys::window() {
                    let _ = window.add_event_listener_with_callback(
                        "lightfriend-chat-sent",
                        callback.as_ref().unchecked_ref(),
                    );
                }

                // Return cleanup
                let cleanup_callback = callback;
                move || {
                    if let Some(window) = web_sys::window() {
                        let _ = window.remove_event_listener_with_callback(
                            "lightfriend-chat-sent",
                            cleanup_callback.as_ref().unchecked_ref(),
                        );
                    }
                }
            },
            (),
        );
    }

    // Convert API response to component props
    let (attention_count, attention_items) = match (*summary).as_ref() {
        Some(s) => (
            s.attention_count,
            s.attention_items
                .iter()
                .map(|item| TriageAttentionItem {
                    id: item.id,
                    item_type: item.item_type.clone(),
                    summary: item.summary.clone(),
                    timestamp: item.timestamp,
                    source: item.source.clone(),
                })
                .collect(),
        ),
        None => (0, vec![]),
    };

    let upcoming_tasks: Vec<UpcomingTask> = (*summary)
        .as_ref()
        .map(|s| {
            s.upcoming_tasks
                .iter()
                .map(|t| UpcomingTask {
                    task_id: t.task_id,
                    timestamp: t.timestamp,
                    trigger_type: t.trigger_type.clone(),
                    time_display: t.time_display.clone(),
                    description: t.description.clone(),
                    date_display: t.date_display.clone(),
                    relative_display: t.relative_display.clone(),
                    condition: t.condition.clone(),
                    sources_display: t.sources_display.clone(),
                })
                .collect()
        })
        .unwrap_or_default();

    let upcoming_digests: Vec<UpcomingDigest> = (*summary)
        .as_ref()
        .map(|s| {
            s.upcoming_digests
                .iter()
                .map(|d| UpcomingDigest {
                    task_id: d.task_id,
                    timestamp: d.timestamp,
                    time_display: d.time_display.clone(),
                    sources: d.sources.clone(),
                })
                .collect()
        })
        .unwrap_or_default();


    // Update selected_task with fresh data when summary changes
    {
        let selected_task = selected_task.clone();
        let upcoming_tasks = upcoming_tasks.clone();
        use_effect_with_deps(
            move |tasks: &Vec<UpcomingTask>| {
                if let Some(current) = (*selected_task).as_ref() {
                    if let Some(task_id) = current.task_id {
                        // Find the updated task in the new data
                        if let Some(updated) = tasks.iter().find(|t| t.task_id == Some(task_id)) {
                            selected_task.set(Some(updated.clone()));
                        }
                    }
                }
                || ()
            },
            upcoming_tasks,
        );
    }

    // Get current timestamp for timeline
    let now_timestamp = (js_sys::Date::now() / 1000.0) as i32;

    let next_digest = (*summary).as_ref().and_then(|s| {
        s.next_digest.as_ref().map(|d| NextDigestInfo {
            time_display: d.time_display.clone(),
        })
    });

    let quiet_mode = (*summary)
        .as_ref()
        .map(|s| QuietModeStatus {
            is_quiet: s.quiet_mode.is_quiet,
            until: s.quiet_mode.until,
            until_display: s.quiet_mode.until_display.clone(),
        })
        .unwrap_or_default();

    // Extract sunrise/sunset hours for timeline
    let sunrise_hour = (*summary).as_ref().and_then(|s| s.sunrise_hour);
    let sunset_hour = (*summary).as_ref().and_then(|s| s.sunset_hour);

    // Extract quiet_until for timeline visualization (only if is_quiet is true)
    let quiet_until = if quiet_mode.is_quiet {
        quiet_mode.until
    } else {
        None
    };

    // Callbacks for footer buttons
    let on_quiet_mode_change = {
        let fetch_summary = fetch_summary.clone();
        Callback::from(move |_: ()| {
            fetch_summary.emit(());
        })
    };

    let on_activity_click = {
        let activity_open = activity_open.clone();
        Callback::from(move |_| {
            activity_open.set(true);
        })
    };

    let on_settings_close = {
        let settings_open = settings_open.clone();
        Callback::from(move |_| {
            settings_open.set(false);
        })
    };

    let on_activity_close = {
        let activity_open = activity_open.clone();
        Callback::from(move |_| {
            activity_open.set(false);
        })
    };

    // Task click callback for timeline
    let on_task_click = {
        let selected_task = selected_task.clone();
        Callback::from(move |task: UpcomingTask| {
            selected_task.set(Some(task));
        })
    };

    // Digest click callback - convert to UpcomingTask for editing
    let on_digest_click = {
        let selected_task = selected_task.clone();
        Callback::from(move |digest: UpcomingDigest| {
            // Convert digest to UpcomingTask for the edit UI
            let task = UpcomingTask {
                task_id: digest.task_id,
                timestamp: digest.timestamp,
                trigger_type: "once".to_string(),
                time_display: digest.time_display.clone(),
                description: format!("Digest: {}", digest.sources.as_deref().unwrap_or("all sources")),
                date_display: String::new(),
                relative_display: String::new(),
                condition: None,
                sources_display: None,
            };
            selected_task.set(Some(task));
        })
    };

    // Close task modal callback
    let on_task_modal_close = {
        let selected_task = selected_task.clone();
        Callback::from(move |_: MouseEvent| {
            selected_task.set(None);
        })
    };

    // Delete task callback
    let on_delete_task = {
        let selected_task = selected_task.clone();
        let fetch_summary = fetch_summary.clone();
        Callback::from(move |_: MouseEvent| {
            if let Some(task) = (*selected_task).as_ref() {
                if let Some(task_id) = task.task_id {
                    let selected_task = selected_task.clone();
                    let fetch_summary = fetch_summary.clone();
                    spawn_local(async move {
                        if let Ok(resp) = Api::delete(&format!("/api/tasks/{}", task_id)).send().await {
                            if resp.ok() {
                                selected_task.set(None);
                                fetch_summary.emit(());
                            }
                        }
                    });
                }
            }
        })
    };

    // Callback for when task is cleared after editing
    let on_task_cleared = {
        let selected_task = selected_task.clone();
        let fetch_summary = fetch_summary.clone();
        Callback::from(move |_: ()| {
            selected_task.set(None);
            fetch_summary.emit(());
        })
    };

    // Callback for usage changes (refresh summary after chat)
    let on_usage_change = fetch_summary.clone();


    // Callback for when a task is created via chat - show preview below chatbox
    let on_task_created = {
        let preview_task = preview_task.clone();
        let fetch_summary = fetch_summary.clone();
        Callback::from(move |task_id: i32| {
            // Refresh the dashboard to get the new task
            fetch_summary.emit(());

            // Schedule a check after a short delay to find and show preview
            let preview_task = preview_task.clone();
            gloo_timers::callback::Timeout::new(500, move || {
                let preview_task = preview_task.clone();
                spawn_local(async move {
                    if let Ok(response) = Api::get(&format!("/api/tasks/{}", task_id)).send().await {
                        if response.ok() {
                            if let Ok(task_data) = response.json::<serde_json::Value>().await {
                                let task = UpcomingTask {
                                    task_id: task_data["id"].as_i64().map(|i| i as i32),
                                    timestamp: task_data["trigger_timestamp"].as_i64().unwrap_or(0) as i32,
                                    trigger_type: task_data["trigger_type"].as_str().unwrap_or("once").to_string(),
                                    time_display: task_data["time_display"].as_str().unwrap_or("").to_string(),
                                    description: task_data["description"].as_str().unwrap_or("").to_string(),
                                    date_display: task_data["date_display"].as_str().unwrap_or("").to_string(),
                                    relative_display: task_data["relative_display"].as_str().unwrap_or("").to_string(),
                                    condition: task_data["condition"].as_str().map(|s| s.to_string()),
                                    sources_display: task_data["sources_display"].as_str().map(|s| s.to_string()),
                                };
                                preview_task.set(Some(task));
                            }
                        }
                    }
                });
            }).forget();
        })
    };

    // Callback for when user clicks on task preview to edit it
    let on_preview_click = {
        let selected_task = selected_task.clone();
        let preview_task = preview_task.clone();
        Callback::from(move |task: UpcomingTask| {
            selected_task.set(Some(task));
            preview_task.set(None);
        })
    };

    // Callback to close task preview
    let on_preview_close = {
        let preview_task = preview_task.clone();
        Callback::from(move |_: ()| {
            preview_task.set(None);
        })
    };

    html! {
        <>
            <style>{DASHBOARD_STYLES}</style>
            <div class="peace-dashboard">
                // Overlay for clicking outside to close task edit mode
                if selected_task.is_some() {
                    <div class="task-focus-overlay" onclick={on_task_modal_close.clone()}></div>
                }

                // Chat box and task bar in a container above the overlay
                <div class={if selected_task.is_some() { "task-edit-container" } else { "" }}>
                    // Chat box - always at the top, pass focused_task for edit mode
                    <ChatBox
                        on_usage_change={on_usage_change}
                        youtube_connected={*youtube_connected}
                        tesla_connected={*tesla_connected}
                        focused_task={(*selected_task).clone()}
                        on_task_cleared={on_task_cleared}
                        on_task_created={on_task_created}
                        preview_task={(*preview_task).clone()}
                        on_preview_click={on_preview_click}
                        on_preview_close={on_preview_close}
                    />

                    // Task detail bar (shown when task selected) - below ChatBox
                    if let Some(task) = (*selected_task).as_ref() {
                        <div class="task-detail-bar">
                            <div class="task-detail-info">
                                <div class="task-detail-time">{&task.time_display}</div>
                                if let Some(ref src) = task.sources_display {
                                    <div class="task-detail-source">{format!("Check: {}", src)}</div>
                                    if src.to_lowercase().contains("weather") {
                                        <div class="task-detail-note">{"Location from Settings > Account"}</div>
                                    }
                                }
                                if let Some(ref cond) = task.condition {
                                    <div class="task-detail-condition">{format!("Condition: {}", cond)}</div>
                                }
                                <div class="task-detail-desc">
                                    {if task.condition.is_some() || task.sources_display.is_some() {
                                        format!("Then: {}", &task.description)
                                    } else {
                                        task.description.clone()
                                    }}
                                </div>
                                if task.trigger_type != "reminder" {
                                    <div class="task-detail-note">{"You'll be notified when this task runs"}</div>
                                }
                            </div>
                            <button class="task-btn-delete" onclick={on_delete_task}>{"Delete"}</button>
                            <button class="task-btn-close" onclick={on_task_modal_close.clone()}>{"x"}</button>
                        </div>
                    }
                </div>

            // Main dashboard content - blurred when task focused
            <div class={if selected_task.is_some() { "peace-main task-focused" } else { "peace-main" }}>
                // Triage indicator (admin-only for now)
                { if props.user_profile.id == 1 {
                    html! {
                        <TriageIndicator
                            attention_count={attention_count}
                            attention_items={attention_items}
                        />
                    }
                } else {
                    html! {}
                }}

                // Tasks section label with info icon
                <div class="section-label">
                    <span>{"Tasks"}</span>
                    <button class="info-icon-btn" onclick={{
                        let show_tasks_info = show_tasks_info.clone();
                        Callback::from(move |_: MouseEvent| show_tasks_info.set(true))
                    }}>
                        <i class="fa-solid fa-circle-info"></i>
                    </button>
                </div>

                // Timeline view showing upcoming tasks and digests
                <TimelineView
                    upcoming_tasks={upcoming_tasks}
                    upcoming_digests={upcoming_digests}
                    now_timestamp={now_timestamp}
                    on_task_click={on_task_click}
                    on_digest_click={on_digest_click}
                    sunrise_hour={sunrise_hour}
                    sunset_hour={sunset_hour}
                    quiet_until={quiet_until}
                />

                // Tasks info modal
                if *show_tasks_info {
                    <div class="info-modal-overlay" onclick={{
                        let show_tasks_info = show_tasks_info.clone();
                        Callback::from(move |_: MouseEvent| show_tasks_info.set(false))
                    }}>
                        <div class="info-modal-box" onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                            <h3>{"Tasks"}</h3>
                            <p class="info-modal-hint">{"Create tasks by describing them in the chat. Each task you see on the timeline was created by you."}</p>

                            <h4>{"How Tasks Work"}</h4>
                            <div class="info-modal-section">
                                <p>{"When you create a task, the following is saved: the schedule (when to trigger), an optional condition (checked at trigger time), an action (what to do), which sources to check (email, WhatsApp, etc.), and how to notify you."}</p>
                                <p><strong>{"Fixed at creation: "}</strong>{"The schedule, recurrence rule, action to run, and which sources to check. These don't change between runs."}</p>
                                <p><strong>{"Dynamic at trigger time: "}</strong>{"Source data is fetched fresh, conditions are evaluated against live data, and the notification message is generated from current results."}</p>
                            </div>

                            <h4>{"Trigger Types"}</h4>
                            <div class="info-modal-section">
                                <p><strong>{"Deterministic: "}</strong>{"Reminders and digests always fire at their scheduled time. If you set a reminder for 3pm, it will notify you at 3pm - no AI decision involved."}</p>
                                <p><strong>{"Probabilistic: "}</strong>{"Conditional tasks (e.g. \"if it's going to rain\") depend on AI evaluation of live data. The trigger time is fixed, but whether the action runs depends on whether the condition matches."}</p>
                                <p><strong>{"Event-driven: "}</strong>{"Message monitoring and email watching fire whenever a new message or email arrives, then evaluate any conditions against that content."}</p>
                            </div>

                            <h4>{"Safety by Design"}</h4>
                            <div class="info-modal-section">
                                <p>{"Lightfriend can only run a fixed set of actions: send you a notification, generate a digest, send a message to a specific contact, check weather, fetch calendar events, or control Tesla. No other actions exist - this is hardcoded, not AI-decided."}</p>
                                <p>{"Tasks can only be created by you. Lightfriend cannot create, modify, or chain tasks on its own. Every task on your timeline is one you explicitly asked for."}</p>
                                <p>{"Conditions are evaluated (matched true/false against data), never executed. Even if source data contained malicious instructions, the system only asks \"does this match the condition?\" - it cannot run new actions based on message content."}</p>
                                <p>{"This means the risk of any task is knowable at creation time: you can see exactly what it will check, what action it will take, and when."}</p>
                            </div>

                            <div class="info-modal-divider"></div>

                            <h4>{"Examples"}</h4>
                            <p class="info-modal-hint">{"Describe any of these in the chat to create a task"}</p>
                            <h4>{"Reminders & Scheduling"}</h4>
                            <ul>
                                <li>{"\"Remind me at 3pm to call mom\""}</li>
                                <li>{"\"Remind me every Monday at 9am to submit the weekly report\""}</li>
                                <li>{"\"Tell me to take my medicine every day at 8am and 8pm\""}</li>
                            </ul>
                            <h4>{"Digests"}</h4>
                            <ul>
                                <li>{"\"Send me a daily digest at 8am with my emails and WhatsApp messages\""}</li>
                                <li>{"\"Give me a morning briefing at 7am with weather, calendar, and emails\""}</li>
                            </ul>
                            <h4>{"Message & Email Monitoring"}</h4>
                            <ul>
                                <li>{"\"Let me know when mom texts me on WhatsApp\""}</li>
                                <li>{"\"Notify me if I get a message from my boss on Telegram\""}</li>
                                <li>{"\"Tell me when I get an email about my job application\""}</li>
                            </ul>
                            <h4>{"Conditional Tasks"}</h4>
                            <ul>
                                <li>{"\"If it's above 25 degrees at 8am, remind me to water the plants\""}</li>
                                <li>{"\"Check the weather at 7am - if it's going to rain, remind me to bring an umbrella\""}</li>
                            </ul>
                            <h4>{"Smart Home / Tesla"}</h4>
                            <ul>
                                <li>{"\"Turn on Tesla climate at 7:30am every weekday\""}</li>
                                <li>{"\"Start warming up my car in 20 minutes\""}</li>
                            </ul>
                            <h4>{"Calendar"}</h4>
                            <ul>
                                <li>{"\"Send me my calendar events every morning at 7am\""}</li>
                            </ul>

                            <div class="info-modal-limits">
                                <p>{"Can't make purchases or payments"}</p>
                                <p>{"Can't access apps not connected in Capabilities"}</p>
                                <p>{"Can't take actions you didn't explicitly create a task for"}</p>
                            </div>
                            <button class="info-modal-close" onclick={{
                                let show_tasks_info = show_tasks_info.clone();
                                Callback::from(move |_: MouseEvent| show_tasks_info.set(false))
                            }}>{"Close"}</button>
                        </div>
                    </div>
                }

                // Horizontal separator
                <div class="peace-separator"></div>

                // Footer with watching info and buttons
                <DashboardFooter
                    next_digest={next_digest}
                    quiet_mode={quiet_mode}
                    on_activity_click={on_activity_click}
                    on_quiet_mode_change={Some(on_quiet_mode_change)}
                />
            </div>

            // Settings panel (slide-in)
            <SettingsPanel
                is_open={*settings_open}
                user_profile={Some(props.user_profile.clone())}
                on_close={on_settings_close}
                on_profile_update={props.on_profile_update.clone()}
                initial_tab={*settings_initial_tab}
            />

            // Activity panel (slide-in)
            <ActivityPanel
                is_open={*activity_open}
                on_close={on_activity_close}
            />

            </div>
        </>
    }
}
