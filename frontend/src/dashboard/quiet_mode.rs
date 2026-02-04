use yew::prelude::*;
use wasm_bindgen_futures::spawn_local;
use wasm_bindgen::prelude::*;
use wasm_bindgen::closure::Closure;
use serde::{Deserialize, Serialize};
use crate::utils::api::Api;
use web_sys::MouseEvent;
use std::rc::Rc;
use std::cell::RefCell;

const QUIET_MODE_STYLES: &str = r#"
.quiet-mode-indicator {
    position: relative;
    display: inline-block;
}
.quiet-mode-btn {
    background: transparent;
    border: 1px solid rgba(255, 255, 255, 0.15);
    color: #999;
    padding: 0.4rem 0.75rem;
    border-radius: 6px;
    font-size: 0.85rem;
    cursor: pointer;
    transition: all 0.2s ease;
    display: flex;
    align-items: center;
    gap: 0.5rem;
}
.quiet-mode-btn:hover {
    background: rgba(255, 255, 255, 0.05);
    border-color: rgba(255, 255, 255, 0.25);
    color: #ccc;
}
.quiet-mode-btn.active {
    background: rgba(52, 211, 153, 0.1);
    border-color: rgba(52, 211, 153, 0.3);
    color: #34D399;
}
.quiet-mode-btn.quiet {
    background: rgba(245, 158, 11, 0.1);
    border-color: rgba(245, 158, 11, 0.3);
    color: #F59E0B;
}
.quiet-mode-dropdown {
    position: absolute;
    bottom: 100%;
    left: 50%;
    transform: translateX(-50%);
    margin-bottom: 0.5rem;
    background: #1a1a1a;
    border: 1px solid rgba(255, 255, 255, 0.15);
    border-radius: 8px;
    padding: 0.5rem 0;
    min-width: 200px;
    box-shadow: 0 -4px 20px rgba(0, 0, 0, 0.4);
    z-index: 100;
}
.quiet-mode-option {
    width: 100%;
    background: transparent;
    border: none;
    color: #ccc;
    padding: 0.6rem 1rem;
    text-align: left;
    cursor: pointer;
    font-size: 0.9rem;
    transition: background 0.2s ease;
}
.quiet-mode-option:hover {
    background: rgba(255, 255, 255, 0.05);
}
.quiet-mode-option.turn-off {
    color: #34D399;
}
.quiet-mode-option.turn-off:hover {
    background: rgba(52, 211, 153, 0.1);
}
"#;

#[derive(Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct QuietModeStatus {
    pub is_quiet: bool,
    pub until: Option<i32>,
    pub until_display: Option<String>,
}

#[derive(Properties, PartialEq, Clone)]
pub struct QuietModeIndicatorProps {
    pub initial_status: QuietModeStatus,
}

// Helper to compute display text from a timestamp
fn compute_display_text(until: Option<i32>) -> String {
    match until {
        None => "Active".to_string(),
        Some(0) => "Quiet indefinitely".to_string(),
        Some(ts) => {
            let now = (js_sys::Date::now() / 1000.0) as i32;
            let diff = ts - now;
            if diff <= 0 {
                "Active".to_string()
            } else if diff < 3600 {
                let mins = diff / 60;
                format!("Quiet for {}m", mins)
            } else if diff < 86400 {
                let hours = diff / 3600;
                format!("Quiet for {}h", hours)
            } else {
                "Quiet until tomorrow".to_string()
            }
        }
    }
}

#[function_component(QuietModeIndicator)]
pub fn quiet_mode_indicator(props: &QuietModeIndicatorProps) -> Html {
    let status = use_state(|| props.initial_status.clone());
    let dropdown_open = use_state(|| false);
    let loading = use_state(|| false);
    let container_ref = use_node_ref();

    // Sync with props when they change
    {
        let status = status.clone();
        let initial = props.initial_status.clone();
        use_effect_with_deps(move |_| {
            status.set(initial);
            || ()
        }, props.initial_status.clone());
    }

    // Click outside handler using web_sys
    {
        let dropdown_open_for_effect = dropdown_open.clone();
        let dropdown_open_for_deps = dropdown_open.clone();
        let container_ref = container_ref.clone();
        use_effect_with_deps(
            move |is_open: &bool| {
                if !*is_open {
                    return Box::new(|| ()) as Box<dyn FnOnce()>;
                }

                let dropdown_open = dropdown_open_for_effect.clone();
                let container_ref = container_ref.clone();

                let closure: Rc<RefCell<Option<Closure<dyn Fn(MouseEvent)>>>> = Rc::new(RefCell::new(None));
                let closure_clone = closure.clone();

                let cb = Closure::wrap(Box::new(move |e: MouseEvent| {
                    if let Some(container) = container_ref.cast::<web_sys::HtmlElement>() {
                        if let Some(target) = e.target() {
                            let target_node = target.dyn_ref::<web_sys::Node>();
                            if let Some(target_node) = target_node {
                                if !container.contains(Some(target_node)) {
                                    dropdown_open.set(false);
                                }
                            }
                        }
                    }
                }) as Box<dyn Fn(MouseEvent)>);

                if let Some(window) = web_sys::window() {
                    if let Some(document) = window.document() {
                        let _ = document.add_event_listener_with_callback("mousedown", cb.as_ref().unchecked_ref());
                    }
                }

                *closure_clone.borrow_mut() = Some(cb);

                Box::new(move || {
                    if let Some(cb) = closure.borrow_mut().take() {
                        if let Some(window) = web_sys::window() {
                            if let Some(document) = window.document() {
                                let _ = document.remove_event_listener_with_callback("mousedown", cb.as_ref().unchecked_ref());
                            }
                        }
                    }
                }) as Box<dyn FnOnce()>
            },
            *dropdown_open_for_deps,
        );
    }

    let toggle_dropdown = {
        let dropdown_open = dropdown_open.clone();
        Callback::from(move |_| {
            dropdown_open.set(!*dropdown_open);
        })
    };

    let set_quiet_mode = {
        let status = status.clone();
        let dropdown_open = dropdown_open.clone();
        let loading = loading.clone();
        move |until: Option<i32>, optimistic_display: &'static str| {
            let status = status.clone();
            let dropdown_open = dropdown_open.clone();
            let loading = loading.clone();

            // Close dropdown immediately
            dropdown_open.set(false);

            // Optimistic update
            let optimistic_status = QuietModeStatus {
                is_quiet: until.is_some(),
                until,
                until_display: Some(optimistic_display.to_string()),
            };
            status.set(optimistic_status);

            loading.set(true);
            spawn_local(async move {
                let body = serde_json::json!({ "until": until });
                match Api::post("/api/profile/quiet-mode")
                    .header("Content-Type", "application/json")
                    .body(body.to_string())
                    .send()
                    .await
                {
                    Ok(resp) if resp.ok() => {
                        // Fetch the updated status to get server-computed display
                        if let Ok(resp) = Api::get("/api/profile/quiet-mode").send().await {
                            if let Ok(new_status) = resp.json::<QuietModeStatus>().await {
                                status.set(new_status);
                            }
                        }
                    }
                    _ => {
                        web_sys::console::error_1(&"Failed to set quiet mode".into());
                    }
                }
                loading.set(false);
            });
        }
    };

    let on_turn_off = {
        let set_quiet_mode = set_quiet_mode.clone();
        Callback::from(move |_| set_quiet_mode(None, "Active"))
    };

    let on_1_hour = {
        let set_quiet_mode = set_quiet_mode.clone();
        Callback::from(move |_| {
            let now = js_sys::Date::now() as i32 / 1000;
            set_quiet_mode(Some(now + 3600), "Quiet for 1h")
        })
    };

    let on_tomorrow_morning = {
        let set_quiet_mode = set_quiet_mode.clone();
        Callback::from(move |_| {
            // Calculate tomorrow at 9am in user's local timezone
            let now = js_sys::Date::new_0();
            let tomorrow = js_sys::Date::new_0();
            tomorrow.set_date(now.get_date() + 1);
            tomorrow.set_hours(9);
            tomorrow.set_minutes(0);
            tomorrow.set_seconds(0);
            tomorrow.set_milliseconds(0);
            let ts = (tomorrow.get_time() / 1000.0) as i32;
            set_quiet_mode(Some(ts), "Quiet until tomorrow")
        })
    };

    let on_indefinite = {
        let set_quiet_mode = set_quiet_mode.clone();
        Callback::from(move |_| set_quiet_mode(Some(0), "Quiet indefinitely"))
    };

    let (btn_class, icon, label) = if status.is_quiet {
        let display = status.until_display.clone()
            .unwrap_or_else(|| compute_display_text(status.until));
        ("quiet-mode-btn quiet", "fa-solid fa-bell-slash", display)
    } else {
        ("quiet-mode-btn active", "fa-solid fa-bell", "Active".to_string())
    };

    html! {
        <>
            <style>{QUIET_MODE_STYLES}</style>
            <div class="quiet-mode-indicator" ref={container_ref}>
                <button
                    class={btn_class}
                    onclick={toggle_dropdown.clone()}
                    disabled={*loading}
                >
                    <i class={icon}></i>
                    <span>{label}</span>
                    <i class="fa-solid fa-chevron-up" style="font-size: 0.7rem; margin-left: 0.25rem;"></i>
                </button>

                if *dropdown_open {
                    <div class="quiet-mode-dropdown">
                        if status.is_quiet {
                            <button class="quiet-mode-option turn-off" onclick={on_turn_off}>
                                {"Turn off quiet mode"}
                            </button>
                        }
                        if !status.is_quiet || status.until != Some(0) {
                            <button class="quiet-mode-option" onclick={on_1_hour}>
                                {"Quiet for 1 hour"}
                            </button>
                            <button class="quiet-mode-option" onclick={on_tomorrow_morning}>
                                {"Quiet until tomorrow morning"}
                            </button>
                            <button class="quiet-mode-option" onclick={on_indefinite}>
                                {"Quiet until I say so"}
                            </button>
                        }
                    </div>
                }
            </div>
        </>
    }
}
