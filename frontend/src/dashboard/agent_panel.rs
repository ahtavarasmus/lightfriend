use crate::utils::api::Api;
use serde::Deserialize;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::HtmlInputElement;
use yew::prelude::*;

const SETUP_INSTRUCTION: &str = "Set up Lightfriend's write-only local CLI. Clone https://github.com/ahtavarasmus/lightfriend, run `cargo install --path lightfriend/lightfriend-cli`, then run `lightfriend login` in the local terminal and give me the pairing code to approve in Lightfriend Settings > Connect an agent. Never paste or request a Lightfriend token in this chat, any prompt, or any URL. After login, use only `lightfriend remind --at <RFC3339 time with offset> --message <text>` or `lightfriend watch-reply --email <sender> --for-minutes <15-1440>`. This integration can create those outbound actions but cannot read conversations, contacts, history, reminders, or account data.";

const AGENT_STYLES: &str = r#"
.agent-panel { color: #111; background: #fff; border: 1px solid #111; border-radius: 10px; overflow: hidden; }
.agent-panel-header { padding: 1rem; border-bottom: 1px solid #111; }
.agent-panel-header h4 { margin: 0; color: #111; font-size: 1rem; }
.agent-panel-header p { margin: .45rem 0 0; color: #333; font-size: .78rem; line-height: 1.5; }
.agent-boundary { margin: .75rem 0 0; padding: .65rem .7rem; border: 1px solid #111; background: #f6f6f6; color: #111; font-size: .75rem; line-height: 1.45; }
.agent-body { padding: 1rem; display: grid; gap: 1rem; }
.agent-step { display: grid; gap: .45rem; }
.agent-step strong { color: #111; font-size: .8rem; }
.agent-step p { color: #444; font-size: .74rem; line-height: 1.45; margin: 0; }
.agent-code-form { display: flex; gap: .45rem; }
.agent-code-form input { min-width: 0; flex: 1; min-height: 42px; border: 1px solid #111; border-radius: 6px; background: #fff; color: #111; padding: .55rem .65rem; font: 600 .8rem/1 monospace; text-transform: uppercase; letter-spacing: .04em; }
.agent-button { min-height: 42px; padding: .55rem .75rem; border: 1px solid #111; border-radius: 6px; background: #111; color: #fff; font: 600 .76rem/1 sans-serif; cursor: pointer; }
.agent-button.secondary { background: #fff; color: #111; }
.agent-button:disabled { opacity: .5; cursor: wait; }
.agent-message { margin: 0; font-size: .74rem; color: #111; }
.agent-message.error { color: #9b111e; }
.agent-list { display: grid; gap: .5rem; }
.agent-row { display: flex; justify-content: space-between; align-items: center; gap: .75rem; padding: .7rem; border: 1px solid #bbb; border-radius: 6px; }
.agent-row-meta { min-width: 0; display: grid; gap: .16rem; }
.agent-row-label { color: #111; font-size: .8rem; font-weight: 650; }
.agent-row-sub { color: #555; font-size: .68rem; line-height: 1.35; overflow-wrap: anywhere; }
.agent-helper { position: fixed; right: 1rem; bottom: 1rem; z-index: 1250; width: min(330px, calc(100vw - 2rem)); padding: .85rem; border: 1px solid #111; border-radius: 12px; background: #fff; box-shadow: 0 10px 35px rgba(0,0,0,.25); color: #111; }
.agent-helper strong { display: block; font-size: .8rem; }
.agent-helper p { margin: .35rem 0 .65rem; color: #444; font-size: .7rem; line-height: 1.4; }
.agent-helper .agent-button { width: 100%; }
@media (max-width: 540px) { .agent-code-form { flex-direction: column; } .agent-helper { right: .65rem; bottom: .65rem; width: calc(100vw - 1.3rem); } }
@media (prefers-reduced-motion: reduce) { .agent-button { transition: none; } }
"#;

#[derive(Clone, Deserialize, PartialEq)]
struct CredentialSummary {
    id: i32,
    label: String,
    token_prefix: String,
    scopes: Vec<String>,
    daily_cap: i32,
    daily_used: i32,
    expires_at: i32,
    last_used_at: Option<i32>,
}

#[function_component(AgentPanel)]
pub fn agent_panel() -> Html {
    let credentials = use_state(Vec::<CredentialSummary>::new);
    let code = use_state(String::new);
    let busy = use_state(|| false);
    let message = use_state(|| None::<(bool, String)>);
    let copied = use_state(|| false);

    let refresh = {
        let credentials = credentials.clone();
        Callback::from(move |_| {
            let credentials = credentials.clone();
            spawn_local(async move {
                if let Ok(response) = Api::get("/api/me/agent-credentials").send().await {
                    if response.ok() {
                        if let Ok(rows) = response.json::<Vec<CredentialSummary>>().await {
                            credentials.set(rows);
                        }
                    }
                }
            });
        })
    };

    {
        let refresh = refresh.clone();
        use_effect_with_deps(
            move |_| {
                refresh.emit(());
                || ()
            },
            (),
        );
    }

    let on_code = {
        let code = code.clone();
        Callback::from(move |event: InputEvent| {
            code.set(event.target_unchecked_into::<HtmlInputElement>().value());
        })
    };

    let approve = {
        let code = code.clone();
        let busy = busy.clone();
        let message = message.clone();
        let refresh = refresh.clone();
        Callback::from(move |event: SubmitEvent| {
            event.prevent_default();
            if *busy || code.trim().is_empty() {
                return;
            }
            busy.set(true);
            message.set(None);
            let entered = (*code).clone();
            let code = code.clone();
            let busy = busy.clone();
            let message = message.clone();
            let refresh = refresh.clone();
            spawn_local(async move {
                let body = serde_json::json!({ "user_code": entered });
                let accepted = match Api::post("/api/me/agent-pairing/approve").json(&body) {
                    Ok(request) => matches!(request.send().await, Ok(response) if response.ok()),
                    Err(_) => false,
                };
                if accepted {
                    code.set(String::new());
                    message.set(Some((
                        true,
                        "Approved. The local CLI will finish securely.".to_string(),
                    )));
                    refresh.emit(());
                } else {
                    message.set(Some((
                        false,
                        "That code is invalid, expired, or already used.".to_string(),
                    )));
                }
                busy.set(false);
            });
        })
    };

    let revoke = {
        let credentials = credentials.clone();
        let message = message.clone();
        Callback::from(move |id: i32| {
            let credentials = credentials.clone();
            let message = message.clone();
            spawn_local(async move {
                match Api::delete(&format!("/api/me/agent-credentials/{id}"))
                    .send()
                    .await
                {
                    Ok(response) if response.ok() => {
                        credentials.set(
                            (*credentials)
                                .iter()
                                .filter(|credential| credential.id != id)
                                .cloned()
                                .collect(),
                        );
                        message.set(Some((
                            true,
                            "Credential revoked. Run `lightfriend login` to rotate it.".to_string(),
                        )));
                    }
                    _ => message.set(Some((
                        false,
                        "Could not revoke that credential.".to_string(),
                    ))),
                }
            });
        })
    };

    let copy_instruction = {
        let copied = copied.clone();
        Callback::from(move |_| {
            let copied = copied.clone();
            spawn_local(async move {
                let Some(window) = web_sys::window() else {
                    return;
                };
                let result =
                    JsFuture::from(window.navigator().clipboard().write_text(SETUP_INSTRUCTION))
                        .await;
                copied.set(result.is_ok());
            });
        })
    };

    html! {
        <>
            <style>{AGENT_STYLES}</style>
            <section class="agent-panel" aria-labelledby="agent-panel-title">
                <header class="agent-panel-header">
                    <h4 id="agent-panel-title">{"Connect an agent"}</h4>
                    <p>{"Use Codex, Claude Code, or another local assistant to create a reminder or start a short email reply watch."}</p>
                    <div class="agent-boundary" role="note">
                        <strong>{"Write-only by design. "}</strong>
                        {"Agents can ask Lightfriend to perform these limited outbound actions. They cannot read conversations, contacts, message history, existing reminders, or account data."}
                    </div>
                </header>
                <div class="agent-body">
                    <div class="agent-step">
                        <strong>{"1. Start pairing locally"}</strong>
                        <p>{"Copy the helper instruction below to your assistant. It uses `lightfriend login`; no secret is pasted into a chat, prompt, or URL."}</p>
                    </div>
                    <form class="agent-step" onsubmit={approve}>
                        <strong>{"2. Approve the one-time code"}</strong>
                        <div class="agent-code-form">
                            <input value={(*code).clone()} oninput={on_code} maxlength="14" autocomplete="one-time-code" spellcheck="false" aria-label="Pairing code" placeholder="ABCD-EFGH-JKLM" />
                            <button class="agent-button" type="submit" disabled={*busy}>{if *busy { "Approving..." } else { "Approve" }}</button>
                        </div>
                    </form>
                    if let Some((ok, text)) = (*message).as_ref() {
                        <p class={classes!("agent-message", (!*ok).then_some("error"))} role="status">{text}</p>
                    }
                    <div class="agent-step">
                        <strong>{"Connected credentials"}</strong>
                        if credentials.is_empty() {
                            <p>{"No local agents are connected."}</p>
                        } else {
                            <div class="agent-list">
                                {for credentials.iter().map(|credential| {
                                    let id = credential.id;
                                    let revoke = revoke.clone();
                                    let remaining = (credential.daily_cap - credential.daily_used).max(0);
                                    html! {
                                        <div class="agent-row">
                                            <div class="agent-row-meta">
                                                <span class="agent-row-label">{&credential.label}</span>
                                                <span class="agent-row-sub">{format!("{}… · {} actions left today · expires {}", credential.token_prefix, remaining, format_date(credential.expires_at))}</span>
                                                <span class="agent-row-sub">{credential.scopes.join(" · ")}</span>
                                            </div>
                                            <button class="agent-button secondary" type="button" onclick={Callback::from(move |_| revoke.emit(id))}>{"Revoke"}</button>
                                        </div>
                                    }
                                })}
                            </div>
                        }
                    </div>
                </div>
            </section>
            <aside class="agent-helper" aria-label="Agent setup helper">
                <strong>{"Ask your local agent to set this up"}</strong>
                <p>{"Copies a safe CLI instruction. It contains no token or private Lightfriend data."}</p>
                <button class="agent-button" type="button" onclick={copy_instruction}>{if *copied { "Copied" } else { "Copy safe setup instruction" }}</button>
            </aside>
        </>
    }
}

fn format_date(timestamp: i32) -> String {
    chrono::DateTime::from_timestamp(i64::from(timestamp), 0)
        .map(|date| date.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "unknown".to_string())
}
