use yew::prelude::*;
use crate::auth::connect::Connect;
use yew_router::prelude::*;
use crate::Route;
use yew_router::components::Link;
use crate::utils::api::Api;
use crate::config;
use web_sys::{window, HtmlInputElement, UrlSearchParams};
use serde_json::{json, Value};
use serde::Deserialize;
use wasm_bindgen_futures::spawn_local;
use crate::pages::landing::Landing;
use crate::profile::settings::SettingsPage;
use crate::profile::billing_credits::BillingPage;
use crate::profile::billing_models::UserProfile;
use crate::controls::tesla_controls::TeslaControls;
use crate::media::youtube_hub::YouTubeHub;
use crate::components::backup_key_sender::BackupKeySender;

#[derive(Deserialize)]
struct TotpStatusResponse {
    enabled: bool,
}

/// Usage projection response from backend (for simplified status display)
#[derive(Clone, PartialEq, Deserialize)]
struct UsageProjectionResponse {
    plan_type: Option<String>,
    status: String,
    usage_percentage: f32,
    usage_percentage_display: String,
    overage_days_remaining: Option<i32>,
    estimated_monthly_extra_cost: Option<f32>,
    recommendation: Option<UsageRecommendation>,
    overage_credits: f32,
    has_auto_topup: bool,
    days_until_billing: Option<i32>,
    is_example_data: bool,
}

#[derive(Clone, PartialEq, Deserialize)]
struct UsageRecommendation {
    message: String,
    action_type: String,
    action_link: Option<String>,
}

/// BYOT usage response from backend
#[derive(Clone, PartialEq, Deserialize)]
struct ByotUsageResponse {
    total_cost_eur: f32,
    country_code: String,
    country_name: String,
    days_until_billing: Option<i32>,
}

#[derive(Clone, PartialEq)]
enum DashboardTab {
    Connections,
    Controls,
    Media,
    Billing,
    Settings,
}

/// Usage status indicator component - compact inline display
#[derive(Properties, PartialEq, Clone)]
struct UsageStatusIndicatorProps {
    pub data: Option<UsageProjectionResponse>,
    pub loading: bool,
    pub on_details_click: Callback<()>,
}

#[function_component]
fn UsageStatusIndicator(props: &UsageStatusIndicatorProps) -> Html {
    let on_details = props.on_details_click.clone();

    if props.loading {
        return html! {
            <span style="color: #999; font-size: 0.85rem;">{"..."}</span>
        };
    }

    let Some(data) = props.data.clone() else {
        return html! {};
    };

    // Determine status color based on percentage and overage buffer
    let days_buffer = data.overage_days_remaining.unwrap_or(0);
    let status_color = if data.has_auto_topup {
        "#4ade80" // green - auto top-up covers overage
    } else if data.usage_percentage > 100.0 {
        // Over quota - check overage credits buffer
        if days_buffer > 90 {
            "#4ade80" // green - credits cover 3+ months
        } else if days_buffer > 60 {
            "#fbbf24" // yellow - credits cover 2-3 months
        } else {
            "#f87171" // red - credits cover less than 2 months
        }
    } else if data.usage_percentage > 95.0 {
        "#fbbf24" // yellow - warning
    } else {
        "#4ade80" // green - on track
    };

    // Format percentage - change "used" to "projected"
    let percentage = data.usage_percentage_display.replace("used", "projected");

    // Secondary info based on percentage (warn at 95%+)
    let secondary_info = if data.usage_percentage > 95.0 {
        if data.has_auto_topup {
            data.estimated_monthly_extra_cost.map(|cost| format!("~{:.0}EUR extra/mo", cost))
        } else if data.overage_credits > 0.0 {
            data.overage_days_remaining.map(|days| {
                if days <= 1 { "credits low".to_string() } else { format!("~{} days buffer", days) }
            })
        } else {
            None
        }
    } else if data.overage_credits > 0.0 {
        Some(format!("{:.2}EUR buffer", data.overage_credits))
    } else {
        None
    };

    html! {
        <span style="display: inline-flex; align-items: center; flex-wrap: wrap; gap: 0.4rem;">
            <span style={format!("color: {}; font-weight: 600;", status_color)}>
                {
                    if data.has_auto_topup {
                        "OK" // auto top-up covers any overage
                    } else if data.usage_percentage > 100.0 {
                        // Over quota - check overage credits buffer
                        if days_buffer > 90 {
                            "OK" // credits cover 3+ months
                        } else if days_buffer > 60 {
                            "!" // credits cover 2-3 months
                        } else {
                            "!!" // credits cover less than 2 months
                        }
                    } else if data.usage_percentage > 95.0 {
                        "!"
                    } else {
                        "OK"
                    }
                }
            </span>
            <span style="color: #aaa; font-size: 0.85rem;">{percentage}</span>
            {
                if let Some(ref info) = secondary_info {
                    html! {
                        <span style="color: #777; font-size: 0.8rem;">{format!("({})", info)}</span>
                    }
                } else {
                    html! {}
                }
            }
            {
                if data.is_example_data {
                    html! {
                        <span style="color: #666; font-size: 0.75rem; font-style: italic;">{"est."}</span>
                    }
                } else {
                    html! {}
                }
            }
            {
                if let Some(ref rec) = data.recommendation {
                    html! {
                        <span style="color: #7EB2FF; font-size: 0.8rem;">
                            {format!("- {}", match rec.action_type.as_str() {
                                "reduce_digests" => "reduce digests",
                                "upgrade_plan" => "upgrade plan",
                                "enable_topup" => "enable top-up",
                                _ => "see billing",
                            })}
                        </span>
                    }
                } else {
                    html! {}
                }
            }
            <a href="#" onclick={Callback::from(move |e: MouseEvent| {
                e.prevent_default();
                on_details.emit(());
            })} style="color: #7EB2FF; text-decoration: none; font-size: 0.8rem;">
                {"details"}
            </a>
        </span>
    }
}

/// BYOT usage status indicator component - shows estimated Twilio costs
#[derive(Properties, PartialEq, Clone)]
struct ByotUsageStatusIndicatorProps {
    pub data: Option<ByotUsageResponse>,
    pub loading: bool,
    pub on_details_click: Callback<()>,
}

#[function_component]
fn ByotUsageStatusIndicator(props: &ByotUsageStatusIndicatorProps) -> Html {
    let on_details = props.on_details_click.clone();

    if props.loading {
        return html! {
            <span style="color: #999; font-size: 0.85rem;">{"..."}</span>
        };
    }

    let Some(data) = props.data.clone() else {
        return html! {};
    };

    // Determine usage level based on cost
    let (status_color, status_label) = if data.total_cost_eur < 5.0 {
        ("#4ade80", "Low usage") // green
    } else if data.total_cost_eur < 15.0 {
        ("#fbbf24", "Moderate") // yellow
    } else {
        ("#f87171", "High usage") // red/orange
    };

    html! {
        <span style="display: inline-flex; align-items: center; flex-wrap: wrap; gap: 0.4rem;">
            <span style={format!("color: {}; font-weight: 600; font-size: 0.9rem;", status_color)}>
                {format!("{:.2}EUR", data.total_cost_eur)}
            </span>
            <span style="color: #aaa; font-size: 0.85rem;">{"this month"}</span>
            <span style="color: #777; font-size: 0.8rem;">{format!("({})", status_label)}</span>
            <a href="#" onclick={Callback::from(move |e: MouseEvent| {
                e.prevent_default();
                on_details.emit(());
            })} style="color: #7EB2FF; text-decoration: none; font-size: 0.8rem;">
                {"details"}
            </a>
        </span>
    }
}

#[function_component]
pub fn Home() -> Html {
    let auth_status = use_state(|| None::<bool>); // None = loading, Some(true) = authenticated, Some(false) = not authenticated
    let profile_data = use_state(|| None::<UserProfile>);
    let user_verified = use_state(|| true);
    let error = use_state(|| None::<String>);
    let _is_expanded = use_state(|| false);
    let active_tab = use_state(|| DashboardTab::Connections);
    let _navigator = use_navigator().unwrap();
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

    // Usage status data - lifted to Home to prevent re-fetching on tab change
    let usage_data = use_state(|| None::<UsageProjectionResponse>);
    let usage_loading = use_state(|| true);

    // BYOT usage data - for users with their own Twilio number
    let byot_usage_data = use_state(|| None::<ByotUsageResponse>);
    let byot_usage_loading = use_state(|| true);

    // Web chat state - only stores the most recent exchange (user msg, bot reply)
    let chat_user_msg = use_state(|| None::<String>);
    let chat_bot_reply = use_state(|| None::<String>);
    let chat_input = use_state(|| String::new());
    let chat_loading = use_state(|| false);
    let chat_error = use_state(|| None::<String>);
    let chat_input_ref = use_node_ref();
    let has_focused_chat = use_state(|| false);
    // Image attachment state for web chat
    let chat_image: UseStateHandle<Option<web_sys::File>> = use_state(|| None);
    let chat_image_preview: UseStateHandle<Option<String>> = use_state(|| None);
    let chat_file_input_ref = use_node_ref();

    // Auto-focus chat input when profile loads (input is conditionally rendered)
    // Only focus once on initial load, not on subsequent profile updates
    {
        let chat_input_ref = chat_input_ref.clone();
        let profile_data = profile_data.clone();
        let has_focused_chat_inner = has_focused_chat.clone();
        let has_focused_chat_dep = has_focused_chat.clone();
        use_effect_with_deps(move |(profile, already_focused)| {
            // Only focus when profile is loaded, has subscription, and we haven't focused yet
            if !**already_focused {
                if let Some(p) = profile.as_ref() {
                    if p.sub_tier.is_some() {
                        // Small delay to ensure DOM is updated
                        let chat_input_ref = chat_input_ref.clone();
                        let has_focused_chat = has_focused_chat_inner.clone();
                        gloo_timers::callback::Timeout::new(100, move || {
                            if let Some(input) = chat_input_ref.cast::<HtmlInputElement>() {
                                let _ = input.focus();
                                has_focused_chat.set(true);
                            }
                        }).forget();
                    }
                }
            }
            || ()
        }, (profile_data, has_focused_chat_dep));
    }

    // Web call state
    let call_active = use_state(|| false);
    let call_connecting = use_state(|| false);
    let call_duration = use_state(|| 0i32);
    let call_error = use_state(|| None::<String>);
    let call_cost_per_min = use_state(|| 0.15f32);

    // Update call duration every second when call is active
    {
        let call_active = call_active.clone();
        let call_duration = call_duration.clone();
        use_effect_with_deps(move |is_active| {
            let interval_handle: Option<gloo_timers::callback::Interval> = if **is_active {
                Some(gloo_timers::callback::Interval::new(1000, move || {
                    let duration = crate::utils::elevenlabs_web::get_elevenlabs_call_duration();
                    call_duration.set(duration);
                }))
            } else {
                None
            };
            move || {
                drop(interval_handle);
            }
        }, call_active);
    }

    // YouTube connection state
    let youtube_connected = use_state(|| false);
    let youtube_can_subscribe = use_state(|| false);

    // Fetch YouTube connection status
    {
        let youtube_connected = youtube_connected.clone();
        let youtube_can_subscribe = youtube_can_subscribe.clone();
        use_effect_with_deps(move |_| {
            let youtube_connected = youtube_connected.clone();
            let youtube_can_subscribe = youtube_can_subscribe.clone();
            spawn_local(async move {
                if let Ok(response) = Api::get("/api/auth/youtube/status").send().await {
                    if response.ok() {
                        if let Ok(data) = response.json::<serde_json::Value>().await {
                            if let Some(connected) = data.get("connected").and_then(|v| v.as_bool()) {
                                youtube_connected.set(connected);
                            }
                            if let Some(can_sub) = data.get("can_subscribe").and_then(|v| v.as_bool()) {
                                youtube_can_subscribe.set(can_sub);
                            }
                        }
                    }
                }
            });
            || ()
        }, ());
    }

    // Refetch usage data callback - can be called when switching to Billing tab
    let refetch_usage = {
        let usage_data = usage_data.clone();
        let usage_loading = usage_loading.clone();
        Callback::from(move |_: ()| {
            let usage_data = usage_data.clone();
            let usage_loading = usage_loading.clone();
            spawn_local(async move {
                if let Ok(resp) = Api::get("/api/pricing/usage-projection").send().await {
                    if resp.ok() {
                        if let Ok(data) = resp.json::<UsageProjectionResponse>().await {
                            usage_data.set(Some(data));
                        }
                    }
                }
                usage_loading.set(false);
            });
        })
    };

    // Fetch usage data once on mount
    {
        let refetch_usage = refetch_usage.clone();
        use_effect_with_deps(move |_| {
            refetch_usage.emit(());
            || ()
        }, ());
    }

    // Refetch BYOT usage data callback
    let refetch_byot_usage = {
        let byot_usage_data = byot_usage_data.clone();
        let byot_usage_loading = byot_usage_loading.clone();
        Callback::from(move |_: ()| {
            let byot_usage_data = byot_usage_data.clone();
            let byot_usage_loading = byot_usage_loading.clone();
            spawn_local(async move {
                if let Ok(resp) = Api::get("/api/pricing/byot-usage").send().await {
                    if resp.ok() {
                        if let Ok(data) = resp.json::<ByotUsageResponse>().await {
                            byot_usage_data.set(Some(data));
                        }
                    }
                }
                byot_usage_loading.set(false);
            });
        })
    };

    // Fetch BYOT usage data when profile loads and user is BYOT
    {
        let refetch_byot_usage = refetch_byot_usage.clone();
        let profile_data = profile_data.clone();
        let byot_usage_loading = byot_usage_loading.clone();
        use_effect_with_deps(move |profile| {
            if let Some(p) = profile.as_ref() {
                if p.plan_type.as_deref() == Some("byot") {
                    refetch_byot_usage.emit(());
                } else {
                    byot_usage_loading.set(false);
                }
            }
            || ()
        }, (*profile_data).clone());
    }

    let _refetch_profile = {
        let profile_data = profile_data.clone();
        let user_verified = user_verified.clone();
        let error = error.clone();
        let auth_status = auth_status.clone();
        Callback::from(move |_: ()| {
            let profile_data = profile_data.clone();
            let user_verified = user_verified.clone();
            let error = error.clone();
            let auth_status = auth_status.clone();
            spawn_local(async move {
                let result = Api::get("/api/profile").send().await;
                match result {
                    Ok(response) => {
                        // After automatic retry, if we still get 401, user will be redirected to login
                        // So we only need to check for success
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
                            Err(_) => {
                                error.set(Some("Failed to parse profile data".to_string()));
                            }
                        }
                    }
                    Err(_) => {
                        error.set(Some("Failed to fetch profile".to_string()));
                    }
                }
            });
        })
    };

    // Check for OAuth success parameters
    // Handle URL query params for success messages and tab switching
    {
        let success = success.clone();
        let active_tab = active_tab.clone();
        let refresh_trigger = refresh_trigger.clone();
        use_effect_with_deps(move |_| {
            let query = location.query_str();
            if let Ok(params) = UrlSearchParams::new_with_str(query) {
                // Check for subscription success - show success message on Billing tab
                if params.get("subscription").as_deref() == Some("success") {
                    success.set(Some("Subscription activated successfully!".to_string()));
                    active_tab.set(DashboardTab::Billing);
                    // Trigger profile refresh to get updated subscription status
                    refresh_trigger.set(*refresh_trigger + 1);

                    // Clean up the URL
                    if let Some(window) = window() {
                        if let Ok(history) = window.history() {
                            let _ = history.replace_state_with_url(
                                &wasm_bindgen::JsValue::NULL,
                                "",
                                Some("/")
                            );
                        }
                    }
                } else if params.get("subscription").as_deref() == Some("changed") {
                    // Handle subscription plan change
                    success.set(Some("Subscription updated successfully!".to_string()));
                    active_tab.set(DashboardTab::Billing);
                    // Trigger profile refresh to get updated subscription status
                    refresh_trigger.set(*refresh_trigger + 1);

                    // Clean up the URL
                    if let Some(window) = window() {
                        if let Ok(history) = window.history() {
                            let _ = history.replace_state_with_url(
                                &wasm_bindgen::JsValue::NULL,
                                "",
                                Some("/")
                            );
                        }
                    }
                } else if params.get("subscription").as_deref() == Some("canceled") {
                    // Check for subscription canceled - show on Billing tab
                    success.set(Some("Subscription canceled successfully.".to_string()));
                    active_tab.set(DashboardTab::Billing);

                    // Clean up the URL
                    if let Some(window) = window() {
                        if let Ok(history) = window.history() {
                            let _ = history.replace_state_with_url(
                                &wasm_bindgen::JsValue::NULL,
                                "",
                                Some("/")
                            );
                        }
                    }
                } else if params.has("billing") {
                    // Check for billing tab param (for returning from Stripe portal, etc.)
                    active_tab.set(DashboardTab::Billing);

                    // Clean up the URL
                    if let Some(window) = window() {
                        if let Ok(history) = window.history() {
                            let _ = history.replace_state_with_url(
                                &wasm_bindgen::JsValue::NULL,
                                "",
                                Some("/")
                            );
                        }
                    }
                } else {
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

                        // Clean up the URL after showing the message
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
                }
            }
            || ()
        }, ());
    }

    // Single profile fetch effect - re-runs when refresh_trigger changes
    {
        let profile_data = profile_data.clone();
        let user_verified = user_verified.clone();
        let error = error.clone();
        let auth_status = auth_status.clone();
        let show_onboarding = show_onboarding.clone();
        let refresh_trigger_dep = *refresh_trigger;
        use_effect_with_deps(move |_| {
            let profile_data = profile_data.clone();
            let user_verified = user_verified.clone();
            let error = error.clone();
            let auth_status = auth_status.clone();
            let show_onboarding = show_onboarding.clone();
            spawn_local(async move {
                let result = Api::get("/api/profile").send().await;
                match result {
                    Ok(response) => {
                        // After automatic retry, if we still get non-OK, show landing page
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
                            Err(_) => {
                                error.set(Some("Failed to parse profile data".to_string()));
                            }
                        }
                    }
                    Err(_) => {
                        error.set(Some("Failed to fetch profile".to_string()));
                    }
                }
            });

            || ()
        }, refresh_trigger_dep);
    }
    // Fetch TOTP status when authenticated
    {
        let totp_enabled = totp_enabled.clone();
        let auth_status = auth_status.clone();
        use_effect_with_deps(move |auth| {
            if **auth == Some(true) {
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
        }, auth_status);
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
            let on_setup_2fa = {
                let active_tab = active_tab.clone();
                Callback::from(move |e: MouseEvent| {
                    e.prevent_default();
                    active_tab.set(DashboardTab::Settings);
                })
            };
            let on_dismiss_onboarding = {
                let show_onboarding = show_onboarding.clone();
                Callback::from(move |_: MouseEvent| {
                    show_onboarding.set(false);
                })
            };
        html! {
            <>
                // Background component: sends backup session key every 5 minutes
                <BackupKeySender />
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
                                            <span style="position: absolute; left: 0; color: #1E90FF;">{"•"}</span>
                                            <strong style="color: #fff;">{"Connect your services"}</strong>
                                            {" - Link your calendar, email, or messaging apps"}
                                        </li>
                                        <li style="color: rgba(255, 255, 255, 0.7); margin-bottom: 0.75rem; padding-left: 1.5rem; position: relative;">
                                            <span style="position: absolute; left: 0; color: #1E90FF;">{"•"}</span>
                                            <strong style="color: #fff;">{"Ask anything"}</strong>
                                            {" - Use the web chat or send an SMS to your Lightfriend number"}
                                        </li>
                                        <li style="color: rgba(255, 255, 255, 0.7); margin-bottom: 0.75rem; padding-left: 1.5rem; position: relative;">
                                            <span style="position: absolute; left: 0; color: #1E90FF;">{"•"}</span>
                                            <strong style="color: #fff;">{"Explore your tools"}</strong>
                                            {" - Check out the tools available to your assistant"}
                                        </li>
                                        <li style="color: rgba(255, 255, 255, 0.7); padding-left: 1.5rem; position: relative;">
                                            <span style="position: absolute; left: 0; color: #1E90FF;">{"•"}</span>
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
                    {
                        if *totp_enabled == Some(false) && !*banner_dismissed {
                            html! {
                                <div class="twofa-banner">
                                    <div class="twofa-banner-content">
                                        <span class="twofa-banner-text">
                                            {"Secure your account with two-factor authentication. "}
                                            <a onclick={on_setup_2fa}>{"Set up 2FA"}</a>
                                        </span>
                                    </div>
                                    <button class="twofa-banner-dismiss" onclick={on_dismiss_banner}>
                                        {"✕"}
                                    </button>
                                </div>
                            }
                        } else {
                            html! {}
                        }
                    }
                    {
                        if let Some(success_msg) = (*success).as_ref() {
                            html! {
                                <div class="message success-message">
                                    <div class="success-content">
                                        <span class="success-icon">{"✓"}</span>
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
                    <div class="status-section">
                        {
                            if let Some(profile) = (*profile_data).as_ref() {
                                let active_tab_for_usage = active_tab.clone();
                                    let refetch_usage_for_details = refetch_usage.clone();
                                    let is_byot = profile.plan_type.as_deref() == Some("byot");
                                    let byot_data = (*byot_usage_data).clone();
                                    let byot_loading = *byot_usage_loading;
                                    let active_tab_for_byot = active_tab.clone();
                                    let refetch_byot_for_details = refetch_byot_usage.clone();
                                    html! {
                                        <div class="credits-info">
                                            {
                                                if profile.sub_tier.is_some() {
                                                    // Subscribed user - compact info row
                                                    html! {
                                                        <div style="display: flex; flex-direction: column; gap: 0.75rem;">
                                                            // Phone number row
                                                            <div style="display: flex; align-items: center; gap: 0.5rem; flex-wrap: wrap;">
                                                                <span style="color: #888; font-size: 0.85rem;">{"Number:"}</span>
                                                                {
                                                                    if let Some(num) = &profile.preferred_number {
                                                                        html! {
                                                                            <span style="color: #7EB2FF; font-weight: 500;">{num}</span>
                                                                        }
                                                                    } else if profile.plan_type.as_deref() == Some("byot") {
                                                                        // BYOT users need to set up their Twilio number
                                                                        html! {
                                                                            <span style="color: #ff4444;">
                                                                                <Link<Route> to={Route::TwilioHostedInstructions} classes="setup-link">
                                                                                    {"Set up your Twilio number →"}
                                                                                </Link<Route>>
                                                                            </span>
                                                                        }
                                                                    } else {
                                                                        html! {
                                                                            <span style="color: #ff4444;">{"Not configured"}</span>
                                                                        }
                                                                    }
                                                                }
                                                            </div>
                                                            // Usage status row - show BYOT indicator for BYOT users
                                                            <div style="display: flex; align-items: center; gap: 0.5rem; flex-wrap: wrap;">
                                                                <span style="color: #888; font-size: 0.85rem;">{"Usage:"}</span>
                                                                {
                                                                    if is_byot {
                                                                        html! {
                                                                            <ByotUsageStatusIndicator
                                                                                data={byot_data}
                                                                                loading={byot_loading}
                                                                                on_details_click={Callback::from(move |_| {
                                                                                    active_tab_for_byot.set(DashboardTab::Billing);
                                                                                    refetch_byot_for_details.emit(());
                                                                                })}
                                                                            />
                                                                        }
                                                                    } else {
                                                                        html! {
                                                                            <UsageStatusIndicator
                                                                                data={(*usage_data).clone()}
                                                                                loading={*usage_loading}
                                                                                on_details_click={Callback::from(move |_| {
                                                                                    active_tab_for_usage.set(DashboardTab::Billing);
                                                                                    refetch_usage_for_details.emit(());
                                                                                })}
                                                                            />
                                                                        }
                                                                    }
                                                                }
                                                            </div>
                                                        </div>
                                                    }
                                                } else {
                                                    // Non-subscribed user - show subscribe prompt
                                                    html! {
                                                        <div class="subscription-promo">
                                                            <Link<Route> to={Route::Pricing} classes="promo-link">
                                                                {"Subscribe to start ->"}
                                                            </Link<Route>>
                                                        </div>
                                                    }
                                                }
                                            }
                                        </div>
                                    }
                            } else {
                                html! {}
                            }
                        }
                    </div>
                    // Web Chat Section
                    {
                        if let Some(profile) = (*profile_data).as_ref() {
                            if profile.sub_tier.is_some() {
                                let on_send = {
                                    let chat_input = chat_input.clone();
                                    let chat_user_msg = chat_user_msg.clone();
                                    let chat_bot_reply = chat_bot_reply.clone();
                                    let chat_loading = chat_loading.clone();
                                    let chat_error = chat_error.clone();
                                    let refetch_usage = refetch_usage.clone();
                                    let chat_image = chat_image.clone();
                                    let chat_image_preview = chat_image_preview.clone();
                                    Callback::from(move |_| {
                                        let message = (*chat_input).clone();
                                        let has_image = (*chat_image).is_some();

                                        // Allow send if there's text OR an image
                                        if message.trim().is_empty() && !has_image {
                                            return;
                                        }

                                        let chat_input = chat_input.clone();
                                        let chat_user_msg = chat_user_msg.clone();
                                        let chat_bot_reply = chat_bot_reply.clone();
                                        let chat_loading = chat_loading.clone();
                                        let chat_error = chat_error.clone();
                                        let refetch_usage = refetch_usage.clone();
                                        let chat_image = chat_image.clone();
                                        let chat_image_preview = chat_image_preview.clone();
                                        let image_file = (*chat_image).clone();

                                        // Set user message and clear previous reply
                                        let display_msg = if has_image {
                                            if message.trim().is_empty() {
                                                "[Image]".to_string()
                                            } else {
                                                format!("[Image] {}", message)
                                            }
                                        } else {
                                            message.clone()
                                        };
                                        chat_user_msg.set(Some(display_msg));
                                        chat_bot_reply.set(None);
                                        chat_input.set(String::new());
                                        chat_image.set(None);
                                        chat_image_preview.set(None);
                                        chat_loading.set(true);
                                        chat_error.set(None);

                                        spawn_local(async move {
                                            let result = if let Some(file) = image_file {
                                                // Use multipart FormData for image upload
                                                let form_data = web_sys::FormData::new().unwrap();
                                                form_data.append_with_str("message", &message).unwrap();
                                                form_data.append_with_blob("image", &file).unwrap();

                                                // Use credentials (cookies) for authentication like the Api utility does
                                                gloo_net::http::Request::post(&format!("{}/api/chat/web-with-image", config::get_backend_url()))
                                                    .credentials(web_sys::RequestCredentials::Include)
                                                    .body(form_data)
                                                    .send()
                                                    .await
                                            } else {
                                                // Regular JSON request without image
                                                Api::post("/api/chat/web")
                                                    .json(&json!({"message": message}))
                                                    .expect("Failed to serialize")
                                                    .send()
                                                    .await
                                            };

                                            match result {
                                                Ok(response) => {
                                                    if response.ok() {
                                                        match response.json::<Value>().await {
                                                            Ok(data) => {
                                                                let reply = data["message"].as_str().unwrap_or("No response").to_string();
                                                                chat_bot_reply.set(Some(reply));
                                                                // Refresh usage after chat
                                                                refetch_usage.emit(());
                                                                // Dispatch event to refresh tasks
                                                                if let Some(window) = web_sys::window() {
                                                                    if let Ok(event) = web_sys::CustomEvent::new("lightfriend-chat-sent") {
                                                                        let _ = window.dispatch_event(&event);
                                                                    }
                                                                }
                                                            }
                                                            Err(_) => {
                                                                chat_error.set(Some("Failed to parse response".to_string()));
                                                            }
                                                        }
                                                    } else {
                                                        match response.json::<Value>().await {
                                                            Ok(data) => {
                                                                let err = data["error"].as_str().unwrap_or("Request failed").to_string();
                                                                chat_error.set(Some(err));
                                                            }
                                                            Err(_) => {
                                                                chat_error.set(Some("Request failed".to_string()));
                                                            }
                                                        }
                                                    }
                                                }
                                                Err(_) => {
                                                    chat_error.set(Some("Network error".to_string()));
                                                }
                                            }
                                            chat_loading.set(false);
                                        });
                                    })
                                };

                                // Handler for "What's new?" digest button
                                let on_digest = {
                                    let chat_user_msg = chat_user_msg.clone();
                                    let chat_bot_reply = chat_bot_reply.clone();
                                    let chat_loading = chat_loading.clone();
                                    let chat_error = chat_error.clone();
                                    let refetch_usage = refetch_usage.clone();
                                    Callback::from(move |_| {
                                        let chat_user_msg = chat_user_msg.clone();
                                        let chat_bot_reply = chat_bot_reply.clone();
                                        let chat_loading = chat_loading.clone();
                                        let chat_error = chat_error.clone();
                                        let refetch_usage = refetch_usage.clone();

                                        // Set user message as "What's new?"
                                        chat_user_msg.set(Some("What's new?".to_string()));
                                        chat_bot_reply.set(None);
                                        chat_loading.set(true);
                                        chat_error.set(None);

                                        spawn_local(async move {
                                            match Api::get("/api/chat/digest").send().await {
                                                Ok(response) => {
                                                    if response.ok() {
                                                        match response.json::<Value>().await {
                                                            Ok(data) => {
                                                                let reply = data["message"].as_str().unwrap_or("No response").to_string();
                                                                chat_bot_reply.set(Some(reply));
                                                                refetch_usage.emit(());
                                                            }
                                                            Err(_) => {
                                                                chat_error.set(Some("Failed to parse response".to_string()));
                                                            }
                                                        }
                                                    } else {
                                                        match response.json::<Value>().await {
                                                            Ok(data) => {
                                                                let err = data["error"].as_str().unwrap_or("Request failed").to_string();
                                                                chat_error.set(Some(err));
                                                            }
                                                            Err(_) => {
                                                                chat_error.set(Some("Request failed".to_string()));
                                                            }
                                                        }
                                                    }
                                                }
                                                Err(_) => {
                                                    chat_error.set(Some("Network error".to_string()));
                                                }
                                            }
                                            chat_loading.set(false);
                                        });
                                    })
                                };

                                // Handler for starting a web call
                                let on_start_call = {
                                    let call_active = call_active.clone();
                                    let call_connecting = call_connecting.clone();
                                    let call_duration = call_duration.clone();
                                    let call_error = call_error.clone();
                                    let call_cost_per_min = call_cost_per_min.clone();
                                    let refetch_usage = refetch_usage.clone();
                                    Callback::from(move |_| {
                                        let call_active = call_active.clone();
                                        let call_connecting = call_connecting.clone();
                                        let call_duration = call_duration.clone();
                                        let call_error = call_error.clone();
                                        let call_cost_per_min = call_cost_per_min.clone();
                                        let _refetch_usage = refetch_usage.clone();

                                        call_connecting.set(true);
                                        call_error.set(None);

                                        spawn_local(async move {
                                            // Get signed URL from backend
                                            match Api::get("/api/call/web-signed-url").send().await {
                                                Ok(response) => {
                                                    if response.ok() {
                                                        match response.json::<Value>().await {
                                                            Ok(data) => {
                                                                if let Some(signed_url) = data["signed_url"].as_str() {
                                                                    if let Some(cost) = data["cost_per_minute"].as_f64() {
                                                                        call_cost_per_min.set(cost as f32);
                                                                    }
                                                                    // Get agent overrides if available
                                                                    let overrides = data.get("agent_overrides")
                                                                        .map(|v| serde_wasm_bindgen::to_value(v).unwrap_or(wasm_bindgen::JsValue::NULL))
                                                                        .unwrap_or(wasm_bindgen::JsValue::NULL);
                                                                    // Start ElevenLabs call via JS interop with overrides
                                                                    let result = crate::utils::elevenlabs_web::start_elevenlabs_call(signed_url, overrides).await;
                                                                    if result.is_truthy() {
                                                                        call_active.set(true);
                                                                        call_duration.set(0);
                                                                    } else {
                                                                        call_error.set(Some("Failed to start call. Check microphone permissions.".to_string()));
                                                                    }
                                                                } else {
                                                                    call_error.set(Some("Invalid response from server".to_string()));
                                                                }
                                                            }
                                                            Err(_) => {
                                                                call_error.set(Some("Failed to parse server response".to_string()));
                                                            }
                                                        }
                                                    } else {
                                                        match response.json::<Value>().await {
                                                            Ok(data) => {
                                                                let err = data["error"].as_str().unwrap_or("Failed to start call").to_string();
                                                                call_error.set(Some(err));
                                                            }
                                                            Err(_) => {
                                                                call_error.set(Some("Failed to start call".to_string()));
                                                            }
                                                        }
                                                    }
                                                }
                                                Err(_) => {
                                                    call_error.set(Some("Network error".to_string()));
                                                }
                                            }
                                            call_connecting.set(false);
                                        });
                                    })
                                };

                                // Handler for ending a web call
                                let on_end_call = {
                                    let call_active = call_active.clone();
                                    let call_duration = call_duration.clone();
                                    let call_error = call_error.clone();
                                    let refetch_usage = refetch_usage.clone();
                                    Callback::from(move |_| {
                                        let call_active = call_active.clone();
                                        let call_duration = call_duration.clone();
                                        let _call_error = call_error.clone();
                                        let refetch_usage = refetch_usage.clone();

                                        spawn_local(async move {
                                            // End ElevenLabs call via JS interop
                                            let duration = crate::utils::elevenlabs_web::end_elevenlabs_call().await;
                                            let duration_secs = duration.as_f64().unwrap_or(0.0) as i32;

                                            call_active.set(false);

                                            // Report usage to backend
                                            if duration_secs > 0 {
                                                let _ = Api::post("/api/call/web-end")
                                                    .json(&serde_json::json!({"duration_secs": duration_secs}))
                                                    .expect("Failed to serialize")
                                                    .send()
                                                    .await;
                                                refetch_usage.emit(());
                                            }
                                            call_duration.set(0);
                                        });
                                    })
                                };

                                html! {
                                    <div class="web-chat-section">
                                        <div class="web-chat-messages">
                                            {
                                                match ((*chat_user_msg).clone(), (*chat_bot_reply).clone(), *chat_loading) {
                                                    (None, _, false) => html! {},
                                                    (Some(user_msg), None, true) => html! {
                                                        <>
                                                            <div class="chat-msg user">{user_msg}</div>
                                                            <div class="chat-msg assistant loading">{"..."}</div>
                                                        </>
                                                    },
                                                    (Some(user_msg), Some(bot_reply), _) => html! {
                                                        <>
                                                            <div class="chat-msg user">{user_msg}</div>
                                                            <div class="chat-msg assistant">{bot_reply}</div>
                                                        </>
                                                    },
                                                    (Some(user_msg), None, false) => html! {
                                                        <div class="chat-msg user">{user_msg}</div>
                                                    },
                                                    _ => html! {}
                                                }
                                            }
                                        </div>
                                        {
                                            if let Some(err) = (*chat_error).as_ref() {
                                                html! { <div class="chat-error">{err}</div> }
                                            } else {
                                                html! {}
                                            }
                                        }
                                        {
                                            if let Some(err) = (*call_error).as_ref() {
                                                html! { <div class="chat-error">{err}</div> }
                                            } else {
                                                html! {}
                                            }
                                        }
                                        // Image preview above input
                                        {
                                            if let Some(preview_url) = (*chat_image_preview).clone() {
                                                let chat_image_clear = chat_image.clone();
                                                let chat_image_preview_clear = chat_image_preview.clone();
                                                html! {
                                                    <div class="chat-image-preview">
                                                        <img src={preview_url} alt="Attached image" />
                                                        <button
                                                            class="remove-image-btn"
                                                            onclick={Callback::from(move |_: MouseEvent| {
                                                                chat_image_clear.set(None);
                                                                chat_image_preview_clear.set(None);
                                                            })}
                                                            title="Remove image"
                                                        >
                                                            {"×"}
                                                        </button>
                                                    </div>
                                                }
                                            } else {
                                                html! {}
                                            }
                                        }
                                        <div class="web-chat-input">
                                            // Hidden file input for image selection
                                            <input
                                                type="file"
                                                ref={chat_file_input_ref.clone()}
                                                accept="image/*"
                                                style="display: none;"
                                                onchange={{
                                                    let chat_image = chat_image.clone();
                                                    let chat_image_preview = chat_image_preview.clone();
                                                    let chat_error = chat_error.clone();
                                                    Callback::from(move |e: Event| {
                                                        let input: HtmlInputElement = e.target_unchecked_into();
                                                        if let Some(files) = input.files() {
                                                            if let Some(file) = files.get(0) {
                                                                // Check file size (10MB limit)
                                                                if file.size() > 10.0 * 1024.0 * 1024.0 {
                                                                    chat_error.set(Some("Image must be less than 10MB".to_string()));
                                                                    return;
                                                                }
                                                                // Generate preview
                                                                let chat_image = chat_image.clone();
                                                                let chat_image_preview = chat_image_preview.clone();
                                                                let file_clone = file.clone();
                                                                wasm_bindgen_futures::spawn_local(async move {
                                                                    let array_buffer = wasm_bindgen_futures::JsFuture::from(file_clone.array_buffer()).await;
                                                                    if let Ok(buffer) = array_buffer {
                                                                        let uint8_array = js_sys::Uint8Array::new(&buffer);
                                                                        let bytes: Vec<u8> = uint8_array.to_vec();
                                                                        let base64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
                                                                        let content_type = file_clone.type_();
                                                                        let data_url = format!("data:{};base64,{}", content_type, base64);
                                                                        chat_image_preview.set(Some(data_url));
                                                                        chat_image.set(Some(file_clone));
                                                                    }
                                                                });
                                                            }
                                                        }
                                                        // Clear the input so same file can be selected again
                                                        input.set_value("");
                                                    })
                                                }}
                                            />
                                            <button
                                                class="image-attach-button"
                                                onclick={{
                                                    let chat_file_input_ref = chat_file_input_ref.clone();
                                                    Callback::from(move |_: MouseEvent| {
                                                        if let Some(input) = chat_file_input_ref.cast::<HtmlInputElement>() {
                                                            input.click();
                                                        }
                                                    })
                                                }}
                                                disabled={*chat_loading || *call_active}
                                                title="Attach an image"
                                            >
                                                <i class="fas fa-paperclip"></i>
                                            </button>
                                            <button
                                                class="digest-button"
                                                onclick={{
                                                    let on_digest = on_digest.clone();
                                                    Callback::from(move |_: MouseEvent| {
                                                        on_digest.emit(());
                                                    })
                                                }}
                                                disabled={*chat_loading || *call_active || *call_connecting}
                                                title="Get a digest of recent activity from your connected apps"
                                            >
                                                {"What's new?"}
                                                <span class="digest-price">{"0.01€"}</span>
                                            </button>
                                            {
                                                if *call_active {
                                                    let duration = *call_duration;
                                                    let mins = duration / 60;
                                                    let secs = duration % 60;
                                                    html! {
                                                        <button
                                                            class="call-button call-active"
                                                            onclick={{
                                                                let on_end_call = on_end_call.clone();
                                                                Callback::from(move |_: MouseEvent| {
                                                                    on_end_call.emit(());
                                                                })
                                                            }}
                                                            title="End the call"
                                                        >
                                                            {format!("End {mins}:{secs:02}")}
                                                        </button>
                                                    }
                                                } else if *call_connecting {
                                                    html! {
                                                                <button
                                                            class="call-button call-connecting"
                                                            disabled=true
                                                            title="Connecting..."
                                                        >
                                                            {"..."}
                                                        </button>
                                                    }
                                                } else {
                                                    let cost = *call_cost_per_min;
                                                    html! {
                                                        <button
                                                            class="call-button"
                                                            onclick={{
                                                                let on_start_call = on_start_call.clone();
                                                                Callback::from(move |_: MouseEvent| {
                                                                    on_start_call.emit(());
                                                                })
                                                            }}
                                                            disabled={*chat_loading}
                                                            title={format!("Start voice call ({:.0}c/min)", cost * 100.0)}
                                                        >
                                                            <i class="fas fa-phone"></i>
                                                            <span class="call-price">{format!("{:.2}€/m", cost)}</span>
                                                        </button>
                                                    }
                                                }
                                            }
                                            <input
                                                type="text"
                                                ref={chat_input_ref.clone()}
                                                value={(*chat_input).clone()}
                                                placeholder="Ask your assistant..."
                                                disabled={*chat_loading || *call_active}
                                                oninput={{
                                                    let chat_input = chat_input.clone();
                                                    Callback::from(move |e: InputEvent| {
                                                        let input: HtmlInputElement = e.target_unchecked_into();
                                                        chat_input.set(input.value());
                                                    })
                                                }}
                                                onkeypress={{
                                                    let on_send = on_send.clone();
                                                    Callback::from(move |e: KeyboardEvent| {
                                                        if e.key() == "Enter" {
                                                            on_send.emit(());
                                                        }
                                                    })
                                                }}
                                                onpaste={{
                                                    let chat_image = chat_image.clone();
                                                    let chat_image_preview = chat_image_preview.clone();
                                                    let chat_error = chat_error.clone();
                                                    Callback::from(move |e: Event| {
                                                        use wasm_bindgen::JsCast;
                                                        // Cast to ClipboardEvent to access clipboard_data
                                                        if let Some(clipboard_event) = e.dyn_ref::<web_sys::ClipboardEvent>() {
                                                            if let Some(clipboard_data) = clipboard_event.clipboard_data() {
                                                                if let Some(items) = clipboard_data.files() {
                                                                    for i in 0..items.length() {
                                                                        if let Some(file) = items.get(i) {
                                                                            if file.type_().starts_with("image/") {
                                                                                e.prevent_default();
                                                                                // Check file size (10MB limit)
                                                                                if file.size() > 10.0 * 1024.0 * 1024.0 {
                                                                                    chat_error.set(Some("Image must be less than 10MB".to_string()));
                                                                                    return;
                                                                                }
                                                                                // Generate preview
                                                                                let chat_image = chat_image.clone();
                                                                                let chat_image_preview = chat_image_preview.clone();
                                                                                let file_clone = file.clone();
                                                                                wasm_bindgen_futures::spawn_local(async move {
                                                                                    let array_buffer = wasm_bindgen_futures::JsFuture::from(file_clone.array_buffer()).await;
                                                                                    if let Ok(buffer) = array_buffer {
                                                                                        let uint8_array = js_sys::Uint8Array::new(&buffer);
                                                                                        let bytes: Vec<u8> = uint8_array.to_vec();
                                                                                        let base64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
                                                                                        let content_type = file_clone.type_();
                                                                                        let data_url = format!("data:{};base64,{}", content_type, base64);
                                                                                        chat_image_preview.set(Some(data_url));
                                                                                        chat_image.set(Some(file_clone));
                                                                                    }
                                                                                });
                                                                                return;
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    })
                                                }}
                                            />
                                            <button
                                                class="send-button"
                                                onclick={{
                                                    let on_send = on_send.clone();
                                                    Callback::from(move |_: MouseEvent| {
                                                        on_send.emit(());
                                                    })
                                                }}
                                                disabled={*chat_loading || *call_active}
                                            >
                                                {"Send"}
                                                <span class="send-price">{"0.01€"}</span>
                                            </button>
                                        </div>
                                    </div>
                                }
                            } else {
                                html! {}
                            }
                        } else {
                            html! {}
                        }
                    }
                    {
                        if (*profile_data).is_some() {
                            html! {
                                <>
                                        <br/>
                                        <div class="dashboard-tabs">
                                            <button
                                                class={classes!("tab-button", (*active_tab == DashboardTab::Connections).then(|| "active"))}
                                                onclick={{
                                                    let active_tab = active_tab.clone();
                                                    Callback::from(move |_| active_tab.set(DashboardTab::Connections))
                                                }}
                                            >
                                                {"Connections"}
                                            </button>
                                            <button
                                                class={classes!("tab-button", (*active_tab == DashboardTab::Controls).then(|| "active"))}
                                                onclick={{
                                                    let active_tab = active_tab.clone();
                                                    Callback::from(move |_| active_tab.set(DashboardTab::Controls))
                                                }}
                                            >
                                                {"Controls"}
                                            </button>
                                            <button
                                                class={classes!("tab-button", (*active_tab == DashboardTab::Media).then(|| "active"))}
                                                onclick={{
                                                    let active_tab = active_tab.clone();
                                                    Callback::from(move |_| active_tab.set(DashboardTab::Media))
                                                }}
                                            >
                                                {"Media"}
                                            </button>
                                            <button
                                                class={classes!("tab-button", (*active_tab == DashboardTab::Billing).then(|| "active"))}
                                                onclick={{
                                                    let active_tab = active_tab.clone();
                                                    let refetch_usage = refetch_usage.clone();
                                                    Callback::from(move |_| {
                                                        active_tab.set(DashboardTab::Billing);
                                                        refetch_usage.emit(());
                                                    })
                                                }}
                                            >
                                                {"Billing"}
                                            </button>
                                            <button
                                                class={classes!("tab-button", (*active_tab == DashboardTab::Settings).then(|| "active"))}
                                                onclick={{
                                                    let active_tab = active_tab.clone();
                                                    Callback::from(move |_| active_tab.set(DashboardTab::Settings))
                                                }}
                                            >
                                                {"Settings"}
                                            </button>
                                        </div>
                                        {
                                            match *active_tab {
                                                DashboardTab::Connections => html! {
                                                    <div class="connections-tab">
                                                        {
                                                            if let Some(profile) = (*profile_data).as_ref() {
                                                                html! {
                                                                    <Connect user_id={profile.id} sub_tier={profile.sub_tier.clone()} discount={profile.discount} phone_number={profile.phone_number.clone()} estimated_monitoring_cost={profile.estimated_monitoring_cost.clone()}/>
                                                                }
                                                            } else {
                                                                html! {}
                                                            }
                                                        }
                                                        <div class="feature-status">
                                                            <p class="feature-suggestion">
                                                                {"Have a feature in mind? Email your suggestions to "}
                                                                <a href="mailto:rasmus@ahtava.com">{"rasmus@ahtava.com"}</a>
                                                            </p>
                                                            <h4>{"Tips"}</h4>
                                                            <ul>
                                                                <li>{"You can ask multiple questions in a single SMS to save money. Note that answers will be less detailed due to SMS character limits. Example: 'did sam altman tweet today and whats the weather?' -> 'Sam Altman hasn't tweeted today. Last tweet was on March 3, a cryptic \"!!!\" image suggesting a major AI development. Weather in Tampere: partly cloudy, 0°C, 82% humidity, wind at 4 m/s.'"}</li>
                                                                <li>{"Start your message with 'forget' to make the assistant forget previous conversation context and start fresh. Note that this only applies to that one message - the next message will again remember previous context."}</li>
                                                            </ul>
                                                        </div>
                                                        {
                                                            if let Some(profile) = (*profile_data).as_ref() {
                                                                if profile.sub_tier.is_some() {
                                                                    html! {
                                                                        <div class="subscriber-promo">
                                                                            <p>{"Subscribed users can get 20% off from Cold Turkey Blocker Pro with code LIGHTFRIEND"}</p>
                                                                            <a href="https://getcoldturkey.com" target="_blank" rel="noopener noreferrer">{"getcoldturkey.com"}</a>
                                                                        </div>
                                                                    }
                                                                } else {
                                                                    html! {}
                                                                }
                                                            } else {
                                                                html! {}
                                                            }
                                                        }
                                                    </div>
                                                },
                                                DashboardTab::Controls => html! {
                                                    <div class="controls-tab">
                                                        <TeslaControls />
                                                    </div>
                                                },
                                                DashboardTab::Media => html! {
                                                    <div class="media-tab">
                                                        <YouTubeHub youtube_connected={*youtube_connected} can_subscribe={*youtube_can_subscribe} />
                                                    </div>
                                                },
                                                DashboardTab::Billing => html! {
                                                    <div class="billing-tab">
                                                        {
                                                            if let Some(profile) = (*profile_data).as_ref() {
                                                                html! {
                                                                    <BillingPage user_profile={profile.clone()} />
                                                                }
                                                            } else {
                                                                html! {
                                                                    <div class="loading-profile">{"Loading billing..."}</div>
                                                                }
                                                            }
                                                        }
                                                    </div>
                                                },
                                                DashboardTab::Settings => html! {
                                                    <div class="settings-tab">
                                                        {
                                                            if let Some(profile) = (*profile_data).as_ref() {
                                                                html! {
                                                                    <SettingsPage
                                                                        user_profile={profile.clone()}
                                                                        on_profile_update={{
                                                                            let profile_data = profile_data.clone();
                                                                            Callback::from(move |updated_profile| {
                                                                                profile_data.set(Some(updated_profile));
                                                                            })
                                                                        }}
                                                                    />
                                                                }
                                                            } else {
                                                                html! {
                                                                    <div class="loading-profile">{"Loading profile..."}</div>
                                                                }
                                                            }
                                                        }
                                                    </div>
                                                }
                                            }
                                        }
                                    </>
                                }
                        } else {
                            html! {}
                        }
                    }
                    <footer class="dashboard-footer">
                        <div class="development-links">
                            <p>{"Source code on "}
                                <a href="https://github.com/ahtavarasmus/lightfriend" target="_blank" rel="noopener noreferrer">{"GitHub"}</a>
                            </p>
                            <div class="legal-links">
                                <a href="/terms">{"Terms & Conditions"}</a>
                                {" | "}
                                <a href="/privacy">{"Privacy Policy"}</a>
                                {" | "}
                                <a href="/updates">{"Updates"}</a>
                            </div>
                        </div>
                    </footer>
                </div>
                <style>
                    {r#"
                        .twofa-banner {
                            display: flex;
                            align-items: center;
                            justify-content: space-between;
                            background: rgba(30, 144, 255, 0.1);
                            border: 1px solid rgba(30, 144, 255, 0.2);
                            border-radius: 8px;
                            padding: 0.75rem 1rem;
                            margin-bottom: 1.5rem;
                        }
                        .twofa-banner-content {
                            display: flex;
                            align-items: center;
                            gap: 0.75rem;
                        }
                        .twofa-banner-text {
                            color: #ccc;
                            font-size: 0.9rem;
                        }
                        .twofa-banner-text a {
                            color: #1E90FF;
                            cursor: pointer;
                            text-decoration: none;
                        }
                        .twofa-banner-text a:hover {
                            text-decoration: underline;
                        }
                        .twofa-banner-dismiss {
                            background: transparent;
                            border: none;
                            color: #666;
                            cursor: pointer;
                            font-size: 1rem;
                            padding: 0.25rem;
                        }
                        .twofa-banner-dismiss:hover {
                            color: #999;
                        }
                        .status-section {
                            display: flex;
                            align-items: center;
                            justify-content: space-between;
                            margin-bottom: 1.5rem;
                        }
                        .success-message {
                            border: 1px solid rgba(76, 175, 80, 0.3);
                            background: none !important;
                            border-radius: 8px;
                            padding: 1rem;
                            margin-bottom: 1.5rem;
                            animation: fadeIn 0.5s ease-in-out;
                        }
                        .success-content {
                            display: flex;
                            background: none !important;
                            align-items: center;
                            gap: 1rem;
                        }
                        .success-icon {
                            background-color: rgba(76, 175, 80, 0.2);
                            border-radius: 50%;
                            width: 24px;
                            height: 24px;
                            display: flex;
                            align-items: center;
                            justify-content: center;
                            color: #4CAF50;
                        }
                        .success-text {
                            color: #4CAF50;
                            font-size: 0.95rem;
                        }
                        @keyframes fadeIn {
                            from { opacity: 0; transform: translateY(-10px); }
                            to { opacity: 1; transform: translateY(0); }
                        }
                        .credits-info {
                            width: 100%;
                            margin-bottom: 1rem;
                        }
                        .credits-grid {
                            display: grid;
                            grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
                            gap: 1rem;
                            width: 100%;
                        }
                        .credit-item {
                            background: rgba(30, 144, 255, 0.05);
                            border: 1px solid rgba(30, 144, 255, 0.1);
                            border-radius: 8px;
                            padding: 0.75rem;
                            display: flex;
                            flex-direction: column;
                            align-items: center;
                            gap: 0.25rem;
                            position: relative;
                            cursor: pointer;
                            transition: all 0.3s ease;
                            outline: none;
                        }
                        .magic-link-item {
                            background: rgba(30, 144, 255, 0.05);
                            border: 1px solid rgba(30, 144, 255, 0.1);
                            border-radius: 8px;
                            padding: 0.75rem;
                            display: flex;
                            flex-direction: column;
                            align-items: flex-start;
                            gap: 0.25rem;
                            position: relative;
                            cursor: pointer;
                            transition: all 0.3s ease;
                            outline: none;
                            width: 100%;
                        }
                        .magic-link-item:hover {
                            background: rgba(30, 144, 255, 0.1);
                            border-color: rgba(30, 144, 255, 0.2);
                        }
                        .magic-link-item button:hover {
                            background: rgba(30, 144, 255, 0.3) !important;
                            transform: translateY(-1px);
                        }
                        .credit-item:hover,
                        .credit-item:focus {
                            background: rgba(30, 144, 255, 0.1);
                            border-color: rgba(30, 144, 255, 0.2);
                        }
                        .credit-tooltip {
                            position: absolute;
                            bottom: calc(100% + 10px);
                            left: 50%;
                            transform: translateX(-50%);
                            background: rgba(0, 0, 0, 0.9);
                            color: #fff;
                            padding: 1rem;
                            border-radius: 8px;
                            font-size: 0.80rem;
                            width: max-content;
                            max-width: 350px;
                            z-index: 1000;
                            opacity: 0;
                            visibility: hidden;
                            transition: all 0.3s ease;
                            box-shadow: 0 4px 12px rgba(0, 0, 0, 0.2);
                            border: 1px solid rgba(30, 144, 255, 0.2);
                            text-align: center;
                        }
                        .credit-tooltip::after {
                            content: '';
                            position: absolute;
                            top: 100%;
                            left: 50%;
                            transform: translateX(-50%);
                            border-width: 8px;
                            border-style: solid;
                            border-color: rgba(0, 0, 0, 0.9) transparent transparent transparent;
                        }
                        .credit-item:hover .credit-tooltip,
                        .credit-item:focus .credit-tooltip {
                            opacity: 1;
                            visibility: visible;
                        }
                        .credit-label {
                            color: #999;
                            font-size: 0.8rem;
                            text-transform: uppercase;
                            letter-spacing: 0.5px;
                        }
                        .credit-value {
                            color: #7EB2FF;
                            font-size: 1.1rem;
                            font-weight: 500;
                        }
                        .credit-equivalents {
                            color: #888;
                            font-size: 0.75rem;
                            margin-top: 0.25rem;
                            text-align: center;
                            line-height: 1.3;
                        }
                        .credit-equivalents-main {
                            color: #7EB2FF;
                            font-size: 0.9rem;
                            text-align: center;
                            line-height: 1.4;
                        }
                        .reset-timer {
                            display: block;
                            color: #999;
                            font-size: 0.8rem;
                            margin-top: 0.3rem;
                        }
                        .credit-warning {
                            grid-column: 1 / -1;
                            color: #ff4444;
                            font-size: 0.9rem;
                            text-align: center;
                            padding: 0.5rem;
                            background: rgba(255, 68, 68, 0.1);
                            border-radius: 6px;
                            margin-top: 0.5rem;
                        }
                        .subscription-promo {
                            background: linear-gradient(45deg, rgba(30, 144, 255, 0.1), rgba(65, 105, 225, 0.1));
                            border: 1px solid rgba(30, 144, 255, 0.2);
                            border-radius: 8px;
                            padding: 0.75rem 1.25rem;
                            margin-left: 1rem;
                            flex-shrink: 0;
                            grid-column: 1 / -1;
                        }
                        .promo-link {
                            color: #1E90FF;
                            text-decoration: none;
                            font-size: 0.9rem;
                            display: inline-flex;
                            align-items: center;
                            transition: all 0.3s ease;
                        }
                        .promo-link:hover {
                            color: #7EB2FF;
                            transform: translateX(5px);
                        }
                        .phone-selector {
                            margin: 1.5rem 0;
                        }
                        .dashboard-container {
                            min-height: 100vh;
                            border-radius: 12px;
                            background: #1a1a1a;
                            padding: 3rem 2rem;
                            width: 100%;
                            max-width: 800px;
                            margin: 4rem auto;
                        }
                        .panel-title {
                            font-size: 2.5rem;
                            background: linear-gradient(45deg, #fff, #7EB2FF);
                            -webkit-background-clip: text;
                            -webkit-text-fill-color: transparent;
                            margin: 0 0 1.5rem 0;
                            text-align: center;
                        }
                        /* Web Chat Styles */
                        .web-chat-section {
                            margin: 1.5rem 0;
                        }
                        .web-chat-messages {
                            max-height: 200px;
                            overflow-y: auto;
                            margin-bottom: 0.75rem;
                            display: flex;
                            flex-direction: column;
                            gap: 0.5rem;
                        }
                        .web-chat-messages:empty {
                            display: none;
                        }
                        .chat-msg {
                            padding: 0.5rem 0.75rem;
                            border-radius: 8px;
                            max-width: 85%;
                            font-size: 0.9rem;
                            line-height: 1.4;
                            word-wrap: break-word;
                        }
                        .chat-msg.user {
                            background: rgba(30, 144, 255, 0.2);
                            color: #fff;
                            align-self: flex-end;
                            margin-left: auto;
                        }
                        .chat-msg.assistant {
                            background: rgba(76, 175, 80, 0.15);
                            color: #ccc;
                            align-self: flex-start;
                        }
                        .chat-msg.loading {
                            color: #666;
                            font-style: italic;
                        }
                        .chat-error {
                            color: #ff4444;
                            font-size: 0.8rem;
                            margin-bottom: 0.5rem;
                        }
                        .web-chat-input {
                            display: flex;
                            gap: 0.5rem;
                            align-items: stretch;
                        }
                        .web-chat-input input {
                            flex: 1;
                            padding: 0.75rem 1rem;
                            border: 1px solid rgba(30, 144, 255, 0.2);
                            border-radius: 8px;
                            background: rgba(30, 30, 30, 0.7);
                            color: #fff;
                            font-size: 0.95rem;
                        }
                        .web-chat-input input:focus {
                            outline: none;
                            border-color: #1E90FF;
                        }
                        .web-chat-input input:disabled {
                            opacity: 0.6;
                        }
                        .digest-button {
                            position: relative;
                            padding: 0.75rem 1rem 1.1rem 1rem;
                            background: rgba(76, 175, 80, 0.15);
                            border: 1px solid rgba(76, 175, 80, 0.25);
                            border-radius: 8px;
                            color: #4ade80;
                            font-size: 0.85rem;
                            cursor: pointer;
                            transition: all 0.3s ease;
                            white-space: nowrap;
                        }
                        .digest-price {
                            position: absolute;
                            bottom: 2px;
                            right: 4px;
                            font-size: 0.55rem;
                            color: rgba(76, 175, 80, 0.6);
                        }
                        .digest-button:hover:not(:disabled) {
                            background: rgba(76, 175, 80, 0.25);
                            border-color: rgba(76, 175, 80, 0.4);
                        }
                        .digest-button:disabled {
                            opacity: 0.5;
                            cursor: not-allowed;
                        }
                        .send-button {
                            position: relative;
                            padding: 0.75rem 1.25rem 1.1rem 1.25rem;
                            background: linear-gradient(45deg, #1E90FF, #4169E1);
                            border: none;
                            border-radius: 8px;
                            color: #fff;
                            font-size: 0.9rem;
                            cursor: pointer;
                            transition: all 0.3s ease;
                        }
                        .send-button:hover:not(:disabled) {
                            background: linear-gradient(45deg, #4169E1, #1E90FF);
                        }
                        .send-button:disabled {
                            opacity: 0.5;
                            cursor: not-allowed;
                        }
                        .send-price {
                            position: absolute;
                            bottom: 2px;
                            right: 4px;
                            font-size: 0.6rem;
                            color: rgba(255, 255, 255, 0.6);
                        }
                        .call-button {
                            position: relative;
                            padding: 0.75rem 1rem;
                            background: rgba(255, 165, 0, 0.15);
                            border: 1px solid rgba(255, 165, 0, 0.25);
                            border-radius: 8px;
                            color: #ffa500;
                            font-size: 0.9rem;
                            cursor: pointer;
                            transition: all 0.3s ease;
                            display: flex;
                            align-items: center;
                            justify-content: center;
                            min-width: 44px;
                        }
                        .call-price {
                            position: absolute;
                            bottom: 2px;
                            right: 4px;
                            font-size: 0.55rem;
                            color: rgba(255, 165, 0, 0.6);
                        }
                        .call-button:hover:not(:disabled) {
                            background: rgba(255, 165, 0, 0.25);
                            border-color: rgba(255, 165, 0, 0.4);
                        }
                        .call-button:disabled {
                            opacity: 0.5;
                            cursor: not-allowed;
                        }
                        .call-button.call-active {
                            background: rgba(255, 69, 0, 0.2);
                            border-color: rgba(255, 69, 0, 0.4);
                            color: #ff4500;
                            min-width: 100px;
                        }
                        .call-button.call-active:hover {
                            background: rgba(255, 69, 0, 0.3);
                        }
                        .call-button.call-connecting {
                            background: rgba(255, 165, 0, 0.1);
                            animation: pulse 1.5s infinite;
                        }
                        @keyframes pulse {
                            0%, 100% { opacity: 0.5; }
                            50% { opacity: 1; }
                        }
                        .dashboard-tabs {
                            display: flex;
                            gap: 1rem;
                            margin-bottom: 2rem;
                            border-bottom: 1px solid rgba(30, 144, 255, 0.1);
                            padding-bottom: 1rem;
                            flex-wrap: wrap;
                        }
                        .tab-button {
                            background: transparent;
                            border: none;
                            color: #999;
                            padding: 0.5rem 1rem;
                            cursor: pointer;
                            font-size: 1rem;
                            transition: all 0.3s ease;
                            position: relative;
                            white-space: nowrap;
                            flex: 1;
                            min-width: fit-content;
                        }
                        .tab-button::after {
                            content: '';
                            position: absolute;
                            bottom: -1rem;
                            left: 0;
                            width: 100%;
                            height: 2px;
                            background: transparent;
                            transition: background-color 0.3s ease;
                        }
                        .tab-button.active {
                            color: white;
                        }
                        .tab-button.active::after {
                            background: #1E90FF;
                        }
                        .tab-button:hover {
                            color: #7EB2FF;
                        }
                        .connections-tab, .controls-tab, .media-tab, .billing-tab, .settings-tab {
                            min-height: 400px;
                        }
                        .feature-status {
                            margin-top: 3rem;
                            text-align: left;
                            padding: 2rem;
                            background: rgba(30, 30, 30, 0.7);
                            border-radius: 12px;
                            border: 1px solid rgba(30, 144, 255, 0.1);
                            backdrop-filter: blur(10px);
                        }
                        .feature-status h4 {
                            color: #7EB2FF;
                            font-size: 0.9rem;
                            margin: 1rem 0 0.5rem 0;
                        }
                        .feature-status ul {
                            list-style: none;
                            padding: 0;
                            margin: 0 0 1.5rem 0;
                        }
                        .feature-status li {
                            color: #999;
                            font-size: 0.9rem;
                            padding: 0.3rem 0;
                            padding-left: 1.5rem;
                            position: relative;
                        }
                        .feature-status li::before {
                            content: '•';
                            position: absolute;
                            left: 0.5rem;
                            color: #1E90FF;
                        }
                        .feature-suggestion {
                            margin-top: 1rem;
                            color: #999;
                            font-size: 0.9rem;
                        }
                        .feature-suggestion a {
                            color: #1E90FF;
                            text-decoration: none;
                            transition: color 0.3s ease;
                        }
                        .feature-suggestion a:hover {
                            color: #7EB2FF;
                            text-decoration: underline;
                        }
                        .subscriber-promo {
                            margin-top: 1rem;
                            padding: 1rem;
                            background: rgba(30, 144, 255, 0.05);
                            border: 1px solid rgba(30, 144, 255, 0.1);
                            border-radius: 8px;
                            text-align: center;
                        }
                        .subscriber-promo p {
                            color: #fff;
                            margin-bottom: 0.5rem;
                        }
                        .subscriber-promo a {
                            color: #1E90FF;
                            text-decoration: none;
                        }
                        .subscriber-promo a:hover {
                            text-decoration: underline;
                        }
                        .development-links {
                            margin-top: 2rem;
                            font-size: 0.9rem;
                            color: #666;
                        }
                        .development-links p {
                            margin: 0.5rem 0;
                        }
                        .development-links a {
                            color: #007bff;
                            text-decoration: none;
                            position: relative;
                            padding: 0 2px;
                            transition: all 0.3s ease;
                        }
                        .development-links a::after {
                            content: '';
                            position: absolute;
                            width: 100%;
                            height: 1px;
                            bottom: -2px;
                            left: 0;
                            background: linear-gradient(90deg, #1E90FF, #4169E1);
                            transform: scaleX(0);
                            transform-origin: bottom right;
                            transition: transform 0.3s ease;
                        }
                        .development-links a:hover {
                            color: #7EB2FF;
                            text-shadow: 0 0 8px rgba(30, 144, 255, 0.3);
                        }
                        .development-links a:hover::after {
                            transform: scaleX(1);
                            transform-origin: bottom left;
                        }
                        .switch {
                            position: relative !important;
                            display: inline-block !important;
                            width: 60px !important;
                            height: 34px !important;
                            margin-left: 1rem !important;
                        }
                        .switch input {
                            opacity: 0 !important;
                            width: 0 !important;
                            height: 0 !important;
                        }
                        .slider {
                            position: absolute !important;
                            cursor: pointer !important;
                            top: 0 !important;
                            left: 0 !important;
                            right: 0 !important;
                            bottom: 0 !important;
                            background-color: #666 !important;
                            transition: .4s !important;
                            border-radius: 34px !important;
                            border: 1px solid rgba(255, 255, 255, 0.1) !important;
                        }
                        .slider:before {
                            position: absolute !important;
                            content: "" !important;
                            height: 26px !important;
                            width: 26px !important;
                            left: 4px !important;
                            bottom: 4px !important;
                            background-color: white !important;
                            transition: .4s !important;
                            border-radius: 50% !important;
                            box-shadow: 0 2px 5px rgba(0, 0, 0, 0.2) !important;
                        }
                        input:checked + .slider {
                            background-color: #1E90FF !important;
                        }
                        input:checked + .slider:before {
                            transform: translateX(26px) !important;
                        }
                        input:focus + .slider {
                            box-shadow: 0 0 1px rgba(30, 144, 255, 0.5) !important;
                        }
                        .slider.round {
                            border-radius: 34px !important;
                        }
                        .slider.round:before {
                            border-radius: 50% !important;
                        }
                        @media (max-width: 768px) {
                            .status-section {
                                flex-direction: column;
                                align-items: flex-start;
                            }
                            .subscription-promo {
                                margin: 1rem 0 0 0;
                                width: 100%;
                            }
                            .credit-item {
                                padding: 1rem;
                            }
                            .credit-tooltip {
                                position: fixed;
                                bottom: 20px;
                                left: 50%;
                                transform: translateX(-50%);
                                width: 90%;
                                max-width: 300px;
                                z-index: 1001;
                            }
                            .credit-tooltip::after {
                                display: none;
                            }
                            .dashboard-container {
                                padding: 2rem;
                            }
                            .panel-title {
                                font-size: 1.75rem;
                            }
                        }
                        @media (max-width: 480px) {
                            .dashboard-tabs {
                                gap: 0.5rem;
                                justify-content: center;
                            }
                            .tab-button {
                                padding: 0.5rem 0.75rem;
                                font-size: 0.9rem;
                            }
                        }
                    "#}
                </style>
            </>
        }
        }
    }
}
