use crate::dashboard::dashboard_view::DashboardView;
use crate::dashboard::settings_panel::{SettingsPanel, SettingsTab};
use crate::pages::landing::Landing;
use crate::profile::billing_models::UserProfile;
use crate::profile::stripe::StripePricingTable;
use crate::utils::api::Api;
use futures::future::{select, Either};
use gloo_timers::future::TimeoutFuture;
use gloo_timers::callback::Timeout;
use serde::Deserialize;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::window;
use yew::prelude::*;
use yew_router::prelude::*;

#[derive(Deserialize)]
struct TotpStatusResponse {
    enabled: bool,
}

#[function_component]
pub fn Home() -> Html {
    let auth_status = use_state(|| None::<bool>); // None = loading, Some(true) = authenticated, Some(false) = not authenticated
    let profile_data = use_state(|| None::<UserProfile>);
    let user_verified = use_state(|| true);
    let error = use_state(|| None::<String>);
    let location = use_location().unwrap();
    let success = use_state(|| None::<String>);
    let post_checkout_success = use_state(|| false);
    let totp_enabled = use_state(|| None::<bool>);
    let settings_open = use_state(|| false);
    let settings_initial_tab = use_state(|| SettingsTab::Account);
    let banner_dismissed = use_state(|| {
        window()
            .and_then(|w| w.local_storage().ok().flatten())
            .and_then(|s| s.get_item("twofa_banner_dismissed").ok().flatten())
            .map(|v| v == "true")
            .unwrap_or(false)
    });

    // Trigger to force profile refresh (increments when subscription changes)
    let refresh_trigger = use_state(|| 0u32);

    // Listen for nav Settings button. Subscribed users also have a dashboard-level
    // listener; this page-level panel is rendered only for unsubscribed users.
    {
        let settings_open = settings_open.clone();
        let settings_initial_tab = settings_initial_tab.clone();
        use_effect_with_deps(
            move |_| {
                let settings_open = settings_open.clone();
                let settings_initial_tab = settings_initial_tab.clone();
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

                let cleanup = callback;
                move || {
                    if let Some(window) = web_sys::window() {
                        let _ = window.remove_event_listener_with_callback(
                            "open-settings",
                            cleanup.as_ref().unchecked_ref(),
                        );
                    }
                }
            },
            (),
        );
    }

    // Check for OAuth success parameters and URL query params
    {
        let success = success.clone();
        let post_checkout_success = post_checkout_success.clone();
        let refresh_trigger = refresh_trigger.clone();
        use_effect_with_deps(
            move |_| {
                let query = location.query_str();
                if let Ok(params) = web_sys::UrlSearchParams::new_with_str(query) {
                    // Check for subscription success
                    if params.get("subscription").as_deref() == Some("success") {
                        post_checkout_success.set(true);
                        success.set(Some("Subscription activated successfully!".to_string()));
                        refresh_trigger.set(*refresh_trigger + 1);
                        clean_url();
                    } else if params.get("subscription").as_deref() == Some("changed") {
                        success.set(Some("Subscription updated successfully!".to_string()));
                        refresh_trigger.set(*refresh_trigger + 1);
                        clean_url();
                    } else if params.get("subscription").as_deref() == Some("canceled") {
                        success.set(Some("Subscription canceled successfully.".to_string()));
                        clean_url();
                    } else if params.has("billing") {
                        clean_url();
                    } else {
                        // OAuth success messages
                        let success_message = if params.get("tesla").as_deref() == Some("success") {
                            Some("Tesla account connected successfully!")
                        } else if params.get("youtube").as_deref() == Some("success") {
                            Some("YouTube connected successfully!")
                        } else {
                            None
                        };

                        if let Some(message) = success_message {
                            success.set(Some(message.to_string()));
                            clean_url();
                        }
                    }
                }
                || ()
            },
            (),
        );
    }

    // Auto-dismiss success banner after 5 seconds
    {
        let success_dismiss = success.clone();
        let success_dep = (*success).clone();
        use_effect_with_deps(
            move |msg: &Option<String>| {
                let timeout = if msg.is_some() {
                    let success = success_dismiss.clone();
                    Some(Timeout::new(5_000, move || {
                        success.set(None);
                    }))
                } else {
                    None
                };
                move || drop(timeout)
            },
            success_dep,
        );
    }

    // Profile fetch effect
    {
        let profile_data = profile_data.clone();
        let user_verified = user_verified.clone();
        let error = error.clone();
        let auth_status = auth_status.clone();
        let refresh_trigger_dep = *refresh_trigger;

        use_effect_with_deps(
            move |_| {
                let profile_data = profile_data.clone();
                let user_verified = user_verified.clone();
                let error = error.clone();
                let auth_status = auth_status.clone();

                spawn_local(async move {
                    let profile_request = Api::get("/api/profile").send();
                    let timeout = TimeoutFuture::new(5_000);
                    let result =
                        match select(Box::pin(profile_request), Box::pin(timeout)).await {
                            Either::Left((result, _)) => result,
                            Either::Right((_, _)) => {
                                gloo_console::log!("Profile fetch timed out; showing public page");
                                auth_status.set(Some(false));
                                error.set(Some("Profile request timed out".to_string()));
                                return;
                            }
                        };
                    match result {
                        Ok(response) => {
                            if !response.ok() {
                                auth_status.set(Some(false));
                                return;
                            }

                            match response.json::<UserProfile>().await {
                                Ok(profile) => {
                                    auth_status.set(Some(true));
                                    user_verified.set(profile.verified);
                                    profile_data.set(Some(profile.clone()));
                                    error.set(None);
                                }
                                Err(e) => {
                                    gloo_console::log!(
                                        "Failed to parse profile data:",
                                        format!("{:?}", e)
                                    );
                                    auth_status.set(Some(false));
                                    error.set(Some("Failed to parse profile data".to_string()));
                                }
                            }
                        }
                        Err(e) => {
                            gloo_console::log!("Failed to fetch profile:", format!("{:?}", e));
                            auth_status.set(Some(false));
                            error.set(Some("Failed to fetch profile".to_string()));
                        }
                    }
                });

                || ()
            },
            refresh_trigger_dep,
        );
    }

    // Fetch TOTP status when authenticated
    {
        let totp_enabled = totp_enabled.clone();
        let auth_status_dep = (*auth_status).clone();
        use_effect_with_deps(
            move |auth: &Option<bool>| {
                if *auth == Some(true) {
                    let totp_enabled = totp_enabled.clone();
                    spawn_local(async move {
                        if let Ok(resp) = Api::get("/api/totp/status").send().await {
                            if resp.ok() {
                                if let Ok(status) = resp.json::<TotpStatusResponse>().await {
                                    totp_enabled.set(Some(status.enabled));
                                }
                            }
                        }
                    });
                }
                || ()
            },
            auth_status_dep,
        );
    }

    // Render based on authentication status
    match *auth_status {
        None => {
            if *post_checkout_success {
                return html! {
                    <PostCheckoutSuccess />
                };
            }

            // Loading - checking authentication
            html! {
                <div style="min-height: 100vh; display: flex; align-items: center; justify-content: center;">
                    <div style="text-align: center;">
                        <div class="loading-spinner-inline"></div>
                    </div>
                </div>
            }
        }
        Some(false) => {
            if *post_checkout_success {
                return html! {
                    <PostCheckoutSuccess />
                };
            }

            // Not authenticated - show landing page
            html! { <Landing /> }
        }
        Some(true) => {
            // Authenticated - show dashboard
            let on_dismiss_banner = {
                let banner_dismissed = banner_dismissed.clone();
                Callback::from(move |_: MouseEvent| {
                    banner_dismissed.set(true);
                    if let Some(w) = window() {
                        if let Ok(Some(storage)) = w.local_storage() {
                            let _ = storage.set_item("twofa_banner_dismissed", "true");
                        }
                    }
                })
            };

            let on_profile_update = {
                let profile_data = profile_data.clone();
                Callback::from(move |updated_profile: UserProfile| {
                    profile_data.set(Some(updated_profile));
                })
            };
            let close_settings = {
                let settings_open = settings_open.clone();
                Callback::from(move |_| settings_open.set(false))
            };

            html! {
                <>
                    <style>
                        {r#"
                            @media (min-width: 769px) {
                                html, body {
                                    overflow: hidden;
                                }
                            }
                            .dashboard-container {
                                max-width: 100%;
                                margin: 0;
                                padding: 0;
                                padding-top: 77px;
                                height: 100vh;
                                height: 100dvh;
                                overflow: hidden;
                            }
                            @media (max-width: 768px) {
                                .dashboard-container {
                                    height: auto;
                                    overflow-y: auto;
                                    overflow-x: hidden;
                                }
                            }
                            .panel-title {
                                display: none;
                            }
                            .twofa-banner {
                                display: flex;
                                align-items: center;
                                justify-content: space-between;
                                background: rgba(30, 144, 255, 0.1);
                                border: 1px solid rgba(30, 144, 255, 0.2);
                                border-radius: 8px;
                                padding: 0.75rem 1rem;
                                margin-bottom: 1rem;
                            }
                            .twofa-banner-content {
                                flex: 1;
                            }
                            .twofa-banner-text {
                                color: rgba(255, 255, 255, 0.8);
                                font-size: 0.9rem;
                            }
                            .twofa-banner-dismiss {
                                background: transparent;
                                border: none;
                                color: rgba(255, 255, 255, 0.5);
                                cursor: pointer;
                                font-size: 1.2rem;
                                padding: 0.25rem 0.5rem;
                            }
                            .twofa-banner-dismiss:hover {
                                color: rgba(255, 255, 255, 0.8);
                            }
                        "#}
                    </style>
                    <div class="dashboard-container">
                        <h1 class="panel-title">{"Dashboard"}</h1>
                        // 2FA banner
                        {
                            if *totp_enabled == Some(false) && !*banner_dismissed {
                                html! {
                                    <div class="twofa-banner">
                                        <div class="twofa-banner-content">
                                            <span class="twofa-banner-text">
                                                {"Secure your account with two-factor authentication. "}
                                                <a style="color: #1E90FF; cursor: pointer;" onclick={Callback::from(|_| {
                                                    // Will be handled by settings panel
                                                })}>{"Set up 2FA"}</a>
                                            </span>
                                        </div>
                                        <button class="twofa-banner-dismiss" onclick={on_dismiss_banner}>
                                            {"x"}
                                        </button>
                                    </div>
                                }
                            } else {
                                html! {}
                            }
                        }
                        // Success message
                        {
                            if let Some(success_msg) = (*success).as_ref() {
                                html! {
                                    <div class="message success-message">
                                        <div class="success-content">
                                            <div class="success-text">
                                                {success_msg}
                                            </div>
                                        </div>
                                    </div>
                                }
                            } else {
                                html! {}
                            }
                        }
                        // Main dashboard view
                        {
                            if let Some(profile) = (*profile_data).as_ref() {
                                if profile.sub_tier.is_some() {
                                    html! {
                                        <DashboardView
                                            user_profile={profile.clone()}
                                            on_profile_update={on_profile_update}
                                        />
                                    }
                                } else {
                                    // Non-subscribed user - show the pricing table directly.
                                    html! {
                                        <>
                                            <div class="subscribe-prompt">
                                                <p>{"Subscribe to access your AI assistant"}</p>
                                                <StripePricingTable
                                                    user_id={Some(profile.id)}
                                                    customer_email={Some(profile.email.clone())}
                                                />
                                            </div>
                                            <SettingsPanel
                                                is_open={*settings_open}
                                                user_profile={Some(profile.clone())}
                                                on_close={close_settings.clone()}
                                                on_profile_update={on_profile_update.clone()}
                                                initial_tab={*settings_initial_tab}
                                            />
                                        </>
                                    }
                                }
                            } else {
                                html! {
                                    <div class="loading-profile">{"Loading profile..."}</div>
                                }
                            }
                        }
                    </div>
                </>
            }
        }
    }
}

#[function_component(PostCheckoutSuccess)]
fn post_checkout_success() -> Html {
    html! {
        <div style="min-height: calc(100vh - 77px); display: flex; align-items: center; justify-content: center; padding: 2rem;">
            <div style="max-width: 520px; text-align: center;">
                <h1 style="color: #fff; font-size: 2rem; margin-bottom: 1rem;">{"Payment received"}</h1>
                <p style="color: rgba(255,255,255,0.72); font-size: 1.05rem; line-height: 1.6; margin-bottom: 1rem;">
                    {"Your Lightfriend account is being set up. Check your email for the password setup link."}
                </p>
                <p style="color: rgba(255,255,255,0.54); font-size: 0.95rem; line-height: 1.5; margin-bottom: 1.5rem;">
                    {"If you used an email that already has an account, log in with that account instead."}
                </p>
                <a href="/login" class="nav-login-button" style="display: inline-flex;">
                    {"Log in"}
                </a>
            </div>
        </div>
    }
}

fn clean_url() {
    if let Some(window) = window() {
        if let Ok(history) = window.history() {
            let _ = history.replace_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some("/"));
        }
    }
}
