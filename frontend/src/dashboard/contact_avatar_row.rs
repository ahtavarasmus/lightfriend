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
/* Modal overlay */
.avatar-modal-overlay {
    position: fixed;
    top: 0; left: 0; right: 0; bottom: 0;
    background: rgba(0,0,0,0.8);
    display: flex;
    align-items: flex-start;
    justify-content: center;
    z-index: 9999;
    overflow-y: auto;
    padding: 2rem 0;
}
.avatar-modal-box {
    background: #1e1e2f;
    border: 1px solid rgba(255,255,255,0.1);
    border-radius: 12px;
    padding: 1.25rem;
    max-width: 400px;
    width: 90%;
    color: #ddd;
    max-height: calc(100vh - 4rem);
    overflow-y: auto;
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

/* Arena layout - SMS left, figures center, Call right */
.people-arena {
    display: flex;
    flex-direction: row;
    align-items: center;
    padding: 0.5rem 0;
    width: 100%;
    overflow: visible;
}

.people-target {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.2rem;
    flex-shrink: 0;
    width: 50px;
}

.target-circle {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.65rem;
    animation: target-pulse 3s ease-in-out infinite;
}

.target-label {
    font-size: 0.55rem;
    color: #888;
}

@keyframes target-pulse {
    0%, 100% { transform: scale(1); opacity: 0.8; }
    50% { transform: scale(1.06); opacity: 1; }
}

.people-figures-outer {
    flex: 1;
    min-width: 0;
    position: relative;
}
.people-figures-outer::after {
    content: '';
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    height: 30px;
    background: linear-gradient(to bottom, transparent, #16161e);
    pointer-events: none;
    z-index: 2;
}
.people-figures-wrap {
    max-height: 260px;
    overflow-y: auto;
    overflow-x: visible;
    -webkit-overflow-scrolling: touch;
    scrollbar-width: thin;
    scrollbar-color: rgba(255,255,255,0.15) transparent;
}
.people-figures-wrap::-webkit-scrollbar { width: 3px; }
.people-figures-wrap::-webkit-scrollbar-thumb { background: rgba(255,255,255,0.15); border-radius: 3px; }

.people-figures {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0;
    padding: 0.15rem 0;
}

.figure-item {
    display: flex;
    flex-direction: column;
    align-items: center;
    cursor: pointer;
    position: relative;
    padding: 0.1rem 0 1rem 0;
    width: 60px;
}
.figure-item:hover .figure-label {
    color: #bbb;
}

.figure-label {
    font-size: 0.6rem;
    color: #666;
    max-width: 58px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-align: center;
}

/* Flying icons - use CSS custom props for per-figure targeting */
.flying-icon {
    position: absolute;
    top: 12px;
    left: 50%;
    margin-left: 10px;
    font-size: 0.65rem;
    pointer-events: none;
    z-index: 1;
    --fly-x: -120px;
    --fly-y: 0px;
}

.fly-sms { animation: fly-to-target 4s linear infinite; }
.fly-call { animation: fly-to-target 4s linear infinite; }

.fly-delay-0 { animation-delay: 0s; }
.fly-delay-1 { animation-delay: 1s; }
.fly-delay-2 { animation-delay: 2s; }
.fly-delay-3 { animation-delay: 3s; }

@keyframes fly-to-target {
    0%   { transform: translate(0, 0); opacity: 0; }
    5%   { opacity: 1; }
    80%  { opacity: 0.6; }
    100% { transform: translate(var(--fly-x), var(--fly-y)); opacity: 0; }
}

.fly-mode-badge {
    font-size: 0.4rem;
    font-weight: bold;
    position: absolute;
    top: -3px;
    right: -6px;
}

/* Platform section in modal */
.platform-section {
    margin-top: 0.75rem;
    padding-top: 0.5rem;
    border-top: 1px solid rgba(255,255,255,0.08);
}
.platform-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 0.5rem;
    padding: 0.4rem;
    border-radius: 6px;
    background: rgba(255,255,255,0.03);
    flex-wrap: wrap;
}
.platform-row i {
    font-size: 0.85rem;
    width: 20px;
    text-align: center;
    flex-shrink: 0;
}
.platform-row .input-with-suggestions {
    flex: 1;
    min-width: 80px;
    position: relative;
}
.platform-row .input-with-suggestions input[type="text"] {
    width: 100%;
    padding: 0.3rem 0.4rem;
    background: #12121f;
    border: 1px solid rgba(255,255,255,0.12);
    border-radius: 4px;
    color: #ddd;
    font-size: 0.75rem;
}
.platform-row .input-with-suggestions input[type="text"]:focus {
    outline: none;
    border-color: rgba(255,255,255,0.3);
}
.platform-row .input-with-suggestions input.warn-border {
    border-color: #e55 !important;
}
.platform-row input.platform-email-input {
    flex: 1;
    min-width: 80px;
    padding: 0.3rem 0.4rem;
    background: #12121f;
    border: 1px solid rgba(255,255,255,0.12);
    border-radius: 4px;
    color: #ddd;
    font-size: 0.75rem;
}
.platform-row input.platform-email-input:focus {
    outline: none;
    border-color: rgba(255,255,255,0.3);
}
.platform-row select {
    padding: 0.25rem 0.35rem;
    background: #12121f;
    border: 1px solid rgba(255,255,255,0.12);
    border-radius: 4px;
    color: #ccc;
    font-size: 0.7rem;
    max-width: 100px;
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

struct PlatformInfo {
    key: &'static str,
    icon: &'static str,
    color: &'static str,
    label: &'static str,
}

const PLATFORMS: [PlatformInfo; 4] = [
    PlatformInfo { key: "whatsapp", icon: "fa-brands fa-whatsapp", color: "#25D366", label: "WhatsApp" },
    PlatformInfo { key: "telegram", icon: "fa-brands fa-telegram", color: "#0088CC", label: "Telegram" },
    PlatformInfo { key: "signal", icon: "fa-brands fa-signal-messenger", color: "#3A76F1", label: "Signal" },
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

fn render_platform_row(
    icon: &str,
    color: &str,
    is_bridge: bool,
    search_html: Html,
    exc_mode: String,
    exc_type: String,
    on_mode: Callback<Event>,
    on_type: Callback<Event>,
) -> Html {
    html! {
        <div class="platform-row">
            <i class={icon.to_string()} style={format!("color:{};", color)}></i>
            {search_html}
            <select onchange={on_mode}>
                <option value="" selected={exc_mode.is_empty()}>{"Default"}</option>
                <option value="all" selected={exc_mode == "all"}>{"All"}</option>
                if is_bridge {
                    <option value="mention" selected={exc_mode == "mention"}>{"@mention"}</option>
                }
                <option value="critical" selected={exc_mode == "critical"}>{"Critical"}</option>
                <option value="digest" selected={exc_mode == "digest"}>{"Digest"}</option>
                <option value="ignore" selected={exc_mode == "ignore"}>{"Ignore"}</option>
            </select>
            if exc_mode != "ignore" {
                <select onchange={on_type}>
                    <option value="" selected={exc_type.is_empty()}>{"Default"}</option>
                    <option value="sms" selected={exc_type == "sms"}>{"SMS"}</option>
                    <option value="call" selected={exc_type == "call"}>{"Call"}</option>
                </select>
            }
        </div>
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

/// Renders an SVG stick figure with one arm raised in a throwing pose.
/// `special`: None for normal, Some("unknown") for ? head, Some("add") for dashed/plus, Some("contacts") for plain gray
fn render_stick_figure_svg(color: &str, special: Option<&str>) -> Html {
    let stroke = match special {
        Some("add") => "stroke=\"rgba(255,255,255,0.25)\" stroke-dasharray=\"3,3\"".to_string(),
        _ => format!("stroke=\"{}\"", color),
    };
    let head_content = match special {
        Some("unknown") => format!(
            r#"<text x="15" y="11" text-anchor="middle" font-size="8" fill="{}" font-weight="bold">?</text>"#,
            color
        ),
        Some("add") => r#"<text x="15" y="12" text-anchor="middle" font-size="12" fill="rgba(255,255,255,0.25)" font-weight="300">+</text>"#.to_string(),
        _ => String::new(),
    };
    let svg = format!(
        r#"<svg viewBox="0 0 30 48" width="30" height="48" xmlns="http://www.w3.org/2000/svg"><circle cx="15" cy="8" r="5" fill="none" {} stroke-width="2"/>{}<line x1="15" y1="13" x2="15" y2="30" {} stroke-width="2"/><line x1="15" y1="19" x2="24" y2="12" {} stroke-width="2"/><line x1="15" y1="19" x2="6" y2="25" {} stroke-width="2"/><line x1="15" y1="30" x2="8" y2="42" {} stroke-width="2"/><line x1="15" y1="30" x2="22" y2="42" {} stroke-width="2"/></svg>"#,
        stroke, head_content, stroke, stroke, stroke, stroke, stroke
    );
    Html::from_html_unchecked(AttrValue::from(svg))
}

/// Renders flying platform icons for a contact profile.
/// Each icon gets --fly-x set here; --fly-y is updated dynamically by JS scroll handler.
fn render_flying_icons(profile: &ContactProfile) -> Html {
    let platforms = connected_platforms(profile);
    let mut icons = Vec::new();
    let mut delay_idx = 0usize;

    for pi in &platforms {
        let mode = effective_mode_for_platform(profile, pi.key);
        if mode == "ignore" {
            continue;
        }

        let noti_type = if let Some(exc) = find_exception(profile, pi.key) {
            exc.notification_type.as_str()
        } else {
            profile.notification_type.as_str()
        };

        // Determine which targets this icon flies toward
        let targets: Vec<&str> = match noti_type {
            "call" => vec!["fly-call"],
            _ => vec!["fly-sms"],
        };

        for fly_class in &targets {
            let delay_class = format!("fly-delay-{}", delay_idx % 4);

            let badge_html = match mode {
                "critical" => html! { <span class="fly-mode-badge" style="color:#f59e0b;">{"!"}</span> },
                "mention" => html! { <span class="fly-mode-badge" style="color:#3b82f6;">{"@"}</span> },
                _ => html! {},
            };

            icons.push(html! {
                <span class={format!("flying-icon {} {}", fly_class, delay_class)}>
                    <i class={pi.icon} style={format!("color:{};", pi.color)}></i>
                    {badge_html}
                </span>
            });

            delay_idx += 1;
        }
    }

    html! { <>{ for icons }</> }
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
pub struct ContactAvatarRowProps {
    /// Increment to open the PeopleInfo modal from the parent.
    #[prop_or_default]
    pub open_info_seq: u32,
}

#[function_component(ContactAvatarRow)]
pub fn contact_avatar_row(props: &ContactAvatarRowProps) -> Html {
    let profiles = use_state(|| Vec::<ContactProfile>::new());
    let default_mode = use_state(|| "critical".to_string());
    let default_noti_type = use_state(|| "sms".to_string());
    let default_notify_on_call = use_state(|| true);
    let loading = use_state(|| true);
    let modal = use_state(|| None::<ModalType>);
    let error_msg = use_state(|| None::<String>);
    let saving = use_state(|| false);

    // Open info modal when parent increments the sequence counter
    {
        let modal = modal.clone();
        let seq = props.open_info_seq;
        use_effect_with_deps(move |s: &u32| {
            if *s > 0 {
                modal.set(Some(ModalType::PeopleInfo));
            }
            || ()
        }, seq);
    }

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

    // Platform exception form state (for PlatformException modal - kept for compatibility)
    let exc_mode = use_state(|| String::new());
    let exc_type = use_state(|| "sms".to_string());
    let exc_notify_call = use_state(|| true);
    let exc_override_mode = use_state(|| false);

    // Per-platform inline exception state (for unified ContactSettings modal)
    // Empty string "" means "same as default" (no override)
    let exc_wa_mode = use_state(|| String::new());
    let exc_wa_type = use_state(|| String::new());
    let exc_tg_mode = use_state(|| String::new());
    let exc_tg_type = use_state(|| String::new());
    let exc_sg_mode = use_state(|| String::new());
    let exc_sg_type = use_state(|| String::new());
    let exc_em_mode = use_state(|| String::new());
    let exc_em_type = use_state(|| String::new());

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
    let add_notes = use_state(|| String::new());
    let add_exc_wa_mode = use_state(|| String::new());
    let add_exc_wa_type = use_state(|| String::new());
    let add_exc_tg_mode = use_state(|| String::new());
    let add_exc_tg_type = use_state(|| String::new());
    let add_exc_sg_mode = use_state(|| String::new());
    let add_exc_sg_type = use_state(|| String::new());
    let add_exc_em_mode = use_state(|| String::new());
    let add_exc_em_type = use_state(|| String::new());

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

    // Dynamic --fly-y updater: adjusts icon throw direction based on scroll position
    {
        use_effect_with_deps(
            move |_| {
                let update_fn = wasm_bindgen::closure::Closure::wrap(Box::new(move || {
                    let doc = web_sys::window().unwrap().document().unwrap();
                    // Find the actual SMS and Call target circle positions
                    let targets = doc.query_selector_all(".target-circle").unwrap();
                    if targets.length() < 2 { return; }
                    let sms_el: web_sys::Element = targets.item(0).unwrap().unchecked_into();
                    let call_el: web_sys::Element = targets.item(1).unwrap().unchecked_into();
                    let sms_rect = sms_el.get_bounding_client_rect();
                    let call_rect = call_el.get_bounding_client_rect();
                    let sms_center_y = sms_rect.top() + sms_rect.height() / 2.0;
                    let call_center_y = call_rect.top() + call_rect.height() / 2.0;

                    let sms_center_x = sms_rect.left() + sms_rect.width() / 2.0;
                    let call_center_x = call_rect.left() + call_rect.width() / 2.0;

                    let items = doc.query_selector_all(".figure-item").unwrap();
                    for i in 0..items.length() {
                        let item = items.item(i).unwrap();
                        let item_el: web_sys::Element = item.unchecked_into();
                        let item_rect = item_el.get_bounding_client_rect();
                        let item_center_x = item_rect.left() + item_rect.width() / 2.0;
                        let item_center_y = item_rect.top() + item_rect.height() / 2.0;

                        let icons = item_el.query_selector_all(".flying-icon").unwrap();
                        for j in 0..icons.length() {
                            let icon = icons.item(j).unwrap();
                            let icon_html: &web_sys::HtmlElement = icon.unchecked_ref();
                            let classes = icon_html.class_name();
                            let (target_x, target_y) = if classes.contains("fly-call") {
                                (call_center_x, call_center_y)
                            } else {
                                (sms_center_x, sms_center_y)
                            };
                            let fly_x = (target_x - item_center_x) as i32;
                            // +15 to compensate for icon starting above figure center
                            let fly_y = (target_y - item_center_y) as i32 + 15;
                            let _ = icon_html.style().set_property("--fly-x", &format!("{}px", fly_x));
                            let _ = icon_html.style().set_property("--fly-y", &format!("{}px", fly_y));
                        }
                    }
                }) as Box<dyn FnMut()>);

                // Run once on mount
                update_fn.as_ref().unchecked_ref::<js_sys::Function>().call0(&wasm_bindgen::JsValue::NULL).ok();

                // Attach scroll listener
                let doc = web_sys::window().unwrap().document().unwrap();
                if let Ok(Some(wrap)) = doc.query_selector(".people-figures-wrap") {
                    let _ = wrap.add_event_listener_with_callback(
                        "scroll",
                        update_fn.as_ref().unchecked_ref(),
                    );
                }

                // Also run periodically to catch initial render
                let interval_fn = update_fn.as_ref().unchecked_ref::<js_sys::Function>().clone();
                let window = web_sys::window().unwrap();
                let interval_id = window.set_interval_with_callback_and_timeout_and_arguments_0(
                    &interval_fn, 500
                ).unwrap_or(0);

                let cleanup = update_fn;
                move || {
                    if let Some(window) = web_sys::window() {
                        window.clear_interval_with_handle(interval_id);
                        if let Some(doc) = window.document() {
                            if let Ok(Some(wrap)) = doc.query_selector(".people-figures-wrap") {
                                let _ = wrap.remove_event_listener_with_callback(
                                    "scroll",
                                    cleanup.as_ref().unchecked_ref(),
                                );
                            }
                        }
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
        let exc_wa_mode = exc_wa_mode.clone();
        let exc_wa_type = exc_wa_type.clone();
        let exc_tg_mode = exc_tg_mode.clone();
        let exc_tg_type = exc_tg_type.clone();
        let exc_sg_mode = exc_sg_mode.clone();
        let exc_sg_type = exc_sg_type.clone();
        let exc_em_mode = exc_em_mode.clone();
        let exc_em_type = exc_em_type.clone();
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
                // Initialize per-platform exception overrides
                for plat in &["whatsapp", "telegram", "signal", "email"] {
                    let (mode_val, type_val) = if let Some(exc) = find_exception(p, plat) {
                        (exc.notification_mode.clone(), exc.notification_type.clone())
                    } else {
                        (String::new(), String::new())
                    };
                    match *plat {
                        "whatsapp" => { exc_wa_mode.set(mode_val); exc_wa_type.set(type_val); }
                        "telegram" => { exc_tg_mode.set(mode_val); exc_tg_type.set(type_val); }
                        "signal" => { exc_sg_mode.set(mode_val); exc_sg_type.set(type_val); }
                        "email" => { exc_em_mode.set(mode_val); exc_em_type.set(type_val); }
                        _ => {}
                    }
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
    let _on_bubble_click = {
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
        let add_notes = add_notes.clone();
        let add_exc_wa_mode = add_exc_wa_mode.clone();
        let add_exc_wa_type = add_exc_wa_type.clone();
        let add_exc_tg_mode = add_exc_tg_mode.clone();
        let add_exc_tg_type = add_exc_tg_type.clone();
        let add_exc_sg_mode = add_exc_sg_mode.clone();
        let add_exc_sg_type = add_exc_sg_type.clone();
        let add_exc_em_mode = add_exc_em_mode.clone();
        let add_exc_em_type = add_exc_em_type.clone();
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
            add_notes.set(String::new());
            add_whatsapp_room_id.set(None);
            add_telegram_room_id.set(None);
            add_signal_room_id.set(None);
            add_whatsapp_selected.set(false);
            add_telegram_selected.set(false);
            add_signal_selected.set(false);
            add_exc_wa_mode.set(String::new());
            add_exc_wa_type.set(String::new());
            add_exc_tg_mode.set(String::new());
            add_exc_tg_type.set(String::new());
            add_exc_sg_mode.set(String::new());
            add_exc_sg_type.set(String::new());
            add_exc_em_mode.set(String::new());
            add_exc_em_type.set(String::new());
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
        let exc_wa_mode = exc_wa_mode.clone();
        let exc_wa_type = exc_wa_type.clone();
        let exc_tg_mode = exc_tg_mode.clone();
        let exc_tg_type = exc_tg_type.clone();
        let exc_sg_mode = exc_sg_mode.clone();
        let exc_sg_type = exc_sg_type.clone();
        let exc_em_mode = exc_em_mode.clone();
        let exc_em_type = exc_em_type.clone();
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

            // Build exceptions from per-platform inline overrides
            let default_mode_val = (*form_mode).clone();
            let default_type_val = (*form_type).clone();
            let default_call_val = *form_notify_call;
            let mut exceptions: Vec<ExceptionRequest> = Vec::new();
            for (plat, exc_m, exc_t) in [
                ("whatsapp", (*exc_wa_mode).clone(), (*exc_wa_type).clone()),
                ("telegram", (*exc_tg_mode).clone(), (*exc_tg_type).clone()),
                ("signal", (*exc_sg_mode).clone(), (*exc_sg_type).clone()),
                ("email", (*exc_em_mode).clone(), (*exc_em_type).clone()),
            ] {
                // Non-empty mode or type means this platform has an override
                if !exc_m.is_empty() || !exc_t.is_empty() {
                    let existing_call = profile.exceptions.iter()
                        .find(|e| e.platform == plat)
                        .map(|e| e.notify_on_call)
                        .unwrap_or(default_call_val);
                    exceptions.push(ExceptionRequest {
                        platform: plat.to_string(),
                        notification_mode: if exc_m.is_empty() { default_mode_val.clone() } else { exc_m },
                        notification_type: if exc_t.is_empty() { default_type_val.clone() } else { exc_t },
                        notify_on_call: existing_call,
                    });
                }
            }

            let em = (*form_email).clone();
            let notes_val = (*form_notes).trim().to_string();
            let request = CreateProfileRequest {
                nickname,
                whatsapp_chat: if wa.is_empty() { None } else { Some(wa) },
                telegram_chat: if tg.is_empty() { None } else { Some(tg) },
                signal_chat: if sg.is_empty() { None } else { Some(sg) },
                email_addresses: if em.is_empty() { None } else { Some(em) },
                notification_mode: default_mode_val,
                notification_type: default_type_val,
                notify_on_call: default_call_val,
                exceptions: Some(exceptions),
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
        let add_notes = add_notes.clone();
        let add_whatsapp_room_id = add_whatsapp_room_id.clone();
        let add_telegram_room_id = add_telegram_room_id.clone();
        let add_signal_room_id = add_signal_room_id.clone();
        let add_whatsapp_selected = add_whatsapp_selected.clone();
        let add_telegram_selected = add_telegram_selected.clone();
        let add_signal_selected = add_signal_selected.clone();
        let add_exc_wa_mode = add_exc_wa_mode.clone();
        let add_exc_wa_type = add_exc_wa_type.clone();
        let add_exc_tg_mode = add_exc_tg_mode.clone();
        let add_exc_tg_type = add_exc_tg_type.clone();
        let add_exc_sg_mode = add_exc_sg_mode.clone();
        let add_exc_sg_type = add_exc_sg_type.clone();
        let add_exc_em_mode = add_exc_em_mode.clone();
        let add_exc_em_type = add_exc_em_type.clone();
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

            let default_mode_val = (*add_mode).clone();
            let default_type_val = (*add_type).clone();
            let default_call_val = *add_notify_call;
            let mut exceptions: Vec<ExceptionRequest> = Vec::new();
            for (plat, exc_m, exc_t) in [
                ("whatsapp", (*add_exc_wa_mode).clone(), (*add_exc_wa_type).clone()),
                ("telegram", (*add_exc_tg_mode).clone(), (*add_exc_tg_type).clone()),
                ("signal", (*add_exc_sg_mode).clone(), (*add_exc_sg_type).clone()),
                ("email", (*add_exc_em_mode).clone(), (*add_exc_em_type).clone()),
            ] {
                if !exc_m.is_empty() || !exc_t.is_empty() {
                    exceptions.push(ExceptionRequest {
                        platform: plat.to_string(),
                        notification_mode: if exc_m.is_empty() { default_mode_val.clone() } else { exc_m },
                        notification_type: if exc_t.is_empty() { default_type_val.clone() } else { exc_t },
                        notify_on_call: default_call_val,
                    });
                }
            }

            let notes_val = (*add_notes).trim().to_string();

            let request = CreateProfileRequest {
                nickname,
                whatsapp_chat: whatsapp,
                telegram_chat: telegram,
                signal_chat: signal,
                email_addresses: email,
                notification_mode: default_mode_val,
                notification_type: default_type_val,
                notify_on_call: default_call_val,
                exceptions: Some(exceptions),
                whatsapp_room_id: (*add_whatsapp_room_id).clone(),
                telegram_room_id: (*add_telegram_room_id).clone(),
                signal_room_id: (*add_signal_room_id).clone(),
                notes: if notes_val.is_empty() { None } else { Some(notes_val) },
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

    let render_figure = |profile: &ContactProfile| -> Html {
        let id = profile.id;
        let nick = profile.nickname.clone();
        let color = color_for_name(&nick);

        let on_click = {
            let cb = on_avatar_click.clone();
            Callback::from(move |e: MouseEvent| {
                e.stop_propagation();
                cb.emit(id);
            })
        };

        let flying = render_flying_icons(profile);
        let figure_svg = render_stick_figure_svg(color, None);

        html! {
            <div class="figure-item" onclick={on_click}>
                {figure_svg}
                {flying}
                <span class="figure-label">{nick}</span>
            </div>
        }
    };

    let render_phone_contact_figure = {
        let on_click = on_phone_contact_click.clone();
        html! {
            <div class="figure-item" onclick={on_click}>
                {render_stick_figure_svg("#555", None)}
                <span class="figure-label">{"Contacts"}</span>
            </div>
        }
    };

    let render_unknown_figure = {
        let on_click = on_default_click.clone();
        html! {
            <div class="figure-item" onclick={on_click}>
                {render_stick_figure_svg("#555", Some("unknown"))}
                <span class="figure-label">{"Unknown"}</span>
            </div>
        }
    };

    let render_add_figure = {
        let on_click = on_add_click.clone();
        html! {
            <div class="figure-item" onclick={on_click}>
                {render_stick_figure_svg("rgba(255,255,255,0.25)", Some("add"))}
                <span class="figure-label">{"Add"}</span>
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

                // Build each platform row via helper function to reduce locals
                let wa_row_html = render_platform_row(
                    "fa-brands fa-whatsapp", "#25D366", true,
                    html! {
                        <div class="input-with-suggestions">
                            <input type="text" value={(*form_whatsapp).clone()} oninput={on_form_whatsapp_input}
                                class={if !form_whatsapp.is_empty() && !*form_whatsapp_selected { "warn-border" } else { "" }}
                                placeholder="Search WhatsApp" />
                            {settings_wa_suggestions}
                        </div>
                    },
                    (*exc_wa_mode).clone(), (*exc_wa_type).clone(),
                    { let s = exc_wa_mode.clone(); Callback::from(move |e: Event| { let t: HtmlSelectElement = e.target_unchecked_into(); s.set(t.value()); }) },
                    { let s = exc_wa_type.clone(); Callback::from(move |e: Event| { let t: HtmlSelectElement = e.target_unchecked_into(); s.set(t.value()); }) },
                );
                let tg_row_html = render_platform_row(
                    "fa-brands fa-telegram", "#0088CC", true,
                    html! {
                        <div class="input-with-suggestions">
                            <input type="text" value={(*form_telegram).clone()} oninput={on_form_telegram_input}
                                class={if !form_telegram.is_empty() && !*form_telegram_selected { "warn-border" } else { "" }}
                                placeholder="Search Telegram" />
                            {settings_tg_suggestions}
                        </div>
                    },
                    (*exc_tg_mode).clone(), (*exc_tg_type).clone(),
                    { let s = exc_tg_mode.clone(); Callback::from(move |e: Event| { let t: HtmlSelectElement = e.target_unchecked_into(); s.set(t.value()); }) },
                    { let s = exc_tg_type.clone(); Callback::from(move |e: Event| { let t: HtmlSelectElement = e.target_unchecked_into(); s.set(t.value()); }) },
                );
                let sg_row_html = render_platform_row(
                    "fa-brands fa-signal-messenger", "#3A76F1", true,
                    html! {
                        <div class="input-with-suggestions">
                            <input type="text" value={(*form_signal).clone()} oninput={on_form_signal_input}
                                class={if !form_signal.is_empty() && !*form_signal_selected { "warn-border" } else { "" }}
                                placeholder="Search Signal" />
                            {settings_sg_suggestions}
                        </div>
                    },
                    (*exc_sg_mode).clone(), (*exc_sg_type).clone(),
                    { let s = exc_sg_mode.clone(); Callback::from(move |e: Event| { let t: HtmlSelectElement = e.target_unchecked_into(); s.set(t.value()); }) },
                    { let s = exc_sg_type.clone(); Callback::from(move |e: Event| { let t: HtmlSelectElement = e.target_unchecked_into(); s.set(t.value()); }) },
                );
                let em_row_html = render_platform_row(
                    "fa-solid fa-envelope", "#7EB2FF", false,
                    html! {
                        <input type="text" class="platform-email-input"
                            value={(*form_email).clone()} oninput={on_form_email_input}
                            placeholder="email@example.com" />
                    },
                    (*exc_em_mode).clone(), (*exc_em_type).clone(),
                    { let s = exc_em_mode.clone(); Callback::from(move |e: Event| { let t: HtmlSelectElement = e.target_unchecked_into(); s.set(t.value()); }) },
                    { let s = exc_em_type.clone(); Callback::from(move |e: Event| { let t: HtmlSelectElement = e.target_unchecked_into(); s.set(t.value()); }) },
                );

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
                            <div class="avatar-modal-row">
                                <label>{"Notes"}</label>
                                <textarea class="avatar-modal-notes" rows="2"
                                    value={(*form_notes).clone()}
                                    oninput={on_notes_input}
                                    placeholder="e.g. My mom. Reply in Finnish." />
                                <div class="avatar-modal-notes-hint">{"Helps AI understand context"}</div>
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
                            // Per-platform rows: chat search + override dropdowns
                            <div class="platform-section">
                                {wa_row_html}
                                {tg_row_html}
                                {sg_row_html}
                                {em_row_html}
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
                                    <option value="digest" selected={current_exc_mode == "digest"}>{"Digest"}</option>
                                    <option value="ignore" selected={current_exc_mode == "ignore"}>{"Ignore"}</option>
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
                                    <option value="digest" selected={current_pc_mode == "digest"}>{"Digest"}</option>
                                    <option value="ignore" selected={current_pc_mode == "ignore"}>{"Ignore"}</option>
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
                                    <option value="digest" selected={current_def_mode == "digest"}>{"Digest"}</option>
                                    <option value="ignore" selected={current_def_mode == "ignore"}>{"Ignore"}</option>
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
                                <p>{"Click a figure to edit settings. Click \"+\" to add a new contact profile. Click \"Contacts\" or \"Unknown\" to change settings for those groups."}</p>
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
                                <strong>{"Tip: "}</strong>{"Use the Network Overrides section below to customize notification settings per platform."}
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

                let on_notes_input = {
                    let add_notes = add_notes.clone();
                    Callback::from(move |e: InputEvent| {
                        let target: HtmlInputElement = e.target_unchecked_into();
                        add_notes.set(target.value());
                    })
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

                let add_wa_row = render_platform_row(
                    "fa-brands fa-whatsapp", "#25D366", true,
                    html! {
                        <div class="input-with-suggestions">
                            <input type="text" value={(*add_whatsapp).clone()} oninput={on_whatsapp_input}
                                class={if !add_whatsapp.is_empty() && !*add_whatsapp_selected { "warn-border" } else { "" }}
                                placeholder="Search WhatsApp" />
                            {wa_suggestions}
                        </div>
                    },
                    (*add_exc_wa_mode).clone(), (*add_exc_wa_type).clone(),
                    { let s = add_exc_wa_mode.clone(); Callback::from(move |e: Event| { let t: HtmlSelectElement = e.target_unchecked_into(); s.set(t.value()); }) },
                    { let s = add_exc_wa_type.clone(); Callback::from(move |e: Event| { let t: HtmlSelectElement = e.target_unchecked_into(); s.set(t.value()); }) },
                );
                let add_tg_row = render_platform_row(
                    "fa-brands fa-telegram", "#0088CC", true,
                    html! {
                        <div class="input-with-suggestions">
                            <input type="text" value={(*add_telegram).clone()} oninput={on_telegram_input}
                                class={if !add_telegram.is_empty() && !*add_telegram_selected { "warn-border" } else { "" }}
                                placeholder="Search Telegram" />
                            {tg_suggestions}
                        </div>
                    },
                    (*add_exc_tg_mode).clone(), (*add_exc_tg_type).clone(),
                    { let s = add_exc_tg_mode.clone(); Callback::from(move |e: Event| { let t: HtmlSelectElement = e.target_unchecked_into(); s.set(t.value()); }) },
                    { let s = add_exc_tg_type.clone(); Callback::from(move |e: Event| { let t: HtmlSelectElement = e.target_unchecked_into(); s.set(t.value()); }) },
                );
                let add_sg_row = render_platform_row(
                    "fa-brands fa-signal-messenger", "#3A76F1", true,
                    html! {
                        <div class="input-with-suggestions">
                            <input type="text" value={(*add_signal).clone()} oninput={on_signal_input}
                                class={if !add_signal.is_empty() && !*add_signal_selected { "warn-border" } else { "" }}
                                placeholder="Search Signal" />
                            {sg_suggestions}
                        </div>
                    },
                    (*add_exc_sg_mode).clone(), (*add_exc_sg_type).clone(),
                    { let s = add_exc_sg_mode.clone(); Callback::from(move |e: Event| { let t: HtmlSelectElement = e.target_unchecked_into(); s.set(t.value()); }) },
                    { let s = add_exc_sg_type.clone(); Callback::from(move |e: Event| { let t: HtmlSelectElement = e.target_unchecked_into(); s.set(t.value()); }) },
                );
                let add_em_row = render_platform_row(
                    "fa-solid fa-envelope", "#7EB2FF", false,
                    html! {
                        <input type="text" class="platform-email-input"
                            value={(*add_email).clone()} oninput={on_email}
                            placeholder="email@example.com" />
                    },
                    (*add_exc_em_mode).clone(), (*add_exc_em_type).clone(),
                    { let s = add_exc_em_mode.clone(); Callback::from(move |e: Event| { let t: HtmlSelectElement = e.target_unchecked_into(); s.set(t.value()); }) },
                    { let s = add_exc_em_type.clone(); Callback::from(move |e: Event| { let t: HtmlSelectElement = e.target_unchecked_into(); s.set(t.value()); }) },
                );

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
                                <label>{"Notes"}</label>
                                <textarea class="avatar-modal-notes" rows="2"
                                    value={(*add_notes).clone()}
                                    oninput={on_notes_input}
                                    placeholder="e.g. My mom. Reply in Finnish." />
                                <div class="avatar-modal-notes-hint">{"Helps AI understand context"}</div>
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
                            <div class="platform-section">
                                {add_wa_row}
                                {add_tg_row}
                                {add_sg_row}
                                {add_em_row}
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
                <div class="people-arena">
                    <span style="color:#666;font-size:0.8rem;">{"Loading contacts..."}</span>
                </div>
            </>
        };
    }

    let profile_figures = profiles.iter().map(|p| render_figure(p)).collect::<Html>();

    html! {
        <>
            <style>{AVATAR_ROW_STYLES}</style>
            <div class="people-arena">
                // SMS target - left side
                <div class="people-target">
                    <div class="target-circle" style="background: rgba(74,222,128,0.12); border: 1.5px solid rgba(74,222,128,0.35);">
                        <i class="fa-solid fa-comment-sms" style="color: #4ade80;"></i>
                    </div>
                    <span class="target-label">{"SMS"}</span>
                </div>
                // Figures stacked vertically in center
                <div class="people-figures-outer">
                    <div class="people-figures-wrap">
                        <div class="people-figures">
                            {profile_figures}
                            {render_phone_contact_figure}
                            {render_unknown_figure}
                            {render_add_figure}
                        </div>
                    </div>
                </div>
                // Call target - right side
                <div class="people-target">
                    <div class="target-circle" style="background: rgba(251,191,36,0.12); border: 1.5px solid rgba(251,191,36,0.35);">
                        <i class="fa-solid fa-phone" style="color: #fbbf24;"></i>
                    </div>
                    <span class="target-label">{"Call"}</span>
                </div>
            </div>
            {render_modal()}
        </>
    }
}
