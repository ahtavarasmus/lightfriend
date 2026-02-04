use yew::prelude::*;
use web_sys::MouseEvent;
use wasm_bindgen_futures::spawn_local;
use serde::Deserialize;
use crate::utils::api::Api;

const ACTIVITY_STYLES: &str = r#"
.activity-panel-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.7);
    z-index: 1100;
    display: flex;
    justify-content: flex-end;
}
.activity-panel {
    width: 100%;
    max-width: 500px;
    height: 100%;
    background: #1a1a1a;
    overflow-y: auto;
    animation: slideInPanel 0.3s ease;
}
@keyframes slideInPanel {
    from { transform: translateX(100%); }
    to { transform: translateX(0); }
}
.activity-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1.25rem 1.5rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
    position: sticky;
    top: 0;
    background: #1a1a1a;
    z-index: 10;
}
.activity-header h2 {
    color: #fff;
    font-size: 1.25rem;
    font-weight: 600;
    margin: 0;
}
.activity-header .close-btn {
    background: transparent;
    border: none;
    color: #888;
    font-size: 1.5rem;
    cursor: pointer;
    padding: 0.25rem 0.5rem;
    line-height: 1;
}
.activity-header .close-btn:hover {
    color: #fff;
}
.activity-body {
    padding: 1.5rem;
}
.activity-loading,
.activity-error {
    color: #888;
    text-align: center;
    padding: 2rem;
}
.activity-error {
    color: #ff6b6b;
}
.activity-empty {
    text-align: center;
    padding: 2rem;
}
.activity-empty p {
    color: #888;
    margin: 0;
}
.activity-hint {
    font-size: 0.85rem;
    margin-top: 0.5rem !important;
    color: #666 !important;
}
.activity-list {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
}
.activity-item {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    padding: 0.75rem;
    background: rgba(255, 255, 255, 0.03);
    border-radius: 8px;
    border-left: 3px solid transparent;
}
.activity-item.success {
    border-left-color: #4CAF50;
}
.activity-item.failed {
    border-left-color: #f44336;
}
.activity-desc {
    color: #ddd;
    font-size: 0.9rem;
    flex: 1;
}
.activity-time {
    color: #666;
    font-size: 0.8rem;
    white-space: nowrap;
    margin-left: 1rem;
}
"#;

#[derive(Clone, PartialEq, Deserialize)]
pub struct ActivityEntry {
    pub id: i32,
    pub activity_type: String,
    pub created_at: i32,
    pub reason: Option<String>,
    pub success: Option<bool>,
}

#[derive(Properties, PartialEq, Clone)]
pub struct ActivityPanelProps {
    pub is_open: bool,
    pub on_close: Callback<()>,
}

#[function_component(ActivityPanel)]
pub fn activity_panel(props: &ActivityPanelProps) -> Html {
    let activities = use_state(|| Vec::<ActivityEntry>::new());
    let loading = use_state(|| true);
    let error = use_state(|| None::<String>);

    // Fetch activities when panel opens
    {
        let activities = activities.clone();
        let loading = loading.clone();
        let error = error.clone();
        let is_open = props.is_open;

        use_effect_with_deps(
            move |is_open: &bool| {
                if *is_open {
                    let activities = activities.clone();
                    let loading = loading.clone();
                    let error = error.clone();
                    loading.set(true);
                    error.set(None);

                    spawn_local(async move {
                        // Fetch from usage_logs endpoint
                        match Api::get("/api/profile/recent-activity").send().await {
                            Ok(response) => {
                                if response.ok() {
                                    match response.json::<Vec<ActivityEntry>>().await {
                                        Ok(data) => {
                                            activities.set(data);
                                        }
                                        Err(_) => {
                                            // Endpoint might not exist yet - show empty state
                                            activities.set(vec![]);
                                        }
                                    }
                                } else {
                                    activities.set(vec![]);
                                }
                            }
                            Err(_) => {
                                error.set(Some("Failed to load activity".to_string()));
                            }
                        }
                        loading.set(false);
                    });
                }
                || ()
            },
            is_open,
        );
    }

    if !props.is_open {
        return html! {};
    }

    let content = if *loading {
        html! { <div class="activity-loading">{"Loading..."}</div> }
    } else if let Some(err) = (*error).as_ref() {
        html! { <div class="activity-error">{err}</div> }
    } else if activities.is_empty() {
        html! {
            <div class="activity-empty">
                <p>{"No recent activity"}</p>
                <p class="activity-hint">{"Actions like sending digests, notifications, and reminders will appear here."}</p>
            </div>
        }
    } else {
        html! {
            <div class="activity-list">
                {
                    activities.iter().map(|activity| {
                        let description = format_activity(&activity);
                        let time_ago = format_time_ago(activity.created_at);
                        let success_class = match activity.success {
                            Some(true) => "success",
                            Some(false) => "failed",
                            None => "",
                        };

                        html! {
                            <div class={classes!("activity-item", success_class)}>
                                <div class="activity-desc">{description}</div>
                                <div class="activity-time">{time_ago}</div>
                            </div>
                        }
                    }).collect::<Html>()
                }
            </div>
        }
    };

    let overlay_click = {
        let on_close = props.on_close.clone();
        Callback::from(move |_: MouseEvent| {
            on_close.emit(());
        })
    };

    let stop_propagation = Callback::from(|e: MouseEvent| {
        e.stop_propagation();
    });

    html! {
        <>
            <style>{ACTIVITY_STYLES}</style>
            <div class="activity-panel-overlay" onclick={overlay_click}>
                <div class="activity-panel" onclick={stop_propagation}>
                    <div class="activity-header">
                        <h2>{"Recent Activity"}</h2>
                        <button
                            class="close-btn"
                            onclick={{
                                let cb = props.on_close.clone();
                                Callback::from(move |_| cb.emit(()))
                            }}
                        >
                            {"x"}
                        </button>
                    </div>
                    <div class="activity-body">
                        {content}
                    </div>
                </div>
            </div>
        </>
    }
}

fn format_activity(activity: &ActivityEntry) -> String {
    match activity.activity_type.as_str() {
        "digest" | "generate_digest" => {
            if let Some(ref reason) = activity.reason {
                format!("Sent digest: {}", reason)
            } else {
                "Sent morning digest".to_string()
            }
        }
        "sms" => {
            if let Some(ref reason) = activity.reason {
                format!("SMS notification: {}", reason)
            } else {
                "Sent SMS notification".to_string()
            }
        }
        "call" => {
            if let Some(ref reason) = activity.reason {
                format!("Voice call: {}", reason)
            } else {
                "Made voice call".to_string()
            }
        }
        "email_critical" | "email_priority" => {
            if let Some(ref reason) = activity.reason {
                format!("Email alert: {}", reason)
            } else {
                "Email notification".to_string()
            }
        }
        "whatsapp_critical" | "whatsapp_priority" => {
            if let Some(ref reason) = activity.reason {
                format!("WhatsApp alert: {}", reason)
            } else {
                "WhatsApp notification".to_string()
            }
        }
        "reminder" => {
            if let Some(ref reason) = activity.reason {
                reason.clone()
            } else {
                "Sent reminder".to_string()
            }
        }
        _ => {
            if let Some(ref reason) = activity.reason {
                reason.clone()
            } else {
                activity.activity_type.replace('_', " ")
            }
        }
    }
}

fn format_time_ago(timestamp: i32) -> String {
    let now = js_sys::Date::now() as i64 / 1000;
    let diff = now - timestamp as i64;

    if diff < 60 {
        "just now".to_string()
    } else if diff < 3600 {
        let mins = diff / 60;
        format!("{}m ago", mins)
    } else if diff < 86400 {
        let hours = diff / 3600;
        format!("{}h ago", hours)
    } else {
        let days = diff / 86400;
        format!("{}d ago", days)
    }
}
