use yew::prelude::*;
use super::triage_indicator::AttentionItem;

const ITEMS_STATUS_STYLES: &str = r#"
.items-status {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
}
.items-grid {
    display: flex;
    gap: 0.75rem;
}
.items-col {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
}
@media (max-width: 500px) {
    .items-grid {
        flex-direction: column;
        gap: 0.35rem;
    }
}

/* --- Overdue item --- */
.item-overdue {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.45rem 0.6rem;
    border-radius: 8px;
    background: rgba(255, 107, 107, 0.07);
    border: 1px solid rgba(255, 107, 107, 0.18);
}
.item-overdue .item-desc { color: #ddd; }
.item-overdue .item-when { color: #ff6b6b; }
.item-overdue .item-clock .clock-face { border-color: rgba(255,107,107,0.4); }
.item-overdue .item-clock .clock-face::after { background: rgba(255,107,107,0.5); }
.item-overdue .item-clock .clock-hand-line { background: rgba(255,107,107,0.5); }

/* --- Scheduled item --- */
.item-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.3rem 0.6rem;
    border-radius: 6px;
}

/* --- Animated clock icon --- */
.item-clock {
    width: 14px;
    height: 14px;
    position: relative;
    flex-shrink: 0;
}
.clock-face {
    position: absolute;
    inset: 0;
    border: 1.2px solid rgba(255,255,255,0.2);
    border-radius: 50%;
}
.clock-face::after {
    content: '';
    position: absolute;
    top: 50%;
    left: 50%;
    width: 2px;
    height: 2px;
    background: rgba(255,255,255,0.25);
    border-radius: 50%;
    transform: translate(-50%, -50%);
}
.clock-hand-wrap {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    animation: clock-spin 8s linear infinite;
}
.clock-hand-line {
    position: absolute;
    top: 2.5px;
    left: 50%;
    width: 1.2px;
    height: 4px;
    background: rgba(255,255,255,0.3);
    border-radius: 1px;
    transform: translateX(-50%);
}
@keyframes clock-spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
}

/* --- Shared item parts --- */
.item-desc {
    flex: 1;
    min-width: 0;
    font-size: 0.82rem;
    color: #aaa;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}
.item-when {
    font-size: 0.7rem;
    color: #666;
    flex-shrink: 0;
    white-space: nowrap;
}
.item-badge {
    font-size: 0.6rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    padding: 0.1rem 0.35rem;
    border-radius: 3px;
    flex-shrink: 0;
    font-weight: 600;
}
.badge-call {
    color: #ff6b6b;
    background: rgba(255, 107, 107, 0.12);
}
.badge-sms {
    color: #e8a838;
    background: rgba(232, 168, 56, 0.1);
}
.item-x {
    background: none;
    border: none;
    color: #444;
    font-size: 0.75rem;
    cursor: pointer;
    padding: 0.15rem 0.3rem;
    flex-shrink: 0;
    opacity: 0;
    transition: opacity 0.15s, color 0.15s;
}
.item-overdue:hover .item-x { opacity: 1; }
.item-x:hover { color: #aaa; }

/* ===== Monitor section ===== */
.mon-section {
    margin-top: 0.25rem;
}
.mon-header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.4rem 0.6rem;
    cursor: pointer;
    user-select: none;
    border-radius: 8px;
    transition: background 0.15s;
}
.mon-header:hover {
    background: rgba(126, 178, 255, 0.06);
}
.mon-dots {
    display: flex;
    gap: 4px;
    align-items: center;
}
.mon-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    animation: dot-breathe 2.5s ease-in-out infinite;
}
.mon-dot:nth-child(2) { animation-delay: 0.4s; }
.mon-dot:nth-child(3) { animation-delay: 0.8s; }
.mon-dot:nth-child(4) { animation-delay: 1.2s; }
.mon-dot:nth-child(5) { animation-delay: 1.6s; }
@keyframes dot-breathe {
    0%, 100% { opacity: 0.35; transform: scale(1); }
    50% { opacity: 1; transform: scale(1.4); }
}
.mon-label {
    font-size: 0.75rem;
    color: #7EB2FF;
    flex: 1;
}
.mon-chevron {
    font-size: 0.55rem;
    color: #7EB2FF;
    transition: transform 0.2s;
    width: 0.7rem;
    text-align: center;
}
.mon-chevron.open {
    transform: rotate(90deg);
}

/* Expanded list */
.mon-list {
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    padding: 0.4rem 0.25rem 0.25rem 0.25rem;
}
.mon-card {
    display: flex;
    align-items: center;
    gap: 0.65rem;
    padding: 0.4rem 0.5rem;
    border-radius: 8px;
    transition: background 0.15s;
}
.mon-card:hover {
    background: rgba(255, 255, 255, 0.02);
}

/* Monitor animation scene */
.mon-scene {
    width: 36px;
    height: 30px;
    position: relative;
    flex-shrink: 0;
}
/* Incoming icon particles */
.mon-incoming {
    position: absolute;
    top: 50%;
    opacity: 0;
    z-index: 3;
    animation: icon-drift 3.2s ease-in-out infinite;
    display: flex;
    align-items: center;
    justify-content: center;
}
.mon-incoming.p2 {
    animation-delay: -1.6s;
    top: 35%;
}
@keyframes icon-drift {
    0% { left: -2px; opacity: 0; transform: translateY(-50%) scale(1); }
    10% { opacity: 0.7; }
    65% { opacity: 0.4; }
    100% { left: calc(50% - 4px); opacity: 0; transform: translateY(-50%) scale(0.3); }
}
/* Center icon with glow */
.mon-center {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    z-index: 2;
    display: flex;
    align-items: center;
    justify-content: center;
    animation: center-pulse 2.5s ease-in-out infinite;
}
@keyframes center-pulse {
    0%, 100% { opacity: 0.7; transform: translate(-50%, -50%) scale(1); }
    50% { opacity: 1; transform: translate(-50%, -50%) scale(1.1); }
}
.mon-glow {
    position: absolute;
    top: 50%;
    left: 50%;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    transform: translate(-50%, -50%);
    animation: glow-pulse 2.5s ease-in-out infinite;
    z-index: 1;
}
@keyframes glow-pulse {
    0%, 100% { opacity: 0.15; transform: translate(-50%, -50%) scale(0.7); }
    50% { opacity: 0.4; transform: translate(-50%, -50%) scale(1.15); }
}

/* Monitor info */
.mon-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 0.05rem;
}
.mon-sender {
    font-size: 0.82rem;
    font-weight: 500;
    color: #ccc;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}
.mon-detail {
    font-size: 0.7rem;
    color: #666;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

/* --- More button --- */
.items-more-btn {
    background: none;
    border: none;
    color: #555;
    font-size: 0.72rem;
    cursor: pointer;
    padding: 0.2rem 0.6rem;
    transition: color 0.15s;
    text-align: left;
}
.items-more-btn:hover {
    color: #999;
}

/* Monitor section label row */
.mon-label-row {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.15rem 0.6rem;
    margin-top: 0.15rem;
}
.mon-label-text {
    font-size: 0.7rem;
    color: #555;
    text-transform: uppercase;
    letter-spacing: 0.04em;
}

/* --- Status line --- */
.items-quiet {
    font-size: 0.72rem;
    color: #4a4a5a;
    text-align: center;
    padding: 0.2rem 0;
}
"#;

// -- SVG icon helpers --

fn svg_chat_bubble(size: u32, color: &str) -> Html {
    let h = (size as f32 * 0.8) as u32;
    let svg = format!(
        r#"<svg width="{w}" height="{h}" viewBox="0 0 16 13" fill="none" xmlns="http://www.w3.org/2000/svg"><path d="M2.5 1h11a1.5 1.5 0 011.5 1.5v6a1.5 1.5 0 01-1.5 1.5H6.5l-3 2.5V10H2.5A1.5 1.5 0 011 8.5v-6A1.5 1.5 0 012.5 1z" fill="{c}"/></svg>"#,
        w = size, h = h, c = color
    );
    Html::from_html_unchecked(yew::AttrValue::from(svg))
}

fn svg_envelope(size: u32, color: &str) -> Html {
    let h = (size as f32 * 0.7) as u32;
    let svg = format!(
        r#"<svg width="{w}" height="{h}" viewBox="0 0 16 11" fill="none" xmlns="http://www.w3.org/2000/svg"><rect x="0.7" y="0.7" width="14.6" height="9.6" rx="1.5" stroke="{c}" stroke-width="1.2"/><path d="M1.5 1.5L8 6.5l6.5-5" stroke="{c}" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round"/></svg>"#,
        w = size, h = h, c = color
    );
    Html::from_html_unchecked(yew::AttrValue::from(svg))
}

// -- Platform info --

struct PlatformVisual {
    name: &'static str,
    color: &'static str,
    glow: &'static str,
    is_chat: bool, // true = chat bubble icon, false = envelope icon
}

fn platform_visual(platform_tag: Option<&str>, desc: &str) -> PlatformVisual {
    if let Some(p) = platform_tag {
        match p {
            "whatsapp" => return PlatformVisual { name: "WhatsApp", color: "#25D366", glow: "rgba(37,211,102,0.3)", is_chat: true },
            "email" => return PlatformVisual { name: "Email", color: "#5B9AFF", glow: "rgba(91,154,255,0.25)", is_chat: false },
            "telegram" => return PlatformVisual { name: "Telegram", color: "#26A5E4", glow: "rgba(38,165,228,0.3)", is_chat: true },
            "signal" => return PlatformVisual { name: "Signal", color: "#3A76F0", glow: "rgba(58,118,240,0.3)", is_chat: true },
            "messenger" => return PlatformVisual { name: "Messenger", color: "#0084FF", glow: "rgba(0,132,255,0.3)", is_chat: true },
            "instagram" => return PlatformVisual { name: "Instagram", color: "#E4405F", glow: "rgba(228,64,95,0.25)", is_chat: true },
            _ => {}
        }
    }
    let lower = desc.to_lowercase();
    if lower.contains("whatsapp") {
        PlatformVisual { name: "WhatsApp", color: "#25D366", glow: "rgba(37,211,102,0.3)", is_chat: true }
    } else if lower.contains("email") {
        PlatformVisual { name: "Email", color: "#5B9AFF", glow: "rgba(91,154,255,0.25)", is_chat: false }
    } else if lower.contains("telegram") {
        PlatformVisual { name: "Telegram", color: "#26A5E4", glow: "rgba(38,165,228,0.3)", is_chat: true }
    } else if lower.contains("signal") {
        PlatformVisual { name: "Signal", color: "#3A76F0", glow: "rgba(58,118,240,0.3)", is_chat: true }
    } else if lower.contains("messenger") {
        PlatformVisual { name: "Messenger", color: "#0084FF", glow: "rgba(0,132,255,0.3)", is_chat: true }
    } else if lower.contains("instagram") {
        PlatformVisual { name: "Instagram", color: "#E4405F", glow: "rgba(228,64,95,0.25)", is_chat: true }
    } else {
        PlatformVisual { name: "Monitor", color: "#7EB2FF", glow: "rgba(126,178,255,0.25)", is_chat: true }
    }
}

// -- Description cleaning --

fn clean_description(desc: &str) -> String {
    let s = desc.trim();
    let prefixes = [
        "Remind the user to ",
        "Remind user to ",
        "remind the user to ",
        "remind user to ",
        "Check weather and notify the user if ",
    ];
    for prefix in &prefixes {
        if let Some(rest) = s.strip_prefix(prefix) {
            let mut chars = rest.chars();
            return match chars.next() {
                Some(c) => c.to_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            };
        }
    }
    s.to_string()
}

/// Try to extract sender name from description text when tag is missing.
fn extract_sender(desc: &str) -> Option<String> {
    let idx = desc.find("from ")?;
    let rest = &desc[idx + 5..];
    // Take until "and ", ".", ",", or end
    let end = rest
        .find(" and ")
        .or_else(|| rest.find('.'))
        .or_else(|| rest.find(','))
        .unwrap_or(rest.len());
    let candidate = rest[..end].trim();
    if !candidate.is_empty() && candidate.len() < 25 && !candidate.contains(' ') || candidate.split_whitespace().count() <= 2 {
        Some(candidate.to_string())
    } else {
        None
    }
}

/// Build a short topic line for monitor display.
fn monitor_topic(desc: &str, sender: Option<&str>) -> String {
    let mut s = desc.trim().to_string();

    // Strip leading "Watch for " / "Monitor: "
    for prefix in &["Watch for ", "watch for ", "Monitor: "] {
        if let Some(rest) = s.strip_prefix(prefix) {
            s = rest.to_string();
            break;
        }
    }
    // Strip platform prefixes
    for prefix in &[
        "WhatsApp messages ", "WhatsApp ", "Emails ", "Email ",
        "Telegram messages ", "Signal messages ", "Messenger messages ",
        "Instagram messages ", "Messages ", "messages ",
    ] {
        if let Some(rest) = s.strip_prefix(prefix) {
            s = rest.to_string();
            break;
        }
    }
    // Strip "from Sender" (shown separately)
    if let Some(sender) = sender {
        for pat in &[
            format!("from {}. ", sender),
            format!("from {} ", sender),
            format!("from {}", sender),
        ] {
            if let Some(idx) = s.find(pat.as_str()) {
                s = format!("{}{}", &s[..idx], &s[idx + pat.len()..]);
                break;
            }
        }
    }
    // Strip common LLM suffixes
    for suffix in &[
        "Alert when match arrives.",
        "Alert when match arrives",
        "and call the user if she says anything urgent.",
        "and call the user if she says anything urgent",
        "and call the user if they say anything urgent.",
        "and call the user if they say anything urgent",
        "and call the user",
        "and sms the user",
        "and notify the user",
    ] {
        if let Some(idx) = s.find(suffix) {
            s = s[..idx].to_string();
        }
    }
    let s = s.trim();
    let s = s.strip_prefix("about ").unwrap_or(s);
    let s = s.trim().trim_end_matches('.').trim();

    if s.is_empty() {
        return String::new();
    }
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

// -- Component --

#[derive(Properties, PartialEq)]
pub struct ItemsStatusProps {
    pub items: Vec<AttentionItem>,
    pub total_tracked_count: i32,
    pub on_dismiss: Callback<AttentionItem>,
}

#[function_component(ItemsStatusSection)]
pub fn items_status_section(props: &ItemsStatusProps) -> Html {
    let show_all_scheduled = use_state(|| false);
    let show_all_monitors = use_state(|| false);

    if props.items.is_empty() && props.total_tracked_count == 0 {
        return html! {};
    }

    let overdue_items: Vec<&AttentionItem> = props.items.iter()
        .filter(|i| !i.monitor && i.relative_display.as_deref() == Some("overdue"))
        .collect();

    let scheduled_items: Vec<&AttentionItem> = props.items.iter()
        .filter(|i| !i.monitor && i.relative_display.as_deref() != Some("overdue"))
        .collect();

    // Deduplicate monitors by sender, sorted by next_check_at (closest first)
    let deduped_monitors: Vec<&AttentionItem> = {
        let mut monitors: Vec<&AttentionItem> = props.items.iter()
            .filter(|i| i.monitor)
            .collect();
        // Sort by next_check_at ascending (closest first, None last)
        monitors.sort_by(|a, b| match (a.next_check_at, b.next_check_at) {
            (Some(a_ts), Some(b_ts)) => a_ts.cmp(&b_ts),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        });
        let mut seen: Vec<String> = Vec::new();
        let mut result = Vec::new();
        for m in monitors {
            let key = m.sender.clone()
                .or_else(|| extract_sender(&m.description))
                .unwrap_or_else(|| m.description.clone());
            if !seen.contains(&key) {
                seen.push(key);
                result.push(m);
            }
        }
        result
    };

    let has_anything = !overdue_items.is_empty()
        || !scheduled_items.is_empty()
        || !deduped_monitors.is_empty();

    // How many to show
    const VISIBLE_LIMIT: usize = 3;
    let scheduled_hidden = if scheduled_items.len() > VISIBLE_LIMIT && !*show_all_scheduled {
        scheduled_items.len() - VISIBLE_LIMIT
    } else {
        0
    };
    let visible_scheduled: Vec<&&AttentionItem> = if *show_all_scheduled {
        scheduled_items.iter().collect()
    } else {
        scheduled_items.iter().take(VISIBLE_LIMIT).collect()
    };

    let monitors_hidden = if deduped_monitors.len() > VISIBLE_LIMIT && !*show_all_monitors {
        deduped_monitors.len() - VISIBLE_LIMIT
    } else {
        0
    };
    let visible_monitors: Vec<&&AttentionItem> = if *show_all_monitors {
        deduped_monitors.iter().collect()
    } else {
        deduped_monitors.iter().take(VISIBLE_LIMIT).collect()
    };

    let on_show_all_scheduled = {
        let show_all_scheduled = show_all_scheduled.clone();
        Callback::from(move |_: MouseEvent| {
            show_all_scheduled.set(!*show_all_scheduled);
        })
    };
    let on_show_all_monitors = {
        let show_all_monitors = show_all_monitors.clone();
        Callback::from(move |_: MouseEvent| {
            show_all_monitors.set(!*show_all_monitors);
        })
    };

    html! {
        <>
        <style>{ITEMS_STATUS_STYLES}</style>
        <div class="items-status">
            // Overdue items - always show all
            { for overdue_items.iter().map(|item| {
                let desc = clean_description(&item.description);
                let dismiss_item = (*item).clone();
                let on_dismiss = props.on_dismiss.clone();
                html! {
                    <div class="item-overdue">
                        <div class="item-clock">
                            <div class="clock-face"></div>
                            <div class="clock-hand-wrap">
                                <div class="clock-hand-line"></div>
                            </div>
                        </div>
                        <span class="item-desc">{desc}</span>
                        { render_badge(item.notify.as_deref()) }
                        <span class="item-when">{"overdue"}</span>
                        <button class="item-x"
                            onclick={Callback::from(move |e: MouseEvent| {
                                e.stop_propagation();
                                on_dismiss.emit(dismiss_item.clone());
                            })}
                        >{"x"}</button>
                    </div>
                }
            })}

            // Scheduled items - first 3, then "+N more"
            { for visible_scheduled.iter().map(|item| {
                let desc = clean_description(&item.description);
                let when = match (&item.time_display, &item.relative_display) {
                    (Some(t), Some(r)) => format!("{} - {}", t, r),
                    (Some(t), None) => t.clone(),
                    (None, Some(r)) => r.clone(),
                    (None, None) => String::new(),
                };
                html! {
                    <div class="item-row">
                        <div class="item-clock">
                            <div class="clock-face"></div>
                            <div class="clock-hand-wrap">
                                <div class="clock-hand-line"></div>
                            </div>
                        </div>
                        <span class="item-desc">{desc}</span>
                        { render_badge(item.notify.as_deref()) }
                        { if !when.is_empty() {
                            html! { <span class="item-when">{when}</span> }
                        } else {
                            html! {}
                        }}
                    </div>
                }
            })}
            { if scheduled_hidden > 0 {
                html! {
                    <button class="items-more-btn" onclick={on_show_all_scheduled.clone()}>
                        {format!("+{} more", scheduled_hidden)}
                    </button>
                }
            } else if *show_all_scheduled && scheduled_items.len() > VISIBLE_LIMIT {
                html! {
                    <button class="items-more-btn" onclick={on_show_all_scheduled}>
                        {"show less"}
                    </button>
                }
            } else {
                html! {}
            }}

            // Monitors - first 3 shown directly, then "+N more"
            { if !deduped_monitors.is_empty() {
                html! {
                    <div class="mon-section">
                        <div class="mon-label-row">
                            <div class="mon-dots">
                                { for deduped_monitors.iter().map(|m| {
                                    let pv = platform_visual(m.platform.as_deref(), &m.description);
                                    let style = format!("background: {};", pv.color);
                                    html! { <span class="mon-dot" style={style}></span> }
                                })}
                            </div>
                            <span class="mon-label-text">{"watching"}</span>
                        </div>
                        <div class="mon-list">
                            { for visible_monitors.iter().map(|item| {
                                render_monitor_card(item)
                            })}
                        </div>
                        { if monitors_hidden > 0 {
                            html! {
                                <button class="items-more-btn" onclick={on_show_all_monitors.clone()}>
                                    {format!("+{} more", monitors_hidden)}
                                </button>
                            }
                        } else if *show_all_monitors && deduped_monitors.len() > VISIBLE_LIMIT {
                            html! {
                                <button class="items-more-btn" onclick={on_show_all_monitors}>
                                    {"show less"}
                                </button>
                            }
                        } else {
                            html! {}
                        }}
                    </div>
                }
            } else {
                html! {}
            }}

            // Status line
            { if !has_anything && props.total_tracked_count > 0 {
                html! {
                    <div class="items-quiet">
                        {format!("Tracking {} item{} - all on schedule",
                            props.total_tracked_count,
                            if props.total_tracked_count != 1 { "s" } else { "" }
                        )}
                    </div>
                }
            } else {
                html! {}
            }}
        </div>
        </>
    }
}

fn render_badge(notify: Option<&str>) -> Html {
    match notify {
        Some("call") => html! { <span class="item-badge badge-call">{"call"}</span> },
        Some("sms") => html! { <span class="item-badge badge-sms">{"sms"}</span> },
        _ => html! {},
    }
}

fn render_monitor_card(item: &AttentionItem) -> Html {
    let pv = platform_visual(item.platform.as_deref(), &item.description);

    // Build icon functions based on platform type
    let small_icon = if pv.is_chat {
        svg_chat_bubble(8, pv.color)
    } else {
        svg_envelope(9, pv.color)
    };
    let small_icon_2 = if pv.is_chat {
        svg_chat_bubble(8, pv.color)
    } else {
        svg_envelope(9, pv.color)
    };
    let center_icon = if pv.is_chat {
        svg_chat_bubble(16, pv.color)
    } else {
        svg_envelope(16, pv.color)
    };

    let glow_style = format!("background: {};", pv.glow);

    // Sender: from tag, or extract from description, or platform name
    let sender_display = item.sender.clone()
        .or_else(|| extract_sender(&item.description))
        .unwrap_or_else(|| pv.name.to_string());

    // Check if we have a real sender (not platform fallback)
    let has_sender = item.sender.is_some() || extract_sender(&item.description).is_some();

    let topic = monitor_topic(
        &item.description,
        if has_sender { Some(&sender_display) } else { None },
    );
    let detail = if topic.is_empty() {
        if has_sender { pv.name.to_string() } else { String::new() }
    } else if has_sender {
        format!("{} - {}", pv.name, topic)
    } else {
        topic
    };

    html! {
        <div class="mon-card">
            <div class="mon-scene">
                <div class="mon-incoming">{small_icon}</div>
                <div class="mon-incoming p2">{small_icon_2}</div>
                <span class="mon-glow" style={glow_style}></span>
                <div class="mon-center">{center_icon}</div>
            </div>
            <div class="mon-info">
                <span class="mon-sender">{sender_display}</span>
                { if !detail.is_empty() {
                    html! { <span class="mon-detail">{detail}</span> }
                } else {
                    html! {}
                }}
            </div>
            { render_badge(item.notify.as_deref()) }
        </div>
    }
}
