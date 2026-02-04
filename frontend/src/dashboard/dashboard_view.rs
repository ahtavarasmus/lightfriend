use yew::prelude::*;
use wasm_bindgen_futures::spawn_local;
use serde::Deserialize;
use crate::utils::api::Api;
use crate::profile::billing_models::UserProfile;

use super::chat_box::ChatBox;
use super::triage_indicator::{TriageIndicator, AttentionItem as TriageAttentionItem};
use super::timeline_view::{TimelineView, UpcomingTask, UpcomingDigest};
use super::dashboard_footer::{DashboardFooter, WatchedContact, NextDigestInfo};
use super::settings_panel::SettingsPanel;
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
    time_display: String,
    description: String,
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

    // Panel visibility state
    let settings_open = use_state(|| false);
    let activity_open = use_state(|| false);

    // Task detail modal state
    let selected_task = use_state(|| None::<UpcomingTask>);

    // Fetch YouTube connection status
    {
        let youtube_connected = youtube_connected.clone();
        use_effect_with_deps(move |_| {
            spawn_local(async move {
                match Api::get("/api/auth/youtube/status").send().await {
                    Ok(response) => {
                        let status = response.status();
                        match response.json::<serde_json::Value>().await {
                            Ok(data) => {
                                web_sys::console::log_1(&format!("YouTube status response: {:?}", data).into());
                                if let Some(connected) = data.get("connected").and_then(|v| v.as_bool()) {
                                    web_sys::console::log_1(&format!("YouTube connected: {}", connected).into());
                                    youtube_connected.set(connected);
                                }
                            }
                            Err(e) => {
                                web_sys::console::error_1(&format!("Failed to parse YouTube status: {:?}", e).into());
                            }
                        }
                    }
                    Err(e) => {
                        web_sys::console::error_1(&format!("Failed to fetch YouTube status: {:?}", e).into());
                    }
                }
            });
            || ()
        }, ());
    }

    // Fetch dashboard summary
    let fetch_summary = {
        let summary = summary.clone();
        let summary_loading = summary_loading.clone();
        Callback::from(move |_: ()| {
            let summary = summary.clone();
            let summary_loading = summary_loading.clone();

            spawn_local(async move {
                match Api::get("/api/dashboard/summary").send().await {
                    Ok(response) => {
                        if response.ok() {
                            if let Ok(data) = response.json::<DashboardSummaryResponse>().await {
                                summary.set(Some(data));
                            }
                        }
                    }
                    Err(_) => {
                        // Silently handle error - show empty state
                    }
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
                    time_display: t.time_display.clone(),
                    description: t.description.clone(),
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

    let watched_contacts: Vec<WatchedContact> = (*summary)
        .as_ref()
        .map(|s| {
            s.watched_contacts
                .iter()
                .map(|c| WatchedContact {
                    nickname: c.nickname.clone(),
                    notification_mode: c.notification_mode.clone(),
                })
                .collect()
        })
        .unwrap_or_default();

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

    // Callbacks for footer buttons
    let on_settings_click = {
        let settings_open = settings_open.clone();
        Callback::from(move |_| {
            settings_open.set(true);
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
                        focused_task={(*selected_task).clone()}
                        on_task_cleared={on_task_cleared}
                    />

                    // Task detail bar (shown when task selected) - below ChatBox
                    if let Some(task) = (*selected_task).as_ref() {
                        <div class="task-detail-bar">
                            <div class="task-detail-info">
                                <div class="task-detail-time">{&task.time_display}</div>
                                <div class="task-detail-desc">{&task.description}</div>
                                <div class="task-detail-note">{"You'll be notified when this task runs"}</div>
                            </div>
                            <button class="task-btn-delete" onclick={on_delete_task}>{"Delete"}</button>
                            <button class="task-btn-close" onclick={on_task_modal_close.clone()}>{"x"}</button>
                        </div>
                    }
                </div>

            // Main dashboard content - blurred when task focused
            <div class={if selected_task.is_some() { "peace-main task-focused" } else { "peace-main" }}>
                // Triage indicator
                <TriageIndicator
                    attention_count={attention_count}
                    attention_items={attention_items}
                />

                // Timeline view showing upcoming tasks and digests
                <TimelineView
                    upcoming_tasks={upcoming_tasks}
                    upcoming_digests={upcoming_digests}
                    now_timestamp={now_timestamp}
                    on_task_click={on_task_click}
                    sunrise_hour={sunrise_hour}
                    sunset_hour={sunset_hour}
                />

                // Horizontal separator
                <div class="peace-separator"></div>

                // Footer with watching info and buttons
                <DashboardFooter
                    watched_contacts={watched_contacts}
                    next_digest={next_digest}
                    quiet_mode={quiet_mode}
                    on_settings_click={on_settings_click}
                    on_activity_click={on_activity_click}
                />
            </div>

            // Settings panel (slide-in)
            <SettingsPanel
                is_open={*settings_open}
                user_profile={Some(props.user_profile.clone())}
                on_close={on_settings_close}
                on_profile_update={props.on_profile_update.clone()}
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
