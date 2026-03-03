use yew::prelude::*;
use wasm_bindgen_futures::spawn_local;
use serde::Deserialize;
use crate::utils::api::Api;

const STATS_STYLES: &str = r#"
.stats-card {
    background: rgba(30, 30, 46, 0.95);
    border: 1px solid rgba(126, 178, 255, 0.3);
    border-radius: 12px;
    padding: 1rem 1.25rem;
}
.stats-row {
    display: flex;
    gap: 0.75rem;
    justify-content: space-between;
}
.stats-item {
    flex: 1;
    text-align: center;
    padding: 0.5rem 0;
}
.stats-value {
    font-size: 1.5rem;
    font-weight: 700;
    color: #7EB2FF;
    line-height: 1;
}
.stats-unit {
    font-size: 0.7rem;
    color: #888;
    margin-top: 0.25rem;
}
.stats-label {
    font-size: 0.75rem;
    color: #999;
    margin-top: 0.35rem;
}
.stats-divider {
    width: 1px;
    background: rgba(255, 255, 255, 0.08);
    margin: 0.25rem 0;
}
"#;

#[derive(Deserialize)]
struct StatsResponse {
    days_active: i32,
    hours_saved: f32,
    notifications_reduced: i32,
}

#[function_component(WellbeingStats)]
pub fn wellbeing_stats() -> Html {
    let stats = use_state(|| None::<StatsResponse>);
    let loading = use_state(|| true);

    {
        let stats = stats.clone();
        let loading = loading.clone();
        use_effect_with_deps(move |_| {
            spawn_local(async move {
                if let Ok(resp) = Api::get("/api/wellbeing/stats").send().await {
                    if let Ok(data) = resp.json::<StatsResponse>().await {
                        stats.set(Some(data));
                    }
                }
                loading.set(false);
            });
            || ()
        }, ());
    }

    let (days, hours, notifs) = match &*stats {
        Some(s) => (s.days_active, s.hours_saved, s.notifications_reduced),
        None => (0, 0.0, 0),
    };

    html! {
        <>
            <style>{STATS_STYLES}</style>
            <div class="stats-card">
                <div class="stats-row">
                    <div class="stats-item">
                        <div class="stats-value">{days}</div>
                        <div class="stats-unit">{"days"}</div>
                        <div class="stats-label">{"Active"}</div>
                    </div>
                    <div class="stats-divider"></div>
                    <div class="stats-item">
                        <div class="stats-value">{format!("{:.0}", hours)}</div>
                        <div class="stats-unit">{"hours"}</div>
                        <div class="stats-label">{"Saved"}</div>
                    </div>
                    <div class="stats-divider"></div>
                    <div class="stats-item">
                        <div class="stats-value">{notifs}</div>
                        <div class="stats-unit">{"fewer"}</div>
                        <div class="stats-label">{"Notifications"}</div>
                    </div>
                </div>
            </div>
        </>
    }
}
