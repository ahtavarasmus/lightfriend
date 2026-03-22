use crate::profile::billing_models::UserProfile;
use crate::utils::api::Api;
use serde::Deserialize;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use super::activity_feed::ActivityFeed;
use super::chat_box::ChatBox;
use super::rule_builder::{RuleBuilder, RuleTemplate, RuleTemplatePicker};
use super::rules_section::RulesSection;
use super::settings_panel::{SettingsPanel, SettingsTab};

const DASHBOARD_STYLES: &str = r#"
.palantir-dashboard {
    display: grid;
    grid-template-columns: 2fr 3fr;
    grid-template-rows: 1fr auto;
    grid-template-areas:
        "left right"
        "footer right";
    gap: 0;
    height: 100%;
    max-width: 100%;
    margin: 0;
    overflow: hidden;
}
.panel-left {
    grid-area: left;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    padding: 1rem;
    border-right: 1px solid rgba(255, 255, 255, 0.06);
    overflow-y: auto;
}
.panel-right {
    grid-area: right;
    grid-row: 1 / -1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
}
.panel-right-rules {
    flex-shrink: 0;
    max-height: 40%;
    overflow-y: auto;
    padding: 0.75rem 1rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    display: flex;
    gap: 1rem;
}
.panel-right-rules > .rules-scroll-section {
    flex: 1;
    min-width: 0;
}
.panel-right-activity {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
}

/* ---- Status Hero (compact) ---- */
.status-compact {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.5rem 0;
}
.status-compact-icon {
    font-size: 1.4rem;
}
.status-compact-icon.all-good { color: #4ade80; }
.status-compact-icon.needs-attention { color: #fbbf24; }
.status-compact-text {
    font-size: 1rem;
    font-weight: 500;
}
.status-compact-text.all-good { color: #4ade80; }
.status-compact-text.needs-attention { color: #fbbf24; }
.trust-stats-compact {
    color: #555;
    font-size: 0.75rem;
    padding: 0 0 0.25rem;
}
.trust-stat-sep {
    margin: 0 0.4rem;
    color: #444;
}

/* ---- Tracked Events ---- */
.events-section {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    padding: 0.25rem 0 0.5rem;
}
.events-header {
    display: flex;
    align-items: center;
    padding-bottom: 0.15rem;
}
.event-card {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
    background: rgba(126, 178, 255, 0.06);
    border: 1px solid rgba(126, 178, 255, 0.12);
    border-radius: 8px;
    padding: 0.5rem 0.6rem;
}
.event-card-body {
    flex: 1;
    min-width: 0;
}
.event-card-description {
    font-size: 0.8rem;
    font-weight: 600;
    color: #ccc;
}
.event-card-meta {
    font-size: 0.75rem;
    color: #888;
    line-height: 1.3;
}
.event-card-dismiss {
    flex-shrink: 0;
    background: transparent;
    border: none;
    color: #555;
    cursor: pointer;
    padding: 0.2rem;
    font-size: 0.75rem;
    line-height: 1;
}
.event-card-dismiss:hover {
    color: #ff6b6b;
}

/* ---- Action Cards ---- */
.action-cards {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
}
.action-card {
    background: rgba(30, 30, 30, 0.6);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 10px;
    padding: 0.75rem;
}
.action-card-header {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    margin-bottom: 0.35rem;
}
.action-card-person {
    font-size: 0.85rem;
    font-weight: 500;
    color: #e0e0e0;
}
.action-card-platform {
    font-size: 0.65rem;
    color: #888;
    background: rgba(255, 255, 255, 0.06);
    padding: 0.1rem 0.4rem;
    border-radius: 3px;
    text-transform: capitalize;
}
.action-card-preview {
    font-size: 0.8rem;
    color: #aaa;
    line-height: 1.3;
    margin-bottom: 0.5rem;
    word-break: break-word;
}
.action-card-actions {
    display: flex;
    gap: 0.4rem;
}
.action-btn {
    font-size: 0.75rem;
    padding: 0.25rem 0.7rem;
    border-radius: 5px;
    cursor: pointer;
    border: none;
}
.action-btn-reply {
    background: rgba(126, 178, 255, 0.15);
    color: #7EB2FF;
}
.action-btn-reply:hover { background: rgba(126, 178, 255, 0.25); }
.action-btn-dismiss {
    background: transparent;
    color: #666;
    border: 1px solid rgba(255, 255, 255, 0.08);
}
.action-btn-dismiss:hover { color: #999; }

/* ---- Rules scroll sections ---- */
.rules-scroll-section {
    max-height: 220px;
    overflow-y: auto;
}

/* ---- Section labels ---- */
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
}
.info-icon-btn:hover { color: #7EB2FF; }

.peace-separator {
    height: 1px;
    background: linear-gradient(to right, transparent, rgba(255, 255, 255, 0.08), transparent);
    margin: 0.25rem 0;
}

.sidebar-footer {
    grid-area: footer;
    padding: 0.5rem 1rem;
    text-align: center;
    font-size: 0.75rem;
    color: rgba(255, 255, 255, 0.35);
    border-right: 1px solid rgba(255, 255, 255, 0.06);
}
.sidebar-footer a {
    color: rgba(255, 255, 255, 0.45);
    text-decoration: none;
}
.sidebar-footer a:hover {
    color: #1E90FF;
    text-decoration: underline;
}
.sidebar-footer .sidebar-footer-links {
    margin-top: 0.25rem;
}
@media (prefers-color-scheme: light) {
    .sidebar-footer { color: rgba(0, 0, 0, 0.35); }
    .sidebar-footer a { color: rgba(0, 0, 0, 0.45); }
    .sidebar-footer a:hover { color: #1E90FF; }
}

.lf-number-label {
    font-size: 0.7rem;
    color: #555;
    text-align: center;
    margin-bottom: 0.25rem;
}

/* ---- Responsive: single column on mobile ---- */
@media (max-width: 768px) {
    .palantir-dashboard {
        grid-template-columns: 1fr;
        grid-template-rows: auto;
        grid-template-areas:
            "left"
            "right"
            "footer";
        height: auto;
        overflow: auto;
    }
    .panel-left {
        border-right: none;
        height: auto;
        overflow-y: visible;
    }
    .panel-right {
        grid-row: auto;
        height: auto;
        overflow: visible;
    }
    .panel-right-rules {
        flex-direction: column;
        gap: 0.5rem;
        max-height: none;
    }
    .panel-right-activity {
        max-height: 60vh;
    }
    .sidebar-footer {
        border-right: none;
        padding: 1rem;
    }
}

/* ---- Light mode ---- */
@media (prefers-color-scheme: light) {
    .panel-left { border-right-color: rgba(0,0,0,0.06); }
    .status-compact-text.all-good { color: #16a34a; }
    .status-compact-icon.all-good { color: #16a34a; }
    .status-compact-text.needs-attention { color: #d97706; }
    .status-compact-icon.needs-attention { color: #d97706; }
    .trust-stats-compact { color: #888; }
    .trust-stat-sep { color: #ccc; }
    .action-card { background: rgba(255,255,255,0.8); border-color: rgba(0,0,0,0.08); }
    .action-card-person { color: #333; }
    .action-card-platform { color: #666; background: rgba(0,0,0,0.04); }
    .action-card-preview { color: #555; }
    .action-btn-dismiss { color: #999; border-color: rgba(0,0,0,0.1); }
    .person-name { color: #333; }
    .person-channel-badge { color: #888; background: rgba(0,0,0,0.05); }
    .person-row:hover { background: rgba(0,0,0,0.03); }
}
"#;

/// API response types
#[derive(Clone, PartialEq, Deserialize)]
struct DashboardSummaryResponse {
    status: String,
    messages_handled_today: i64,
    notifications_sent_today: i64,
    rules_active: i64,
    action_items: Vec<ActionItemResponse>,
    filtered_count: i64,
    #[serde(default)]
    events: Vec<EventResponse>,
    watched_contacts: Vec<WatchedContactResponse>,
    quiet_mode: QuietModeResponse,
    sunrise_hour: Option<f32>,
    sunset_hour: Option<f32>,
}

#[derive(Clone, PartialEq, Deserialize)]
struct EventResponse {
    id: i32,
    description: String,
    #[serde(default)]
    notify_at: Option<i32>,
    #[serde(default)]
    expires_at: Option<i32>,
    status: String,
    created_at: i32,
}

#[derive(Clone, PartialEq, Deserialize)]
struct ActionItemResponse {
    message_id: i64,
    person_name: String,
    platform: String,
    preview: String,
    timestamp: i32,
    person_id: Option<i32>,
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
struct WatchedContactResponse {
    nickname: String,
    notification_mode: String,
}

#[derive(Properties, PartialEq, Clone)]
pub struct DashboardViewProps {
    pub user_profile: UserProfile,
    pub on_profile_update: Callback<UserProfile>,
}

fn get_dismissed_ids() -> std::collections::HashSet<i64> {
    let mut set = std::collections::HashSet::new();
    if let Some(window) = web_sys::window() {
        if let Ok(Some(storage)) = window.local_storage() {
            if let Ok(Some(val)) = storage.get_item("lf_dismissed_messages") {
                for part in val.split(',') {
                    if let Ok(id) = part.trim().parse::<i64>() {
                        set.insert(id);
                    }
                }
            }
        }
    }
    set
}

fn dismiss_message(id: i64) {
    if let Some(window) = web_sys::window() {
        if let Ok(Some(storage)) = window.local_storage() {
            let mut ids = get_dismissed_ids();
            ids.insert(id);
            let val: String = ids
                .iter()
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(",");
            let _ = storage.set_item("lf_dismissed_messages", &val);
        }
    }
}

#[function_component(DashboardView)]
pub fn dashboard_view(props: &DashboardViewProps) -> Html {
    let summary = use_state(|| None::<DashboardSummaryResponse>);
    let summary_loading = use_state(|| true);
    let youtube_connected = use_state(|| false);
    let tesla_connected = use_state(|| false);
    let settings_open = use_state(|| false);
    let settings_initial_tab = use_state(|| SettingsTab::Capabilities);
    let dismissed_ids = use_state(get_dismissed_ids);
    let chat_prefill = use_state(|| None::<String>);
    let activity_refresh_seq = use_state(|| 0u32);

    // Handle URL parameters for settings panel
    {
        let settings_open = settings_open.clone();
        let settings_initial_tab = settings_initial_tab.clone();
        use_effect_with_deps(
            move |_| {
                if let Some(window) = web_sys::window() {
                    if let Ok(search) = window.location().search() {
                        if let Ok(params) = web_sys::UrlSearchParams::new_with_str(&search) {
                            if let Some(tab) = params.get("settings") {
                                let tab_enum = match tab.to_lowercase().as_str() {
                                    "capabilities" | "connections" => {
                                        Some(SettingsTab::Capabilities)
                                    }
                                    "account" => Some(SettingsTab::Account),
                                    "billing" => Some(SettingsTab::Billing),
                                    _ => None,
                                };
                                if let Some(tab) = tab_enum {
                                    settings_initial_tab.set(tab);
                                    settings_open.set(true);
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

    // Listen for nav Settings button
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
                })
                    as Box<dyn FnMut()>);

                if let Some(window) = web_sys::window() {
                    let _ = window.add_event_listener_with_callback(
                        "open-settings",
                        callback.as_ref().unchecked_ref(),
                    );
                }

                let cleanup = callback;
                move || {
                    if let Some(window) = web_sys::window() {
                        let _ = window.remove_event_listener_with_callback(
                            "open-settings",
                            cleanup.as_ref().unchecked_ref(),
                        );
                    }
                }
            },
            (),
        );
    }

    // Rule builder state
    let rule_builder_open = use_state(|| false);
    let editing_rule = use_state(|| None::<super::rules_section::RuleData>);
    let rules_refresh_seq = use_state(|| 0u32);
    let template_picker_open = use_state(|| false);
    let selected_template = use_state(|| None::<RuleTemplate>);

    // Fetch YouTube/Tesla status
    {
        let youtube_connected = youtube_connected.clone();
        use_effect_with_deps(
            move |_| {
                spawn_local(async move {
                    if let Ok(r) = Api::get("/api/auth/youtube/status").send().await {
                        if let Ok(data) = r.json::<serde_json::Value>().await {
                            if let Some(c) = data.get("connected").and_then(|v| v.as_bool()) {
                                youtube_connected.set(c);
                            }
                        }
                    }
                });
                || ()
            },
            (),
        );
    }
    {
        let tesla_connected = tesla_connected.clone();
        use_effect_with_deps(
            move |_| {
                spawn_local(async move {
                    if let Ok(r) = Api::get("/api/auth/tesla/status").send().await {
                        if let Ok(data) = r.json::<serde_json::Value>().await {
                            if let Some(c) = data.get("has_tesla").and_then(|v| v.as_bool()) {
                                tesla_connected.set(c);
                            }
                        }
                    }
                });
                || ()
            },
            (),
        );
    }

    // Fetch dashboard summary
    let fetch_summary = {
        let summary = summary.clone();
        let summary_loading = summary_loading.clone();
        let activity_refresh_seq = activity_refresh_seq.clone();
        Callback::from(move |_: ()| {
            let summary = summary.clone();
            let summary_loading = summary_loading.clone();
            let activity_refresh_seq = activity_refresh_seq.clone();
            spawn_local(async move {
                match Api::get("/api/dashboard/summary").send().await {
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
                activity_refresh_seq.set(js_sys::Date::now() as u32);
            });
        })
    };

    // Fetch on mount
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

    // Refresh on chat events
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
                let cleanup = callback;
                move || {
                    if let Some(window) = web_sys::window() {
                        let _ = window.remove_event_listener_with_callback(
                            "lightfriend-chat-sent",
                            cleanup.as_ref().unchecked_ref(),
                        );
                    }
                }
            },
            (),
        );
    }

    let on_settings_close = {
        let settings_open = settings_open.clone();
        Callback::from(move |_| settings_open.set(false))
    };

    let on_usage_change = fetch_summary.clone();

    let on_rule_create_click = {
        let template_picker_open = template_picker_open.clone();
        let editing_rule = editing_rule.clone();
        Callback::from(move |_: ()| {
            editing_rule.set(None);
            template_picker_open.set(true);
        })
    };
    let on_template_picker_close = {
        let template_picker_open = template_picker_open.clone();
        Callback::from(move |_: ()| template_picker_open.set(false))
    };
    let on_template_selected = {
        let template_picker_open = template_picker_open.clone();
        let rule_builder_open = rule_builder_open.clone();
        let selected_template = selected_template.clone();
        let editing_rule = editing_rule.clone();
        Callback::from(move |tmpl: RuleTemplate| {
            editing_rule.set(None);
            selected_template.set(Some(tmpl));
            template_picker_open.set(false);
            rule_builder_open.set(true);
        })
    };
    let on_rule_edit_click = {
        let rule_builder_open = rule_builder_open.clone();
        let editing_rule = editing_rule.clone();
        Callback::from(move |rule: super::rules_section::RuleData| {
            editing_rule.set(Some(rule));
            rule_builder_open.set(true);
        })
    };
    let on_rule_builder_close = {
        let rule_builder_open = rule_builder_open.clone();
        let selected_template = selected_template.clone();
        Callback::from(move |_: ()| {
            rule_builder_open.set(false);
            selected_template.set(None);
        })
    };
    let on_rule_saved = {
        let rule_builder_open = rule_builder_open.clone();
        let rules_refresh_seq = rules_refresh_seq.clone();
        let selected_template = selected_template.clone();
        Callback::from(move |_: ()| {
            rule_builder_open.set(false);
            selected_template.set(None);
            rules_refresh_seq.set(*rules_refresh_seq + 1);
        })
    };

    let on_prefill_consumed = {
        let chat_prefill = chat_prefill.clone();
        Callback::from(move |_: ()| chat_prefill.set(None))
    };

    // Build visible action items
    let visible_action_items: Vec<&ActionItemResponse> = (*summary)
        .as_ref()
        .map(|s| {
            s.action_items
                .iter()
                .filter(|i| !dismissed_ids.contains(&i.message_id))
                .collect()
        })
        .unwrap_or_default();

    let has_action_items = !visible_action_items.is_empty();
    let events: Vec<&EventResponse> = (*summary)
        .as_ref()
        .map(|s| s.events.iter().collect())
        .unwrap_or_default();
    let has_events = !events.is_empty();
    let messages_handled = (*summary)
        .as_ref()
        .map(|s| s.messages_handled_today)
        .unwrap_or(0);
    let notifications_sent = (*summary)
        .as_ref()
        .map(|s| s.notifications_sent_today)
        .unwrap_or(0);
    let rules_active = (*summary).as_ref().map(|s| s.rules_active).unwrap_or(0);
    let filtered_count = (*summary).as_ref().map(|s| s.filtered_count).unwrap_or(0);

    html! {
        <>
            <style>{DASHBOARD_STYLES}</style>
            <div class="palantir-dashboard">

                // ======== LEFT PANEL ========
                <div class="panel-left">

                    // ---- Status (compact) - only show when action items need attention ----
                    if has_action_items {
                        <div class="status-compact">
                            <span class="status-compact-icon needs-attention">
                                <i class="fa-solid fa-bell"></i>
                            </span>
                            <span class="status-compact-text needs-attention">
                                {format!("{} {} your attention",
                                    visible_action_items.len(),
                                    if visible_action_items.len() == 1 { "message needs" } else { "messages need" }
                                )}
                            </span>
                        </div>
                    }

                    // ---- Tracked events ----
                    if has_events {
                        <div class="events-section">
                            <div class="events-header">
                                <i class="fa-solid fa-calendar-check" style="color: #7EB2FF; margin-right: 0.4rem; font-size: 0.75rem;"></i>
                                <span style="font-size: 0.75rem; color: #888; text-transform: uppercase; letter-spacing: 0.03em;">
                                    {"Events"}
                                </span>
                            </div>
                            { for events.iter().map(|evt| {
                                let evt_id = evt.id;
                                let now_ts = (js_sys::Date::now() / 1000.0) as i32;
                                let deadline_html = evt.expires_at.map(|ea| {
                                    if now_ts > ea {
                                        html! { <span style="color: #ff6b6b; font-size: 0.65rem; font-weight: 600;">{"overdue"}</span> }
                                    } else {
                                        let days_left = (ea - now_ts) / 86400;
                                        if days_left <= 2 {
                                            html! { <span style="color: #fbbf24; font-size: 0.65rem;">{format!("due in {}d", days_left)}</span> }
                                        } else {
                                            html! { <span style="color: #666; font-size: 0.65rem;">{format!("due in {}d", days_left)}</span> }
                                        }
                                    }
                                });
                                let on_dismiss = {
                                    let fetch_summary = fetch_summary.clone();
                                    Callback::from(move |e: MouseEvent| {
                                        e.stop_propagation();
                                        let fetch_summary = fetch_summary.clone();
                                        spawn_local(async move {
                                            let url = format!("/api/events/{}/dismiss", evt_id);
                                            let _ = Api::post(&url).send().await;
                                            fetch_summary.emit(());
                                        });
                                    })
                                };
                                html! {
                                    <div class="event-card">
                                        <div class="event-card-body">
                                            <div class="event-card-description">
                                                {&evt.description}
                                                if let Some(ref dl) = deadline_html {
                                                    <span style="margin-left: 0.4rem;">{dl.clone()}</span>
                                                }
                                            </div>
                                        </div>
                                        <button class="event-card-dismiss" onclick={on_dismiss} title="Dismiss">
                                            <i class="fa-solid fa-xmark"></i>
                                        </button>
                                    </div>
                                }
                            })}
                        </div>
                    }

                    // ---- Trust stats ----
                    if messages_handled > 0 || notifications_sent > 0 || rules_active > 0 {
                        <div class="trust-stats-compact">
                            if messages_handled > 0 {
                                <span>{format!("{} messages handled", messages_handled)}</span>
                            }
                            if messages_handled > 0 && notifications_sent > 0 {
                                <span class="trust-stat-sep">{"-"}</span>
                            }
                            if notifications_sent > 0 {
                                <span>{format!("{} notifications sent", notifications_sent)}</span>
                            }
                            if (messages_handled > 0 || notifications_sent > 0) && rules_active > 0 {
                                <span class="trust-stat-sep">{"-"}</span>
                            }
                            if rules_active > 0 {
                                <span>{format!("{} {} active", rules_active, if rules_active == 1 { "rule" } else { "rules" })}</span>
                            }
                        </div>
                    }

                    // ---- Action Cards ----
                    if has_action_items {
                        <div class="action-cards">
                            { for visible_action_items.iter().map(|item| {
                                let msg_id = item.message_id;
                                let person_name = item.person_name.clone();
                                let platform = item.platform.clone();
                                let preview_text = item.preview.clone();

                                let on_dismiss = {
                                    let dismissed_ids = dismissed_ids.clone();
                                    Callback::from(move |_: MouseEvent| {
                                        dismiss_message(msg_id);
                                        let mut new_set = (*dismissed_ids).clone();
                                        new_set.insert(msg_id);
                                        dismissed_ids.set(new_set);
                                    })
                                };

                                let reply_person = person_name.clone();
                                let reply_platform = platform.clone();
                                let on_reply = {
                                    let chat_prefill = chat_prefill.clone();
                                    Callback::from(move |_: MouseEvent| {
                                        chat_prefill.set(Some(format!("Reply to {} on {}: ", reply_person, reply_platform)));
                                    })
                                };

                                html! {
                                    <div class="action-card" key={msg_id.to_string()}>
                                        <div class="action-card-header">
                                            <span class="action-card-person">{&person_name}</span>
                                            <span class="action-card-platform">{&platform}</span>
                                        </div>
                                        <div class="action-card-preview">{format!("\"{}\"", preview_text)}</div>
                                        <div class="action-card-actions">
                                            <button class="action-btn action-btn-reply" onclick={on_reply}>{"Reply"}</button>
                                            <button class="action-btn action-btn-dismiss" onclick={on_dismiss}>{"Dismiss"}</button>
                                        </div>
                                    </div>
                                }
                            })}
                        </div>
                    }

                    // ---- ChatBox ----
                    <div>
                        if let Some(ref num) = props.user_profile.preferred_number {
                            <div class="lf-number-label">{"SMS: "}{num}</div>
                        }
                        <ChatBox
                            on_usage_change={on_usage_change}
                            youtube_connected={*youtube_connected}
                            tesla_connected={*tesla_connected}
                            prefill_text={(*chat_prefill).clone()}
                            on_prefill_consumed={on_prefill_consumed}
                        />
                    </div>

                    <div class="peace-separator"></div>
                </div>

                // ======== RIGHT PANEL ========
                <div class="panel-right">
                    // ---- Rules (top) ----
                    <div class="panel-right-rules">
                        <div class="rules-scroll-section">
                            <RulesSection
                                filter_trigger_type={Some("schedule".to_string())}
                                label_override={Some("Schedule".to_string())}
                                on_create_click={on_rule_create_click.clone()}
                                on_edit_click={on_rule_edit_click.clone()}
                                refresh_seq={*rules_refresh_seq}
                            />
                        </div>
                        <div class="rules-scroll-section">
                            <RulesSection
                                filter_trigger_type={Some("ontology_change".to_string())}
                                label_override={Some("Monitoring".to_string())}
                                on_create_click={on_rule_create_click.clone()}
                                on_edit_click={on_rule_edit_click.clone()}
                                refresh_seq={*rules_refresh_seq}
                                show_create_button={false}
                            />
                        </div>
                    </div>

                    // ---- Activity feed (bottom) ----
                    <div class="panel-right-activity">
                        <ActivityFeed refresh_seq={*activity_refresh_seq} />
                    </div>
                </div>

                // ---- Footer links ----
                <div class="sidebar-footer">
                    <div>{"Source code on "}
                        <a href="https://github.com/ahtavarasmus/lightfriend" target="_blank" rel="noopener noreferrer">{"GitHub"}</a>
                    </div>
                    <div class="sidebar-footer-links">
                        <a href="/faq">{"FAQ"}</a>
                        {" | "}
                        <a href="/blog">{"Blog"}</a>
                        {" | "}
                        <a href="/pricing">{"Pricing"}</a>
                        {" | "}
                        <a href="/terms">{"Terms"}</a>
                        {" | "}
                        <a href="/privacy">{"Privacy"}</a>
                        {" | "}
                        <a href="/updates">{"Updates"}</a>
                    </div>
                </div>
            </div>

            // Overlays
            <SettingsPanel
                is_open={*settings_open}
                user_profile={Some(props.user_profile.clone())}
                on_close={on_settings_close}
                on_profile_update={props.on_profile_update.clone()}
                initial_tab={*settings_initial_tab}
            />
            <RuleTemplatePicker
                is_open={*template_picker_open}
                on_close={on_template_picker_close}
                on_select={on_template_selected}
            />
            <RuleBuilder
                is_open={*rule_builder_open}
                on_close={on_rule_builder_close}
                on_saved={on_rule_saved}
                editing_rule={(*editing_rule).clone()}
                initial_template={(*selected_template).clone()}
                plan_type={props.user_profile.plan_type.clone()}
            />
        </>
    }
}
