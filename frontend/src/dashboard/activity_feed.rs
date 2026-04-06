use crate::config;
use crate::utils::api::Api;
use serde::Deserialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

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
.feed-icon.type-screened {
    color: #6b7280;
    background: rgba(107, 114, 128, 0.1);
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
.feed-batch-children .feed-entry {
    padding: 0.3rem 0.25rem;
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
    #[serde(default)]
    classification_prompt: Option<String>,
    #[serde(default)]
    classification_result: Option<String>,
    #[serde(default)]
    urgency: Option<String>,
    #[serde(default)]
    category: Option<String>,
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
                    match Api::get("/api/dashboard/activity-feed?limit=100")
                        .send()
                        .await
                    {
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

    // SSE: subscribe to server-sent activity feed events for real-time updates
    {
        let refresh_trigger = refresh_trigger.clone();
        let es_handle = use_state(|| None::<web_sys::EventSource>);
        let es_handle_clone = es_handle.clone();
        use_effect_with_deps(
            move |_| {
                let rt = refresh_trigger.clone();

                let url = format!(
                    "{}/api/dashboard/activity-feed/stream",
                    config::get_backend_url()
                );
                let mut init = web_sys::EventSourceInit::new();
                init.with_credentials(true);
                if let Ok(es) =
                    web_sys::EventSource::new_with_event_source_init_dict(&url, &init)
                {
                    let cb = Closure::wrap(Box::new(move |_: web_sys::MessageEvent| {
                        rt.set(js_sys::Date::now() as u32);
                    })
                        as Box<dyn Fn(web_sys::MessageEvent)>);

                    let _ =
                        es.add_event_listener_with_callback("refresh", cb.as_ref().unchecked_ref());
                    cb.forget();

                    es_handle_clone.set(Some(es));
                }

                move || {
                    // Cleanup handled by drop
                }
            },
            (),
        );

        // Close EventSource on unmount
        {
            let es_handle = es_handle.clone();
            use_effect_with_deps(
                move |_| {
                    move || {
                        if let Some(es) = &*es_handle {
                            es.close();
                        }
                    }
                },
                (),
            );
        }
    }

    // Listen for chat-sent and rules-changed events (user-initiated, no SSE)
    {
        let refresh_trigger = refresh_trigger.clone();
        use_effect_with_deps(
            move |_| {
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

/// Key for grouping consecutive similar entries
fn batch_key(entry: &ActivityFeedEntry) -> String {
    // Group by type + detail (which contains sender + platform info)
    let detail = entry.detail.as_deref().unwrap_or("");
    format!("{}|{}", entry.entry_type, detail)
}

fn render_grouped_entries(
    entries: &[ActivityFeedEntry],
    now_secs: i32,
    expanded_id: &Option<String>,
    expanded_handle: &UseStateHandle<Option<String>>,
) -> Html {
    if entries.is_empty() {
        return html! {};
    }

    // Build batches: consecutive entries with the same type+detail get grouped
    let mut batches: Vec<Vec<&ActivityFeedEntry>> = Vec::new();
    for entry in entries {
        if let Some(last_batch) = batches.last() {
            if batch_key(last_batch[0]) == batch_key(entry)
                && date_group_label(last_batch[0].timestamp, now_secs)
                    == date_group_label(entry.timestamp, now_secs)
            {
                // Safe: we just checked last() is Some
                batches.last_mut().unwrap().push(entry);
                continue;
            }
        }
        batches.push(vec![entry]);
    }

    let mut result = Vec::new();
    let mut current_date_label = String::new();

    for batch in &batches {
        let first = batch[0];
        let date_label = date_group_label(first.timestamp, now_secs);
        if date_label != current_date_label {
            current_date_label = date_label.clone();
            result.push(html! {
                <div class="feed-date-group">{&date_label}</div>
            });
        }

        if batch.len() == 1 {
            // Single entry - render normally
            let is_expanded = expanded_id.as_ref() == Some(&first.id);
            result.push(render_entry(first, now_secs, is_expanded, expanded_handle));
        } else {
            // Batch - render collapsed group
            let batch_id = format!("batch-{}", first.id);
            let is_batch_expanded = expanded_id.as_ref() == Some(&batch_id);
            result.push(render_batch(batch, &batch_id, now_secs, is_batch_expanded, expanded_id, expanded_handle));
        }
    }

    html! { <>{for result}</> }
}

fn render_batch(
    batch: &[&ActivityFeedEntry],
    batch_id: &str,
    now_secs: i32,
    is_batch_expanded: bool,
    expanded_id: &Option<String>,
    expanded_handle: &UseStateHandle<Option<String>>,
) -> Html {
    let first = batch[0];
    let last = batch[batch.len() - 1];
    let count = batch.len();

    let icon_class = format!(
        "feed-icon type-{}{}",
        first.entry_type,
        if first.success == Some(false) { " failed" } else { "" }
    );

    let time_str = relative_time(first.timestamp, now_secs);

    let batch_id_owned = batch_id.to_string();
    let on_click = {
        let expanded_handle = expanded_handle.clone();
        let bid = batch_id_owned.clone();
        Callback::from(move |_: MouseEvent| {
            if *expanded_handle == Some(bid.clone()) {
                expanded_handle.set(None);
            } else {
                expanded_handle.set(Some(bid.clone()));
            }
        })
    };

    // Time range
    let time_range = {
        let first_time = relative_time(first.timestamp, now_secs);
        let last_time = relative_time(last.timestamp, now_secs);
        if first_time == last_time {
            first_time
        } else {
            format!("{} - {}", last_time, first_time)
        }
    };

    html! {
        <div key={batch_id_owned.clone()}>
            <div class="feed-entry" onclick={on_click}>
                <div class={icon_class}>
                    <i class={first.icon.clone()}></i>
                </div>
                <div class="feed-body">
                    <div class="feed-title">{format!("{} ({}x)", first.title, count)}</div>
                    if let Some(ref detail) = first.detail {
                        <div class="feed-detail">{detail}</div>
                    }
                </div>
                <div class="feed-time">
                    {&time_str}
                    <div style="font-size: 0.6rem; color: #555; margin-top: 0.1rem;">
                        {if is_batch_expanded { "Hide" } else { "Expand" }}
                    </div>
                </div>
            </div>
            if is_batch_expanded {
                <div style="padding-left: 2.1rem; border-left: 1px solid rgba(255,255,255,0.05); margin-left: 0.75rem;">
                    { for batch.iter().map(|entry| {
                        let is_expanded = expanded_id.as_ref() == Some(&entry.id);
                        render_entry(entry, now_secs, is_expanded, expanded_handle)
                    })}
                </div>
            }
        </div>
    }
}

fn render_entry(
    entry: &ActivityFeedEntry,
    now_secs: i32,
    is_expanded: bool,
    expanded_handle: &UseStateHandle<Option<String>>,
) -> Html {
    let icon_class = format!(
        "feed-icon type-{}{}",
        entry.entry_type,
        if entry.success == Some(false) {
            " failed"
        } else {
            ""
        }
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
        let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(
            entry.timestamp as f64 * 1000.0,
        ));
        let month_names = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        let month = month_names.get(date.get_month() as usize).unwrap_or(&"?");
        let day = date.get_date();
        let hours = date.get_hours();
        let minutes = date.get_minutes();
        let ampm = if hours < 12 { "am" } else { "pm" };
        let h = if hours == 0 {
            12
        } else if hours > 12 {
            hours - 12
        } else {
            hours
        };
        format!("{} {}, {}:{:02}{}", month, day, h, minutes, ampm)
    };

    let type_label = match entry.entry_type.as_str() {
        "changelog" => "System change",
        "notification" => "Notification",
        "screened" => "Screened",
        "message" => "Message",
        "tracked_item" => "Tracked item",
        "proposed_item" => "Proposed item",
        _ => "Event",
    };

    let is_tracked = entry.entry_type == "tracked_item" || entry.entry_type == "proposed_item";
    let is_proposed = entry.entry_type == "proposed_item";
    let event_id_for_action = if is_tracked {
        entry
            .id
            .strip_prefix("event-")
            .and_then(|s| s.parse::<i32>().ok())
    } else {
        None
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
                        // Classification details (urgency, category, prompt, result)
                        if entry.urgency.is_some() || entry.category.is_some() {
                            <div class="feed-expanded-row">
                                <span class="feed-expanded-label">{"Classification"}</span>
                                <span class="feed-expanded-value">{
                                    format!("{} / {}",
                                        entry.urgency.as_deref().unwrap_or("-"),
                                        entry.category.as_deref().unwrap_or("-"))
                                }</span>
                            </div>
                        }
                        if let Some(ref prompt) = entry.classification_prompt {
                            <div class="feed-expanded-row" style="flex-direction: column; gap: 0.25rem;">
                                <span class="feed-expanded-label">{"Signal Report"}</span>
                                <pre style="font-size: 0.7rem; color: #888; white-space: pre-wrap; word-break: break-word; margin: 0; max-height: 200px; overflow-y: auto; background: rgba(0,0,0,0.2); padding: 0.4rem; border-radius: 4px;">{prompt}</pre>
                            </div>
                        }
                        if let Some(ref result) = entry.classification_result {
                            <div class="feed-expanded-row" style="flex-direction: column; gap: 0.25rem;">
                                <span class="feed-expanded-label">{"LLM Result"}</span>
                                <pre style="font-size: 0.7rem; color: #888; white-space: pre-wrap; word-break: break-word; margin: 0; max-height: 150px; overflow-y: auto; background: rgba(0,0,0,0.2); padding: 0.4rem; border-radius: 4px;">{result}</pre>
                            </div>
                        }
                        // Tracked item actions (dismiss / confirm)
                        if is_tracked {
                            <div style="display: flex; gap: 0.5rem; margin-top: 0.4rem;">
                                {if is_proposed {
                                    if let Some(eid) = event_id_for_action {
                                        html! {
                                            <button
                                                style="font-size: 0.75rem; padding: 4px 12px; border-radius: 6px; border: 1px solid rgba(74,222,128,0.3); background: rgba(74,222,128,0.1); color: #4ade80; cursor: pointer;"
                                                onclick={{
                                                    let expanded = expanded_handle.clone();
                                                    Callback::from(move |e: MouseEvent| {
                                                        e.stop_propagation();
                                                        let expanded = expanded.clone();
                                                        spawn_local(async move {
                                                            let _ = Api::post(&format!("/api/events/{}/confirm", eid)).send().await;
                                                            expanded.set(None);
                                                        });
                                                    })
                                                }}
                                            >{"Confirm"}</button>
                                        }
                                    } else {
                                        html! {}
                                    }
                                } else {
                                    html! {}
                                }}
                                {if let Some(eid) = event_id_for_action {
                                    html! {
                                        <button
                                            style="font-size: 0.75rem; padding: 4px 12px; border-radius: 6px; border: 1px solid rgba(255,107,107,0.3); background: rgba(255,107,107,0.1); color: #ff6b6b; cursor: pointer;"
                                            onclick={{
                                                let expanded = expanded_handle.clone();
                                                Callback::from(move |e: MouseEvent| {
                                                    e.stop_propagation();
                                                    let expanded = expanded.clone();
                                                    spawn_local(async move {
                                                        let _ = Api::post(&format!("/api/events/{}/dismiss", eid)).send().await;
                                                        expanded.set(None);
                                                    });
                                                })
                                            }}
                                        >{"Stop tracking"}</button>
                                    }
                                } else {
                                    html! {}
                                }}
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
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
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
            let h = if hours == 0 {
                12
            } else if hours > 12 {
                hours - 12
            } else {
                hours
            };
            format!("{}{}", h, ampm)
        } else {
            let ampm = if hours < 12 { "am" } else { "pm" };
            let h = if hours == 0 {
                12
            } else if hours > 12 {
                hours - 12
            } else {
                hours
            };
            format!("{}:{:02}{}", h, minutes, ampm)
        }
    }
}
