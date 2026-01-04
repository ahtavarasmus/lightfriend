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
                    .info-button {
                        background: none;
                        border: none;
                        color: #F59E0B;
                        font-size: 1.2rem;
                        cursor: pointer;
                        padding: 0.5rem;
                        border-radius: 50%;
                        width: 32px;
                        height: 32px;
                        display: flex;
                        align-items: center;
                        justify-content: center;
                        transition: all 0.3s ease;
                    }
                    .info-button:hover {
                        background: rgba(245, 158, 11, 0.1);
                        transform: scale(1.1);
                    }
                    .info-section {
                        background: rgba(0, 0, 0, 0.2);
                        border: 1px solid rgba(245, 158, 11, 0.1);
                        border-radius: 12px;
                        padding: 1.5rem;
                        margin-top: 1rem;
                    }
                    .info-section h4 {
                        color: #F59E0B;
                        margin: 0 0 1rem 0;
                        font-size: 1rem;
                    }
                    .info-subsection {
                        color: #999;
                        font-size: 0.9rem;
                    }
                    .info-subsection ul {
                        margin: 0;
                        padding-left: 1.5rem;
                    }
                    .info-subsection li {
                        margin-bottom: 0.5rem;
                    }
                    .info-subsection li:last-child {
                        margin-bottom: 0;
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
                    <button
                        class="info-button"
                        onclick={Callback::from({
                            let show_info = show_info.clone();
                            move |_| show_info.set(!*show_info)
                        })}
                    >
                        {"ⓘ"}
                    </button>
                </div>
                <div class="flow-description">
                    {"Scheduled reminders and message monitoring - all set up via SMS or voice calls."}
                </div>
                <div class="info-section" style={if *show_info { "display: block" } else { "display: none" }}>
                    <h4>{"What You Can Do"}</h4>
                    <div class="info-subsection">
                        <ul>
                            <li><strong>{"Scheduled reminders: "}</strong>{"\"Remind me at 3pm to call mom\""}</li>
                            <li><strong>{"Timed actions: "}</strong>{"\"Turn on Tesla climate in 30 minutes\""}</li>
                            <li><strong>{"Message monitoring: "}</strong>{"\"Let me know when mom texts\""}</li>
                            <li><strong>{"Email watching: "}</strong>{"\"Notify me when I get an email about my job application\""}</li>
                            <li><strong>{"Conditional tasks: "}</strong>{"\"If mom hasn't replied by 8pm, remind me to follow up\""}</li>
                        </ul>
                    </div>
                    <h4>{"How It Works"}</h4>
                    <div class="info-subsection">
                        <ul>
                            <li>{"Just text or call Lightfriend and ask - tasks are created automatically"}</li>
                            <li>{"Scheduled tasks run at the specified time"}</li>
                            <li>{"Monitoring tasks check each incoming message/email until a match is found"}</li>
                            <li>{"Notifications sent via SMS or Call depending on your preference"}</li>
                        </ul>
                    </div>
                </div>
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
                        {"No active tasks. Ask Lightfriend via SMS or call: \"Remind me at 5pm to pick up groceries\""}
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

                            html! {
                                <li>
                                    <div class="task-header">
                                        <span class={classes!("trigger-badge", trigger_class)}>
                                            {format_trigger(&task.trigger)}
                                        </span>
                                        <span class={classes!("noti-type-badge", noti_class)}>
                                            {if noti_type == "call_sms" { "CALL+SMS".to_string() } else { noti_type.to_uppercase() }}
                                        </span>
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

// Re-export old names for backwards compatibility during transition
pub type WaitingCheck = Task;
pub type WaitingChecksProps = TasksSectionProps;
pub use TasksSection as WaitingChecksSection;
