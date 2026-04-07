use crate::profile::billing_models::UserProfile;
use crate::utils::api::Api;
use serde::Deserialize;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use super::activity_feed::ActivityFeed;
use super::chat_box::ChatBox;
use super::rule_builder::{RuleBuilder, RuleTemplate};
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
}
.panel-right-rules > .rules-scroll-section {
    flex: 1;
    min-width: 0;
}
/* Custom Rules collapsible */
.custom-rules-header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    cursor: pointer;
    padding: 0.4rem 0;
    color: #888;
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    user-select: none;
}
.custom-rules-header:hover { color: #aaa; }
.custom-rules-chevron {
    font-size: 0.6rem;
    transition: transform 0.2s ease;
}
.custom-rules-chevron.open { transform: rotate(90deg); }
.custom-rules-body {
    display: flex;
    gap: 1rem;
    margin-top: 0.5rem;
}
.custom-rules-body > .rules-scroll-section {
    flex: 1;
    min-width: 0;
}
/* Cards row */
.cards-row {
    display: flex;
    gap: 0.5rem;
    margin-bottom: 0.75rem;
}
.cards-row > .critical-notif-card {
    flex: 1;
    min-width: 0;
    margin-bottom: 0;
}
/* Critical Notifications card */
.critical-notif-card {
    padding: 0.6rem 0.7rem;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.06);
    margin-bottom: 0.75rem;
}
.critical-notif-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
}
.critical-notif-left {
    display: flex;
    align-items: center;
    gap: 0.5rem;
}
.critical-notif-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
}
.critical-notif-dot.active { background: #4ade80; }
.critical-notif-dot.inactive { background: #666; }
.critical-notif-title {
    font-size: 0.85rem;
    font-weight: 500;
    color: #ccc;
}
.critical-notif-badge {
    font-size: 0.7rem;
    padding: 0.15rem 0.5rem;
    border-radius: 4px;
    font-weight: 500;
    cursor: pointer;
    border: none;
    transition: all 0.2s;
}
.critical-notif-badge.active {
    background: rgba(74, 222, 128, 0.15);
    color: #4ade80;
}
.critical-notif-badge.inactive {
    background: rgba(102, 102, 102, 0.2);
    color: #888;
}
.critical-notif-badge:hover { opacity: 0.8; }
.critical-notif-desc {
    font-size: 0.75rem;
    color: #777;
    margin-top: 0.4rem;
    line-height: 1.4;
}
.critical-notif-details-toggle {
    background: none;
    border: none;
    color: #7EB2FF;
    font-size: 0.7rem;
    cursor: pointer;
    padding: 0.2rem 0;
    margin-top: 0.3rem;
}
.critical-notif-details-toggle:hover { text-decoration: underline; }
.critical-notif-details {
    margin-top: 0.4rem;
    padding: 0.5rem;
    background: rgba(0, 0, 0, 0.2);
    border-radius: 6px;
    font-size: 0.7rem;
    color: #999;
    line-height: 1.5;
}
.critical-notif-details h4 {
    margin: 0 0 0.3rem;
    font-size: 0.72rem;
    color: #aaa;
}
.critical-notif-details ul {
    margin: 0.2rem 0 0.5rem;
    padding-left: 1.2rem;
}
.critical-notif-details li { margin-bottom: 0.15rem; }
.digest-schedule-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-top: 0.4rem;
}
.digest-schedule-row label {
    font-size: 0.72rem;
    color: #999;
}
.digest-schedule-select {
    background: rgba(0, 0, 0, 0.3);
    border: 1px solid rgba(255, 255, 255, 0.15);
    color: #ccc;
    font-size: 0.72rem;
    padding: 0.25rem 0.4rem;
    border-radius: 4px;
    cursor: pointer;
    outline: none;
}
.digest-schedule-select:hover {
    border-color: rgba(255, 255, 255, 0.3);
}
.digest-schedule-select:focus {
    border-color: #7EB2FF;
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
.status-compact-icon.pending-digest { color: #888; }
.status-compact-text {
    font-size: 1rem;
    font-weight: 500;
}
.status-compact-text.all-good { color: #4ade80; }
.status-compact-text.pending-digest { color: #888; }
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
    max-height: min(46vh, 420px);
    min-height: 0;
}
.events-list {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    overflow-y: auto;
    min-height: 0;
    padding-right: 0.2rem;
}
.events-header {
    display: flex;
    align-items: center;
    padding-bottom: 0.15rem;
    flex-shrink: 0;
}
.event-card {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
    background: rgba(126, 178, 255, 0.06);
    border: 1px solid rgba(126, 178, 255, 0.12);
    border-radius: 8px;
    padding: 0.5rem 0.6rem;
    cursor: pointer;
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
    margin-top: 0.3rem;
}
.event-card-time {
    display: block;
}
.event-card-detail {
    margin-top: 0.55rem;
    padding-top: 0.55rem;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
}
.event-card-detail-title {
    font-size: 0.68rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: #7EB2FF;
    margin-bottom: 0.35rem;
}
.event-card-message-list {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
}
.event-card-message {
    background: rgba(255, 255, 255, 0.03);
    border-radius: 6px;
    padding: 0.45rem 0.5rem;
}
.event-card-message-meta {
    font-size: 0.68rem;
    color: #7f8a9a;
    margin-bottom: 0.2rem;
}
.event-card-message-content {
    font-size: 0.75rem;
    color: #b9c0ca;
    line-height: 1.35;
    white-space: pre-wrap;
    word-break: break-word;
}
.event-card-empty {
    font-size: 0.72rem;
    color: #777;
}
.event-card-expanded {
    border-color: rgba(126, 178, 255, 0.24);
    background: rgba(126, 178, 255, 0.09);
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
    font-size: 1.1rem;
    color: #999;
    text-align: center;
    margin-bottom: 0.5rem;
    font-weight: 500;
    letter-spacing: 0.02em;
}

.connect-prompt {
    font-size: 0.8rem;
    color: #888;
    text-align: center;
    margin-top: 0.5rem;
    padding: 0.4rem 0.8rem;
}
.connect-prompt a {
    color: #7c9eff;
    text-decoration: none;
}
.connect-prompt a:hover {
    text-decoration: underline;
}

/* ---- Setup mode (no connections) ---- */
.palantir-dashboard.setup-mode {
    grid-template-columns: 1fr;
    grid-template-areas:
        "left"
        "footer";
}
.palantir-dashboard.setup-mode .panel-left {
    border-right: none;
    max-width: 560px;
    margin: 0 auto;
    justify-content: center;
}
.setup-hero {
    text-align: center;
    padding: 1rem 0;
}
.setup-title {
    font-size: 1.3rem;
    font-weight: 600;
    color: #e0e0e0;
    margin-bottom: 0.6rem;
}
.setup-subtitle {
    font-size: 0.9rem;
    color: #888;
    line-height: 1.5;
    margin-bottom: 1.5rem;
}
.setup-sources {
    display: flex;
    gap: 1.5rem;
    justify-content: center;
    flex-wrap: wrap;
    margin-bottom: 1.5rem;
}
.setup-source-icon {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.72rem;
    color: #666;
}
.setup-connect-btn {
    background: rgba(126, 178, 255, 0.15);
    border: 1px solid rgba(126, 178, 255, 0.35);
    color: #7EB2FF;
    padding: 0.6rem 1.5rem;
    border-radius: 8px;
    font-size: 0.95rem;
    cursor: pointer;
    transition: background 0.15s;
}
.setup-connect-btn:hover {
    background: rgba(126, 178, 255, 0.25);
}
.setup-chat-hint {
    font-size: 0.75rem;
    color: #555;
    text-align: center;
    margin: 1.5rem 0 0.5rem;
}

/* ---- Running mode additions ---- */
.monitoring-status {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.75rem;
    color: #4ade80;
    padding: 0.25rem 0;
}
.monitoring-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #4ade80;
    flex-shrink: 0;
}
.assistant-plan-note {
    font-size: 0.78rem;
    color: #999;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 6px;
    padding: 0.5rem 0.75rem;
    line-height: 1.4;
}
.assistant-plan-note a {
    color: #7EB2FF;
    text-decoration: none;
}
.assistant-plan-note a:hover {
    text-decoration: underline;
}
.rules-compact-bar {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.6rem 1rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    font-size: 0.75rem;
    color: #666;
    flex-shrink: 0;
}
.rules-compact-bar .rules-add-btn {
    font-size: 0.7rem;
    color: #7EB2FF;
    background: rgba(126, 178, 255, 0.1);
    border: 1px solid rgba(126, 178, 255, 0.25);
    border-radius: 5px;
    padding: 0.2rem 0.6rem;
    cursor: pointer;
}
.rules-compact-bar .rules-add-btn:hover {
    background: rgba(126, 178, 255, 0.2);
}
.rules-compact-bar .rules-manage-btn {
    font-size: 0.7rem;
    color: #888;
    background: transparent;
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 5px;
    padding: 0.2rem 0.6rem;
    cursor: pointer;
}
.rules-compact-bar .rules-manage-btn:hover {
    color: #bbb;
    border-color: rgba(255, 255, 255, 0.15);
}
.rules-expandable {
    max-height: 0;
    overflow: hidden;
    transition: max-height 0.25s ease;
    border-bottom: 1px solid transparent;
}
.rules-expandable.expanded {
    max-height: 50vh;
    overflow-y: auto;
    border-bottom-color: rgba(255, 255, 255, 0.06);
}
.rules-expandable-inner {
    padding: 0.5rem 1rem;
    display: flex;
    gap: 1rem;
}
.rules-expandable-inner > .rules-scroll-section {
    flex: 1;
    min-width: 0;
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
    .status-compact-text.pending-digest { color: #666; }
    .status-compact-icon.pending-digest { color: #666; }
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
    .setup-title { color: #222; }
    .setup-subtitle { color: #666; }
    .setup-source-icon { color: #888; }
    .setup-connect-btn { background: rgba(30, 100, 220, 0.1); border-color: rgba(30, 100, 220, 0.3); color: #1a65dc; }
    .setup-connect-btn:hover { background: rgba(30, 100, 220, 0.18); }
    .setup-chat-hint { color: #999; }
    .monitoring-status { color: #16a34a; }
    .monitoring-dot { background: #16a34a; }
    .assistant-plan-note { color: #666; background: rgba(0,0,0,0.02); border-color: rgba(0,0,0,0.08); }
    .rules-compact-bar { border-bottom-color: rgba(0,0,0,0.06); color: #888; }
    .rules-expandable.expanded { border-bottom-color: rgba(0,0,0,0.06); }
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
    sunrise_hour: Option<f32>,
    sunset_hour: Option<f32>,
}

#[derive(Clone, PartialEq, Deserialize)]
struct EventResponse {
    id: i32,
    description: String,
    #[serde(default)]
    remind_at: Option<i32>,
    #[serde(default)]
    due_at: Option<i32>,
    status: String,
    created_at: i32,
}

#[derive(Clone, PartialEq, Deserialize)]
struct EventMessageResponse {
    id: i64,
    platform: String,
    sender_name: String,
    content: String,
    created_at: i32,
    room_id: String,
}

#[derive(Clone, PartialEq, Deserialize)]
struct EventDetailResponse {
    event: EventResponse,
    linked_messages: Vec<EventMessageResponse>,
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
    let has_any_bridge = use_state(|| false);
    let has_whatsapp = use_state(|| false);
    let has_signal = use_state(|| false);
    let has_telegram = use_state(|| false);
    let has_email = use_state(|| false);
    let settings_open = use_state(|| false);
    let expanded_event_id = use_state(|| None::<i32>);
    let event_details = use_state(std::collections::HashMap::<i32, EventDetailResponse>::new);
    let event_detail_loading = use_state(std::collections::HashSet::<i32>::new);
    let settings_initial_tab = use_state(|| SettingsTab::Capabilities);
    let dismissed_ids = use_state(get_dismissed_ids);
    let action_items_expanded = use_state(|| false);
    let chat_prefill = use_state(|| None::<String>);
    let activity_refresh_seq = use_state(|| 0u32);
    let critical_notis_enabled = use_state(|| {
        props
            .user_profile
            .system_important_notify
            .unwrap_or(false)
    });
    let digest_enabled = use_state(|| {
        props.user_profile.digest_enabled.unwrap_or(false)
    });
    let digest_time_display = use_state(|| {
        props.user_profile.digest_time.clone().unwrap_or_default()
    });
    let custom_rules_open = use_state(|| false);
    let critical_notif_details_open = use_state(|| false);
    let digest_details_open = use_state(|| false);
    let digest_custom_input = use_state(|| String::new());
    let digest_show_custom = use_state({
        let t = (*digest_time_display).clone();
        move || !t.is_empty()
    });

    // Critical notifications toggle handler
    let on_critical_toggle = {
        let enabled = critical_notis_enabled.clone();
        let profile = props.user_profile.clone();
        let on_update = props.on_profile_update.clone();
        Callback::from(move |_: web_sys::MouseEvent| {
            let new_val = !*enabled;
            enabled.set(new_val);
            let profile = profile.clone();
            let on_update = on_update.clone();
            spawn_local(async move {
                let request = serde_json::json!({
                    "field": "system_important_notify",
                    "value": new_val
                });
                if let Ok(r) = Api::patch("/api/profile/field")
                    .json(&request)
                    .unwrap()
                    .send()
                    .await
                {
                    if r.ok() {
                        let mut p = profile.clone();
                        p.system_important_notify = Some(new_val);
                        on_update.emit(p);
                    }
                }
            });
        })
    };

    // Digest toggle handler
    let on_digest_toggle = {
        let enabled = digest_enabled.clone();
        let profile = props.user_profile.clone();
        let on_update = props.on_profile_update.clone();
        Callback::from(move |_: web_sys::MouseEvent| {
            let new_val = !*enabled;
            enabled.set(new_val);
            let profile = profile.clone();
            let on_update = on_update.clone();
            spawn_local(async move {
                let request = serde_json::json!({
                    "field": "digest_enabled",
                    "value": new_val
                });
                if let Ok(r) = Api::patch("/api/profile/field")
                    .json(&request)
                    .unwrap()
                    .send()
                    .await
                {
                    if r.ok() {
                        let mut p = profile.clone();
                        p.digest_enabled = Some(new_val);
                        on_update.emit(p);
                    }
                }
            });
        })
    };

    // Digest schedule change handler
    let on_digest_schedule_change = {
        let digest_time_display = digest_time_display.clone();
        let digest_show_custom = digest_show_custom.clone();
        let digest_custom_input = digest_custom_input.clone();
        let profile = props.user_profile.clone();
        let on_update = props.on_profile_update.clone();
        Callback::from(move |e: web_sys::Event| {
            let target: web_sys::HtmlSelectElement = e.target_unchecked_into();
            let selected = target.value();

            if selected == "custom" {
                digest_custom_input.set((*digest_time_display).clone());
                digest_show_custom.set(true);
                return;
            }

            // "auto" selected - clear custom times
            digest_show_custom.set(false);
            digest_time_display.set(String::new());
            let profile = profile.clone();
            let on_update = on_update.clone();
            spawn_local(async move {
                let request = serde_json::json!({
                    "field": "digest_time",
                    "value": serde_json::Value::Null
                });
                if let Ok(r) = Api::patch("/api/profile/field")
                    .json(&request)
                    .unwrap()
                    .send()
                    .await
                {
                    if r.ok() {
                        let mut p = profile.clone();
                        p.digest_time = None;
                        on_update.emit(p);
                    }
                }
            });
        })
    };

    // Custom digest time input handler
    let on_digest_custom_input = {
        let digest_custom_input = digest_custom_input.clone();
        Callback::from(move |e: web_sys::InputEvent| {
            let target: web_sys::HtmlInputElement = e.target_unchecked_into();
            digest_custom_input.set(target.value());
        })
    };

    // Save custom digest times
    let on_digest_custom_save = {
        let digest_custom_input = digest_custom_input.clone();
        let digest_time_display = digest_time_display.clone();
        let profile = props.user_profile.clone();
        let on_update = props.on_profile_update.clone();
        Callback::from(move |_: web_sys::MouseEvent| {
            let raw = (*digest_custom_input).clone();
            // Validate: comma-separated HH:MM values
            let times: Vec<&str> = raw.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
            let valid = !times.is_empty() && times.iter().all(|t| {
                let parts: Vec<&str> = t.split(':').collect();
                parts.len() == 2
                    && parts[0].parse::<u32>().map(|h| h < 24).unwrap_or(false)
                    && parts[1].parse::<u32>().map(|m| m < 60).unwrap_or(false)
            });
            if !valid {
                return;
            }
            let cleaned: String = times.join(",");
            digest_time_display.set(cleaned.clone());
            let profile = profile.clone();
            let on_update = on_update.clone();
            spawn_local(async move {
                let request = serde_json::json!({
                    "field": "digest_time",
                    "value": cleaned
                });
                if let Ok(r) = Api::patch("/api/profile/field")
                    .json(&request)
                    .unwrap()
                    .send()
                    .await
                {
                    if r.ok() {
                        let mut p = profile.clone();
                        p.digest_time = Some(cleaned);
                        on_update.emit(p);
                    }
                }
            });
        })
    };

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

    // Check bridge connections (any of whatsapp/signal/telegram)
    {
        let has_any_bridge = has_any_bridge.clone();
        let has_whatsapp = has_whatsapp.clone();
        let has_signal = has_signal.clone();
        let has_telegram = has_telegram.clone();
        use_effect_with_deps(
            move |_| {
                spawn_local(async move {
                    for bridge in &["whatsapp", "signal", "telegram"] {
                        let url = format!("/api/auth/{}/status", bridge);
                        if let Ok(r) = Api::get(&url).send().await {
                            if let Ok(data) = r.json::<serde_json::Value>().await {
                                if data.get("connected").and_then(|v| v.as_bool()) == Some(true) {
                                    has_any_bridge.set(true);
                                    match *bridge {
                                        "whatsapp" => has_whatsapp.set(true),
                                        "signal" => has_signal.set(true),
                                        "telegram" => has_telegram.set(true),
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                });
                || ()
            },
            (),
        );
    }
    // Check email connection
    {
        let has_email = has_email.clone();
        use_effect_with_deps(
            move |_| {
                spawn_local(async move {
                    if let Ok(r) = Api::get("/api/auth/imap/status").send().await {
                        if let Ok(data) = r.json::<serde_json::Value>().await {
                            if data.get("connected").and_then(|v| v.as_bool()) == Some(true) {
                                has_email.set(true);
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

    let open_capabilities = {
        let settings_open = settings_open.clone();
        let settings_initial_tab = settings_initial_tab.clone();
        Callback::from(move |e: web_sys::MouseEvent| {
            e.prevent_default();
            settings_initial_tab.set(SettingsTab::Capabilities);
            settings_open.set(true);
        })
    };

    let on_usage_change = fetch_summary.clone();

    let on_rule_create_click = {
        let rule_builder_open = rule_builder_open.clone();
        let selected_template = selected_template.clone();
        let editing_rule = editing_rule.clone();
        Callback::from(move |_: ()| {
            editing_rule.set(None);
            selected_template.set(Some(RuleTemplate::Custom));
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

    // Mode detection
    let has_auto = matches!(
        props.user_profile.plan_type.as_deref(),
        Some("autopilot") | Some("byot")
    );
    let is_setup_mode =
        !*has_any_bridge && !*has_email && !props.user_profile.has_any_connection;

    let format_event_time = |timestamp: i32| {
        let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(timestamp as f64 * 1000.0));
        date.to_locale_string("en-GB", &js_sys::Object::new())
            .as_string()
            .unwrap_or_else(|| timestamp.to_string())
    };

    let on_toggle_custom_rules = {
        let custom_rules_open = custom_rules_open.clone();
        Callback::from(move |_: web_sys::MouseEvent| {
            custom_rules_open.set(!*custom_rules_open);
        })
    };

    let on_toggle_critical_details = {
        let critical_notif_details_open = critical_notif_details_open.clone();
        Callback::from(move |_: web_sys::MouseEvent| {
            critical_notif_details_open.set(!*critical_notif_details_open);
        })
    };


    let on_toggle_digest_details = {
        let digest_details_open = digest_details_open.clone();
        Callback::from(move |_: web_sys::MouseEvent| {
            digest_details_open.set(!*digest_details_open);
        })
    };

    html! {
        <>
            <style>{DASHBOARD_STYLES}</style>
            <div class={classes!("palantir-dashboard", is_setup_mode.then_some("setup-mode"))}>

                // ======== LEFT PANEL ========
                <div class="panel-left">

                if is_setup_mode {
                    // ---- Setup mode: guide user to connect data sources ----
                    <div class="setup-hero">
                        <div class="setup-title">{"Get started with Lightfriend"}</div>
                        <div class="setup-subtitle">
                            if has_auto {
                                {"Connect your messaging apps and Lightfriend will monitor your messages, track commitments, and alert you when something needs your attention."}
                            } else {
                                {"Connect your messaging apps so you can ask Lightfriend to check messages, send replies, and manage your communications."}
                            }
                        </div>
                        <div class="setup-sources">
                            <div class="setup-source-icon">
                                <i class="fa-brands fa-whatsapp" style="font-size: 1.8rem; color: #25D366;"></i>
                                <span>{"WhatsApp"}</span>
                            </div>
                            <div class="setup-source-icon">
                                <i class="fa-brands fa-signal-messenger" style="font-size: 1.8rem; color: #3A76F0;"></i>
                                <span>{"Signal"}</span>
                            </div>
                            <div class="setup-source-icon">
                                <i class="fa-brands fa-telegram" style="font-size: 1.8rem; color: #26A5E4;"></i>
                                <span>{"Telegram"}</span>
                            </div>
                            <div class="setup-source-icon">
                                <i class="fa-solid fa-envelope" style="font-size: 1.8rem; color: #EA4335;"></i>
                                <span>{"Email"}</span>
                            </div>
                        </div>
                        <button class="setup-connect-btn" onclick={open_capabilities.clone()}>
                            {"Connect your first app"}
                        </button>
                        <div class="setup-chat-hint">{"Or chat with your assistant directly"}</div>
                        if props.user_profile.plan_type.as_deref() == Some("byot") {
                            <div class="assistant-plan-note" style="margin-top: 1rem;">
                                {"BYOT plan: you also need to set up your own Twilio number. "}
                                <a href="/bring-own-number">{"Setup guide"}</a>
                            </div>
                        }
                    </div>
                    <div>
                        <ChatBox
                            on_usage_change={on_usage_change.clone()}
                            youtube_connected={*youtube_connected}
                            tesla_connected={*tesla_connected}
                            prefill_text={(*chat_prefill).clone()}
                            on_prefill_consumed={on_prefill_consumed.clone()}
                        />
                    </div>
                } else {
                    // ---- Running mode ----

                    // Assistant plan note
                    if !has_auto {
                        <div class="assistant-plan-note">
                            {"Monitoring is not automatic on the Assistant plan. Ask your assistant when you want to check messages or send replies. "}
                            <a href="/pricing">{"Upgrade to Autopilot"}</a>
                        </div>
                    }

                    // BYOT setup note
                    if props.user_profile.plan_type.as_deref() == Some("byot") {
                        if props.user_profile.twilio_sid.is_none()
                            || props.user_profile.twilio_token.is_none()
                        {
                            <div class="assistant-plan-note">
                                {"You're on the BYOT plan. Set up your own Twilio number to start receiving messages and calls. "}
                                <a href="/bring-own-number">{"Setup guide"}</a>
                            </div>
                        } else {
                            <div class="assistant-plan-note">
                                {"BYOT plan active with your Twilio number. "}
                                <a href="/bring-own-number">{"View setup guide"}</a>
                            </div>
                        }
                    }

                    // Status (compact) - clickable to expand/collapse digest items
                    if has_action_items {
                        <div class="status-compact" style="cursor: pointer;" onclick={{
                            let expanded = action_items_expanded.clone();
                            Callback::from(move |_: MouseEvent| expanded.set(!*expanded))
                        }}>
                            <span class="status-compact-icon pending-digest">
                                <i class="fa-solid fa-inbox"></i>
                            </span>
                            <span class="status-compact-text pending-digest">
                                {format!("{} {} in your next digest",
                                    visible_action_items.len(),
                                    if visible_action_items.len() == 1 { "message" } else { "messages" }
                                )}
                            </span>
                            <span style="margin-left: auto; color: #888; font-size: 0.8rem;">
                                {if *action_items_expanded { "Hide" } else { "Show" }}
                            </span>
                        </div>
                    }

                    // Tracked events
                    if has_events {
                        <div class="events-section">
                            <div class="events-header">
                                <i class="fa-solid fa-calendar-check" style="color: #7EB2FF; margin-right: 0.4rem; font-size: 0.75rem;"></i>
                                <span style="font-size: 0.75rem; color: #888; text-transform: uppercase; letter-spacing: 0.03em;">
                                    {"Events"}
                                </span>
                            </div>
                            <div class="events-list">
                                { for events.iter().map(|evt| {
                                    let evt_id = evt.id;
                                    let is_expanded = *expanded_event_id == Some(evt_id);
                                    let detail = (*event_details).get(&evt_id).cloned();
                                    let is_detail_loading = (*event_detail_loading).contains(&evt_id);
                                    let remind_display = evt.remind_at.map(&format_event_time);
                                    let due_display = evt.due_at.map(&format_event_time);
                                    let detail_html = if is_expanded {
                                        if is_detail_loading {
                                            html! { <div class="event-card-empty">{"Loading linked messages..."}</div> }
                                        } else if let Some(detail) = detail.clone() {
                                            if detail.linked_messages.is_empty() {
                                                html! { <div class="event-card-empty">{"No linked messages yet."}</div> }
                                            } else {
                                                html! {
                                                    <div class="event-card-message-list">
                                                        {for detail.linked_messages.iter().map(|message| {
                                                            html! {
                                                                <div class="event-card-message">
                                                                    <div class="event-card-message-meta">
                                                                        {format!("{} · {} · {}", message.sender_name, message.platform, format_event_time(message.created_at))}
                                                                    </div>
                                                                    <div class="event-card-message-content">
                                                                        {&message.content}
                                                                    </div>
                                                                </div>
                                                            }
                                                        })}
                                                    </div>
                                                }
                                            }
                                        } else {
                                            html! { <div class="event-card-empty">{"No detail loaded."}</div> }
                                        }
                                    } else {
                                        Html::default()
                                    };
                                    let on_expand = {
                                        let expanded_event_id = expanded_event_id.clone();
                                        let event_details = event_details.clone();
                                        let event_detail_loading = event_detail_loading.clone();
                                        Callback::from(move |_| {
                                            if *expanded_event_id == Some(evt_id) {
                                                expanded_event_id.set(None);
                                                return;
                                            }
                                            expanded_event_id.set(Some(evt_id));
                                            if (*event_details).contains_key(&evt_id) || (*event_detail_loading).contains(&evt_id) {
                                                return;
                                            }
                                            let mut loading = (*event_detail_loading).clone();
                                            loading.insert(evt_id);
                                            event_detail_loading.set(loading);
                                            let event_details = event_details.clone();
                                            let event_detail_loading = event_detail_loading.clone();
                                            spawn_local(async move {
                                                let url = format!("/api/events/{}", evt_id);
                                                if let Ok(response) = Api::get(&url).send().await {
                                                    if let Ok(detail) = response.json::<EventDetailResponse>().await {
                                                        let mut next = (*event_details).clone();
                                                        next.insert(evt_id, detail);
                                                        event_details.set(next);
                                                    }
                                                }
                                                let mut loading = (*event_detail_loading).clone();
                                                loading.remove(&evt_id);
                                                event_detail_loading.set(loading);
                                            });
                                        })
                                    };
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
                                        <div class={classes!("event-card", is_expanded.then_some("event-card-expanded"))} onclick={on_expand}>
                                            <div class="event-card-body">
                                                <div class="event-card-description">
                                                    {&evt.description}
                                                </div>
                                                <div class="event-card-meta">
                                                    <span class="event-card-time">
                                                        <strong>{"Remind at: "}</strong>
                                                        {remind_display.unwrap_or_else(|| "Not set".to_string())}
                                                    </span>
                                                    <span class="event-card-time">
                                                        <strong>{"Due at: "}</strong>
                                                        {due_display.unwrap_or_else(|| "Not set".to_string())}
                                                    </span>
                                                </div>
                                                if is_expanded {
                                                    <div class="event-card-detail">
                                                        <div class="event-card-detail-title">{"Linked messages"}</div>
                                                        {detail_html}
                                                    </div>
                                                }
                                            </div>
                                            <button class="event-card-dismiss" onclick={on_dismiss} title="Dismiss">
                                                <i class="fa-solid fa-xmark"></i>
                                            </button>
                                        </div>
                                    }
                                })}
                            </div>
                        </div>
                    }

                    // Action Cards
                    if has_action_items && *action_items_expanded {
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

                    // ChatBox
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
                        // Partial connection hints with icons
                        if !*has_any_bridge {
                            <div class="connect-prompt">
                                <i class="fa-brands fa-whatsapp" style="color: #25D366; margin-right: 0.2rem;"></i>
                                <i class="fa-brands fa-signal-messenger" style="color: #3A76F0; margin-right: 0.2rem;"></i>
                                <i class="fa-brands fa-telegram" style="color: #26A5E4; margin-right: 0.4rem;"></i>
                                {"Connect a messaging app. "}
                                <a href="#" onclick={open_capabilities.clone()}>{"Set up connections"}</a>
                            </div>
                        } else if !*has_email {
                            <div class="connect-prompt">
                                <i class="fa-solid fa-envelope" style="color: #EA4335; margin-right: 0.4rem;"></i>
                                {"Connect your email. "}
                                <a href="#" onclick={open_capabilities.clone()}>{"Set up connections"}</a>
                            </div>
                        }
                    </div>

                    <div class="peace-separator"></div>
                }
                </div>

                // ======== RIGHT PANEL (running mode only) ========
                if !is_setup_mode {
                    <div class="panel-right">
                        <div class="panel-right-rules">
                            // ---- Cards row: Critical Notifications + Digests ----
                            <div class="cards-row">
                            <div class="critical-notif-card">
                                <div class="critical-notif-header">
                                    <div class="critical-notif-left">
                                        <div class={classes!("critical-notif-dot", if *critical_notis_enabled { "active" } else { "inactive" })}></div>
                                        <span class="critical-notif-title">{"Critical Notifications"}</span>
                                    </div>
                                    <button
                                        class={classes!("critical-notif-badge", if *critical_notis_enabled { "active" } else { "inactive" })}
                                        onclick={on_critical_toggle.clone()}
                                    >
                                        {if *critical_notis_enabled { "Active" } else { "Inactive" }}
                                    </button>
                                </div>
                                <div class="monitoring-status">
                                    <span class="monitoring-dot"></span>
                                    <span>{"Monitoring: "}</span>
                                    if *has_whatsapp {
                                        <i class="fa-brands fa-whatsapp" style="font-size: 0.9rem; color: #25D366;" title="WhatsApp"></i>
                                    }
                                    if *has_signal {
                                        <i class="fa-brands fa-signal-messenger" style="font-size: 0.9rem; color: #3A76F0;" title="Signal"></i>
                                    }
                                    if *has_telegram {
                                        <i class="fa-brands fa-telegram" style="font-size: 0.9rem; color: #26A5E4;" title="Telegram"></i>
                                    }
                                    if *has_email {
                                        <i class="fa-solid fa-envelope" style="font-size: 0.85rem; color: #EA4335;" title="Email"></i>
                                    }
                                </div>
                                <button class="critical-notif-details-toggle" onclick={on_toggle_critical_details}>
                                    {if *critical_notif_details_open { "Hide info" } else { "More info" }}
                                </button>
                                if *critical_notif_details_open {
                                    <div class="critical-notif-details">
                                        <p>{"AI evaluates every incoming message and notifies you via SMS when something is time-sensitive or urgent. Routine messages are silently collected for your next digest."}</p>
                                        <h4>{"Urgency levels"}</h4>
                                        <ul>
                                            <li><strong>{"Critical: "}</strong>{"immediate danger, medical emergency, security breach"}</li>
                                            <li><strong>{"High: "}</strong>{"a 2-hour delay would cause real consequences - missed meeting, financial loss, time-sensitive decision"}</li>
                                            <li><strong>{"Medium: "}</strong>{"important but can wait a few hours - friend asking to meet later, non-urgent work question"}</li>
                                            <li><strong>{"Low: "}</strong>{"routine updates, casual conversation"}</li>
                                            <li><strong>{"None: "}</strong>{"spam, automated messages"}</li>
                                        </ul>
                                        <p>{"You get notified only for critical or high urgency. The AI also considers sender relationship, messaging patterns, time of day, and cross-platform escalation (e.g., someone messaging you on both WhatsApp and Signal)."}</p>
                                        <p style="margin-top: 0.5rem; font-size: 0.85rem;">
                                            <a
                                                href="https://github.com/ahtavarasmus/lightfriend/blob/master/backend/src/proactive/system_behaviors.rs#L249"
                                                target="_blank"
                                                rel="noopener noreferrer"
                                                style="color: #60a5fa; text-decoration: none;"
                                            >
                                                {"View the classification prompt on GitHub"}
                                                <i class="fa-solid fa-arrow-up-right-from-square" style="font-size: 0.7rem; margin-left: 0.3rem;"></i>
                                            </a>
                                        </p>
                                    </div>
                                }
                            </div>

                            // ---- Message Digests card ----
                            <div class="critical-notif-card">
                                <div class="critical-notif-header">
                                    <div class="critical-notif-left">
                                        <div class={classes!("critical-notif-dot", if *digest_enabled { "active" } else { "inactive" })}></div>
                                        <span class="critical-notif-title">{"Message Digests"}</span>
                                    </div>
                                    <button
                                        class={classes!("critical-notif-badge", if *digest_enabled { "active" } else { "inactive" })}
                                        onclick={on_digest_toggle.clone()}
                                    >
                                        {if *digest_enabled { "Active" } else { "Inactive" }}
                                    </button>
                                </div>
                                <div class="critical-notif-desc">
                                    {if *digest_enabled {
                                        if (*digest_time_display).is_empty() {
                                            "Auto-scheduled based on your activity patterns."
                                        } else {
                                            "Delivering at your custom times."
                                        }
                                    } else {
                                        "Medium-priority messages collected and delivered periodically."
                                    }}
                                </div>
                                if *digest_enabled {
                                    <div class="digest-schedule-row">
                                        <label>{"Schedule:"}</label>
                                        <select
                                            class="digest-schedule-select"
                                            onchange={on_digest_schedule_change.clone()}
                                        >
                                            <option value="auto" selected={(*digest_time_display).is_empty()}>
                                                {"Auto (activity-based)"}
                                            </option>
                                            <option value="custom" selected={!(*digest_time_display).is_empty()}>
                                                {"Custom times"}
                                            </option>
                                        </select>
                                    </div>
                                    if *digest_show_custom {
                                        <div class="digest-schedule-row">
                                            <input
                                                type="text"
                                                class="digest-schedule-select"
                                                style="flex: 1;"
                                                placeholder="e.g. 08:00,12:30,19:00"
                                                value={(*digest_custom_input).clone()}
                                                oninput={on_digest_custom_input.clone()}
                                            />
                                            <button
                                                class="critical-notif-badge active"
                                                style="font-size: 0.7rem; padding: 0.2rem 0.5rem;"
                                                onclick={on_digest_custom_save.clone()}
                                            >
                                                {"Save"}
                                            </button>
                                        </div>
                                    }
                                }
                                <button class="critical-notif-details-toggle" onclick={on_toggle_digest_details}>
                                    {if *digest_details_open { "Hide info" } else { "More info" }}
                                </button>
                                if *digest_details_open {
                                    <div class="critical-notif-details">
                                        <p>{"Messages that aren't urgent enough for an immediate SMS notification get collected into a digest. Instead of interrupting you for every routine message, they're bundled and delivered at once."}</p>
                                        <h4>{"Scheduling"}</h4>
                                        <ul>
                                            <li><strong>{"Auto mode: "}</strong>{"The system learns your activity patterns and delivers digests when you're likely checking your phone - no configuration needed."}</li>
                                            <li><strong>{"Custom times: "}</strong>{"Set specific delivery times in Settings > Digest to override auto-scheduling (e.g., 8:00 AM and 6:00 PM)."}</li>
                                        </ul>
                                        <h4>{"What's included"}</h4>
                                        <ul>
                                            <li>{"Medium and low urgency messages from all connected platforms"}</li>
                                            <li>{"Each message summarized with sender, platform, and key content"}</li>
                                            <li>{"Messages grouped by conversation for easy scanning"}</li>
                                        </ul>
                                        <p>{"Digests are delivered via SMS. If no new messages have accumulated since your last digest, no SMS is sent."}</p>
                                    </div>
                                }
                            </div>
                            </div>

                            // ---- Custom Rules (collapsible) ----
                            <div class="custom-rules-header" onclick={on_toggle_custom_rules}>
                                <i class={classes!("fa-solid", "fa-chevron-right", "custom-rules-chevron", (*custom_rules_open).then_some("open"))}></i>
                                <span>{"Custom Rules"}</span>
                            </div>
                            if *custom_rules_open {
                                <div class="custom-rules-body">
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
                            }
                        </div>

                        // Activity feed (primary surface)
                        <div class="panel-right-activity">
                            <ActivityFeed refresh_seq={*activity_refresh_seq} />
                        </div>
                    </div>
                }

                // ---- Footer links ----
                <div class="sidebar-footer">
                    <div>{"Source code on "}
                        <a href="https://github.com/ahtavarasmus/lightfriend" target="_blank" rel="noopener noreferrer">{"GitHub"}</a>
                    </div>
                    <div class="sidebar-footer-links">
                        <a href="/blog">{"Blog"}</a>
                        {" | "}
                        <a href="mailto:support@lightfriend.ai">{"Support"}</a>
                        {" | "}
                        <a href="/pricing">{"Pricing"}</a>
                        {" | "}
                        <a href="/terms">{"Terms"}</a>
                        {" | "}
                        <a href="/privacy">{"Privacy"}</a>
                        {" | "}
                        <a href="/trustless">{"Verifiably Private"}</a>
                        {" | "}
                        <a href="/trust-chain">{"Trust Chain"}</a>
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
