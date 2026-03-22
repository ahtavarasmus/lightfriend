use super::triage_indicator::AttentionItem;
use yew::prelude::*;

const ITEMS_STATUS_STYLES: &str = r#"
.items-status {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
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
    cursor: pointer;
    transition: background 0.15s;
}
.item-overdue:hover {
    background: rgba(255, 107, 107, 0.12);
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
    cursor: pointer;
    transition: background 0.15s;
}
.item-row:hover {
    background: rgba(255, 255, 255, 0.03);
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
    color: #ccc;
}
.item-when {
    font-size: 0.7rem;
    color: #666;
    flex-shrink: 0;
    white-space: nowrap;
}
.item-badge {
    font-size: 0.7rem;
    flex-shrink: 0;
}
.badge-call {
    color: #ff6b6b;
}
.badge-sms {
    color: #e8a838;
}
.badge-silent {
    font-size: 0.6rem;
    opacity: 0.4;
}
.item-type-tag {
    font-size: 0.55rem;
    color: #555;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 2px;
}
.item-type-tag i { font-size: 0.5rem; }
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
    cursor: pointer;
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

/* Platform icon (center of scene) */
.mon-center i {
    font-size: 0.85rem;
}
.mon-incoming i {
    font-size: 0.4rem;
}

/* Platform tag next to sender */
.mon-platform-tag {
    font-size: 0.55rem;
    text-transform: uppercase;
    letter-spacing: 0.03em;
    padding: 0.05rem 0.3rem;
    border-radius: 3px;
    flex-shrink: 0;
    font-weight: 600;
    white-space: nowrap;
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
}

/* --- Item info (two-line: title + subtitle, used for digests) --- */
.item-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 0.05rem;
}
.item-title {
    font-size: 0.82rem;
    font-weight: 500;
    color: #ccc;
}
.item-subtitle {
    font-size: 0.7rem;
    color: #666;
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

/* --- Example/ghost items --- */
.example-items-wrapper {
    position: relative;
    cursor: pointer;
}
.example-items-hint {
    position: absolute;
    top: -0.1rem;
    right: 0.5rem;
    font-size: 0.6rem;
    color: #555;
    letter-spacing: 0.02em;
    pointer-events: none;
    opacity: 0;
    transition: opacity 0.2s;
}
.example-items-wrapper:hover .example-items-hint {
    opacity: 1;
}
.example-item {
    opacity: 0.35;
    transition: opacity 0.2s;
    position: relative;
}
.example-item:hover {
    opacity: 0.6;
}
.example-item::before {
    content: '';
    position: absolute;
    inset: 0;
    border: 1px dashed rgba(255, 255, 255, 0.08);
    border-radius: 8px;
    pointer-events: none;
}

/* --- Items info header --- */
.items-header {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    margin-bottom: 0.15rem;
}
.items-header-label {
    font-size: 0.75rem;
    color: #666;
    text-transform: uppercase;
    letter-spacing: 0.05em;
}
.items-info-btn {
    background: transparent;
    border: none;
    color: #555;
    font-size: 0.7rem;
    cursor: pointer;
    padding: 0.1rem 0.25rem;
    transition: color 0.2s;
}
.items-info-btn:hover {
    color: #7EB2FF;
}
.items-info-overlay {
    position: fixed;
    top: 0; left: 0; right: 0; bottom: 0;
    background: rgba(0,0,0,0.8);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 9999;
}
.items-info-box {
    background: #1e1e2f;
    border: 1px solid rgba(255,255,255,0.1);
    border-radius: 12px;
    padding: 1.25rem;
    max-width: 420px;
    width: 90%;
    max-height: 80vh;
    overflow-y: auto;
    color: #ddd;
}
.items-info-box h3 {
    margin: 0 0 0.5rem 0;
    font-size: 1rem;
    color: #fff;
}
.items-info-box h4 {
    margin: 0.75rem 0 0.25rem 0;
    font-size: 0.85rem;
    color: #7EB2FF;
}
.items-info-box p {
    font-size: 0.78rem;
    color: #aaa;
    margin: 0.15rem 0;
    line-height: 1.45;
}
.items-info-box strong {
    color: #ccc;
}
.items-info-divider {
    height: 1px;
    background: rgba(255,255,255,0.08);
    margin: 0.75rem 0;
}
.items-info-close {
    display: block;
    margin: 1rem auto 0;
    background: transparent;
    border: 1px solid rgba(255,255,255,0.15);
    color: #999;
    padding: 0.35rem 1.25rem;
    border-radius: 6px;
    cursor: pointer;
    font-size: 0.8rem;
}
.items-info-close:hover {
    color: #ccc;
}
"#;

// -- Platform info --

struct PlatformVisual {
    name: &'static str,
    color: &'static str,
    glow: &'static str,
    icon: &'static str, // Font Awesome icon class
}

fn platform_visual(platform_tag: Option<&str>, desc: &str) -> PlatformVisual {
    if let Some(p) = platform_tag {
        match p {
            "whatsapp" => {
                return PlatformVisual {
                    name: "WhatsApp",
                    color: "#25D366",
                    glow: "rgba(37,211,102,0.3)",
                    icon: "fa-brands fa-whatsapp",
                }
            }
            "email" => {
                return PlatformVisual {
                    name: "Email",
                    color: "#5B9AFF",
                    glow: "rgba(91,154,255,0.25)",
                    icon: "fa-solid fa-envelope",
                }
            }
            "telegram" => {
                return PlatformVisual {
                    name: "Telegram",
                    color: "#26A5E4",
                    glow: "rgba(38,165,228,0.3)",
                    icon: "fa-brands fa-telegram",
                }
            }
            "signal" => {
                return PlatformVisual {
                    name: "Signal",
                    color: "#3A76F0",
                    glow: "rgba(58,118,240,0.3)",
                    icon: "fa-brands fa-signal-messenger",
                }
            }
            "internet" => {
                return PlatformVisual {
                    name: "Web",
                    color: "#e8a838",
                    glow: "rgba(232,168,56,0.25)",
                    icon: "fa-solid fa-globe",
                }
            }
            "items" => {
                return PlatformVisual {
                    name: "Web",
                    color: "#e8a838",
                    glow: "rgba(232,168,56,0.25)",
                    icon: "fa-solid fa-globe",
                }
            }
            "weather" => {
                return PlatformVisual {
                    name: "Weather",
                    color: "#38bdf8",
                    glow: "rgba(56,189,248,0.25)",
                    icon: "fa-solid fa-cloud-sun",
                }
            }
            _ => {}
        }
    }
    let lower = desc.to_lowercase();
    if lower.contains("whatsapp") {
        PlatformVisual {
            name: "WhatsApp",
            color: "#25D366",
            glow: "rgba(37,211,102,0.3)",
            icon: "fa-brands fa-whatsapp",
        }
    } else if lower.contains("email") {
        PlatformVisual {
            name: "Email",
            color: "#5B9AFF",
            glow: "rgba(91,154,255,0.25)",
            icon: "fa-solid fa-envelope",
        }
    } else if lower.contains("telegram") {
        PlatformVisual {
            name: "Telegram",
            color: "#26A5E4",
            glow: "rgba(38,165,228,0.3)",
            icon: "fa-brands fa-telegram",
        }
    } else if lower.contains("signal") {
        PlatformVisual {
            name: "Signal",
            color: "#3A76F0",
            glow: "rgba(58,118,240,0.3)",
            icon: "fa-brands fa-signal-messenger",
        }
    } else {
        PlatformVisual {
            name: "Monitor",
            color: "#7EB2FF",
            glow: "rgba(126,178,255,0.25)",
            icon: "fa-solid fa-eye",
        }
    }
}

// -- Description cleaning --

fn clean_description(desc: &str) -> String {
    let s = desc.trim();

    // Digest items: strip everything after the colon
    if s.starts_with("Daily digest:") || s.starts_with("daily digest:") {
        return "Digest".to_string();
    }

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
    if !candidate.is_empty() && candidate.len() < 25 && !candidate.contains(' ')
        || candidate.split_whitespace().count() <= 2
    {
        Some(candidate.to_string())
    } else {
        None
    }
}

/// Build a short topic line for tracking item display.
fn tracking_topic(desc: &str, sender: Option<&str>) -> String {
    let mut s = desc.trim().to_string();

    // Strip leading "Watch for " / "Monitor: "
    for prefix in &["Watch for ", "watch for ", "Monitor: "] {
        if let Some(rest) = s.strip_prefix(prefix) {
            s = rest.to_string();
            break;
        }
    }
    // Strip platform prefixes (case variants)
    for prefix in &[
        "WhatsApp messages ",
        "WhatsApp ",
        "whatsapp messages ",
        "whatsapp ",
        "Emails ",
        "Email ",
        "emails ",
        "email ",
        "Telegram messages ",
        "telegram messages ",
        "Telegram ",
        "telegram ",
        "Signal messages ",
        "signal messages ",
        "Signal ",
        "signal ",
        "Messages ",
        "messages ",
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

    // Filter out generic/contact-profile phrases - these aren't real topics
    let lower = s.to_lowercase();
    let generic = [
        "anything urgent",
        "anything important",
        "anything",
        "something urgent",
        "something important",
        "something",
        "urgent messages",
        "important messages",
    ];
    if generic.iter().any(|g| lower == *g) {
        return String::new();
    }

    let mut chars = s.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
        None => String::new(),
    }
}

// -- Summary tag parsing (frontend mirror of backend parse_summary_tags) --

/// Extract value for a `[key:value]` tag from the summary's first line.
fn extract_tag<'a>(summary: &'a str, key: &str) -> Option<&'a str> {
    let first_line = summary.lines().next().unwrap_or("");
    let needle = format!("[{}:", key);
    let start = first_line.find(&needle)? + needle.len();
    let end = start + first_line[start..].find(']')?;
    let val = first_line[start..end].trim();
    if val.is_empty() {
        None
    } else {
        Some(val)
    }
}

/// Check if an item is a digest: has both [fetch:] and [notify:] tags,
/// or is a legacy item whose summary starts with "Daily digest".
fn is_digest_item(summary: &str) -> bool {
    if extract_tag(summary, "fetch").is_some() && extract_tag(summary, "notify").is_some() {
        return true;
    }
    // Legacy digests created before the tag system
    summary.starts_with("Daily digest")
}

/// Get the `[repeat:daily HH:MM]` hour from summary tags.
fn parse_repeat_hour(summary: &str) -> Option<u32> {
    let val = extract_tag(summary, "repeat")?; // e.g. "daily 19:00"
    let time_part = val.split_whitespace().last()?; // "19:00"
    let colon = time_part.find(':')?;
    time_part[..colon].parse().ok()
}

/// Get readable sources: from `[fetch:]` tag, or from "Sources:" in legacy descriptions.
pub fn digest_sources(summary: &str, description: &str) -> Option<String> {
    // Tagged items: parse [fetch:email,chat,items]
    if let Some(val) = extract_tag(summary, "fetch") {
        let parts: Vec<&str> = val
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if !parts.is_empty() {
            let labels: Vec<&str> = parts
                .iter()
                .map(|s| match *s {
                    "email" => "email",
                    "chat" => "messages",
                    "weather" => "weather",
                    "items" => "tracked items",
                    other => other,
                })
                .collect();
            return Some(labels.join(", "));
        }
    }
    // Legacy items: look for "Sources: x,y,z" in the summary text
    let text = if summary.len() > description.len() {
        summary
    } else {
        description
    };
    if let Some(idx) = text.find("Sources:").or_else(|| text.find("sources:")) {
        let rest = text[idx + 8..].trim();
        // Take until period or "Repeats"
        let end = rest
            .find(". ")
            .or_else(|| rest.find(".\n"))
            .or_else(|| rest.find(". Repeats"))
            .unwrap_or(rest.len());
        let sources = rest[..end].trim().trim_end_matches('.');
        if !sources.is_empty() {
            return Some(sources.to_string());
        }
    }
    None
}

// -- Digest detection --

fn detect_occupied_digest_hours(items: &[AttentionItem]) -> Vec<u32> {
    let mut occupied = Vec::new();
    for item in items {
        if !is_digest_item(&item.summary) {
            continue;
        }
        if let Some(h) = parse_repeat_hour(&item.summary) {
            let slot_hour = match h {
                5..=11 => 8,
                12..=16 => 13,
                _ => 19,
            };
            if !occupied.contains(&slot_hour) {
                occupied.push(slot_hour);
            }
        }
    }
    occupied
}

// -- Contextual suggestions based on existing items --

fn build_suggestions(items: &[AttentionItem]) -> Vec<AttentionItem> {
    let mut suggestions = Vec::new();
    let occupied = detect_occupied_digest_hours(items);

    // Suggest one digest: first free time slot
    if !occupied.contains(&8) {
        suggestions.push(AttentionItem {
            id: -1,
            item_type: "recurring".to_string(),
            summary: "[fetch:email,chat,items][notify:sms][repeat:daily 08:00]".to_string(),
            description: "Daily digest: morning briefing".to_string(),
            priority: 0,
            due_at: None,
            source: None,
            source_id: None,
            notify: Some("sms".to_string()),
            sender: None,
            platform: None,
            time_display: Some("8:00 AM".to_string()),
            relative_display: Some("daily".to_string()),
        });
    } else if !occupied.contains(&13) {
        suggestions.push(AttentionItem {
            id: -4,
            item_type: "recurring".to_string(),
            summary: "[fetch:email,chat,items][notify:sms][repeat:daily 13:00]".to_string(),
            description: "Daily digest: afternoon update".to_string(),
            priority: 0,
            due_at: None,
            source: None,
            source_id: None,
            notify: Some("sms".to_string()),
            sender: None,
            platform: None,
            time_display: Some("1:00 PM".to_string()),
            relative_display: Some("daily".to_string()),
        });
    } else if !occupied.contains(&19) {
        suggestions.push(AttentionItem {
            id: -5,
            item_type: "recurring".to_string(),
            summary: "[fetch:email,chat,items][notify:sms][repeat:daily 19:00]".to_string(),
            description: "Daily digest: evening recap".to_string(),
            priority: 0,
            due_at: None,
            source: None,
            source_id: None,
            notify: Some("sms".to_string()),
            sender: None,
            platform: None,
            time_display: Some("7:00 PM".to_string()),
            relative_display: Some("daily".to_string()),
        });
    }

    // Suggest tracking if no tracking items exist
    let has_tracking = items.iter().any(|i| i.item_type == "tracking");
    if !has_tracking {
        suggestions.push(AttentionItem {
            id: -2,
            item_type: "tracking".to_string(),
            summary: "Watch for delivery updates".to_string(),
            description: "Watch for delivery updates from Amazon".to_string(),
            priority: 0,
            due_at: None,
            source: None,
            source_id: None,
            notify: Some("sms".to_string()),
            sender: Some("Amazon".to_string()),
            platform: Some("email".to_string()),
            time_display: None,
            relative_display: None,
        });
    }

    // Always suggest a reminder (one-shot reminders are always useful)
    suggestions.push(AttentionItem {
        id: -3,
        item_type: "oneshot".to_string(),
        summary: "Reminder".to_string(),
        description: "Take the cake out of the oven".to_string(),
        priority: 0,
        due_at: None,
        source: None,
        source_id: None,
        notify: Some("sms".to_string()),
        sender: None,
        platform: None,
        time_display: Some("3:30 PM".to_string()),
        relative_display: Some("in 2h".to_string()),
    });

    suggestions
}

fn example_prefill_prompt(id: i32) -> Option<String> {
    match id {
        -1 => Some(
            "Set up a daily digest at 8am covering my emails, messages, and tracked items"
                .to_string(),
        ),
        -4 => Some(
            "Set up a daily digest at 1pm covering my emails, messages, and tracked items"
                .to_string(),
        ),
        -5 => Some(
            "Set up a daily digest at 7pm covering my emails, messages, and tracked items"
                .to_string(),
        ),
        -2 => Some(
            "Watch for delivery updates from Amazon in my email and text me when it ships"
                .to_string(),
        ),
        -3 => Some("Remind me to take the cake out of the oven in 2 hours".to_string()),
        _ => None,
    }
}

// -- Component --

#[derive(Properties, PartialEq)]
pub struct ItemsStatusProps {
    pub items: Vec<AttentionItem>,
    pub total_tracked_count: i32,
    pub on_dismiss: Callback<AttentionItem>,
    #[prop_or_default]
    pub on_digest_prefill: Option<Callback<String>>,
    #[prop_or_default]
    pub on_item_click: Option<Callback<AttentionItem>>,
}

#[function_component(ItemsStatusSection)]
pub fn items_status_section(props: &ItemsStatusProps) -> Html {
    let show_all = use_state(|| false);
    let show_info = use_state(|| false);

    // Build contextual suggestions based on what the user already has
    let suggestions = build_suggestions(&props.items);

    // Deduplicate tracking items by sender
    let mut seen_tracking: Vec<String> = Vec::new();

    // Build unified list: all items sorted by due_at (soonest first)
    let mut all_items: Vec<&AttentionItem> = props
        .items
        .iter()
        .filter(|i| {
            if i.item_type == "tracking" {
                let key = i
                    .sender
                    .clone()
                    .or_else(|| extract_sender(&i.description))
                    .unwrap_or_else(|| i.description.clone());
                if seen_tracking.contains(&key) {
                    return false;
                }
                seen_tracking.push(key);
            }
            true
        })
        .collect();

    // Sort: overdue first, then by due_at ascending
    all_items.sort_by(|a, b| {
        let a_overdue = a.relative_display.as_deref() == Some("overdue");
        let b_overdue = b.relative_display.as_deref() == Some("overdue");
        match (a_overdue, b_overdue) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => match (a.due_at, b.due_at) {
                (Some(a_ts), Some(b_ts)) => a_ts.cmp(&b_ts),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            },
        }
    });

    const VISIBLE_LIMIT: usize = 5;
    let total = all_items.len();
    let hidden = if total > VISIBLE_LIMIT && !*show_all {
        total - VISIBLE_LIMIT
    } else {
        0
    };
    let visible: Vec<&&AttentionItem> = if *show_all {
        all_items.iter().collect()
    } else {
        all_items.iter().take(VISIBLE_LIMIT).collect()
    };

    let on_toggle = {
        let show_all = show_all.clone();
        Callback::from(move |_: MouseEvent| {
            show_all.set(!*show_all);
        })
    };

    // Build suggestion HTML
    let suggestions_html = if !suggestions.is_empty() {
        html! {
            <div class="example-items-wrapper">
                <span class="example-items-hint">{"click to try"}</span>
                { for suggestions.iter().map(|item| {
                    let prefill_cb = props.on_digest_prefill.clone();
                    let item_id = item.id;
                    let onclick = Callback::from(move |_: MouseEvent| {
                        if let (Some(ref cb), Some(prompt)) = (&prefill_cb, example_prefill_prompt(item_id)) {
                            cb.emit(prompt);
                        }
                    });
                    html! {
                        <div class="example-item">
                            { if item.item_type == "tracking" {
                                render_tracking_card(item, onclick)
                            } else {
                                render_scheduled_item(item, onclick)
                            }}
                        </div>
                    }
                })}
            </div>
        }
    } else {
        html! {}
    };

    let on_info_open = {
        let show_info = show_info.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            show_info.set(true);
        })
    };
    let on_info_close = {
        let show_info = show_info.clone();
        Callback::from(move |_: MouseEvent| {
            show_info.set(false);
        })
    };
    let stop_prop = Callback::from(|e: MouseEvent| {
        e.stop_propagation();
    });

    let info_modal = if *show_info {
        html! {
            <div class="items-info-overlay" onclick={on_info_close.clone()}>
                <div class="items-info-box" onclick={stop_prop}>
                    <h3>{"Items"}</h3>
                    <p>{"Items are things Lightfriend keeps track of for you. Create them by texting a request like \"remind me\" or \"watch for\"."}</p>

                    <div class="items-info-divider"></div>

                    <h4>{"Reminder (oneshot)"}</h4>
                    <p>{"Fires once at the scheduled time, sends you a text or call, then deletes itself."}</p>
                    <p><strong>{"Example: "}</strong>{"\"Remind me to call the dentist at 3pm\""}</p>

                    <h4>{"Scheduled report (recurring)"}</h4>
                    <p>{"Runs on a repeating schedule - daily, weekdays, or weekly. Pulls fresh data from your connected sources (email, messages, tracked items) and sends a summary."}</p>
                    <p><strong>{"Example: "}</strong>{"\"Every morning at 8am summarize my emails and messages\""}</p>

                    <h4>{"Tracking"}</h4>
                    <p>{"Watches your incoming messages or external data for a specific condition. Only notifies you when there's a match - stays silent otherwise. Expires after 30 days."}</p>
                    <p><strong>{"Example: "}</strong>{"\"Let me know when mom emails about the trip\""}</p>

                    <div class="items-info-divider"></div>

                    <h4>{"Delivery"}</h4>
                    <p><strong>{"SMS: "}</strong>{"default - concise text message."}</p>
                    <p><strong>{"Call: "}</strong>{"phone call for urgent or wake-up items."}</p>
                    <p><strong>{"Silent: "}</strong>{"no notification, background only."}</p>

                    <button class="items-info-close" onclick={on_info_close}>{"Close"}</button>
                </div>
            </div>
        }
    } else {
        html! {}
    };

    html! {
        <>
        <style>{ITEMS_STATUS_STYLES}</style>
        {info_modal}
        <div class="items-status">
            <div class="items-header">
                <span class="items-header-label">{"Items"}</span>
                <button class="items-info-btn" onclick={on_info_open}>
                    <i class="fa-solid fa-circle-info"></i>
                </button>
            </div>
            { for visible.iter().map(|item| {
                let on_click = props.on_item_click.clone();
                let click_item: AttentionItem = (***item).clone();
                let onclick = Callback::from(move |_: MouseEvent| {
                    if let Some(ref cb) = on_click {
                        cb.emit(click_item.clone());
                    }
                });
                if item.item_type == "tracking" {
                    render_tracking_card(item, onclick)
                } else if item.relative_display.as_deref() == Some("overdue") {
                    let desc = clean_description(&item.description);
                    let dismiss_item: AttentionItem = (***item).clone();
                    let on_dismiss = props.on_dismiss.clone();
                    html! {
                        <div class="item-overdue" {onclick}>
                            <div class="item-clock">
                                <div class="clock-face"></div>
                                <div class="clock-hand-wrap">
                                    <div class="clock-hand-line"></div>
                                </div>
                            </div>
                            <span class="item-desc">{super::emoji_utils::emojify_description(&desc)}</span>

                            <span class="item-when">{"overdue"}</span>
                            { render_badge(item.notify.as_deref()) }
                            <button class="item-x"
                                onclick={Callback::from(move |e: MouseEvent| {
                                    e.stop_propagation();
                                    on_dismiss.emit(dismiss_item.clone());
                                })}
                            >{"x"}</button>
                        </div>
                    }
                } else {
                    render_scheduled_item(item, onclick)
                }
            })}
            { if hidden > 0 {
                html! {
                    <button class="items-more-btn" onclick={on_toggle.clone()}>
                        {format!("+{} more", hidden)}
                    </button>
                }
            } else if *show_all && total > VISIBLE_LIMIT {
                html! {
                    <button class="items-more-btn" onclick={on_toggle}>
                        {"show less"}
                    </button>
                }
            } else {
                html! {}
            }}

            // Status line when nothing visible
            { if all_items.is_empty() && props.total_tracked_count > 0 {
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

            // Contextual suggestions
            {suggestions_html}
        </div>
        </>
    }
}

fn render_badge(notify: Option<&str>) -> Html {
    match notify {
        Some("call") => {
            html! { <span class="item-badge badge-call" title="Call (+SMS)"><i class="fa-solid fa-phone"></i></span> }
        }
        Some("sms") => {
            html! { <span class="item-badge badge-sms" title="SMS"><i class="fa-solid fa-comment-sms"></i></span> }
        }
        _ => html! { <span class="item-badge badge-silent" title="Silent">{"👀"}</span> },
    }
}

fn render_type_tag(item_type: &str) -> Html {
    match item_type {
        "recurring" => html! {
            <span class="item-type-tag" title="Recurring">
                <i class="fa-solid fa-arrows-rotate"></i>
            </span>
        },
        "oneshot" => html! {
            <span class="item-type-tag" title="One-time">
                <i class="fa-solid fa-1"></i>
            </span>
        },
        _ => html! {},
    }
}

fn render_scheduled_item(item: &AttentionItem, onclick: Callback<MouseEvent>) -> Html {
    let is_digest = is_digest_item(&item.summary);
    let is_recurring = item.item_type == "recurring" || item.item_type == "tracking" || is_digest;
    let when = match (&item.time_display, &item.relative_display) {
        (Some(t), Some(r)) => {
            if is_recurring {
                format!("next {} - {}", t, r)
            } else {
                format!("{} - {}", t, r)
            }
        }
        (Some(t), None) => {
            if is_recurring {
                format!("next {}", t)
            } else {
                t.clone()
            }
        }
        (None, Some(r)) => r.clone(),
        (None, None) => String::new(),
    };

    if is_digest {
        let title = clean_description(&item.description);
        let sources = digest_sources(&item.summary, &item.description);
        return html! {
            <div class="item-row" {onclick}>
                <div class="item-clock">
                    <div class="clock-face"></div>
                    <div class="clock-hand-wrap">
                        <div class="clock-hand-line"></div>
                    </div>
                </div>
                <div class="item-info">
                    <span class="item-title">{title}</span>
                    { if let Some(src) = sources {
                        html! { <span class="item-subtitle">{src}</span> }
                    } else {
                        html! {}
                    }}
                </div>
                { if !when.is_empty() {
                    html! { <span class="item-when">{when}</span> }
                } else {
                    html! {}
                }}
                // Digests always notify - default to sms for legacy items
                { render_badge(Some(item.notify.as_deref().unwrap_or("sms"))) }
            </div>
        };
    }

    let desc = clean_description(&item.description);
    html! {
        <div class="item-row" {onclick}>
            <div class="item-clock">
                <div class="clock-face"></div>
                <div class="clock-hand-wrap">
                    <div class="clock-hand-line"></div>
                </div>
            </div>
            <span class="item-desc">{super::emoji_utils::emojify_description(&desc)}</span>
            { if !when.is_empty() {
                html! { <span class="item-when">{when}</span> }
            } else {
                html! {}
            }}
            { render_badge(item.notify.as_deref()) }
        </div>
    }
}

fn render_tracking_card(item: &AttentionItem, onclick: Callback<MouseEvent>) -> Html {
    let pv = platform_visual(item.platform.as_deref(), &item.description);
    let icon_style = format!("color: {};", pv.color);
    let glow_style = format!("background: {};", pv.glow);
    let tag_style = format!("color: {}; background: {}33;", pv.color, pv.color);

    // Sender: from tag, or extract from description, or platform name
    let sender_display = item
        .sender
        .clone()
        .or_else(|| extract_sender(&item.description))
        .unwrap_or_else(|| pv.name.to_string());
    let has_sender = item.sender.is_some() || extract_sender(&item.description).is_some();
    let topic = tracking_topic(
        &item.description,
        if has_sender {
            Some(&sender_display)
        } else {
            None
        },
    );
    let detail = if !topic.is_empty() {
        topic
    } else {
        String::new()
    };

    let when = match (&item.time_display, &item.relative_display) {
        (Some(t), Some(r)) if r != "overdue" => format!("{} - {}", t, r),
        (_, Some(r)) if r == "overdue" => "checking...".to_string(),
        _ => String::new(),
    };

    html! {
        <div class="mon-card" {onclick}>
            <div class="mon-scene">
                <div class="mon-incoming"><i class={pv.icon} style={icon_style.clone()}></i></div>
                <div class="mon-incoming p2"><i class={pv.icon} style={icon_style.clone()}></i></div>
                <span class="mon-glow" style={glow_style}></span>
                <div class="mon-center"><i class={pv.icon} style={icon_style}></i></div>
            </div>
            <div class="mon-info">
                <span class="mon-sender">{sender_display}</span>
                { if !detail.is_empty() {
                    html! { <span class="mon-detail">{detail}</span> }
                } else {
                    html! {}
                }}
            </div>
            // platform icon already shows visually, no text tag needed
            { if !when.is_empty() {
                let label = if when == "checking..." { when.clone() } else { format!("next check in {}", when) };
                html! { <span class="item-when">{label}</span> }
            } else {
                html! {}
            }}
            { render_badge(item.notify.as_deref()) }
        </div>
    }
}
