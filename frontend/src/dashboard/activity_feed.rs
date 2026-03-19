use yew::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use serde::Deserialize;
use crate::utils::api::Api;

const FEED_STYLES: &str = r#"
.activity-feed {
    display: flex;
    flex-direction: column;
    height: 100%;
}
.activity-feed-header {
    font-size: 0.75rem;
    color: #666;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 1rem 1rem 0.5rem;
    position: sticky;
    top: 0;
    background: #121212;
    z-index: 1;
}
.activity-feed-list {
    flex: 1;
    overflow-y: auto;
    padding: 0 0.75rem 1rem;
}
.feed-date-group {
    font-size: 0.7rem;
    color: #555;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    padding: 0.75rem 0 0.3rem 0.25rem;
}
.feed-entry {
    display: flex;
    gap: 0.6rem;
    padding: 0.5rem 0.25rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.03);
    align-items: flex-start;
    cursor: pointer;
    border-radius: 6px;
    transition: background 0.15s;
}
.feed-entry:hover {
    background: rgba(255, 255, 255, 0.03);
}
.feed-entry:last-child {
    border-bottom: none;
}
.feed-icon {
    width: 1.5rem;
    height: 1.5rem;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
    font-size: 0.7rem;
    flex-shrink: 0;
    margin-top: 0.1rem;
}
.feed-icon.type-changelog {
    color: #60a5fa;
    background: rgba(96, 165, 250, 0.1);
}
.feed-icon.type-notification {
    color: #4ade80;
    background: rgba(74, 222, 128, 0.1);
}
.feed-icon.type-notification.failed {
    color: #f87171;
    background: rgba(248, 113, 113, 0.1);
}
.feed-icon.type-message {
    color: #a78bfa;
    background: rgba(167, 139, 250, 0.1);
}
.feed-body {
    flex: 1;
    min-width: 0;
}
.feed-title {
    font-size: 0.8rem;
    color: #ccc;
    line-height: 1.3;
}
.feed-detail {
    font-size: 0.72rem;
    color: #666;
    line-height: 1.3;
    margin-top: 0.15rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}
.feed-detail.expanded {
    white-space: normal;
    overflow: visible;
}
.feed-expanded-info {
    margin-top: 0.35rem;
    padding: 0.4rem 0.5rem;
    background: rgba(255, 255, 255, 0.03);
    border-radius: 6px;
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
}
.feed-expanded-row {
    font-size: 0.72rem;
    color: #777;
    display: flex;
    gap: 0.4rem;
}
.feed-expanded-label {
    color: #555;
    min-width: 50px;
}
.feed-expanded-value {
    color: #aaa;
}
.feed-status-badge {
    display: inline-block;
    font-size: 0.65rem;
    padding: 0.05rem 0.35rem;
    border-radius: 3px;
}
.feed-status-badge.success {
    color: #4ade80;
    background: rgba(74, 222, 128, 0.1);
}
.feed-status-badge.failed {
    color: #f87171;
    background: rgba(248, 113, 113, 0.1);
}
.feed-time {
    font-size: 0.68rem;
    color: #555;
    white-space: nowrap;
    flex-shrink: 0;
    margin-top: 0.15rem;
}
.feed-empty {
    text-align: center;
    color: #555;
    font-size: 0.8rem;
    padding: 3rem 1rem;
}

@media (prefers-color-scheme: light) {
    .activity-feed-header { background: #f8f8f8; color: #888; }
    .feed-date-group { color: #999; }
    .feed-entry { border-bottom-color: rgba(0, 0, 0, 0.04); }
    .feed-entry:hover { background: rgba(0, 0, 0, 0.02); }
    .feed-title { color: #333; }
    .feed-detail { color: #888; }
    .feed-time { color: #999; }
}
"#;

#[derive(Clone, PartialEq, Deserialize)]
struct ActivityFeedEntry {
    id: String,
    entry_type: String,
    timestamp: i32,
    title: String,
    detail: Option<String>,
    icon: String,
    success: Option<bool>,
}

#[derive(Properties, PartialEq)]
pub struct ActivityFeedProps {
    #[prop_or_default]
    pub refresh_seq: u32,
}

#[function_component(ActivityFeed)]
pub fn activity_feed(props: &ActivityFeedProps) -> Html {
    let entries = use_state(|| Vec::<ActivityFeedEntry>::new());
    let loading = use_state(|| true);
    let refresh_trigger = use_state(|| 0u32);
    let expanded_id = use_state(|| None::<String>);

    // Fetch entries
    {
        let entries = entries.clone();
        let loading = loading.clone();
        let seq = props.refresh_seq;
        let trigger = *refresh_trigger;
        use_effect_with_deps(
            move |_| {
                let entries = entries.clone();
                let loading = loading.clone();
                spawn_local(async move {
                    match Api::get("/api/dashboard/activity-feed?limit=100").send().await {
                        Ok(response) => {
                            if response.ok() {
                                if let Ok(data) = response.json::<Vec<ActivityFeedEntry>>().await {
                                    entries.set(data);
                                }
                            }
                        }
                        Err(_) => {}
                    }
                    loading.set(false);
                });
                || ()
            },
            (seq, trigger),
        );
    }

    // Auto-refresh every 30 seconds
    {
        let refresh_trigger = refresh_trigger.clone();
        use_effect_with_deps(
            move |_| {
                let refresh_trigger = refresh_trigger.clone();
                let interval = gloo_timers::callback::Interval::new(30_000, move || {
                    refresh_trigger.set(js_sys::Date::now() as u32);
                });
                move || drop(interval)
            },
            (),
        );
    }

    // Listen for chat-sent and rules-changed events
    {
        let refresh_trigger = refresh_trigger.clone();
        use_effect_with_deps(
            move |_| {
                use wasm_bindgen::closure::Closure;
                use wasm_bindgen::JsCast;

                let rt1 = refresh_trigger.clone();
                let rt2 = refresh_trigger.clone();

                let chat_cb = Closure::wrap(Box::new(move || {
                    rt1.set(js_sys::Date::now() as u32);
                }) as Box<dyn Fn()>);

                let rules_cb = Closure::wrap(Box::new(move || {
                    rt2.set(js_sys::Date::now() as u32);
                }) as Box<dyn Fn()>);

                if let Some(window) = web_sys::window() {
                    let _ = window.add_event_listener_with_callback(
                        "lightfriend-chat-sent",
                        chat_cb.as_ref().unchecked_ref(),
                    );
                    let _ = window.add_event_listener_with_callback(
                        "lightfriend-rules-changed",
                        rules_cb.as_ref().unchecked_ref(),
                    );
                }

                move || {
                    if let Some(window) = web_sys::window() {
                        let _ = window.remove_event_listener_with_callback(
                            "lightfriend-chat-sent",
                            chat_cb.as_ref().unchecked_ref(),
                        );
                        let _ = window.remove_event_listener_with_callback(
                            "lightfriend-rules-changed",
                            rules_cb.as_ref().unchecked_ref(),
                        );
                    }
                }
            },
            (),
        );
    }

    // Group entries by date
    let now_ms = js_sys::Date::now();
    let now_secs = (now_ms / 1000.0) as i32;

    let current_expanded = (*expanded_id).clone();

    html! {
        <>
            <style>{FEED_STYLES}</style>
            <div class="activity-feed">
                <div class="activity-feed-header">{"Activity"}</div>
                <div class="activity-feed-list">
                    if !*loading && entries.is_empty() {
                        <div class="feed-empty">{"No activity yet. Events will appear here as your assistant handles messages and runs rules."}</div>
                    }
                    { render_grouped_entries(&entries, now_secs, &current_expanded, &expanded_id) }
                </div>
            </div>
        </>
    }
}

fn render_grouped_entries(entries: &[ActivityFeedEntry], now_secs: i32, expanded_id: &Option<String>, expanded_handle: &UseStateHandle<Option<String>>) -> Html {
    if entries.is_empty() {
        return html! {};
    }

    let mut result = Vec::new();
    let mut current_date_label = String::new();

    for entry in entries {
        let date_label = date_group_label(entry.timestamp, now_secs);
        if date_label != current_date_label {
            current_date_label = date_label.clone();
            result.push(html! {
                <div class="feed-date-group">{&date_label}</div>
            });
        }
        let is_expanded = expanded_id.as_ref() == Some(&entry.id);
        result.push(render_entry(entry, now_secs, is_expanded, expanded_handle));
    }

    html! { <>{for result}</> }
}

fn render_entry(entry: &ActivityFeedEntry, now_secs: i32, is_expanded: bool, expanded_handle: &UseStateHandle<Option<String>>) -> Html {
    let icon_class = format!(
        "feed-icon type-{}{}",
        entry.entry_type,
        if entry.success == Some(false) { " failed" } else { "" }
    );

    let time_str = relative_time(entry.timestamp, now_secs);

    let on_click = {
        let expanded_handle = expanded_handle.clone();
        let entry_id = entry.id.clone();
        Callback::from(move |_: MouseEvent| {
            if *expanded_handle == Some(entry_id.clone()) {
                expanded_handle.set(None);
            } else {
                expanded_handle.set(Some(entry_id.clone()));
            }
        })
    };

    // Full timestamp for expanded view
    let full_time = {
        let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(entry.timestamp as f64 * 1000.0));
        let month_names = ["Jan","Feb","Mar","Apr","May","Jun","Jul","Aug","Sep","Oct","Nov","Dec"];
        let month = month_names.get(date.get_month() as usize).unwrap_or(&"?");
        let day = date.get_date();
        let hours = date.get_hours();
        let minutes = date.get_minutes();
        let ampm = if hours < 12 { "am" } else { "pm" };
        let h = if hours == 0 { 12 } else if hours > 12 { hours - 12 } else { hours };
        format!("{} {}, {}:{:02}{}", month, day, h, minutes, ampm)
    };

    let type_label = match entry.entry_type.as_str() {
        "changelog" => "System change",
        "notification" => "Notification",
        "message" => "Message",
        _ => "Event",
    };

    html! {
        <div class="feed-entry" key={entry.id.clone()} onclick={on_click}>
            <div class={icon_class}>
                <i class={entry.icon.clone()}></i>
            </div>
            <div class="feed-body">
                <div class="feed-title">{&entry.title}</div>
                if !is_expanded {
                    if let Some(ref detail) = entry.detail {
                        <div class="feed-detail">{detail}</div>
                    } else if entry.success == Some(false) {
                        <div class="feed-detail">{"Delivery failed - check notification settings"}</div>
                    }
                }
                if is_expanded {
                    <div class="feed-expanded-info">
                        <div class="feed-expanded-row">
                            <span class="feed-expanded-label">{"Time"}</span>
                            <span class="feed-expanded-value">{&full_time}</span>
                        </div>
                        <div class="feed-expanded-row">
                            <span class="feed-expanded-label">{"Type"}</span>
                            <span class="feed-expanded-value">{type_label}</span>
                        </div>
                        if let Some(ref success) = entry.success {
                            <div class="feed-expanded-row">
                                <span class="feed-expanded-label">{"Status"}</span>
                                if *success {
                                    <span class="feed-status-badge success">{"Delivered"}</span>
                                } else {
                                    <span class="feed-status-badge failed">{"Failed"}</span>
                                }
                            </div>
                        }
                        if let Some(ref detail) = entry.detail {
                            <div class="feed-expanded-row">
                                <span class="feed-expanded-label">{"Detail"}</span>
                                <span class="feed-expanded-value">{detail}</span>
                            </div>
                        } else if entry.success == Some(false) {
                            <div class="feed-expanded-row">
                                <span class="feed-expanded-label">{"Detail"}</span>
                                <span class="feed-expanded-value">{"Delivery failed - check notification settings"}</span>
                            </div>
                        }
                    </div>
                }
            </div>
            <div class="feed-time">{time_str}</div>
        </div>
    }
}

fn date_group_label(timestamp: i32, now_secs: i32) -> String {
    let diff = now_secs - timestamp;
    let days = diff / 86400;

    if days == 0 {
        "Today".to_string()
    } else if days == 1 {
        "Yesterday".to_string()
    } else if days < 7 {
        // Use JS Date to get day name
        let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(timestamp as f64 * 1000.0));
        let day_names = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
        let day_idx = date.get_day() as usize;
        day_names.get(day_idx).unwrap_or(&"?").to_string()
    } else {
        let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(timestamp as f64 * 1000.0));
        let month_names = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun",
            "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        let month_idx = date.get_month() as usize;
        let day = date.get_date();
        let month = month_names.get(month_idx).unwrap_or(&"?");
        format!("{} {}", month, day)
    }
}

fn relative_time(timestamp: i32, now_secs: i32) -> String {
    let diff = now_secs - timestamp;

    if diff < 60 {
        "just now".to_string()
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else {
        let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(timestamp as f64 * 1000.0));
        let hours = date.get_hours();
        let minutes = date.get_minutes();
        if minutes == 0 {
            let ampm = if hours < 12 { "am" } else { "pm" };
            let h = if hours == 0 { 12 } else if hours > 12 { hours - 12 } else { hours };
            format!("{}{}", h, ampm)
        } else {
            let ampm = if hours < 12 { "am" } else { "pm" };
            let h = if hours == 0 { 12 } else if hours > 12 { hours - 12 } else { hours };
            format!("{}:{:02}{}", h, minutes, ampm)
        }
    }
}
