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
.webhooks-help-block { margin-top: 0.55rem; }
.webhooks-help pre {
    background: rgba(0,0,0,0.4);
    border: 1px solid rgba(255,255,255,0.08);
    border-radius: 4px;
    padding: 0.55rem 0.7rem;
    margin: 0.4rem 0 0 0;
    font-size: 0.72rem;
    color: #ddd;
    overflow-x: auto;
    white-space: pre;
    line-height: 1.45;
}
.webhook-tag-preview {
    font-size: 0.72rem; color: #888;
    padding: 0 0.1rem;
}
.webhook-tag-preview code {
    background: rgba(255,255,255,0.06); padding: 0.05rem 0.3rem;
    border-radius: 3px; font-size: 0.72rem; color: #cde;
}
.webhook-tag-badge {
    font-family: monospace; font-size: 0.72rem;
    background: rgba(100,180,255,0.08);
    color: #9cf;
    border: 1px solid rgba(100,180,255,0.18);
    padding: 0.05rem 0.35rem;
    border-radius: 3px;
    margin-right: 0.35rem;
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
.webhook-revoke:hover:not(:disabled) { background: rgba(220,50,50,0.25); }
.webhook-revoke:disabled { opacity: 0.4; cursor: wait; }
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
.webhook-error {
    font-size: 0.75rem; color: #f88;
    background: rgba(220,50,50,0.08);
    border: 1px solid rgba(220,50,50,0.2);
    border-radius: 4px; padding: 0.45rem 0.6rem;
}
.webhook-upgrade {
    font-size: 0.78rem; color: #fc8;
    background: rgba(255,200,80,0.06);
    border: 1px solid rgba(255,200,80,0.2);
    border-radius: 4px; padding: 0.5rem 0.6rem;
}
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
    #[allow(dead_code)]
    created_at: i32,
}

#[derive(Deserialize, Clone, Debug)]
struct CreateTokenResponse {
    token: String,
    label: String,
}

#[derive(Deserialize, Clone, Debug)]
struct ErrorBody {
    error: String,
}

#[derive(Serialize)]
struct CreateTokenRequest {
    label: String,
}

#[function_component(WebhooksPanel)]
pub fn webhooks_panel() -> Html {
    let tokens = use_state(Vec::<WebhookTokenSummary>::new);
    let loading = use_state(|| true);
    let load_error = use_state(|| None::<String>);
    let new_label = use_state(String::new);
    let creating = use_state(|| false);
    let create_error = use_state(|| None::<String>);
    let revoke_in_flight = use_state(|| None::<i32>);
    let revoke_error = use_state(|| None::<String>);
    let just_minted = use_state(|| None::<CreateTokenResponse>);
    let needs_subscription = use_state(|| false);
    let refresh_seq = use_state(|| 0u32);

    // Load tokens on mount and when refresh_seq bumps.
    {
        let tokens = tokens.clone();
        let loading = loading.clone();
        let load_error = load_error.clone();
        let refresh_seq = refresh_seq.clone();
        use_effect_with_deps(
            move |_| {
                loading.set(true);
                load_error.set(None);
                spawn_local(async move {
                    match Api::get("/api/me/webhook-tokens").send().await {
                        Ok(r) if r.ok() => match r.json::<Vec<WebhookTokenSummary>>().await {
                            Ok(list) => tokens.set(list),
                            Err(_) => {
                                load_error.set(Some("Could not parse server response.".to_string()))
                            }
                        },
                        Ok(r) => {
                            let status = r.status();
                            let msg = parse_error_body(r)
                                .await
                                .unwrap_or_else(|| format!("Load failed ({}).", status));
                            load_error.set(Some(msg));
                        }
                        Err(_) => load_error.set(Some("Network error loading tokens.".to_string())),
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
        let create_error = create_error.clone();
        Callback::from(move |e: InputEvent| {
            // Typing clears the previous create error so the user
            // doesn't see stale feedback while editing.
            create_error.set(None);
            if let Some(input) = e.target_dyn_into::<HtmlInputElement>() {
                new_label.set(input.value());
            }
        })
    };

    let on_create = {
        let new_label = new_label.clone();
        let creating = creating.clone();
        let create_error = create_error.clone();
        let just_minted = just_minted.clone();
        let needs_subscription = needs_subscription.clone();
        let refresh_seq = refresh_seq.clone();
        Callback::from(move |_: MouseEvent| {
            let label = (*new_label).trim().to_string();
            if label.is_empty() || *creating {
                return;
            }
            creating.set(true);
            create_error.set(None);
            let body = CreateTokenRequest { label };
            let new_label = new_label.clone();
            let creating = creating.clone();
            let create_error = create_error.clone();
            let just_minted = just_minted.clone();
            let needs_subscription = needs_subscription.clone();
            let refresh_seq = refresh_seq.clone();
            spawn_local(async move {
                match Api::post("/api/me/webhook-tokens").json(&body) {
                    Ok(req) => match req.send().await {
                        Ok(r) if r.ok() => {
                            if let Ok(resp) = r.json::<CreateTokenResponse>().await {
                                just_minted.set(Some(resp));
                            }
                            new_label.set(String::new());
                            refresh_seq.set(js_sys::Date::now() as u32);
                        }
                        Ok(r) => {
                            let status = r.status();
                            if status == 403 {
                                needs_subscription.set(true);
                            }
                            let msg = parse_error_body(r)
                                .await
                                .unwrap_or_else(|| format!("Create failed ({}).", status));
                            create_error.set(Some(msg));
                        }
                        Err(_) => create_error.set(Some("Network error.".to_string())),
                    },
                    Err(_) => create_error.set(Some("Could not encode request.".to_string())),
                }
                creating.set(false);
            });
        })
    };

    let render_revoke = |id: i32,
                         refresh_seq: UseStateHandle<u32>,
                         in_flight: UseStateHandle<Option<i32>>,
                         err: UseStateHandle<Option<String>>|
     -> Callback<MouseEvent> {
        Callback::from(move |_: MouseEvent| {
            if in_flight.is_some() {
                return;
            }
            in_flight.set(Some(id));
            err.set(None);
            let refresh_seq = refresh_seq.clone();
            let in_flight = in_flight.clone();
            let err = err.clone();
            spawn_local(async move {
                match Api::delete(&format!("/api/me/webhook-tokens/{}", id))
                    .send()
                    .await
                {
                    Ok(r) if r.ok() => {
                        refresh_seq.set(js_sys::Date::now() as u32);
                    }
                    Ok(r) => {
                        let status = r.status();
                        let msg = parse_error_body(r)
                            .await
                            .unwrap_or_else(|| format!("Revoke failed ({}).", status));
                        err.set(Some(msg));
                    }
                    Err(_) => err.set(Some("Network error revoking token.".to_string())),
                }
                in_flight.set(None);
            });
        })
    };

    html! {
        <>
            <style>{WEBHOOKS_STYLES}</style>
            <div class="webhooks-section">
                <div class="webhooks-help">
                    {"Send a text to your own phone from anywhere — cron jobs, CI alerts, IFTTT, your own scripts. The "}
                    <strong>{"tag"}</strong>
                    {" you give each token becomes the "}
                    <code>{"[tag]"}</code>
                    {" prefix on the SMS so you know which sender it came from."}
                    <pre>{format!(
    "curl -X POST {url} \\
    -H \"Authorization: Bearer <your-token>\" \\
    -H \"Content-Type: application/json\" \\
    -d '{{\"message\":\"deploy finished\"}}'",
                        url = webhook_endpoint_url()
                    )}</pre>
                    <div class="webhooks-help-block">
                        {"Optional: add "}
                        <code>{"-H \"Idempotency-Key: <unique-id>\""}</code>
                        {" to dedupe retried requests within 24h. Daily cap resets at UTC midnight. Successful responses return "}
                        <code>{r#"{"status":"sent","sid":"..."}"#}</code>{"."}
                    </div>
                </div>

                {
                    if *needs_subscription {
                        html! {
                            <div class="webhook-upgrade">
                                {"Webhooks require an active subscription. Subscribe under the Billing tab to enable."}
                            </div>
                        }
                    } else { html! {} }
                }

                {
                    if let Some(err) = (*load_error).clone() {
                        html! { <div class="webhook-error">{ err }</div> }
                    } else { html! {} }
                }

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
                        placeholder="Tag (e.g. \"fazm\", \"deploy\", \"github\")"
                        value={(*new_label).clone()}
                        oninput={on_label_input}
                    />
                    <button onclick={on_create} disabled={*creating || (*new_label).trim().is_empty()}>
                        { if *creating { "Creating..." } else { "Create" } }
                    </button>
                </div>
                <div class="webhook-tag-preview">
                    { "Your SMS will read: " }
                    <code>{ format!("[{}] your message", preview_tag(&(*new_label))) }</code>
                </div>

                {
                    if let Some(err) = (*create_error).clone() {
                        html! { <div class="webhook-error">{ err }</div> }
                    } else { html! {} }
                }

                {
                    if let Some(err) = (*revoke_error).clone() {
                        html! { <div class="webhook-error">{ err }</div> }
                    } else { html! {} }
                }

                <div class="webhooks-list">
                    {
                        if *loading {
                            html! { <div class="webhook-empty">{"Loading..."}</div> }
                        } else if tokens.is_empty() && load_error.is_none() {
                            html! { <div class="webhook-empty">{"No tokens yet."}</div> }
                        } else {
                            tokens.iter().map(|t| {
                                let in_flight = revoke_in_flight.clone();
                                let revoke_cb = render_revoke(
                                    t.id,
                                    refresh_seq.clone(),
                                    revoke_in_flight.clone(),
                                    revoke_error.clone(),
                                );
                                let is_revoking = (*in_flight) == Some(t.id);
                                let displayed_tag = preview_tag(&t.label);
                                html! {
                                    <div class="webhook-row" key={t.id}>
                                        <div class="webhook-meta">
                                            <div class="webhook-label">
                                                <span class="webhook-tag-badge">{ format!("[{}]", displayed_tag) }</span>
                                                { &t.label }
                                            </div>
                                            <div class="webhook-sub">
                                                <code>{ format!("{}…", t.token_prefix) }</code>
                                                { format!(" · {}/{} today", t.daily_sent, t.daily_cap) }
                                                { format!(" · resets {}", format_future(t.daily_reset_at)) }
                                                {
                                                    if let Some(ts) = t.last_used_at {
                                                        format!(" · last used {}", format_relative(ts))
                                                    } else {
                                                        " · never used".to_string()
                                                    }
                                                }
                                            </div>
                                        </div>
                                        <button class="webhook-revoke" onclick={revoke_cb} disabled={is_revoking}>
                                            { if is_revoking { "Revoking..." } else { "Revoke" } }
                                        </button>
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

async fn parse_error_body(resp: gloo_net::http::Response) -> Option<String> {
    resp.json::<ErrorBody>().await.ok().map(|b| b.error)
}

/// Mirror of backend `sanitize_label` (webhook_sms_handlers.rs) so the
/// preview matches exactly what will arrive on the user's phone. Strips
/// control chars and brackets, trims, falls back to "webhook" if empty.
/// Keep these two in sync.
fn preview_tag(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .filter(|c| !c.is_control() && *c != '[' && *c != ']')
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "webhook".to_string()
    } else {
        trimmed.to_string()
    }
}

/// The absolute URL clients should POST to. We deliberately hardcode the
/// production host here rather than using `config::get_backend_url()`
/// because we want the docs to show what the user pastes into their own
/// scripts — not what the WASM bundle happens to be calling from the
/// browser. In dev that means showing `http://localhost:3000` so the
/// developer's curl actually hits their local backend; in release we
/// always show the canonical prod host even if the bundle is loaded
/// from a staging URL.
#[cfg(debug_assertions)]
fn webhook_endpoint_url() -> &'static str {
    "http://localhost:3000/api/webhook/sms"
}

#[cfg(not(debug_assertions))]
fn webhook_endpoint_url() -> &'static str {
    "https://lightfriend.ai/api/webhook/sms"
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

fn format_future(unix_ts: i32) -> String {
    let now = (js_sys::Date::now() / 1000.0) as i64;
    let diff = unix_ts as i64 - now;
    if diff <= 0 {
        "soon".to_string()
    } else if diff < 60 {
        "in <1m".to_string()
    } else if diff < 3600 {
        format!("in {}m", diff / 60)
    } else if diff < 86_400 {
        format!("in {}h", diff / 3600)
    } else {
        format!("in {}d", diff / 86_400)
    }
}
