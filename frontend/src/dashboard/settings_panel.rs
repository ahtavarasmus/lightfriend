use yew::prelude::*;
use web_sys::MouseEvent;
use crate::auth::connect::Connect;
use crate::profile::settings::SettingsPage;
use crate::profile::billing_credits::BillingPage;
use crate::profile::billing_models::UserProfile;
use crate::proactive::contact_profiles::ContactProfilesSection;
use crate::proactive::waiting_checks::TasksSection;

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
    animation: slideInPanel 0.3s ease;
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
}
.settings-tab {
    background: transparent;
    border: none;
    color: #888;
    padding: 0.75rem 1rem;
    font-size: 0.9rem;
    cursor: pointer;
    border-bottom: 2px solid transparent;
    transition: all 0.2s;
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
    margin-bottom: 1.5rem;
}
"#;

#[derive(Clone, PartialEq, Copy)]
pub enum SettingsTab {
    People,
    Tasks,
    Capabilities,
    Account,
    Billing,
}

#[derive(Properties, PartialEq, Clone)]
pub struct SettingsPanelProps {
    pub is_open: bool,
    pub user_profile: Option<UserProfile>,
    pub on_close: Callback<()>,
    pub on_profile_update: Callback<UserProfile>,
    #[prop_or(SettingsTab::People)]
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

    if !props.is_open {
        return html! {};
    }

    let tab_content = match *active_tab {
        SettingsTab::People => html! {
            <div class="settings-content">
                <h3>{"People"}</h3>
                <p class="settings-hint">{"Configure notification settings for contacts you want to monitor"}</p>
                <ContactProfilesSection />
            </div>
        },
        SettingsTab::Tasks => {
            if let Some(profile) = props.user_profile.as_ref() {
                html! {
                    <div class="settings-content">
                        <h3>{"Tasks"}</h3>
                        <p class="settings-hint">{"Scheduled reminders, message monitoring, and recurring digests"}</p>
                        <TasksSection
                            tasks={vec![]}
                            on_change={Callback::from(|_| {})}
                            phone_number={profile.phone_number.clone()}
                            critical_disabled={false}
                        />
                    </div>
                }
            } else {
                html! { <div class="settings-content">{"Loading..."}</div> }
            }
        }
        SettingsTab::Capabilities => {
            if let Some(profile) = props.user_profile.as_ref() {
                html! {
                    <div class="settings-content">
                        <h3>{"Capabilities"}</h3>
                        <Connect
                            user_id={profile.id}
                            sub_tier={profile.sub_tier.clone()}
                            discount={profile.discount}
                            phone_number={profile.phone_number.clone()}
                            estimated_monitoring_cost={profile.estimated_monitoring_cost.clone()}
                        />
                    </div>
                }
            } else {
                html! { <div class="settings-content">{"Loading..."}</div> }
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
                html! { <div class="settings-content">{"Loading..."}</div> }
            }
        }
        SettingsTab::Billing => {
            if let Some(profile) = props.user_profile.as_ref() {
                html! {
                    <div class="settings-content">
                        <h3>{"Billing"}</h3>
                        <BillingPage user_profile={profile.clone()} />
                    </div>
                }
            } else {
                html! { <div class="settings-content">{"Loading..."}</div> }
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
            <div class="settings-panel-overlay" onclick={overlay_click}>
                <div class="settings-panel" onclick={stop_propagation}>
                <div class="settings-header">
                    <h2>{"Settings"}</h2>
                    <button
                        class="close-btn"
                        onclick={{
                            let cb = props.on_close.clone();
                            Callback::from(move |_| cb.emit(()))
                        }}
                    >
                        {"x"}
                    </button>
                </div>
                <div class="settings-tabs">
                    <button
                        class={classes!("settings-tab", (*active_tab == SettingsTab::People).then(|| "active"))}
                        onclick={{
                            let active_tab = active_tab.clone();
                            Callback::from(move |_| active_tab.set(SettingsTab::People))
                        }}
                    >
                        {"People"}
                    </button>
                    <button
                        class={classes!("settings-tab", (*active_tab == SettingsTab::Tasks).then(|| "active"))}
                        onclick={{
                            let active_tab = active_tab.clone();
                            Callback::from(move |_| active_tab.set(SettingsTab::Tasks))
                        }}
                    >
                        {"Tasks"}
                    </button>
                    <button
                        class={classes!("settings-tab", (*active_tab == SettingsTab::Capabilities).then(|| "active"))}
                        onclick={{
                            let active_tab = active_tab.clone();
                            Callback::from(move |_| active_tab.set(SettingsTab::Capabilities))
                        }}
                    >
                        {"Capabilities"}
                    </button>
                    <button
                        class={classes!("settings-tab", (*active_tab == SettingsTab::Account).then(|| "active"))}
                        onclick={{
                            let active_tab = active_tab.clone();
                            Callback::from(move |_| active_tab.set(SettingsTab::Account))
                        }}
                    >
                        {"Account"}
                    </button>
                    <button
                        class={classes!("settings-tab", (*active_tab == SettingsTab::Billing).then(|| "active"))}
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
