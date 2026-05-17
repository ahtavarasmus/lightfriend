//! Dashboard section for managing commitment detection signal state.
//! Lists muted senders, always-track senders, and recent SMS-prompt history.
//! Shown as a collapsible panel below the Auto-track Commitments toggle in
//! settings — there's no value in surfacing it before the user has opted in.

use serde::Deserialize;
use wasm_bindgen_futures::spawn_local;
use web_sys::js_sys;
use yew::prelude::*;

use crate::utils::api::Api;

#[derive(Clone, PartialEq, Deserialize, Debug)]
struct SenderRuleView {
    id: i32,
    platform: String,
    sender_key: String,
    #[serde(default)]
    rule_type: String,
    #[serde(default)]
    source: String,
    created_at: i32,
}

#[derive(Clone, PartialEq, Deserialize, Debug, Default)]
struct SenderRulesResponse {
    #[serde(default)]
    muted: Vec<SenderRuleView>,
    #[serde(default)]
    always_track: Vec<SenderRuleView>,
}

#[derive(Clone, PartialEq, Deserialize, Debug)]
struct PromptView {
    #[allow(dead_code)]
    id: i32,
    platform: String,
    sender_display_name: String,
    commitment_description: String,
    sent_at: i32,
    user_label: Option<String>,
    #[serde(default)]
    resulting_event_id: Option<i32>,
}

#[derive(PartialEq)]
enum LoadState {
    Loading,
    Ready,
    Failed(String),
}

#[function_component(CommitmentDashboard)]
pub fn commitment_dashboard() -> Html {
    let rules = use_state(SenderRulesResponse::default);
    let prompts = use_state(Vec::<PromptView>::new);
    let load_state = use_state(|| LoadState::Loading);
    let expanded = use_state(|| false);

    // Initial fetch on mount, and refetch when the user expands the panel
    // (lets them see fresh data when they come back to it later).
    {
        let rules = rules.clone();
        let prompts = prompts.clone();
        let load_state = load_state.clone();
        let expanded_v = *expanded;
        use_effect_with_deps(
            move |_| {
                if expanded_v {
                    load_state.set(LoadState::Loading);
                    let rules = rules.clone();
                    let prompts = prompts.clone();
                    let load_state = load_state.clone();
                    spawn_local(async move {
                        let rules_result = match Api::get("/api/commitment/sender-rules")
                            .send()
                            .await
                        {
                            Ok(resp) if resp.ok() => resp.json::<SenderRulesResponse>().await.ok(),
                            _ => None,
                        };
                        let prompts_result = match Api::get("/api/commitment/recent-prompts")
                            .send()
                            .await
                        {
                            Ok(resp) if resp.ok() => resp.json::<Vec<PromptView>>().await.ok(),
                            _ => None,
                        };
                        match (rules_result, prompts_result) {
                            (Some(r), Some(p)) => {
                                rules.set(r);
                                prompts.set(p);
                                load_state.set(LoadState::Ready);
                            }
                            _ => load_state.set(LoadState::Failed(
                                "Couldn't load commitment data.".to_string(),
                            )),
                        }
                    });
                }
                || ()
            },
            expanded_v,
        );
    }

    let toggle_expanded = {
        let expanded = expanded.clone();
        Callback::from(move |_| expanded.set(!*expanded))
    };

    let remove_rule = {
        let rules = rules.clone();
        Callback::from(move |rule_id: i32| {
            let rules = rules.clone();
            spawn_local(async move {
                let url = format!("/api/commitment/sender-rules/{}", rule_id);
                let _ = Api::delete(&url).send().await;
                // Optimistic update: drop the rule from local state without a
                // full refetch. If the backend call failed the UI will be
                // inconsistent until next expand, which is acceptable for v1.
                let mut current = (*rules).clone();
                current.muted.retain(|r| r.id != rule_id);
                current.always_track.retain(|r| r.id != rule_id);
                rules.set(current);
            })
        })
    };

    let muted_count = rules.muted.len();
    let always_track_count = rules.always_track.len();

    html! {
        <div class="profile-field" style="flex-direction: column; align-items: stretch;">
            <div class="field-label-group" style="display: flex; justify-content: space-between; align-items: center; cursor: pointer;" onclick={toggle_expanded.clone()}>
                <span class="field-label">
                    {"Commitment detection rules"}
                    <span style="margin-left: 6px; font-size: 0.75rem; color: #888;">
                        { format!("({} muted, {} always-track)", muted_count, always_track_count) }
                    </span>
                </span>
                <span style="font-size: 0.85rem; color: #7EB2FF;">
                    { if *expanded { "Hide" } else { "Show" } }
                </span>
            </div>
            { if *expanded {
                html! {
                    <div style="margin-top: 12px; display: flex; flex-direction: column; gap: 16px;">
                        { render_load_state(&load_state) }
                        { if matches!(*load_state, LoadState::Ready) {
                            html! {
                                <>
                                    { render_rule_section("Muted senders", &rules.muted, "These senders won't trigger commitment SMS prompts.", remove_rule.clone()) }
                                    { render_rule_section("Always-track senders", &rules.always_track, "Commitments from these senders are tracked without asking.", remove_rule.clone()) }
                                    { render_prompts_section(&prompts) }
                                </>
                            }
                        } else {
                            html! {}
                        } }
                    </div>
                }
            } else {
                html! {}
            } }
        </div>
    }
}

fn render_load_state(state: &LoadState) -> Html {
    match state {
        LoadState::Loading => html! {
            <div style="color: #888; font-size: 0.9rem;">{"Loading..."}</div>
        },
        LoadState::Failed(msg) => html! {
            <div style="color: #FF7E7E; font-size: 0.9rem;">{ msg.clone() }</div>
        },
        LoadState::Ready => html! {},
    }
}

fn render_rule_section(
    title: &str,
    rules: &[SenderRuleView],
    description: &str,
    on_remove: Callback<i32>,
) -> Html {
    html! {
        <div>
            <div style="font-weight: 600; font-size: 0.9rem; margin-bottom: 4px;">{ title.to_string() }</div>
            <div style="font-size: 0.8rem; color: #999; margin-bottom: 8px;">{ description.to_string() }</div>
            { if rules.is_empty() {
                html! {
                    <div style="font-size: 0.85rem; color: #888; font-style: italic;">{"None yet."}</div>
                }
            } else {
                html! {
                    <ul style="list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 6px;">
                        { for rules.iter().map(|r| render_rule_row(r, on_remove.clone())) }
                    </ul>
                }
            } }
        </div>
    }
}

fn render_rule_row(rule: &SenderRuleView, on_remove: Callback<i32>) -> Html {
    let rule_id = rule.id;
    let onclick = Callback::from(move |_| on_remove.emit(rule_id));
    html! {
        <li style="display: flex; justify-content: space-between; align-items: center; padding: 6px 10px; background: rgba(255,255,255,0.03); border-radius: 4px;">
            <div style="display: flex; flex-direction: column; gap: 2px;">
                <span style="font-size: 0.9rem;">{ rule.sender_key.clone() }</span>
                <span style="font-size: 0.7rem; color: #888;">{ rule.platform.clone() }</span>
            </div>
            <button
                onclick={onclick}
                style="background: transparent; border: 1px solid #555; color: #ccc; padding: 4px 10px; border-radius: 3px; font-size: 0.8rem; cursor: pointer;"
            >
                {"Remove"}
            </button>
        </li>
    }
}

fn render_prompts_section(prompts: &[PromptView]) -> Html {
    html! {
        <div>
            <div style="font-weight: 600; font-size: 0.9rem; margin-bottom: 4px;">{"Recent prompts"}</div>
            <div style="font-size: 0.8rem; color: #999; margin-bottom: 8px;">
                {"The last 50 commitment-detection SMS prompts sent to you and how you replied."}
            </div>
            { if prompts.is_empty() {
                html! {
                    <div style="font-size: 0.85rem; color: #888; font-style: italic;">{"None yet."}</div>
                }
            } else {
                html! {
                    <ul style="list-style: none; padding: 0; margin: 0; display: flex; flex-direction: column; gap: 6px;">
                        { for prompts.iter().map(render_prompt_row) }
                    </ul>
                }
            } }
        </div>
    }
}

fn render_prompt_row(prompt: &PromptView) -> Html {
    let label_text = match prompt.user_label.as_deref() {
        Some("1") => "Tracked",
        Some("2") => "Tracked + always-track sender",
        Some("3") => "Muted sender",
        Some("4") => "Not a commitment",
        _ => "Pending",
    };
    let label_color = match prompt.user_label.as_deref() {
        Some("1") | Some("2") => "#7EFF9A",
        Some("3") => "#FFB97E",
        Some("4") => "#FF7E7E",
        _ => "#888",
    };
    html! {
        <li style="padding: 6px 10px; background: rgba(255,255,255,0.03); border-radius: 4px;">
            <div style="display: flex; justify-content: space-between; align-items: center; gap: 8px;">
                <span style="font-size: 0.9rem; flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">
                    { prompt.commitment_description.clone() }
                </span>
                <span style={format!("font-size: 0.75rem; color: {}; flex-shrink: 0;", label_color)}>
                    { label_text }
                </span>
            </div>
            <div style="font-size: 0.7rem; color: #888; margin-top: 2px;">
                { format!("{} via {} - {}", prompt.sender_display_name, prompt.platform, format_relative_ts(prompt.sent_at)) }
                { if let Some(eid) = prompt.resulting_event_id {
                    format!(" - event #{}", eid)
                } else {
                    String::new()
                } }
            </div>
        </li>
    }
}

fn format_relative_ts(unix_ts: i32) -> String {
    let now = js_sys::Date::now() as i64 / 1000;
    let delta = now - unix_ts as i64;
    if delta < 60 {
        "just now".to_string()
    } else if delta < 3600 {
        format!("{}m ago", delta / 60)
    } else if delta < 86400 {
        format!("{}h ago", delta / 3600)
    } else if delta < 86400 * 30 {
        format!("{}d ago", delta / 86400)
    } else {
        format!("{}mo ago", delta / (86400 * 30))
    }
}
