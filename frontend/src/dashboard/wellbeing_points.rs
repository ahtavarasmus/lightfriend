use yew::prelude::*;
use wasm_bindgen_futures::spawn_local;
use serde::Deserialize;
use crate::utils::api::Api;

const POINTS_STYLES: &str = r#"
.points-card {
    background: rgba(30, 30, 46, 0.95);
    border: 1px solid rgba(126, 178, 255, 0.3);
    border-radius: 12px;
    padding: 1rem 1.25rem;
}
.points-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
}
.points-score {
    display: flex;
    align-items: baseline;
    gap: 0.35rem;
}
.points-number {
    font-size: 2rem;
    font-weight: 700;
    color: #7EB2FF;
    line-height: 1;
}
.points-label {
    font-size: 0.75rem;
    color: #888;
}
.points-streak {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.35rem 0.75rem;
    background: rgba(245, 158, 11, 0.1);
    border-radius: 20px;
    border: 1px solid rgba(245, 158, 11, 0.25);
}
.points-streak-icon {
    font-size: 1rem;
}
.points-streak-text {
    font-size: 0.85rem;
    color: #F59E0B;
    font-weight: 500;
}
.points-events {
    margin-top: 0.75rem;
    padding-top: 0.75rem;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
}
.points-event {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 0.8rem;
}
.points-event-label {
    color: #999;
}
.points-event-pts {
    color: #7EB2FF;
    font-weight: 500;
}
"#;

#[derive(Deserialize)]
struct PointsResponse {
    points: i32,
    current_streak: i32,
    recent_events: Vec<PointEvent>,
}

#[derive(Deserialize, Clone)]
struct PointEvent {
    event_type: String,
    points_earned: i32,
    event_date: String,
}

fn event_label(event_type: &str) -> &str {
    match event_type {
        "checkin" => "Daily check-in",
        "dumbphone_on" => "Dumbphone mode",
        "calmer_on" => "Notification calmer",
        _ => event_type,
    }
}

#[function_component(WellbeingPoints)]
pub fn wellbeing_points() -> Html {
    let points = use_state(|| 0i32);
    let streak = use_state(|| 0i32);
    let events = use_state(Vec::<PointEvent>::new);
    let loading = use_state(|| true);

    {
        let points = points.clone();
        let streak = streak.clone();
        let events = events.clone();
        let loading = loading.clone();
        use_effect_with_deps(move |_| {
            spawn_local(async move {
                if let Ok(resp) = Api::get("/api/wellbeing/points").send().await {
                    if let Ok(data) = resp.json::<PointsResponse>().await {
                        points.set(data.points);
                        streak.set(data.current_streak);
                        events.set(data.recent_events);
                    }
                }
                loading.set(false);
            });
            || ()
        }, ());
    }

    let recent: Vec<&PointEvent> = (*events).iter().take(5).collect();

    html! {
        <>
            <style>{POINTS_STYLES}</style>
            <div class="points-card">
                <div class="points-top">
                    <div class="points-score">
                        <span class="points-number">{*points}</span>
                        <span class="points-label">{"pts"}</span>
                    </div>
                    if *streak > 0 {
                        <div class="points-streak">
                            <span class="points-streak-icon">{"🔥"}</span>
                            <span class="points-streak-text">{format!("{} day{}", *streak, if *streak == 1 { "" } else { "s" })}</span>
                        </div>
                    }
                </div>
                if !recent.is_empty() {
                    <div class="points-events">
                        {for recent.iter().map(|e| html! {
                            <div class="points-event">
                                <span class="points-event-label">{format!("{} · {}", event_label(&e.event_type), &e.event_date)}</span>
                                <span class="points-event-pts">{format!("+{}", e.points_earned)}</span>
                            </div>
                        })}
                    </div>
                }
            </div>
        </>
    }
}
