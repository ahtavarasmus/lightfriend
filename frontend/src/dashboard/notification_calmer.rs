use yew::prelude::*;
use wasm_bindgen_futures::spawn_local;
use serde::Deserialize;
use crate::utils::api::Api;

const CALMER_STYLES: &str = r#"
.calmer-card {
    background: rgba(30, 30, 46, 0.95);
    border: 1px solid rgba(126, 178, 255, 0.3);
    border-radius: 12px;
    padding: 1rem 1.25rem;
}
.calmer-card.active {
    border-color: rgba(52, 211, 153, 0.5);
    background: rgba(52, 211, 153, 0.08);
}
.calmer-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
}
.calmer-left {
    display: flex;
    align-items: center;
    gap: 0.75rem;
}
.calmer-icon {
    font-size: 1.3rem;
    color: #7EB2FF;
}
.calmer-card.active .calmer-icon {
    color: #34D399;
}
.calmer-label {
    font-size: 0.95rem;
    color: #e0e0e0;
    font-weight: 500;
}
.calmer-sub {
    font-size: 0.75rem;
    color: #888;
    margin-top: 0.15rem;
}
.calmer-schedule {
    display: flex;
    gap: 0.5rem;
    margin-top: 0.75rem;
    padding-top: 0.75rem;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
}
.calmer-option {
    flex: 1;
    padding: 0.5rem;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    color: #999;
    font-size: 0.85rem;
    cursor: pointer;
    text-align: center;
    transition: all 0.2s ease;
}
.calmer-option:hover {
    background: rgba(126, 178, 255, 0.1);
    border-color: rgba(126, 178, 255, 0.3);
}
.calmer-option.selected {
    background: rgba(126, 178, 255, 0.15);
    border-color: rgba(126, 178, 255, 0.4);
    color: #7EB2FF;
}
"#;

#[derive(Deserialize)]
struct CalmerResponse {
    on: bool,
    schedule: Option<String>,
}

#[function_component(NotificationCalmer)]
pub fn notification_calmer() -> Html {
    let on = use_state(|| false);
    let schedule = use_state(|| "2x".to_string());
    let loading = use_state(|| true);

    // Fetch initial state
    {
        let on = on.clone();
        let schedule = schedule.clone();
        let loading = loading.clone();
        use_effect_with_deps(move |_| {
            spawn_local(async move {
                if let Ok(resp) = Api::get("/api/wellbeing/calmer").send().await {
                    if let Ok(data) = resp.json::<CalmerResponse>().await {
                        on.set(data.on);
                        if let Some(s) = data.schedule {
                            schedule.set(s);
                        }
                    }
                }
                loading.set(false);
            });
            || ()
        }, ());
    }

    let update_calmer = {
        let on = on.clone();
        let schedule = schedule.clone();
        move |new_on: bool, new_schedule: String| {
            let on = on.clone();
            let schedule = schedule.clone();
            on.set(new_on);
            schedule.set(new_schedule.clone());
            spawn_local(async move {
                let body = serde_json::json!({
                    "on": new_on,
                    "schedule": new_schedule
                });
                let _ = Api::post("/api/wellbeing/calmer")
                    .header("Content-Type", "application/json")
                    .body(body.to_string())
                    .send()
                    .await;
            });
        }
    };

    let on_toggle = {
        let on = on.clone();
        let schedule = schedule.clone();
        let update_calmer = update_calmer.clone();
        Callback::from(move |_| {
            let new_on = !*on;
            update_calmer(new_on, (*schedule).clone());
        })
    };

    let on_select_2x = {
        let update_calmer = update_calmer.clone();
        Callback::from(move |_| {
            update_calmer(true, "2x".to_string());
        })
    };

    let on_select_3x = {
        let update_calmer = update_calmer.clone();
        Callback::from(move |_| {
            update_calmer(true, "3x".to_string());
        })
    };

    let card_class = if *on { "calmer-card active" } else { "calmer-card" };
    let toggle_class = if *on { "toggle-switch on" } else { "toggle-switch" };

    html! {
        <>
            <style>{CALMER_STYLES}</style>
            <div class={card_class}>
                <div class="calmer-top">
                    <div class="calmer-left">
                        <i class="fa-solid fa-moon calmer-icon"></i>
                        <div>
                            <div class="calmer-label">{"Notification Calmer"}</div>
                            <div class="calmer-sub">
                                {if *on {
                                    format!("Batching to {}x/day", *schedule)
                                } else {
                                    "Batch notifications".to_string()
                                }}
                            </div>
                        </div>
                    </div>
                    <button
                        class={toggle_class}
                        onclick={on_toggle}
                        disabled={*loading}
                    />
                </div>
                if *on {
                    <div class="calmer-schedule">
                        <button
                            class={if *schedule == "2x" { "calmer-option selected" } else { "calmer-option" }}
                            onclick={on_select_2x}
                        >
                            {"2x / day"}
                        </button>
                        <button
                            class={if *schedule == "3x" { "calmer-option selected" } else { "calmer-option" }}
                            onclick={on_select_3x}
                        >
                            {"3x / day"}
                        </button>
                    </div>
                }
            </div>
        </>
    }
}
