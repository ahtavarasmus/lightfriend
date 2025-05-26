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
pub fn format_date_for_input(ts: i32) -> String {
    if ts == 0 { return String::new(); }
    let date = js_sys::Date::new(&js_sys::Number::from(ts as f64 * 1000.0));
    let y = date.get_full_year();
    let m = (date.get_month() + 1).to_string().pad(2, '0');
    let d = date.get_date().to_string().pad(2, '0');
    format!("{y}-{m}-{d}")
}
pub fn parse_date(str_: &str) -> i32 {
    if str_.is_empty() { 0 } else {
        js_sys::Date::new(&JsValue::from_str(str_)).get_time() as i32 / 1000
    }
}

