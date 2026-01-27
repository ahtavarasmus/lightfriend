use yew::prelude::*;
use wasm_bindgen_futures::spawn_local;
use serde::{Deserialize, Serialize};
use crate::utils::api::Api;
use web_sys::HtmlInputElement;

#[derive(Clone, PartialEq)]
pub enum SaveState {
    Idle,
    Saving,
    Success,
    Error(String),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct TaskResponse {
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
    pub sources: Option<String>,
    pub source_lookback_hours: Option<i32>,
}

#[derive(Debug, Serialize, Clone)]
pub struct CreateTaskRequest {
    pub action: String,
    pub recurrence_rule: Option<String>,
    pub recurrence_time: Option<String>,
    pub sources: Option<String>,
    pub source_lookback_hours: Option<i32>,
    pub notification_type: Option<String>,
    pub condition: Option<String>,
}

#[derive(Properties, PartialEq)]
pub struct DigestSectionProps {
    pub phone_number: String,
    #[prop_or(false)]
    pub disabled: bool,
}

/// Format sources list for display
fn format_sources(sources: &Option<String>) -> String {
    match sources {
        Some(s) if !s.is_empty() => {
            s.split(',')
                .map(|src| match src.trim() {
                    "email" => "Email",
                    "whatsapp" => "WhatsApp",
                    "telegram" => "Telegram",
                    "signal" => "Signal",
                    "calendar" => "Calendar",
                    other => other,
                })
                .collect::<Vec<_>>()
                .join(", ")
        }
        _ => "None".to_string(),
    }
}

/// Format time for display (HH:MM -> human readable)
fn format_time(time: &Option<String>) -> String {
    match time {
        Some(t) => t.clone(),
        None => "Not set".to_string(),
    }
}

#[function_component(DigestSection)]
pub fn digest_section(props: &DigestSectionProps) -> Html {
    let tasks = use_state(Vec::<TaskResponse>::new);
    let loading = use_state(|| true);
    let save_state = use_state(|| SaveState::Idle);
    let show_create_form = use_state(|| false);

    // Form state for creating new digest
    let new_time = use_state(|| "08:00".to_string());
    let new_sources_email = use_state(|| true);
    let new_sources_whatsapp = use_state(|| true);
    let new_sources_telegram = use_state(|| true);
    let new_sources_signal = use_state(|| true);
    let new_sources_calendar = use_state(|| true);

    // Load tasks on mount
    {
        let tasks = tasks.clone();
        let loading = loading.clone();
        use_effect_with_deps(
            move |_| {
                spawn_local(async move {
                    if let Ok(resp) = Api::get("/api/filters/tasks").send().await {
                        if let Ok(all_tasks) = resp.json::<Vec<TaskResponse>>().await {
                            // Filter to show only digest tasks (recurring with generate_digest action)
                            let digest_tasks: Vec<TaskResponse> = all_tasks
                                .into_iter()
                                .filter(|t| {
                                    t.action == "generate_digest"
                                        && t.is_permanent == Some(1)
                                        && t.recurrence_rule.is_some()
                                })
                                .collect();
                            tasks.set(digest_tasks);
                        }
                    }
                    loading.set(false);
                });
                || ()
            },
            (),
        );
    }

    // Create digest task callback
    let on_create = {
        let tasks = tasks.clone();
        let save_state = save_state.clone();
        let show_create_form = show_create_form.clone();
        let new_time = new_time.clone();
        let new_sources_email = new_sources_email.clone();
        let new_sources_whatsapp = new_sources_whatsapp.clone();
        let new_sources_telegram = new_sources_telegram.clone();
        let new_sources_signal = new_sources_signal.clone();
        let new_sources_calendar = new_sources_calendar.clone();

        Callback::from(move |_| {
            let tasks = tasks.clone();
            let save_state = save_state.clone();
            let show_create_form = show_create_form.clone();

            // Build sources string
            let mut sources = Vec::new();
            if *new_sources_email {
                sources.push("email");
            }
            if *new_sources_whatsapp {
                sources.push("whatsapp");
            }
            if *new_sources_telegram {
                sources.push("telegram");
            }
            if *new_sources_signal {
                sources.push("signal");
            }
            if *new_sources_calendar {
                sources.push("calendar");
            }
            let sources_str = if sources.is_empty() {
                None
            } else {
                Some(sources.join(","))
            };

            let time = (*new_time).clone();

            save_state.set(SaveState::Saving);

            spawn_local(async move {
                let request = CreateTaskRequest {
                    action: "generate_digest".to_string(),
                    recurrence_rule: Some("daily".to_string()),
                    recurrence_time: Some(time),
                    sources: sources_str,
                    source_lookback_hours: Some(24),
                    notification_type: Some("sms".to_string()),
                    condition: None,
                };

                match Api::post("/api/filters/tasks")
                    .json(&request)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(response) if response.ok() => {
                        if let Ok(created_task) = response.json::<TaskResponse>().await {
                            let mut current_tasks = (*tasks).clone();
                            current_tasks.insert(0, created_task);
                            tasks.set(current_tasks);
                        }
                        save_state.set(SaveState::Success);
                        show_create_form.set(false);

                        // Reset to idle after a delay
                        let save_state_clone = save_state.clone();
                        spawn_local(async move {
                            gloo_timers::future::TimeoutFuture::new(2000).await;
                            save_state_clone.set(SaveState::Idle);
                        });
                    }
                    Ok(_) => {
                        save_state.set(SaveState::Error("Failed to create".to_string()));
                    }
                    Err(e) => {
                        save_state.set(SaveState::Error(format!("Error: {}", e)));
                    }
                }
            });
        })
    };

    // Delete task callback
    let on_delete = {
        let tasks = tasks.clone();
        Callback::from(move |task_id: i32| {
            let tasks = tasks.clone();
            spawn_local(async move {
                if let Ok(resp) = Api::delete(&format!("/api/filters/task/{}", task_id))
                    .send()
                    .await
                {
                    if resp.ok() {
                        let filtered: Vec<TaskResponse> = (*tasks)
                            .iter()
                            .filter(|t| t.id != Some(task_id))
                            .cloned()
                            .collect();
                        tasks.set(filtered);
                    }
                }
            });
        })
    };

    let disabled = props.disabled;

    html! {
        <div class="digest-section">
            <div class="service-header">
                <div class="service-name">
                    <i class="fa-solid fa-clock"></i>
                    {"Scheduled Digests"}
                </div>
            </div>
            <p class="service-description">
                {"Get a daily summary of your messages and calendar at scheduled times. Each digest pulls from your selected sources."}
            </p>

            if *loading {
                <div class="loading-spinner">{"Loading..."}</div>
            } else {
                // Existing digest tasks
                if !tasks.is_empty() {
                    <div class="digest-tasks-list">
                        { for (*tasks).iter().map(|task| {
                            let task_id = task.id.unwrap_or(0);
                            let on_delete = on_delete.clone();
                            html! {
                                <div class="digest-task-item" key={task_id}>
                                    <div class="digest-task-info">
                                        <div class="digest-task-time">
                                            <i class="fa-regular fa-clock"></i>
                                            {" "}{format_time(&task.recurrence_time)}
                                            {" daily"}
                                        </div>
                                        <div class="digest-task-sources">
                                            <i class="fa-solid fa-database"></i>
                                            {" Sources: "}{format_sources(&task.sources)}
                                        </div>
                                    </div>
                                    <button
                                        class="delete-task-btn"
                                        onclick={Callback::from(move |_| on_delete.emit(task_id))}
                                        disabled={disabled}
                                        title="Remove this digest"
                                    >
                                        <i class="fa-solid fa-trash"></i>
                                    </button>
                                </div>
                            }
                        })}
                    </div>
                }

                // Create new digest form
                if *show_create_form {
                    <div class="create-digest-form">
                        <h4>{"Add New Digest"}</h4>

                        <div class="form-group">
                            <label>{"Time (your local timezone)"}</label>
                            <input
                                type="time"
                                value={(*new_time).clone()}
                                onchange={{
                                    let new_time = new_time.clone();
                                    Callback::from(move |e: Event| {
                                        let input: HtmlInputElement = e.target_unchecked_into();
                                        new_time.set(input.value());
                                    })
                                }}
                                disabled={disabled}
                            />
                        </div>

                        <div class="form-group">
                            <label>{"Sources to include"}</label>
                            <div class="source-checkboxes">
                                <label class="checkbox-label">
                                    <input
                                        type="checkbox"
                                        checked={*new_sources_email}
                                        onchange={{
                                            let new_sources_email = new_sources_email.clone();
                                            Callback::from(move |e: Event| {
                                                let input: HtmlInputElement = e.target_unchecked_into();
                                                new_sources_email.set(input.checked());
                                            })
                                        }}
                                        disabled={disabled}
                                    />
                                    {"Email"}
                                </label>
                                <label class="checkbox-label">
                                    <input
                                        type="checkbox"
                                        checked={*new_sources_whatsapp}
                                        onchange={{
                                            let new_sources_whatsapp = new_sources_whatsapp.clone();
                                            Callback::from(move |e: Event| {
                                                let input: HtmlInputElement = e.target_unchecked_into();
                                                new_sources_whatsapp.set(input.checked());
                                            })
                                        }}
                                        disabled={disabled}
                                    />
                                    {"WhatsApp"}
                                </label>
                                <label class="checkbox-label">
                                    <input
                                        type="checkbox"
                                        checked={*new_sources_telegram}
                                        onchange={{
                                            let new_sources_telegram = new_sources_telegram.clone();
                                            Callback::from(move |e: Event| {
                                                let input: HtmlInputElement = e.target_unchecked_into();
                                                new_sources_telegram.set(input.checked());
                                            })
                                        }}
                                        disabled={disabled}
                                    />
                                    {"Telegram"}
                                </label>
                                <label class="checkbox-label">
                                    <input
                                        type="checkbox"
                                        checked={*new_sources_signal}
                                        onchange={{
                                            let new_sources_signal = new_sources_signal.clone();
                                            Callback::from(move |e: Event| {
                                                let input: HtmlInputElement = e.target_unchecked_into();
                                                new_sources_signal.set(input.checked());
                                            })
                                        }}
                                        disabled={disabled}
                                    />
                                    {"Signal"}
                                </label>
                                <label class="checkbox-label">
                                    <input
                                        type="checkbox"
                                        checked={*new_sources_calendar}
                                        onchange={{
                                            let new_sources_calendar = new_sources_calendar.clone();
                                            Callback::from(move |e: Event| {
                                                let input: HtmlInputElement = e.target_unchecked_into();
                                                new_sources_calendar.set(input.checked());
                                            })
                                        }}
                                        disabled={disabled}
                                    />
                                    {"Calendar"}
                                </label>
                            </div>
                        </div>

                        <div class="form-actions">
                            <button
                                class="btn-primary"
                                onclick={on_create}
                                disabled={disabled || matches!(*save_state, SaveState::Saving)}
                            >
                                {
                                    match &*save_state {
                                        SaveState::Saving => "Creating...",
                                        _ => "Create Digest"
                                    }
                                }
                            </button>
                            <button
                                class="btn-secondary"
                                onclick={{
                                    let show_create_form = show_create_form.clone();
                                    Callback::from(move |_| show_create_form.set(false))
                                }}
                                disabled={disabled}
                            >
                                {"Cancel"}
                            </button>
                        </div>

                        {
                            match &*save_state {
                                SaveState::Error(msg) => html! {
                                    <div class="error-message">{msg}</div>
                                },
                                SaveState::Success => html! {
                                    <div class="success-message">{"Digest created!"}</div>
                                },
                                _ => html! {}
                            }
                        }
                    </div>
                } else {
                    <button
                        class="add-digest-btn"
                        onclick={{
                            let show_create_form = show_create_form.clone();
                            Callback::from(move |_| show_create_form.set(true))
                        }}
                        disabled={disabled}
                    >
                        <i class="fa-solid fa-plus"></i>
                        {" Add Scheduled Digest"}
                    </button>
                }

                if tasks.is_empty() && !*show_create_form {
                    <p class="no-digests-message">
                        {"No scheduled digests yet. Add one to get daily summaries of your messages and calendar."}
                    </p>
                }
            }
        </div>
    }
}
