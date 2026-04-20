use crate::utils::api::Api;
use gloo_timers::future::TimeoutFuture;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use yew::prelude::*;

const CONFIRM_PHRASE: &str = "DELETE MY DATA";
const GITHUB_REPO: &str = "ahtavarasmus/lightfriend";
const PURGE_FILE_PATH: &str = "backend/src/services/data_purge.rs";

#[derive(Deserialize, Clone, PartialEq, Debug)]
struct AttestationMetadata {
    commit_sha: Option<String>,
}

#[derive(Serialize)]
struct PurgeRequest {
    password: String,
}

#[derive(Deserialize)]
struct PurgeKickoffResponse {
    purge_id: String,
}

#[derive(Deserialize, Clone, PartialEq, Debug)]
struct PurgeStep {
    id: String,
    label: String,
    status: String,
    detail: Option<String>,
}

#[derive(Deserialize, Clone, PartialEq, Debug)]
struct PurgeStatus {
    steps: Vec<PurgeStep>,
    complete: bool,
    error: Option<String>,
}

#[derive(Clone, PartialEq)]
enum ModalStage {
    Closed,
    Explain,
    Confirm,
    Running(PurgeStatus),
}

#[function_component(DangerZone)]
pub fn danger_zone() -> Html {
    let commit_sha = use_state(|| None::<String>);
    let stage = use_state(|| ModalStage::Closed);
    let confirm_text = use_state(String::new);
    let password = use_state(String::new);
    let submit_error = use_state(|| None::<String>);
    let submitting = use_state(|| false);

    {
        let commit_sha = commit_sha.clone();
        use_effect_with_deps(
            move |_| {
                spawn_local(async move {
                    if let Ok(resp) = Api::get("/.well-known/lightfriend/attestation").send().await {
                        if resp.ok() {
                            if let Ok(meta) = resp.json::<AttestationMetadata>().await {
                                commit_sha.set(meta.commit_sha);
                            }
                        }
                    }
                });
                || ()
            },
            (),
        );
    }

    let github_url = {
        let sha_ref = (*commit_sha).clone();
        let reference = sha_ref.unwrap_or_else(|| "master".to_string());
        format!(
            "https://github.com/{}/blob/{}/{}",
            GITHUB_REPO, reference, PURGE_FILE_PATH
        )
    };
    let link_label = if commit_sha.is_some() {
        "View source on GitHub (deployed commit)"
    } else {
        "View source on GitHub"
    };

    let open_modal = {
        let stage = stage.clone();
        Callback::from(move |_| stage.set(ModalStage::Explain))
    };
    let close_modal = {
        let stage = stage.clone();
        let confirm_text = confirm_text.clone();
        let password = password.clone();
        let submit_error = submit_error.clone();
        Callback::from(move |_| {
            stage.set(ModalStage::Closed);
            confirm_text.set(String::new());
            password.set(String::new());
            submit_error.set(None);
        })
    };
    let to_confirm = {
        let stage = stage.clone();
        Callback::from(move |_| stage.set(ModalStage::Confirm))
    };
    let on_confirm_change = {
        let confirm_text = confirm_text.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            confirm_text.set(input.value());
        })
    };
    let on_password_change = {
        let password = password.clone();
        Callback::from(move |e: InputEvent| {
            let input: HtmlInputElement = e.target_unchecked_into();
            password.set(input.value());
        })
    };

    let on_submit = {
        let stage = stage.clone();
        let confirm_text = confirm_text.clone();
        let password = password.clone();
        let submit_error = submit_error.clone();
        let submitting = submitting.clone();
        Callback::from(move |_| {
            if (*confirm_text).trim() != CONFIRM_PHRASE {
                submit_error.set(Some(format!(
                    "Type exactly {} to confirm.",
                    CONFIRM_PHRASE
                )));
                return;
            }
            if (*password).is_empty() {
                submit_error.set(Some("Password required.".to_string()));
                return;
            }
            submit_error.set(None);
            submitting.set(true);

            let stage = stage.clone();
            let submit_error = submit_error.clone();
            let submitting = submitting.clone();
            let pw = (*password).clone();

            spawn_local(async move {
                let body = PurgeRequest { password: pw };
                let post = match Api::post("/api/profile/purge-data").json(&body) {
                    Ok(r) => r,
                    Err(e) => {
                        submit_error.set(Some(format!("Request build failed: {:?}", e)));
                        submitting.set(false);
                        return;
                    }
                };
                let resp = match post.send().await {
                    Ok(r) => r,
                    Err(e) => {
                        submit_error.set(Some(format!("Network error: {:?}", e)));
                        submitting.set(false);
                        return;
                    }
                };
                if !resp.ok() {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    submit_error.set(Some(if status == 401 {
                        "Incorrect password.".to_string()
                    } else {
                        format!("Server error ({}): {}", status, body)
                    }));
                    submitting.set(false);
                    return;
                }
                let kickoff: PurgeKickoffResponse = match resp.json().await {
                    Ok(k) => k,
                    Err(e) => {
                        submit_error.set(Some(format!("Bad response: {:?}", e)));
                        submitting.set(false);
                        return;
                    }
                };

                stage.set(ModalStage::Running(PurgeStatus {
                    steps: Vec::new(),
                    complete: false,
                    error: None,
                }));

                let url = format!("/api/profile/purge-data/status/{}", kickoff.purge_id);
                loop {
                    TimeoutFuture::new(500).await;
                    let poll_resp = match Api::get(&url).send().await {
                        Ok(r) => r,
                        Err(_) => continue,
                    };
                    if !poll_resp.ok() {
                        break;
                    }
                    let status: PurgeStatus = match poll_resp.json().await {
                        Ok(s) => s,
                        Err(_) => continue,
                    };
                    let done = status.complete;
                    stage.set(ModalStage::Running(status));
                    if done {
                        break;
                    }
                }
                submitting.set(false);
            });
        })
    };

    let reload = Callback::from(|_| {
        if let Some(win) = web_sys::window() {
            let _ = win.location().reload();
        }
    });

    let modal = match (*stage).clone() {
        ModalStage::Closed => html! {},
        ModalStage::Explain => html! {
            <div class="dz-modal-backdrop">
                <div class="dz-modal">
                    <h3>{"Delete my data"}</h3>
                    <p>
                        {"This permanently deletes everything tied to your account on our servers:"}
                    </p>
                    <ul>
                        <li>{"All messages, contacts, events, and rules in your Personal OS"}</li>
                        <li>{"Bridge connections (WhatsApp, Signal, Telegram) — you will be logged out on those services too"}</li>
                        <li>{"Email (IMAP) connections, Tesla, YouTube, MCP servers"}</li>
                        <li>{"Usage logs, notification preferences, location, timezone"}</li>
                        <li>{"Matrix session files on disk"}</li>
                    </ul>
                    <p><strong>{"What stays:"}</strong>{" your login (email, password, 2FA) and your subscription. You can sign in after this and start fresh."}</p>
                    <p class="dz-hint">
                        {"This button exists so you can remove your data if a proposed code change worries you, before the 24-hour deploy delay elapses. You can also use it any time for peace of mind."}
                    </p>
                    <p class="dz-hint">
                        <a href={github_url.clone()} target="_blank" rel="noopener noreferrer">
                            {link_label}
                        </a>
                        {" — read exactly what this button runs."}
                    </p>
                    <p><strong>{"This cannot be undone."}</strong></p>
                    <div class="dz-modal-actions">
                        <button class="dz-btn-secondary" onclick={close_modal.clone()}>{"Cancel"}</button>
                        <button class="dz-btn-danger" onclick={to_confirm}>{"I understand, continue"}</button>
                    </div>
                </div>
            </div>
        },
        ModalStage::Confirm => {
            let typed_ok = (*confirm_text).trim() == CONFIRM_PHRASE;
            let pw_ok = !(*password).is_empty();
            let can_submit = typed_ok && pw_ok && !*submitting;
            html! {
                <div class="dz-modal-backdrop">
                    <div class="dz-modal">
                        <h3>{"Confirm data deletion"}</h3>
                        <p>{"Type "}<code>{CONFIRM_PHRASE}</code>{" to confirm:"}</p>
                        <input
                            type="text"
                            class="dz-input"
                            value={(*confirm_text).clone()}
                            oninput={on_confirm_change}
                            placeholder={CONFIRM_PHRASE}
                        />
                        <p>{"Password:"}</p>
                        <input
                            type="password"
                            class="dz-input"
                            value={(*password).clone()}
                            oninput={on_password_change}
                            placeholder="Your account password"
                        />
                        if let Some(err) = (*submit_error).clone() {
                            <div class="dz-error">{err}</div>
                        }
                        <div class="dz-modal-actions">
                            <button class="dz-btn-secondary" onclick={close_modal} disabled={*submitting}>{"Cancel"}</button>
                            <button
                                class="dz-btn-danger"
                                onclick={on_submit}
                                disabled={!can_submit}
                            >
                                {if *submitting { "Starting..." } else { "Delete my data" }}
                            </button>
                        </div>
                    </div>
                </div>
            }
        }
        ModalStage::Running(status) => {
            let is_done = status.complete;
            let had_error = status.error.is_some();
            html! {
                <div class="dz-modal-backdrop">
                    <div class="dz-modal dz-modal-wide">
                        <h3>{
                            if had_error { "Purge finished with errors" }
                            else if is_done { "Data deleted" }
                            else { "Deleting your data..." }
                        }</h3>
                        <ol class="dz-steps">
                            { for status.steps.iter().map(|step| {
                                let icon = match step.status.as_str() {
                                    "done" => "✓",
                                    "failed" => "✗",
                                    "skipped" => "—",
                                    "running" => "…",
                                    _ => "·",
                                };
                                let class = format!("dz-step dz-step-{}", step.status);
                                html! {
                                    <li class={class}>
                                        <span class="dz-step-icon">{icon}</span>
                                        <span class="dz-step-label">{&step.label}</span>
                                        if let Some(detail) = &step.detail {
                                            <span class="dz-step-detail">{" — "}{detail}</span>
                                        }
                                    </li>
                                }
                            }) }
                        </ol>
                        if let Some(err) = &status.error {
                            <div class="dz-error">{"Error: "}{err}</div>
                        }
                        if is_done {
                            <div class="dz-modal-actions">
                                <button class="dz-btn-secondary" onclick={reload}>{"Close and reload"}</button>
                            </div>
                        }
                    </div>
                </div>
            }
        }
    };

    html! {
        <>
            <div class="danger-zone">
                <h3>{"Danger Zone"}</h3>
                <p class="dz-lead">
                    {"Delete all of your data from Lightfriend's servers. Keeps your login and subscription so you can start fresh afterwards."}
                </p>
                <p class="dz-explain">
                    {"Use this if something about a proposed code change worries you — we plan to introduce a 24-hour delay between proposed deploys and production rollout so you have time to remove your data before new code runs. You can also use this any time for peace of mind."}
                </p>
                <div class="dz-row">
                    <a
                        class="dz-source-link"
                        href={github_url}
                        target="_blank"
                        rel="noopener noreferrer"
                    >
                        {link_label}
                    </a>
                    <button class="dz-btn-danger" onclick={open_modal}>
                        {"Delete my data..."}
                    </button>
                </div>
            </div>
            { modal }
            <style>
                {r#"
.danger-zone {
    margin-top: 2rem;
    padding: 1.5rem;
    border: 1px solid rgba(239, 68, 68, 0.4);
    border-radius: 10px;
    background: rgba(239, 68, 68, 0.04);
}
.danger-zone h3 {
    color: #ef4444;
    margin: 0 0 0.5rem 0;
}
.dz-lead {
    color: rgba(255, 255, 255, 0.85);
    margin: 0 0 0.5rem 0;
}
.dz-explain {
    color: rgba(255, 255, 255, 0.6);
    font-size: 0.9rem;
    margin: 0 0 1rem 0;
}
.dz-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    flex-wrap: wrap;
}
.dz-source-link {
    color: #1e90ff;
    font-size: 0.9rem;
    text-decoration: underline;
}
.dz-btn-danger {
    background: #ef4444;
    color: #fff;
    border: none;
    border-radius: 8px;
    padding: 0.6rem 1rem;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.2s;
}
.dz-btn-danger:hover:not(:disabled) {
    background: #dc2626;
}
.dz-btn-danger:disabled {
    background: rgba(239, 68, 68, 0.4);
    cursor: not-allowed;
}
.dz-btn-secondary {
    background: rgba(255, 255, 255, 0.08);
    color: #fff;
    border: 1px solid rgba(255, 255, 255, 0.2);
    border-radius: 8px;
    padding: 0.6rem 1rem;
    cursor: pointer;
}
.dz-btn-secondary:disabled {
    opacity: 0.5;
    cursor: not-allowed;
}
.dz-modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.65);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 9999;
    padding: 1rem;
}
.dz-modal {
    background: #0f1419;
    border: 1px solid rgba(239, 68, 68, 0.4);
    border-radius: 12px;
    padding: 1.5rem;
    max-width: 520px;
    width: 100%;
    color: #fff;
}
.dz-modal-wide {
    max-width: 640px;
}
.dz-modal h3 {
    margin: 0 0 0.75rem 0;
    color: #ef4444;
}
.dz-modal p {
    margin: 0.5rem 0;
    line-height: 1.5;
}
.dz-modal ul {
    margin: 0.5rem 0 1rem 1.2rem;
    padding: 0;
    color: rgba(255, 255, 255, 0.85);
}
.dz-modal ul li {
    margin: 0.25rem 0;
}
.dz-modal code {
    background: rgba(255, 255, 255, 0.1);
    padding: 0.1rem 0.4rem;
    border-radius: 4px;
    font-family: monospace;
}
.dz-hint {
    color: rgba(255, 255, 255, 0.7);
    font-size: 0.9rem;
}
.dz-hint a {
    color: #1e90ff;
}
.dz-input {
    width: 100%;
    background: rgba(0, 0, 0, 0.3);
    border: 1px solid rgba(255, 255, 255, 0.2);
    border-radius: 8px;
    padding: 0.6rem 0.75rem;
    color: #fff;
    font-size: 1rem;
    margin-bottom: 0.75rem;
    box-sizing: border-box;
}
.dz-input:focus {
    outline: none;
    border-color: #ef4444;
}
.dz-error {
    color: #ef4444;
    background: rgba(239, 68, 68, 0.1);
    border-radius: 6px;
    padding: 0.5rem 0.75rem;
    margin: 0.5rem 0;
    font-size: 0.9rem;
}
.dz-modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 0.75rem;
    margin-top: 1rem;
}
.dz-steps {
    list-style: none;
    padding: 0;
    margin: 0.75rem 0;
}
.dz-step {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
    padding: 0.35rem 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
    font-size: 0.95rem;
}
.dz-step:last-child {
    border-bottom: none;
}
.dz-step-icon {
    font-family: monospace;
    width: 1.2rem;
    flex-shrink: 0;
    color: rgba(255, 255, 255, 0.5);
}
.dz-step-done .dz-step-icon { color: #22c55e; }
.dz-step-failed .dz-step-icon { color: #ef4444; }
.dz-step-running .dz-step-icon { color: #f59e0b; }
.dz-step-label {
    color: #fff;
}
.dz-step-detail {
    color: rgba(255, 255, 255, 0.55);
    font-size: 0.85rem;
}
                "#}
            </style>
        </>
    }
}
