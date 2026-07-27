use super::phone_device_panel::PhoneDevicePanel;
use super::webhooks_panel::WebhooksPanel;
use crate::auth::connect::Connect;
use crate::profile::billing_credits::BillingPage;
use crate::profile::billing_models::UserProfile;
use crate::profile::settings::SettingsPage;
use wasm_bindgen::JsCast;
use web_sys::MouseEvent;
use yew::prelude::*;

const SETTINGS_STYLES: &str = r#"
.settings-panel-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.7);
    z-index: 1100;
    display: flex;
    justify-content: flex-end;
}
.settings-panel {
    width: 100%;
    max-width: 500px;
    height: 100%;
    background: #1a1a1a;
    overflow-y: auto;
    animation: slideInPanel 240ms ease-out;
}
@keyframes slideInPanel {
    from { transform: translateX(100%); }
    to { transform: translateX(0); }
}
.settings-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1.25rem 1.5rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
    position: sticky;
    top: 0;
    background: #1a1a1a;
    z-index: 10;
}
.settings-header h2 {
    color: #fff;
    font-size: 1.25rem;
    font-weight: 600;
    margin: 0;
}
.settings-header .close-btn {
    min-width: 36px;
    min-height: 36px;
    background: transparent;
    border: none;
    color: #888;
    font-size: 1.5rem;
    cursor: pointer;
    padding: 0.25rem 0.5rem;
    line-height: 1;
}
.settings-header .close-btn:hover {
    color: #fff;
}
.settings-tabs {
    display: flex;
    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
    padding: 0 1rem;
    overflow-x: auto;
    scrollbar-width: thin;
}
.settings-tab {
    flex: 0 0 auto;
    min-height: 44px;
    background: transparent;
    border: none;
    color: #888;
    padding: 0.75rem 1rem;
    font-size: 0.9rem;
    cursor: pointer;
    border-bottom: 2px solid transparent;
    transition: color 180ms ease, border-color 180ms ease;
    white-space: nowrap;
}
.settings-tab:hover {
    color: #ccc;
}
.settings-tab.active {
    color: #1E90FF;
    border-bottom-color: #1E90FF;
}
.settings-body {
    padding: 1.5rem;
}
.settings-content h3 {
    color: #fff;
    font-size: 1.1rem;
    margin: 0 0 0.5rem 0;
}
.settings-hint {
    color: #666;
    font-size: 0.85rem;
    line-height: 1.5;
    margin: 0 0 1.5rem;
}
.settings-connection-prompt {
    color: #ddd;
    font-size: 1rem;
    line-height: 1.5;
    margin: 0 0 1.5rem;
}
.connections-stack {
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
}
.connections-group {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
}
.connections-group-header h4 {
    color: #fff;
    font-size: 0.95rem;
    margin: 0;
}
.connections-group-header p {
    color: #777;
    font-size: 0.78rem;
    line-height: 1.45;
    margin: 0.25rem 0 0;
}
.connections-group .apps-icons-row {
    gap: 1rem;
    margin: 0;
    padding: 0.75rem 0;
}
.connections-group .app-icon {
    min-width: 40px;
    min-height: 40px;
    transition: background 180ms ease, border-color 180ms ease, transform 180ms ease;
}
.connections-group .app-icon:active {
    transform: scale(0.96);
}
.phone-disclosure summary {
    gap: 1rem;
}
.phone-disclosure .connections-disclosure-body {
    padding-left: 0;
    padding-right: 0;
    padding-bottom: 0;
}
.connection-summary-line {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 1rem;
    width: 100%;
}
.connection-summary-status {
    color: #888;
    font-size: 0.76rem;
    font-weight: 400;
    text-align: right;
}
.connections-disclosure {
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.025);
    overflow: hidden;
}
.connections-disclosure summary {
    min-height: 48px;
    padding: 0.75rem 0.9rem;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    color: #ddd;
    cursor: pointer;
    font-size: 0.88rem;
    font-weight: 600;
    list-style: none;
}
.connections-disclosure summary::-webkit-details-marker {
    display: none;
}
.connections-disclosure summary::after {
    content: "+";
    color: #7eb2ff;
    font-size: 1.1rem;
    font-weight: 400;
}
.connections-disclosure[open] summary::after {
    content: "−";
}
.connections-disclosure summary:focus-visible {
    outline: 2px solid rgba(126, 178, 255, 0.7);
    outline-offset: -2px;
}
.connections-disclosure-copy {
    display: block;
    color: #777;
    font-size: 0.72rem;
    font-weight: 400;
    margin-top: 0.15rem;
}
.connections-disclosure-body {
    padding: 0 0.9rem 0.9rem;
    border-top: 1px solid rgba(255, 255, 255, 0.08);
    padding-top: 0.9rem;
}
@media (prefers-reduced-motion: reduce) {
    .settings-panel {
        animation: none;
    }
}
"#;

#[derive(Clone, PartialEq, Copy)]
pub enum SettingsTab {
    Connections,
    Account,
    Billing,
}

#[derive(Properties, PartialEq, Clone)]
pub struct SettingsPanelProps {
    pub is_open: bool,
    pub user_profile: Option<UserProfile>,
    pub on_close: Callback<()>,
    pub on_profile_update: Callback<UserProfile>,
    #[prop_or(SettingsTab::Connections)]
    pub initial_tab: SettingsTab,
}

#[function_component(SettingsPanel)]
pub fn settings_panel(props: &SettingsPanelProps) -> Html {
    let active_tab = use_state(|| props.initial_tab);

    // Update active tab when initial_tab prop changes (e.g., from URL param)
    {
        let active_tab = active_tab.clone();
        let initial_tab = props.initial_tab;
        use_effect_with_deps(
            move |tab| {
                active_tab.set(*tab);
                || ()
            },
            initial_tab,
        );
    }

    // Escape key to close panel
    {
        let on_close = props.on_close.clone();
        let is_open = props.is_open;
        use_effect_with_deps(
            move |is_open: &bool| {
                let closure_holder: std::rc::Rc<
                    std::cell::RefCell<
                        Option<wasm_bindgen::closure::Closure<dyn Fn(web_sys::KeyboardEvent)>>,
                    >,
                > = std::rc::Rc::new(std::cell::RefCell::new(None));
                if *is_open {
                    let on_close = on_close.clone();
                    let closure =
                        wasm_bindgen::closure::Closure::<dyn Fn(web_sys::KeyboardEvent)>::new(
                            move |e: web_sys::KeyboardEvent| {
                                if e.key() == "Escape" {
                                    on_close.emit(());
                                }
                            },
                        );
                    if let Some(document) = web_sys::window().and_then(|w| w.document()) {
                        let _ = document.add_event_listener_with_callback(
                            "keydown",
                            closure.as_ref().unchecked_ref(),
                        );
                    }
                    *closure_holder.borrow_mut() = Some(closure);
                }
                let holder = closure_holder;
                move || {
                    if let Some(closure) = holder.borrow_mut().take() {
                        if let Some(document) = web_sys::window().and_then(|w| w.document()) {
                            let _ = document.remove_event_listener_with_callback(
                                "keydown",
                                closure.as_ref().unchecked_ref(),
                            );
                        }
                    }
                }
            },
            is_open,
        );
    }

    if !props.is_open {
        return html! {};
    }

    let panel_title = match *active_tab {
        SettingsTab::Connections => "Connections",
        SettingsTab::Billing => "Billing",
        SettingsTab::Account => "Settings",
    };

    let tab_content = match *active_tab {
        SettingsTab::Connections => {
            if let Some(profile) = props.user_profile.as_ref() {
                html! {
                    <div class="settings-content">
                        <p class="settings-connection-prompt">{"Choose what Lightfriend can watch."}</p>
                        <div class="connections-stack">
                            <section class="connections-group" aria-label="Apps">
                                <Connect
                                    user_id={profile.id}
                                    sub_tier={profile.sub_tier.clone()}
                                    phone_number={profile.phone_number.clone()}
                                    estimated_monitoring_cost={profile.estimated_monitoring_cost.clone()}
                                />
                            </section>

                            <section class="connections-group" aria-label="Phone and device">
                                <details class="connections-disclosure phone-disclosure">
                                    <summary>
                                        <span class="connection-summary-line">
                                            <span>{"Phone & device"}</span>
                                            <span class="connection-summary-status">
                                                if profile.phone_number.trim().is_empty() {
                                                    {"Not set up"}
                                                } else {
                                                    {format!("Connected · {}", profile.phone_number)}
                                                }
                                            </span>
                                        </span>
                                    </summary>
                                    <div class="connections-disclosure-body">
                                        <PhoneDevicePanel
                                            user_profile={profile.clone()}
                                            on_profile_update={props.on_profile_update.clone()}
                                        />
                                    </div>
                                </details>
                            </section>

                            <section class="connections-group" aria-label="Webhooks and API">
                                <details class="connections-disclosure">
                                    <summary>
                                        <span>
                                            {"Webhooks & API"}
                                            <span class="connections-disclosure-copy">
                                                {"Send Lightfriend messages from scripts, services, and external assistants."}
                                            </span>
                                        </span>
                                    </summary>
                                    <div class="connections-disclosure-body">
                                        <WebhooksPanel />
                                    </div>
                                </details>
                            </section>
                        </div>
                    </div>
                }
            } else {
                html! { <div class="settings-content"><div class="loading-spinner-inline"></div></div> }
            }
        }
        SettingsTab::Account => {
            if let Some(profile) = props.user_profile.as_ref() {
                let on_profile_update = props.on_profile_update.clone();
                html! {
                    <div class="settings-content">
                        <h3>{"Account"}</h3>
                        <SettingsPage
                            user_profile={profile.clone()}
                            on_profile_update={on_profile_update}
                        />
                    </div>
                }
            } else {
                html! { <div class="settings-content"><div class="loading-spinner-inline"></div></div> }
            }
        }
        SettingsTab::Billing => {
            if let Some(profile) = props.user_profile.as_ref() {
                html! {
                    <div class="settings-content">
                        <BillingPage user_profile={profile.clone()} />
                    </div>
                }
            } else {
                html! { <div class="settings-content"><div class="loading-spinner-inline"></div></div> }
            }
        }
    };

    let overlay_click = {
        let on_close = props.on_close.clone();
        Callback::from(move |_: MouseEvent| {
            on_close.emit(());
        })
    };

    let stop_propagation = Callback::from(|e: MouseEvent| {
        e.stop_propagation();
    });

    html! {
        <>
            <style>{SETTINGS_STYLES}</style>
            <div class="settings-panel-overlay" onclick={overlay_click} role="dialog" aria-modal="true" aria-label="Settings">
                <div class="settings-panel" onclick={stop_propagation}>
                <div class="settings-header">
                    <h2>{panel_title}</h2>
                    <button
                        class="close-btn"
                        aria-label="Close settings"
                        onclick={{
                            let cb = props.on_close.clone();
                            Callback::from(move |_| cb.emit(()))
                        }}
                    >
                        {"x"}
                    </button>
                </div>
                <div class="settings-tabs" role="tablist" aria-label="Settings sections">
                    <button
                        class={classes!("settings-tab", (*active_tab == SettingsTab::Connections).then(|| "active"))}
                        role="tab"
                        aria-selected={(*active_tab == SettingsTab::Connections).to_string()}
                        onclick={{
                            let active_tab = active_tab.clone();
                            Callback::from(move |_| active_tab.set(SettingsTab::Connections))
                        }}
                    >
                        {"Connections"}
                    </button>
                    <button
                        class={classes!("settings-tab", (*active_tab == SettingsTab::Account).then(|| "active"))}
                        role="tab"
                        aria-selected={(*active_tab == SettingsTab::Account).to_string()}
                        onclick={{
                            let active_tab = active_tab.clone();
                            Callback::from(move |_| active_tab.set(SettingsTab::Account))
                        }}
                    >
                        {"Account"}
                    </button>
                    <button
                        class={classes!("settings-tab", (*active_tab == SettingsTab::Billing).then(|| "active"))}
                        role="tab"
                        aria-selected={(*active_tab == SettingsTab::Billing).to_string()}
                        onclick={{
                            let active_tab = active_tab.clone();
                            Callback::from(move |_| active_tab.set(SettingsTab::Billing))
                        }}
                    >
                        {"Billing"}
                    </button>
                </div>
                <div class="settings-body">
                    {tab_content}
                </div>
                </div>
            </div>
        </>
    }
}
