use crate::utils::api::Api;
use serde::Deserialize;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

const RULES_STYLES: &str = r#"
@keyframes ruleFlashIn {
    0% { background: rgba(126, 178, 255, 0.25); border-color: rgba(126, 178, 255, 0.4); }
    100% { background: rgba(255, 255, 255, 0.03); border-color: rgba(255, 255, 255, 0.06); }
}
.rule-card.new-rule {
    animation: ruleFlashIn 2s ease-out;
}
.rules-section {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
}
.rules-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
}
.rules-header-label {
    font-size: 0.75rem;
    color: #666;
    text-transform: uppercase;
    letter-spacing: 0.05em;
}
.rules-add-btn {
    background: transparent;
    border: 1px solid rgba(126, 178, 255, 0.3);
    color: #7EB2FF;
    font-size: 0.75rem;
    padding: 0.2rem 0.6rem;
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.2s;
}
.rules-add-btn:hover {
    background: rgba(126, 178, 255, 0.1);
}
.rule-card {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.5rem 0.7rem;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-left: 3px solid rgba(255, 255, 255, 0.06);
    transition: background 0.15s;
}
.rule-card:hover {
    background: rgba(255, 255, 255, 0.06);
}
.rule-card.recurring {
    border-left-color: #60a5fa;
}
.rule-card.oneshot {
    border-left-color: #a78bfa;
}
.rule-icon {
    font-size: 0.85rem;
    color: #666;
    width: 1.2rem;
    text-align: center;
    flex-shrink: 0;
}
.rule-icon.schedule { color: #7EB2FF; }
.rule-icon.event { color: #e8a838; }
.rule-body {
    flex: 1;
    min-width: 0;
}
.rule-name {
    font-size: 0.85rem;
    color: #ddd;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}
.rule-detail {
    font-size: 0.72rem;
    color: #888;
    display: flex;
    align-items: center;
    gap: 0.4rem;
    margin-top: 0.1rem;
}
.rule-prompt {
    font-size: 0.7rem;
    color: #777;
    margin-top: 0.15rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-style: italic;
}
.rule-countdown {
    font-size: 0.68rem;
    color: #7EB2FF;
    margin-left: 0.3rem;
}
.rule-detail-sep {
    color: #444;
}
.rule-status-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
}
.rule-type-badge {
    font-size: 0.6rem;
    padding: 0.05rem 0.35rem;
    border-radius: 3px;
    white-space: nowrap;
}
.rule-type-badge.recurring {
    color: #60a5fa;
    background: rgba(96, 165, 250, 0.12);
}
.rule-type-badge.oneshot {
    color: #a78bfa;
    background: rgba(167, 139, 250, 0.12);
}
.rule-status-dot.active { background: #4ade80; }
.rule-status-dot.paused { background: #facc15; }
.rule-status-dot.completed, .rule-status-dot.expired { background: #666; }
.rule-actions {
    display: flex;
    gap: 0.3rem;
    flex-shrink: 0;
}
.rule-action-btn {
    background: transparent;
    border: none;
    color: #555;
    font-size: 0.75rem;
    cursor: pointer;
    padding: 0.2rem 0.35rem;
    border-radius: 4px;
    transition: color 0.15s, background 0.15s;
}
.rule-action-btn:hover {
    color: #ccc;
    background: rgba(255, 255, 255, 0.08);
}
.rule-action-btn.delete:hover {
    color: #ff6b6b;
    background: rgba(255, 68, 68, 0.1);
}
.rules-empty {
    font-size: 0.8rem;
    color: #555;
    text-align: center;
    padding: 1rem 0;
}
"#;

#[derive(Clone, PartialEq, Deserialize, Debug)]
pub struct RuleData {
    pub id: i32,
    pub name: String,
    pub trigger_type: String,
    pub trigger_config: String,
    pub logic_type: String,
    pub logic_prompt: Option<String>,
    pub logic_fetch: Option<String>,
    pub action_type: String,
    pub action_config: String,
    pub status: String,
    pub next_fire_at: Option<i32>,
    pub expires_at: Option<i32>,
    pub last_triggered_at: Option<i32>,
    pub created_at: i32,
    pub updated_at: i32,
    #[serde(default)]
    pub flow_config: Option<String>,
}

#[derive(Properties, PartialEq)]
pub struct RulesSectionProps {
    pub on_create_click: Callback<()>,
    pub on_edit_click: Callback<RuleData>,
    pub refresh_seq: u32,
    #[prop_or_default]
    pub filter_trigger_type: Option<String>,
    #[prop_or_default]
    pub label_override: Option<String>,
    #[prop_or(true)]
    pub show_create_button: bool,
}

#[function_component(RulesSection)]
pub fn rules_section(props: &RulesSectionProps) -> Html {
    let rules = use_state(|| Vec::<RuleData>::new());
    let loading = use_state(|| true);
    let refresh_trigger = use_state(|| 0u32);
    let new_rule_id = use_state(|| None::<i32>);

    // Listen for rule-created events to highlight the new rule
    {
        let new_rule_id = new_rule_id.clone();
        let refresh_trigger = refresh_trigger.clone();
        use_effect_with_deps(
            move |_| {
                use wasm_bindgen::closure::Closure;
                use wasm_bindgen::JsCast;

                let callback = Closure::wrap(Box::new(move |e: web_sys::CustomEvent| {
                    if let Some(id) = e.detail().as_f64() {
                        new_rule_id.set(Some(id as i32));
                        refresh_trigger.set(js_sys::Date::now() as u32);
                        // Clear highlight after animation
                        let new_rule_id_clear = new_rule_id.clone();
                        gloo_timers::callback::Timeout::new(2500, move || {
                            new_rule_id_clear.set(None);
                        })
                        .forget();
                    }
                })
                    as Box<dyn Fn(web_sys::CustomEvent)>);

                if let Some(window) = web_sys::window() {
                    let _ = window.add_event_listener_with_callback(
                        "lightfriend-rule-created",
                        callback.as_ref().unchecked_ref(),
                    );
                }

                move || {
                    if let Some(window) = web_sys::window() {
                        let _ = window.remove_event_listener_with_callback(
                            "lightfriend-rule-created",
                            callback.as_ref().unchecked_ref(),
                        );
                    }
                }
            },
            (),
        );
    }

    // Fetch rules when refresh_trigger or refresh_seq changes
    {
        let rules = rules.clone();
        let loading = loading.clone();
        let seq = props.refresh_seq;
        let trigger = *refresh_trigger;
        use_effect_with_deps(
            move |_| {
                let rules = rules.clone();
                let loading = loading.clone();
                spawn_local(async move {
                    match Api::get("/api/rules").send().await {
                        Ok(response) => {
                            if response.ok() {
                                if let Ok(data) = response.json::<Vec<RuleData>>().await {
                                    rules.set(data);
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

    // Listen for chat-sent and rules-changed events to auto-refresh
    {
        let refresh_trigger = refresh_trigger.clone();
        use_effect_with_deps(
            move |_| {
                use wasm_bindgen::closure::Closure;
                use wasm_bindgen::JsCast;

                let rt1 = refresh_trigger.clone();
                let rt2 = refresh_trigger.clone();

                let chat_cb = Closure::wrap(Box::new(move || {
                    // Use Date.now() to guarantee a unique value each time
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

    let on_create = {
        let cb = props.on_create_click.clone();
        Callback::from(move |_: MouseEvent| cb.emit(()))
    };

    let filtered_rules: Vec<&RuleData> = rules
        .iter()
        .filter(|r| r.status != "completed" && r.status != "expired")
        .filter(|r| match &props.filter_trigger_type {
            Some(ft) => r.trigger_type == *ft,
            None => true,
        })
        .collect();

    let filtered_empty = !*loading && filtered_rules.is_empty();

    html! {
        <>
            <style>{RULES_STYLES}</style>
            <div class="rules-section">
                <div class="rules-header">
                    <span class="rules-header-label">
                        {props.label_override.as_deref().unwrap_or("Rules")}
                    </span>
                    if props.show_create_button {
                        <button class="rules-add-btn" onclick={on_create}>
                            <i class="fa-solid fa-plus" style="margin-right: 0.3rem; font-size: 0.65rem;"></i>
                            {"New rule"}
                        </button>
                    }
                </div>

                if filtered_empty {
                    <div class="rules-empty">
                        {"No rules yet. Create one above."}
                    </div>
                }

                { for filtered_rules.iter().map(|rule| {
                    let is_new = *new_rule_id == Some(rule.id);
                    render_rule_card(rule, &props.on_edit_click, is_new, &rules)
                })}
            </div>
        </>
    }
}

fn render_rule_card(
    rule: &RuleData,
    on_edit: &Callback<RuleData>,
    is_new: bool,
    rules_state: &UseStateHandle<Vec<RuleData>>,
) -> Html {
    let rule_id = rule.id;
    let status = rule.status.clone();
    let is_active = status == "active";

    let icon_class = if rule.trigger_type == "schedule" {
        "rule-icon schedule fa-solid fa-clock"
    } else {
        "rule-icon event fa-solid fa-bolt"
    };

    let trigger_desc =
        describe_trigger(&rule.trigger_type, &rule.trigger_config, rule.next_fire_at);
    let action_desc = describe_action(&rule.action_type, &rule.action_config);

    // Countdown: use next_fire_at, or parse datetime from trigger_config as fallback
    let effective_fire_at = rule.next_fire_at.or_else(|| {
        if rule.trigger_type == "schedule" {
            let parsed: serde_json::Value =
                serde_json::from_str(&rule.trigger_config).unwrap_or_default();
            // Try "at" field first, then "schedule" field if it contains a datetime
            let datetime_str = parsed.get("at").and_then(|v| v.as_str()).or_else(|| {
                parsed
                    .get("schedule")
                    .and_then(|v| v.as_str())
                    .filter(|s| s.contains('T'))
            });
            datetime_str.and_then(|dt| {
                // Parse "YYYY-MM-DDTHH:MM" using JS Date
                let js_date = js_sys::Date::new(&wasm_bindgen::JsValue::from_str(dt));
                let ts = js_date.get_time();
                if ts.is_nan() {
                    None
                } else {
                    Some((ts / 1000.0) as i32)
                }
            })
        } else {
            None
        }
    });

    let countdown_text = effective_fire_at.and_then(|fire_at| {
        let now_secs = (js_sys::Date::now() / 1000.0) as i32;
        let diff = fire_at - now_secs;
        if diff <= 0 {
            None
        } else if diff < 60 {
            Some("in <1m".to_string())
        } else if diff < 3600 {
            Some(format!("in {}m", diff / 60))
        } else if diff < 86400 {
            let h = diff / 3600;
            let m = (diff % 3600) / 60;
            if m == 0 {
                Some(format!("in {}h", h))
            } else {
                Some(format!("in {}h {}m", h, m))
            }
        } else {
            let d = diff / 86400;
            let h = (diff % 86400) / 3600;
            Some(format!("in {}d {}h", d, h))
        }
    });

    // Logic prompt preview (what the rule does)
    let prompt_preview = rule.logic_prompt.as_ref().map(|p| {
        let trimmed = p.trim();
        if trimmed.len() > 80 {
            format!("{}...", &trimmed[..80])
        } else {
            trimmed.to_string()
        }
    });

    // Pause/resume toggle
    let on_toggle = {
        let new_status = if is_active { "paused" } else { "active" };
        let rules_state = rules_state.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            // Optimistic local update
            let mut updated = (*rules_state).clone();
            if let Some(r) = updated.iter_mut().find(|r| r.id == rule_id) {
                r.status = new_status.to_string();
            }
            rules_state.set(updated);
            let new_status = new_status.to_string();
            spawn_local(async move {
                let body = serde_json::json!({ "status": new_status });
                if let Ok(req) = Api::patch(&format!("/api/rules/{}/status", rule_id)).json(&body) {
                    let _ = req.send().await;
                }
                // Dispatch refresh event
                if let Some(window) = web_sys::window() {
                    let event = web_sys::CustomEvent::new("lightfriend-rules-changed").unwrap();
                    let _ = window.dispatch_event(&event);
                }
            });
        })
    };

    // Delete
    let on_delete = {
        let rules_state = rules_state.clone();
        Callback::from(move |e: MouseEvent| {
            e.stop_propagation();
            let window = web_sys::window().unwrap();
            if !window
                .confirm_with_message("Delete this rule?")
                .unwrap_or(false)
            {
                return;
            }
            // Optimistic local removal
            let updated: Vec<RuleData> = (*rules_state)
                .iter()
                .filter(|r| r.id != rule_id)
                .cloned()
                .collect();
            rules_state.set(updated);
            spawn_local(async move {
                let _ = Api::delete(&format!("/api/rules/{}", rule_id)).send().await;
                if let Some(window) = web_sys::window() {
                    let event = web_sys::CustomEvent::new("lightfriend-rules-changed").unwrap();
                    let _ = window.dispatch_event(&event);
                }
            });
        })
    };

    // Click card to view
    let rule_clone = rule.clone();
    let on_card_click = {
        let on_edit = on_edit.clone();
        Callback::from(move |_: MouseEvent| {
            on_edit.emit(rule_clone.clone());
        })
    };

    let toggle_icon = if is_active {
        "fa-solid fa-pause"
    } else {
        "fa-solid fa-play"
    };

    // Determine if rule is recurring (permanent) or one-shot (temporary)
    let is_recurring = {
        let parsed: serde_json::Value =
            serde_json::from_str(&rule.trigger_config).unwrap_or_default();
        if rule.trigger_type == "schedule" {
            parsed.get("schedule").and_then(|v| v.as_str()) == Some("recurring")
        } else {
            // monitoring rules: check fire_once flag (defaults to true = one-shot)
            let fire_once = parsed
                .get("fire_once")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            !fire_once
        }
    };

    let duration_class = if is_recurring { "recurring" } else { "oneshot" };
    let card_class = if is_new {
        format!("rule-card new-rule {}", duration_class)
    } else {
        format!("rule-card {}", duration_class)
    };

    html! {
        <div class={card_class} onclick={on_card_click}>
            <i class={icon_class}></i>
            <div class="rule-body">
                <div class="rule-name" style="display: flex; align-items: center; gap: 0.4rem;">
                    {&rule.name}
                    <span class={format!("rule-type-badge {}", duration_class)}>
                        {if is_recurring { "Recurring" } else { "One-time" }}
                    </span>
                </div>
                <div class="rule-detail">
                    <span>{trigger_desc}</span>
                    if let Some(ref ct) = countdown_text {
                        <span class="rule-countdown">{ct}</span>
                    }
                    <span class="rule-detail-sep">{"|"}</span>
                    <span>{action_desc}</span>
                </div>
                if let Some(ref prompt) = prompt_preview {
                    <div class="rule-prompt">{format!("\"{}\"", prompt)}</div>
                }
            </div>
            <div class={format!("rule-status-dot {}", status)} title={match status.as_str() {
                "active" => "Active",
                "paused" => "Paused",
                "completed" => "Completed",
                "expired" => "Expired",
                _ => "",
            }}></div>
            <div class="rule-actions">
                <button class="rule-action-btn" onclick={on_toggle} title={if is_active { "Pause" } else { "Resume" }}>
                    <i class={toggle_icon}></i>
                </button>
                <button class="rule-action-btn delete" onclick={on_delete} title="Delete">
                    <i class="fa-solid fa-xmark"></i>
                </button>
            </div>
        </div>
    }
}

fn describe_trigger(trigger_type: &str, trigger_config: &str, next_fire_at: Option<i32>) -> String {
    let parsed: serde_json::Value = serde_json::from_str(trigger_config).unwrap_or_default();

    if trigger_type == "schedule" {
        match parsed.get("schedule").and_then(|v| v.as_str()) {
            Some("once") => {
                if let Some(at) = parsed.get("at").and_then(|v| v.as_str()) {
                    format!("Once: {}", format_datetime_short(at))
                } else {
                    "Once".to_string()
                }
            }
            Some("recurring") => {
                if let Some(pattern) = parsed.get("pattern").and_then(|v| v.as_str()) {
                    format_pattern(pattern)
                } else {
                    "Recurring".to_string()
                }
            }
            Some(datetime) if datetime.contains('T') => {
                // schedule field contains a datetime directly (e.g. "2025-03-19T03:00")
                format!("Once: {}", format_datetime_short(datetime))
            }
            _ => {
                // Fallback: use next_fire_at to show when it fires
                if let Some(fire_at) = next_fire_at {
                    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(
                        fire_at as f64 * 1000.0,
                    ));
                    let hours = date.get_hours();
                    let minutes = date.get_minutes();
                    let time_str = format!("{:02}:{:02}", hours, minutes);
                    format!("Next: {}", format_time_12h(&time_str))
                } else {
                    "Scheduled".to_string()
                }
            }
        }
    } else {
        // ontology_change
        let entity = parsed
            .get("entity_type")
            .and_then(|v| v.as_str())
            .unwrap_or("event");
        let filters = parsed.get("filters").and_then(|v| v.as_object());
        if let Some(f) = filters {
            if let Some((key, val)) = f.iter().next() {
                let val_str = val.as_str().unwrap_or("");
                format!("When {} {} {}", entity.to_lowercase(), key, val_str)
            } else {
                format!("When {} received", entity.to_lowercase())
            }
        } else {
            format!("When {} received", entity.to_lowercase())
        }
    }
}

fn describe_action(action_type: &str, action_config: &str) -> String {
    let parsed: serde_json::Value = serde_json::from_str(action_config).unwrap_or_default();

    if action_type == "notify" {
        let method = parsed
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("sms");
        match method {
            "call" => "Call".to_string(),
            _ => "SMS".to_string(),
        }
    } else {
        // tool_call
        let tool = parsed
            .get("tool")
            .and_then(|v| v.as_str())
            .unwrap_or("tool");
        match tool {
            "create_event" => "Create obligation".to_string(),
            "send_email" => "Send email".to_string(),
            "send_chat_message" => "Send message".to_string(),
            "control_tesla" => "Tesla command".to_string(),
            "update_event" => "Update event".to_string(),
            _ => "Run action".to_string(),
        }
    }
}

fn format_datetime_short(at: &str) -> String {
    // Parse "YYYY-MM-DDTHH:MM" to something like "Mar 20, 2:30pm"
    if at.len() >= 16 {
        let month_day = &at[5..10]; // "03-20"
        let time = &at[11..16]; // "14:30"
        let parts: Vec<&str> = month_day.split('-').collect();
        if parts.len() == 2 {
            let month = match parts[0] {
                "01" => "Jan",
                "02" => "Feb",
                "03" => "Mar",
                "04" => "Apr",
                "05" => "May",
                "06" => "Jun",
                "07" => "Jul",
                "08" => "Aug",
                "09" => "Sep",
                "10" => "Oct",
                "11" => "Nov",
                "12" => "Dec",
                _ => parts[0],
            };
            let day: u32 = parts[1].parse().unwrap_or(0);
            return format!("{} {}, {}", month, day, format_time_12h(time));
        }
    }
    at.to_string()
}

fn format_time_12h(time: &str) -> String {
    let parts: Vec<&str> = time.split(':').collect();
    if parts.len() >= 2 {
        let hour: u32 = parts[0].parse().unwrap_or(0);
        let minute: u32 = parts[1].parse().unwrap_or(0);
        let (h12, ampm) = if hour == 0 {
            (12, "am")
        } else if hour < 12 {
            (hour, "am")
        } else if hour == 12 {
            (12, "pm")
        } else {
            (hour - 12, "pm")
        };
        if minute == 0 {
            format!("{}{}", h12, ampm)
        } else {
            format!("{}:{:02}{}", h12, minute, ampm)
        }
    } else {
        time.to_string()
    }
}

fn format_pattern(pattern: &str) -> String {
    let parts: Vec<&str> = pattern.splitn(2, ' ').collect();
    if parts.is_empty() {
        return pattern.to_string();
    }
    let freq = parts[0];
    let time_str = parts.get(1).unwrap_or(&"");

    match freq {
        "hourly" => "Every hour".to_string(),
        "daily" => format!("Daily at {}", format_time_12h(time_str)),
        "weekdays" => format!("Weekdays at {}", format_time_12h(time_str)),
        "weekly" => {
            let sub: Vec<&str> = time_str.splitn(2, ' ').collect();
            if sub.len() >= 2 {
                let day = capitalize(sub[0]);
                format!("{}s at {}", day, format_time_12h(sub[1]))
            } else {
                format!("Weekly {}", time_str)
            }
        }
        _ => pattern.to_string(),
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}
