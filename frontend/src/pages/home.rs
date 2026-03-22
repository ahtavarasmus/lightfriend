use yew::prelude::*;
use yew_router::prelude::*;
use crate::Route;
use crate::utils::api::Api;
use web_sys::window;
use serde::Deserialize;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use crate::pages::landing::Landing;
use crate::profile::billing_models::UserProfile;
use crate::dashboard::dashboard_view::DashboardView;

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
    let totp_enabled = use_state(|| None::<bool>);
    let banner_dismissed = use_state(|| {
        window()
            .and_then(|w| w.local_storage().ok().flatten())
            .and_then(|s| s.get_item("twofa_banner_dismissed").ok().flatten())
            .map(|v| v == "true")
            .unwrap_or(false)
    });

    // Trigger to force profile refresh (increments when subscription changes)
    let refresh_trigger = use_state(|| 0u32);

    // Check for OAuth success parameters and URL query params
    {
        let success = success.clone();
        let refresh_trigger = refresh_trigger.clone();
        use_effect_with_deps(
            move |_| {
                let query = location.query_str();
                if let Ok(params) = web_sys::UrlSearchParams::new_with_str(query) {
                    // Check for subscription success
                    if params.get("subscription").as_deref() == Some("success") {
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
                    let result = Api::get("/api/profile").send().await;
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
                                    gloo_console::log!("Failed to parse profile data:", format!("{:?}", e));
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
                                    overflow: auto;
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
                                            <span class="success-icon">{"+"}</span>
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
                                    // Non-subscribed user - show subscribe prompt
                                    html! {
                                        <div class="subscribe-prompt">
                                            <p>{"Subscribe to access your AI assistant"}</p>
                                            <yew_router::components::Link<Route> to={Route::Pricing} classes="subscribe-link">
                                                {"View Plans"}
                                            </yew_router::components::Link<Route>>
                                        </div>
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

fn clean_url() {
    if let Some(window) = window() {
        if let Ok(history) = window.history() {
            let _ = history.replace_state_with_url(
                &wasm_bindgen::JsValue::NULL,
                "",
                Some("/")
            );
        }
    }
}
