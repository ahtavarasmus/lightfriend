use crate::utils::api::Api;
use std::collections::HashSet;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;
use serde_json;

pub const TRIAGE_STYLES: &str = r#"
.quick-reply-btn {
    display: inline-flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem 1rem;
    background: rgba(76, 175, 80, 0.08);
    border: 1px solid rgba(76, 175, 80, 0.2);
    border-radius: 20px;
    color: #81c784;
    font-size: 0.85rem;
    cursor: pointer;
    transition: all 0.2s;
    user-select: none;
}
.quick-reply-btn:hover {
    background: rgba(76, 175, 80, 0.15);
    border-color: rgba(76, 175, 80, 0.35);
}
.quick-reply-btn i {
    font-size: 0.7rem;
    opacity: 0.7;
}

/* Card flow overlay */
.qr-overlay {
    position: fixed;
    top: 0; left: 0; right: 0; bottom: 0;
    background: rgba(0, 0, 0, 0.75);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 10000;
}

.qr-card {
    background: #1e1e2f;
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 16px;
    padding: 1.5rem;
    max-width: 420px;
    width: 92%;
    color: #ddd;
    position: relative;
    animation: qr-slide-in 0.2s ease-out;
}
@keyframes qr-slide-in {
    from { opacity: 0; transform: translateY(20px); }
    to { opacity: 1; transform: translateY(0); }
}

.qr-close {
    position: absolute;
    top: 0.75rem;
    right: 0.75rem;
    background: none;
    border: none;
    color: #666;
    font-size: 1.1rem;
    cursor: pointer;
    padding: 0.25rem 0.5rem;
    transition: color 0.15s;
}
.qr-close:hover {
    color: #e57373;
}

.qr-sender-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 1rem;
}
.qr-service-icon {
    font-size: 1rem;
    opacity: 0.7;
}
.qr-service-icon.whatsapp { color: #25D366; }
.qr-service-icon.telegram { color: #0088CC; }
.qr-service-icon.signal { color: #3A76F1; }
.qr-sender-name {
    font-size: 1rem;
    font-weight: 600;
    color: #fff;
}

/* Conversation thread */
.qr-thread {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    margin-bottom: 1rem;
    max-height: 250px;
    overflow-y: auto;
    padding-right: 0.25rem;
}
.qr-thread-msg {
    font-size: 0.8rem;
    color: #999;
    line-height: 1.4;
}
.qr-thread-msg .qr-msg-sender {
    color: #aaa;
    font-weight: 500;
}
.qr-msg-time {
    font-size: 0.65rem;
    color: #555;
    margin-left: 0.35rem;
}
.qr-thread-msg.qr-msg-highlight {
    color: #ccc;
    background: rgba(255, 255, 255, 0.04);
    padding: 0.3rem 0.5rem;
    border-radius: 6px;
    border-left: 2px solid rgba(245, 158, 11, 0.4);
}
.qr-received-time {
    font-size: 0.7rem;
    color: #666;
    margin-bottom: 0.75rem;
}

/* Draft reply section */
.qr-draft-label {
    font-size: 0.7rem;
    color: #666;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    margin-bottom: 0.35rem;
}
.qr-edit-area {
    width: 100%;
    background: rgba(255, 255, 255, 0.06);
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 8px;
    color: #ddd;
    font-size: 0.9rem;
    padding: 0.5rem 0.6rem;
    resize: vertical;
    font-family: inherit;
    margin-bottom: 1rem;
    outline: none;
    min-height: 60px;
}
.qr-edit-area:focus {
    border-color: rgba(126, 178, 255, 0.4);
}

/* Action buttons */
.qr-actions {
    display: flex;
    gap: 0.5rem;
    align-items: center;
}
.qr-send-btn {
    flex: 2;
    padding: 0.65rem 1rem;
    background: rgba(76, 175, 80, 0.2);
    border: 1px solid rgba(76, 175, 80, 0.4);
    border-radius: 10px;
    color: #81c784;
    font-size: 0.95rem;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.15s;
}
.qr-send-btn:hover {
    background: rgba(76, 175, 80, 0.3);
    color: #a5d6a7;
}
.qr-send-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
}
.qr-skip-btn {
    flex: 1;
    padding: 0.65rem 0.75rem;
    background: transparent;
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 10px;
    color: #888;
    font-size: 0.85rem;
    cursor: pointer;
    transition: all 0.15s;
}
.qr-skip-btn:hover {
    color: #bbb;
    border-color: rgba(255, 255, 255, 0.2);
}

/* All caught up state */
.qr-done {
    text-align: center;
    padding: 2rem 1rem;
}
.qr-done-check {
    font-size: 2rem;
    color: #81c784;
    margin-bottom: 0.5rem;
}
.qr-done-text {
    color: #aaa;
    font-size: 0.95rem;
}

/* Sending state */
.qr-sending {
    color: #81c784;
    font-size: 0.85rem;
    text-align: center;
    padding: 0.5rem 0;
}
"#;

#[derive(Clone, PartialEq, Debug)]
pub struct ConversationMessage {
    pub sender: String,
    pub text: String,
    pub ts: i64,
}

#[derive(Clone, PartialEq)]
pub struct AttentionItem {
    pub id: i32,
    pub item_type: String,
    pub summary: String,
    pub timestamp: i32,
    pub source: Option<String>,
    pub suggested_action: Option<String>,
    pub reasoning: Option<String>,
    pub original_message: Option<String>,
    pub source_id: Option<String>,
    pub conversation_snippet: Vec<ConversationMessage>,
    pub service: Option<String>,
    pub sender_name: Option<String>,
    pub context_json: Option<serde_json::Value>,
}

/// Format a unix timestamp into a relative or absolute time string.
fn format_msg_time(ts: i64) -> String {
    if ts == 0 {
        return String::new();
    }
    let now = js_sys::Date::now() as i64 / 1000;
    let diff = now - ts;
    if diff < 60 {
        "just now".to_string()
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else if diff < 172800 {
        "yesterday".to_string()
    } else {
        // Show date via JS Date
        let d = js_sys::Date::new_0();
        d.set_time((ts * 1000) as f64);
        let month = d.get_month() + 1;
        let day = d.get_date();
        let hours = d.get_hours();
        let mins = d.get_minutes();
        format!("{}/{} {:02}:{:02}", day, month, hours, mins)
    }
}

// ---------------------------------------------------------------------------
// QuickReplyButton - subtle entry point on dashboard
// ---------------------------------------------------------------------------

#[derive(Properties, PartialEq, Clone)]
pub struct QuickReplyButtonProps {
    pub message_reply_count: usize,
    pub on_open: Callback<()>,
}

#[function_component(QuickReplyButton)]
pub fn quick_reply_button(props: &QuickReplyButtonProps) -> Html {
    if props.message_reply_count == 0 {
        return html! {};
    }

    let on_click = {
        let on_open = props.on_open.clone();
        Callback::from(move |_: MouseEvent| {
            on_open.emit(());
        })
    };

    html! {
        <>
            <style>{TRIAGE_STYLES}</style>
            <button class="quick-reply-btn" onclick={on_click}>
                {"Replies drafted"}
                <i class="fa-solid fa-chevron-right"></i>
            </button>
        </>
    }
}

// ---------------------------------------------------------------------------
// QuickReplyFlow - the card modal
// ---------------------------------------------------------------------------

#[derive(Properties, PartialEq, Clone)]
pub struct QuickReplyFlowProps {
    pub items: Vec<AttentionItem>,
    pub on_close: Callback<()>,
    pub on_item_sent: Callback<AttentionItem>,
    pub on_item_dismissed: Callback<AttentionItem>,
}

#[function_component(QuickReplyFlow)]
pub fn quick_reply_flow(props: &QuickReplyFlowProps) -> Html {
    let edit_text = use_state(String::new);
    let sending = use_state(|| false);
    let all_done = use_state(|| false);
    let actioned_ids = use_state(HashSet::<i32>::new);
    let text_initialized = use_state(|| -1i32); // track which item we initialized text for

    // Filter to only items not yet actioned
    let remaining: Vec<&AttentionItem> = props
        .items
        .iter()
        .filter(|item| !actioned_ids.contains(&item.id))
        .collect();

    // Escape key handler
    {
        let on_close = props.on_close.clone();
        use_effect_with_deps(move |_| {
            let on_close = on_close.clone();
            let handler = Closure::wrap(Box::new(move |e: web_sys::KeyboardEvent| {
                if e.key() == "Escape" {
                    on_close.emit(());
                }
            }) as Box<dyn FnMut(_)>);

            if let Some(window) = web_sys::window() {
                let _ = window.add_event_listener_with_callback(
                    "keydown",
                    handler.as_ref().unchecked_ref(),
                );
            }

            let cleanup = handler;
            move || {
                if let Some(window) = web_sys::window() {
                    let _ = window.remove_event_listener_with_callback(
                        "keydown",
                        cleanup.as_ref().unchecked_ref(),
                    );
                }
            }
        }, ());
    }

    // Auto-close after "all done" state
    {
        let all_done_val = *all_done;
        let on_close = props.on_close.clone();
        use_effect_with_deps(move |done| {
            if *done {
                let timeout = gloo_timers::callback::Timeout::new(1500, move || {
                    on_close.emit(());
                });
                timeout.forget();
            }
            || ()
        }, all_done_val);
    }

    if *all_done {
        return html! {
            <>
                <style>{TRIAGE_STYLES}</style>
                <div class="qr-overlay">
                    <div class="qr-card">
                        <div class="qr-done">
                            <div class="qr-done-check">{"*"}</div>
                            <div class="qr-done-text">{"All caught up"}</div>
                        </div>
                    </div>
                </div>
            </>
        };
    }

    if remaining.is_empty() {
        // All items were actioned
        all_done.set(true);
        return html! {
            <>
                <style>{TRIAGE_STYLES}</style>
                <div class="qr-overlay">
                    <div class="qr-card">
                        <div class="qr-done">
                            <div class="qr-done-check">{"*"}</div>
                            <div class="qr-done-text">{"All caught up"}</div>
                        </div>
                    </div>
                </div>
            </>
        };
    }

    let item = remaining[0];
    let item_clone = item.clone();

    // Service icon
    let service = item.service.as_deref().unwrap_or("");
    let (icon_class, icon_css) = match service {
        "whatsapp" => ("fa-brands fa-whatsapp", "qr-service-icon whatsapp"),
        "telegram" => ("fa-brands fa-telegram", "qr-service-icon telegram"),
        "signal" => ("fa-solid fa-comment-dots", "qr-service-icon signal"),
        _ => ("fa-solid fa-message", "qr-service-icon"),
    };

    let sender = item
        .sender_name
        .as_deref()
        .unwrap_or("Unknown");

    // Close handler
    let on_close_click = {
        let on_close = props.on_close.clone();
        Callback::from(move |_: MouseEvent| {
            on_close.emit(());
        })
    };

    // Overlay click handler (close)
    let on_overlay_click = {
        let on_close = props.on_close.clone();
        Callback::from(move |_: MouseEvent| {
            on_close.emit(());
        })
    };

    // Prevent card click from closing
    let on_card_click = Callback::from(|e: MouseEvent| {
        e.stop_propagation();
    });

    // Initialize edit_text with suggested action for current item
    {
        let item_id = item.id;
        let text_initialized = text_initialized.clone();
        let edit_text = edit_text.clone();
        let suggested = item.suggested_action.clone().unwrap_or_default();
        if *text_initialized != item_id {
            edit_text.set(suggested);
            text_initialized.set(item_id);
        }
    }

    // Send handler
    let on_send = {
        let item_id = item.id;
        let sending = sending.clone();
        let edit_text = edit_text.clone();
        let on_item_sent = props.on_item_sent.clone();
        let item_for_sent = item.clone();
        let actioned_ids = actioned_ids.clone();
        Callback::from(move |_: MouseEvent| {
            let sending = sending.clone();
            let on_item_sent = on_item_sent.clone();
            let item_for_sent = item_for_sent.clone();
            let actioned_ids = actioned_ids.clone();
            let action_text = (*edit_text).clone();
            sending.set(true);
            spawn_local(async move {
                let url = format!("/api/triage/{}/execute", item_id);
                let result = if let Ok(req) =
                    Api::post(&url).json(&serde_json::json!({"action": action_text}))
                {
                    req.send().await
                } else {
                    sending.set(false);
                    return;
                };

                if let Ok(resp) = result {
                    if resp.ok() {
                        let mut ids = (*actioned_ids).clone();
                        ids.insert(item_id);
                        actioned_ids.set(ids);
                        on_item_sent.emit(item_for_sent);
                    }
                }
                sending.set(false);
            });
        })
    };

    // Skip handler - just move to next, don't call API
    let on_skip = {
        let actioned_ids = actioned_ids.clone();
        let on_item_dismissed = props.on_item_dismissed.clone();
        let item_for_dismiss = item.clone();
        Callback::from(move |_: MouseEvent| {
            let item_id = item_for_dismiss.id;
            let mut ids = (*actioned_ids).clone();
            ids.insert(item_id);
            actioned_ids.set(ids);
            on_item_dismissed.emit(item_for_dismiss.clone());
        })
    };

    // Edit input handler
    let on_edit_input = {
        let edit_text = edit_text.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlTextAreaElement = e.target_unchecked_into();
            edit_text.set(input.value());
        })
    };

    html! {
        <>
            <style>{TRIAGE_STYLES}</style>
            <div class="qr-overlay" onclick={on_overlay_click}>
                <div class="qr-card" onclick={on_card_click}>
                    <button class="qr-close" onclick={on_close_click}>
                        <i class="fa-solid fa-xmark"></i>
                    </button>

                    // Sender row
                    <div class="qr-sender-row">
                        <i class={classes!(icon_class, icon_css)}></i>
                        <span class="qr-sender-name">{sender}</span>
                    </div>

                    // Conversation thread
                    { if !item_clone.conversation_snippet.is_empty() {
                        html! {
                            <div class="qr-thread">
                                { for item_clone.conversation_snippet.iter().enumerate().map(|(i, msg)| {
                                    let is_last = i == item_clone.conversation_snippet.len() - 1;
                                    let class = if is_last { "qr-thread-msg qr-msg-highlight" } else { "qr-thread-msg" };
                                    let time_str = format_msg_time(msg.ts);
                                    html! {
                                        <div class={class}>
                                            <span class="qr-msg-sender">{format!("{}: ", &msg.sender)}</span>
                                            {&msg.text}
                                            if !time_str.is_empty() {
                                                <span class="qr-msg-time">{time_str}</span>
                                            }
                                        </div>
                                    }
                                })}
                            </div>
                        }
                    } else if let Some(ref orig) = item_clone.original_message {
                        let received = format_msg_time(item_clone.timestamp as i64);
                        html! {
                            <>
                                <div class="qr-thread">
                                    <div class="qr-thread-msg qr-msg-highlight">
                                        {format!("\"{}\"", orig)}
                                        if !received.is_empty() {
                                            <span class="qr-msg-time">{&received}</span>
                                        }
                                    </div>
                                </div>
                            </>
                        }
                    } else {
                        html! {}
                    }}

                    // Received timestamp
                    { {
                        let received = format_msg_time(item_clone.timestamp as i64);
                        if !received.is_empty() {
                            html! { <div class="qr-received-time">{format!("Received {}", received)}</div> }
                        } else {
                            html! {}
                        }
                    }}

                    // Draft reply
                    { if *sending {
                        html! { <div class="qr-sending">{"Sending..."}</div> }
                    } else if item_clone.suggested_action.is_some() {
                        html! {
                            <>
                                <div class="qr-draft-label">{"Draft reply"}</div>
                                <textarea
                                    class="qr-edit-area"
                                    rows="3"
                                    value={(*edit_text).clone()}
                                    oninput={on_edit_input}
                                />
                                <div class="qr-actions">
                                    <button class="qr-send-btn" onclick={on_send} disabled={*sending}>
                                        {"Send"}
                                    </button>
                                    <button class="qr-skip-btn" onclick={on_skip}>
                                        {"Skip"}
                                    </button>
                                </div>
                            </>
                        }
                    } else {
                        html! {
                            <div class="qr-actions">
                                <button class="qr-skip-btn" onclick={on_skip} style="flex:1;">
                                    {"Skip"}
                                </button>
                            </div>
                        }
                    }}
                </div>
            </div>
        </>
    }
}

// ---------------------------------------------------------------------------
// TrackedItemsList - compact list of auto-tracked email items
// ---------------------------------------------------------------------------

pub const TRACKED_ITEMS_STYLES: &str = r#"
.tracked-list {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
}
.tracked-item {
    display: flex;
    align-items: flex-start;
    gap: 0.6rem;
    padding: 0.5rem 0.6rem;
    background: rgba(245, 158, 11, 0.06);
    border: 1px solid rgba(245, 158, 11, 0.15);
    border-radius: 8px;
    transition: background 0.15s;
}
.tracked-item:hover {
    background: rgba(245, 158, 11, 0.1);
}
.tracked-icon {
    color: #e8a838;
    font-size: 0.85rem;
    width: 1.2rem;
    text-align: center;
    flex-shrink: 0;
    margin-top: 0.1rem;
}
.tracked-body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
}
.tracked-summary {
    font-size: 0.85rem;
    color: #ccc;
    line-height: 1.3;
}
.tracked-meta {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
}
.tracked-category {
    font-size: 0.65rem;
    color: #e8a838;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    font-weight: 600;
}
.tracked-due {
    font-size: 0.65rem;
    color: #999;
    flex-shrink: 0;
}
.tracked-due.overdue {
    color: #e57373;
    font-weight: 600;
}
.tracked-due.soon {
    color: #e8a838;
}
.tracked-detected {
    font-size: 0.65rem;
    color: #555;
}
.tracked-actions {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    flex-shrink: 0;
}
.tracked-btn {
    background: none;
    border: none;
    color: #666;
    font-size: 0.8rem;
    cursor: pointer;
    padding: 0.15rem 0.35rem;
    transition: color 0.15s;
    flex-shrink: 0;
}
.tracked-btn:hover {
    color: #81c784;
}
.tracked-btn.dismiss:hover {
    color: #e57373;
}
"#;

fn tracking_icon_class(item_type: &str) -> &'static str {
    match item_type {
        "email_invoice" => "fa-solid fa-file-invoice-dollar",
        "email_shipment" => "fa-solid fa-box",
        "email_deadline" => "fa-solid fa-clock",
        "email_document" => "fa-solid fa-file-signature",
        "email_appointment" => "fa-solid fa-calendar-check",
        _ => "fa-solid fa-envelope",
    }
}

fn tracking_category_label(item_type: &str) -> &'static str {
    match item_type {
        "email_invoice" => "Invoice",
        "email_shipment" => "Shipment",
        "email_deadline" => "Deadline",
        "email_document" => "Document",
        "email_appointment" => "Appointment",
        _ => "Email",
    }
}

/// Format a due date string (ISO format) into a relative description.
/// Returns (display_text, css_class) where css_class indicates urgency.
fn format_due_date(due_str: &str) -> (String, &'static str) {
    // Parse "YYYY-MM-DD" into components
    let parts: Vec<&str> = due_str.split('-').collect();
    if parts.len() != 3 {
        return (due_str.to_string(), "");
    }
    let (y, m, d) = match (parts[0].parse::<i32>(), parts[1].parse::<u32>(), parts[2].parse::<u32>()) {
        (Ok(y), Ok(m), Ok(d)) => (y, m, d),
        _ => return (due_str.to_string(), ""),
    };

    let now_ms = js_sys::Date::now();
    let now_secs = (now_ms / 1000.0) as i64;

    // Build target date at midnight local time via JS Date
    let target = js_sys::Date::new_0();
    target.set_full_year(y as u32);
    target.set_month(m - 1);
    target.set_date(d);
    target.set_hours(0);
    target.set_minutes(0);
    target.set_seconds(0);
    let target_secs = (target.get_time() / 1000.0) as i64;

    let diff_days = (target_secs - now_secs) / 86400;

    if diff_days < 0 {
        let abs_days = (-diff_days) as u32;
        if abs_days == 1 {
            ("1 day overdue".to_string(), "overdue")
        } else {
            (format!("{} days overdue", abs_days), "overdue")
        }
    } else if diff_days == 0 {
        ("Due today".to_string(), "overdue")
    } else if diff_days == 1 {
        ("Due tomorrow".to_string(), "soon")
    } else if diff_days <= 7 {
        (format!("Due in {} days", diff_days), "soon")
    } else {
        (format!("Due {}", due_str), "")
    }
}

#[derive(Properties, PartialEq, Clone)]
pub struct TrackedItemsListProps {
    pub items: Vec<AttentionItem>,
    pub on_complete: Callback<AttentionItem>,
    pub on_dismiss: Callback<AttentionItem>,
}

#[function_component(TrackedItemsList)]
pub fn tracked_items_list(props: &TrackedItemsListProps) -> Html {
    if props.items.is_empty() {
        return html! {};
    }

    html! {
        <>
            <style>{TRACKED_ITEMS_STYLES}</style>
            <div class="tracked-list">
                { for props.items.iter().map(|item| {
                    let icon = tracking_icon_class(&item.item_type);
                    let category = tracking_category_label(&item.item_type);

                    let due_date = item.context_json.as_ref()
                        .and_then(|v| v["due_date"].as_str())
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string());

                    let (due_display, due_class) = due_date.as_deref()
                        .map(format_due_date)
                        .unwrap_or_default();

                    let detected = format_msg_time(item.timestamp as i64);

                    let complete_item = item.clone();
                    let on_complete = props.on_complete.clone();
                    let dismiss_item = item.clone();
                    let on_dismiss = props.on_dismiss.clone();

                    html! {
                        <div class="tracked-item">
                            <i class={classes!("tracked-icon", icon)}></i>
                            <div class="tracked-body">
                                <span class="tracked-summary">{&item.summary}</span>
                                <div class="tracked-meta">
                                    <span class="tracked-category">{category}</span>
                                    if !due_display.is_empty() {
                                        <span class={classes!("tracked-due", due_class)}>{&due_display}</span>
                                    }
                                    if !detected.is_empty() {
                                        <span class="tracked-detected">{format!("Detected {}", detected)}</span>
                                    }
                                </div>
                            </div>
                            <div class="tracked-actions">
                                <button
                                    class="tracked-btn"
                                    title="Mark complete"
                                    onclick={Callback::from(move |e: MouseEvent| {
                                        e.stop_propagation();
                                        on_complete.emit(complete_item.clone());
                                    })}
                                >
                                    <i class="fa-solid fa-check"></i>
                                </button>
                                <button
                                    class="tracked-btn dismiss"
                                    title="Dismiss"
                                    onclick={Callback::from(move |e: MouseEvent| {
                                        e.stop_propagation();
                                        on_dismiss.emit(dismiss_item.clone());
                                    })}
                                >
                                    <i class="fa-solid fa-xmark"></i>
                                </button>
                            </div>
                        </div>
                    }
                })}
            </div>
        </>
    }
}
