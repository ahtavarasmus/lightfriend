use yew::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlInputElement, HtmlSelectElement, Event, MouseEvent, InputEvent};
use crate::utils::api::Api;
use crate::proactive::contact_profiles::{
    ContactProfile, ContactProfilesResponse, ProfileException,
    CreateProfileRequest, ExceptionRequest, UpdateDefaultModeRequest,
    UpdatePhoneContactModeRequest, Room, SearchResponse,
};
// ---------------------------------------------------------------------------
// Styles
// ---------------------------------------------------------------------------

const AVATAR_ROW_STYLES: &str = r#"
.avatar-row-wrap {
    width: 100%;
    overflow-x: auto;
    -webkit-overflow-scrolling: touch;
    scrollbar-width: none;
}
.avatar-row-wrap::-webkit-scrollbar { display: none; }

.avatar-row {
    display: flex;
    gap: 1rem;
    justify-content: center;
    padding: 1rem 0.25rem 0.5rem;
    min-width: min-content;
}

.avatar-item {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.25rem;
    flex-shrink: 0;
    cursor: pointer;
}

.avatar-circle-wrap {
    position: relative;
    width: 44px;
    height: 44px;
}

.avatar-circle {
    width: 44px;
    height: 44px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.85rem;
    font-weight: 600;
    color: #fff;
    user-select: none;
}

.avatar-circle.default-avatar {
    background: #555;
    font-size: 1rem;
}

.avatar-circle.add-avatar {
    background: transparent;
    border: 2px dashed rgba(255,255,255,0.25);
    color: #888;
    font-size: 1rem;
}
.avatar-circle.add-avatar:hover {
    border-color: rgba(255,255,255,0.45);
    color: #bbb;
}

.avatar-label {
    font-size: 0.65rem;
    color: #888;
    max-width: 50px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-align: center;
}

/* Platform bubbles */
.platform-bubble {
    position: absolute;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.55rem;
    color: #fff;
    cursor: pointer;
    border: 2px solid #1a1a2e;
    z-index: 2;
    animation: bubble-breathe 3s ease-in-out infinite;
}
.platform-bubble:nth-child(2) { animation-delay: 0.5s; }
.platform-bubble:nth-child(3) { animation-delay: 1.0s; }
.platform-bubble:nth-child(4) { animation-delay: 1.5s; }
@keyframes bubble-breathe {
    0%, 100% { transform: scale(1); }
    50% { transform: scale(1.05); }
}
.platform-bubble:hover {
    animation: none;
    transform: scale(1.2);
    z-index: 3;
}

.bubble-pos-br { bottom: -4px; right: -4px; }
.bubble-pos-bl { bottom: -4px; left: -4px; }
.bubble-pos-tr { top: -4px; right: -4px; }
.bubble-pos-tl { top: -4px; left: -4px; }

/* Animated glow ring behind avatar */
.avatar-glow {
    position: absolute;
    inset: -3px;
    border-radius: 50%;
    animation: avatar-glow-pulse 5s ease-in-out infinite;
    z-index: 0;
    pointer-events: none;
}
@keyframes avatar-glow-pulse {
    0%, 100% { opacity: 0.15; transform: scale(0.95); }
    50% { opacity: 0.28; transform: scale(1.02); }
}

/* Floating incoming particles for avatars */
.avatar-particle {
    position: absolute;
    opacity: 0;
    z-index: 1;
    pointer-events: none;
    animation: avatar-drift 4s ease-in-out infinite;
    display: flex;
    align-items: center;
}
.avatar-particle.p2 {
    animation-delay: -2s;
}
.avatar-particle.p3 {
    animation-delay: -1s;
}
@keyframes avatar-drift {
    0% { opacity: 0; transform: scale(1) translate(0, 0); }
    15% { opacity: 0.6; }
    80% { opacity: 0.2; }
    100% { opacity: 0; transform: scale(0.3) translate(0, 0); }
}
.avatar-particle.from-top { top: -8px; left: 50%; }
.avatar-particle.from-top { animation-name: drift-top; }
@keyframes drift-top {
    0% { opacity: 0; transform: translate(-50%, -6px) scale(1); }
    15% { opacity: 0.5; }
    80% { opacity: 0.15; }
    100% { opacity: 0; transform: translate(-20%, 30px) scale(0.2); }
}
.avatar-particle.from-right { top: 50%; right: -8px; }
.avatar-particle.from-right { animation-name: drift-right; }
@keyframes drift-right {
    0% { opacity: 0; transform: translate(6px, -50%) scale(1); }
    15% { opacity: 0.5; }
    80% { opacity: 0.15; }
    100% { opacity: 0; transform: translate(-40px, -30%) scale(0.2); }
}

/* Modal overlay */
.avatar-modal-overlay {
    position: fixed;
    top: 0; left: 0; right: 0; bottom: 0;
    background: rgba(0,0,0,0.8);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 9999;
}
.avatar-modal-box {
    background: #1e1e2f;
    border: 1px solid rgba(255,255,255,0.1);
    border-radius: 12px;
    padding: 1.5rem;
    max-width: 400px;
    width: 90%;
    color: #ddd;
}
.avatar-modal-box h3 {
    margin: 0 0 1rem 0;
    font-size: 1.1rem;
    color: #fff;
}
.avatar-modal-row {
    margin-bottom: 0.75rem;
}
.avatar-modal-row label {
    display: block;
    font-size: 0.8rem;
    color: #999;
    margin-bottom: 0.25rem;
}
.avatar-modal-row select,
.avatar-modal-row input[type="text"] {
    width: 100%;
    padding: 0.5rem;
    background: #12121f;
    border: 1px solid rgba(255,255,255,0.15);
    border-radius: 6px;
    color: #ddd;
    font-size: 0.9rem;
}
.avatar-modal-row select:focus,
.avatar-modal-row input[type="text"]:focus {
    outline: none;
    border-color: rgba(255,255,255,0.3);
}
.avatar-modal-check {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 0.75rem;
}
.avatar-modal-check input[type="checkbox"] {
    accent-color: #6366f1;
}
.avatar-modal-check label {
    font-size: 0.85rem;
    color: #bbb;
    cursor: pointer;
}
.avatar-modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.5rem;
    margin-top: 1rem;
}
.avatar-modal-actions button {
    padding: 0.45rem 1rem;
    border-radius: 6px;
    font-size: 0.85rem;
    cursor: pointer;
    border: none;
}
.avatar-modal-btn-cancel {
    background: transparent;
    border: 1px solid rgba(255,255,255,0.15) !important;
    color: #999;
}
.avatar-modal-btn-cancel:hover { color: #ccc; }
.avatar-modal-btn-save {
    background: #6366f1;
    color: #fff;
}
.avatar-modal-btn-save:hover { background: #5558e6; }
.avatar-modal-btn-delete {
    background: transparent;
    color: #e55;
    font-size: 0.75rem;
    padding: 0.3rem 0.5rem;
    border: 1px solid rgba(255,80,80,0.2) !important;
    margin-right: auto;
}
.avatar-modal-btn-delete:hover { background: rgba(255,80,80,0.1); }
.avatar-modal-btn-default {
    background: transparent;
    color: #888;
    font-size: 0.8rem;
    padding: 0.3rem 0.75rem;
    border: 1px solid rgba(255,255,255,0.12) !important;
    margin-right: auto;
}
.avatar-modal-btn-default:hover { color: #bbb; }
.avatar-modal-error {
    color: #e55;
    font-size: 0.8rem;
    margin-bottom: 0.5rem;
}
.avatar-modal-platform-header {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
    margin-bottom: 1rem;
}
.avatar-modal-platform-header i {
    font-size: 1.1rem;
}

/* Search suggestion dropdowns */
.avatar-modal-box .input-with-suggestions {
    position: relative;
}
.avatar-modal-box .suggestions-dropdown {
    position: absolute;
    top: 100%;
    left: 0;
    right: 0;
    background: #12121f;
    border: 1px solid rgba(255,255,255,0.15);
    border-radius: 0 0 6px 6px;
    max-height: 150px;
    overflow-y: auto;
    z-index: 10;
}
.avatar-modal-box .suggestion-item {
    padding: 0.45rem 0.5rem;
    cursor: pointer;
    color: #ccc;
    font-size: 0.85rem;
    display: flex;
    align-items: center;
}
.avatar-modal-box .suggestion-item:hover {
    background: rgba(99,102,241,0.15);
}
.avatar-modal-box .suggestion-item.searching,
.avatar-modal-box .suggestion-item.no-results {
    color: #888;
    font-style: italic;
    cursor: default;
}
.avatar-modal-box .suggestion-item.searching:hover,
.avatar-modal-box .suggestion-item.no-results:hover {
    background: transparent;
}
.avatar-modal-box .suggestion-item.error {
    color: #e55;
    font-style: italic;
    cursor: default;
    font-size: 0.8rem;
}
.avatar-modal-box .suggestion-item.error:hover {
    background: transparent;
}
.avatar-modal-box .suggestion-item.disabled {
    opacity: 0.5;
    cursor: not-allowed;
}
.avatar-modal-box .suggestion-item.disabled:hover {
    background: transparent;
}
.group-tag {
    display: inline-block;
    font-size: 0.6rem;
    padding: 0.1rem 0.35rem;
    border-radius: 3px;
    background: rgba(99,102,241,0.2);
    color: #8b8bf5;
    margin-left: 0.35rem;
    vertical-align: middle;
    line-height: 1.2;
}
.contact-tag {
    display: inline-block;
    font-size: 0.65rem;
    padding: 1px 5px;
    border-radius: 3px;
    background: #2d5a3d;
    color: #7fdf9a;
    margin-left: 4px;
    vertical-align: middle;
    line-height: 1.2;
}
.contact-tag.push-name {
    background: #5a4a2d;
    color: #dfbf7f;
}

.avatar-modal-row input.warn-border {
    border-color: #e55 !important;
}

/* People info icon */
.avatar-row-info {
    display: flex;
    align-items: center;
    flex-shrink: 0;
}
.avatar-row-info-btn {
    background: transparent;
    border: none;
    color: #555;
    font-size: 0.85rem;
    cursor: pointer;
    padding: 0.25rem;
    transition: color 0.2s;
}
.avatar-row-info-btn:hover {
    color: #7EB2FF;
}

/* People info modal content */
.people-info-section {
    margin-bottom: 0.75rem;
}
.people-info-section h4 {
    margin: 0.75rem 0 0.3rem 0;
    font-size: 0.9rem;
    color: #7EB2FF;
}
.people-info-section p {
    font-size: 0.8rem;
    color: #aaa;
    margin: 0.2rem 0;
    line-height: 1.4;
}
.people-info-mode {
    margin: 0.3rem 0;
    font-size: 0.8rem;
    color: #aaa;
    line-height: 1.4;
}
.people-info-mode strong {
    color: #ccc;
}
.people-info-tip {
    margin-top: 0.75rem;
    padding-top: 0.5rem;
    border-top: 1px solid rgba(255,255,255,0.08);
    font-size: 0.8rem;
    color: #888;
    line-height: 1.4;
}
.people-info-close {
    display: block;
    margin: 1rem auto 0;
    background: transparent;
    border: 1px solid rgba(255,255,255,0.15);
    color: #999;
    padding: 0.4rem 1.25rem;
    border-radius: 6px;
    cursor: pointer;
    font-size: 0.85rem;
}
.people-info-close:hover {
    color: #ccc;
}

/* Override indicator on platform bubbles */
.platform-bubble.overridden::before {
    content: '\2605';
    position: absolute;
    top: -5px;
    right: -5px;
    font-size: 0.45rem;
    color: #f5c542;
    z-index: 4;
}

/* Mode badge on platform bubbles */
.mode-badge {
    position: absolute;
    bottom: -4px;
    left: -4px;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.3rem;
    color: #fff;
    z-index: 5;
    border: 1px solid #1a1a2e;
    pointer-events: none;
    line-height: 1;
}
.mode-badge.mode-mention { background: #3b82f6; }
.mode-badge.mode-critical { background: #f59e0b; }
.mode-badge.mode-ignore { background: #ef4444; }

/* Notification type icon below contact name */
.avatar-noti-type {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 2px;
    font-size: 0.5rem;
    color: #666;
    margin-top: -2px;
}
.avatar-noti-type i { font-size: 0.45rem; }

/* Read-only state for modal fields */
.avatar-modal-row select:disabled,
.avatar-modal-check input:disabled {
    opacity: 0.5;
    cursor: not-allowed;
}

/* Override status text */
.avatar-modal-override-status {
    font-size: 0.75rem;
    color: #888;
    margin-bottom: 0.5rem;
    text-align: center;
}

/* Customize / Reset to default button */
.avatar-modal-btn-customize {
    background: transparent;
    color: #7EB2FF;
    font-size: 0.8rem;
    padding: 0.3rem 0.75rem;
    border: 1px solid rgba(126,178,255,0.25) !important;
}
.avatar-modal-btn-customize:hover {
    background: rgba(126,178,255,0.08);
    color: #a0c8ff;
}

/* Notes textarea in contact settings */
.avatar-modal-notes {
    width: 100%;
    padding: 0.5rem;
    background: #12121f;
    border: 1px solid rgba(255,255,255,0.15);
    border-radius: 6px;
    color: #ddd;
    font-size: 0.85rem;
    font-family: inherit;
    resize: vertical;
    min-height: 50px;
    outline: none;
}
.avatar-modal-notes:focus {
    border-color: rgba(255,255,255,0.3);
}
.avatar-modal-notes-hint {
    font-size: 0.7rem;
    color: #666;
    margin-top: 0.2rem;
}
"#;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const AVATAR_COLORS: [&str; 8] = [
    "#6366f1", "#8b5cf6", "#ec4899", "#f59e0b",
    "#10b981", "#3b82f6", "#ef4444", "#14b8a6",
];

fn color_for_name(name: &str) -> &'static str {
    let hash: u32 = name.bytes().fold(0u32, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u32));
    AVATAR_COLORS[(hash as usize) % AVATAR_COLORS.len()]
}

fn initials_for_name(name: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().collect();
    match words.len() {
        0 => "?".to_string(),
        1 => words[0].chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default(),
        _ => {
            let a = words[0].chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
            let b = words[1].chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
            format!("{}{}", a, b)
        }
    }
}

struct PlatformInfo {
    key: &'static str,
    icon: &'static str,
    color: &'static str,
    label: &'static str,
}

const PLATFORMS: [PlatformInfo; 4] = [
    PlatformInfo { key: "whatsapp", icon: "fa-brands fa-whatsapp", color: "#25D366", label: "WhatsApp" },
    PlatformInfo { key: "telegram", icon: "fa-brands fa-telegram", color: "#0088CC", label: "Telegram" },
    PlatformInfo { key: "signal", icon: "fa-solid fa-comment-dots", color: "#3A76F1", label: "Signal" },
    PlatformInfo { key: "email", icon: "fa-solid fa-envelope", color: "#7EB2FF", label: "Email" },
];

fn connected_platforms(profile: &ContactProfile) -> Vec<&'static PlatformInfo> {
    let mut out = Vec::new();
    if profile.whatsapp_chat.as_ref().map(|s| !s.is_empty()).unwrap_or(false) {
        out.push(&PLATFORMS[0]);
    }
    if profile.telegram_chat.as_ref().map(|s| !s.is_empty()).unwrap_or(false) {
        out.push(&PLATFORMS[1]);
    }
    if profile.signal_chat.as_ref().map(|s| !s.is_empty()).unwrap_or(false) {
        out.push(&PLATFORMS[2]);
    }
    if profile.email_addresses.as_ref().map(|s| !s.is_empty()).unwrap_or(false) {
        out.push(&PLATFORMS[3]);
    }
    out
}

/// Positions for 1-4 platform bubbles around the avatar circle.
fn bubble_position_class(count: usize, idx: usize) -> &'static str {
    match (count, idx) {
        (1, 0) => "bubble-pos-br",
        (2, 0) => "bubble-pos-br",
        (2, 1) => "bubble-pos-bl",
        (3, 0) => "bubble-pos-br",
        (3, 1) => "bubble-pos-bl",
        (3, 2) => "bubble-pos-tr",
        (4, 0) => "bubble-pos-br",
        (4, 1) => "bubble-pos-bl",
        (4, 2) => "bubble-pos-tr",
        (4, 3) => "bubble-pos-tl",
        _ => "bubble-pos-br",
    }
}

fn find_exception<'a>(profile: &'a ContactProfile, platform: &str) -> Option<&'a ProfileException> {
    profile.exceptions.iter().find(|e| e.platform == platform)
}

/// Returns the effective notification mode for a platform: exception mode if overridden, else contact default.
fn effective_mode_for_platform<'a>(profile: &'a ContactProfile, platform_key: &str) -> &'a str {
    if let Some(exc) = find_exception(profile, platform_key) {
        exc.notification_mode.as_str()
    } else {
        profile.notification_mode.as_str()
    }
}

/// Returns (icon_class, css_class) for modes other than "all". None means no badge needed.
fn mode_badge_info(mode: &str) -> Option<(&'static str, &'static str)> {
    match mode {
        "mention" => Some(("fa-solid fa-at", "mode-mention")),
        "critical" => Some(("fa-solid fa-bell", "mode-critical")),
        "ignore" => Some(("fa-solid fa-xmark", "mode-ignore")),
        _ => None, // "all" or unknown - no badge
    }
}

// ---------------------------------------------------------------------------
// Modal state
// ---------------------------------------------------------------------------

#[derive(Clone, PartialEq)]
enum ModalType {
    ContactSettings(i32),
    PlatformException(i32, String),
    DefaultSettings,
    PhoneContactSettings,
    AddContact,
    PeopleInfo,
    PlatformInfo(i32, String),
    ContactSettingsInfo(i32),
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

#[derive(Properties, PartialEq, Clone)]
pub struct ContactAvatarRowProps {}

#[function_component(ContactAvatarRow)]
pub fn contact_avatar_row(_props: &ContactAvatarRowProps) -> Html {
    let profiles = use_state(|| Vec::<ContactProfile>::new());
    let default_mode = use_state(|| "critical".to_string());
    let default_noti_type = use_state(|| "sms".to_string());
    let default_notify_on_call = use_state(|| true);
    let loading = use_state(|| true);
    let modal = use_state(|| None::<ModalType>);
    let error_msg = use_state(|| None::<String>);
    let saving = use_state(|| false);

    // Modal form state
    let form_nickname = use_state(|| String::new());
    let form_mode = use_state(|| "critical".to_string());
    let form_type = use_state(|| "sms".to_string());
    let form_notify_call = use_state(|| true);

    let form_notes = use_state(|| String::new());

    // Contact settings form state (bridge fields)
    let form_whatsapp = use_state(|| String::new());
    let form_telegram = use_state(|| String::new());
    let form_signal = use_state(|| String::new());
    let form_email = use_state(|| String::new());
    let form_whatsapp_room_id = use_state(|| None::<String>);
    let form_telegram_room_id = use_state(|| None::<String>);
    let form_signal_room_id = use_state(|| None::<String>);
    let form_whatsapp_selected = use_state(|| false);
    let form_telegram_selected = use_state(|| false);
    let form_signal_selected = use_state(|| false);

    // Platform exception form state
    let exc_mode = use_state(|| String::new());
    let exc_type = use_state(|| "sms".to_string());
    let exc_notify_call = use_state(|| true);
    let exc_override_mode = use_state(|| false);

    // Default settings form state
    let def_form_mode = use_state(|| "critical".to_string());
    let def_form_type = use_state(|| "sms".to_string());
    let def_form_notify_call = use_state(|| true);

    // Phone contact settings state
    let phone_contact_mode = use_state(|| "critical".to_string());
    let phone_contact_noti_type = use_state(|| "sms".to_string());
    let phone_contact_notify_on_call = use_state(|| true);
    let pc_form_mode = use_state(|| "critical".to_string());
    let pc_form_type = use_state(|| "sms".to_string());
    let pc_form_notify_call = use_state(|| true);

    // Add contact form state
    let add_nickname = use_state(|| String::new());
    let add_whatsapp = use_state(|| String::new());
    let add_telegram = use_state(|| String::new());
    let add_signal = use_state(|| String::new());
    let add_email = use_state(|| String::new());
    let add_mode = use_state(|| "critical".to_string());
    let add_type = use_state(|| "sms".to_string());
    let add_notify_call = use_state(|| true);
    // Room ID state for stable matching
    let add_whatsapp_room_id = use_state(|| None::<String>);
    let add_telegram_room_id = use_state(|| None::<String>);
    let add_signal_room_id = use_state(|| None::<String>);
    let add_whatsapp_selected = use_state(|| false);
    let add_telegram_selected = use_state(|| false);
    let add_signal_selected = use_state(|| false);

    // Search state per platform
    let whatsapp_results = use_state(|| Vec::<Room>::new());
    let telegram_results = use_state(|| Vec::<Room>::new());
    let signal_results = use_state(|| Vec::<Room>::new());
    let searching_whatsapp = use_state(|| false);
    let searching_telegram = use_state(|| false);
    let searching_signal = use_state(|| false);
    let show_whatsapp_suggestions = use_state(|| false);
    let show_telegram_suggestions = use_state(|| false);
    let show_signal_suggestions = use_state(|| false);
    let search_error_whatsapp = use_state(|| None::<String>);
    let search_error_telegram = use_state(|| None::<String>);
    let search_error_signal = use_state(|| None::<String>);

    // Fetch profiles
    let fetch_profiles = {
        let profiles = profiles.clone();
        let default_mode = default_mode.clone();
        let default_noti_type = default_noti_type.clone();
        let default_notify_on_call = default_notify_on_call.clone();
        let phone_contact_mode = phone_contact_mode.clone();
        let phone_contact_noti_type = phone_contact_noti_type.clone();
        let phone_contact_notify_on_call = phone_contact_notify_on_call.clone();
        let loading = loading.clone();
        Callback::from(move |_: ()| {
            let profiles = profiles.clone();
            let default_mode = default_mode.clone();
            let default_noti_type = default_noti_type.clone();
            let default_notify_on_call = default_notify_on_call.clone();
            let phone_contact_mode = phone_contact_mode.clone();
            let phone_contact_noti_type = phone_contact_noti_type.clone();
            let phone_contact_notify_on_call = phone_contact_notify_on_call.clone();
            let loading = loading.clone();
            spawn_local(async move {
                if let Ok(response) = Api::get("/api/contact-profiles").send().await {
                    if let Ok(data) = response.json::<ContactProfilesResponse>().await {
                        profiles.set(data.profiles);
                        default_mode.set(data.default_mode);
                        default_noti_type.set(data.default_noti_type);
                        default_notify_on_call.set(data.default_notify_on_call);
                        phone_contact_mode.set(data.phone_contact_mode);
                        phone_contact_noti_type.set(data.phone_contact_noti_type);
                        phone_contact_notify_on_call.set(data.phone_contact_notify_on_call);
                    }
                }
                loading.set(false);
            });
        })
    };

    // Initial load
    {
        let fetch = fetch_profiles.clone();
        use_effect_with_deps(move |_| {
            fetch.emit(());
            || ()
        }, ());
    }

    // Listen for cross-component sync events
    {
        let fetch = fetch_profiles.clone();
        use_effect_with_deps(
            move |_| {
                let callback = wasm_bindgen::closure::Closure::wrap(Box::new(move || {
                    fetch.emit(());
                }) as Box<dyn FnMut()>);

                if let Some(window) = web_sys::window() {
                    let _ = window.add_event_listener_with_callback(
                        "lightfriend-contact-profiles-updated",
                        callback.as_ref().unchecked_ref(),
                    );
                }

                let cleanup = callback;
                move || {
                    if let Some(window) = web_sys::window() {
                        let _ = window.remove_event_listener_with_callback(
                            "lightfriend-contact-profiles-updated",
                            cleanup.as_ref().unchecked_ref(),
                        );
                    }
                }
            },
            (),
        );
    }

    // Dispatch sync event helper
    fn dispatch_sync_event() {
        if let Some(window) = web_sys::window() {
            if let Ok(event) = web_sys::CustomEvent::new("lightfriend-contact-profiles-updated") {
                let _ = window.dispatch_event(&event);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Search chats for platform suggestions
    // -----------------------------------------------------------------------
    let search_chats = {
        let whatsapp_results = whatsapp_results.clone();
        let telegram_results = telegram_results.clone();
        let signal_results = signal_results.clone();
        let searching_whatsapp = searching_whatsapp.clone();
        let searching_telegram = searching_telegram.clone();
        let searching_signal = searching_signal.clone();
        let show_whatsapp_suggestions = show_whatsapp_suggestions.clone();
        let show_telegram_suggestions = show_telegram_suggestions.clone();
        let show_signal_suggestions = show_signal_suggestions.clone();
        let search_error_whatsapp = search_error_whatsapp.clone();
        let search_error_telegram = search_error_telegram.clone();
        let search_error_signal = search_error_signal.clone();

        Callback::from(move |(service, query): (String, String)| {
            if query.trim().len() < 2 {
                match service.as_str() {
                    "whatsapp" => { whatsapp_results.set(vec![]); show_whatsapp_suggestions.set(false); searching_whatsapp.set(false); search_error_whatsapp.set(None); }
                    "telegram" => { telegram_results.set(vec![]); show_telegram_suggestions.set(false); searching_telegram.set(false); search_error_telegram.set(None); }
                    "signal" => { signal_results.set(vec![]); show_signal_suggestions.set(false); searching_signal.set(false); search_error_signal.set(None); }
                    _ => {}
                }
                return;
            }

            let whatsapp_results = whatsapp_results.clone();
            let telegram_results = telegram_results.clone();
            let signal_results = signal_results.clone();
            let searching_whatsapp = searching_whatsapp.clone();
            let searching_telegram = searching_telegram.clone();
            let searching_signal = searching_signal.clone();
            let show_whatsapp_suggestions = show_whatsapp_suggestions.clone();
            let show_telegram_suggestions = show_telegram_suggestions.clone();
            let show_signal_suggestions = show_signal_suggestions.clone();
            let search_error_whatsapp = search_error_whatsapp.clone();
            let search_error_telegram = search_error_telegram.clone();
            let search_error_signal = search_error_signal.clone();
            let service = service.clone();

            match service.as_str() {
                "whatsapp" => { searching_whatsapp.set(true); show_whatsapp_suggestions.set(true); search_error_whatsapp.set(None); }
                "telegram" => { searching_telegram.set(true); show_telegram_suggestions.set(true); search_error_telegram.set(None); }
                "signal" => { searching_signal.set(true); show_signal_suggestions.set(true); search_error_signal.set(None); }
                _ => {}
            }

            spawn_local(async move {
                let encoded_query = js_sys::encode_uri_component(&query);
                let url = format!("/api/contact-profiles/search/{}?q={}", service, encoded_query);
                match Api::get(&url).send().await {
                    Ok(response) => {
                        if response.ok() {
                            if let Ok(data) = response.json::<SearchResponse>().await {
                                match service.as_str() {
                                    "whatsapp" => { whatsapp_results.set(data.results); searching_whatsapp.set(false); }
                                    "telegram" => { telegram_results.set(data.results); searching_telegram.set(false); }
                                    "signal" => { signal_results.set(data.results); searching_signal.set(false); }
                                    _ => {}
                                }
                            }
                        } else {
                            let error_msg = if let Ok(text) = response.text().await {
                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                                    json.get("error").and_then(|e| e.as_str()).unwrap_or("Search failed").to_string()
                                } else {
                                    "Search failed".to_string()
                                }
                            } else {
                                "Search failed".to_string()
                            };
                            match service.as_str() {
                                "whatsapp" => { searching_whatsapp.set(false); search_error_whatsapp.set(Some(error_msg)); }
                                "telegram" => { searching_telegram.set(false); search_error_telegram.set(Some(error_msg)); }
                                "signal" => { searching_signal.set(false); search_error_signal.set(Some(error_msg)); }
                                _ => {}
                            }
                        }
                    }
                    Err(_) => {
                        match service.as_str() {
                            "whatsapp" => { searching_whatsapp.set(false); search_error_whatsapp.set(Some("Network error".to_string())); }
                            "telegram" => { searching_telegram.set(false); search_error_telegram.set(Some("Network error".to_string())); }
                            "signal" => { searching_signal.set(false); search_error_signal.set(Some("Network error".to_string())); }
                            _ => {}
                        }
                    }
                }
            });
        })
    };

    // -----------------------------------------------------------------------
    // Avatar click: open contact settings modal
    // -----------------------------------------------------------------------
    // Helper to open contact settings modal (shared between avatar click and triage gear)
    let open_contact_settings = {
        let modal = modal.clone();
        let profiles = profiles.clone();
        let form_nickname = form_nickname.clone();
        let form_mode = form_mode.clone();
        let form_type = form_type.clone();
        let form_notify_call = form_notify_call.clone();
        let form_whatsapp = form_whatsapp.clone();
        let form_telegram = form_telegram.clone();
        let form_signal = form_signal.clone();
        let form_email = form_email.clone();
        let form_whatsapp_room_id = form_whatsapp_room_id.clone();
        let form_telegram_room_id = form_telegram_room_id.clone();
        let form_signal_room_id = form_signal_room_id.clone();
        let form_whatsapp_selected = form_whatsapp_selected.clone();
        let form_telegram_selected = form_telegram_selected.clone();
        let form_signal_selected = form_signal_selected.clone();
        let error_msg = error_msg.clone();
        let whatsapp_results = whatsapp_results.clone();
        let telegram_results = telegram_results.clone();
        let signal_results = signal_results.clone();
        let show_whatsapp_suggestions = show_whatsapp_suggestions.clone();
        let show_telegram_suggestions = show_telegram_suggestions.clone();
        let show_signal_suggestions = show_signal_suggestions.clone();
        let search_error_whatsapp = search_error_whatsapp.clone();
        let search_error_telegram = search_error_telegram.clone();
        let form_notes = form_notes.clone();
        let search_error_signal = search_error_signal.clone();
        Callback::from(move |profile_id: i32| {
            if let Some(p) = profiles.iter().find(|p| p.id == profile_id) {
                form_nickname.set(p.nickname.clone());
                form_mode.set(p.notification_mode.clone());
                form_type.set(p.notification_type.clone());
                form_notify_call.set(p.notify_on_call);
                form_whatsapp.set(p.whatsapp_chat.clone().unwrap_or_default());
                form_telegram.set(p.telegram_chat.clone().unwrap_or_default());
                form_signal.set(p.signal_chat.clone().unwrap_or_default());
                form_email.set(p.email_addresses.clone().unwrap_or_default());
                form_notes.set(p.notes.clone().unwrap_or_default());
                form_whatsapp_room_id.set(p.whatsapp_room_id.clone());
                form_telegram_room_id.set(p.telegram_room_id.clone());
                form_signal_room_id.set(p.signal_room_id.clone());
                form_whatsapp_selected.set(p.whatsapp_chat.is_some());
                form_telegram_selected.set(p.telegram_chat.is_some());
                form_signal_selected.set(p.signal_chat.is_some());
                whatsapp_results.set(vec![]);
                telegram_results.set(vec![]);
                signal_results.set(vec![]);
                show_whatsapp_suggestions.set(false);
                show_telegram_suggestions.set(false);
                show_signal_suggestions.set(false);
                search_error_whatsapp.set(None);
                search_error_telegram.set(None);
                search_error_signal.set(None);
                error_msg.set(None);
                modal.set(Some(ModalType::ContactSettings(profile_id)));
            }
        })
    };

    let on_avatar_click = {
        let open_settings = open_contact_settings.clone();
        Callback::from(move |profile_id: i32| {
            open_settings.emit(profile_id);
        })
    };

    // -----------------------------------------------------------------------
    // Bubble click: open platform exception modal
    // -----------------------------------------------------------------------
    let on_bubble_click = {
        let modal = modal.clone();
        let profiles = profiles.clone();
        let exc_mode = exc_mode.clone();
        let exc_type = exc_type.clone();
        let exc_notify_call = exc_notify_call.clone();
        let exc_override_mode = exc_override_mode.clone();
        let form_whatsapp = form_whatsapp.clone();
        let form_telegram = form_telegram.clone();
        let form_signal = form_signal.clone();
        let form_whatsapp_room_id = form_whatsapp_room_id.clone();
        let form_telegram_room_id = form_telegram_room_id.clone();
        let form_signal_room_id = form_signal_room_id.clone();
        let form_whatsapp_selected = form_whatsapp_selected.clone();
        let form_telegram_selected = form_telegram_selected.clone();
        let form_signal_selected = form_signal_selected.clone();
        let whatsapp_results = whatsapp_results.clone();
        let telegram_results = telegram_results.clone();
        let signal_results = signal_results.clone();
        let show_whatsapp_suggestions = show_whatsapp_suggestions.clone();
        let show_telegram_suggestions = show_telegram_suggestions.clone();
        let show_signal_suggestions = show_signal_suggestions.clone();
        let search_error_whatsapp = search_error_whatsapp.clone();
        let search_error_telegram = search_error_telegram.clone();
        let search_error_signal = search_error_signal.clone();
        let error_msg = error_msg.clone();
        Callback::from(move |(profile_id, platform): (i32, String)| {
            if let Some(p) = profiles.iter().find(|p| p.id == profile_id) {
                let has_exc = find_exception(p, &platform).is_some();
                exc_override_mode.set(has_exc);
                if let Some(exc) = find_exception(p, &platform) {
                    exc_mode.set(exc.notification_mode.clone());
                    exc_type.set(exc.notification_type.clone());
                    exc_notify_call.set(exc.notify_on_call);
                } else {
                    exc_mode.set(p.notification_mode.clone());
                    exc_type.set(p.notification_type.clone());
                    exc_notify_call.set(p.notify_on_call);
                }
                // Initialize chat form state for the platform
                match platform.as_str() {
                    "whatsapp" => {
                        form_whatsapp.set(p.whatsapp_chat.clone().unwrap_or_default());
                        form_whatsapp_room_id.set(p.whatsapp_room_id.clone());
                        form_whatsapp_selected.set(p.whatsapp_chat.is_some());
                    }
                    "telegram" => {
                        form_telegram.set(p.telegram_chat.clone().unwrap_or_default());
                        form_telegram_room_id.set(p.telegram_room_id.clone());
                        form_telegram_selected.set(p.telegram_chat.is_some());
                    }
                    "signal" => {
                        form_signal.set(p.signal_chat.clone().unwrap_or_default());
                        form_signal_room_id.set(p.signal_room_id.clone());
                        form_signal_selected.set(p.signal_chat.is_some());
                    }
                    _ => {}
                }
                whatsapp_results.set(vec![]);
                telegram_results.set(vec![]);
                signal_results.set(vec![]);
                show_whatsapp_suggestions.set(false);
                show_telegram_suggestions.set(false);
                show_signal_suggestions.set(false);
                search_error_whatsapp.set(None);
                search_error_telegram.set(None);
                search_error_signal.set(None);
                error_msg.set(None);
                modal.set(Some(ModalType::PlatformException(profile_id, platform)));
            }
        })
    };

    // -----------------------------------------------------------------------
    // Default settings click
    // -----------------------------------------------------------------------
    let on_default_click = {
        let modal = modal.clone();
        let default_mode = default_mode.clone();
        let default_noti_type = default_noti_type.clone();
        let default_notify_on_call = default_notify_on_call.clone();
        let def_form_mode = def_form_mode.clone();
        let def_form_type = def_form_type.clone();
        let def_form_notify_call = def_form_notify_call.clone();
        let error_msg = error_msg.clone();
        Callback::from(move |_: MouseEvent| {
            def_form_mode.set((*default_mode).clone());
            def_form_type.set((*default_noti_type).clone());
            def_form_notify_call.set(*default_notify_on_call);
            error_msg.set(None);
            modal.set(Some(ModalType::DefaultSettings));
        })
    };

    // -----------------------------------------------------------------------
    // Phone contact settings click
    // -----------------------------------------------------------------------
    let on_phone_contact_click = {
        let modal = modal.clone();
        let phone_contact_mode = phone_contact_mode.clone();
        let phone_contact_noti_type = phone_contact_noti_type.clone();
        let phone_contact_notify_on_call = phone_contact_notify_on_call.clone();
        let pc_form_mode = pc_form_mode.clone();
        let pc_form_type = pc_form_type.clone();
        let pc_form_notify_call = pc_form_notify_call.clone();
        let error_msg = error_msg.clone();
        Callback::from(move |_: MouseEvent| {
            pc_form_mode.set((*phone_contact_mode).clone());
            pc_form_type.set((*phone_contact_noti_type).clone());
            pc_form_notify_call.set(*phone_contact_notify_on_call);
            error_msg.set(None);
            modal.set(Some(ModalType::PhoneContactSettings));
        })
    };

    // -----------------------------------------------------------------------
    // Add contact click: open AddContact modal
    // -----------------------------------------------------------------------
    let on_add_click = {
        let modal = modal.clone();
        let add_nickname = add_nickname.clone();
        let add_whatsapp = add_whatsapp.clone();
        let add_telegram = add_telegram.clone();
        let add_signal = add_signal.clone();
        let add_email = add_email.clone();
        let add_mode = add_mode.clone();
        let add_type = add_type.clone();
        let add_notify_call = add_notify_call.clone();
        let add_whatsapp_room_id = add_whatsapp_room_id.clone();
        let add_telegram_room_id = add_telegram_room_id.clone();
        let add_signal_room_id = add_signal_room_id.clone();
        let add_whatsapp_selected = add_whatsapp_selected.clone();
        let add_telegram_selected = add_telegram_selected.clone();
        let add_signal_selected = add_signal_selected.clone();
        let error_msg = error_msg.clone();
        let whatsapp_results = whatsapp_results.clone();
        let telegram_results = telegram_results.clone();
        let signal_results = signal_results.clone();
        let show_whatsapp_suggestions = show_whatsapp_suggestions.clone();
        let show_telegram_suggestions = show_telegram_suggestions.clone();
        let show_signal_suggestions = show_signal_suggestions.clone();
        let search_error_whatsapp = search_error_whatsapp.clone();
        let search_error_telegram = search_error_telegram.clone();
        let search_error_signal = search_error_signal.clone();
        Callback::from(move |_: MouseEvent| {
            add_nickname.set(String::new());
            add_whatsapp.set(String::new());
            add_telegram.set(String::new());
            add_signal.set(String::new());
            add_email.set(String::new());
            add_mode.set("critical".to_string());
            add_type.set("sms".to_string());
            add_notify_call.set(true);
            add_whatsapp_room_id.set(None);
            add_telegram_room_id.set(None);
            add_signal_room_id.set(None);
            add_whatsapp_selected.set(false);
            add_telegram_selected.set(false);
            add_signal_selected.set(false);
            error_msg.set(None);
            whatsapp_results.set(vec![]);
            telegram_results.set(vec![]);
            signal_results.set(vec![]);
            show_whatsapp_suggestions.set(false);
            show_telegram_suggestions.set(false);
            show_signal_suggestions.set(false);
            search_error_whatsapp.set(None);
            search_error_telegram.set(None);
            search_error_signal.set(None);
            modal.set(Some(ModalType::AddContact));
        })
    };

    // -----------------------------------------------------------------------
    // Close modal
    // -----------------------------------------------------------------------
    let close_modal = {
        let modal = modal.clone();
        Callback::from(move |_: ()| {
            modal.set(None);
        })
    };

    // -----------------------------------------------------------------------
    // Save contact settings
    // -----------------------------------------------------------------------
    let save_contact = {
        let profiles = profiles.clone();
        let modal = modal.clone();
        let form_nickname = form_nickname.clone();
        let form_mode = form_mode.clone();
        let form_type = form_type.clone();
        let form_notify_call = form_notify_call.clone();
        let form_whatsapp = form_whatsapp.clone();
        let form_telegram = form_telegram.clone();
        let form_signal = form_signal.clone();
        let form_email = form_email.clone();
        let form_whatsapp_room_id = form_whatsapp_room_id.clone();
        let form_telegram_room_id = form_telegram_room_id.clone();
        let form_signal_room_id = form_signal_room_id.clone();
        let form_whatsapp_selected = form_whatsapp_selected.clone();
        let form_telegram_selected = form_telegram_selected.clone();
        let form_signal_selected = form_signal_selected.clone();
        let form_notes = form_notes.clone();
        let error_msg = error_msg.clone();
        let saving = saving.clone();
        let fetch_profiles = fetch_profiles.clone();
        Callback::from(move |profile_id: i32| {
            let profile = profiles.iter().find(|p| p.id == profile_id).cloned();
            let Some(profile) = profile else { return };

            let nickname = (*form_nickname).clone();
            if nickname.contains('@') {
                error_msg.set(Some("Nickname cannot contain '@'.".to_string()));
                return;
            }
            if nickname.trim().is_empty() {
                error_msg.set(Some("Nickname cannot be empty.".to_string()));
                return;
            }

            let wa = (*form_whatsapp).clone();
            let tg = (*form_telegram).clone();
            let sg = (*form_signal).clone();

            if (!wa.is_empty() && !*form_whatsapp_selected)
                || (!tg.is_empty() && !*form_telegram_selected)
                || (!sg.is_empty() && !*form_signal_selected) {
                error_msg.set(Some("Select a chat from the search results.".to_string()));
                return;
            }

            let exceptions: Vec<ExceptionRequest> = profile.exceptions.iter().map(|e| ExceptionRequest {
                platform: e.platform.clone(),
                notification_mode: e.notification_mode.clone(),
                notification_type: e.notification_type.clone(),
                notify_on_call: e.notify_on_call,
            }).collect();

            let em = (*form_email).clone();
            let notes_val = (*form_notes).trim().to_string();
            let request = CreateProfileRequest {
                nickname,
                whatsapp_chat: if wa.is_empty() { None } else { Some(wa) },
                telegram_chat: if tg.is_empty() { None } else { Some(tg) },
                signal_chat: if sg.is_empty() { None } else { Some(sg) },
                email_addresses: if em.is_empty() { None } else { Some(em) },
                notification_mode: (*form_mode).clone(),
                notification_type: (*form_type).clone(),
                notify_on_call: *form_notify_call,
                exceptions: if exceptions.is_empty() { None } else { Some(exceptions) },
                whatsapp_room_id: (*form_whatsapp_room_id).clone(),
                telegram_room_id: (*form_telegram_room_id).clone(),
                signal_room_id: (*form_signal_room_id).clone(),
                notes: if notes_val.is_empty() { None } else { Some(notes_val) },
            };

            let modal = modal.clone();
            let error_msg = error_msg.clone();
            let saving = saving.clone();
            let fetch_profiles = fetch_profiles.clone();

            saving.set(true);
            spawn_local(async move {
                if let Ok(req) = Api::put(&format!("/api/contact-profiles/{}", profile_id)).json(&request) {
                    if let Ok(response) = req.send().await {
                        if response.ok() {
                            dispatch_sync_event();
                            fetch_profiles.emit(());
                            modal.set(None);
                        } else if let Ok(body) = response.json::<serde_json::Value>().await {
                            let msg = body["error"].as_str().unwrap_or("Failed to save");
                            error_msg.set(Some(msg.to_string()));
                        } else {
                            error_msg.set(Some("Failed to save".to_string()));
                        }
                    } else {
                        error_msg.set(Some("Network error".to_string()));
                    }
                }
                saving.set(false);
            });
        })
    };

    // -----------------------------------------------------------------------
    // Delete contact
    // -----------------------------------------------------------------------
    let delete_contact = {
        let modal = modal.clone();
        let error_msg = error_msg.clone();
        let saving = saving.clone();
        let fetch_profiles = fetch_profiles.clone();
        Callback::from(move |profile_id: i32| {
            let modal = modal.clone();
            let error_msg = error_msg.clone();
            let saving = saving.clone();
            let fetch_profiles = fetch_profiles.clone();

            saving.set(true);
            spawn_local(async move {
                if let Ok(response) = Api::delete(&format!("/api/contact-profiles/{}", profile_id)).send().await {
                    if response.ok() {
                        dispatch_sync_event();
                        fetch_profiles.emit(());
                        modal.set(None);
                    } else {
                        error_msg.set(Some("Failed to delete".to_string()));
                    }
                } else {
                    error_msg.set(Some("Network error".to_string()));
                }
                saving.set(false);
            });
        })
    };

    // -----------------------------------------------------------------------
    // Save platform exception
    // -----------------------------------------------------------------------
    let save_exception = {
        let profiles = profiles.clone();
        let modal = modal.clone();
        let exc_mode = exc_mode.clone();
        let exc_type = exc_type.clone();
        let exc_notify_call = exc_notify_call.clone();
        let form_whatsapp = form_whatsapp.clone();
        let form_telegram = form_telegram.clone();
        let form_signal = form_signal.clone();
        let form_whatsapp_room_id = form_whatsapp_room_id.clone();
        let form_telegram_room_id = form_telegram_room_id.clone();
        let form_signal_room_id = form_signal_room_id.clone();
        let form_whatsapp_selected = form_whatsapp_selected.clone();
        let form_telegram_selected = form_telegram_selected.clone();
        let form_signal_selected = form_signal_selected.clone();
        let error_msg = error_msg.clone();
        let saving = saving.clone();
        let fetch_profiles = fetch_profiles.clone();
        Callback::from(move |(profile_id, platform): (i32, String)| {
            let profile = profiles.iter().find(|p| p.id == profile_id).cloned();
            let Some(profile) = profile else { return };

            // Validate that the bridge chat was selected from search results
            let chat_val = match platform.as_str() {
                "whatsapp" => (*form_whatsapp).clone(),
                "telegram" => (*form_telegram).clone(),
                "signal" => (*form_signal).clone(),
                _ => String::new(),
            };
            let is_selected = match platform.as_str() {
                "whatsapp" => *form_whatsapp_selected,
                "telegram" => *form_telegram_selected,
                "signal" => *form_signal_selected,
                _ => true,
            };
            if !chat_val.is_empty() && !is_selected {
                error_msg.set(Some("Select a chat from the search results.".to_string()));
                return;
            }

            // Build exceptions list, replacing the one for this platform
            let mut exceptions: Vec<ExceptionRequest> = profile.exceptions.iter()
                .filter(|e| e.platform != platform)
                .map(|e| ExceptionRequest {
                    platform: e.platform.clone(),
                    notification_mode: e.notification_mode.clone(),
                    notification_type: e.notification_type.clone(),
                    notify_on_call: e.notify_on_call,
                })
                .collect();

            exceptions.push(ExceptionRequest {
                platform: platform.clone(),
                notification_mode: (*exc_mode).clone(),
                notification_type: (*exc_type).clone(),
                notify_on_call: *exc_notify_call,
            });

            // Use form state for the current platform's chat, profile values for others
            let (wa_chat, wa_rid) = if platform == "whatsapp" {
                let v = (*form_whatsapp).clone();
                (if v.is_empty() { None } else { Some(v) }, (*form_whatsapp_room_id).clone())
            } else {
                (profile.whatsapp_chat.clone(), profile.whatsapp_room_id.clone())
            };
            let (tg_chat, tg_rid) = if platform == "telegram" {
                let v = (*form_telegram).clone();
                (if v.is_empty() { None } else { Some(v) }, (*form_telegram_room_id).clone())
            } else {
                (profile.telegram_chat.clone(), profile.telegram_room_id.clone())
            };
            let (sg_chat, sg_rid) = if platform == "signal" {
                let v = (*form_signal).clone();
                (if v.is_empty() { None } else { Some(v) }, (*form_signal_room_id).clone())
            } else {
                (profile.signal_chat.clone(), profile.signal_room_id.clone())
            };

            let request = CreateProfileRequest {
                nickname: profile.nickname.clone(),
                whatsapp_chat: wa_chat,
                telegram_chat: tg_chat,
                signal_chat: sg_chat,
                email_addresses: profile.email_addresses.clone(),
                notification_mode: profile.notification_mode.clone(),
                notification_type: profile.notification_type.clone(),
                notify_on_call: profile.notify_on_call,
                exceptions: if exceptions.is_empty() { None } else { Some(exceptions) },
                whatsapp_room_id: wa_rid,
                telegram_room_id: tg_rid,
                signal_room_id: sg_rid,
                notes: profile.notes.clone(),
            };

            let modal = modal.clone();
            let error_msg = error_msg.clone();
            let saving = saving.clone();
            let fetch_profiles = fetch_profiles.clone();

            saving.set(true);
            spawn_local(async move {
                if let Ok(req) = Api::put(&format!("/api/contact-profiles/{}", profile_id)).json(&request) {
                    if let Ok(response) = req.send().await {
                        if response.ok() {
                            dispatch_sync_event();
                            fetch_profiles.emit(());
                            modal.set(None);
                        } else {
                            error_msg.set(Some("Failed to save".to_string()));
                        }
                    } else {
                        error_msg.set(Some("Network error".to_string()));
                    }
                }
                saving.set(false);
            });
        })
    };

    // -----------------------------------------------------------------------
    // Save chat-only changes (no exception created/modified)
    // -----------------------------------------------------------------------
    let save_chat_only = {
        let profiles = profiles.clone();
        let modal = modal.clone();
        let form_whatsapp = form_whatsapp.clone();
        let form_telegram = form_telegram.clone();
        let form_signal = form_signal.clone();
        let form_whatsapp_room_id = form_whatsapp_room_id.clone();
        let form_telegram_room_id = form_telegram_room_id.clone();
        let form_signal_room_id = form_signal_room_id.clone();
        let form_whatsapp_selected = form_whatsapp_selected.clone();
        let form_telegram_selected = form_telegram_selected.clone();
        let form_signal_selected = form_signal_selected.clone();
        let error_msg = error_msg.clone();
        let saving = saving.clone();
        let fetch_profiles = fetch_profiles.clone();
        Callback::from(move |(profile_id, platform): (i32, String)| {
            let profile = profiles.iter().find(|p| p.id == profile_id).cloned();
            let Some(profile) = profile else { return };

            // Validate that the bridge chat was selected from search results
            let chat_val = match platform.as_str() {
                "whatsapp" => (*form_whatsapp).clone(),
                "telegram" => (*form_telegram).clone(),
                "signal" => (*form_signal).clone(),
                _ => String::new(),
            };
            let is_selected = match platform.as_str() {
                "whatsapp" => *form_whatsapp_selected,
                "telegram" => *form_telegram_selected,
                "signal" => *form_signal_selected,
                _ => true,
            };
            if !chat_val.is_empty() && !is_selected {
                error_msg.set(Some("Select a chat from the search results.".to_string()));
                return;
            }

            // Keep existing exceptions but exclude this platform (override mode is off)
            let exceptions: Vec<ExceptionRequest> = profile.exceptions.iter()
                .filter(|e| e.platform != platform)
                .map(|e| ExceptionRequest {
                    platform: e.platform.clone(),
                    notification_mode: e.notification_mode.clone(),
                    notification_type: e.notification_type.clone(),
                    notify_on_call: e.notify_on_call,
                })
                .collect();

            // Use form state for the current platform's chat, profile values for others
            let (wa_chat, wa_rid) = if platform == "whatsapp" {
                let v = (*form_whatsapp).clone();
                (if v.is_empty() { None } else { Some(v) }, (*form_whatsapp_room_id).clone())
            } else {
                (profile.whatsapp_chat.clone(), profile.whatsapp_room_id.clone())
            };
            let (tg_chat, tg_rid) = if platform == "telegram" {
                let v = (*form_telegram).clone();
                (if v.is_empty() { None } else { Some(v) }, (*form_telegram_room_id).clone())
            } else {
                (profile.telegram_chat.clone(), profile.telegram_room_id.clone())
            };
            let (sg_chat, sg_rid) = if platform == "signal" {
                let v = (*form_signal).clone();
                (if v.is_empty() { None } else { Some(v) }, (*form_signal_room_id).clone())
            } else {
                (profile.signal_chat.clone(), profile.signal_room_id.clone())
            };

            // Always send Some() so backend replaces exceptions (None = "don't change")
            let request = CreateProfileRequest {
                nickname: profile.nickname.clone(),
                whatsapp_chat: wa_chat,
                telegram_chat: tg_chat,
                signal_chat: sg_chat,
                email_addresses: profile.email_addresses.clone(),
                notification_mode: profile.notification_mode.clone(),
                notification_type: profile.notification_type.clone(),
                notify_on_call: profile.notify_on_call,
                exceptions: Some(exceptions),
                whatsapp_room_id: wa_rid,
                telegram_room_id: tg_rid,
                signal_room_id: sg_rid,
                notes: profile.notes.clone(),
            };

            let modal = modal.clone();
            let error_msg = error_msg.clone();
            let saving = saving.clone();
            let fetch_profiles = fetch_profiles.clone();

            saving.set(true);
            spawn_local(async move {
                if let Ok(req) = Api::put(&format!("/api/contact-profiles/{}", profile_id)).json(&request) {
                    if let Ok(response) = req.send().await {
                        if response.ok() {
                            dispatch_sync_event();
                            fetch_profiles.emit(());
                            modal.set(None);
                        } else {
                            error_msg.set(Some("Failed to save".to_string()));
                        }
                    } else {
                        error_msg.set(Some("Network error".to_string()));
                    }
                }
                saving.set(false);
            });
        })
    };

    // -----------------------------------------------------------------------
    // Remove chat assignment for a platform
    // -----------------------------------------------------------------------
    let remove_chat = {
        let profiles = profiles.clone();
        let modal = modal.clone();
        let error_msg = error_msg.clone();
        let saving = saving.clone();
        let fetch_profiles = fetch_profiles.clone();
        Callback::from(move |(profile_id, platform): (i32, String)| {
            let profile = profiles.iter().find(|p| p.id == profile_id).cloned();
            let Some(profile) = profile else { return };

            // Remove exception for this platform too (no chat = no exception needed)
            let exceptions: Vec<ExceptionRequest> = profile.exceptions.iter()
                .filter(|e| e.platform != platform)
                .map(|e| ExceptionRequest {
                    platform: e.platform.clone(),
                    notification_mode: e.notification_mode.clone(),
                    notification_type: e.notification_type.clone(),
                    notify_on_call: e.notify_on_call,
                })
                .collect();

            // Clear the chat and room_id for the target platform
            let (whatsapp_chat, whatsapp_room_id) = if platform == "whatsapp" {
                (None, None)
            } else {
                (profile.whatsapp_chat.clone(), profile.whatsapp_room_id.clone())
            };
            let (telegram_chat, telegram_room_id) = if platform == "telegram" {
                (None, None)
            } else {
                (profile.telegram_chat.clone(), profile.telegram_room_id.clone())
            };
            let (signal_chat, signal_room_id) = if platform == "signal" {
                (None, None)
            } else {
                (profile.signal_chat.clone(), profile.signal_room_id.clone())
            };

            let request = CreateProfileRequest {
                nickname: profile.nickname.clone(),
                whatsapp_chat,
                telegram_chat,
                signal_chat,
                email_addresses: profile.email_addresses.clone(),
                notification_mode: profile.notification_mode.clone(),
                notification_type: profile.notification_type.clone(),
                notify_on_call: profile.notify_on_call,
                exceptions: if exceptions.is_empty() { None } else { Some(exceptions) },
                whatsapp_room_id,
                telegram_room_id,
                signal_room_id,
                notes: profile.notes.clone(),
            };

            let modal = modal.clone();
            let error_msg = error_msg.clone();
            let saving = saving.clone();
            let fetch_profiles = fetch_profiles.clone();

            saving.set(true);
            spawn_local(async move {
                if let Ok(req) = Api::put(&format!("/api/contact-profiles/{}", profile_id)).json(&request) {
                    if let Ok(response) = req.send().await {
                        if response.ok() {
                            dispatch_sync_event();
                            fetch_profiles.emit(());
                            modal.set(None);
                        } else {
                            error_msg.set(Some("Failed to remove chat".to_string()));
                        }
                    } else {
                        error_msg.set(Some("Network error".to_string()));
                    }
                }
                saving.set(false);
            });
        })
    };

    // -----------------------------------------------------------------------
    // Save default settings
    // -----------------------------------------------------------------------
    let save_defaults = {
        let modal = modal.clone();
        let default_mode = default_mode.clone();
        let default_noti_type = default_noti_type.clone();
        let default_notify_on_call = default_notify_on_call.clone();
        let def_form_mode = def_form_mode.clone();
        let def_form_type = def_form_type.clone();
        let def_form_notify_call = def_form_notify_call.clone();
        let error_msg = error_msg.clone();
        let saving = saving.clone();
        Callback::from(move |_: ()| {
            let modal = modal.clone();
            let default_mode = default_mode.clone();
            let default_noti_type = default_noti_type.clone();
            let default_notify_on_call = default_notify_on_call.clone();
            let new_mode = (*def_form_mode).clone();
            let new_type = (*def_form_type).clone();
            let new_call = *def_form_notify_call;
            let error_msg = error_msg.clone();
            let saving = saving.clone();

            saving.set(true);
            spawn_local(async move {
                let request = UpdateDefaultModeRequest {
                    mode: Some(new_mode.clone()),
                    noti_type: Some(new_type.clone()),
                    notify_on_call: Some(new_call),
                };
                if let Ok(req) = Api::put("/api/contact-profiles/default-mode").json(&request) {
                    if let Ok(response) = req.send().await {
                        if response.ok() {
                            default_mode.set(new_mode);
                            default_noti_type.set(new_type);
                            default_notify_on_call.set(new_call);
                            dispatch_sync_event();
                            modal.set(None);
                        } else {
                            error_msg.set(Some("Failed to save".to_string()));
                        }
                    } else {
                        error_msg.set(Some("Network error".to_string()));
                    }
                }
                saving.set(false);
            });
        })
    };

    // -----------------------------------------------------------------------
    // Save phone contact defaults
    // -----------------------------------------------------------------------
    let save_phone_contact_defaults = {
        let modal = modal.clone();
        let phone_contact_mode = phone_contact_mode.clone();
        let phone_contact_noti_type = phone_contact_noti_type.clone();
        let phone_contact_notify_on_call = phone_contact_notify_on_call.clone();
        let pc_form_mode = pc_form_mode.clone();
        let pc_form_type = pc_form_type.clone();
        let pc_form_notify_call = pc_form_notify_call.clone();
        let error_msg = error_msg.clone();
        let saving = saving.clone();
        Callback::from(move |_: ()| {
            let modal = modal.clone();
            let phone_contact_mode = phone_contact_mode.clone();
            let phone_contact_noti_type = phone_contact_noti_type.clone();
            let phone_contact_notify_on_call = phone_contact_notify_on_call.clone();
            let new_mode = (*pc_form_mode).clone();
            let new_type = (*pc_form_type).clone();
            let new_call = *pc_form_notify_call;
            let error_msg = error_msg.clone();
            let saving = saving.clone();

            saving.set(true);
            spawn_local(async move {
                let request = UpdatePhoneContactModeRequest {
                    mode: Some(new_mode.clone()),
                    noti_type: Some(new_type.clone()),
                    notify_on_call: Some(new_call),
                };
                if let Ok(req) = Api::put("/api/contact-profiles/phone-contact-mode").json(&request) {
                    if let Ok(response) = req.send().await {
                        if response.ok() {
                            phone_contact_mode.set(new_mode);
                            phone_contact_noti_type.set(new_type);
                            phone_contact_notify_on_call.set(new_call);
                            dispatch_sync_event();
                            modal.set(None);
                        } else {
                            error_msg.set(Some("Failed to save".to_string()));
                        }
                    } else {
                        error_msg.set(Some("Network error".to_string()));
                    }
                }
                saving.set(false);
            });
        })
    };

    // -----------------------------------------------------------------------
    // Save new contact
    // -----------------------------------------------------------------------
    let save_new_contact = {
        let modal = modal.clone();
        let add_nickname = add_nickname.clone();
        let add_whatsapp = add_whatsapp.clone();
        let add_telegram = add_telegram.clone();
        let add_signal = add_signal.clone();
        let add_email = add_email.clone();
        let add_mode = add_mode.clone();
        let add_type = add_type.clone();
        let add_notify_call = add_notify_call.clone();
        let add_whatsapp_room_id = add_whatsapp_room_id.clone();
        let add_telegram_room_id = add_telegram_room_id.clone();
        let add_signal_room_id = add_signal_room_id.clone();
        let add_whatsapp_selected = add_whatsapp_selected.clone();
        let add_telegram_selected = add_telegram_selected.clone();
        let add_signal_selected = add_signal_selected.clone();
        let error_msg = error_msg.clone();
        let saving = saving.clone();
        let fetch_profiles = fetch_profiles.clone();
        Callback::from(move |_: ()| {
            let nickname = (*add_nickname).trim().to_string();
            if nickname.is_empty() {
                error_msg.set(Some("Nickname cannot be empty.".to_string()));
                return;
            }
            if nickname.contains('@') {
                error_msg.set(Some("Nickname cannot contain '@'.".to_string()));
                return;
            }

            let wa = (*add_whatsapp).clone();
            let tg = (*add_telegram).clone();
            let sg = (*add_signal).clone();
            if (!wa.is_empty() && !*add_whatsapp_selected)
                || (!tg.is_empty() && !*add_telegram_selected)
                || (!sg.is_empty() && !*add_signal_selected) {
                error_msg.set(Some("Select a chat from the search results.".to_string()));
                return;
            }

            let whatsapp = if add_whatsapp.is_empty() { None } else { Some((*add_whatsapp).clone()) };
            let telegram = if add_telegram.is_empty() { None } else { Some((*add_telegram).clone()) };
            let signal = if add_signal.is_empty() { None } else { Some((*add_signal).clone()) };
            let email = if add_email.is_empty() { None } else { Some((*add_email).clone()) };

            let request = CreateProfileRequest {
                nickname,
                whatsapp_chat: whatsapp,
                telegram_chat: telegram,
                signal_chat: signal,
                email_addresses: email,
                notification_mode: (*add_mode).clone(),
                notification_type: (*add_type).clone(),
                notify_on_call: *add_notify_call,
                exceptions: None,
                whatsapp_room_id: (*add_whatsapp_room_id).clone(),
                telegram_room_id: (*add_telegram_room_id).clone(),
                signal_room_id: (*add_signal_room_id).clone(),
                notes: None,
            };

            let modal = modal.clone();
            let error_msg = error_msg.clone();
            let saving = saving.clone();
            let fetch_profiles = fetch_profiles.clone();

            saving.set(true);
            spawn_local(async move {
                if let Ok(req) = Api::post("/api/contact-profiles").json(&request) {
                    if let Ok(response) = req.send().await {
                        if response.ok() {
                            dispatch_sync_event();
                            fetch_profiles.emit(());
                            modal.set(None);
                        } else if let Ok(body) = response.json::<serde_json::Value>().await {
                            let msg = body["error"].as_str().unwrap_or("Failed to create contact");
                            error_msg.set(Some(msg.to_string()));
                        } else {
                            error_msg.set(Some("Failed to create contact".to_string()));
                        }
                    } else {
                        error_msg.set(Some("Network error".to_string()));
                    }
                }
                saving.set(false);
            });
        })
    };

    // -----------------------------------------------------------------------
    // Render helpers
    // -----------------------------------------------------------------------

    let render_avatar = |profile: &ContactProfile| -> Html {
        let id = profile.id;
        let nick = profile.nickname.clone();
        let initials = initials_for_name(&nick);
        let bg = color_for_name(&nick);
        let platforms = connected_platforms(profile);
        let count = platforms.len();

        let on_avatar = {
            let cb = on_avatar_click.clone();
            Callback::from(move |e: MouseEvent| {
                e.stop_propagation();
                cb.emit(id);
            })
        };

        let bubbles = platforms.iter().enumerate().map(|(idx, pi)| {
            let pos = bubble_position_class(count, idx);
            let has_override = find_exception(profile, pi.key).is_some();
            let overridden_class = if has_override { " overridden" } else { "" };
            let platform_key = pi.key.to_string();
            let icon_class = pi.icon.to_string();
            let eff_mode = effective_mode_for_platform(profile, pi.key);
            // Only show badge if this platform has an override that differs from the contact default
            let badge = if has_override && eff_mode != profile.notification_mode.as_str() {
                mode_badge_info(eff_mode)
            } else {
                None
            };
            let on_click = {
                let cb = on_bubble_click.clone();
                let pk = platform_key.clone();
                Callback::from(move |e: MouseEvent| {
                    e.stop_propagation();
                    cb.emit((id, pk.clone()));
                })
            };

            let badge_html = if let Some((badge_icon, badge_css)) = badge {
                html! {
                    <span class={format!("mode-badge {}", badge_css)}>
                        <i class={badge_icon}></i>
                    </span>
                }
            } else {
                html! {}
            };

            html! {
                <div
                    class={format!("platform-bubble {}{}", pos, overridden_class)}
                    style={format!("background: {};", pi.color)}
                    onclick={on_click}
                    title={pi.label.to_string()}
                >
                    <i class={icon_class}></i>
                    {badge_html}
                </div>
            }
        }).collect::<Html>();

        let noti_type_html = match profile.notification_type.as_str() {
            "sms" => html! {
                <div class="avatar-noti-type">
                    <i class="fa-solid fa-comment-sms"></i>
                </div>
            },
            "call" => html! {
                <div class="avatar-noti-type">
                    <i class="fa-solid fa-phone"></i>
                </div>
            },
            _ => html! {},
        };

        // Glow color from primary platform, or avatar background color
        let glow_color = if !platforms.is_empty() {
            platforms[0].color
        } else {
            bg
        };

        // Floating particles: up to 2 tiny platform-colored icons drifting in
        // Each particle shows the platform icon plus a filter-mode marker.
        // Platforms in "ignore" mode produce no particle at all.
        let particles = {
            let mut particle_htmls = Vec::new();
            let classes = ["from-top", "from-right p2"];
            for (i, pi) in platforms.iter().enumerate().take(2) {
                let mode = effective_mode_for_platform(profile, pi.key);
                if mode == "ignore" {
                    continue;
                }
                let marker = match mode {
                    "critical" => html! {
                        <i class="fa-solid fa-exclamation" style="font-size:0.55rem;color:#f59e0b;margin-left:1px;"></i>
                    },
                    "mention" => html! {
                        <i class="fa-solid fa-at" style="font-size:0.55rem;color:#3b82f6;margin-left:1px;"></i>
                    },
                    _ => html! {},
                };
                let cls = classes.get(i).unwrap_or(&"from-top");
                particle_htmls.push(html! {
                    <span class={format!("avatar-particle {}", cls)}>
                        <i class={pi.icon} style={format!("font-size:0.7rem;color:{};", pi.color)}></i>
                        {marker}
                    </span>
                });
            }
            html! { <>{ for particle_htmls }</> }
        };

        html! {
            <div class="avatar-item" onclick={on_avatar}>
                <div class="avatar-circle-wrap">
                    <span class="avatar-glow" style={format!("background: {};", glow_color)}></span>
                    <div class="avatar-circle" style={format!("background: {};", bg)}>
                        {initials}
                    </div>
                    {bubbles}
                    {particles}
                </div>
                <span class="avatar-label" title={nick.clone()}>{nick}</span>
                {noti_type_html}
            </div>
        }
    };

    let render_phone_contact_avatar = {
        let on_click = on_phone_contact_click.clone();
        html! {
            <div class="avatar-item" onclick={on_click}>
                <div class="avatar-circle-wrap">
                    <div class="avatar-circle default-avatar">
                        <i class="fa-solid fa-address-book"></i>
                    </div>
                </div>
                <span class="avatar-label">{"Contacts"}</span>
            </div>
        }
    };

    let render_unknown_avatar = {
        let on_click = on_default_click.clone();
        html! {
            <div class="avatar-item" onclick={on_click}>
                <div class="avatar-circle-wrap">
                    <div class="avatar-circle default-avatar">
                        <i class="fa-solid fa-user-question"></i>
                    </div>
                </div>
                <span class="avatar-label">{"Unknown"}</span>
            </div>
        }
    };

    let render_add_avatar = {
        let on_click = on_add_click.clone();
        html! {
            <div class="avatar-item" onclick={on_click}>
                <div class="avatar-circle-wrap">
                    <div class="avatar-circle add-avatar">
                        <i class="fa-solid fa-plus"></i>
                    </div>
                </div>
                <span class="avatar-label">{"Add"}</span>
            </div>
        }
    };

    // -----------------------------------------------------------------------
    // Render modals
    // -----------------------------------------------------------------------

    let render_modal = || -> Html {
        let modal_val = (*modal).clone();
        let Some(modal_type) = modal_val else {
            return html! {};
        };

        let close = close_modal.clone();
        let on_overlay_click = {
            let close = close.clone();
            Callback::from(move |_: MouseEvent| {
                close.emit(());
            })
        };
        let stop_prop = Callback::from(|e: MouseEvent| { e.stop_propagation(); });

        match modal_type {
            ModalType::ContactSettings(pid) => {
                let profile = profiles.iter().find(|p| p.id == pid).cloned();
                let Some(profile) = profile else { return html! {} };
                let has_whatsapp = profile.whatsapp_chat.is_some();
                let has_telegram = profile.telegram_chat.is_some();
                let has_signal = profile.signal_chat.is_some();

                let err = (*error_msg).clone();
                let is_saving = *saving;

                // Nickname
                let on_nick = {
                    let form_nickname = form_nickname.clone();
                    Callback::from(move |e: Event| {
                        let target: HtmlInputElement = e.target_unchecked_into();
                        form_nickname.set(target.value());
                    })
                };

                // Bridge search inputs
                let on_form_whatsapp_input = {
                    let form_whatsapp = form_whatsapp.clone();
                    let form_whatsapp_room_id = form_whatsapp_room_id.clone();
                    let form_whatsapp_selected = form_whatsapp_selected.clone();
                    let search_chats = search_chats.clone();
                    Callback::from(move |e: InputEvent| {
                        let target: HtmlInputElement = e.target_unchecked_into();
                        let value = target.value();
                        form_whatsapp.set(value.clone());
                        form_whatsapp_room_id.set(None);
                        form_whatsapp_selected.set(false);
                        search_chats.emit(("whatsapp".to_string(), value));
                    })
                };
                let on_form_telegram_input = {
                    let form_telegram = form_telegram.clone();
                    let form_telegram_room_id = form_telegram_room_id.clone();
                    let form_telegram_selected = form_telegram_selected.clone();
                    let search_chats = search_chats.clone();
                    Callback::from(move |e: InputEvent| {
                        let target: HtmlInputElement = e.target_unchecked_into();
                        let value = target.value();
                        form_telegram.set(value.clone());
                        form_telegram_room_id.set(None);
                        form_telegram_selected.set(false);
                        search_chats.emit(("telegram".to_string(), value));
                    })
                };
                let on_form_signal_input = {
                    let form_signal = form_signal.clone();
                    let form_signal_room_id = form_signal_room_id.clone();
                    let form_signal_selected = form_signal_selected.clone();
                    let search_chats = search_chats.clone();
                    Callback::from(move |e: InputEvent| {
                        let target: HtmlInputElement = e.target_unchecked_into();
                        let value = target.value();
                        form_signal.set(value.clone());
                        form_signal_room_id.set(None);
                        form_signal_selected.set(false);
                        search_chats.emit(("signal".to_string(), value));
                    })
                };
                let on_form_email_input = {
                    let form_email = form_email.clone();
                    Callback::from(move |e: InputEvent| {
                        let target: HtmlInputElement = e.target_unchecked_into();
                        form_email.set(target.value());
                    })
                };

                // WhatsApp suggestions for settings modal
                let settings_wa_suggestions = if *show_whatsapp_suggestions {
                    let results = (*whatsapp_results).clone();
                    let searching = *searching_whatsapp;
                    let err = (*search_error_whatsapp).clone();
                    html! {
                        <div class="suggestions-dropdown">
                            if searching {
                                <div class="suggestion-item searching">{"Searching..."}</div>
                            } else if let Some(err) = err {
                                <div class="suggestion-item error">{err}</div>
                            } else if results.is_empty() {
                                <div class="suggestion-item no-results">{"No chats found"}</div>
                            } else {
                                { for results.iter().map(|room| {
                                    let name = room.display_name.clone();
                                    let rid = room.room_id.clone();
                                    let is_group = room.is_group;
                                    let attached = room.attached_to.clone();
                                    let is_disabled = attached.is_some();
                                    let form_whatsapp = form_whatsapp.clone();
                                    let form_whatsapp_room_id = form_whatsapp_room_id.clone();
                                    let form_whatsapp_selected = form_whatsapp_selected.clone();
                                    let show_whatsapp_suggestions = show_whatsapp_suggestions.clone();
                                    let item_class = if is_disabled { "suggestion-item disabled" } else { "suggestion-item" };
                                    let on_click = if is_disabled {
                                        Callback::noop()
                                    } else {
                                        let name = name.clone();
                                        let rid = rid.clone();
                                        Callback::from(move |_: MouseEvent| {
                                            form_whatsapp.set(name.clone());
                                            let rid_opt = if rid.is_empty() { None } else { Some(rid.clone()) };
                                            form_whatsapp_room_id.set(rid_opt);
                                            form_whatsapp_selected.set(true);
                                            show_whatsapp_suggestions.set(false);
                                        })
                                    };
                                    let right_text = if let Some(ref owner) = attached {
                                        format!("Attached to {}", owner)
                                    } else {
                                        String::new()
                                    };
                                    html! {
                                        <div class={item_class} onclick={on_click}>
                                            <span>{&room.display_name}</span>
                                            if is_group {
                                                <span class="group-tag">{"Group"}</span>
                                            }
                                            if room.is_phone_contact == Some(true) {
                                                <span class="contact-tag">{"Saved contact"}</span>
                                            }
                                            if room.is_phone_contact == Some(false) {
                                                <span class="contact-tag push-name">{"Push name"}</span>
                                            }
                                            <span style="color:#666;font-size:0.75rem;margin-left:auto;padding-left:0.5rem;">{right_text}</span>
                                        </div>
                                    }
                                })}
                            }
                        </div>
                    }
                } else {
                    html! {}
                };

                // Telegram suggestions for settings modal
                let settings_tg_suggestions = if *show_telegram_suggestions {
                    let results = (*telegram_results).clone();
                    let searching = *searching_telegram;
                    let err = (*search_error_telegram).clone();
                    html! {
                        <div class="suggestions-dropdown">
                            if searching {
                                <div class="suggestion-item searching">{"Searching..."}</div>
                            } else if let Some(err) = err {
                                <div class="suggestion-item error">{err}</div>
                            } else if results.is_empty() {
                                <div class="suggestion-item no-results">{"No chats found"}</div>
                            } else {
                                { for results.iter().map(|room| {
                                    let name = room.display_name.clone();
                                    let rid = room.room_id.clone();
                                    let is_group = room.is_group;
                                    let attached = room.attached_to.clone();
                                    let is_disabled = attached.is_some();
                                    let form_telegram = form_telegram.clone();
                                    let form_telegram_room_id = form_telegram_room_id.clone();
                                    let form_telegram_selected = form_telegram_selected.clone();
                                    let show_telegram_suggestions = show_telegram_suggestions.clone();
                                    let item_class = if is_disabled { "suggestion-item disabled" } else { "suggestion-item" };
                                    let on_click = if is_disabled {
                                        Callback::noop()
                                    } else {
                                        let name = name.clone();
                                        let rid = rid.clone();
                                        Callback::from(move |_: MouseEvent| {
                                            form_telegram.set(name.clone());
                                            let rid_opt = if rid.is_empty() { None } else { Some(rid.clone()) };
                                            form_telegram_room_id.set(rid_opt);
                                            form_telegram_selected.set(true);
                                            show_telegram_suggestions.set(false);
                                        })
                                    };
                                    let right_text = if let Some(ref owner) = attached {
                                        format!("Attached to {}", owner)
                                    } else {
                                        String::new()
                                    };
                                    html! {
                                        <div class={item_class} onclick={on_click}>
                                            <span>{&room.display_name}</span>
                                            if is_group {
                                                <span class="group-tag">{"Group"}</span>
                                            }
                                            if room.is_phone_contact == Some(true) {
                                                <span class="contact-tag">{"Saved contact"}</span>
                                            }
                                            if room.is_phone_contact == Some(false) {
                                                <span class="contact-tag push-name">{"Push name"}</span>
                                            }
                                            <span style="color:#666;font-size:0.75rem;margin-left:auto;padding-left:0.5rem;">{right_text}</span>
                                        </div>
                                    }
                                })}
                            }
                        </div>
                    }
                } else {
                    html! {}
                };

                // Signal suggestions for settings modal
                let settings_sg_suggestions = if *show_signal_suggestions {
                    let results = (*signal_results).clone();
                    let searching = *searching_signal;
                    let err = (*search_error_signal).clone();
                    html! {
                        <div class="suggestions-dropdown">
                            if searching {
                                <div class="suggestion-item searching">{"Searching..."}</div>
                            } else if let Some(err) = err {
                                <div class="suggestion-item error">{err}</div>
                            } else if results.is_empty() {
                                <div class="suggestion-item no-results">{"No chats found"}</div>
                            } else {
                                { for results.iter().map(|room| {
                                    let name = room.display_name.clone();
                                    let rid = room.room_id.clone();
                                    let is_group = room.is_group;
                                    let attached = room.attached_to.clone();
                                    let is_disabled = attached.is_some();
                                    let form_signal = form_signal.clone();
                                    let form_signal_room_id = form_signal_room_id.clone();
                                    let form_signal_selected = form_signal_selected.clone();
                                    let show_signal_suggestions = show_signal_suggestions.clone();
                                    let item_class = if is_disabled { "suggestion-item disabled" } else { "suggestion-item" };
                                    let on_click = if is_disabled {
                                        Callback::noop()
                                    } else {
                                        let name = name.clone();
                                        let rid = rid.clone();
                                        Callback::from(move |_: MouseEvent| {
                                            form_signal.set(name.clone());
                                            let rid_opt = if rid.is_empty() { None } else { Some(rid.clone()) };
                                            form_signal_room_id.set(rid_opt);
                                            form_signal_selected.set(true);
                                            show_signal_suggestions.set(false);
                                        })
                                    };
                                    let right_text = if let Some(ref owner) = attached {
                                        format!("Attached to {}", owner)
                                    } else {
                                        String::new()
                                    };
                                    html! {
                                        <div class={item_class} onclick={on_click}>
                                            <span>{&room.display_name}</span>
                                            if is_group {
                                                <span class="group-tag">{"Group"}</span>
                                            }
                                            if room.is_phone_contact == Some(true) {
                                                <span class="contact-tag">{"Saved contact"}</span>
                                            }
                                            if room.is_phone_contact == Some(false) {
                                                <span class="contact-tag push-name">{"Push name"}</span>
                                            }
                                            <span style="color:#666;font-size:0.75rem;margin-left:auto;padding-left:0.5rem;">{right_text}</span>
                                        </div>
                                    }
                                })}
                            }
                        </div>
                    }
                } else {
                    html! {}
                };

                // Notes
                let on_notes_input = {
                    let form_notes = form_notes.clone();
                    Callback::from(move |e: InputEvent| {
                        let target: HtmlInputElement = e.target_unchecked_into();
                        form_notes.set(target.value());
                    })
                };

                // Mode
                let on_mode = {
                    let form_mode = form_mode.clone();
                    Callback::from(move |e: Event| {
                        let target: HtmlSelectElement = e.target_unchecked_into();
                        form_mode.set(target.value());
                    })
                };

                // Type
                let on_type = {
                    let form_type = form_type.clone();
                    Callback::from(move |e: Event| {
                        let target: HtmlSelectElement = e.target_unchecked_into();
                        form_type.set(target.value());
                    })
                };

                // Notify on call
                let on_call = {
                    let form_notify_call = form_notify_call.clone();
                    let current = *form_notify_call;
                    Callback::from(move |_: Event| {
                        form_notify_call.set(!current);
                    })
                };

                let on_save = {
                    let save = save_contact.clone();
                    Callback::from(move |_: MouseEvent| { save.emit(pid); })
                };

                let on_delete = {
                    let del = delete_contact.clone();
                    Callback::from(move |_: MouseEvent| { del.emit(pid); })
                };

                let current_mode = (*form_mode).clone();

                html! {
                    <div class="avatar-modal-overlay" onclick={on_overlay_click}>
                        <div class="avatar-modal-box" onclick={stop_prop}>
                            <div class="avatar-modal-platform-header">
                                <h3>{"Contact Settings"}</h3>
                                <button class="avatar-row-info-btn" onclick={{
                                    let modal = modal.clone();
                                    Callback::from(move |e: MouseEvent| {
                                        e.stop_propagation();
                                        modal.set(Some(ModalType::ContactSettingsInfo(pid)));
                                    })
                                }}>
                                    <i class="fa-solid fa-circle-info"></i>
                                </button>
                            </div>
                            if let Some(e) = err {
                                <div class="avatar-modal-error">{e}</div>
                            }
                            <div class="avatar-modal-row">
                                <label>{"Nickname"}</label>
                                <input type="text" value={(*form_nickname).clone()} onchange={on_nick} />
                            </div>
                            if !has_whatsapp {
                                <div class="avatar-modal-row">
                                    <label>{"WhatsApp"}</label>
                                    <div class="input-with-suggestions">
                                        <input type="text" value={(*form_whatsapp).clone()} oninput={on_form_whatsapp_input}
                                            class={if !form_whatsapp.is_empty() && !*form_whatsapp_selected { "warn-border" } else { "" }}
                                            placeholder="Search chat name" />
                                        {settings_wa_suggestions}
                                    </div>
                                </div>
                            }
                            if !has_telegram {
                                <div class="avatar-modal-row">
                                    <label>{"Telegram"}</label>
                                    <div class="input-with-suggestions">
                                        <input type="text" value={(*form_telegram).clone()} oninput={on_form_telegram_input}
                                            class={if !form_telegram.is_empty() && !*form_telegram_selected { "warn-border" } else { "" }}
                                            placeholder="Search chat name" />
                                        {settings_tg_suggestions}
                                    </div>
                                </div>
                            }
                            if !has_signal {
                                <div class="avatar-modal-row">
                                    <label>{"Signal"}</label>
                                    <div class="input-with-suggestions">
                                        <input type="text" value={(*form_signal).clone()} oninput={on_form_signal_input}
                                            class={if !form_signal.is_empty() && !*form_signal_selected { "warn-border" } else { "" }}
                                            placeholder="Search chat name" />
                                        {settings_sg_suggestions}
                                    </div>
                                </div>
                            }
                            <div class="avatar-modal-row">
                                <label>{"Email"}</label>
                                <input type="text" value={(*form_email).clone()} oninput={on_form_email_input}
                                    placeholder="email@example.com" />
                            </div>
                            <div class="avatar-modal-row">
                                <label>{"Notes"}</label>
                                <textarea class="avatar-modal-notes" rows="2"
                                    value={(*form_notes).clone()}
                                    oninput={on_notes_input}
                                    placeholder="e.g. My mom. Reply in Finnish." />
                                <div class="avatar-modal-notes-hint">{"Helps AI draft better replies"}</div>
                            </div>
                            <div class="avatar-modal-row">
                                <label>{"Notification mode"}</label>
                                <select onchange={on_mode}>
                                    <option value="all" selected={current_mode == "all"}>{"All"}</option>
                                    <option value="critical" selected={current_mode == "critical"}>{"Critical"}</option>
                                </select>
                            </div>
                            <div class="avatar-modal-row">
                                <label>{"Notification type"}</label>
                                <select onchange={on_type}>
                                    <option value="sms" selected={*form_type == "sms"}>{"SMS"}</option>
                                    <option value="call" selected={*form_type == "call"}>{"Call (+SMS)"}</option>
                                </select>
                            </div>
                            <div class="avatar-modal-check">
                                <input type="checkbox" id="av-notify-call" checked={*form_notify_call} onchange={on_call} />
                                <label for="av-notify-call">{"Notify on incoming call"}</label>
                            </div>
                            <div class="avatar-modal-actions">
                                <button class="avatar-modal-btn-delete" onclick={on_delete} disabled={is_saving}>{"Delete"}</button>
                                <button class="avatar-modal-btn-cancel" onclick={{
                                    let close = close.clone();
                                    Callback::from(move |_: MouseEvent| close.emit(()))
                                }}>{"Cancel"}</button>
                                <button class="avatar-modal-btn-save" onclick={on_save} disabled={is_saving}>
                                    {if is_saving { "Saving..." } else { "Save" }}
                                </button>
                            </div>
                        </div>
                    </div>
                }
            }

            ModalType::PlatformException(pid, ref platform) => {
                let platform = platform.clone();
                let profile = profiles.iter().find(|p| p.id == pid).cloned();
                let Some(profile) = profile else { return html! {} };

                let pi = PLATFORMS.iter().find(|p| p.key == platform.as_str());
                let Some(pi) = pi else { return html! {} };

                let err = (*error_msg).clone();
                let is_saving = *saving;
                let is_bridge = platform != "email";

                // Chat search input for this platform
                let (exc_chat_value, exc_chat_not_selected, exc_chat_suggestions, on_exc_chat_input) = if is_bridge {
                    let (chat_val, not_selected) = match platform.as_str() {
                        "whatsapp" => ((*form_whatsapp).clone(), !*form_whatsapp_selected),
                        "telegram" => ((*form_telegram).clone(), !*form_telegram_selected),
                        "signal" => ((*form_signal).clone(), !*form_signal_selected),
                        _ => (String::new(), true),
                    };

                    let on_input = {
                        let platform = platform.clone();
                        let form_whatsapp = form_whatsapp.clone();
                        let form_telegram = form_telegram.clone();
                        let form_signal = form_signal.clone();
                        let form_whatsapp_room_id = form_whatsapp_room_id.clone();
                        let form_telegram_room_id = form_telegram_room_id.clone();
                        let form_signal_room_id = form_signal_room_id.clone();
                        let form_whatsapp_selected = form_whatsapp_selected.clone();
                        let form_telegram_selected = form_telegram_selected.clone();
                        let form_signal_selected = form_signal_selected.clone();
                        let search_chats = search_chats.clone();
                        Callback::from(move |e: InputEvent| {
                            let target: HtmlInputElement = e.target_unchecked_into();
                            let value = target.value();
                            match platform.as_str() {
                                "whatsapp" => { form_whatsapp.set(value.clone()); form_whatsapp_room_id.set(None); form_whatsapp_selected.set(false); }
                                "telegram" => { form_telegram.set(value.clone()); form_telegram_room_id.set(None); form_telegram_selected.set(false); }
                                "signal" => { form_signal.set(value.clone()); form_signal_room_id.set(None); form_signal_selected.set(false); }
                                _ => {}
                            }
                            search_chats.emit((platform.clone(), value));
                        })
                    };

                    let suggestions = match platform.as_str() {
                        "whatsapp" => if *show_whatsapp_suggestions {
                            let results = (*whatsapp_results).clone();
                            let searching = *searching_whatsapp;
                            let err = (*search_error_whatsapp).clone();
                            html! {
                                <div class="suggestions-dropdown">
                                    if searching {
                                        <div class="suggestion-item searching">{"Searching..."}</div>
                                    } else if let Some(err) = err {
                                        <div class="suggestion-item error">{err}</div>
                                    } else if results.is_empty() {
                                        <div class="suggestion-item no-results">{"No chats found"}</div>
                                    } else {
                                        { for results.iter().map(|room| {
                                            let name = room.display_name.clone();
                                            let rid = room.room_id.clone();
                                            let is_group = room.is_group;
                                            let attached = room.attached_to.clone();
                                            let is_disabled = attached.is_some();
                                            let form_whatsapp = form_whatsapp.clone();
                                            let form_whatsapp_room_id = form_whatsapp_room_id.clone();
                                            let form_whatsapp_selected = form_whatsapp_selected.clone();
                                            let show_whatsapp_suggestions = show_whatsapp_suggestions.clone();
                                            let item_class = if is_disabled { "suggestion-item disabled" } else { "suggestion-item" };
                                            let on_click = if is_disabled {
                                                Callback::noop()
                                            } else {
                                                let name = name.clone();
                                                let rid = rid.clone();
                                                Callback::from(move |_: MouseEvent| {
                                                    form_whatsapp.set(name.clone());
                                                    let rid_opt = if rid.is_empty() { None } else { Some(rid.clone()) };
                                                    form_whatsapp_room_id.set(rid_opt);
                                                    form_whatsapp_selected.set(true);
                                                    show_whatsapp_suggestions.set(false);
                                                })
                                            };
                                            let right_text = if let Some(ref owner) = attached {
                                                format!("Attached to {}", owner)
                                            } else {
                                                String::new()
                                            };
                                            html! {
                                                <div class={item_class} onclick={on_click}>
                                                    <span>{&room.display_name}</span>
                                                    if is_group {
                                                        <span class="group-tag">{"Group"}</span>
                                                    }
                                                    if room.is_phone_contact == Some(true) {
                                                        <span class="contact-tag">{"Saved contact"}</span>
                                                    }
                                                    if room.is_phone_contact == Some(false) {
                                                        <span class="contact-tag push-name">{"Push name"}</span>
                                                    }
                                                    <span style="color:#666;font-size:0.75rem;margin-left:auto;padding-left:0.5rem;">{right_text}</span>
                                                </div>
                                            }
                                        })}
                                    }
                                </div>
                            }
                        } else { html! {} },
                        "telegram" => if *show_telegram_suggestions {
                            let results = (*telegram_results).clone();
                            let searching = *searching_telegram;
                            let err = (*search_error_telegram).clone();
                            html! {
                                <div class="suggestions-dropdown">
                                    if searching {
                                        <div class="suggestion-item searching">{"Searching..."}</div>
                                    } else if let Some(err) = err {
                                        <div class="suggestion-item error">{err}</div>
                                    } else if results.is_empty() {
                                        <div class="suggestion-item no-results">{"No chats found"}</div>
                                    } else {
                                        { for results.iter().map(|room| {
                                            let name = room.display_name.clone();
                                            let rid = room.room_id.clone();
                                            let is_group = room.is_group;
                                            let attached = room.attached_to.clone();
                                            let is_disabled = attached.is_some();
                                            let form_telegram = form_telegram.clone();
                                            let form_telegram_room_id = form_telegram_room_id.clone();
                                            let form_telegram_selected = form_telegram_selected.clone();
                                            let show_telegram_suggestions = show_telegram_suggestions.clone();
                                            let item_class = if is_disabled { "suggestion-item disabled" } else { "suggestion-item" };
                                            let on_click = if is_disabled {
                                                Callback::noop()
                                            } else {
                                                let name = name.clone();
                                                let rid = rid.clone();
                                                Callback::from(move |_: MouseEvent| {
                                                    form_telegram.set(name.clone());
                                                    let rid_opt = if rid.is_empty() { None } else { Some(rid.clone()) };
                                                    form_telegram_room_id.set(rid_opt);
                                                    form_telegram_selected.set(true);
                                                    show_telegram_suggestions.set(false);
                                                })
                                            };
                                            let right_text = if let Some(ref owner) = attached {
                                                format!("Attached to {}", owner)
                                            } else {
                                                String::new()
                                            };
                                            html! {
                                                <div class={item_class} onclick={on_click}>
                                                    <span>{&room.display_name}</span>
                                                    if is_group {
                                                        <span class="group-tag">{"Group"}</span>
                                                    }
                                                    if room.is_phone_contact == Some(true) {
                                                        <span class="contact-tag">{"Saved contact"}</span>
                                                    }
                                                    if room.is_phone_contact == Some(false) {
                                                        <span class="contact-tag push-name">{"Push name"}</span>
                                                    }
                                                    <span style="color:#666;font-size:0.75rem;margin-left:auto;padding-left:0.5rem;">{right_text}</span>
                                                </div>
                                            }
                                        })}
                                    }
                                </div>
                            }
                        } else { html! {} },
                        "signal" => if *show_signal_suggestions {
                            let results = (*signal_results).clone();
                            let searching = *searching_signal;
                            let err = (*search_error_signal).clone();
                            html! {
                                <div class="suggestions-dropdown">
                                    if searching {
                                        <div class="suggestion-item searching">{"Searching..."}</div>
                                    } else if let Some(err) = err {
                                        <div class="suggestion-item error">{err}</div>
                                    } else if results.is_empty() {
                                        <div class="suggestion-item no-results">{"No chats found"}</div>
                                    } else {
                                        { for results.iter().map(|room| {
                                            let name = room.display_name.clone();
                                            let rid = room.room_id.clone();
                                            let is_group = room.is_group;
                                            let attached = room.attached_to.clone();
                                            let is_disabled = attached.is_some();
                                            let form_signal = form_signal.clone();
                                            let form_signal_room_id = form_signal_room_id.clone();
                                            let form_signal_selected = form_signal_selected.clone();
                                            let show_signal_suggestions = show_signal_suggestions.clone();
                                            let item_class = if is_disabled { "suggestion-item disabled" } else { "suggestion-item" };
                                            let on_click = if is_disabled {
                                                Callback::noop()
                                            } else {
                                                let name = name.clone();
                                                let rid = rid.clone();
                                                Callback::from(move |_: MouseEvent| {
                                                    form_signal.set(name.clone());
                                                    let rid_opt = if rid.is_empty() { None } else { Some(rid.clone()) };
                                                    form_signal_room_id.set(rid_opt);
                                                    form_signal_selected.set(true);
                                                    show_signal_suggestions.set(false);
                                                })
                                            };
                                            let right_text = if let Some(ref owner) = attached {
                                                format!("Attached to {}", owner)
                                            } else {
                                                String::new()
                                            };
                                            html! {
                                                <div class={item_class} onclick={on_click}>
                                                    <span>{&room.display_name}</span>
                                                    if is_group {
                                                        <span class="group-tag">{"Group"}</span>
                                                    }
                                                    if room.is_phone_contact == Some(true) {
                                                        <span class="contact-tag">{"Saved contact"}</span>
                                                    }
                                                    if room.is_phone_contact == Some(false) {
                                                        <span class="contact-tag push-name">{"Push name"}</span>
                                                    }
                                                    <span style="color:#666;font-size:0.75rem;margin-left:auto;padding-left:0.5rem;">{right_text}</span>
                                                </div>
                                            }
                                        })}
                                    }
                                </div>
                            }
                        } else { html! {} },
                        _ => html! {},
                    };

                    (chat_val, not_selected, suggestions, on_input)
                } else {
                    (String::new(), true, html! {}, Callback::noop())
                };

                let is_override = *exc_override_mode;

                let on_mode = {
                    let exc_mode = exc_mode.clone();
                    Callback::from(move |e: Event| {
                        let target: HtmlSelectElement = e.target_unchecked_into();
                        exc_mode.set(target.value());
                    })
                };

                let on_type = {
                    let exc_type = exc_type.clone();
                    Callback::from(move |e: Event| {
                        let target: HtmlSelectElement = e.target_unchecked_into();
                        exc_type.set(target.value());
                    })
                };

                let on_call = {
                    let exc_notify_call = exc_notify_call.clone();
                    let current = *exc_notify_call;
                    Callback::from(move |_: Event| {
                        exc_notify_call.set(!current);
                    })
                };

                // Save: calls save_exception if override mode, save_chat_only otherwise
                let on_save = if is_override {
                    let save = save_exception.clone();
                    let platform = platform.clone();
                    Callback::from(move |_: MouseEvent| { save.emit((pid, platform.clone())); })
                } else {
                    let save = save_chat_only.clone();
                    let platform = platform.clone();
                    Callback::from(move |_: MouseEvent| { save.emit((pid, platform.clone())); })
                };

                let on_customize = {
                    let exc_override_mode = exc_override_mode.clone();
                    Callback::from(move |_: MouseEvent| {
                        exc_override_mode.set(true);
                    })
                };

                let on_reset_default = {
                    let exc_override_mode = exc_override_mode.clone();
                    let exc_mode = exc_mode.clone();
                    let exc_type = exc_type.clone();
                    let exc_notify_call = exc_notify_call.clone();
                    let profile_mode = profile.notification_mode.clone();
                    let profile_type = profile.notification_type.clone();
                    let profile_call = profile.notify_on_call;
                    Callback::from(move |_: MouseEvent| {
                        // Reset form to contact defaults (Save will persist the change)
                        exc_override_mode.set(false);
                        exc_mode.set(profile_mode.clone());
                        exc_type.set(profile_type.clone());
                        exc_notify_call.set(profile_call);
                    })
                };

                let on_remove_chat = {
                    let remove = remove_chat.clone();
                    let platform = platform.clone();
                    Callback::from(move |_: MouseEvent| { remove.emit((pid, platform.clone())); })
                };

                let has_chat = match platform.as_str() {
                    "whatsapp" => profile.whatsapp_chat.is_some(),
                    "telegram" => profile.telegram_chat.is_some(),
                    "signal" => profile.signal_chat.is_some(),
                    _ => false,
                };

                let current_exc_mode = (*exc_mode).clone();
                let show_type = current_exc_mode != "ignore";

                html! {
                    <div class="avatar-modal-overlay" onclick={on_overlay_click}>
                        <div class="avatar-modal-box" onclick={stop_prop}>
                            <div class="avatar-modal-platform-header">
                                <i class={pi.icon.to_string()} style={format!("color: {};", pi.color)}></i>
                                <h3>{format!("{} for {}", pi.label, profile.nickname)}</h3>
                                <button class="avatar-row-info-btn" onclick={{
                                    let modal = modal.clone();
                                    let platform = platform.clone();
                                    Callback::from(move |e: MouseEvent| {
                                        e.stop_propagation();
                                        modal.set(Some(ModalType::PlatformInfo(pid, platform.clone())));
                                    })
                                }}>
                                    <i class="fa-solid fa-circle-info"></i>
                                </button>
                            </div>
                            if is_bridge {
                                <div class="avatar-modal-row">
                                    <label>{"Chat"}</label>
                                    <div class="input-with-suggestions">
                                        <input type="text" value={exc_chat_value.clone()} oninput={on_exc_chat_input}
                                            class={if !exc_chat_value.is_empty() && exc_chat_not_selected { "warn-border" } else { "" }}
                                            placeholder="Search chat name" />
                                        {exc_chat_suggestions}
                                    </div>
                                </div>
                            }
                            if let Some(e) = err {
                                <div class="avatar-modal-error">{e}</div>
                            }
                            <div class="avatar-modal-override-status">
                                {if is_override { "Custom settings active" } else { "Using contact defaults" }}
                            </div>
                            <div class="avatar-modal-row">
                                <label>{"Notification mode"}</label>
                                <select onchange={on_mode} disabled={!is_override}>
                                    <option value="all" selected={current_exc_mode == "all"}>{"All"}</option>
                                    if is_bridge {
                                        <option value="mention" selected={current_exc_mode == "mention"}>{"@mention only"}</option>
                                    }
                                    <option value="critical" selected={current_exc_mode == "critical"}>{"Critical"}</option>
                                </select>
                            </div>
                            if show_type {
                                <div class="avatar-modal-row">
                                    <label>{"Notification type"}</label>
                                    <select onchange={on_type} disabled={!is_override}>
                                        <option value="sms" selected={*exc_type == "sms"}>{"SMS"}</option>
                                        <option value="call" selected={*exc_type == "call"}>{"Call (+SMS)"}</option>
                                    </select>
                                </div>
                                <div class="avatar-modal-check">
                                    <input type="checkbox" id="av-exc-call" checked={*exc_notify_call} onchange={on_call} disabled={!is_override} />
                                    <label for="av-exc-call">{"Notify on incoming call"}</label>
                                </div>
                            }
                            <div class="avatar-modal-actions">
                                if is_bridge && has_chat {
                                    <button class="avatar-modal-btn-delete" onclick={on_remove_chat} disabled={is_saving}>
                                        {"Remove chat"}
                                    </button>
                                }
                                if is_override {
                                    <button class="avatar-modal-btn-customize" onclick={on_reset_default} disabled={is_saving}>
                                        {"Reset to default"}
                                    </button>
                                } else {
                                    <button class="avatar-modal-btn-customize" onclick={on_customize}>
                                        {"Customize"}
                                    </button>
                                }
                                <button class="avatar-modal-btn-cancel" onclick={{
                                    let close = close.clone();
                                    Callback::from(move |_: MouseEvent| close.emit(()))
                                }}>{"Cancel"}</button>
                                <button class="avatar-modal-btn-save" onclick={on_save} disabled={is_saving}>
                                    {if is_saving { "Saving..." } else { "Save" }}
                                </button>
                            </div>
                        </div>
                    </div>
                }
            }

            ModalType::PhoneContactSettings => {
                let err = (*error_msg).clone();
                let is_saving = *saving;

                let on_mode = {
                    let pc_form_mode = pc_form_mode.clone();
                    Callback::from(move |e: Event| {
                        let target: HtmlSelectElement = e.target_unchecked_into();
                        pc_form_mode.set(target.value());
                    })
                };

                let on_type = {
                    let pc_form_type = pc_form_type.clone();
                    Callback::from(move |e: Event| {
                        let target: HtmlSelectElement = e.target_unchecked_into();
                        pc_form_type.set(target.value());
                    })
                };

                let on_call = {
                    let pc_form_notify_call = pc_form_notify_call.clone();
                    let current = *pc_form_notify_call;
                    Callback::from(move |_: Event| {
                        pc_form_notify_call.set(!current);
                    })
                };

                let on_save = {
                    let save = save_phone_contact_defaults.clone();
                    Callback::from(move |_: MouseEvent| { save.emit(()); })
                };

                let current_pc_mode = (*pc_form_mode).clone();

                html! {
                    <div class="avatar-modal-overlay" onclick={on_overlay_click}>
                        <div class="avatar-modal-box" onclick={stop_prop}>
                            <h3>{"Phone Contacts"}</h3>
                            <p style="font-size:0.8rem;color:#888;margin:0 0 1rem 0;">
                                {"Applied to people saved in your phone contacts who don't have a profile above."}
                            </p>
                            if let Some(e) = err {
                                <div class="avatar-modal-error">{e}</div>
                            }
                            <div class="avatar-modal-row">
                                <label>{"Notification mode"}</label>
                                <select onchange={on_mode}>
                                    <option value="all" selected={current_pc_mode == "all"}>{"All"}</option>
                                    <option value="critical" selected={current_pc_mode == "critical"}>{"Critical"}</option>
                                </select>
                            </div>
                            <div class="avatar-modal-row">
                                <label>{"Notification type"}</label>
                                <select onchange={on_type}>
                                    <option value="sms" selected={*pc_form_type == "sms"}>{"SMS"}</option>
                                    <option value="call" selected={*pc_form_type == "call"}>{"Call (+SMS)"}</option>
                                </select>
                            </div>
                            <div class="avatar-modal-check">
                                <input type="checkbox" id="av-pc-call" checked={*pc_form_notify_call} onchange={on_call} />
                                <label for="av-pc-call">{"Notify on incoming call"}</label>
                            </div>
                            <div class="avatar-modal-actions">
                                <button class="avatar-modal-btn-cancel" onclick={{
                                    let close = close.clone();
                                    Callback::from(move |_: MouseEvent| close.emit(()))
                                }}>{"Cancel"}</button>
                                <button class="avatar-modal-btn-save" onclick={on_save} disabled={is_saving}>
                                    {if is_saving { "Saving..." } else { "Save" }}
                                </button>
                            </div>
                        </div>
                    </div>
                }
            }

            ModalType::DefaultSettings => {
                let err = (*error_msg).clone();
                let is_saving = *saving;

                let on_mode = {
                    let def_form_mode = def_form_mode.clone();
                    Callback::from(move |e: Event| {
                        let target: HtmlSelectElement = e.target_unchecked_into();
                        def_form_mode.set(target.value());
                    })
                };

                let on_type = {
                    let def_form_type = def_form_type.clone();
                    Callback::from(move |e: Event| {
                        let target: HtmlSelectElement = e.target_unchecked_into();
                        def_form_type.set(target.value());
                    })
                };

                let on_call = {
                    let def_form_notify_call = def_form_notify_call.clone();
                    let current = *def_form_notify_call;
                    Callback::from(move |_: Event| {
                        def_form_notify_call.set(!current);
                    })
                };

                let on_save = {
                    let save = save_defaults.clone();
                    Callback::from(move |_: MouseEvent| { save.emit(()); })
                };

                let current_def_mode = (*def_form_mode).clone();

                html! {
                    <div class="avatar-modal-overlay" onclick={on_overlay_click}>
                        <div class="avatar-modal-box" onclick={stop_prop}>
                            <h3>{"Unknown People"}</h3>
                            <p style="font-size:0.8rem;color:#888;margin:0 0 1rem 0;">
                                {"Applied to messages from unknown numbers not in your phone contacts."}
                            </p>
                            if let Some(e) = err {
                                <div class="avatar-modal-error">{e}</div>
                            }
                            <div class="avatar-modal-row">
                                <label>{"Notification mode"}</label>
                                <select onchange={on_mode}>
                                    <option value="all" selected={current_def_mode == "all"}>{"All"}</option>
                                    <option value="critical" selected={current_def_mode == "critical"}>{"Critical"}</option>
                                </select>
                            </div>
                            <div class="avatar-modal-row">
                                <label>{"Notification type"}</label>
                                <select onchange={on_type}>
                                    <option value="sms" selected={*def_form_type == "sms"}>{"SMS"}</option>
                                    <option value="call" selected={*def_form_type == "call"}>{"Call (+SMS)"}</option>
                                </select>
                            </div>
                            <div class="avatar-modal-check">
                                <input type="checkbox" id="av-def-call" checked={*def_form_notify_call} onchange={on_call} />
                                <label for="av-def-call">{"Notify on incoming call"}</label>
                            </div>
                            <div class="avatar-modal-actions">
                                <button class="avatar-modal-btn-cancel" onclick={{
                                    let close = close.clone();
                                    Callback::from(move |_: MouseEvent| close.emit(()))
                                }}>{"Cancel"}</button>
                                <button class="avatar-modal-btn-save" onclick={on_save} disabled={is_saving}>
                                    {if is_saving { "Saving..." } else { "Save" }}
                                </button>
                            </div>
                        </div>
                    </div>
                }
            }

            ModalType::PeopleInfo => {
                html! {
                    <div class="avatar-modal-overlay" onclick={on_overlay_click}>
                        <div class="avatar-modal-box" onclick={stop_prop}>
                            <h3>{"People & Notifications"}</h3>
                            <div class="people-info-section">
                                <p>{"Create profiles for specific contacts or chats to customize how you're notified about their messages."}</p>
                                <p>{"Each profile can be linked to WhatsApp, Telegram, Signal chats, or email addresses."}</p>
                            </div>
                            <div class="people-info-section">
                                <h4>{"Notification Modes"}</h4>
                                <p class="people-info-mode"><strong>{"Critical: "}</strong>{"AI determines urgency - notifies you only when delaying over 2 hours could cause harm, financial loss, or miss a time-sensitive opportunity. Examples: emergency messages, someone asking to meet now, immediate decisions needed. Routine updates and vague requests are not considered critical."}</p>
                                <p class="people-info-mode"><strong>{"All: "}</strong>{"Get notified about every message from this contact."}</p>
                                <p class="people-info-mode"><strong>{"Ignore: "}</strong>{"No notifications from this contact."}</p>
                            </div>
                            <div class="people-info-section">
                                <h4>{"3 Tiers"}</h4>
                                <p class="people-info-mode"><strong>{"Profiles: "}</strong>{"People you've created profiles for above get per-profile notification settings."}</p>
                                <p class="people-info-mode"><strong>{"Contacts: "}</strong>{"People saved in your phone contacts (but without a profile) use \"Contacts\" settings."}</p>
                                <p class="people-info-mode"><strong>{"Unknown: "}</strong>{"Everyone else (unknown numbers not in your phone) uses \"Unknown\" settings."}</p>
                            </div>
                            <div class="people-info-section">
                                <h4>{"How to Use"}</h4>
                                <p>{"Click an avatar to edit settings. Click \"+\" to add a new contact profile. Click \"Contacts\" or \"Unknown\" to change settings for those groups."}</p>
                            </div>
                            <div class="people-info-tip">
                                <strong>{"Tip: "}</strong>{"Use nicknames when chatting with Lightfriend! For example, \"has Mom messaged me?\" or \"send my Boss a WhatsApp message\" will automatically find the right chat."}
                            </div>
                            <button class="people-info-close" onclick={{
                                let close = close.clone();
                                Callback::from(move |_: MouseEvent| close.emit(()))
                            }}>{"Close"}</button>
                        </div>
                    </div>
                }
            }

            ModalType::PlatformInfo(return_pid, ref return_platform) => {
                let return_pid = return_pid;
                let return_platform = return_platform.clone();
                html! {
                    <div class="avatar-modal-overlay" onclick={{
                        let modal = modal.clone();
                        let platform = return_platform.clone();
                        Callback::from(move |_: MouseEvent| {
                            modal.set(Some(ModalType::PlatformException(return_pid, platform.clone())));
                        })
                    }}>
                        <div class="avatar-modal-box" onclick={stop_prop}>
                            <h3>{"Platform Settings"}</h3>
                            <div class="people-info-section">
                                <p>{"Set per-platform notification overrides for this contact. For example, \"All\" for WhatsApp but \"Critical\" for Telegram."}</p>
                                <p>{"If no override is saved, the contact's default settings are used."}</p>
                            </div>
                            <div class="people-info-section">
                                <h4>{"Notification Modes"}</h4>
                                <p class="people-info-mode"><strong>{"All: "}</strong>{"Every message triggers a notification."}</p>
                                <p class="people-info-mode"><strong>{"@mention only: "}</strong>{"Only notifies when you're directly mentioned. Useful for group chats."}</p>
                                <p class="people-info-mode"><strong>{"Critical: "}</strong>{"AI determines urgency - notifies you only when delaying over 2 hours could cause harm, financial loss, or miss a time-sensitive opportunity. Examples: emergency messages, someone asking to meet now, immediate decisions needed. Routine updates and vague requests are not considered critical."}</p>
                                <p class="people-info-mode"><strong>{"Ignore: "}</strong>{"No notifications at all."}</p>
                            </div>
                            <div class="people-info-section">
                                <h4>{"Notification Types"}</h4>
                                <p class="people-info-mode"><strong>{"SMS: "}</strong>{"You receive a text message with the notification."}</p>
                                <p class="people-info-mode"><strong>{"Call: "}</strong>{"Lightfriend calls you and reads the message aloud."}</p>
                                <p class="people-info-mode"><strong>{"Call + SMS: "}</strong>{"Lightfriend calls you first, then sends an SMS. You don't need to answer the call - it's just a ring to get your attention. You won't be charged for unanswered calls."}</p>
                            </div>
                            <div class="people-info-section">
                                <h4>{"Notify on Incoming Call"}</h4>
                                <p>{"When enabled, Lightfriend notifies you if this contact tries to call you on the linked platform (e.g. a WhatsApp call). Useful when your phone is on silent or you're away from it."}</p>
                            </div>
                            <div class="people-info-tip">
                                <strong>{"Tip: "}</strong>{"Use \"Remove chat\" to unlink a chat from this contact without deleting the contact itself."}
                            </div>
                            <button class="people-info-close" onclick={{
                                let modal = modal.clone();
                                let platform = return_platform.clone();
                                Callback::from(move |_: MouseEvent| {
                                    modal.set(Some(ModalType::PlatformException(return_pid, platform.clone())));
                                })
                            }}>{"Back"}</button>
                        </div>
                    </div>
                }
            }

            ModalType::ContactSettingsInfo(return_pid) => {
                let return_pid = return_pid;
                html! {
                    <div class="avatar-modal-overlay" onclick={{
                        let modal = modal.clone();
                        Callback::from(move |_: MouseEvent| {
                            modal.set(Some(ModalType::ContactSettings(return_pid)));
                        })
                    }}>
                        <div class="avatar-modal-box" onclick={stop_prop}>
                            <h3>{"Contact Settings"}</h3>
                            <div class="people-info-section">
                                <p>{"Configure how Lightfriend handles messages from a specific contact."}</p>
                            </div>
                            <div class="people-info-section">
                                <h4>{"Nickname"}</h4>
                                <p>{"The name you use when talking to Lightfriend. For example, \"has Mom messaged me?\" will match a contact nicknamed \"Mom\"."}</p>
                            </div>
                            <div class="people-info-section">
                                <h4>{"Chat Links"}</h4>
                                <p>{"Link WhatsApp, Telegram, or Signal chats to this contact. Search by chat name and select from results."}</p>
                                <p>{"Each chat can only be linked to one contact at a time."}</p>
                            </div>
                            <div class="people-info-section">
                                <h4>{"Email Addresses"}</h4>
                                <p>{"Add comma-separated email addresses to match incoming emails to this contact."}</p>
                            </div>
                            <div class="people-info-section">
                                <h4>{"Notification Modes"}</h4>
                                <p class="people-info-mode"><strong>{"All: "}</strong>{"Every message triggers a notification."}</p>
                                <p class="people-info-mode"><strong>{"Critical: "}</strong>{"AI determines urgency - notifies you only when delaying over 2 hours could cause harm, financial loss, or miss a time-sensitive opportunity. Examples: emergency messages, someone asking to meet now, immediate decisions needed. Routine updates and vague requests are not considered critical."}</p>
                                <p class="people-info-mode"><strong>{"Ignore: "}</strong>{"No notifications at all."}</p>
                            </div>
                            <div class="people-info-section">
                                <h4>{"Notification Types"}</h4>
                                <p class="people-info-mode"><strong>{"SMS: "}</strong>{"You receive a text message with the notification."}</p>
                                <p class="people-info-mode"><strong>{"Call: "}</strong>{"Lightfriend calls you and reads the message aloud."}</p>
                                <p class="people-info-mode"><strong>{"Call + SMS: "}</strong>{"Lightfriend calls you first, then sends an SMS. You don't need to answer the call - it's just a ring to get your attention. You won't be charged for unanswered calls."}</p>
                            </div>
                            <div class="people-info-section">
                                <h4>{"Notify on Incoming Call"}</h4>
                                <p>{"When enabled, Lightfriend notifies you if this contact tries to call you on a linked platform (e.g. a WhatsApp call). Useful when your phone is on silent or you're away from it."}</p>
                            </div>
                            <div class="people-info-tip">
                                <strong>{"Tip: "}</strong>{"Click the colored platform bubbles on a contact's avatar to set per-platform notification overrides."}
                            </div>
                            <button class="people-info-close" onclick={{
                                let modal = modal.clone();
                                Callback::from(move |_: MouseEvent| {
                                    modal.set(Some(ModalType::ContactSettings(return_pid)));
                                })
                            }}>{"Back"}</button>
                        </div>
                    </div>
                }
            }

            ModalType::AddContact => {
                let err = (*error_msg).clone();
                let is_saving = *saving;

                let on_nick = {
                    let add_nickname = add_nickname.clone();
                    Callback::from(move |e: InputEvent| {
                        let target: HtmlInputElement = e.target_unchecked_into();
                        add_nickname.set(target.value());
                    })
                };

                // WhatsApp search input
                let on_whatsapp_input = {
                    let add_whatsapp = add_whatsapp.clone();
                    let add_whatsapp_room_id = add_whatsapp_room_id.clone();
                    let add_whatsapp_selected = add_whatsapp_selected.clone();
                    let search_chats = search_chats.clone();
                    Callback::from(move |e: InputEvent| {
                        let target: HtmlInputElement = e.target_unchecked_into();
                        let value = target.value();
                        add_whatsapp.set(value.clone());
                        add_whatsapp_room_id.set(None);
                        add_whatsapp_selected.set(false);
                        search_chats.emit(("whatsapp".to_string(), value));
                    })
                };

                // Telegram search input
                let on_telegram_input = {
                    let add_telegram = add_telegram.clone();
                    let add_telegram_room_id = add_telegram_room_id.clone();
                    let add_telegram_selected = add_telegram_selected.clone();
                    let search_chats = search_chats.clone();
                    Callback::from(move |e: InputEvent| {
                        let target: HtmlInputElement = e.target_unchecked_into();
                        let value = target.value();
                        add_telegram.set(value.clone());
                        add_telegram_room_id.set(None);
                        add_telegram_selected.set(false);
                        search_chats.emit(("telegram".to_string(), value));
                    })
                };

                // Signal search input
                let on_signal_input = {
                    let add_signal = add_signal.clone();
                    let add_signal_room_id = add_signal_room_id.clone();
                    let add_signal_selected = add_signal_selected.clone();
                    let search_chats = search_chats.clone();
                    Callback::from(move |e: InputEvent| {
                        let target: HtmlInputElement = e.target_unchecked_into();
                        let value = target.value();
                        add_signal.set(value.clone());
                        add_signal_room_id.set(None);
                        add_signal_selected.set(false);
                        search_chats.emit(("signal".to_string(), value));
                    })
                };

                let on_email = {
                    let add_email = add_email.clone();
                    Callback::from(move |e: InputEvent| {
                        let target: HtmlInputElement = e.target_unchecked_into();
                        add_email.set(target.value());
                    })
                };

                let on_mode = {
                    let add_mode = add_mode.clone();
                    Callback::from(move |e: Event| {
                        let target: HtmlSelectElement = e.target_unchecked_into();
                        add_mode.set(target.value());
                    })
                };

                let on_type = {
                    let add_type = add_type.clone();
                    Callback::from(move |e: Event| {
                        let target: HtmlSelectElement = e.target_unchecked_into();
                        add_type.set(target.value());
                    })
                };

                let on_call = {
                    let add_notify_call = add_notify_call.clone();
                    let current = *add_notify_call;
                    Callback::from(move |_: Event| {
                        add_notify_call.set(!current);
                    })
                };

                let on_create = {
                    let save = save_new_contact.clone();
                    Callback::from(move |_: MouseEvent| { save.emit(()); })
                };

                let current_add_mode = (*add_mode).clone();

                // WhatsApp suggestions
                let wa_suggestions = if *show_whatsapp_suggestions {
                    let results = (*whatsapp_results).clone();
                    let searching = *searching_whatsapp;
                    let err = (*search_error_whatsapp).clone();
                    html! {
                        <div class="suggestions-dropdown">
                            if searching {
                                <div class="suggestion-item searching">{"Searching..."}</div>
                            } else if let Some(err) = err {
                                <div class="suggestion-item error">{err}</div>
                            } else if results.is_empty() {
                                <div class="suggestion-item no-results">{"No chats found"}</div>
                            } else {
                                { for results.iter().map(|room| {
                                    let name = room.display_name.clone();
                                    let rid = room.room_id.clone();
                                    let is_group = room.is_group;
                                    let attached = room.attached_to.clone();
                                    let is_disabled = attached.is_some();
                                    let add_whatsapp = add_whatsapp.clone();
                                    let add_whatsapp_room_id = add_whatsapp_room_id.clone();
                                    let add_whatsapp_selected = add_whatsapp_selected.clone();
                                    let show_whatsapp_suggestions = show_whatsapp_suggestions.clone();
                                    let item_class = if is_disabled { "suggestion-item disabled" } else { "suggestion-item" };
                                    let on_click = if is_disabled {
                                        Callback::noop()
                                    } else {
                                        let name = name.clone();
                                        let rid = rid.clone();
                                        Callback::from(move |_: MouseEvent| {
                                            add_whatsapp.set(name.clone());
                                            let rid_opt = if rid.is_empty() { None } else { Some(rid.clone()) };
                                            add_whatsapp_room_id.set(rid_opt);
                                            add_whatsapp_selected.set(true);
                                            show_whatsapp_suggestions.set(false);
                                        })
                                    };
                                    let right_text = if let Some(ref owner) = attached {
                                        format!("Attached to {}", owner)
                                    } else {
                                        String::new()
                                    };
                                    html! {
                                        <div class={item_class} onclick={on_click}>
                                            <span>{&room.display_name}</span>
                                            if is_group {
                                                <span class="group-tag">{"Group"}</span>
                                            }
                                            if room.is_phone_contact == Some(true) {
                                                <span class="contact-tag">{"Saved contact"}</span>
                                            }
                                            if room.is_phone_contact == Some(false) {
                                                <span class="contact-tag push-name">{"Push name"}</span>
                                            }
                                            <span style="color:#666;font-size:0.75rem;margin-left:auto;padding-left:0.5rem;">{right_text}</span>
                                        </div>
                                    }
                                })}
                            }
                        </div>
                    }
                } else {
                    html! {}
                };

                // Telegram suggestions
                let tg_suggestions = if *show_telegram_suggestions {
                    let results = (*telegram_results).clone();
                    let searching = *searching_telegram;
                    let err = (*search_error_telegram).clone();
                    html! {
                        <div class="suggestions-dropdown">
                            if searching {
                                <div class="suggestion-item searching">{"Searching..."}</div>
                            } else if let Some(err) = err {
                                <div class="suggestion-item error">{err}</div>
                            } else if results.is_empty() {
                                <div class="suggestion-item no-results">{"No chats found"}</div>
                            } else {
                                { for results.iter().map(|room| {
                                    let name = room.display_name.clone();
                                    let rid = room.room_id.clone();
                                    let is_group = room.is_group;
                                    let attached = room.attached_to.clone();
                                    let is_disabled = attached.is_some();
                                    let add_telegram = add_telegram.clone();
                                    let add_telegram_room_id = add_telegram_room_id.clone();
                                    let add_telegram_selected = add_telegram_selected.clone();
                                    let show_telegram_suggestions = show_telegram_suggestions.clone();
                                    let item_class = if is_disabled { "suggestion-item disabled" } else { "suggestion-item" };
                                    let on_click = if is_disabled {
                                        Callback::noop()
                                    } else {
                                        let name = name.clone();
                                        let rid = rid.clone();
                                        Callback::from(move |_: MouseEvent| {
                                            add_telegram.set(name.clone());
                                            let rid_opt = if rid.is_empty() { None } else { Some(rid.clone()) };
                                            add_telegram_room_id.set(rid_opt);
                                            add_telegram_selected.set(true);
                                            show_telegram_suggestions.set(false);
                                        })
                                    };
                                    let right_text = if let Some(ref owner) = attached {
                                        format!("Attached to {}", owner)
                                    } else {
                                        String::new()
                                    };
                                    html! {
                                        <div class={item_class} onclick={on_click}>
                                            <span>{&room.display_name}</span>
                                            if is_group {
                                                <span class="group-tag">{"Group"}</span>
                                            }
                                            if room.is_phone_contact == Some(true) {
                                                <span class="contact-tag">{"Saved contact"}</span>
                                            }
                                            if room.is_phone_contact == Some(false) {
                                                <span class="contact-tag push-name">{"Push name"}</span>
                                            }
                                            <span style="color:#666;font-size:0.75rem;margin-left:auto;padding-left:0.5rem;">{right_text}</span>
                                        </div>
                                    }
                                })}
                            }
                        </div>
                    }
                } else {
                    html! {}
                };

                // Signal suggestions
                let sg_suggestions = if *show_signal_suggestions {
                    let results = (*signal_results).clone();
                    let searching = *searching_signal;
                    let err = (*search_error_signal).clone();
                    html! {
                        <div class="suggestions-dropdown">
                            if searching {
                                <div class="suggestion-item searching">{"Searching..."}</div>
                            } else if let Some(err) = err {
                                <div class="suggestion-item error">{err}</div>
                            } else if results.is_empty() {
                                <div class="suggestion-item no-results">{"No chats found"}</div>
                            } else {
                                { for results.iter().map(|room| {
                                    let name = room.display_name.clone();
                                    let rid = room.room_id.clone();
                                    let is_group = room.is_group;
                                    let attached = room.attached_to.clone();
                                    let is_disabled = attached.is_some();
                                    let add_signal = add_signal.clone();
                                    let add_signal_room_id = add_signal_room_id.clone();
                                    let add_signal_selected = add_signal_selected.clone();
                                    let show_signal_suggestions = show_signal_suggestions.clone();
                                    let item_class = if is_disabled { "suggestion-item disabled" } else { "suggestion-item" };
                                    let on_click = if is_disabled {
                                        Callback::noop()
                                    } else {
                                        let name = name.clone();
                                        let rid = rid.clone();
                                        Callback::from(move |_: MouseEvent| {
                                            add_signal.set(name.clone());
                                            let rid_opt = if rid.is_empty() { None } else { Some(rid.clone()) };
                                            add_signal_room_id.set(rid_opt);
                                            add_signal_selected.set(true);
                                            show_signal_suggestions.set(false);
                                        })
                                    };
                                    let right_text = if let Some(ref owner) = attached {
                                        format!("Attached to {}", owner)
                                    } else {
                                        String::new()
                                    };
                                    html! {
                                        <div class={item_class} onclick={on_click}>
                                            <span>{&room.display_name}</span>
                                            if is_group {
                                                <span class="group-tag">{"Group"}</span>
                                            }
                                            if room.is_phone_contact == Some(true) {
                                                <span class="contact-tag">{"Saved contact"}</span>
                                            }
                                            if room.is_phone_contact == Some(false) {
                                                <span class="contact-tag push-name">{"Push name"}</span>
                                            }
                                            <span style="color:#666;font-size:0.75rem;margin-left:auto;padding-left:0.5rem;">{right_text}</span>
                                        </div>
                                    }
                                })}
                            }
                        </div>
                    }
                } else {
                    html! {}
                };

                html! {
                    <div class="avatar-modal-overlay" onclick={on_overlay_click}>
                        <div class="avatar-modal-box" onclick={stop_prop}>
                            <h3>{"Add Contact"}</h3>
                            if let Some(e) = err {
                                <div class="avatar-modal-error">{e}</div>
                            }
                            <div class="avatar-modal-row">
                                <label>{"Nickname"}</label>
                                <input type="text" value={(*add_nickname).clone()} oninput={on_nick}
                                    placeholder="e.g. Mom, Boss, Partner" />
                            </div>
                            <div class="avatar-modal-row">
                                <label>{"WhatsApp"}</label>
                                <div class="input-with-suggestions">
                                    <input type="text" value={(*add_whatsapp).clone()} oninput={on_whatsapp_input}
                                        class={if !add_whatsapp.is_empty() && !*add_whatsapp_selected { "warn-border" } else { "" }}
                                        placeholder="Search chat name" />
                                    {wa_suggestions}
                                </div>
                            </div>
                            <div class="avatar-modal-row">
                                <label>{"Telegram"}</label>
                                <div class="input-with-suggestions">
                                    <input type="text" value={(*add_telegram).clone()} oninput={on_telegram_input}
                                        class={if !add_telegram.is_empty() && !*add_telegram_selected { "warn-border" } else { "" }}
                                        placeholder="Search chat name" />
                                    {tg_suggestions}
                                </div>
                            </div>
                            <div class="avatar-modal-row">
                                <label>{"Signal"}</label>
                                <div class="input-with-suggestions">
                                    <input type="text" value={(*add_signal).clone()} oninput={on_signal_input}
                                        class={if !add_signal.is_empty() && !*add_signal_selected { "warn-border" } else { "" }}
                                        placeholder="Search chat name" />
                                    {sg_suggestions}
                                </div>
                            </div>
                            <div class="avatar-modal-row">
                                <label>{"Email"}</label>
                                <input type="text" value={(*add_email).clone()} oninput={on_email}
                                    placeholder="email@example.com" />
                            </div>
                            <div class="avatar-modal-row">
                                <label>{"Notification mode"}</label>
                                <select onchange={on_mode}>
                                    <option value="all" selected={current_add_mode == "all"}>{"All"}</option>
                                    <option value="critical" selected={current_add_mode == "critical"}>{"Critical"}</option>
                                </select>
                            </div>
                            <div class="avatar-modal-row">
                                <label>{"Notification type"}</label>
                                <select onchange={on_type}>
                                    <option value="sms" selected={*add_type == "sms"}>{"SMS"}</option>
                                    <option value="call" selected={*add_type == "call"}>{"Call (+SMS)"}</option>
                                </select>
                            </div>
                            <div class="avatar-modal-check">
                                <input type="checkbox" id="av-add-call" checked={*add_notify_call} onchange={on_call} />
                                <label for="av-add-call">{"Notify on incoming call"}</label>
                            </div>
                            <div class="avatar-modal-actions">
                                <button class="avatar-modal-btn-cancel" onclick={{
                                    let close = close.clone();
                                    Callback::from(move |_: MouseEvent| close.emit(()))
                                }}>{"Cancel"}</button>
                                <button class="avatar-modal-btn-save" onclick={on_create} disabled={is_saving}>
                                    {if is_saving { "Creating..." } else { "Create" }}
                                </button>
                            </div>
                        </div>
                    </div>
                }
            }

        }
    };

    // -----------------------------------------------------------------------
    // Main render
    // -----------------------------------------------------------------------

    if *loading {
        return html! {
            <>
                <style>{AVATAR_ROW_STYLES}</style>
                <div class="avatar-row-wrap">
                    <div class="avatar-row">
                        <span style="color:#666;font-size:0.8rem;">{"Loading contacts..."}</span>
                    </div>
                </div>
            </>
        };
    }

    let profile_avatars = profiles.iter().map(|p| render_avatar(p)).collect::<Html>();

    html! {
        <>
            <style>{AVATAR_ROW_STYLES}</style>
            <div class="avatar-row-wrap">
                <div class="avatar-row">
                    <div class="avatar-row-info">
                        <button class="avatar-row-info-btn" onclick={{
                            let modal = modal.clone();
                            Callback::from(move |e: MouseEvent| {
                                e.stop_propagation();
                                modal.set(Some(ModalType::PeopleInfo));
                            })
                        }}>
                            <i class="fa-solid fa-circle-info"></i>
                        </button>
                    </div>
                    {profile_avatars}
                    {render_phone_contact_avatar}
                    {render_unknown_avatar}
                    {render_add_avatar}
                </div>
            </div>
            {render_modal()}
        </>
    }
}
