use yew::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use serde::Deserialize;
use crate::utils::api::Api;
use crate::profile::billing_models::UserProfile;

use super::chat_box::ChatBox;
use super::triage_indicator::AttentionItem;
use super::timeline_view::{UpcomingItem, UpcomingDigest};
use super::settings_panel::{SettingsPanel, SettingsTab};
use super::contact_avatar_row::ContactAvatarRow;
use super::quiet_mode::QuietModeStatus;
use super::items_status::{ItemsStatusSection, digest_sources};

fn format_date_from_ts(ts: i32) -> String {
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(ts as f64 * 1000.0));
    let months = ["Jan","Feb","Mar","Apr","May","Jun","Jul","Aug","Sep","Oct","Nov","Dec"];
    let month = months.get(date.get_month() as usize).unwrap_or(&"");
    format!("{} {}", month, date.get_date())
}

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
.item-focus-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.3);
    z-index: 100;
    cursor: pointer;
}
.item-edit-container {
    position: relative;
    z-index: 101;
}
.peace-main {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
    transition: all 0.2s ease;
}
.peace-main.item-focused {
    filter: blur(4px);
    opacity: 0.6;
}
.peace-separator {
    height: 1px;
    background: linear-gradient(to right, transparent, rgba(255, 255, 255, 0.1), transparent);
    margin: 1rem 0;
}
.item-detail-bar {
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
.item-detail-info {
    flex: 1;
    min-width: 0;
}
.item-detail-time {
    font-size: 1rem;
    font-weight: 600;
    color: #7EB2FF;
}
.item-detail-desc {
    font-size: 0.85rem;
    color: #999;
    white-space: pre-wrap;
    word-wrap: break-word;
    line-height: 1.4;
}
.item-detail-source {
    font-size: 0.75rem;
    color: #7eb2ff;
    margin-top: 0.15rem;
    opacity: 0.8;
}
.item-detail-meta {
    display: flex;
    gap: 0.4rem;
    margin-top: 0.15rem;
    flex-wrap: wrap;
}
.item-detail-meta span {
    font-size: 0.7rem;
    padding: 0.1rem 0.4rem;
    border-radius: 0.25rem;
    background: rgba(255,255,255,0.08);
    color: #aaa;
}
.item-detail-tracking {
    color: #7eb2ff !important;
}
.item-detail-notify {
    color: #e8a838 !important;
}
.item-detail-note {
    font-size: 0.7rem;
    color: #666;
    margin-top: 0.25rem;
    font-style: italic;
}
.item-btn-delete {
    background: rgba(255, 68, 68, 0.15);
    border: 1px solid rgba(255, 68, 68, 0.4);
    color: #ff6b6b;
    padding: 0.5rem 0.75rem;
    border-radius: 8px;
    cursor: pointer;
    font-size: 0.85rem;
    transition: all 0.2s;
}
.item-btn-delete:hover {
    background: rgba(255, 68, 68, 0.25);
}
.item-btn-close {
    background: transparent;
    border: 1px solid rgba(255, 255, 255, 0.2);
    color: #888;
    padding: 0.5rem 0.75rem;
    border-radius: 8px;
    cursor: pointer;
    font-size: 0.85rem;
}
.item-btn-close:hover {
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
.lf-number-label {
    font-size: 0.75rem;
    color: #666;
    text-align: center;
    margin-bottom: 0.35rem;
    letter-spacing: 0.02em;
}
"#;

/// API response types matching backend
#[derive(Clone, PartialEq, Deserialize)]
struct DashboardSummaryResponse {
    attention_count: i32,
    attention_items: Vec<AttentionItemResponse>,
    next_scheduled: Option<ScheduledItemResponse>,
    upcoming_items: Vec<UpcomingItemResponse>,
    #[serde(default)]
    upcoming_digests: Vec<UpcomingDigestResponse>,
    watched_contacts: Vec<WatchedContactResponse>,
    quiet_mode: QuietModeResponse,
    sunrise_hour: Option<f32>,
    sunset_hour: Option<f32>,
    /// Items beyond the current timeline range (for extend button preview)
    #[serde(default)]
    items_beyond: Vec<UpcomingItemResponse>,
    /// Total count of items beyond the timeline range
    #[serde(default)]
    items_beyond_count: i32,
    /// Total number of tracked items
    #[serde(default)]
    total_tracked_count: i32,
}

#[derive(Clone, PartialEq, Deserialize, Default)]
struct QuietModeResponse {
    is_quiet: bool,
    until: Option<i32>,
    until_display: Option<String>,
    #[serde(default)]
    rule_count: i32,
}

#[derive(Clone, PartialEq, Deserialize)]
struct AttentionItemResponse {
    id: i32,
    item_type: String,
    summary: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    priority: i32,
    #[serde(default)]
    due_at: Option<i32>,
    source: Option<String>,
    #[serde(default)]
    source_id: Option<String>,
    #[serde(default)]
    notify: Option<String>,
    #[serde(default)]
    sender: Option<String>,
    #[serde(default)]
    platform: Option<String>,
    #[serde(default)]
    time_display: Option<String>,
    #[serde(default)]
    relative_display: Option<String>,
}

#[derive(Clone, PartialEq, Deserialize)]
struct ScheduledItemResponse {
    time_display: String,
    description: String,
    item_id: Option<i32>,
}

#[derive(Clone, PartialEq, Deserialize)]
struct UpcomingItemResponse {
    item_id: Option<i32>,
    timestamp: i32,
    time_display: String,
    description: String,
    #[serde(default)]
    date_display: String,
    #[serde(default)]
    relative_display: String,
    #[serde(default)]
    item_type: Option<String>,
    #[serde(default)]
    notify: Option<String>,
    #[serde(default)]
    sources_display: Option<String>,
}

#[derive(Clone, PartialEq, Deserialize)]
struct WatchedContactResponse {
    nickname: String,
    notification_mode: String,
}

#[derive(Clone, PartialEq, Deserialize)]
struct UpcomingDigestResponse {
    #[serde(default)]
    item_id: Option<i32>,
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

    // Item detail modal state
    let selected_item = use_state(|| None::<UpcomingItem>);

    // Item preview state (shown below chatbox after creation, before entering edit mode)
    let preview_item = use_state(|| None::<UpcomingItem>);

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
        Callback::from(move |_: ()| {
            let summary = summary.clone();
            let summary_loading = summary_loading.clone();
            let until = (js_sys::Date::now() / 1000.0) as i32 + 90 * 24 * 60 * 60;

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
    let (_attention_count, attention_items, total_tracked_count) = match (*summary).as_ref() {
        Some(s) => (
            s.attention_count,
            s.attention_items
                .iter()
                .map(|item| AttentionItem {
                    id: item.id,
                    item_type: item.item_type.clone(),
                    summary: item.summary.clone(),
                    description: item.description.clone(),
                    priority: item.priority,
                    due_at: item.due_at,
                    source: item.source.clone(),
                    source_id: item.source_id.clone(),
                    notify: item.notify.clone(),
                    sender: item.sender.clone(),
                    platform: item.platform.clone(),
                    time_display: item.time_display.clone(),
                    relative_display: item.relative_display.clone(),
                })
                .collect(),
            s.total_tracked_count,
        ),
        None => (0, vec![], 0),
    };

    let upcoming_items: Vec<UpcomingItem> = (*summary)
        .as_ref()
        .map(|s| {
            s.upcoming_items
                .iter()
                .map(|t| UpcomingItem {
                    item_id: t.item_id,
                    timestamp: t.timestamp,
                    time_display: t.time_display.clone(),
                    description: t.description.clone(),
                    date_display: t.date_display.clone(),
                    relative_display: t.relative_display.clone(),
                    item_type: t.item_type.clone(),
                    notify: t.notify.clone(),
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
                    item_id: d.item_id,
                    timestamp: d.timestamp,
                    time_display: d.time_display.clone(),
                    sources: d.sources.clone(),
                })
                .collect()
        })
        .unwrap_or_default();


    // Update selected_item with fresh data when summary changes (check both items and digests)
    {
        let selected_item = selected_item.clone();
        let upcoming_items_for_effect = upcoming_items.clone();
        let upcoming_digests_for_effect = upcoming_digests.clone();
        use_effect_with_deps(
            move |(items, digests): &(Vec<UpcomingItem>, Vec<UpcomingDigest>)| {
                if let Some(current) = (*selected_item).as_ref() {
                    if let Some(id) = current.item_id {
                        if let Some(updated) = items.iter().find(|t| t.item_id == Some(id)) {
                            selected_item.set(Some(updated.clone()));
                        } else if let Some(updated_digest) = digests.iter().find(|d| d.item_id == Some(id)) {
                            let item = UpcomingItem {
                                item_id: updated_digest.item_id,
                                timestamp: updated_digest.timestamp,
                                time_display: updated_digest.time_display.clone(),
                                description: format!("Digest: {}", updated_digest.sources.as_deref().unwrap_or("all sources")),
                                date_display: String::new(),
                                relative_display: String::new(),
                                item_type: Some("recurring".to_string()),
                                notify: None,
                                sources_display: None,
                            };
                            selected_item.set(Some(item));
                        }
                    }
                }
                || ()
            },
            (upcoming_items_for_effect, upcoming_digests_for_effect),
        );
    }

    let quiet_mode = (*summary)
        .as_ref()
        .map(|s| QuietModeStatus {
            is_quiet: s.quiet_mode.is_quiet,
            until: s.quiet_mode.until,
            until_display: s.quiet_mode.until_display.clone(),
            rule_count: s.quiet_mode.rule_count,
        })
        .unwrap_or_default();

    // Extract sunrise/sunset hours for timeline
    let sunrise_hour = (*summary).as_ref().and_then(|s| s.sunrise_hour);
    let sunset_hour = (*summary).as_ref().and_then(|s| s.sunset_hour);

    // Callbacks for footer buttons
    let on_quiet_mode_change = {
        let fetch_summary = fetch_summary.clone();
        Callback::from(move |_: ()| {
            fetch_summary.emit(());
        })
    };

    let on_settings_close = {
        let settings_open = settings_open.clone();
        Callback::from(move |_| {
            settings_open.set(false);
        })
    };

    // Item delete callback
    let on_activity_item_delete = {
        let fetch_summary = fetch_summary.clone();
        Callback::from(move |id: i32| {
            let fetch_summary = fetch_summary.clone();
            spawn_local(async move {
                if let Ok(resp) = Api::delete(&format!("/api/items/{}", id)).send().await {
                    if resp.ok() {
                        fetch_summary.emit(());
                    }
                }
            });
        })
    };

    // Close item modal callback
    let on_item_modal_close = {
        let selected_item = selected_item.clone();
        Callback::from(move |_: MouseEvent| {
            selected_item.set(None);
        })
    };

    // Delete item callback
    let on_delete_item = {
        let selected_item = selected_item.clone();
        let fetch_summary = fetch_summary.clone();
        Callback::from(move |_: MouseEvent| {
            if let Some(item) = (*selected_item).as_ref() {
                if let Some(id) = item.item_id {
                    let selected_item = selected_item.clone();
                    let fetch_summary = fetch_summary.clone();
                    spawn_local(async move {
                        if let Ok(resp) = Api::delete(&format!("/api/items/{}", id)).send().await {
                            if resp.ok() {
                                selected_item.set(None);
                                fetch_summary.emit(());
                            }
                        }
                    });
                }
            }
        })
    };

    // Callback for when item is cleared after editing
    let on_item_cleared = {
        let selected_item = selected_item.clone();
        let fetch_summary = fetch_summary.clone();
        Callback::from(move |_: ()| {
            selected_item.set(None);
            fetch_summary.emit(());
        })
    };

    // Dismiss item callback
    let on_dismiss_item = {
        let fetch_summary = fetch_summary.clone();
        Callback::from(move |item: AttentionItem| {
            let fetch_summary = fetch_summary.clone();
            spawn_local(async move {
                let url = format!("/api/items/{}", item.id);
                if let Ok(resp) = Api::delete(&url).send().await {
                    if resp.ok() {
                        fetch_summary.emit(());
                    }
                }
            });
        })
    };

    // Chat prefill state (for digest suggestion hint)
    let prefill_chat: UseStateHandle<Option<String>> = use_state(|| None);

    // Digest prefill callback - pre-fills chatbox with prompt from digest creator
    let on_digest_prefill = {
        let prefill_chat = prefill_chat.clone();
        Callback::from(move |prompt: String| {
            prefill_chat.set(Some(prompt));
        })
    };

    // Callback to clear prefill after it's consumed
    let on_prefill_consumed = {
        let prefill_chat = prefill_chat.clone();
        Callback::from(move |_: ()| {
            prefill_chat.set(None);
        })
    };

    // Listen for onboarding prefill events
    {
        let prefill_chat = prefill_chat.clone();
        use_effect_with_deps(
            move |_| {
                use wasm_bindgen::closure::Closure;
                use wasm_bindgen::JsCast;

                let callback = Closure::wrap(Box::new(move |e: web_sys::CustomEvent| {
                    if let Some(detail) = e.detail().as_string() {
                        prefill_chat.set(Some(detail));
                    }
                }) as Box<dyn Fn(web_sys::CustomEvent)>);

                if let Some(window) = web_sys::window() {
                    let _ = window.add_event_listener_with_callback(
                        "lightfriend-prefill-chat",
                        callback.as_ref().unchecked_ref(),
                    );
                }

                let cleanup_callback = callback;
                move || {
                    if let Some(window) = web_sys::window() {
                        let _ = window.remove_event_listener_with_callback(
                            "lightfriend-prefill-chat",
                            cleanup_callback.as_ref().unchecked_ref(),
                        );
                    }
                }
            },
            (),
        );
    }

    // Callback for usage changes (refresh summary after chat)
    let on_usage_change = fetch_summary.clone();

    // Callback for clicking items in ItemsStatusSection - open edit modal
    let on_item_status_click = {
        let selected_item = selected_item.clone();
        Callback::from(move |item: AttentionItem| {
            let sources = digest_sources(&item.summary, &item.description);
            selected_item.set(Some(UpcomingItem {
                item_id: Some(item.id),
                timestamp: item.due_at.unwrap_or(0),
                time_display: item.time_display.clone().unwrap_or_default(),
                description: item.description.clone(),
                date_display: item.due_at.map(format_date_from_ts).unwrap_or_default(),
                relative_display: item.relative_display.clone().unwrap_or_default(),
                item_type: Some(item.item_type.clone()),
                notify: item.notify.clone(),
                sources_display: sources,
            }));
        })
    };

    // Callback for when an item is created via chat - show preview below chatbox
    let on_item_created = {
        let preview_item = preview_item.clone();
        let fetch_summary = fetch_summary.clone();
        Callback::from(move |item_id: i32| {
            // Refresh the dashboard to get the new item
            fetch_summary.emit(());

            // Schedule a check after a short delay to find and show preview
            let preview_item = preview_item.clone();
            gloo_timers::callback::Timeout::new(500, move || {
                let preview_item = preview_item.clone();
                spawn_local(async move {
                    if let Ok(response) = Api::get(&format!("/api/items/{}", item_id)).send().await {
                        if response.ok() {
                            if let Ok(data) = response.json::<serde_json::Value>().await {
                                let item = UpcomingItem {
                                    item_id: data["id"].as_i64().map(|i| i as i32),
                                    timestamp: data["trigger_timestamp"].as_i64().unwrap_or(0) as i32,
                                    time_display: data["time_display"].as_str().unwrap_or("").to_string(),
                                    description: data["description"].as_str().unwrap_or("").to_string(),
                                    date_display: data["date_display"].as_str().unwrap_or("").to_string(),
                                    relative_display: data["relative_display"].as_str().unwrap_or("").to_string(),
                                    item_type: data["item_type"].as_str().map(|s| s.to_string()),
                                    notify: data["notify"].as_str().map(|s| s.to_string()),
                                    sources_display: data["sources_display"].as_str().map(|s| s.to_string()),
                                };
                                preview_item.set(Some(item));
                            }
                        }
                    }
                });
            }).forget();
        })
    };

    // Callback for when user clicks on item preview to edit it
    let on_preview_click = {
        let selected_item = selected_item.clone();
        let preview_item = preview_item.clone();
        Callback::from(move |item: UpcomingItem| {
            selected_item.set(Some(item));
            preview_item.set(None);
        })
    };

    // Callback to close item preview
    let on_preview_close = {
        let preview_item = preview_item.clone();
        Callback::from(move |_: ()| {
            preview_item.set(None);
        })
    };

    html! {
        <>
            <style>{DASHBOARD_STYLES}</style>
            <div class="peace-dashboard">
                // Overlay for clicking outside to close item edit mode
                if selected_item.is_some() {
                    <div class="item-focus-overlay" onclick={on_item_modal_close.clone()}></div>
                }

                // Chat box and item bar in a container above the overlay
                <div class={if selected_item.is_some() { "item-edit-container" } else { "" }}>
                    // Show the user's Lightfriend SMS number above chat
                    if let Some(ref num) = props.user_profile.preferred_number {
                        <div class="lf-number-label">
                            {"SMS: "}{num}
                        </div>
                    }
                    // Chat box - always at the top, pass focused_item for edit mode
                    <ChatBox
                        on_usage_change={on_usage_change}
                        youtube_connected={*youtube_connected}
                        tesla_connected={*tesla_connected}
                        focused_item={(*selected_item).clone()}
                        on_item_cleared={on_item_cleared}
                        on_item_created={on_item_created}
                        preview_item={(*preview_item).clone()}
                        on_preview_click={on_preview_click}
                        on_preview_close={on_preview_close}
                        prefill_text={(*prefill_chat).clone()}
                        on_prefill_consumed={Some(on_prefill_consumed)}
                    />

                    // Item detail bar (shown when item selected) - below ChatBox
                    if let Some(item) = (*selected_item).as_ref() {
                        <div class="item-detail-bar">
                            <div class="item-detail-info">
                                <div class="item-detail-time">{
                                    if item.date_display.is_empty() {
                                        item.time_display.clone()
                                    } else {
                                        format!("{} - {}", item.time_display, item.date_display)
                                    }
                                }</div>
                                if let Some(ref src) = item.sources_display {
                                    <div class="item-detail-source">{format!("Check: {}", src)}</div>
                                    if src.to_lowercase().contains("weather") {
                                        <div class="item-detail-note">{"Location from Settings > Account"}</div>
                                    }
                                }
                                <div class="item-detail-meta">
                                    {if let Some(ref t) = item.item_type {
                                        html! { <span class={if t == "tracking" { "item-detail-tracking" } else { "item-detail-type" }}>{t}</span> }
                                    } else {
                                        html! {}
                                    }}
                                    {if let Some(ref n) = item.notify {
                                        html! { <span class="item-detail-notify">{n}</span> }
                                    } else {
                                        html! {}
                                    }}
                                </div>
                                <div class="item-detail-desc">
                                    {super::emoji_utils::emojify_description(&item.description)}
                                </div>
                            </div>
                            <button class="item-btn-delete" onclick={on_delete_item}>{"Delete"}</button>
                            <button class="item-btn-close" onclick={on_item_modal_close.clone()}>{"x"}</button>
                        </div>
                    }
                </div>

            // Main dashboard content - blurred when item focused
            <div class={if selected_item.is_some() { "peace-main item-focused" } else { "peace-main" }}>
                // Items status: urgent cards, tracking group, and status line
                <ItemsStatusSection
                    items={attention_items.clone()}
                    total_tracked_count={total_tracked_count}
                    on_dismiss={on_dismiss_item.clone()}
                    on_digest_prefill={Some(on_digest_prefill)}
                    on_item_click={Some(on_item_status_click)}
                />

                // People section with contact avatars
                <div class="section-label">
                    <span>{"People"}</span>
                </div>
                <ContactAvatarRow />

                <div class="peace-separator"></div>
            </div>

            // Settings panel (slide-in)
            <SettingsPanel
                is_open={*settings_open}
                user_profile={Some(props.user_profile.clone())}
                on_close={on_settings_close}
                on_profile_update={props.on_profile_update.clone()}
                initial_tab={*settings_initial_tab}
            />

            </div>
        </>
    }
}
