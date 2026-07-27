use crate::profile::billing_models::UserProfile;
use crate::utils::api::Api;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use super::activity_feed::ActivityFeed;
use super::settings_panel::{SettingsPanel, SettingsTab};

const FOCUSED_DASHBOARD_STYLES: &str = r#"
.focused-dashboard {
    height: 100%;
    overflow-y: auto;
    color: #f5f5f5;
}
.focused-dashboard-shell {
    width: min(100%, 920px);
    min-height: 100%;
    margin: 0 auto;
    padding: clamp(2.5rem, 7vw, 5.5rem) 1.5rem 3rem;
    box-sizing: border-box;
}
.focused-status {
    padding-bottom: clamp(2rem, 5vw, 3.5rem);
}
.focused-status-label {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    margin-bottom: 1.1rem;
    color: #8c8c8c;
    font-size: 0.76rem;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
}
.focused-status-dot {
    width: 0.55rem;
    height: 0.55rem;
    border-radius: 50%;
    background: #4ade80;
    box-shadow: 0 0 0 0.25rem rgba(74, 222, 128, 0.1);
}
.focused-status-dot.waiting {
    background: #888;
    box-shadow: 0 0 0 0.25rem rgba(136, 136, 136, 0.1);
}
.focused-status-dot.unavailable {
    background: #f59e0b;
    box-shadow: 0 0 0 0.25rem rgba(245, 158, 11, 0.1);
}
.focused-status h1 {
    max-width: 720px;
    margin: 0;
    color: #fff;
    font-size: clamp(2rem, 5vw, 3.35rem);
    font-weight: 520;
    letter-spacing: -0.045em;
    line-height: 1.04;
    text-wrap: balance;
}
.focused-status-copy {
    max-width: 660px;
    margin: 1.15rem 0 0;
    color: #9d9d9d;
    font-size: clamp(1rem, 2vw, 1.16rem);
    line-height: 1.65;
    text-wrap: pretty;
}
.focused-number {
    margin-top: 1.35rem;
    color: #777;
    font-size: 0.82rem;
    line-height: 1.7;
}
.focused-number a {
    color: #aaa;
    text-decoration-color: rgba(255, 255, 255, 0.2);
    text-underline-offset: 0.22rem;
}
.focused-primary-action {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-height: 44px;
    margin-top: 1.5rem;
    padding: 0.65rem 1.05rem;
    border: 1px solid rgba(126, 178, 255, 0.35);
    border-radius: 8px;
    background: rgba(126, 178, 255, 0.1);
    color: #9ec5ff;
    font: inherit;
    font-size: 0.88rem;
    cursor: pointer;
    transition: background 160ms ease, border-color 160ms ease, transform 160ms ease;
}
.focused-primary-action:hover {
    border-color: rgba(126, 178, 255, 0.55);
    background: rgba(126, 178, 255, 0.16);
}
.focused-primary-action:active {
    transform: scale(0.98);
}
.focused-history {
    min-height: 440px;
    overflow: hidden;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
}
.focused-history .activity-feed-list {
    padding-left: 0;
    padding-right: 0;
}
.focused-history-back {
    display: inline-flex;
    align-items: center;
    min-height: 40px;
    margin: 0 0 1.25rem;
    padding: 0;
    border: 0;
    background: transparent;
    color: #8ebcff;
    font: inherit;
    font-size: 0.8rem;
    cursor: pointer;
    transition: color 160ms ease, transform 160ms ease;
}
.focused-history-back:hover {
    color: #b2d0ff;
}
.focused-history-back:active {
    transform: scale(0.98);
}
.focused-history-back:focus-visible {
    outline: 2px solid rgba(126, 178, 255, 0.7);
    outline-offset: 3px;
}
.focused-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem 2rem;
    flex-wrap: wrap;
    margin-top: clamp(2rem, 7vw, 5rem);
    padding-top: 1.25rem;
    border-top: 1px solid rgba(255, 255, 255, 0.07);
}
.focused-footer-nav {
    display: flex;
    align-items: center;
    gap: 0.25rem 1rem;
    flex-wrap: wrap;
}
.focused-footer button,
.focused-footer a {
    display: inline-flex;
    align-items: center;
    min-height: 32px;
    padding: 0;
    border: 0;
    background: transparent;
    color: #686868;
    font: inherit;
    font-size: 0.74rem;
    text-decoration: none;
    cursor: pointer;
    transition: color 160ms ease, transform 160ms ease;
}
.focused-footer button:hover,
.focused-footer a:hover {
    color: #aaa;
}
.focused-footer button:active,
.focused-footer a:active {
    transform: scale(0.98);
}
.focused-footer button:focus-visible,
.focused-footer a:focus-visible {
    outline: 2px solid rgba(126, 178, 255, 0.7);
    outline-offset: 3px;
}
@media (max-width: 768px) {
    .focused-dashboard {
        height: auto;
        min-height: calc(100dvh - 77px);
    }
    .focused-dashboard-shell {
        padding-left: 1rem;
        padding-right: 1rem;
    }
    .focused-history {
        min-height: 50vh;
    }
    .focused-footer {
        align-items: flex-start;
        flex-direction: column;
    }
}
@media (prefers-color-scheme: light) {
    .focused-dashboard {
        color: #222;
    }
    .focused-status h1 {
        color: #1d1d1f;
    }
    .focused-status-copy {
        color: #686868;
    }
    .focused-history {
        border-top-color: rgba(0, 0, 0, 0.08);
    }
    .focused-footer button:hover,
    .focused-footer a:hover {
        color: #444;
    }
}
@media (prefers-reduced-motion: reduce) {
    .focused-primary-action,
    .focused-history-back,
    .focused-footer button,
    .focused-footer a {
        transition: none;
    }
}
"#;

#[derive(Clone, Default, PartialEq)]
struct ConnectionState {
    loaded: bool,
    available: bool,
    whatsapp: bool,
    signal: bool,
    telegram: bool,
    email: bool,
}

impl ConnectionState {
    fn names(&self) -> Vec<&'static str> {
        let mut names = Vec::new();
        if self.whatsapp {
            names.push("WhatsApp");
        }
        if self.signal {
            names.push("Signal");
        }
        if self.telegram {
            names.push("Telegram");
        }
        if self.email {
            names.push("Email");
        }
        names
    }
}

#[derive(Properties, PartialEq, Clone)]
pub struct FocusedDashboardViewProps {
    pub user_profile: UserProfile,
    pub on_profile_update: Callback<UserProfile>,
}

#[function_component(FocusedDashboardView)]
pub fn focused_dashboard_view(props: &FocusedDashboardViewProps) -> Html {
    let connections = use_state(ConnectionState::default);
    let settings_open = use_state(|| false);
    let settings_initial_tab = use_state(|| SettingsTab::Connections);
    let show_history = use_state(|| false);

    {
        let connections = connections.clone();
        use_effect_with_deps(
            move |_| {
                spawn_local(async move {
                    let mut next = ConnectionState {
                        loaded: true,
                        ..ConnectionState::default()
                    };

                    for bridge in ["whatsapp", "signal", "telegram"] {
                        let url = format!("/api/auth/{}/status", bridge);
                        if let Ok(response) = Api::get(&url).send().await {
                            if response.ok() {
                                next.available = true;
                                if let Ok(data) = response.json::<serde_json::Value>().await {
                                    let connected = data
                                        .get("connected")
                                        .and_then(|value| value.as_bool())
                                        .unwrap_or(false);
                                    match bridge {
                                        "whatsapp" => next.whatsapp = connected,
                                        "signal" => next.signal = connected,
                                        "telegram" => next.telegram = connected,
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }

                    if let Ok(response) = Api::get("/api/auth/imap/status").send().await {
                        if response.ok() {
                            next.available = true;
                            if let Ok(data) = response.json::<serde_json::Value>().await {
                                next.email = data
                                    .get("connected")
                                    .and_then(|value| value.as_bool())
                                    .unwrap_or(false);
                            }
                        }
                    }

                    connections.set(next);
                });
                || ()
            },
            (),
        );
    }

    {
        let settings_open = settings_open.clone();
        let settings_initial_tab = settings_initial_tab.clone();
        use_effect_with_deps(
            move |_| {
                let callback = wasm_bindgen::closure::Closure::wrap(Box::new(move || {
                    settings_initial_tab.set(SettingsTab::Account);
                    settings_open.set(true);
                })
                    as Box<dyn FnMut()>);

                if let Some(window) = web_sys::window() {
                    let _ = window.add_event_listener_with_callback(
                        "open-settings",
                        callback.as_ref().unchecked_ref(),
                    );
                }

                move || {
                    if let Some(window) = web_sys::window() {
                        let _ = window.remove_event_listener_with_callback(
                            "open-settings",
                            callback.as_ref().unchecked_ref(),
                        );
                    }
                }
            },
            (),
        );
    }

    let open_connections = {
        let settings_open = settings_open.clone();
        let settings_initial_tab = settings_initial_tab.clone();
        Callback::from(move |_: MouseEvent| {
            settings_initial_tab.set(SettingsTab::Connections);
            settings_open.set(true);
        })
    };

    let close_settings = {
        let settings_open = settings_open.clone();
        Callback::from(move |_| settings_open.set(false))
    };

    let open_account_settings = {
        let settings_open = settings_open.clone();
        let settings_initial_tab = settings_initial_tab.clone();
        Callback::from(move |_: MouseEvent| {
            settings_initial_tab.set(SettingsTab::Account);
            settings_open.set(true);
        })
    };

    let toggle_history = {
        let show_history = show_history.clone();
        Callback::from(move |_: MouseEvent| show_history.set(!*show_history))
    };

    let names = connections.names();
    let has_connections = !names.is_empty();
    let source_summary = names.join(", ");

    let (status_label, status_class, title, description) = if !connections.loaded {
        (
            "Checking status",
            "waiting",
            "Checking what needs your attention.",
            "This should only take a moment.",
        )
    } else if !connections.available {
        (
            "Needs attention",
            "unavailable",
            "Lightfriend's status could not be checked.",
            "Your existing services may still be running. Check again in a moment.",
        )
    } else if has_connections {
        (
            "Working quietly",
            "",
            "Nothing needs your attention.",
            "Lightfriend is working in the background.",
        )
    } else {
        (
            "Setup needed",
            "waiting",
            "Connect the places that matter.",
            "Lightfriend needs at least one connection before it can watch for important messages.",
        )
    };

    html! {
        <>
            <style>{FOCUSED_DASHBOARD_STYLES}</style>
            <main class="focused-dashboard">
                <div class="focused-dashboard-shell">
                    if *show_history {
                        <section class="focused-status">
                            <button
                                type="button"
                                class="focused-history-back"
                                onclick={toggle_history.clone()}
                            >
                                {"← Home"}
                            </button>
                            <div class="focused-status-label">{"History"}</div>
                            <h1>{"What Lightfriend has done."}</h1>
                            <p class="focused-status-copy">
                                {"A record of the messages Lightfriend handled and the decisions it made."}
                            </p>
                        </section>
                        <section class="focused-history" aria-label="Lightfriend history">
                            <ActivityFeed show_header={false} />
                        </section>
                    } else {
                        <section class="focused-status" aria-live="polite">
                            <div class="focused-status-label">
                                <span class={classes!("focused-status-dot", status_class)}></span>
                                {status_label}
                            </div>
                            <h1>{title}</h1>
                            <p class="focused-status-copy">{description}</p>

                            if has_connections {
                                <p class="focused-number">
                                    {format!("Watching {}.", source_summary)}
                                    if let Some(number) = props.user_profile.sms_from_number.as_ref() {
                                        <br />
                                        {"Text or call: "}
                                        <a href={format!("sms:{}", number)}>{number}</a>
                                    }
                                </p>
                            } else if connections.loaded && connections.available {
                                <button class="focused-primary-action" onclick={open_connections}>
                                    {"Connect your first app"}
                                </button>
                            }
                        </section>
                    }

                    <footer class="focused-footer">
                        <nav class="focused-footer-nav" aria-label="Dashboard">
                            if !*show_history {
                                <button type="button" onclick={toggle_history}>{"History"}</button>
                            }
                            <button type="button" onclick={open_account_settings}>{"Settings"}</button>
                        </nav>
                        <nav class="focused-footer-nav" aria-label="Legal and project links">
                            <a href="/privacy">{"Privacy"}</a>
                            <a href="/terms">{"Terms"}</a>
                            <a href="/trust-chain">{"Trust"}</a>
                            <a href="https://github.com/ahtavarasmus/lightfriend" target="_blank" rel="noopener noreferrer">{"Source"}</a>
                        </nav>
                    </footer>
                </div>
            </main>
            <SettingsPanel
                is_open={*settings_open}
                user_profile={Some(props.user_profile.clone())}
                on_close={close_settings}
                on_profile_update={props.on_profile_update.clone()}
                initial_tab={*settings_initial_tab}
            />
        </>
    }
}
