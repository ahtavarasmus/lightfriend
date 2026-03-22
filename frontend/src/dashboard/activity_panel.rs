use yew::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::MouseEvent;
use wasm_bindgen_futures::spawn_local;
use serde::Deserialize;
use crate::utils::api::Api;
use super::timeline_view::{UpcomingItem, UpcomingDigest};

const ACTIVITY_STYLES: &str = r#"
.activity-panel-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.7);
    z-index: 1100;
    display: flex;
    justify-content: flex-end;
}
.activity-panel {
    width: 100%;
    max-width: 500px;
    height: 100%;
    background: #1a1a1a;
    overflow-y: auto;
    animation: slideInPanel 0.3s ease;
}
@keyframes slideInPanel {
    from { transform: translateX(100%); }
    to { transform: translateX(0); }
}
.activity-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1.25rem 1.5rem 0;
    position: sticky;
    top: 0;
    background: #1a1a1a;
    z-index: 10;
}
.activity-header h2 {
    color: #fff;
    font-size: 1.25rem;
    font-weight: 600;
    margin: 0;
}
.activity-header .close-btn {
    background: transparent;
    border: none;
    color: #888;
    font-size: 1.5rem;
    cursor: pointer;
    padding: 0.25rem 0.5rem;
    line-height: 1;
}
.activity-header .close-btn:hover {
    color: #fff;
}
.activity-tabs {
    display: flex;
    gap: 0;
    padding: 0.75rem 1.5rem 0;
    position: sticky;
    top: 52px;
    background: #1a1a1a;
    z-index: 10;
    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
}
.activity-tab {
    background: transparent;
    border: none;
    border-bottom: 2px solid transparent;
    color: #666;
    font-size: 0.9rem;
    padding: 0.5rem 1rem;
    cursor: pointer;
    transition: all 0.2s;
}
.activity-tab:hover {
    color: #aaa;
}
.activity-tab.active {
    color: #fff;
    border-bottom-color: #7EB2FF;
}
.activity-body {
    padding: 1.5rem;
}
.activity-loading,
.activity-error {
    color: #888;
    text-align: center;
    padding: 2rem;
}
.activity-error {
    color: #ff6b6b;
}
.activity-empty {
    text-align: center;
    padding: 2rem;
}
.activity-empty p {
    color: #888;
    margin: 0;
}
.activity-hint {
    font-size: 0.85rem;
    margin-top: 0.5rem !important;
    color: #666 !important;
}
.activity-list {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
}
.activity-item {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    padding: 0.75rem;
    background: rgba(255, 255, 255, 0.03);
    border-radius: 8px;
    border-left: 3px solid transparent;
}
.activity-item.success {
    border-left-color: #4CAF50;
}
.activity-item.failed {
    border-left-color: #f44336;
}
.activity-desc {
    color: #ddd;
    font-size: 0.9rem;
    flex: 1;
}
.activity-time {
    color: #666;
    font-size: 0.8rem;
    white-space: nowrap;
    margin-left: 1rem;
}

/* Upcoming tab styles */
.upcoming-day-header {
    font-size: 0.75rem;
    color: #666;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    margin-top: 1rem;
    margin-bottom: 0.5rem;
    padding-bottom: 0.25rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
}
.upcoming-day-header:first-child {
    margin-top: 0;
}
.upcoming-item {
    display: flex;
    align-items: flex-start;
    padding: 0.6rem 0.75rem;
    background: rgba(255, 255, 255, 0.03);
    border-radius: 8px;
    border-left: 3px solid #3d4f6f;
    cursor: pointer;
    transition: background 0.15s;
    margin-bottom: 0.5rem;
    gap: 0.75rem;
}
.upcoming-item:hover {
    background: rgba(255, 255, 255, 0.06);
}
.upcoming-item.digest-item {
    border-left-color: #4CAF50;
}
.upcoming-item-body {
    flex: 1;
    min-width: 0;
}
.upcoming-item-time {
    font-size: 0.8rem;
    font-weight: 600;
    color: #7EB2FF;
}
.upcoming-item-time .relative {
    font-weight: 400;
    color: #666;
    margin-left: 0.4rem;
    font-size: 0.75rem;
}
.upcoming-item.digest-item .upcoming-item-time {
    color: #6ec88c;
}
.upcoming-item-desc {
    font-size: 0.85rem;
    color: #ccc;
    margin-top: 0.15rem;
    line-height: 1.3;
}
.upcoming-item-meta {
    font-size: 0.7rem;
    color: #888;
    margin-top: 0.15rem;
}
.upcoming-item-tags {
    display: flex;
    gap: 0.3rem;
    margin-top: 0.15rem;
    flex-wrap: wrap;
}
.upcoming-item-tags span {
    font-size: 0.65rem;
    padding: 0.05rem 0.35rem;
    border-radius: 0.2rem;
    background: rgba(255,255,255,0.06);
    color: #888;
}
.upcoming-item-tracking {
    color: #7eb2ff !important;
}
.upcoming-item-notify {
    color: #e8a838 !important;
}
.upcoming-item-delete {
    background: transparent;
    border: none;
    color: #555;
    font-size: 0.8rem;
    cursor: pointer;
    padding: 0.25rem 0.4rem;
    border-radius: 4px;
    transition: all 0.15s;
    flex-shrink: 0;
    align-self: center;
}
.upcoming-item-delete:hover {
    color: #ff6b6b;
    background: rgba(255, 68, 68, 0.1);
}
"#;

#[derive(Clone, PartialEq)]
enum ActivityTab {
    Recent,
    Upcoming,
}

#[derive(Clone, PartialEq, Deserialize)]
pub struct ActivityEntry {
    pub id: i32,
    pub activity_type: String,
    pub created_at: i32,
    pub reason: Option<String>,
    pub success: Option<bool>,
}

#[derive(Properties, PartialEq, Clone)]
pub struct ActivityPanelProps {
    pub is_open: bool,
    pub on_close: Callback<()>,
    #[prop_or_default]
    pub upcoming_items: Vec<UpcomingItem>,
    #[prop_or_default]
    pub upcoming_digests: Vec<UpcomingDigest>,
    #[prop_or_default]
    pub on_item_click: Option<Callback<UpcomingItem>>,
    #[prop_or_default]
    pub on_item_delete: Option<Callback<i32>>,
    #[prop_or_default]
    pub sunrise_hour: Option<f32>,
    #[prop_or_default]
    pub sunset_hour: Option<f32>,
}

#[function_component(ActivityPanel)]
pub fn activity_panel(props: &ActivityPanelProps) -> Html {
    let activities = use_state(|| Vec::<ActivityEntry>::new());
    let loading = use_state(|| true);
    let error = use_state(|| None::<String>);
    let active_tab = use_state(|| ActivityTab::Recent);

    // Fetch activities when panel opens
    {
        let activities = activities.clone();
        let loading = loading.clone();
        let error = error.clone();
        let is_open = props.is_open;

        use_effect_with_deps(
            move |is_open: &bool| {
                if *is_open {
                    let activities = activities.clone();
                    let loading = loading.clone();
                    let error = error.clone();
                    loading.set(true);
                    error.set(None);

                    spawn_local(async move {
                        match Api::get("/api/profile/recent-activity").send().await {
                            Ok(response) => {
                                if response.ok() {
                                    match response.json::<Vec<ActivityEntry>>().await {
                                        Ok(data) => {
                                            activities.set(data);
                                        }
                                        Err(_) => {
                                            activities.set(vec![]);
                                        }
                                    }
                                } else {
                                    activities.set(vec![]);
                                }
                            }
                            Err(_) => {
                                error.set(Some("Failed to load activity".to_string()));
                            }
                        }
                        loading.set(false);
                    });
                }
                || ()
            },
            is_open,
        );
    }

    // Escape key to close panel
    {
        let on_close = props.on_close.clone();
        let is_open = props.is_open;
        use_effect_with_deps(
            move |is_open: &bool| {
                let closure_holder: std::rc::Rc<std::cell::RefCell<Option<wasm_bindgen::closure::Closure<dyn Fn(web_sys::KeyboardEvent)>>>> =
                    std::rc::Rc::new(std::cell::RefCell::new(None));
                if *is_open {
                    let on_close = on_close.clone();
                    let closure = wasm_bindgen::closure::Closure::<dyn Fn(web_sys::KeyboardEvent)>::new(move |e: web_sys::KeyboardEvent| {
                        if e.key() == "Escape" {
                            on_close.emit(());
                        }
                    });
                    if let Some(document) = web_sys::window().and_then(|w| w.document()) {
                        let _ = document.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref());
                    }
                    *closure_holder.borrow_mut() = Some(closure);
                }
                let holder = closure_holder;
                move || {
                    if let Some(closure) = holder.borrow_mut().take() {
                        if let Some(document) = web_sys::window().and_then(|w| w.document()) {
                            let _ = document.remove_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref());
                        }
                    }
                }
            },
            is_open,
        );
    }

    if !props.is_open {
        return html! {};
    }

    // Recent tab content
    let recent_content = if *active_tab == ActivityTab::Recent {
        if *loading {
            html! { <div class="activity-loading"><div class="loading-spinner-inline"></div></div> }
        } else if let Some(err) = (*error).as_ref() {
            html! { <div class="activity-error">{err}</div> }
        } else if activities.is_empty() {
            html! {
                <div class="activity-empty">
                    <p>{"No recent activity"}</p>
                    <p class="activity-hint">{"Actions like sending digests, notifications, and reminders will appear here."}</p>
                </div>
            }
        } else {
            html! {
                <div class="activity-list">
                    {
                        activities.iter().map(|activity| {
                            let description = format_activity(activity);
                            let time_ago = format_time_ago(activity.created_at);
                            let success_class = match activity.success {
                                Some(true) => "success",
                                Some(false) => "failed",
                                None => "",
                            };

                            html! {
                                <div class={classes!("activity-item", success_class)}>
                                    <div class="activity-desc">{description}</div>
                                    <div class="activity-time">{time_ago}</div>
                                </div>
                            }
                        }).collect::<Html>()
                    }
                </div>
            }
        }
    } else {
        html! {}
    };

    // Upcoming tab content
    let upcoming_content = if *active_tab == ActivityTab::Upcoming {
        render_upcoming(props)
    } else {
        html! {}
    };

    let overlay_click = {
        let on_close = props.on_close.clone();
        Callback::from(move |_: MouseEvent| {
            on_close.emit(());
        })
    };

    let stop_propagation = Callback::from(|e: MouseEvent| {
        e.stop_propagation();
    });

    let on_recent_tab = {
        let active_tab = active_tab.clone();
        Callback::from(move |_: MouseEvent| {
            active_tab.set(ActivityTab::Recent);
        })
    };

    let on_upcoming_tab = {
        let active_tab = active_tab.clone();
        Callback::from(move |_: MouseEvent| {
            active_tab.set(ActivityTab::Upcoming);
        })
    };

    html! {
        <>
            <style>{ACTIVITY_STYLES}</style>
            <div class="activity-panel-overlay" onclick={overlay_click} role="dialog" aria-modal="true" aria-label="Activity">
                <div class="activity-panel" onclick={stop_propagation}>
                    <div class="activity-header">
                        <h2>{"Activity"}</h2>
                        <button
                            class="close-btn"
                            onclick={{
                                let cb = props.on_close.clone();
                                Callback::from(move |_| cb.emit(()))
                            }}
                        >
                            {"x"}
                        </button>
                    </div>
                    <div class="activity-tabs">
                        <button
                            class={classes!("activity-tab", (*active_tab == ActivityTab::Recent).then_some("active"))}
                            onclick={on_recent_tab}
                        >
                            {"Recent"}
                        </button>
                        <button
                            class={classes!("activity-tab", (*active_tab == ActivityTab::Upcoming).then_some("active"))}
                            onclick={on_upcoming_tab}
                        >
                            {"Upcoming"}
                        </button>
                    </div>
                    <div class="activity-body">
                        {recent_content}
                        {upcoming_content}
                    </div>
                </div>
            </div>
        </>
    }
}

/// Merged entry for sorting items and digests together
#[derive(Clone)]
enum UpcomingEntry {
    Item(UpcomingItem),
    Digest(UpcomingDigest),
}

impl UpcomingEntry {
    fn timestamp(&self) -> i32 {
        match self {
            UpcomingEntry::Item(t) => t.timestamp,
            UpcomingEntry::Digest(d) => d.timestamp,
        }
    }
}

fn render_upcoming(props: &ActivityPanelProps) -> Html {
    let sunrise = props.sunrise_hour.unwrap_or(7.0);
    let sunset = props.sunset_hour.unwrap_or(19.0);

    // Merge items + digests into a single sorted list
    let mut entries: Vec<UpcomingEntry> = Vec::new();
    for t in &props.upcoming_items {
        entries.push(UpcomingEntry::Item(t.clone()));
    }
    for d in &props.upcoming_digests {
        entries.push(UpcomingEntry::Digest(d.clone()));
    }
    entries.sort_by_key(|e| e.timestamp());

    if entries.is_empty() {
        return html! {
            <div class="activity-empty">
                <p>{"No upcoming items"}</p>
                <p class="activity-hint">{"Create items by describing them in the chat."}</p>
            </div>
        };
    }

    // Group by day
    let now_ts = (js_sys::Date::now() / 1000.0) as i64;
    let now_date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(js_sys::Date::now()));
    let today_start = {
        let d = now_date.clone();
        d.set_hours(0);
        d.set_minutes(0);
        d.set_seconds(0);
        d.set_milliseconds(0);
        (d.get_time() / 1000.0) as i64
    };

    struct DayGroup {
        label: String,
        entries: Vec<UpcomingEntry>,
    }

    let mut groups: Vec<DayGroup> = Vec::new();
    let mut current_day_start: Option<i64> = None;

    for entry in &entries {
        let entry_date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(entry.timestamp() as f64 * 1000.0));
        let entry_day_start = {
            let d = entry_date.clone();
            d.set_hours(0);
            d.set_minutes(0);
            d.set_seconds(0);
            d.set_milliseconds(0);
            (d.get_time() / 1000.0) as i64
        };

        let needs_new_group = match current_day_start {
            Some(current) => entry_day_start != current,
            None => true,
        };

        if needs_new_group {
            let days_from_today = (entry_day_start - today_start) / 86400;
            let label = if days_from_today == 0 {
                "Today".to_string()
            } else if days_from_today == 1 {
                "Tomorrow".to_string()
            } else {
                let day_of_week = entry_date.get_day();
                let day_str = match day_of_week {
                    0 => "Sun", 1 => "Mon", 2 => "Tue", 3 => "Wed",
                    4 => "Thu", 5 => "Fri", 6 => "Sat", _ => "",
                };
                let month = entry_date.get_month();
                let month_str = match month {
                    0 => "Jan", 1 => "Feb", 2 => "Mar", 3 => "Apr",
                    4 => "May", 5 => "Jun", 6 => "Jul", 7 => "Aug",
                    8 => "Sep", 9 => "Oct", 10 => "Nov", 11 => "Dec",
                    _ => "",
                };
                format!("{} {} {}", day_str, month_str, entry_date.get_date())
            };
            groups.push(DayGroup { label, entries: vec![entry.clone()] });
            current_day_start = Some(entry_day_start);
        } else {
            if let Some(group) = groups.last_mut() {
                group.entries.push(entry.clone());
            }
        }
    }

    let on_item_click = props.on_item_click.clone();
    let on_item_delete = props.on_item_delete.clone();

    html! {
        <div class="activity-list">
            { for groups.into_iter().map(|group| {
                let on_item_click = on_item_click.clone();
                let on_item_delete = on_item_delete.clone();
                html! {
                    <>
                        <div class="upcoming-day-header">{group.label}</div>
                        { for group.entries.into_iter().map(|entry| {
                            render_upcoming_entry(&entry, sunrise, sunset, now_ts, &on_item_click, &on_item_delete)
                        })}
                    </>
                }
            })}
        </div>
    }
}

fn render_upcoming_entry(
    entry: &UpcomingEntry,
    sunrise: f32,
    sunset: f32,
    now_ts: i64,
    on_item_click: &Option<Callback<UpcomingItem>>,
    on_item_delete: &Option<Callback<i32>>,
) -> Html {
    let (is_digest, time_display, description, item_type, notify, sources, item_id, timestamp) = match entry {
        UpcomingEntry::Item(t) => (
            false,
            t.time_display.clone(),
            t.description.clone(),
            t.item_type.clone(),
            t.notify.clone(),
            t.sources_display.clone(),
            t.item_id,
            t.timestamp,
        ),
        UpcomingEntry::Digest(d) => (
            true,
            d.time_display.clone(),
            format!("Digest: {}", d.sources.as_deref().unwrap_or("all sources")),
            Some("recurring".to_string()),
            None,
            None,
            d.item_id,
            d.timestamp,
        ),
    };
    let is_tracking = item_type.as_deref() == Some("tracking");

    // Relative time
    let diff_secs = timestamp as i64 - now_ts;
    let relative = if diff_secs < 0 {
        "overdue".to_string()
    } else if diff_secs < 3600 {
        format!("in {}m", diff_secs / 60)
    } else if diff_secs < 86400 {
        format!("in {}h", diff_secs / 3600)
    } else {
        let days = diff_secs / 86400;
        if days == 1 { "in 1 day".to_string() } else { format!("in {} days", days) }
    };

    // Border-left color based on time of day
    let border_color = get_time_color(timestamp, sunrise, sunset);

    let item_class = if is_digest {
        classes!("upcoming-item", "digest-item")
    } else {
        classes!("upcoming-item")
    };

    // Click handler - emit the item (convert digest to UpcomingItem if needed)
    let onclick = {
        let on_item_click = on_item_click.clone();
        let entry = entry.clone();
        Callback::from(move |_: MouseEvent| {
            if let Some(cb) = &on_item_click {
                match &entry {
                    UpcomingEntry::Item(t) => cb.emit(t.clone()),
                    UpcomingEntry::Digest(d) => {
                        let item = UpcomingItem {
                            item_id: d.item_id,
                            timestamp: d.timestamp,
                            time_display: d.time_display.clone(),
                            description: format!("Digest: {}", d.sources.as_deref().unwrap_or("all sources")),
                            date_display: String::new(),
                            relative_display: String::new(),
                            item_type: Some("recurring".to_string()),
                            notify: None,
                            sources_display: None,
                        };
                        cb.emit(item);
                    }
                }
            }
        })
    };

    // Delete handler
    let on_delete = {
        let on_item_delete = on_item_delete.clone();
        let item_id = item_id;
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            if let (Some(cb), Some(id)) = (&on_item_delete, item_id) {
                cb.emit(id);
            }
        })
    };

    html! {
        <div
            class={item_class}
            style={format!("border-left-color: {};", border_color)}
            onclick={onclick}
        >
            <div class="upcoming-item-body">
                <div class="upcoming-item-time">
                    {&time_display}
                    <span class="relative">{&relative}</span>
                </div>
                <div class="upcoming-item-desc">{super::emoji_utils::emojify_description(&description)}</div>
                <div class="upcoming-item-tags">
                    {if let Some(ref t) = item_type {
                        html! { <span class="upcoming-item-type">{t}</span> }
                    } else {
                        html! {}
                    }}
                    {if is_tracking {
                        html! { <span class="upcoming-item-tracking">{"tracking"}</span> }
                    } else {
                        html! {}
                    }}
                    {if let Some(ref n) = notify {
                        html! { <span class="upcoming-item-notify">{n}</span> }
                    } else {
                        html! {}
                    }}
                </div>
                if let Some(ref src) = sources {
                    <div class="upcoming-item-meta">{format!("Sources: {}", src)}</div>
                }
            </div>
            if item_id.is_some() {
                <button class="upcoming-item-delete" onclick={on_delete} title="Delete item">
                    <i class="fa-solid fa-trash-can"></i>
                </button>
            }
        </div>
    }
}

/// Get a color tinted by time of day, reusing the sunrise/sunset gradient logic
fn get_time_color(timestamp: i32, sunrise: f32, sunset: f32) -> String {
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(timestamp as f64 * 1000.0));
    let hour = date.get_hours() as f32 + (date.get_minutes() as f32 / 60.0);

    if hour < (sunrise - 2.0).max(0.0) { "#1a1a2e".to_string() }
    else if hour < (sunrise - 1.5).max(0.0) { "#2d1f3d".to_string() }
    else if hour < (sunrise - 1.0).max(0.0) { "#4a3050".to_string() }
    else if hour < (sunrise - 0.5).max(0.0) { "#6b4a5e".to_string() }
    else if hour < sunrise + 1.0 { "#87616b".to_string() }
    else if hour < sunset - 1.0 { "#3d4f6f".to_string() }
    else if hour < sunset - 0.5 { "#5a5070".to_string() }
    else if hour < sunset { "#6b4a5e".to_string() }
    else if hour < sunset + 1.0 { "#4a3050".to_string() }
    else if hour < sunset + 2.0 { "#2d1f3d".to_string() }
    else { "#1a1a2e".to_string() }
}

fn format_activity(activity: &ActivityEntry) -> String {
    match activity.activity_type.as_str() {
        "digest" | "generate_digest" => {
            if let Some(ref reason) = activity.reason {
                format!("Sent digest: {}", reason)
            } else {
                "Sent morning digest".to_string()
            }
        }
        "sms" => {
            if let Some(ref reason) = activity.reason {
                format!("SMS notification: {}", reason)
            } else {
                "Sent SMS notification".to_string()
            }
        }
        "call" => {
            if let Some(ref reason) = activity.reason {
                format!("Voice call: {}", reason)
            } else {
                "Made voice call".to_string()
            }
        }
        "email_critical" | "email_priority" => {
            if let Some(ref reason) = activity.reason {
                format!("Email alert: {}", reason)
            } else {
                "Email notification".to_string()
            }
        }
        "whatsapp_critical" | "whatsapp_priority" => {
            if let Some(ref reason) = activity.reason {
                format!("WhatsApp alert: {}", reason)
            } else {
                "WhatsApp notification".to_string()
            }
        }
        "reminder" => {
            if let Some(ref reason) = activity.reason {
                reason.clone()
            } else {
                "Sent reminder".to_string()
            }
        }
        _ => {
            if let Some(ref reason) = activity.reason {
                reason.clone()
            } else {
                activity.activity_type.replace('_', " ")
            }
        }
    }
}

fn format_time_ago(timestamp: i32) -> String {
    let now = js_sys::Date::now() as i64 / 1000;
    let diff = now - timestamp as i64;

    if diff < 60 {
        "just now".to_string()
    } else if diff < 3600 {
        let mins = diff / 60;
        format!("{}m ago", mins)
    } else if diff < 86400 {
        let hours = diff / 3600;
        format!("{}h ago", hours)
    } else {
        let days = diff / 86400;
        format!("{}d ago", days)
    }
}
