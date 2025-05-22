//! IMAP-related helpers that will gradually absorb code from `proactive.rs`.

use yew::prelude::*;
use gloo_net::http::Request;
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, js_sys, HtmlInputElement, KeyboardEvent, InputEvent, Event};
use wasm_bindgen::JsValue;
use serde_json::json;
use crate::config;
use crate::pages::proactive::{PrioritySender, EmailJudgmentResponse, WaitingCheck, ImportancePriority};
use super::common::{format_timestamp, KeywordsSection, PrioritySendersSection, ImportancePrioritySection};

pub fn render_filter_activity_log(
    judgments: &Option<Vec<EmailJudgmentResponse>>,
) -> Html {
    html! {
        <div class="filter-section">
            <h3>{"Filter Activity Log"}</h3>
            <div class="judgment-list">
                {
                    if let Some(list) = judgments {
                        list.iter().map(|j| {
                            let date         = format_timestamp(j.email_timestamp);
                            let processed_at = format_timestamp(j.processed_at);
                            html! {
                                <div class={classes!(
                                    "judgment-item",
                                    if j.should_notify { "notify" } else { "no-notify" }
                                )}>
                                    <div class="judgment-header">
                                        <span class="judgment-date">{date}</span>
                                        <span class={classes!(
                                            "judgment-status",
                                            if j.should_notify { "notify" } else { "no-notify" }
                                        )}>
                                            {if j.should_notify { "Notified" } else { "Skipped" }}
                                        </span>
                                    </div>
                                    <div class="judgment-score">
                                        <span class="score-label">{"Importance Score: "}</span>
                                        <span class="score-value">{j.score}{" / 10"}</span>
                                    </div>
                                    <div class="judgment-reason">
                                        <span class="reason-label">{"Reason: "}</span>
                                        <span class="reason-text">{&j.reason}</span>
                                    </div>
                                    <div class="judgment-processed">
                                        <span class="processed-label">{"Processed: "}</span>
                                        <span class="processed-date">{processed_at}</span>
                                    </div>
                                </div>
                            }
                        }).collect::<Html>()
                    } else {
                        html! {
                            <div class="loading-judgments">
                                {"Loading filter activity..."}
                            </div>
                        }
                    }
                }
            </div>
        </div>
    }
}


/// Component that *owns* the state + network fetch
#[function_component(FilterActivityLog)]
pub fn filter_activity_log() -> Html {
    let judgments = use_state(|| None::<Vec<EmailJudgmentResponse>>);

    {
        let judgments = judgments.clone();
        use_effect_with_deps(move |_| {
            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|s| s.get_item("token").ok())
                .flatten()
            {
                spawn_local(async move {
                    if let Ok(resp) = Request::get(&format!(
                        "{}/api/profile/email-judgments",
                        config::get_backend_url()
                    ))
                    .header("Authorization", &format!("Bearer {}", token))
                    .send()
                    .await
                    {
                        if let Ok(list) = resp.json::<Vec<EmailJudgmentResponse>>().await {
                            judgments.set(Some(list));
                        }
                    }
                });
            }
            || ()
        }, ());
    }

    html! { render_filter_activity_log(&*judgments) }
}



#[derive(Properties, PartialEq, Clone)]
pub struct WaitingChecksProps {
    pub service_type: String,
    pub checks: Vec<WaitingCheck>,
    pub on_change: Callback<Vec<WaitingCheck>>,
}

/* util ──────────────────────────────────────────────────────────────────── */
trait PadStart {
    fn pad(&self, width: usize, ch: char) -> String;
}
impl PadStart for String {
    fn pad(&self, width: usize, ch: char) -> String {
        if self.len() >= width { return self.clone(); }
        format!("{}{}", std::iter::repeat(ch).take(width - self.len()).collect::<String>(), self)
    }
}
fn format_date_for_input(ts: i32) -> String {
    if ts == 0 { return String::new(); }
    let date = js_sys::Date::new(&js_sys::Number::from(ts as f64 * 1000.0));
    let y = date.get_full_year();
    let m = (date.get_month() + 1).to_string().pad(2, '0');
    let d = date.get_date().to_string().pad(2, '0');
    format!("{y}-{m}-{d}")
}
fn parse_date(str_: &str) -> i32 {
    if str_.is_empty() { 0 } else {
        js_sys::Date::new(&JsValue::from_str(str_)).get_time() as i32 / 1000
    }
}
/* component ─────────────────────────────────────────────────────────────── */
#[function_component(WaitingChecksSection)]
pub fn waiting_checks_section(props: &WaitingChecksProps) -> Html {
    let new_content  = use_state(|| String::new());
    let new_due_date = use_state(|| 0);
    let new_remove   = use_state(|| false);

    let local_checks = use_state(|| props.checks.clone());
    /* sync local ↔ parent */
    {
        let local_checks = local_checks.clone();
        let parent_copy  = props.checks.clone();
        use_effect_with_deps(move |_| { local_checks.set(parent_copy); || () }, props.checks.clone());
    }

    /* refresh helper */
    let refresh = {
        let stype = props.service_type.clone();
        let loc   = local_checks.clone();
        let par   = props.on_change.clone();
        Callback::from(move |_| {
            if let Some(tok) = window().and_then(|w| w.local_storage().ok()).flatten()
                              .and_then(|s| s.get_item("token").ok()).flatten()
            {
                let stype = stype.clone(); let loc = loc.clone(); let par = par.clone();
                spawn_local(async move {
                    if let Ok(r) = Request::get(&format!(
                        "{}/api/filters/waiting-checks/{}", crate::config::get_backend_url(), stype
                    ))
                    .header("Authorization", &format!("Bearer {}", tok))
                    .send().await
                    {
                        if let Ok(list) = r.json::<Vec<WaitingCheck>>().await {
                            loc.set(list.clone()); par.emit(list);
                        }
                    }
                });
            }
        })
    };

    /* add helper */
    let add_check = {
        let stype   = props.service_type.clone();
        let cnt     = new_content.clone();
        let due     = new_due_date.clone();
        let rmv     = new_remove.clone();
        let reload  = refresh.clone();
        Callback::from(move |_| {
            let content = (*cnt).trim().to_string();
            if content.is_empty() { return; }
            if let Some(tok) = window().and_then(|w| w.local_storage().ok()).flatten()
                              .and_then(|s| s.get_item("token").ok()).flatten()
            {
                let stype = stype.clone();
                let cnt   = cnt.clone(); let due = due.clone(); let rmv = rmv.clone();
                let reload = reload.clone();
                spawn_local(async move {
                    let _ = Request::post(&format!(
                            "{}/api/filters/waiting-check/{}", crate::config::get_backend_url(), stype
                        ))
                        .header("Authorization", &format!("Bearer {}", tok))
                        .json(&json!({
                            "waiting_type": "content",
                            "content": content,
                            "due_date": *due,
                            "remove_when_found": *rmv,
                            "service_type": stype
                        })).unwrap()
                        .send().await;
                    cnt.set(String::new()); due.set(0); rmv.set(false);
                    reload.emit(());
                });
            }
        })
    };

    /* delete helper */
    let del_check = {
        let stype  = props.service_type.clone();
        let reload = refresh.clone();
        Callback::from(move |what: String| {
            if let Some(tok) = window().and_then(|w| w.local_storage().ok()).flatten()
                              .and_then(|s| s.get_item("token").ok()).flatten()
            {
                let stype = stype.clone(); let reload = reload.clone();
                spawn_local(async move {
                    let _ = Request::delete(&format!(
                            "{}/api/filters/waiting-check/{}/{}", crate::config::get_backend_url(), stype, what
                        ))
                        .header("Authorization", &format!("Bearer {}", tok))
                        .send().await;
                    reload.emit(());
                });
            }
        })
    };

    /* render */
    html! {
        <div class="filter-section">
            <h3>{"Waiting Checks"}</h3>

            <div class="waiting-check-input">
                <div class="waiting-check-fields">
                    <input
                        type="text"
                        placeholder="Content to wait for"
                        value={(*new_content).clone()}
                        oninput={Callback::from({
                            let s = new_content.clone();
                            move |e: InputEvent| {
                                let el: HtmlInputElement = e.target_unchecked_into();
                                s.set(el.value());
                            }
                        })}
                    />
                    <label class="date-label">
                        <input
                            type="date"
                            value={format_date_for_input(*new_due_date)}
                            onchange={Callback::from({
                                let d = new_due_date.clone();
                                move |e: Event| {
                                    let el: HtmlInputElement = e.target_unchecked_into();
                                    d.set(parse_date(&el.value()));
                                }
                            })}
                        />
                    </label>
                    <label>
                        <input
                            type="checkbox"
                            checked={*new_remove}
                            onchange={Callback::from({
                                let r = new_remove.clone();
                                move |e: Event| {
                                    let el: HtmlInputElement = e.target_unchecked_into();
                                    r.set(el.checked());
                                }
                            })}
                        />
                        {"Remove when found"}
                    </label>
                </div>
                <button
                    onclick={Callback::from({
                        let add_check = add_check.clone();
                        move |_| add_check.emit(())
                    })}
                >
                    {"Add"}
                </button>
            </div>

            <ul class="filter-list">
            {
                (*local_checks).iter().map(|chk| {
                    let what = chk.content.clone();
                    html! {
                        <li class="filter-item">
                            <span>{&chk.content}</span>
                            <span class="due-date">{crate::proactive::email::format_timestamp(chk.due_date)}</span>
                            <span class="remove-when-found">
                                { if chk.remove_when_found { "Remove when found" } else { "Keep after found" } }
                            </span>
                            <button class="delete-btn"
                                onclick={Callback::from({
                                    let what  = what.clone();
                                    let del_check = del_check.clone();
                                    move |_| del_check.emit(what.clone())
                                })}
                            >{"×"}</button>
                        </li>
                    }
                }).collect::<Html>()
            }
            </ul>
        </div>
    }
}

