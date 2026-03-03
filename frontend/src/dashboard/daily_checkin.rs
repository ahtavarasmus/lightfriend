use yew::prelude::*;
use wasm_bindgen_futures::spawn_local;
use serde::Deserialize;
use crate::utils::api::Api;

const CHECKIN_STYLES: &str = r#"
.checkin-card {
    background: rgba(30, 30, 46, 0.95);
    border: 1px solid rgba(126, 178, 255, 0.3);
    border-radius: 12px;
    padding: 1rem 1.25rem;
}
.checkin-card.done {
    border-color: rgba(52, 211, 153, 0.4);
}
.checkin-header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 0.75rem;
}
.checkin-header i {
    color: #7EB2FF;
    font-size: 1.1rem;
}
.checkin-card.done .checkin-header i {
    color: #34D399;
}
.checkin-title {
    font-size: 0.95rem;
    color: #e0e0e0;
    font-weight: 500;
}
.checkin-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 0.5rem;
}
.checkin-label {
    font-size: 0.8rem;
    color: #999;
    width: 55px;
    flex-shrink: 0;
}
.checkin-emojis {
    display: flex;
    gap: 0.25rem;
}
.checkin-emoji {
    font-size: 1.3rem;
    cursor: pointer;
    padding: 0.15rem 0.25rem;
    border-radius: 6px;
    border: 2px solid transparent;
    background: transparent;
    transition: all 0.15s ease;
    filter: grayscale(0.7);
    opacity: 0.5;
}
.checkin-emoji:hover {
    filter: grayscale(0);
    opacity: 0.8;
}
.checkin-emoji.selected {
    filter: grayscale(0);
    opacity: 1;
    border-color: rgba(126, 178, 255, 0.5);
    background: rgba(126, 178, 255, 0.1);
}
.checkin-done-row {
    display: flex;
    gap: 1rem;
    justify-content: center;
    padding: 0.25rem 0;
}
.checkin-done-item {
    text-align: center;
}
.checkin-done-emoji {
    font-size: 1.3rem;
}
.checkin-done-label {
    font-size: 0.7rem;
    color: #888;
    margin-top: 0.15rem;
}
.checkin-submit {
    width: 100%;
    margin-top: 0.5rem;
    padding: 0.5rem;
    background: rgba(126, 178, 255, 0.15);
    border: 1px solid rgba(126, 178, 255, 0.3);
    border-radius: 8px;
    color: #7EB2FF;
    font-size: 0.85rem;
    cursor: pointer;
    transition: all 0.2s ease;
}
.checkin-submit:hover {
    background: rgba(126, 178, 255, 0.25);
}
.checkin-submit:disabled {
    opacity: 0.4;
    cursor: default;
}
"#;

const MOOD_EMOJIS: [&str; 5] = ["😞", "😐", "🙂", "😊", "😄"];
const ENERGY_EMOJIS: [&str; 5] = ["🪫", "😴", "🚶", "🏃", "⚡"];
const SLEEP_EMOJIS: [&str; 5] = ["😵", "😩", "😪", "😌", "💤"];

#[derive(Deserialize)]
struct TodayCheckinResponse {
    has_checkin: bool,
    checkin: Option<CheckinData>,
}

#[derive(Deserialize, Clone)]
struct CheckinData {
    mood: i32,
    energy: i32,
    sleep_quality: i32,
}

#[function_component(DailyCheckin)]
pub fn daily_checkin() -> Html {
    let mood = use_state(|| 0i32);
    let energy = use_state(|| 0i32);
    let sleep = use_state(|| 0i32);
    let done = use_state(|| false);
    let done_data = use_state(|| None::<CheckinData>);
    let loading = use_state(|| true);
    let saving = use_state(|| false);

    // Fetch today's checkin
    {
        let mood = mood.clone();
        let energy = energy.clone();
        let sleep = sleep.clone();
        let done = done.clone();
        let done_data = done_data.clone();
        let loading = loading.clone();
        use_effect_with_deps(move |_| {
            spawn_local(async move {
                if let Ok(resp) = Api::get("/api/wellbeing/checkin/today").send().await {
                    if let Ok(data) = resp.json::<TodayCheckinResponse>().await {
                        if data.has_checkin {
                            if let Some(c) = data.checkin {
                                mood.set(c.mood);
                                energy.set(c.energy);
                                sleep.set(c.sleep_quality);
                                done_data.set(Some(c));
                                done.set(true);
                            }
                        }
                    }
                }
                loading.set(false);
            });
            || ()
        }, ());
    }

    let on_submit = {
        let mood = mood.clone();
        let energy = energy.clone();
        let sleep = sleep.clone();
        let done = done.clone();
        let done_data = done_data.clone();
        let saving = saving.clone();
        Callback::from(move |_| {
            let m = *mood;
            let e = *energy;
            let s = *sleep;
            if m == 0 || e == 0 || s == 0 {
                return;
            }
            let done = done.clone();
            let done_data = done_data.clone();
            let saving = saving.clone();
            saving.set(true);
            spawn_local(async move {
                let body = serde_json::json!({
                    "mood": m,
                    "energy": e,
                    "sleep_quality": s
                });
                if let Ok(resp) = Api::post("/api/wellbeing/checkin")
                    .header("Content-Type", "application/json")
                    .body(body.to_string())
                    .send()
                    .await
                {
                    if resp.ok() {
                        done_data.set(Some(CheckinData {
                            mood: m,
                            energy: e,
                            sleep_quality: s,
                        }));
                        done.set(true);
                    }
                }
                saving.set(false);
            });
        })
    };

    let card_class = if *done { "checkin-card done" } else { "checkin-card" };

    html! {
        <>
            <style>{CHECKIN_STYLES}</style>
            <div class={card_class}>
                <div class="checkin-header">
                    <i class="fa-solid fa-heart-pulse"></i>
                    <span class="checkin-title">{"Daily Check-in"}</span>
                </div>

                if *done {
                    if let Some(ref data) = *done_data {
                        <div class="checkin-done-row">
                            <div class="checkin-done-item">
                                <div class="checkin-done-emoji">{MOOD_EMOJIS[(data.mood - 1) as usize]}</div>
                                <div class="checkin-done-label">{"Mood"}</div>
                            </div>
                            <div class="checkin-done-item">
                                <div class="checkin-done-emoji">{ENERGY_EMOJIS[(data.energy - 1) as usize]}</div>
                                <div class="checkin-done-label">{"Energy"}</div>
                            </div>
                            <div class="checkin-done-item">
                                <div class="checkin-done-emoji">{SLEEP_EMOJIS[(data.sleep_quality - 1) as usize]}</div>
                                <div class="checkin-done-label">{"Sleep"}</div>
                            </div>
                        </div>
                    }
                } else if !*loading {
                    {render_emoji_row("Mood", &MOOD_EMOJIS, &mood)}
                    {render_emoji_row("Energy", &ENERGY_EMOJIS, &energy)}
                    {render_emoji_row("Sleep", &SLEEP_EMOJIS, &sleep)}
                    <button
                        class="checkin-submit"
                        onclick={on_submit}
                        disabled={*mood == 0 || *energy == 0 || *sleep == 0 || *saving}
                    >
                        {if *saving { "Saving..." } else { "Save Check-in" }}
                    </button>
                }
            </div>
        </>
    }
}

fn render_emoji_row(label: &str, emojis: &[&str; 5], selected: &UseStateHandle<i32>) -> Html {
    let items: Vec<Html> = emojis
        .iter()
        .enumerate()
        .map(|(i, emoji)| {
            let val = (i + 1) as i32;
            let is_selected = *selected.clone() == val;
            let cls = if is_selected {
                "checkin-emoji selected"
            } else {
                "checkin-emoji"
            };
            let sel = selected.clone();
            let onclick = Callback::from(move |_| sel.set(val));
            html! {
                <button class={cls} onclick={onclick}>{emoji}</button>
            }
        })
        .collect();

    html! {
        <div class="checkin-row">
            <span class="checkin-label">{label}</span>
            <div class="checkin-emojis">{items}</div>
        </div>
    }
}
