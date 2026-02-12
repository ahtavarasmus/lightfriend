use yew::prelude::*;
use wasm_bindgen_futures::spawn_local;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use serde::{Deserialize, Serialize};
use crate::utils::api::Api;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Task {
    pub id: Option<i32>,
    pub user_id: i32,
    pub trigger: String,
    pub condition: Option<String>,
    pub action: String,
    pub notification_type: Option<String>,
    pub status: Option<String>,
    pub created_at: i32,
    pub is_permanent: Option<i32>,
    pub recurrence_rule: Option<String>,
    pub recurrence_time: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SetPermanenceRequest {
    pub is_permanent: bool,
    pub recurrence_rule: Option<String>,
    pub recurrence_time: Option<String>,
}

#[derive(Properties, PartialEq, Clone)]
pub struct TasksSectionProps {
    pub tasks: Vec<Task>,
    pub on_change: Callback<Vec<Task>>,
    pub phone_number: String,
    #[prop_or(false)]
    pub critical_disabled: bool,
}

// Helper to format trigger for display (converts UTC timestamp to local time)
fn format_trigger(trigger: &str) -> String {
    if trigger.starts_with("once_") {
        // Parse timestamp and format in user's local timezone
        if let Ok(ts) = trigger[5..].parse::<i64>() {
            // Use js_sys::Date to convert UTC timestamp to local time
            let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64((ts * 1000) as f64));
            let month = date.get_month(); // 0-11
            let day = date.get_date();
            let year = date.get_full_year();
            let hours = date.get_hours();
            let minutes = date.get_minutes();

            let month_name = match month {
                0 => "Jan", 1 => "Feb", 2 => "Mar", 3 => "Apr",
                4 => "May", 5 => "Jun", 6 => "Jul", 7 => "Aug",
                8 => "Sep", 9 => "Oct", 10 => "Nov", 11 => "Dec",
                _ => "???",
            };

            format!("At {} {}, {} {:02}:{:02}", month_name, day, year, hours, minutes)
        } else {
            "Scheduled".to_string()
        }
    } else if trigger == "recurring_email" {
        "When email arrives".to_string()
    } else if trigger == "recurring_messaging" {
        "When message arrives".to_string()
    } else {
        trigger.to_string()
    }
}

#[function_component(TasksSection)]
pub fn tasks_section(props: &TasksSectionProps) -> Html {
    let tasks_local = use_state(|| props.tasks.clone());
    let error_message = use_state(|| None::<String>);
    let show_info = use_state(|| false);

    let refresh_from_server = {
        let tasks_local = tasks_local.clone();
        let on_change = props.on_change.clone();
        Callback::from(move |_| {
            let tasks_local = tasks_local.clone();
            let on_change = on_change.clone();
            spawn_local(async move {
                if let Ok(resp) = Api::get("/api/filters/tasks")
                    .send()
                    .await
                {
                    if let Ok(list) = resp.json::<Vec<Task>>().await {
                        tasks_local.set(list.clone());
                        on_change.emit(list);
                    }
                }
            });
        })
    };

    // Load tasks when component mounts
    {
        let refresh_from_server = refresh_from_server.clone();
        use_effect_with_deps(
            move |_| {
                refresh_from_server.emit(());
                || ()
            },
            (),
        );
    }

    // Listen for chat-sent event to refresh tasks
    {
        let refresh = refresh_from_server.clone();
        use_effect_with_deps(
            move |_| {
                let refresh = refresh.clone();
                let closure = Closure::<dyn Fn()>::new(move || {
                    refresh.emit(());
                });

                let window = web_sys::window().unwrap();
                let func = closure.as_ref().unchecked_ref::<js_sys::Function>().clone();
                let _ = window.add_event_listener_with_callback(
                    "lightfriend-chat-sent",
                    &func
                );

                // Keep closure alive and clean up on unmount
                move || {
                    if let Some(window) = web_sys::window() {
                        let _ = window.remove_event_listener_with_callback(
                            "lightfriend-chat-sent",
                            &func
                        );
                    }
                    drop(closure);
                }
            },
            (),
        );
    }

    let cancel_task = {
        let refresh = refresh_from_server.clone();

        Callback::from(move |task_id: i32| {
            let refresh = refresh.clone();

            spawn_local(async move {
                let _ = Api::delete(&format!("/api/filters/task/{}", task_id))
                    .send()
                    .await;

                refresh.emit(());
            });
        })
    };

    let update_permanence = {
        let refresh = refresh_from_server.clone();
        let error_message = error_message.clone();

        Callback::from(move |(task_id, is_permanent, recurrence_rule, recurrence_time): (i32, bool, Option<String>, Option<String>)| {
            let refresh = refresh.clone();
            let error_message = error_message.clone();

            spawn_local(async move {
                let request = SetPermanenceRequest {
                    is_permanent,
                    recurrence_rule,
                    recurrence_time,
                };

                let request_result = Api::patch(&format!("/api/filters/task/{}/permanence", task_id))
                    .json(&request);

                let Ok(request_wrapper) = request_result else {
                    error_message.set(Some("Failed to serialize request".to_string()));
                    return;
                };

                match request_wrapper.send().await
                {
                    Ok(resp) if resp.ok() => {
                        error_message.set(None);
                        refresh.emit(());
                    }
                    Ok(resp) => {
                        if let Ok(err) = resp.json::<serde_json::Value>().await {
                            error_message.set(Some(err.get("error").and_then(|e| e.as_str()).unwrap_or("Failed to update").to_string()));
                        }
                    }
                    Err(e) => {
                        error_message.set(Some(format!("Network error: {}", e)));
                    }
                }
            });
        })
    };

    html! {
        <>
            <style>
                {r#"
                    .filter-header {
                        display: flex;
                        flex-direction: column;
                        gap: 0.5rem;
                        margin-bottom: 1.5rem;
                    }
                    .filter-title {
                        display: flex;
                        align-items: center;
                        gap: 1rem;
                    }
                    .filter-title h3 {
                        margin: 0;
                        color: #F59E0B;
                        font-size: 1.2rem;
                    }
                    .status-badge {
                        background: rgba(245, 158, 11, 0.1);
                        color: #F59E0B;
                        padding: 0.25rem 0.75rem;
                        border-radius: 12px;
                        font-size: 0.8rem;
                    }
                    .status-badge.active {
                        background: rgba(52, 211, 153, 0.1);
                        color: #34D399;
                    }
                    .flow-description {
                        color: #999;
                        font-size: 0.9rem;
                    }
                    .filter-list {
                        list-style: none;
                        padding: 0;
                        margin: 0;
                        display: flex;
                        flex-direction: column;
                        gap: 0.75rem;
                    }
                    .filter-list li {
                        display: flex;
                        flex-direction: column;
                        gap: 0.5rem;
                        padding: 1rem;
                        background: rgba(0, 0, 0, 0.2);
                        border: 1px solid rgba(245, 158, 11, 0.1);
                        border-radius: 12px;
                        color: #fff;
                    }
                    .task-header {
                        display: flex;
                        align-items: center;
                        gap: 1rem;
                    }
                    .task-condition {
                        color: #fff;
                        font-size: 0.95rem;
                    }
                    .task-action {
                        color: #999;
                        font-size: 0.85rem;
                    }
                    .trigger-badge {
                        padding: 0.25rem 0.75rem;
                        border-radius: 8px;
                        font-size: 0.8rem;
                        background: rgba(0, 0, 0, 0.2);
                    }
                    .trigger-badge.email {
                        color: #1E90FF;
                        border: 1px solid rgba(30, 144, 255, 0.2);
                    }
                    .trigger-badge.messaging {
                        color: #25D366;
                        border: 1px solid rgba(37, 211, 102, 0.2);
                    }
                    .trigger-badge.scheduled {
                        color: #F59E0B;
                        border: 1px solid rgba(245, 158, 11, 0.2);
                    }
                    .noti-type-badge {
                        padding: 0.25rem 0.75rem;
                        border-radius: 8px;
                        font-size: 0.8rem;
                        background: rgba(0, 0, 0, 0.2);
                    }
                    .noti-type-badge.sms {
                        color: #4ECDC4;
                        border: 1px solid rgba(78, 205, 196, 0.2);
                    }
                    .noti-type-badge.call {
                        color: #FF6347;
                        border: 1px solid rgba(255, 99, 71, 0.2);
                    }
                    .noti-type-badge.call_sms {
                        color: #9B59B6;
                        border: 1px solid rgba(155, 89, 182, 0.2);
                    }
                    .filter-list li:hover {
                        border-color: rgba(245, 158, 11, 0.2);
                        transform: translateY(-1px);
                        transition: all 0.3s ease;
                    }
                    .filter-list .delete-btn {
                        margin-left: auto;
                        background: none;
                        border: none;
                        color: #FF6347;
                        font-size: 1.2rem;
                        cursor: pointer;
                        padding: 0.5rem;
                        border-radius: 8px;
                        transition: all 0.3s ease;
                        display: flex;
                        align-items: center;
                        justify-content: center;
                        width: 32px;
                        height: 32px;
                    }
                    .filter-list .delete-btn:hover {
                        background: rgba(255, 99, 71, 0.1);
                        transform: scale(1.1);
                    }
                    .error-message {
                        background: rgba(255, 99, 71, 0.1);
                        border: 1px solid rgba(255, 99, 71, 0.2);
                        color: #FF6347;
                        padding: 1rem;
                        border-radius: 8px;
                        margin-bottom: 1rem;
                        font-size: 0.9rem;
                    }
                    .chat-banner {
                        display: flex;
                        align-items: center;
                        gap: 0.75rem;
                        padding: 0.75rem 1rem;
                        background: rgba(245, 158, 11, 0.08);
                        border: 1px solid rgba(245, 158, 11, 0.15);
                        border-radius: 10px;
                        color: #ccc;
                        font-size: 0.9rem;
                    }
                    .chat-banner .banner-icon {
                        color: #F59E0B;
                        font-size: 1rem;
                        flex-shrink: 0;
                    }
                    .examples-toggle {
                        background: none;
                        border: 1px solid rgba(245, 158, 11, 0.25);
                        color: #F59E0B;
                        padding: 0.35rem 0.75rem;
                        border-radius: 6px;
                        font-size: 0.8rem;
                        cursor: pointer;
                        transition: all 0.2s ease;
                        white-space: nowrap;
                        flex-shrink: 0;
                    }
                    .examples-toggle:hover {
                        background: rgba(245, 158, 11, 0.1);
                    }
                    .examples-section {
                        background: rgba(0, 0, 0, 0.2);
                        border: 1px solid rgba(245, 158, 11, 0.1);
                        border-radius: 12px;
                        padding: 1.25rem;
                        margin-top: 0.75rem;
                    }
                    .examples-section h4 {
                        color: #F59E0B;
                        margin: 0 0 0.5rem 0;
                        font-size: 0.9rem;
                    }
                    .examples-section h4:not(:first-child) {
                        margin-top: 1rem;
                    }
                    .examples-section ul {
                        margin: 0;
                        padding-left: 1.25rem;
                        color: #999;
                        font-size: 0.85rem;
                    }
                    .examples-section li {
                        margin-bottom: 0.35rem;
                    }
                    .examples-section li:last-child {
                        margin-bottom: 0;
                    }
                    .examples-limitations {
                        margin-top: 1rem;
                        padding-top: 0.75rem;
                        border-top: 1px solid rgba(255, 255, 255, 0.08);
                        color: #666;
                        font-size: 0.8rem;
                    }
                    .examples-limitations p {
                        margin: 0 0 0.25rem 0;
                    }
                    .save-indicator {
                        min-width: 24px;
                        height: 24px;
                        display: inline-flex;
                        align-items: center;
                        justify-content: center;
                        margin-left: 8px;
                    }
                    .save-spinner {
                        width: 16px;
                        height: 16px;
                        border: 2px solid rgba(245, 158, 11, 0.3);
                        border-top-color: #F59E0B;
                        border-radius: 50%;
                        animation: spin 1s linear infinite;
                    }
                    @keyframes spin {
                        to { transform: rotate(360deg); }
                    }
                    .save-success {
                        color: #22C55E;
                        font-size: 18px;
                    }
                    .save-error {
                        color: #EF4444;
                        cursor: help;
                        font-size: 18px;
                    }
                    .section-disabled {
                        opacity: 0.5;
                    }
                    .disabled-hint {
                        font-size: 0.75rem;
                        color: #666;
                        font-style: italic;
                        margin-left: 0.5rem;
                    }
                    .empty-state {
                        color: #666;
                        font-size: 0.9rem;
                        text-align: center;
                        padding: 2rem;
                        background: rgba(0, 0, 0, 0.1);
                        border-radius: 8px;
                    }
                    .task-permanence {
                        display: flex;
                        align-items: center;
                        gap: 0.75rem;
                        margin-top: 0.5rem;
                        padding-top: 0.5rem;
                        border-top: 1px solid rgba(255, 255, 255, 0.1);
                    }
                    .permanence-toggle {
                        display: flex;
                        align-items: center;
                        gap: 0.5rem;
                    }
                    .permanence-toggle label {
                        color: #999;
                        font-size: 0.85rem;
                        cursor: pointer;
                    }
                    .toggle-switch {
                        position: relative;
                        width: 40px;
                        height: 20px;
                    }
                    .toggle-switch input {
                        opacity: 0;
                        width: 0;
                        height: 0;
                    }
                    .toggle-slider {
                        position: absolute;
                        cursor: pointer;
                        top: 0;
                        left: 0;
                        right: 0;
                        bottom: 0;
                        background-color: rgba(255, 255, 255, 0.2);
                        transition: 0.3s;
                        border-radius: 20px;
                    }
                    .toggle-slider:before {
                        position: absolute;
                        content: "";
                        height: 16px;
                        width: 16px;
                        left: 2px;
                        bottom: 2px;
                        background-color: white;
                        transition: 0.3s;
                        border-radius: 50%;
                    }
                    .toggle-switch input:checked + .toggle-slider {
                        background-color: #F59E0B;
                    }
                    .toggle-switch input:checked + .toggle-slider:before {
                        transform: translateX(20px);
                    }
                    .recurrence-settings {
                        display: flex;
                        flex-wrap: wrap;
                        gap: 0.5rem;
                        align-items: center;
                        margin-top: 0.5rem;
                        padding: 0.75rem;
                        background: rgba(0, 0, 0, 0.2);
                        border-radius: 8px;
                    }
                    .recurrence-settings select,
                    .recurrence-settings input {
                        padding: 0.4rem 0.6rem;
                        border-radius: 6px;
                        border: 1px solid rgba(245, 158, 11, 0.3);
                        background: rgba(0, 0, 0, 0.3);
                        color: #fff;
                        font-size: 0.85rem;
                    }
                    .recurrence-settings select:focus,
                    .recurrence-settings input:focus {
                        outline: none;
                        border-color: #F59E0B;
                    }
                    .recurrence-settings label {
                        color: #999;
                        font-size: 0.8rem;
                    }
                    .day-selector {
                        display: flex;
                        gap: 0.25rem;
                    }
                    .day-selector button {
                        width: 28px;
                        height: 28px;
                        border-radius: 4px;
                        border: 1px solid rgba(245, 158, 11, 0.3);
                        background: rgba(0, 0, 0, 0.3);
                        color: #999;
                        font-size: 0.75rem;
                        cursor: pointer;
                        transition: all 0.2s;
                    }
                    .day-selector button.selected {
                        background: rgba(245, 158, 11, 0.3);
                        color: #F59E0B;
                        border-color: #F59E0B;
                    }
                    .day-selector button:hover {
                        border-color: #F59E0B;
                    }
                    .permanent-badge {
                        padding: 0.2rem 0.5rem;
                        border-radius: 6px;
                        font-size: 0.7rem;
                        background: rgba(245, 158, 11, 0.2);
                        color: #F59E0B;
                        border: 1px solid rgba(245, 158, 11, 0.3);
                    }
                "#}
            </style>
            <div class={classes!(if props.critical_disabled { "section-disabled" } else { "" })}>
            <div class="filter-header">
                <div class="filter-title">
                    <i class="fas fa-tasks" style="color: #4ECDC4;"></i>
                    <h3>{"Tasks"}</h3>
                    {if props.critical_disabled {
                        html! { <span class="disabled-hint">{"(not active)"}</span> }
                    } else {
                        html! {}
                    }}
                </div>
                <div class="flow-description">
                    {"Your active tasks - manage them here, create new ones through the chat."}
                </div>
                <div class="chat-banner">
                    <i class="fas fa-comment-dots banner-icon"></i>
                    <span>{"Create tasks through the chat - just describe what you need."}</span>
                    <button
                        class="examples-toggle"
                        onclick={Callback::from({
                            let show_info = show_info.clone();
                            move |_| show_info.set(!*show_info)
                        })}
                    >
                        {if *show_info { "Hide examples" } else { "Show examples" }}
                    </button>
                </div>
                {if *show_info {
                    html! {
                        <div class="examples-section">
                            <h4>{"Reminders & Scheduling"}</h4>
                            <ul>
                                <li>{"\"Remind me at 3pm to call mom\""}</li>
                                <li>{"\"Remind me every Monday at 9am to submit the weekly report\""}</li>
                                <li>{"\"Tell me to take my medicine every day at 8am and 8pm\""}</li>
                                <li>{"\"Remind me in 30 minutes to check the oven\""}</li>
                            </ul>
                            <h4>{"Morning & Evening Digests"}</h4>
                            <ul>
                                <li>{"\"Send me a daily digest at 8am with my emails and WhatsApp messages\""}</li>
                                <li>{"\"Give me a morning briefing at 7am with weather, calendar, and emails\""}</li>
                                <li>{"\"Send me an evening summary at 9pm of all messages I got today\""}</li>
                            </ul>
                            <h4>{"Message Monitoring"}</h4>
                            <ul>
                                <li>{"\"Let me know when mom texts me on WhatsApp\""}</li>
                                <li>{"\"Notify me if I get a message from my boss on Telegram\""}</li>
                                <li>{"\"Alert me when someone mentions 'project deadline' in any chat\""}</li>
                            </ul>
                            <h4>{"Email Watching"}</h4>
                            <ul>
                                <li>{"\"Tell me when I get an email about my job application\""}</li>
                                <li>{"\"Notify me if I receive an email from HR\""}</li>
                                <li>{"\"Watch for emails with 'invoice' in the subject\""}</li>
                            </ul>
                            <h4>{"Conditional Tasks"}</h4>
                            <ul>
                                <li>{"\"If it's above 25 degrees at 8am, remind me to water the plants\""}</li>
                                <li>{"\"Check the weather at 7am - if it's going to rain, remind me to bring an umbrella\""}</li>
                            </ul>
                            <h4>{"Smart Home / Tesla"}</h4>
                            <ul>
                                <li>{"\"Turn on Tesla climate at 7:30am every weekday\""}</li>
                                <li>{"\"Start warming up my car in 20 minutes\""}</li>
                            </ul>
                            <h4>{"Calendar"}</h4>
                            <ul>
                                <li>{"\"What's on my calendar today?\""}</li>
                                <li>{"\"Send me my calendar events every morning at 7am\""}</li>
                            </ul>
                            <div class="examples-limitations">
                                <p>{"Can't send messages on your behalf"}</p>
                                <p>{"Can't make purchases or payments"}</p>
                                <p>{"Can't access apps not connected in Capabilities"}</p>
                            </div>
                        </div>
                    }
                } else {
                    html! {}
                }}
            </div>
            {
                if let Some(error) = (*error_message).as_ref() {
                    html! {
                        <div class="error-message">
                            {error}
                        </div>
                    }
                } else {
                    html! {}
                }
            }
            {if (*tasks_local).is_empty() {
                html! {
                    <div class="empty-state">
                        {"No active tasks yet. Use the chat above to create your first task."}
                    </div>
                }
            } else {
                html! {
                    <ul class="filter-list">
                    {
                        (*tasks_local).iter().filter(|t| t.status.as_ref().map(|s| s == "active").unwrap_or(true)).map(|task| {
                            let task_id = task.id.unwrap_or(0);
                            let trigger_class = if task.trigger == "recurring_email" {
                                "email"
                            } else if task.trigger == "recurring_messaging" {
                                "messaging"
                            } else {
                                "scheduled"
                            };
                            let noti_type = task.notification_type.as_ref().map(|s| s.as_str()).unwrap_or("sms");
                            let noti_class = match noti_type {
                                "call" => "call",
                                "call_sms" => "call_sms",
                                _ => "sms",
                            };
                            let is_permanent = task.is_permanent.unwrap_or(0) == 1;
                            let is_time_based = task.trigger.starts_with("once_");
                            let current_rule = task.recurrence_rule.clone();
                            let current_time = task.recurrence_time.clone();

                            html! {
                                <li>
                                    <div class="task-header">
                                        <span class={classes!("trigger-badge", trigger_class)}>
                                            {format_trigger(&task.trigger)}
                                        </span>
                                        <span class={classes!("noti-type-badge", noti_class)}>
                                            {if noti_type == "call_sms" { "CALL+SMS".to_string() } else { noti_type.to_uppercase() }}
                                        </span>
                                        {if is_permanent {
                                            html! { <span class="permanent-badge">{"Recurring"}</span> }
                                        } else {
                                            html! {}
                                        }}
                                        <button class="delete-btn"
                                            onclick={Callback::from({
                                                let cancel_task = cancel_task.clone();
                                                move |_| cancel_task.emit(task_id)
                                            })}
                                        >{"×"}</button>
                                    </div>
                                    {if let Some(condition) = &task.condition {
                                        html! {
                                            <div class="task-condition">
                                                {"If: "}{condition}
                                            </div>
                                        }
                                    } else {
                                        html! {}
                                    }}
                                    <div class="task-action">
                                        {"Action: "}{&task.action}
                                    </div>
                                    <div class="task-permanence">
                                        <div class="permanence-toggle">
                                            <label class="toggle-switch">
                                                <input
                                                    type="checkbox"
                                                    checked={is_permanent}
                                                    onchange={Callback::from({
                                                        let update_permanence = update_permanence.clone();
                                                        let current_rule = current_rule.clone();
                                                        let current_time = current_time.clone();
                                                        move |e: Event| {
                                                            let target = e.target_unchecked_into::<web_sys::HtmlInputElement>();
                                                            let new_permanent = target.checked();
                                                            update_permanence.emit((
                                                                task_id,
                                                                new_permanent,
                                                                if new_permanent { current_rule.clone() } else { None },
                                                                if new_permanent { current_time.clone() } else { None },
                                                            ));
                                                        }
                                                    })}
                                                />
                                                <span class="toggle-slider"></span>
                                            </label>
                                            <label>{"Recurring"}</label>
                                        </div>
                                        {if is_permanent && is_time_based {
                                            html! {
                                                <TaskRecurrenceSettings
                                                    task_id={task_id}
                                                    recurrence_rule={current_rule}
                                                    recurrence_time={current_time}
                                                    on_change={update_permanence.clone()}
                                                />
                                            }
                                        } else {
                                            html! {}
                                        }}
                                    </div>
                                </li>
                            }
                        }).collect::<Html>()
                    }
                    </ul>
                }
            }}
            </div>
        </>
    }
}

// Save status for visual feedback
#[derive(Clone, PartialEq)]
enum SaveStatus {
    Idle,
    Saving,
    Success,
    Error(String),
}

// Recurrence settings component for time-based permanent tasks
#[derive(Properties, PartialEq, Clone)]
pub struct TaskRecurrenceSettingsProps {
    pub task_id: i32,
    pub recurrence_rule: Option<String>,
    pub recurrence_time: Option<String>,
    pub on_change: Callback<(i32, bool, Option<String>, Option<String>)>,
}

#[function_component(TaskRecurrenceSettings)]
pub fn task_recurrence_settings(props: &TaskRecurrenceSettingsProps) -> Html {
    let save_status = use_state(|| SaveStatus::Idle);

    let rule_type = use_state(|| {
        props.recurrence_rule.as_ref().map(|r| {
            if r.starts_with("weekly:") { "weekly" }
            else if r.starts_with("monthly:") { "monthly" }
            else { "daily" }  // Default to daily
        }).unwrap_or("daily").to_string()
    });

    let selected_days = use_state(|| {
        props.recurrence_rule.as_ref().and_then(|r| {
            if r.starts_with("weekly:") {
                Some(r[7..].split(',').filter_map(|s| s.parse::<u8>().ok()).collect::<Vec<_>>())
            } else {
                None
            }
        }).unwrap_or_default()
    });

    let monthly_day = use_state(|| {
        props.recurrence_rule.as_ref().and_then(|r| {
            if r.starts_with("monthly:") {
                r[8..].parse::<u8>().ok()
            } else {
                None
            }
        }).unwrap_or(1)
    });

    let time_value = use_state(|| {
        props.recurrence_time.clone().unwrap_or_else(|| "09:00".to_string())
    });

    let handle_save = {
        let task_id = props.task_id;
        let on_change = props.on_change.clone();
        let rule_type = rule_type.clone();
        let selected_days = selected_days.clone();
        let monthly_day = monthly_day.clone();
        let time_value = time_value.clone();
        let save_status = save_status.clone();

        Callback::from(move |_| {
            let rule = match (*rule_type).as_str() {
                "weekly" => {
                    let days: Vec<String> = (*selected_days).iter().map(|d| d.to_string()).collect();
                    if days.is_empty() {
                        Some("weekly:1".to_string())
                    } else {
                        Some(format!("weekly:{}", days.join(",")))
                    }
                }
                "monthly" => Some(format!("monthly:{}", *monthly_day)),
                _ => Some("daily".to_string()),  // Default to daily
            };
            let time = Some((*time_value).clone());

            // Set saving status
            save_status.set(SaveStatus::Saving);

            let save_status = save_status.clone();
            let on_change = on_change.clone();

            spawn_local(async move {
                let request = SetPermanenceRequest {
                    is_permanent: true,
                    recurrence_rule: rule.clone(),
                    recurrence_time: time.clone(),
                };

                let request_result = Api::patch(&format!("/api/filters/task/{}/permanence", task_id))
                    .json(&request);

                let Ok(request_wrapper) = request_result else {
                    save_status.set(SaveStatus::Error("Failed to serialize".to_string()));
                    return;
                };

                match request_wrapper.send().await {
                    Ok(resp) if resp.ok() => {
                        save_status.set(SaveStatus::Success);
                        on_change.emit((task_id, true, rule, time));

                        // Reset to idle after 2 seconds
                        let save_status = save_status.clone();
                        gloo_timers::callback::Timeout::new(2000, move || {
                            save_status.set(SaveStatus::Idle);
                        }).forget();
                    }
                    Ok(_) => {
                        save_status.set(SaveStatus::Error("Save failed".to_string()));
                    }
                    Err(_) => {
                        save_status.set(SaveStatus::Error("Network error".to_string()));
                    }
                }
            });
        })
    };

    html! {
        <div class="recurrence-settings">
            <label>{"Repeat:"}</label>
            <select
                value={(*rule_type).clone()}
                onchange={Callback::from({
                    let rule_type = rule_type.clone();
                    move |e: Event| {
                        let target = e.target_unchecked_into::<web_sys::HtmlSelectElement>();
                        rule_type.set(target.value());
                    }
                })}
            >
                <option value="daily">{"Daily"}</option>
                <option value="weekly">{"Weekly"}</option>
                <option value="monthly">{"Monthly"}</option>
            </select>

            <label>{"At:"}</label>
            <input
                type="time"
                value={(*time_value).clone()}
                onchange={Callback::from({
                    let time_value = time_value.clone();
                    move |e: Event| {
                        let target = e.target_unchecked_into::<web_sys::HtmlInputElement>();
                        time_value.set(target.value());
                    }
                })}
            />

            {if *rule_type == "weekly" {
                html! {
                    <div class="day-selector">
                        {["M", "T", "W", "T", "F", "S", "S"].iter().enumerate().map(|(i, day)| {
                            let day_num = (i + 1) as u8;
                            let is_selected = (*selected_days).contains(&day_num);
                            html! {
                                <button
                                    type="button"
                                    class={classes!(if is_selected { "selected" } else { "" })}
                                    onclick={Callback::from({
                                        let selected_days = selected_days.clone();
                                        move |_| {
                                            let mut days = (*selected_days).clone();
                                            if days.contains(&day_num) {
                                                days.retain(|&d| d != day_num);
                                            } else {
                                                days.push(day_num);
                                                days.sort();
                                            }
                                            selected_days.set(days);
                                        }
                                    })}
                                >
                                    {day}
                                </button>
                            }
                        }).collect::<Html>()}
                    </div>
                }
            } else {
                html! {}
            }}

            {if *rule_type == "monthly" {
                html! {
                    <>
                        <label>{"Day:"}</label>
                        <input
                            type="number"
                            min="1"
                            max="31"
                            value={monthly_day.to_string()}
                            style="width: 60px;"
                            onchange={Callback::from({
                                let monthly_day = monthly_day.clone();
                                move |e: Event| {
                                    let target = e.target_unchecked_into::<web_sys::HtmlInputElement>();
                                    if let Ok(day) = target.value().parse::<u8>() {
                                        monthly_day.set(day.clamp(1, 31));
                                    }
                                }
                            })}
                        />
                    </>
                }
            } else {
                html! {}
            }}

            <button
                type="button"
                style="padding: 0.4rem 0.8rem; background: #F59E0B; color: #000; border: none; border-radius: 6px; cursor: pointer; font-size: 0.85rem;"
                onclick={handle_save}
                disabled={matches!(*save_status, SaveStatus::Saving)}
            >
                {"Save"}
            </button>
            <span class="save-indicator">
                {match &*save_status {
                    SaveStatus::Saving => html! { <span class="save-spinner"></span> },
                    SaveStatus::Success => html! { <span class="save-success">{"✓"}</span> },
                    SaveStatus::Error(msg) => html! { <span class="save-error" title={msg.clone()}>{"✗"}</span> },
                    SaveStatus::Idle => html! {},
                }}
            </span>
        </div>
    }
}
