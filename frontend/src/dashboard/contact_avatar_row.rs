use yew::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlInputElement, HtmlSelectElement, Event, MouseEvent, InputEvent};
use crate::utils::api::Api;
use crate::proactive::contact_profiles::{
    ContactProfile, ContactProfilesResponse, ProfileException,
    CreateProfileRequest, ExceptionRequest, UpdateDefaultModeRequest,
    Room, SearchResponse,
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
    padding: 0.5rem 0.25rem;
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
    transition: transform 0.15s ease;
}
.platform-bubble:hover {
    transform: scale(1.2);
    z-index: 3;
}

.platform-bubble.ignored {
    background: #555 !important;
    position: relative;
}
.platform-bubble.ignored::after {
    content: '';
    position: absolute;
    width: 2px;
    height: 14px;
    background: #e55;
    transform: rotate(-45deg);
    border-radius: 1px;
}

.bubble-pos-br { bottom: -4px; right: -4px; }
.bubble-pos-bl { bottom: -4px; left: -4px; }
.bubble-pos-tr { top: -4px; right: -4px; }
.bubble-pos-tl { top: -4px; left: -4px; }

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
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 1rem;
}
.avatar-modal-platform-header i {
    font-size: 1.2rem;
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

fn is_platform_ignored(profile: &ContactProfile, platform_key: &str) -> bool {
    if let Some(exc) = find_exception(profile, platform_key) {
        exc.notification_mode == "ignore"
    } else {
        profile.notification_mode == "ignore"
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
    AddContact,
    PeopleInfo,
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

#[function_component(ContactAvatarRow)]
pub fn contact_avatar_row() -> Html {
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

    // Platform exception form state
    let exc_mode = use_state(|| String::new());
    let exc_type = use_state(|| "sms".to_string());
    let exc_notify_call = use_state(|| true);

    // Default settings form state
    let def_form_mode = use_state(|| "critical".to_string());
    let def_form_type = use_state(|| "sms".to_string());
    let def_form_notify_call = use_state(|| true);

    // Add contact form state
    let add_nickname = use_state(|| String::new());
    let add_whatsapp = use_state(|| String::new());
    let add_telegram = use_state(|| String::new());
    let add_signal = use_state(|| String::new());
    let add_email = use_state(|| String::new());
    let add_mode = use_state(|| "critical".to_string());
    let add_type = use_state(|| "sms".to_string());
    let add_notify_call = use_state(|| true);

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
        let loading = loading.clone();
        Callback::from(move |_: ()| {
            let profiles = profiles.clone();
            let default_mode = default_mode.clone();
            let default_noti_type = default_noti_type.clone();
            let default_notify_on_call = default_notify_on_call.clone();
            let loading = loading.clone();
            spawn_local(async move {
                if let Ok(response) = Api::get("/api/contact-profiles").send().await {
                    if let Ok(data) = response.json::<ContactProfilesResponse>().await {
                        profiles.set(data.profiles);
                        default_mode.set(data.default_mode);
                        default_noti_type.set(data.default_noti_type);
                        default_notify_on_call.set(data.default_notify_on_call);
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
    let on_avatar_click = {
        let modal = modal.clone();
        let profiles = profiles.clone();
        let form_nickname = form_nickname.clone();
        let form_mode = form_mode.clone();
        let form_type = form_type.clone();
        let form_notify_call = form_notify_call.clone();
        let error_msg = error_msg.clone();
        Callback::from(move |profile_id: i32| {
            if let Some(p) = profiles.iter().find(|p| p.id == profile_id) {
                form_nickname.set(p.nickname.clone());
                form_mode.set(p.notification_mode.clone());
                form_type.set(p.notification_type.clone());
                form_notify_call.set(p.notify_on_call);
                error_msg.set(None);
                modal.set(Some(ModalType::ContactSettings(profile_id)));
            }
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
        let error_msg = error_msg.clone();
        Callback::from(move |(profile_id, platform): (i32, String)| {
            if let Some(p) = profiles.iter().find(|p| p.id == profile_id) {
                if let Some(exc) = find_exception(p, &platform) {
                    exc_mode.set(exc.notification_mode.clone());
                    exc_type.set(exc.notification_type.clone());
                    exc_notify_call.set(exc.notify_on_call);
                } else {
                    exc_mode.set(p.notification_mode.clone());
                    exc_type.set(p.notification_type.clone());
                    exc_notify_call.set(p.notify_on_call);
                }
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

            let exceptions: Vec<ExceptionRequest> = profile.exceptions.iter().map(|e| ExceptionRequest {
                platform: e.platform.clone(),
                notification_mode: e.notification_mode.clone(),
                notification_type: e.notification_type.clone(),
                notify_on_call: e.notify_on_call,
            }).collect();

            let request = CreateProfileRequest {
                nickname,
                whatsapp_chat: profile.whatsapp_chat.clone(),
                telegram_chat: profile.telegram_chat.clone(),
                signal_chat: profile.signal_chat.clone(),
                email_addresses: profile.email_addresses.clone(),
                notification_mode: (*form_mode).clone(),
                notification_type: (*form_type).clone(),
                notify_on_call: *form_notify_call,
                exceptions: if exceptions.is_empty() { None } else { Some(exceptions) },
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
        let error_msg = error_msg.clone();
        let saving = saving.clone();
        let fetch_profiles = fetch_profiles.clone();
        Callback::from(move |(profile_id, platform): (i32, String)| {
            let profile = profiles.iter().find(|p| p.id == profile_id).cloned();
            let Some(profile) = profile else { return };

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

            let request = CreateProfileRequest {
                nickname: profile.nickname.clone(),
                whatsapp_chat: profile.whatsapp_chat.clone(),
                telegram_chat: profile.telegram_chat.clone(),
                signal_chat: profile.signal_chat.clone(),
                email_addresses: profile.email_addresses.clone(),
                notification_mode: profile.notification_mode.clone(),
                notification_type: profile.notification_type.clone(),
                notify_on_call: profile.notify_on_call,
                exceptions: if exceptions.is_empty() { None } else { Some(exceptions) },
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
    // Remove platform exception (use contact default)
    // -----------------------------------------------------------------------
    let remove_exception = {
        let profiles = profiles.clone();
        let modal = modal.clone();
        let error_msg = error_msg.clone();
        let saving = saving.clone();
        let fetch_profiles = fetch_profiles.clone();
        Callback::from(move |(profile_id, platform): (i32, String)| {
            let profile = profiles.iter().find(|p| p.id == profile_id).cloned();
            let Some(profile) = profile else { return };

            let exceptions: Vec<ExceptionRequest> = profile.exceptions.iter()
                .filter(|e| e.platform != platform)
                .map(|e| ExceptionRequest {
                    platform: e.platform.clone(),
                    notification_mode: e.notification_mode.clone(),
                    notification_type: e.notification_type.clone(),
                    notify_on_call: e.notify_on_call,
                })
                .collect();

            let request = CreateProfileRequest {
                nickname: profile.nickname.clone(),
                whatsapp_chat: profile.whatsapp_chat.clone(),
                telegram_chat: profile.telegram_chat.clone(),
                signal_chat: profile.signal_chat.clone(),
                email_addresses: profile.email_addresses.clone(),
                notification_mode: profile.notification_mode.clone(),
                notification_type: profile.notification_type.clone(),
                notify_on_call: profile.notify_on_call,
                exceptions: if exceptions.is_empty() { None } else { Some(exceptions) },
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
            let ignored = is_platform_ignored(profile, pi.key);
            let bubble_bg = if ignored { "#555".to_string() } else { pi.color.to_string() };
            let ignored_class = if ignored { " ignored" } else { "" };
            let platform_key = pi.key.to_string();
            let icon_class = pi.icon.to_string();
            let on_click = {
                let cb = on_bubble_click.clone();
                let pk = platform_key.clone();
                Callback::from(move |e: MouseEvent| {
                    e.stop_propagation();
                    cb.emit((id, pk.clone()));
                })
            };

            html! {
                <div
                    class={format!("platform-bubble {}{}", pos, ignored_class)}
                    style={format!("background: {};", bubble_bg)}
                    onclick={on_click}
                    title={pi.label.to_string()}
                >
                    <i class={icon_class}></i>
                </div>
            }
        }).collect::<Html>();

        html! {
            <div class="avatar-item" onclick={on_avatar}>
                <div class="avatar-circle-wrap">
                    <div class="avatar-circle" style={format!("background: {};", bg)}>
                        {initials}
                    </div>
                    {bubbles}
                </div>
                <span class="avatar-label" title={nick.clone()}>{nick}</span>
            </div>
        }
    };

    let render_default_avatar = {
        let on_click = on_default_click.clone();
        html! {
            <div class="avatar-item" onclick={on_click}>
                <div class="avatar-circle-wrap">
                    <div class="avatar-circle default-avatar">
                        <i class="fa-solid fa-users"></i>
                    </div>
                </div>
                <span class="avatar-label">{"Default"}</span>
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
                let Some(_profile) = profile else { return html! {} };

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
                let show_type = current_mode != "ignore" && current_mode != "digest";

                html! {
                    <div class="avatar-modal-overlay" onclick={on_overlay_click}>
                        <div class="avatar-modal-box" onclick={stop_prop}>
                            <h3>{"Contact Settings"}</h3>
                            if let Some(e) = err {
                                <div class="avatar-modal-error">{e}</div>
                            }
                            <div class="avatar-modal-row">
                                <label>{"Nickname"}</label>
                                <input type="text" value={(*form_nickname).clone()} onchange={on_nick} />
                            </div>
                            <div class="avatar-modal-row">
                                <label>{"Notification mode"}</label>
                                <select onchange={on_mode}>
                                    <option value="all" selected={current_mode == "all"}>{"All"}</option>
                                    <option value="critical" selected={current_mode == "critical"}>{"Critical"}</option>
                                    <option value="digest" selected={current_mode == "digest"}>{"Digest"}</option>
                                    <option value="ignore" selected={current_mode == "ignore"}>{"Ignore"}</option>
                                </select>
                            </div>
                            if show_type {
                                <div class="avatar-modal-row">
                                    <label>{"Notification type"}</label>
                                    <select onchange={on_type}>
                                        <option value="sms" selected={*form_type == "sms"}>{"SMS"}</option>
                                        <option value="call" selected={*form_type == "call"}>{"Call"}</option>
                                        <option value="call+sms" selected={*form_type == "call+sms"}>{"Call + SMS"}</option>
                                    </select>
                                </div>
                                <div class="avatar-modal-check">
                                    <input type="checkbox" id="av-notify-call" checked={*form_notify_call} onchange={on_call} />
                                    <label for="av-notify-call">{"Notify on incoming call"}</label>
                                </div>
                            }
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

                let has_exception = find_exception(&profile, &platform).is_some();
                let err = (*error_msg).clone();
                let is_saving = *saving;

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

                let on_save = {
                    let save = save_exception.clone();
                    let platform = platform.clone();
                    Callback::from(move |_: MouseEvent| { save.emit((pid, platform.clone())); })
                };

                let on_remove = {
                    let remove = remove_exception.clone();
                    let platform = platform.clone();
                    Callback::from(move |_: MouseEvent| { remove.emit((pid, platform.clone())); })
                };

                let current_exc_mode = (*exc_mode).clone();
                let show_type = current_exc_mode != "ignore" && current_exc_mode != "digest";

                html! {
                    <div class="avatar-modal-overlay" onclick={on_overlay_click}>
                        <div class="avatar-modal-box" onclick={stop_prop}>
                            <div class="avatar-modal-platform-header">
                                <i class={pi.icon.to_string()} style={format!("color: {};", pi.color)}></i>
                                <h3>{format!("{} for {}", pi.label, profile.nickname)}</h3>
                            </div>
                            if let Some(e) = err {
                                <div class="avatar-modal-error">{e}</div>
                            }
                            <div class="avatar-modal-row">
                                <label>{"Notification mode"}</label>
                                <select onchange={on_mode}>
                                    <option value="all" selected={current_exc_mode == "all"}>{"All"}</option>
                                    <option value="critical" selected={current_exc_mode == "critical"}>{"Critical"}</option>
                                    <option value="digest" selected={current_exc_mode == "digest"}>{"Digest"}</option>
                                    <option value="ignore" selected={current_exc_mode == "ignore"}>{"Ignore"}</option>
                                </select>
                            </div>
                            if show_type {
                                <div class="avatar-modal-row">
                                    <label>{"Notification type"}</label>
                                    <select onchange={on_type}>
                                        <option value="sms" selected={*exc_type == "sms"}>{"SMS"}</option>
                                        <option value="call" selected={*exc_type == "call"}>{"Call"}</option>
                                        <option value="call+sms" selected={*exc_type == "call+sms"}>{"Call + SMS"}</option>
                                    </select>
                                </div>
                                <div class="avatar-modal-check">
                                    <input type="checkbox" id="av-exc-call" checked={*exc_notify_call} onchange={on_call} />
                                    <label for="av-exc-call">{"Notify on incoming call"}</label>
                                </div>
                            }
                            <div class="avatar-modal-actions">
                                if has_exception {
                                    <button class="avatar-modal-btn-default" onclick={on_remove} disabled={is_saving}>
                                        {"Use contact default"}
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
                let show_type = current_def_mode != "ignore" && current_def_mode != "digest";

                html! {
                    <div class="avatar-modal-overlay" onclick={on_overlay_click}>
                        <div class="avatar-modal-box" onclick={stop_prop}>
                            <h3>{"Default Settings"}</h3>
                            <p style="font-size:0.8rem;color:#888;margin:0 0 1rem 0;">
                                {"Applied to messages from contacts not listed above."}
                            </p>
                            if let Some(e) = err {
                                <div class="avatar-modal-error">{e}</div>
                            }
                            <div class="avatar-modal-row">
                                <label>{"Notification mode"}</label>
                                <select onchange={on_mode}>
                                    <option value="all" selected={current_def_mode == "all"}>{"All"}</option>
                                    <option value="critical" selected={current_def_mode == "critical"}>{"Critical"}</option>
                                    <option value="digest" selected={current_def_mode == "digest"}>{"Digest"}</option>
                                    <option value="ignore" selected={current_def_mode == "ignore"}>{"Ignore"}</option>
                                </select>
                            </div>
                            if show_type {
                                <div class="avatar-modal-row">
                                    <label>{"Notification type"}</label>
                                    <select onchange={on_type}>
                                        <option value="sms" selected={*def_form_type == "sms"}>{"SMS"}</option>
                                        <option value="call" selected={*def_form_type == "call"}>{"Call"}</option>
                                        <option value="call+sms" selected={*def_form_type == "call+sms"}>{"Call + SMS"}</option>
                                    </select>
                                </div>
                                <div class="avatar-modal-check">
                                    <input type="checkbox" id="av-def-call" checked={*def_form_notify_call} onchange={on_call} />
                                    <label for="av-def-call">{"Notify on incoming call"}</label>
                                </div>
                            }
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
                                <p class="people-info-mode"><strong>{"Critical: "}</strong>{"AI determines urgency - you only get notified for important messages."}</p>
                                <p class="people-info-mode"><strong>{"All: "}</strong>{"Get notified about every message from this contact."}</p>
                                <p class="people-info-mode"><strong>{"Digest: "}</strong>{"Messages are bundled into scheduled digest summaries."}</p>
                                <p class="people-info-mode"><strong>{"Ignore: "}</strong>{"No notifications from this contact."}</p>
                            </div>
                            <div class="people-info-section">
                                <h4>{"How to Use"}</h4>
                                <p>{"Click an avatar to edit their settings. Click \"+\" to add a new contact. Click platform bubbles to set per-platform overrides. Click \"Default\" to change settings for everyone else."}</p>
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
                    let search_chats = search_chats.clone();
                    Callback::from(move |e: InputEvent| {
                        let target: HtmlInputElement = e.target_unchecked_into();
                        let value = target.value();
                        add_whatsapp.set(value.clone());
                        search_chats.emit(("whatsapp".to_string(), value));
                    })
                };

                // Telegram search input
                let on_telegram_input = {
                    let add_telegram = add_telegram.clone();
                    let search_chats = search_chats.clone();
                    Callback::from(move |e: InputEvent| {
                        let target: HtmlInputElement = e.target_unchecked_into();
                        let value = target.value();
                        add_telegram.set(value.clone());
                        search_chats.emit(("telegram".to_string(), value));
                    })
                };

                // Signal search input
                let on_signal_input = {
                    let add_signal = add_signal.clone();
                    let search_chats = search_chats.clone();
                    Callback::from(move |e: InputEvent| {
                        let target: HtmlInputElement = e.target_unchecked_into();
                        let value = target.value();
                        add_signal.set(value.clone());
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
                let show_type = current_add_mode != "ignore" && current_add_mode != "digest";

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
                                    let add_whatsapp = add_whatsapp.clone();
                                    let show_whatsapp_suggestions = show_whatsapp_suggestions.clone();
                                    html! {
                                        <div class="suggestion-item" onclick={Callback::from(move |_| {
                                            add_whatsapp.set(name.clone());
                                            show_whatsapp_suggestions.set(false);
                                        })}>
                                            {&room.display_name}
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
                                    let add_telegram = add_telegram.clone();
                                    let show_telegram_suggestions = show_telegram_suggestions.clone();
                                    html! {
                                        <div class="suggestion-item" onclick={Callback::from(move |_| {
                                            add_telegram.set(name.clone());
                                            show_telegram_suggestions.set(false);
                                        })}>
                                            {&room.display_name}
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
                                    let add_signal = add_signal.clone();
                                    let show_signal_suggestions = show_signal_suggestions.clone();
                                    html! {
                                        <div class="suggestion-item" onclick={Callback::from(move |_| {
                                            add_signal.set(name.clone());
                                            show_signal_suggestions.set(false);
                                        })}>
                                            {&room.display_name}
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
                                        placeholder="Search chat name" />
                                    {wa_suggestions}
                                </div>
                            </div>
                            <div class="avatar-modal-row">
                                <label>{"Telegram"}</label>
                                <div class="input-with-suggestions">
                                    <input type="text" value={(*add_telegram).clone()} oninput={on_telegram_input}
                                        placeholder="Search chat name" />
                                    {tg_suggestions}
                                </div>
                            </div>
                            <div class="avatar-modal-row">
                                <label>{"Signal"}</label>
                                <div class="input-with-suggestions">
                                    <input type="text" value={(*add_signal).clone()} oninput={on_signal_input}
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
                                    <option value="digest" selected={current_add_mode == "digest"}>{"Digest"}</option>
                                    <option value="ignore" selected={current_add_mode == "ignore"}>{"Ignore"}</option>
                                </select>
                            </div>
                            if show_type {
                                <div class="avatar-modal-row">
                                    <label>{"Notification type"}</label>
                                    <select onchange={on_type}>
                                        <option value="sms" selected={*add_type == "sms"}>{"SMS"}</option>
                                        <option value="call" selected={*add_type == "call"}>{"Call"}</option>
                                        <option value="call+sms" selected={*add_type == "call+sms"}>{"Call + SMS"}</option>
                                    </select>
                                </div>
                                <div class="avatar-modal-check">
                                    <input type="checkbox" id="av-add-call" checked={*add_notify_call} onchange={on_call} />
                                    <label for="av-add-call">{"Notify on incoming call"}</label>
                                </div>
                            }
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
                    {render_default_avatar}
                    {render_add_avatar}
                </div>
            </div>
            {render_modal()}
        </>
    }
}
