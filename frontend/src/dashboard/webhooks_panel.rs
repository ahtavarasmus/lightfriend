use crate::utils::api::Api;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use yew::prelude::*;

const WEBHOOKS_STYLES: &str = r#"
.webhooks-section { display: flex; flex-direction: column; gap: 0.75rem; }
.webhooks-help {
    font-size: 0.78rem; color: #888; line-height: 1.5;
    background: rgba(255,255,255,0.03); border-radius: 6px;
    padding: 0.65rem 0.75rem;
}
.webhooks-help code {
    background: rgba(255,255,255,0.06); padding: 0.05rem 0.3rem;
    border-radius: 3px; font-size: 0.72rem;
}
.webhooks-list { display: flex; flex-direction: column; gap: 0.4rem; }
.webhook-row {
    display: flex; align-items: center; justify-content: space-between;
    padding: 0.55rem 0.7rem; background: rgba(255,255,255,0.03);
    border-radius: 6px; gap: 0.5rem;
}
.webhook-meta { display: flex; flex-direction: column; min-width: 0; flex: 1; }
.webhook-label { font-size: 0.85rem; color: #ddd; font-weight: 500; }
.webhook-sub { font-size: 0.72rem; color: #888; margin-top: 0.15rem; }
.webhook-sub code {
    background: rgba(255,255,255,0.06); padding: 0.02rem 0.3rem;
    border-radius: 3px; font-size: 0.7rem;
}
.webhook-revoke {
    background: rgba(220,50,50,0.15); color: #f88;
    border: 1px solid rgba(220,50,50,0.3);
    border-radius: 4px; padding: 0.3rem 0.55rem;
    font-size: 0.72rem; cursor: pointer;
}
.webhook-revoke:hover { background: rgba(220,50,50,0.25); }
.webhook-create-form { display: flex; gap: 0.4rem; align-items: center; }
.webhook-create-form input {
    flex: 1; background: rgba(255,255,255,0.04);
    border: 1px solid rgba(255,255,255,0.1); color: #ddd;
    border-radius: 4px; padding: 0.4rem 0.55rem; font-size: 0.8rem;
}
.webhook-create-form button {
    background: rgba(100,180,255,0.15); color: #9cf;
    border: 1px solid rgba(100,180,255,0.3);
    border-radius: 4px; padding: 0.4rem 0.7rem;
    font-size: 0.8rem; cursor: pointer;
}
.webhook-create-form button:disabled { opacity: 0.4; cursor: wait; }
.webhook-new-token {
    background: rgba(255,200,80,0.08); border: 1px solid rgba(255,200,80,0.25);
    border-radius: 6px; padding: 0.6rem 0.7rem;
    display: flex; flex-direction: column; gap: 0.35rem;
}
.webhook-new-token-warn { font-size: 0.72rem; color: #fc8; }
.webhook-new-token-value {
    font-family: monospace; font-size: 0.78rem; color: #ffd980;
    background: rgba(0,0,0,0.3); padding: 0.4rem 0.55rem;
    border-radius: 4px; word-break: break-all;
}
.webhook-empty { font-size: 0.78rem; color: #777; font-style: italic; }
"#;

#[derive(Deserialize, Clone, PartialEq, Debug)]
struct WebhookTokenSummary {
    id: i32,
    label: String,
    token_prefix: String,
    daily_cap: i32,
    daily_sent: i32,
    daily_reset_at: i32,
    last_used_at: Option<i32>,
    created_at: i32,
}

#[derive(Deserialize, Clone, Debug)]
struct CreateTokenResponse {
    token: String,
    label: String,
}

#[derive(Serialize)]
struct CreateTokenRequest {
    label: String,
}

#[function_component(WebhooksPanel)]
pub fn webhooks_panel() -> Html {
    let tokens = use_state(Vec::<WebhookTokenSummary>::new);
    let loading = use_state(|| true);
    let new_label = use_state(String::new);
    let creating = use_state(|| false);
    let just_minted = use_state(|| None::<CreateTokenResponse>);
    let refresh_seq = use_state(|| 0u32);

    // Load tokens on mount and when refresh_seq bumps.
    {
        let tokens = tokens.clone();
        let loading = loading.clone();
        let refresh_seq = refresh_seq.clone();
        use_effect_with_deps(
            move |_| {
                loading.set(true);
                spawn_local(async move {
                    if let Ok(r) = Api::get("/api/me/webhook-tokens").send().await {
                        if r.ok() {
                            if let Ok(list) = r.json::<Vec<WebhookTokenSummary>>().await {
                                tokens.set(list);
                            }
                        }
                    }
                    loading.set(false);
                });
                || ()
            },
            (*refresh_seq,),
        );
    }

    let on_label_input = {
        let new_label = new_label.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target_dyn_into::<HtmlInputElement>();
            if let Some(input) = target {
                new_label.set(input.value());
            }
        })
    };

    let on_create = {
        let new_label = new_label.clone();
        let creating = creating.clone();
        let just_minted = just_minted.clone();
        let refresh_seq = refresh_seq.clone();
        Callback::from(move |_: MouseEvent| {
            let label = (*new_label).trim().to_string();
            if label.is_empty() || *creating {
                return;
            }
            creating.set(true);
            let body = CreateTokenRequest { label };
            let new_label = new_label.clone();
            let creating = creating.clone();
            let just_minted = just_minted.clone();
            let refresh_seq = refresh_seq.clone();
            spawn_local(async move {
                if let Ok(req) = Api::post("/api/me/webhook-tokens").json(&body) {
                    if let Ok(r) = req.send().await {
                        if r.ok() {
                            if let Ok(resp) = r.json::<CreateTokenResponse>().await {
                                just_minted.set(Some(resp));
                            }
                            new_label.set(String::new());
                            refresh_seq.set(js_sys::Date::now() as u32);
                        }
                    }
                }
                creating.set(false);
            });
        })
    };

    let render_revoke = |id: i32, refresh_seq: UseStateHandle<u32>| -> Callback<MouseEvent> {
        Callback::from(move |_: MouseEvent| {
            let refresh_seq = refresh_seq.clone();
            spawn_local(async move {
                let _ = Api::delete(&format!("/api/me/webhook-tokens/{}", id))
                    .send()
                    .await;
                refresh_seq.set(js_sys::Date::now() as u32);
            });
        })
    };

    html! {
        <>
            <style>{WEBHOOKS_STYLES}</style>
            <div class="webhooks-section">
                <div class="webhooks-help">
                    {"Send a text to your phone from anywhere. POST to "}
                    <code>{"/api/webhook/sms"}</code>
                    {" with header "}
                    <code>{"Authorization: Bearer <token>"}</code>
                    {" and body "}
                    <code>{r#"{"message":"..."}"#}</code>
                    {". Default daily cap: 50 messages."}
                </div>

                {
                    if let Some(minted) = (*just_minted).clone() {
                        let dismiss = {
                            let just_minted = just_minted.clone();
                            Callback::from(move |_: MouseEvent| just_minted.set(None))
                        };
                        html! {
                            <div class="webhook-new-token">
                                <div class="webhook-new-token-warn">
                                    {format!("New token for \"{}\" — copy now, it won't be shown again:", minted.label)}
                                </div>
                                <div class="webhook-new-token-value">{minted.token.clone()}</div>
                                <button class="webhook-revoke" onclick={dismiss}>{"Dismiss"}</button>
                            </div>
                        }
                    } else { html! {} }
                }

                <div class="webhook-create-form">
                    <input
                        type="text"
                        placeholder="Label (e.g. \"deploy alerts\")"
                        value={(*new_label).clone()}
                        oninput={on_label_input}
                    />
                    <button onclick={on_create} disabled={*creating || (*new_label).trim().is_empty()}>
                        { if *creating { "Creating..." } else { "Create" } }
                    </button>
                </div>

                <div class="webhooks-list">
                    {
                        if *loading {
                            html! { <div class="webhook-empty">{"Loading..."}</div> }
                        } else if tokens.is_empty() {
                            html! { <div class="webhook-empty">{"No tokens yet."}</div> }
                        } else {
                            tokens.iter().map(|t| {
                                let revoke_cb = render_revoke(t.id, refresh_seq.clone());
                                html! {
                                    <div class="webhook-row" key={t.id}>
                                        <div class="webhook-meta">
                                            <div class="webhook-label">{ &t.label }</div>
                                            <div class="webhook-sub">
                                                <code>{ format!("{}…", t.token_prefix) }</code>
                                                { format!(" · {}/{} today", t.daily_sent, t.daily_cap) }
                                                {
                                                    if let Some(ts) = t.last_used_at {
                                                        format!(" · last used {}", format_relative(ts))
                                                    } else {
                                                        " · never used".to_string()
                                                    }
                                                }
                                            </div>
                                        </div>
                                        <button class="webhook-revoke" onclick={revoke_cb}>{"Revoke"}</button>
                                    </div>
                                }
                            }).collect::<Html>()
                        }
                    }
                </div>
            </div>
        </>
    }
}

fn format_relative(unix_ts: i32) -> String {
    let now = (js_sys::Date::now() / 1000.0) as i64;
    let diff = now - unix_ts as i64;
    if diff < 60 {
        "just now".to_string()
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86_400 {
        format!("{}h ago", diff / 3600)
    } else {
        format!("{}d ago", diff / 86_400)
    }
}
