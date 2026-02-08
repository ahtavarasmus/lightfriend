use yew::prelude::*;
use yew_router::prelude::*;
use crate::Route;
use crate::utils::api::Api;
use web_sys::window;
use serde::Deserialize;
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

    // First-time subscriber onboarding overlay
    let show_onboarding = use_state(|| false);

    // Trigger to force profile refresh (incremented when subscription changes)
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
                        } else if params.get("uber").as_deref() == Some("success") {
                            Some("Uber account connected successfully!")
                        } else if params.get("google_calendar").as_deref() == Some("success") {
                            Some("Google Calendar connected successfully!")
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
        let show_onboarding = show_onboarding.clone();
        let refresh_trigger_dep = *refresh_trigger;

        use_effect_with_deps(
            move |_| {
                let profile_data = profile_data.clone();
                let user_verified = user_verified.clone();
                let error = error.clone();
                let auth_status = auth_status.clone();
                let show_onboarding = show_onboarding.clone();

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
                                    // Show onboarding for subscribers who haven't connected any services yet
                                    if profile.sub_tier.is_some() && !profile.has_any_connection {
                                        show_onboarding.set(true);
                                    }
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
                        <p>{"Loading..."}</p>
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

            let on_dismiss_onboarding = {
                let show_onboarding = show_onboarding.clone();
                Callback::from(move |_: MouseEvent| {
                    show_onboarding.set(false);
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
                            .dashboard-container {
                                max-width: 600px;
                                margin: 0 auto;
                                padding: 6rem 1rem 2rem 1rem;
                                min-height: calc(100vh - 80px);
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
                            .dashboard-footer {
                                margin-top: 2rem;
                                padding-top: 1rem;
                                border-top: 1px solid rgba(255, 255, 255, 0.1);
                                text-align: center;
                            }
                            .development-links {
                                color: rgba(255, 255, 255, 0.5);
                                font-size: 0.85rem;
                            }
                            .development-links a {
                                color: #1E90FF;
                                text-decoration: none;
                            }
                            .development-links a:hover {
                                text-decoration: underline;
                            }
                            .legal-links {
                                margin-top: 0.5rem;
                            }
                            .onboarding-overlay {
                                position: fixed;
                                top: 0;
                                left: 0;
                                right: 0;
                                bottom: 0;
                                background: rgba(0, 0, 0, 0.8);
                                display: flex;
                                align-items: center;
                                justify-content: center;
                                z-index: 1000;
                            }
                            .onboarding-modal {
                                background: rgba(30, 30, 30, 0.95);
                                border: 1px solid rgba(30, 144, 255, 0.3);
                                border-radius: 16px;
                                padding: 2rem;
                                max-width: 500px;
                                text-align: center;
                            }
                        "#}
                    </style>
                    // First-time subscriber onboarding overlay
                    {
                        if *show_onboarding {
                            html! {
                                <div class="onboarding-overlay" onclick={on_dismiss_onboarding.clone()}>
                                    <div class="onboarding-modal" onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                                        <h2 style="margin: 0 0 1rem 0; font-size: 1.5rem; background: linear-gradient(45deg, #fff, #7EB2FF); -webkit-background-clip: text; -webkit-text-fill-color: transparent;">
                                            {"Welcome to Lightfriend!"}
                                        </h2>
                                        <p style="color: rgba(255, 255, 255, 0.8); margin-bottom: 1.25rem;">
                                            {"Here's how to get started:"}
                                        </p>
                                        <ul style="list-style: none; padding: 0; margin: 0 0 1.5rem 0; text-align: left;">
                                            <li style="color: rgba(255, 255, 255, 0.7); margin-bottom: 0.75rem; padding-left: 1.5rem; position: relative;">
                                                <span style="position: absolute; left: 0; color: #1E90FF;">{"*"}</span>
                                                <strong style="color: #fff;">{"Connect your services"}</strong>
                                                {" - Link your calendar, email, or messaging apps"}
                                            </li>
                                            <li style="color: rgba(255, 255, 255, 0.7); margin-bottom: 0.75rem; padding-left: 1.5rem; position: relative;">
                                                <span style="position: absolute; left: 0; color: #1E90FF;">{"*"}</span>
                                                <strong style="color: #fff;">{"Ask anything"}</strong>
                                                {" - Use the web chat or send an SMS to your Lightfriend number"}
                                            </li>
                                            <li style="color: rgba(255, 255, 255, 0.7); margin-bottom: 0.75rem; padding-left: 1.5rem; position: relative;">
                                                <span style="position: absolute; left: 0; color: #1E90FF;">{"*"}</span>
                                                <strong style="color: #fff;">{"Explore your tools"}</strong>
                                                {" - Check out the tools available to your assistant"}
                                            </li>
                                            <li style="color: rgba(255, 255, 255, 0.7); padding-left: 1.5rem; position: relative;">
                                                <span style="position: absolute; left: 0; color: #1E90FF;">{"*"}</span>
                                                <strong style="color: #fff;">{"Set up notifications"}</strong>
                                                {" - Configure how you want to be alerted"}
                                            </li>
                                        </ul>
                                        <button
                                            onclick={on_dismiss_onboarding.clone()}
                                            style="background: linear-gradient(135deg, #1E90FF, #4169E1); border: none; color: white; padding: 0.75rem 2rem; border-radius: 8px; font-size: 1rem; cursor: pointer; font-weight: 500; transition: transform 0.2s, box-shadow 0.2s;"
                                        >
                                            {"Got it!"}
                                        </button>
                                    </div>
                                </div>
                            }
                        } else {
                            html! {}
                        }
                    }
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
                        <footer class="dashboard-footer">
                            <div class="development-links">
                                <p>{"Source code on "}
                                    <a href="https://github.com/ahtavarasmus/lightfriend" target="_blank" rel="noopener noreferrer">{"GitHub"}</a>
                                </p>
                                <div class="legal-links">
                                    <a href="/faq">{"FAQ"}</a>
                                    {" | "}
                                    <a href="/blog">{"Blog"}</a>
                                    {" | "}
                                    <a href="/pricing">{"Pricing"}</a>
                                    {" | "}
                                    <a href="/terms">{"Terms"}</a>
                                    {" | "}
                                    <a href="/privacy">{"Privacy"}</a>
                                    {" | "}
                                    <a href="/updates">{"Updates"}</a>
                                </div>
                            </div>
                        </footer>
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
