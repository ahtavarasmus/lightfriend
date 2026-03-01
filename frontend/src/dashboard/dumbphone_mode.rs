use yew::prelude::*;
use wasm_bindgen_futures::spawn_local;
use serde::Deserialize;
use crate::utils::api::Api;

const DUMBPHONE_STYLES: &str = r#"
.dumbphone-card {
    background: rgba(30, 30, 46, 0.95);
    border: 1px solid rgba(126, 178, 255, 0.3);
    border-radius: 12px;
    padding: 1rem 1.25rem;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
}
.dumbphone-card.active {
    border-color: rgba(52, 211, 153, 0.5);
    background: rgba(52, 211, 153, 0.08);
}
.dumbphone-left {
    display: flex;
    align-items: center;
    gap: 0.75rem;
}
.dumbphone-icon {
    font-size: 1.3rem;
    color: #7EB2FF;
}
.dumbphone-card.active .dumbphone-icon {
    color: #34D399;
}
.dumbphone-label {
    font-size: 0.95rem;
    color: #e0e0e0;
    font-weight: 500;
}
.dumbphone-sub {
    font-size: 0.75rem;
    color: #888;
    margin-top: 0.15rem;
}
.toggle-switch {
    position: relative;
    width: 44px;
    height: 24px;
    background: rgba(255, 255, 255, 0.15);
    border-radius: 12px;
    cursor: pointer;
    transition: background 0.2s ease;
    border: none;
    padding: 0;
    flex-shrink: 0;
}
.toggle-switch.on {
    background: #34D399;
}
.toggle-switch::after {
    content: '';
    position: absolute;
    top: 2px;
    left: 2px;
    width: 20px;
    height: 20px;
    background: white;
    border-radius: 50%;
    transition: transform 0.2s ease;
}
.toggle-switch.on::after {
    transform: translateX(20px);
}
"#;

#[derive(Deserialize)]
struct DumbphoneResponse {
    on: bool,
}

#[function_component(DumbphoneMode)]
pub fn dumbphone_mode() -> Html {
    let on = use_state(|| false);
    let loading = use_state(|| true);

    // Fetch initial state
    {
        let on = on.clone();
        let loading = loading.clone();
        use_effect_with_deps(move |_| {
            spawn_local(async move {
                if let Ok(resp) = Api::get("/api/wellbeing/dumbphone").send().await {
                    if let Ok(data) = resp.json::<DumbphoneResponse>().await {
                        on.set(data.on);
                    }
                }
                loading.set(false);
            });
            || ()
        }, ());
    }

    let on_toggle = {
        let on = on.clone();
        Callback::from(move |_| {
            let new_val = !*on;
            let on = on.clone();
            on.set(new_val);
            spawn_local(async move {
                let body = serde_json::json!({ "on": new_val });
                let _ = Api::post("/api/wellbeing/dumbphone")
                    .header("Content-Type", "application/json")
                    .body(body.to_string())
                    .send()
                    .await;
            });
        })
    };

    let card_class = if *on {
        "dumbphone-card active"
    } else {
        "dumbphone-card"
    };

    let toggle_class = if *on {
        "toggle-switch on"
    } else {
        "toggle-switch"
    };

    html! {
        <>
            <style>{DUMBPHONE_STYLES}</style>
            <div class={card_class}>
                <div class="dumbphone-left">
                    <i class="fa-solid fa-phone dumbphone-icon"></i>
                    <div>
                        <div class="dumbphone-label">{"Dumbphone Mode"}</div>
                        <div class="dumbphone-sub">
                            {if *on { "Calls & SMS only" } else { "All notifications active" }}
                        </div>
                    </div>
                </div>
                <button
                    class={toggle_class}
                    onclick={on_toggle}
                    disabled={*loading}
                />
            </div>
        </>
    }
}
