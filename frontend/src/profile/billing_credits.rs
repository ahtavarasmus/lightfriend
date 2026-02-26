use yew::prelude::*;
use yew_router::prelude::Link;
use web_sys::HtmlInputElement;
use crate::utils::api::Api;
use crate::Route;
use serde_json::{Value, json};
use chrono::{Utc, Duration};
use crate::profile::billing_models::{ // Import from the new file
    AutoTopupSettings,
    BuyCreditsRequest,
    ApiResponse,
    UserProfile,
    UsageProjection,
    MIN_TOPUP_AMOUNT_CREDITS,
    RefundEligibilityResponse,
    RefundRequestResponse,
    ByotUsageResponse,
};
use wasm_bindgen_futures::spawn_local;
use gloo_timers::future::TimeoutFuture;
use wasm_bindgen::JsValue;
#[derive(Properties, PartialEq, Clone)]
pub struct BillingPageProps {
    pub user_profile: UserProfile,
}
#[function_component]
pub fn BillingPage(props: &BillingPageProps) -> Html {
    let user_profile_state = use_state(|| props.user_profile.clone());
    let user_profile = &*user_profile_state;
    let error = use_state(|| None::<String>);
    let success = use_state(|| None::<String>);
    // Auto top-up related states
    let show_auto_topup_modal = use_state(|| false);
    let auto_topup_active = use_state(|| user_profile.charge_when_under);
    let auto_topup_amount = use_state(|| user_profile.charge_back_to.unwrap_or(5.00));
    // State to track the saved auto-topup amount for display in "Currently:"
    let saved_auto_topup_amount = use_state(|| user_profile.charge_back_to.unwrap_or(5.00));
    // Buy credits related states
    let show_buy_credits_modal = use_state(|| false);
    let buy_credits_amount = use_state(|| 5.00);
    let show_confirmation_modal = use_state(|| false); // New state for confirmation modal
    let enable_auto_topup_with_purchase = use_state(|| true); // Checkbox for enabling auto top-up with first purchase

    // Usage projection state
    let usage_projection = use_state(|| None::<UsageProjection>);

    // BYOT usage state - for users with their own Twilio number
    let byot_usage = use_state(|| None::<ByotUsageResponse>);

    // State for cycling through quota display metrics (notifications, voice, messages, digests)
    let quota_display_index = use_state(|| 0_usize);

    // Refund-related states
    let refund_eligibility = use_state(|| None::<RefundEligibilityResponse>);
    let refund_loading = use_state(|| false);
    let refund_processing = use_state(|| false);
    let show_refund_confirm = use_state(|| false);

    // Fetch usage projection on mount (only once)
    {
        let usage_projection = usage_projection.clone();
        use_effect_with_deps(move |_| {
            spawn_local(async move {
                match Api::get("/api/pricing/usage-projection")
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.ok() {
                            if let Ok(data) = response.json::<UsageProjection>().await {
                                usage_projection.set(Some(data));
                            }
                        }
                    }
                    Err(e) => {
                        web_sys::console::log_1(&format!("Failed to fetch usage projection: {:?}", e).into());
                    }
                }
            });
            || ()
        }, ());
    }

    // Fetch BYOT usage on mount if user is BYOT
    {
        let byot_usage = byot_usage.clone();
        let is_byot = user_profile.plan_type.as_deref() == Some("byot");
        use_effect_with_deps(move |_| {
            if is_byot {
                spawn_local(async move {
                    match Api::get("/api/pricing/byot-usage")
                        .send()
                        .await
                    {
                        Ok(response) => {
                            if response.ok() {
                                if let Ok(data) = response.json::<ByotUsageResponse>().await {
                                    byot_usage.set(Some(data));
                                }
                            }
                        }
                        Err(e) => {
                            web_sys::console::log_1(&format!("Failed to fetch BYOT usage: {:?}", e).into());
                        }
                    }
                });
            }
            || ()
        }, ());
    }

    // Fetch refund eligibility on mount
    {
        let refund_eligibility = refund_eligibility.clone();
        let refund_loading = refund_loading.clone();
        use_effect_with_deps(move |_| {
            refund_loading.set(true);
            spawn_local(async move {
                match Api::get("/api/refund/eligibility")
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.ok() {
                            if let Ok(data) = response.json::<RefundEligibilityResponse>().await {
                                refund_eligibility.set(Some(data));
                            }
                        }
                    }
                    Err(e) => {
                        web_sys::console::log_1(&format!("Failed to fetch refund eligibility: {:?}", e).into());
                    }
                }
                refund_loading.set(false);
            });
            || ()
        }, ());
    }

    let one_time_credits = user_profile.credits;
    // Function to update auto top-up settings and refresh the profile
    let update_auto_topup = {
        let user_id = user_profile.id;
        let error = error.clone();
        let success = success.clone();
        let auto_topup_active = auto_topup_active.clone();
        let auto_topup_amount = auto_topup_amount.clone();
        let saved_auto_topup_amount = saved_auto_topup_amount.clone();
        let user_profile_state = user_profile_state.clone();
        let usage_projection = usage_projection.clone();

        Callback::from(move |settings: AutoTopupSettings| {
            let user_id = user_id;
            let error = error.clone();
            let success = success.clone();
            let auto_topup_active = auto_topup_active.clone();
            let auto_topup_amount = auto_topup_amount.clone();
            let saved_auto_topup_amount = saved_auto_topup_amount.clone();
            let user_profile_state = user_profile_state.clone();
            let usage_projection = usage_projection.clone();
            let settings = settings.clone();
           
            spawn_local(async move {
                // Update auto-topup settings
                match Api::post(&format!("/api/billing/update-auto-topup/{}", user_id))
                    .header("Content-Type", "application/json")
                    .json(&settings)
                    .expect("Failed to serialize auto top-up settings")
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.ok() {
                            if let Ok(data) = response.json::<ApiResponse>().await {
                                success.set(Some(data.message));
                                // Update local states immediately
                                auto_topup_active.set(settings.active);
                                if let Some(amount) = settings.amount {
                                    auto_topup_amount.set(amount);
                                    saved_auto_topup_amount.set(amount); // Update saved amount locally
                                }
                                // Fetch updated user profile to ensure server state matches
                                match Api::get("/api/profile")
                                    .send()
                                    .await
                                {
                                    Ok(profile_response) => {
                                        if profile_response.ok() {
                                            match profile_response.json::<UserProfile>().await {
                                                Ok(updated_profile) => {
                                                    user_profile_state.set(updated_profile.clone());
                                                    // Update saved amount with the server's value
                                                    if let Some(new_amount) = updated_profile.charge_back_to {
                                                        saved_auto_topup_amount.set(new_amount);
                                                    }
                                                    // Refresh usage projection to reflect new auto top-up status
                                                    web_sys::console::log_1(&"Fetching usage projection after auto top-up toggle".into());
                                                    match Api::get("/api/pricing/usage-projection")
                                                        .send()
                                                        .await
                                                    {
                                                        Ok(proj_response) => {
                                                            if proj_response.ok() {
                                                                match proj_response.json::<UsageProjection>().await {
                                                                    Ok(data) => {
                                                                        web_sys::console::log_1(&format!("Usage projection refreshed, has_auto_topup: {}", data.has_auto_topup).into());
                                                                        usage_projection.set(Some(data));
                                                                    }
                                                                    Err(e) => {
                                                                        web_sys::console::log_1(&format!("Failed to parse usage projection: {:?}", e).into());
                                                                    }
                                                                }
                                                            } else {
                                                                web_sys::console::log_1(&"Usage projection response not ok".into());
                                                            }
                                                        }
                                                        Err(e) => {
                                                            web_sys::console::log_1(&format!("Failed to fetch usage projection: {:?}", e).into());
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    error.set(Some(format!("Failed to parse updated profile: {:?}", e)));
                                                    // Clear error after 3 seconds
                                                    let error_clone = error.clone();
                                                    spawn_local(async move {
                                                        TimeoutFuture::new(3_000).await;
                                                        error_clone.set(None);
                                                    });
                                                }
                                            }
                                        } else {
                                            error.set(Some("Failed to refresh user profile".to_string()));
                                            // Clear error after 3 seconds
                                            let error_clone = error.clone();
                                            spawn_local(async move {
                                                TimeoutFuture::new(3_000).await;
                                                error_clone.set(None);
                                            });
                                        }
                                    }
                                    Err(e) => {
                                        error.set(Some(format!("Network error refreshing profile: {:?}", e)));
                                        // Clear error after 3 seconds
                                        let error_clone = error.clone();
                                        spawn_local(async move {
                                            TimeoutFuture::new(3_000).await;
                                            error_clone.set(None);
                                        });
                                    }
                                }
                                TimeoutFuture::new(3_000).await;
                                success.set(None); // Clear success message after 3 seconds
                            } else {
                                error.set(Some("Failed to parse response".to_string()));
                                // Clear error after 3 seconds
                                let error_clone = error.clone();
                                spawn_local(async move {
                                    TimeoutFuture::new(3_000).await;
                                    error_clone.set(None);
                                });
                            }
                        } else {
                            error.set(Some("Failed to update auto top-up settings".to_string()));
                            // Clear error after 3 seconds
                            let error_clone = error.clone();
                            spawn_local(async move {
                                TimeoutFuture::new(3_000).await;
                                error_clone.set(None);
                            });
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Network error occurred: {:?}", e)));
                        // Clear error after 3 seconds
                        let error_clone = error.clone();
                        spawn_local(async move {
                            TimeoutFuture::new(3_000).await;
                            error_clone.set(None);
                        });
                    }
                }
            });
        })
    };
    // Function to handle toggling the "Buy Credits" modal
    let toggle_buy_credits_modal = {
        let show_buy_credits_modal = show_buy_credits_modal.clone();
        Callback::from(move |_| show_buy_credits_modal.set(!*show_buy_credits_modal))
    };
    // Function to show confirmation modal before buying credits
    let show_confirmation = {
        let show_confirmation_modal = show_confirmation_modal.clone();
        let show_buy_credits_modal = show_buy_credits_modal.clone();
        Callback::from(move |_| {
            show_buy_credits_modal.set(false); // Close the buy credits modal
            show_confirmation_modal.set(true); // Show confirmation modal
        })
    };
    // Function to handle buying credits via Stripe Checkout
    let confirm_buy_credits = {
        let user_id = user_profile.id;
        let error = error.clone();
        let success = success.clone();
        let show_confirmation_modal = show_confirmation_modal.clone();
        let buy_credits_amount = buy_credits_amount.clone();
        let enable_auto_topup_with_purchase = enable_auto_topup_with_purchase.clone();
        let auto_topup_active = auto_topup_active.clone();

        Callback::from(move |_| {
            let user_id = user_id;
            let error = error.clone();
            let _success = success.clone();
            let show_confirmation_modal = show_confirmation_modal.clone();
            let buy_credits_amount = buy_credits_amount.clone();
            let enable_auto_topup = *enable_auto_topup_with_purchase && !*auto_topup_active;

            spawn_local(async move {
                // If auto top-up checkbox is checked, enable it first
                if enable_auto_topup {
                    let settings = AutoTopupSettings {
                        active: true,
                        amount: Some(*buy_credits_amount), // Use the same amount they're buying
                    };
                    let _ = Api::post(&format!("/api/billing/update-auto-topup/{}", user_id))
                        .header("Content-Type", "application/json")
                        .json(&settings)
                        .expect("Failed to serialize auto top-up settings")
                        .send()
                        .await;
                }

                let amount_dollars = *buy_credits_amount; // Safely dereference the cloned handle
                let request = BuyCreditsRequest { amount_dollars };
                match Api::post(&format!("/api/stripe/checkout-session/{}", user_id))
                    .header("Content-Type", "application/json")
                    .json(&request)
                    .expect("Failed to serialize buy credits request")
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.ok() {
                            if let Ok(data) = response.json::<Value>().await {
                                if let Some(url) = data.get("url").and_then(|v| v.as_str()) {
                                    // Redirect to Stripe Checkout
                                    web_sys::window()
                                        .unwrap()
                                        .location()
                                        .set_href(url)
                                        .unwrap_or_else(|e| {
                                            error.set(Some(format!("Failed to redirect to Stripe: {:?}", e)));
                                        });
                                    show_confirmation_modal.set(false); // Close confirmation modal
                                } else {
                                    error.set(Some("No URL in Stripe response".to_string()));
                                }
                            } else {
                                error.set(Some("Failed to parse Stripe response".to_string()));
                            }
                        } else {
                            // Check if this is an upgrade required error
                            if let Ok(data) = response.json::<Value>().await {
                                if data.get("upgrade_required").and_then(|v| v.as_bool()).unwrap_or(false) {
                                    error.set(Some("Credit top-ups are only available on the Digest plan. Upgrade to Digest for more credits and top-up ability.".to_string()));
                                } else if let Some(msg) = data.get("error").and_then(|v| v.as_str()) {
                                    error.set(Some(msg.to_string()));
                                } else {
                                    error.set(Some("Failed to create Stripe Checkout session".to_string()));
                                }
                            } else {
                                error.set(Some("Failed to create Stripe Checkout session".to_string()));
                            }
                        }
                        // Clear error after 3 seconds
                        let error_clone = error.clone();
                        spawn_local(async move {
                            TimeoutFuture::new(3_000).await;
                            error_clone.set(None);
                        });
                    }
                    Err(e) => {
                        error.set(Some(format!("Network error occurred: {:?}", e)));
                        // Clear error after 3 seconds
                        let error_clone = error.clone();
                        spawn_local(async move {
                            TimeoutFuture::new(3_000).await;
                            error_clone.set(None);
                        });
                    }
                }
            });
        })
    };
    // Handle redirect after successful payment
    let _handle_successful_payment = {
        let success = success.clone();
        let error = error.clone();
        use_effect_with_deps(move |_| {
            let window = web_sys::window().unwrap();
            let search = window.location().search().unwrap_or_default();
            let mut need_refresh = false;
            let session_id_opt = if search.contains("session_id=") {
                let sid = search.split("session_id=").nth(1)
                    .and_then(|s| s.split('&').next())
                    .unwrap_or_default()
                    .to_string();
                need_refresh = true;
                Some(sid)
            } else {
                None
            };
            if search.contains("subscription=success") || search.contains("subscription=changed") || search.contains("credits=success") {
                need_refresh = true;
            }
            if need_refresh {
                spawn_local(async move {
                    let mut refresh_success = true;
                    if let Some(session_id) = session_id_opt.clone() {
                        match Api::post("/api/stripe/confirm-checkout")
                            .header("Content-Type", "application/json")
                            .json(&json!({ "session_id": session_id }))
                            .expect("Failed to serialize session ID")
                            .send()
                            .await
                        {
                            Ok(response) => {
                                if response.ok() {
                                    if let Ok(data) = response.json::<ApiResponse>().await {
                                        success.set(Some(data.message));
                                    } else {
                                        error.set(Some("Failed to parse confirmation response".to_string()));
                                        refresh_success = false;
                                    }
                                } else {
                                    error.set(Some("Failed to confirm Stripe payment".to_string()));
                                    refresh_success = false;
                                }
                            }
                            Err(e) => {
                                error.set(Some(format!("Network error confirming payment: {:?}", e)));
                                refresh_success = false;
                            }
                        }
                    }
                    if refresh_success {
                        let message = if session_id_opt.is_some() {
                            "Credits added successfully! Reloading..."
                        } else {
                            "Subscription updated successfully! Reloading..."
                        };
                        success.set(Some(message.to_string()));
                        TimeoutFuture::new(10_000).await;
                        success.set(None);
                        let history = window.history().expect("no history");
                        history.replace_state_with_url(&JsValue::NULL, "", Some("/billing")).expect("replace state failed");
                        window.location().reload().expect("reload failed");
                    } else {
                        let error_clone = error.clone();
                        spawn_local(async move {
                            TimeoutFuture::new(10_000).await;
                            error_clone.set(None);
                        });
                    }
                });
            }
            || () // Cleanup function (none needed here)
        }, ())
    };
    // Function to open Stripe Customer Portal
    let open_customer_portal = {
        let user_id = user_profile.id;
        let error = error.clone();
        let success = success.clone();
        Callback::from(move |_| {
            let user_id = user_id;
            let error = error.clone();
            let success = success.clone();
            spawn_local(async move {
                match Api::get(&format!("/api/stripe/customer-portal/{}", user_id))
                    .header("Content-Type", "application/json")
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.ok() {
                            if let Ok(data) = response.json::<Value>().await {
                                if let Some(url) = data.get("url").and_then(|v| v.as_str()) {
                                    // Redirect to Stripe Customer Portal
                                    web_sys::window()
                                        .unwrap()
                                        .location()
                                        .set_href(url)
                                        .unwrap_or_else(|e| {
                                            error.set(Some(format!("Failed to redirect to Stripe Customer Portal: {:?}", e)));
                                        });
                                    success.set(Some("Redirecting to Stripe Customer Portal".to_string()));
                                } else {
                                    error.set(Some("No URL in Customer Portal response".to_string()));
                                }
                            } else {
                                error.set(Some("Failed to parse Customer Portal response".to_string()));
                            }
                        } else {
                            error.set(Some("Failed to create Customer Portal session".to_string()));
                        }
                        // Clear messages after 3 seconds
                        let error_clone = error.clone();
                        let success_clone = success.clone();
                        spawn_local(async move {
                            TimeoutFuture::new(3_000).await;
                            error_clone.set(None);
                            success_clone.set(None);
                        });
                    }
                    Err(e) => {
                        error.set(Some(format!("Network error occurred: {:?}", e)));
                        // Clear error after 3 seconds
                        let error_clone = error.clone();
                        spawn_local(async move {
                            TimeoutFuture::new(3_000).await;
                            error_clone.set(None);
                        });
                    }
                }
            });
        })
    };

    // Handle refund request
    let handle_refund_request = {
        let error = error.clone();
        let success = success.clone();
        let refund_processing = refund_processing.clone();
        let show_refund_confirm = show_refund_confirm.clone();
        let refund_eligibility = refund_eligibility.clone();

        Callback::from(move |_| {
            let error = error.clone();
            let success = success.clone();
            let refund_processing = refund_processing.clone();
            let show_refund_confirm = show_refund_confirm.clone();
            let refund_eligibility = refund_eligibility.clone();

            refund_processing.set(true);
            show_refund_confirm.set(false);

            spawn_local(async move {
                match Api::post("/api/refund/request")
                    .send()
                    .await
                {
                    Ok(response) => {
                        if let Ok(data) = response.json::<RefundRequestResponse>().await {
                            if data.success {
                                success.set(Some(data.message));
                                // Update eligibility to show already refunded
                                if let Some(mut elig) = (*refund_eligibility).clone() {
                                    elig.already_refunded = true;
                                    elig.eligible = false;
                                    elig.reason = "You have already received a refund.".to_string();
                                    refund_eligibility.set(Some(elig));
                                }
                            } else {
                                error.set(Some(data.message));
                            }
                        } else {
                            error.set(Some("Failed to process refund".to_string()));
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Network error: {:?}", e)));
                    }
                }
                refund_processing.set(false);
                // Clear messages after delay
                let error_clone = error.clone();
                let success_clone = success.clone();
                spawn_local(async move {
                    TimeoutFuture::new(5_000).await;
                    error_clone.set(None);
                    success_clone.set(None);
                });
            });
        })
    };

    html! {
        <>
        <div class="profile-info">
            <div class="billing-section">
                {
                    html! {
                        <>
                            // Usage Section - show BYOT usage for BYOT users, otherwise regular usage projection
                            {
                                if user_profile.sub_tier.is_some() {
                                // BYOT users get their own usage display
                                if user_profile.plan_type.as_deref() == Some("byot") {
                                    if let Some(byot_data) = (*byot_usage).as_ref() {
                                        // Build segments for the segmented bar (cost-based percentages)
                                        let mut segments: Vec<(&str, f32, &str, String)> = vec![];

                                        if byot_data.breakdown.digests.cost_eur > 0.0 {
                                            segments.push((
                                                "#9333ea", // purple
                                                byot_data.percentages.digests,
                                                "Digests",
                                                format!("{} ({:.2}€)", byot_data.breakdown.digests.count, byot_data.breakdown.digests.cost_eur)
                                            ));
                                        }
                                        if byot_data.breakdown.sms_notifications.cost_eur > 0.0 {
                                            segments.push((
                                                "#3b82f6", // blue
                                                byot_data.percentages.sms_notifications,
                                                "SMS Notifications",
                                                format!("{} ({:.2}€)", byot_data.breakdown.sms_notifications.count, byot_data.breakdown.sms_notifications.cost_eur)
                                            ));
                                        }
                                        if byot_data.breakdown.call_notifications.cost_eur > 0.0 {
                                            segments.push((
                                                "#f97316", // orange
                                                byot_data.percentages.call_notifications,
                                                "Call Notifications",
                                                format!("{} ({:.2}€)", byot_data.breakdown.call_notifications.count, byot_data.breakdown.call_notifications.cost_eur)
                                            ));
                                        }
                                        if byot_data.breakdown.messages.cost_eur > 0.0 {
                                            segments.push((
                                                "#22c55e", // green
                                                byot_data.percentages.messages,
                                                "Messages",
                                                format!("{} ({:.2}€)", byot_data.breakdown.messages.count, byot_data.breakdown.messages.cost_eur)
                                            ));
                                        }
                                        if byot_data.breakdown.voice_cost_eur > 0.0 {
                                            segments.push((
                                                "#ef4444", // red
                                                byot_data.percentages.voice,
                                                "Voice",
                                                format!("{:.1} min ({:.2}€)", byot_data.breakdown.voice_minutes, byot_data.breakdown.voice_cost_eur)
                                            ));
                                        }

                                        html! {
                                            <div class="section-container usage-projection-section">
                                                <div class="usage-projection-card">
                                                    <div class="usage-header">
                                                        <h3>{"BYOT Usage This Month"}</h3>
                                                        <div class="usage-summary">
                                                            <span class="total-cost" style="color: #7EB2FF; font-weight: 600; font-size: 1.2rem;">
                                                                {format!("{:.2}€", byot_data.total_cost_eur)}
                                                            </span>
                                                            <span style="color: #888; font-size: 0.85rem; margin-left: 8px;">
                                                                {"estimated Twilio costs"}
                                                            </span>
                                                        </div>
                                                    </div>

                                                    // Segmented usage bar
                                                    {
                                                        if !segments.is_empty() {
                                                            html! {
                                                                <div class="segmented-bar-container" style="margin: 16px 0;">
                                                                    <div class="segmented-bar" style="display: flex; height: 24px; border-radius: 4px; overflow: hidden; background: rgba(255,255,255,0.1);">
                                                                        { for segments.iter().map(|(color, pct, label, detail)| {
                                                                            let show_label = *pct >= 15.0;
                                                                            html! {
                                                                                <div
                                                                                    style={format!("width: {}%; background: {}; display: flex; align-items: center; justify-content: center; transition: width 0.3s;", pct, color)}
                                                                                    title={format!("{}: {}", label, detail)}
                                                                                >
                                                                                    { if show_label {
                                                                                        html! { <span style="color: white; font-size: 0.7rem; font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; padding: 0 4px;">{*label}</span> }
                                                                                    } else {
                                                                                        html! {}
                                                                                    }}
                                                                                </div>
                                                                            }
                                                                        })}
                                                                    </div>
                                                                    // Legend
                                                                    <div style="display: flex; flex-wrap: wrap; gap: 12px; margin-top: 12px;">
                                                                        { for segments.iter().map(|(color, _pct, label, detail)| {
                                                                            html! {
                                                                                <div style="display: flex; align-items: center; gap: 6px;">
                                                                                    <div style={format!("width: 12px; height: 12px; border-radius: 2px; background: {};", color)}></div>
                                                                                    <span style="color: #aaa; font-size: 0.8rem;">{format!("{}: {}", label, detail)}</span>
                                                                                </div>
                                                                            }
                                                                        })}
                                                                    </div>
                                                                </div>
                                                            }
                                                        } else {
                                                            html! {
                                                                <div style="padding: 16px; color: #666; text-align: center;">
                                                                    {"No usage recorded this month"}
                                                                </div>
                                                            }
                                                        }
                                                    }

                                                    // Footer info
                                                    <div class="plan-info-footer" style="margin-top: 12px; color: #888; font-size: 0.85rem; display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 8px;">
                                                        <span>
                                                            {format!("Based on Twilio pricing for {}", byot_data.country_name)}
                                                            {
                                                                if let Some(days) = byot_data.days_until_billing {
                                                                    format!(" ({} days until billing resets)", days)
                                                                } else {
                                                                    String::new()
                                                                }
                                                            }
                                                        </span>
                                                        <span style="color: #7EB2FF;">
                                                            <Link<Route> to={Route::TwilioHostedInstructions}>
                                                                {"Manage Twilio Settings"}
                                                            </Link<Route>>
                                                        </span>
                                                    </div>
                                                </div>
                                            </div>
                                        }
                                    } else {
                                        // Loading or no data
                                        html! {
                                            <div class="section-container usage-projection-section">
                                                <div class="usage-projection-card">
                                                    <div style="padding: 16px; color: #666; text-align: center;">
                                                        {"Loading usage data..."}
                                                    </div>
                                                </div>
                                            </div>
                                        }
                                    }
                                } else if let Some(projection) = (*usage_projection).as_ref() {
                                    let _percentage = projection.usage_percentage.min(100.0);
                                    let _bar_color = if projection.usage_percentage <= 60.0 {
                                        "#4CAF50" // Green
                                    } else if projection.usage_percentage <= 90.0 {
                                        "#FFC107" // Yellow
                                    } else {
                                        "#F44336" // Red
                                    };

                                    let plan_name = match projection.plan_type.as_deref() {
                                        Some("autopilot") => "Autopilot",
                                        Some("assistant") => "Assistant",
                                        Some("byot") => "BYOT",
                                        _ => "Assistant"
                                    };

                                    // Calculate messages per month for display
                                    let messages_per_month = (projection.avg_messages_per_day * 30.0).round() as i32;

                                    // Build segments for the bar (only non-zero values)
                                    let mut segments: Vec<(&str, f32, &str, String)> = vec![];

                                    if projection.digest_percentage > 0.0 {
                                        segments.push((
                                            "#9333ea", // purple
                                            projection.digest_percentage,
                                            "Digests",
                                            format!("{}/mo", projection.digests_per_month)
                                        ));
                                    }
                                    if projection.sms_noti_percentage > 0.0 {
                                        let sms_per_month = (projection.avg_sms_notifications_per_day * 30.0).round() as i32;
                                        segments.push((
                                            "#3b82f6", // blue
                                            projection.sms_noti_percentage,
                                            "SMS Notifications",
                                            format!("~{}/mo", sms_per_month)
                                        ));
                                    }
                                    if projection.call_noti_percentage > 0.0 {
                                        let call_per_month = (projection.avg_call_notifications_per_day * 30.0).round() as i32;
                                        segments.push((
                                            "#f97316", // orange
                                            projection.call_noti_percentage,
                                            "Call Notifications",
                                            format!("~{}/mo", call_per_month)
                                        ));
                                    }
                                    if projection.messages_percentage > 0.0 {
                                        segments.push((
                                            "#22c55e", // green
                                            projection.messages_percentage,
                                            "Messages",
                                            format!("~{}/mo", messages_per_month)
                                        ));
                                    }

                                    // Calculate remaining capacity as actionable units
                                    let remaining_messages = projection.remaining_capacity / 3; // 1 message = 3 notification units
                                    let remaining_notifications = projection.remaining_capacity;
                                    let remaining_voice_mins = (projection.remaining_capacity as f32 * 0.67).round() as i32; // rough estimate

                                    // Check if user should downgrade (Digest plan, usage <= 40)
                                    let should_suggest_downgrade = projection.plan_type.as_deref() == Some("autopilot")
                                        && projection.total_usage_per_month <= 40
                                        && projection.overage.is_none();

                                    // === ACTUAL QUOTA DISPLAY - Build list of available metrics ===
                                    // Each metric: (label, used, capacity)
                                    let mut quota_metrics: Vec<(&str, i32, i32)> = vec![
                                        ("notifications", projection.actual_notifications_used, projection.plan_capacity * 3), // plan_capacity is in units, notifications are 3x
                                    ];

                                    // Only add voice if user has voice usage
                                    if projection.actual_voice_mins_used > 0 || projection.avg_voice_mins_per_day > 0.0 {
                                        // Voice capacity is roughly plan_capacity * 2 (since voice is ~1.5 units per minute)
                                        let voice_capacity = projection.plan_capacity * 2;
                                        quota_metrics.push(("voice minutes", projection.actual_voice_mins_used, voice_capacity));
                                    }

                                    // Only add messages if NOT notification-only country
                                    if !projection.is_notification_only {
                                        // Messages capacity is roughly plan_capacity (1 message = 1 unit)
                                        let messages_capacity = projection.plan_capacity;
                                        quota_metrics.push(("messages", projection.actual_messages_used, messages_capacity));
                                    }

                                    // Only add digests if user has active digests
                                    if projection.digest_count > 0 {
                                        // Digests capacity is plan_capacity (1 digest = 1 unit)
                                        let digests_capacity = projection.plan_capacity;
                                        quota_metrics.push(("digests", projection.actual_digests_used, digests_capacity));
                                    }

                                    let current_index = *quota_display_index % quota_metrics.len().max(1);
                                    let (metric_label, metric_used, metric_capacity) = quota_metrics.get(current_index)
                                        .copied()
                                        .unwrap_or(("notifications", 0, projection.plan_capacity * 3));
                                    let metric_remaining = (metric_capacity - metric_used).max(0);
                                    let metric_percentage = if metric_capacity > 0 {
                                        (metric_remaining as f32 / metric_capacity as f32) * 100.0
                                    } else {
                                        100.0
                                    };

                                    // Color based on percentage remaining
                                    let quota_color = if metric_percentage >= 25.0 {
                                        "#4ade80" // green
                                    } else if metric_percentage >= 10.0 {
                                        "#fbbf24" // yellow
                                    } else {
                                        "#ef4444" // red
                                    };

                                    // Click handler to cycle through metrics
                                    let metrics_len = quota_metrics.len();
                                    let cycle_quota_metric = {
                                        let quota_display_index = quota_display_index.clone();
                                        Callback::from(move |_: web_sys::MouseEvent| {
                                            quota_display_index.set((*quota_display_index + 1) % metrics_len.max(1));
                                        })
                                    };

                                    html! {
                                        <div class="section-container usage-projection-section">
                                            // Remaining usage section - its own card above the projection
                                            <div class="usage-projection-card" style="margin-bottom: 16px;">
                                                <div class="usage-header">
                                                    <h3>{"Remaining This Month"}</h3>
                                                </div>
                                                <div
                                                    style={format!("padding: 8px 12px; background: {}1a; border-radius: 6px; cursor: pointer;", quota_color)}
                                                    onclick={cycle_quota_metric}
                                                >
                                                    <span style={format!("color: {}; font-weight: 500;", quota_color)}>
                                                        {format!("{} of {} ", metric_used, metric_capacity)}
                                                    </span>
                                                    <span style={format!("color: {}; font-weight: 500; text-decoration: underline;", quota_color)}>
                                                        {metric_label}
                                                    </span>
                                                    <span style={format!("color: {}; font-weight: 500;", quota_color)}>
                                                        {" used"}
                                                    </span>
                                                    {
                                                        if quota_metrics.len() > 1 {
                                                            html! {
                                                                <span style="color: #888; font-size: 0.8rem; margin-left: 8px;">
                                                                    {"(click to see equivalents)"}
                                                                </span>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }
                                                    }
                                                </div>
                                                // Quota reset date
                                                {
                                                    if let Some(days) = projection.days_until_billing {
                                                        let billing_date = chrono::Utc::now() + chrono::Duration::days(days as i64);
                                                        let formatted_date = billing_date.format("%B %d, %Y").to_string();
                                                        html! {
                                                            <div style="margin-top: 8px; color: #888; font-size: 0.85rem;">
                                                                {"Quota resets on "}
                                                                <span style="color: #ccc; font-weight: 500;">{formatted_date}</span>
                                                                {format!(" ({} days)", days)}
                                                            </div>
                                                        }
                                                    } else {
                                                        html! {}
                                                    }
                                                }
                                            </div>

                                            // Projected usage card
                                            <div class="usage-projection-card">
                                                <div class="usage-header">
                                                    <h3>
                                                        {"Monthly Projected Usage"}
                                                        {
                                                            if projection.is_example_data {
                                                                html! { <span class="example-badge">{"(est.)"}</span> }
                                                            } else {
                                                                html! {}
                                                            }
                                                        }
                                                    </h3>
                                                    <span class="usage-percentage">{format!("{:.0}%", projection.usage_percentage)}</span>
                                                </div>

                                                // Explanation text
                                                <div style="margin-bottom: 12px; color: #888; font-size: 0.8rem;">
                                                    {
                                                        if projection.digest_count > 0 {
                                                            format!("Based on your average usage pattern and {} scheduled digest{}",
                                                                projection.digest_count,
                                                                if projection.digest_count == 1 { "" } else { "s" })
                                                        } else {
                                                            "Based on your average usage pattern".to_string()
                                                        }
                                                    }
                                                </div>

                                                // Segmented progress bar
                                                <div class="usage-bar-container" style="position: relative; height: 24px; background: rgba(255,255,255,0.1); border-radius: 4px; overflow: hidden;">
                                                    <div style="display: flex; height: 100%;">
                                                        {
                                                            segments.iter().map(|(color, pct, label, value)| {
                                                                let width = if projection.usage_percentage > 100.0 {
                                                                    // Scale down if over 100%
                                                                    pct * 100.0 / projection.usage_percentage
                                                                } else {
                                                                    *pct
                                                                };
                                                                // Show inline text if segment is wide enough (>15%)
                                                                let show_inline_label = width > 15.0;
                                                                html! {
                                                                    <div
                                                                        class="segment-bar"
                                                                        style={format!("width: {}%; background-color: {}; position: relative; cursor: pointer; display: flex; align-items: center; justify-content: center; overflow: hidden;", width, color)}
                                                                        title={format!("{}: {}", label, value)}
                                                                    >
                                                                        {
                                                                            if show_inline_label {
                                                                                html! {
                                                                                    <span style="color: white; font-size: 11px; font-weight: 500; text-shadow: 0 1px 2px rgba(0,0,0,0.5); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; padding: 0 4px;">
                                                                                        {*label}
                                                                                    </span>
                                                                                }
                                                                            } else {
                                                                                html! {}
                                                                            }
                                                                        }
                                                                    </div>
                                                                }
                                                            }).collect::<Html>()
                                                        }
                                                        {
                                                            // Show overflow section if over 100%
                                                            if projection.usage_percentage > 100.0 {
                                                                let overflow_pct = ((projection.usage_percentage - 100.0) / projection.usage_percentage) * 100.0;
                                                                let overage_euros = projection.overage.as_ref().map(|o| o.estimated_cost_euros).unwrap_or(0.0);
                                                                let show_inline = overflow_pct > 12.0; // Show text if segment is wide enough
                                                                html! {
                                                                    <div
                                                                        style={format!("width: {}%; background-color: #ef4444; display: flex; align-items: center; justify-content: center;", overflow_pct)}
                                                                        title={format!("Over limit: ~{:.0}€/mo", overage_euros)}
                                                                    >
                                                                        {
                                                                            if show_inline {
                                                                                html! {
                                                                                    <span style="color: white; font-size: 11px; font-weight: 500; text-shadow: 0 1px 2px rgba(0,0,0,0.5); white-space: nowrap;">
                                                                                        {format!("~{:.0}€", overage_euros)}
                                                                                    </span>
                                                                                }
                                                                            } else {
                                                                                html! {}
                                                                            }
                                                                        }
                                                                    </div>
                                                                }
                                                            } else {
                                                                html! {}
                                                            }
                                                        }
                                                    </div>
                                                </div>

                                                // Remaining capacity or overage info
                                                <div class="capacity-info" style="margin-top: 12px;">
                                                    {
                                                        if projection.remaining_capacity > 0 {
                                                            if projection.is_notification_only {
                                                                html! {
                                                                    <div style="color: #4ade80; font-size: 0.9rem;">
                                                                        {format!("Room for ~{} more notifications this month", remaining_notifications)}
                                                                    </div>
                                                                }
                                                            } else {
                                                                html! {
                                                                    <div style="color: #4ade80; font-size: 0.9rem;">
                                                                        {format!("Room for ~{} more messages, ~{} notifications, or ~{} voice mins", remaining_messages, remaining_notifications, remaining_voice_mins)}
                                                                    </div>
                                                                }
                                                            }
                                                        } else if let Some(overage) = &projection.overage {
                                                            if overage.covered_by_auto_topup {
                                                                html! {
                                                                    <div style="color: #4ade80; font-size: 0.9rem;">
                                                                        {format!("Estimated extra: ~{:.0}EUR/month (covered by auto top-up)", overage.estimated_cost_euros)}
                                                                    </div>
                                                                }
                                                            } else if projection.plan_type.as_deref() == Some("byot") || user_profile.plan_type.as_deref() == Some("byot") {
                                                                // BYOT plan users pay Twilio directly
                                                                html! {
                                                                    <div style="color: #fbbf24; font-size: 0.9rem;">
                                                                        {"BYOT plan - you pay Twilio directly for messaging costs"}
                                                                    </div>
                                                                }
                                                            } else if projection.overage_credits > 0.0 {
                                                                // Has overage credits but no auto top-up - show run-out date
                                                                let days_remaining = projection.overage_days_remaining.unwrap_or(0);
                                                                let days_until_billing = projection.days_until_billing.unwrap_or(30);
                                                                let run_out_date = Utc::now() + Duration::days(days_remaining as i64);
                                                                let formatted_date = run_out_date.format("%b %d").to_string();

                                                                if days_remaining >= days_until_billing {
                                                                    // Credits last through billing cycle - show how long they'll last
                                                                    let months_covered = (days_remaining as f32 / 30.0).floor() as i32;
                                                                    let coverage_text = if months_covered >= 12 {
                                                                        format!("~{}+ months", months_covered)
                                                                    } else if months_covered > 1 {
                                                                        format!("~{} months", months_covered)
                                                                    } else {
                                                                        "this month".to_string()
                                                                    };
                                                                    html! {
                                                                        <div style="color: #4ade80; font-size: 0.9rem;">
                                                                            {format!("Your {:.2}€ credits cover {} of overage, running out ~{}", projection.overage_credits, coverage_text, formatted_date)}
                                                                        </div>
                                                                    }
                                                                } else {
                                                                    // Credits will run out before billing cycle
                                                                    html! {
                                                                        <div style="color: #fbbf24; font-size: 0.9rem;">
                                                                            {format!("Credits ({:.2}€) projected to run out ~{} - buy more or enable auto top-up", projection.overage_credits, formatted_date)}
                                                                        </div>
                                                                    }
                                                                }
                                                            } else {
                                                                html! {
                                                                    <div style="color: #fbbf24; font-size: 0.9rem;">
                                                                        {format!("~{:.0}EUR over limit - buy credits or enable auto top-up", overage.estimated_cost_euros)}
                                                                    </div>
                                                                }
                                                            }
                                                        } else if projection.has_auto_topup {
                                                            // At ~100% but no overage yet, and auto top-up is enabled
                                                            html! {
                                                                <div style="color: #4ade80; font-size: 0.9rem;">
                                                                    {"At quota limit - any extra usage covered by auto top-up"}
                                                                </div>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }
                                                    }
                                                </div>

                                                // Downgrade suggestion
                                                {
                                                    if should_suggest_downgrade {
                                                        html! {
                                                            <div style="margin-top: 12px; padding: 10px; background: rgba(74, 222, 128, 0.1); border: 1px solid rgba(74, 222, 128, 0.3); border-radius: 6px;">
                                                                <span style="color: #4ade80; font-size: 0.9rem;">
                                                                    {"Your usage fits the Monitor Plan - downgrade to save 20EUR/month"}
                                                                </span>
                                                            </div>
                                                        }
                                                    } else {
                                                        html! {}
                                                    }
                                                }

                                                // Example data info
                                                {
                                                    if projection.is_example_data {
                                                        html! {
                                                            <div style="margin-top: 12px; color: #666; font-size: 0.8rem; font-style: italic;">
                                                                {"Based on typical usage. Actual data appears after a few days of activity."}
                                                            </div>
                                                        }
                                                    } else {
                                                        html! {}
                                                    }
                                                }

                                                // Plan info footer
                                                <div class="plan-info-footer" style="margin-top: 12px; color: #888; font-size: 0.85rem;">
                                                    <span>{format!("{} Plan: {}/mo capacity", plan_name, projection.plan_capacity)}</span>
                                                    {
                                                        if let Some(days) = projection.days_until_billing {
                                                            html! { <span>{format!(" ({} days left)", days)}</span> }
                                                        } else {
                                                            html! {}
                                                        }
                                                    }
                                                </div>
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

                            // Purchased Credits & Payment Management - unified card style
                            <div class="usage-projection-card" style={format!("margin-top: 16px;{}", if user_profile.plan_type.as_deref() == Some("byot") { " opacity: 0.6;" } else { "" })}>
                                <div class="usage-header">
                                    <h3>{"Overage Credits"}</h3>
                                    <span class="usage-percentage" style="font-size: 1.2rem;">{format!("{:.2}€", one_time_credits)}</span>
                                </div>
                                <div style="margin-bottom: 12px; color: #888; font-size: 0.8rem;">
                                    {"One-time purchased credits that don't expire. Used for voice calls, messages, or notifications when monthly quota is exhausted."}
                                </div>

                                <div class="auto-topup-container" style="margin-top: 12px; padding: 0;">
                                {
                                    if user_profile.plan_type.as_deref() == Some("byot") {
                                        // BYOT users don't need overage credits - they pay Twilio directly
                                        html! {
                                            <>
                                                <div class="buy-credits-disabled">
                                                    <button
                                                        class="buy-credits-button disabled"
                                                        title="Not needed on BYOT plan"
                                                        disabled=true
                                                        style="opacity: 0.5; cursor: not-allowed;"
                                                    >
                                                        {"Buy Credits"}
                                                    </button>
                                                </div>
                                                <div class="tooltip" style="color: #888; font-size: 0.85rem;">
                                                    {"On the BYOT plan, you pay Twilio directly for usage - no overage credits needed. Your existing credits will be available if you switch plans."}
                                                </div>
                                            </>
                                        }
                                    } else if user_profile.sub_tier.is_some() || user_profile.discount {
                                        html! {
                                            <>
                                                if user_profile.stripe_payment_method_id.is_some() {
                                                    <button
                                                        class="auto-topup-button"
                                                        onclick={{
                                                            let show_modal = show_auto_topup_modal.clone();
                                                            Callback::from(move |_| show_modal.set(!*show_modal))
                                                        }}
                                                    >
                                                        {"Automatic Top-up"}
                                                    </button>
                                                }
                                                <button
                                                    class="buy-credits-button"
                                                    onclick={toggle_buy_credits_modal.clone()}
                                                >
                                                    {"Buy Credits"}
                                                </button>
                                            </>
                                        }
                                    } else {
                                        html! {
                                            <>
                                            <div class="buy-credits-disabled">
                                                <button
                                                    class="buy-credits-button disabled"
                                                    title="Subscribe to enable credit purchases"
                                                    disabled=true
                                                >
                                                    {"Buy Credits"}
                                                </button>

                                            </div>
                                            <div class="tooltip">
                                                    {"Subscribe to a plan to enable overage credit purchases. Overage credits allow you to make more voice calls and send more messages even after your quota is used."}
                                                </div>
                                                    </>
                                        }
                                    }
                                }
                                {
                                    if *show_auto_topup_modal {
                                        html! {
                                            <div class="auto-topup-modal">
                                                <div class="auto-topup-toggle">
                                                    <span>{"Automatic Top-up"}</span>
                                                    <span class="toggle-status">
                                                        {if *auto_topup_active {"Active"} else {"Inactive"}}
                                                    </span>
                                                    <label class="switch">
                                                        <input
                                                            type="checkbox"
                                                            checked={*auto_topup_active}
                                                            onchange={{
                                                                let auto_topup_active = auto_topup_active.clone();
                                                                let update_auto_topup = update_auto_topup.clone();
                                                                let auto_topup_amount = auto_topup_amount.clone();
                                                                Callback::from(move |e: Event| {
                                                                    let input: HtmlInputElement = e.target_unchecked_into();
                                                                    let new_active_state = input.checked();
                                                                    auto_topup_active.set(new_active_state);
                                                                    update_auto_topup.emit(AutoTopupSettings {
                                                                        active: new_active_state,
                                                                        amount: Some(*auto_topup_amount),
                                                                    });
                                                                })
                                                            }}
                                                        />
                                                        <span class="slider round"></span>
                                                    </label>
                                                </div>
                                               
                                                <div class="current-balance">
                                                    <span>{"Currently: "}</span>
                                                    <span class="balance-amount">{format!("${:.2}", *saved_auto_topup_amount)}</span>
                                                </div>
                                               
                                                {
                                                    if *auto_topup_active {
                                                        html! {
                                                            <div class="topup-settings">
                                                                <p>{"How much would you like to automatically top up when your purchased credits drop below $2.00?"}</p>
                                                                <div class="amount-input-container">
                                                                    <label for="amount">{"Amount ($)"}</label>
                                                                    <input
                                                                        id="amount"
                                                                        type="number"
                                                                        step="0.01"
                                                                        min="5"
                                                                        class="amount-input"
                                                                        value="" // Default to empty
                                                                        onchange={{
                                                                            let auto_topup_amount = auto_topup_amount.clone();
                                                                            let error = error.clone();
                                                                            Callback::from(move |e: Event| {
                                                                                let input: HtmlInputElement = e.target_unchecked_into();
                                                                                if let Ok(dollars) = input.value().parse::<f32>() {
                                                                                    // Enforce minimum of $5
                                                                                    let final_dollars = dollars.max(MIN_TOPUP_AMOUNT_CREDITS);
                                                                                    if dollars < MIN_TOPUP_AMOUNT_CREDITS {
                                                                                        error.set(Some("Minimum amount is $5".to_string()));
                                                                                        // Clear error after 3 seconds
                                                                                        let error_clone = error.clone();
                                                                                        spawn_local(async move {
                                                                                            TimeoutFuture::new(3_000).await;
                                                                                            error_clone.set(None);
                                                                                        });
                                                                                    }
                                                                                    // Convert dollars to credits credits
                                                                                    auto_topup_amount.set(final_dollars);
                                                                                    // Update the input value to reflect the enforced minimum
                                                                                    input.set_value(&format!("{:.2}", final_dollars));
                                                                                } else {
                                                                                    // If parsing fails (e.g., empty or invalid input), set to minimum
                                                                                    auto_topup_amount.set(MIN_TOPUP_AMOUNT_CREDITS);
                                                                                    input.set_value(&format!("{:.2}", MIN_TOPUP_AMOUNT_CREDITS));
                                                                                }
                                                                            })
                                                                        }}
                                                                    />
                                                                </div>
                                                                <button
                                                                    class="save-button"
                                                                    onclick={{
                                                                        let update_auto_topup = update_auto_topup.clone();
                                                                        let auto_topup_active = auto_topup_active.clone();
                                                                        let auto_topup_amount = auto_topup_amount.clone();
                                                                        Callback::from(move |_| {
                                                                            update_auto_topup.emit(AutoTopupSettings {
                                                                                active: *auto_topup_active,
                                                                                amount: Some(*auto_topup_amount),
                                                                            });
                                                                        })
                                                                    }}
                                                                >
                                                                    {"Save"}
                                                                </button>
                                                               
                                                                {
                                                                    if let Some(error_msg) = (*error).as_ref() {
                                                                        html! {
                                                                            <div class="message error-message" style="margin-top: 1rem;">
                                                                                {error_msg}
                                                                            </div>
                                                                        }
                                                                    } else {
                                                                        html! {}
                                                                    }
                                                                }
                                                            </div>
                                                        }
                                                    } else {
                                                        html! {}
                                                    }
                                                }
                                            </div>
                                        }
                                    } else {
                                        html! {}
                                    }
                                }
                                {
                                    if *show_buy_credits_modal {
                                        html! {
                                            <div class="buy-credits-modal">
                                                <h3>{"How many credits would you like to buy?"}</h3>
                                                <div class="amount-input-container">
                                                    <label for="credits-amount">{"Amount ($)"}</label>
                                                    <input
                                                        id="credits-amount"
                                                        type="number"
                                                        step="0.01"
                                                        min="3"
                                                        class="amount-input"
                                                        value={format!("{:.2}", *buy_credits_amount)}
                                                        onchange={{
                                                            let buy_credits_amount = buy_credits_amount.clone();
                                                            let error = error.clone();
                                                            Callback::from(move |e: Event| {
                                                                let input: HtmlInputElement = e.target_unchecked_into();
                                                                if let Ok(dollars) = input.value().parse::<f32>() {
                                                                    // Enforce minimum of $5
                                                                    let final_dollars = dollars.max(MIN_TOPUP_AMOUNT_CREDITS);
                                                                    if dollars < MIN_TOPUP_AMOUNT_CREDITS {
                                                                        error.set(Some("Minimum amount is $3".to_string()));
                                                                        // Clear error after 3 seconds
                                                                        let error_clone = error.clone();
                                                                        spawn_local(async move {
                                                                            TimeoutFuture::new(3_000).await;
                                                                            error_clone.set(None);
                                                                        });
                                                                    }
                                                                    buy_credits_amount.set(final_dollars);
                                                                    // Update the input value to reflect the enforced minimum
                                                                    input.set_value(&format!("{:.2}", final_dollars));
                                                                } else {
                                                                    // If parsing fails (e.g., empty or invalid input), set to minimum
                                                                    buy_credits_amount.set(MIN_TOPUP_AMOUNT_CREDITS);
                                                                    input.set_value(&format!("{:.2}", MIN_TOPUP_AMOUNT_CREDITS));
                                                                }
                                                            })
                                                        }}
                                                    />
                                                </div>
                                                // Only show auto top-up checkbox if not already enabled
                                                {
                                                    if !*auto_topup_active {
                                                        html! {
                                                            <div class="auto-topup-checkbox" style="margin-top: 1rem; display: flex; align-items: center; gap: 0.5rem;">
                                                                <input
                                                                    type="checkbox"
                                                                    id="enable-auto-topup"
                                                                    checked={*enable_auto_topup_with_purchase}
                                                                    onchange={{
                                                                        let enable_auto_topup_with_purchase = enable_auto_topup_with_purchase.clone();
                                                                        Callback::from(move |e: Event| {
                                                                            let input: HtmlInputElement = e.target_unchecked_into();
                                                                            enable_auto_topup_with_purchase.set(input.checked());
                                                                        })
                                                                    }}
                                                                    style="width: 18px; height: 18px; cursor: pointer;"
                                                                />
                                                                <label for="enable-auto-topup" style="color: #ccc; font-size: 0.9rem; cursor: pointer;">
                                                                    {"Enable automatic top-up (refill when credits run low)"}
                                                                </label>
                                                            </div>
                                                        }
                                                    } else {
                                                        html! {}
                                                    }
                                                }
                                                <div class="modal-actions">
                                                    <button
                                                        class="cancel-button"
                                                        onclick={toggle_buy_credits_modal.clone()}
                                                    >
                                                        {"Cancel"}
                                                    </button>
                                                    <button
                                                        class="buy-now-button"
                                                        onclick={show_confirmation.clone()}
                                                    >
                                                        {"Buy Now"}
                                                    </button>
                                                </div>
                                                {
                                                    if let Some(error_msg) = (*error).as_ref() {
                                                        html! {
                                                            <div class="message error-message" style="margin-top: 1rem;">
                                                                {error_msg}
                                                            </div>
                                                        }
                                                    } else {
                                                        html! {}
                                                    }
                                                }
                                            </div>
                                        }
                                    } else {
                                        html! {}
                                    }
                                }
                                {
                                    if *show_confirmation_modal {
                                        html! {
                                            <div class="confirmation-modal">
                                                <h3>{"Confirm Purchase"}</h3>
                                                <p>{format!("Are you sure you want to buy ${:.2} in credits?", *buy_credits_amount)}</p>
                                                <div class="modal-actions">
                                                    <button
                                                        class="cancel-button"
                                                        onclick={{
                                                            let show_confirmation_modal = show_confirmation_modal.clone();
                                                            Callback::from(move |_| show_confirmation_modal.set(false))
                                                        }}
                                                    >
                                                        {"Cancel"}
                                                    </button>
                                                    <button
                                                        class="confirm-button"
                                                        onclick={confirm_buy_credits.clone()}
                                                    >
                                                        {"Confirm"}
                                                    </button>
                                                </div>
                                                {
                                                    if let Some(error_msg) = (*error).as_ref() {
                                                        html! {
                                                            <div class="message error-message" style="margin-top: 1rem;">
                                                                {error_msg}
                                                            </div>
                                                        }
                                                    } else {
                                                        html! {}
                                                    }
                                                }
                                            </div>
                                        }
                                    } else {
                                        html! {}
                                    }
                                }
                                </div>
                            </div>
                            if user_profile.stripe_payment_method_id.is_some() || user_profile.sub_tier.is_some() {
                                <button
                                    class="customer-portal-button"
                                    onclick={open_customer_portal.clone()}
                                    style="margin-top: 16px;"
                                >
                                    {"Manage Payments"}
                                </button>
                            }
                        </>
                    }
                }
                <div class="billing-info">
                    //<PaymentMethodButton user_id={user_profile.id} />
                </div>
                //<UsageGraph user_id={user_profile.id} />

                // Refund Section
                <div class="refund-section">
                    <h3 class="refund-title">{"Refund"}</h3>
                    {
                        if *refund_loading {
                            html! {
                                <div class="refund-loading">
                                    {"Checking refund eligibility..."}
                                </div>
                            }
                        } else if let Some(elig) = (*refund_eligibility).as_ref() {
                            html! {
                                <div class="refund-content">
                                    {
                                        if elig.already_refunded {
                                            html! {
                                                <div class="refund-status refund-ineligible">
                                                    <p class="refund-reason">{&elig.reason}</p>
                                                </div>
                                            }
                                        } else if elig.eligible {
                                            let refund_amount = elig.refund_amount_cents.map(|c| c as f32 / 100.0).unwrap_or(0.0);
                                            let _refund_type = elig.refund_type.as_deref().unwrap_or("subscription");
                                            let days_left = elig.days_remaining.unwrap_or(0);
                                            let _usage = elig.usage_percent.unwrap_or(0.0);

                                            html! {
                                                <div class="refund-status refund-eligible">
                                                    <div class="refund-info">
                                                        <p class="refund-reason">{&elig.reason}</p>
                                                        <p class="refund-details">
                                                            {format!("Refund amount: €{:.2}", refund_amount)}
                                                        </p>
                                                        <p class="refund-details">
                                                            {format!("{} days remaining in refund window", days_left)}
                                                        </p>
                                                    </div>
                                                    {
                                                        if *show_refund_confirm {
                                                            let show_refund_confirm = show_refund_confirm.clone();
                                                            let handle_refund = handle_refund_request.clone();
                                                            html! {
                                                                <div class="refund-confirm">
                                                                    <p>{"Are you sure you want to request a refund? This action cannot be undone."}</p>
                                                                    <div class="refund-confirm-buttons">
                                                                        <button
                                                                            class="refund-cancel-btn"
                                                                            onclick={Callback::from(move |_| show_refund_confirm.set(false))}
                                                                        >
                                                                            {"Cancel"}
                                                                        </button>
                                                                        <button
                                                                            class="refund-confirm-btn"
                                                                            onclick={handle_refund}
                                                                            disabled={*refund_processing}
                                                                        >
                                                                            {if *refund_processing { "Processing..." } else { "Yes, Refund" }}
                                                                        </button>
                                                                    </div>
                                                                </div>
                                                            }
                                                        } else {
                                                            let show_refund_confirm = show_refund_confirm.clone();
                                                            html! {
                                                                <button
                                                                    class="refund-btn"
                                                                    onclick={Callback::from(move |_| show_refund_confirm.set(true))}
                                                                >
                                                                    {"Request Refund"}
                                                                </button>
                                                            }
                                                        }
                                                    }
                                                </div>
                                            }
                                        } else {
                                            html! {
                                                <div class="refund-status refund-ineligible">
                                                    <p class="refund-reason">{&elig.reason}</p>
                                                    {
                                                        if let Some(usage) = elig.usage_percent {
                                                            html! {
                                                                <p class="refund-details">{format!("Usage: {:.0}%", usage)}</p>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }
                                                    }
                                                    <p class="refund-contact">
                                                        {"Questions? Contact "}
                                                        <a href={format!("mailto:{}", &elig.contact_email)}>{&elig.contact_email}</a>
                                                    </p>
                                                </div>
                                            }
                                        }
                                    }
                                </div>
                            }
                        } else {
                            html! {
                                <div class="refund-status">
                                    <p>{"Unable to check refund eligibility."}</p>
                                </div>
                            }
                        }
                    }
                </div>
            </div>
        </div>
        <style>
                {r#"
/* Section Containers */
.section-container {
    margin-bottom: 2rem;
}
.section-header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 1rem;
}
.section-header h3 {
    margin: 0;
}
/* Credits Display Containers */
.credits-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 2rem;
    margin-top: 2rem;
    animation: fadeInUp 0.6s ease-out forwards;
}
@keyframes fadeInUp {
    from {
        opacity: 0;
        transform: translateY(20px);
    }
    to {
        opacity: 1;
        transform: translateY(0);
    }
}
@media (max-width: 1200px) {
    .credits-grid {
        grid-template-columns: repeat(2, 1fr);
    }
}
@media (max-width: 768px) {
    .credits-grid {
        grid-template-columns: 1fr;
    }
}
.credits-card {
    background: linear-gradient(145deg, rgba(30, 144, 255, 0.08), rgba(30, 144, 255, 0.03));
    border-radius: 20px;
    padding: 2.5rem;
    border: 1px solid rgba(30, 144, 255, 0.2);
    transition: all 0.4s cubic-bezier(0.4, 0, 0.2, 1);
    backdrop-filter: blur(10px);
    position: relative;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    min-height: 200px;
}
.credits-card::before {
    content: '';
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 1px;
    background: linear-gradient(90deg, transparent, rgba(30, 144, 255, 0.3), transparent);
    opacity: 0;
    transition: opacity 0.3s ease;
}
.credits-card:hover::before {
    opacity: 1;
}
.credits-card:hover {
    transform: translateY(-8px) scale(1.02);
    box-shadow: 0 20px 40px rgba(30, 144, 255, 0.15);
    border-color: rgba(30, 144, 255, 0.4);
    background: linear-gradient(145deg, rgba(30, 144, 255, 0.12), rgba(30, 144, 255, 0.05));
}
.credits-card.proactive-messages {
    background: linear-gradient(to bottom, rgba(76, 175, 80, 0.05), rgba(76, 175, 80, 0.02));
    border: 1px solid rgba(76, 175, 80, 0.2);
}
.credits-card.proactive-messages .credits-header {
    color: #81c784;
}
.credits-card.proactive-messages:hover {
    border-color: rgba(76, 175, 80, 0.4);
    box-shadow: 0 4px 20px rgba(76, 175, 80, 0.15);
}
.credits-card:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 20px rgba(30, 144, 255, 0.15);
    border-color: rgba(30, 144, 255, 0.4);
}
.credits-amount {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    align-items: flex-start;
}
                .credits-amount .amount {
                    color: #e0e0e0;
                    font-size: 1.4rem;
                    font-weight: 600;
                    display: block;
                    line-height: 1.6;
                }
                .reset-info {
                    color: #7EB2FF;
                    font-size: 0.9rem;
                    margin-top: 0.5rem;
                    font-style: italic;
                    opacity: 0.8;
                    transition: opacity 0.3s ease;
                }
                .credits-card:hover .reset-info {
                    opacity: 1;
                }
.usage-estimate {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
    color: #999;
    font-size: 1rem;
}
.time-estimate {
    color: #7EB2FF;
    font-weight: 500;
}
.or {
    color: #666;
    font-size: 0.9rem;
    font-style: italic;
}
.message-estimate {
    color: #7EB2FF;
    font-weight: 500;
}
@media (max-width: 768px) {
    .credits-amount {
        align-items: center;
    }
   
    .usage-estimate {
        justify-content: center;
        text-align: center;
    }
}
.credits-header {
    color: #7EB2FF;
    font-size: 1.1rem;
    font-weight: 600;
    margin-bottom: 1rem;
    padding-bottom: 0.8rem;
}
@media (max-width: 768px) {
    .credits-grid {
        grid-template-columns: 1fr;
    }
}
/* Status Container */
.status-section {
    margin-bottom: 3rem;
}
.status {
    padding: 0;
    border-radius: 16px;
    transition: all 0.3s ease;
}
/* Subscription Status */
.subscription-tier, .discount-status, .no-subscription {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 2rem;
    border-radius: 16px;
    transition: all 0.3s ease;
    border: 1px solid rgba(30, 144, 255, 0.2);
    backdrop-filter: blur(5px);
}
.subscription-tier h3, .discount-status h3, .no-subscription h3 {
    margin: 0;
    white-space: nowrap;
}
.subscription-tier:hover, .discount-status:hover, .no-subscription:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 20px rgba(30, 144, 255, 0.15);
    border-color: rgba(30, 144, 255, 0.4);
}
.discount-status {
    background: linear-gradient(to right, rgba(76, 175, 80, 0.1), rgba(76, 175, 80, 0.05));
    border: 1px solid rgba(76, 175, 80, 0.2);
}
.no-subscription {
    background: linear-gradient(to right, rgba(255, 152, 0, 0.1), rgba(255, 152, 0, 0.05));
    border: 1px solid rgba(255, 152, 0, 0.2);
}
.subscription-tier span, .discount-status span, .no-subscription span {
    color: #e0e0e0;
    font-size: 1.1rem;
    line-height: 1.6;
}
.tier-label {
    color: #1E90FF;
    font-weight: 600;
    font-size: 1.1rem;
    text-transform: capitalize;
}
.subscription-tier:hover, .discount-status:hover, .no-subscription:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 15px rgba(0, 0, 0, 0.2);
}
/* Section Headers */
h3 {
    color: #7EB2FF;
    font-size: 1.2rem;
    margin-bottom: 1rem;
    font-weight: 500;
    letter-spacing: 0.5px;
}
/* Auto Top-up Button (unchanged but included for context) */
.auto-topup-button {
    background: linear-gradient(45deg, #1E90FF, #4169E1);
    color: white;
    margin-top: 1rem;
    padding: 0.75rem 1.5rem;
    border-radius: 8px;
    border: none;
    font-size: 0.9rem;
    cursor: pointer;
    transition: all 0.3s ease;
    position: relative;
    z-index: 100;
}
.auto-topup-button:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 15px rgba(30, 144, 255, 0.3);
}
                /* Auto Top-up Button (unchanged but included for context) */
.auto-topup-button {
    background: linear-gradient(45deg, #1E90FF, #4169E1);
    color: white;
    margin-top: 1rem;
    padding: 0.75rem 1.5rem;
    border-radius: 8px;
    border: none;
    font-size: 0.9rem;
    cursor: pointer;
    transition: all 0.3s ease;
    position: relative;
    z-index: 100;
}
.auto-topup-button:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 15px rgba(30, 144, 255, 0.3);
}
/* Auto Top-up Modal (dark theme with your colors) */
.auto-topup-modal {
    position: absolute;
    background: #222; /* Dark background for the modal */
    border: 1px solid rgba(30, 144, 255, 0.1); /* Subtle blue border */
    border-radius: 12px;
    padding: 1.5rem;
    margin-top: 0.5rem;
    z-index: 90;
    box-shadow: 0 4px 15px rgba(0, 0, 0, 0.2); /* Slightly stronger shadow for depth */
    width: 340px; /* Matches width in your image */
    color: #fff; /* White text for contrast against dark background */
}
/* Modal Header (Automatic Top-up title and toggle) */
.auto-topup-modal h3 {
    color: #7EB2FF; /* Blue accent for title, matching your app’s colors */
    font-size: 1.2rem;
    margin-bottom: 1rem;
    font-weight: 500;
}
.toggle-status {
    color: #B3D1FF; /* Lighter blue for readability on dark background */
    font-size: 1rem;
    margin-left: 1rem; /* Space between the toggle and the status label */
    font-weight: 500;
}
.auto-topup-modal .message {
    padding: 0.8rem;
    border-radius: 8px;
    margin-top: 1rem;
    width: 100%;
    text-align: center;
}
/* Toggle Switch Container */
.auto-topup-toggle {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1.2rem;
}
.auto-topup-toggle span {
    color: #B3D1FF; /* Lighter blue for secondary text, readable on dark */
    font-size: 1rem;
}
.notification-settings {
    margin: 20px 0;
    padding: 15px;
    border-radius: 8px;
    background-color: #f5f5f5;
}
.notify-toggle {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 10px;
}
.notification-description {
    color: #666;
    font-size: 0.9em;
    margin-top: 5px;
}
/* Switch Styling (matches image’s turquoise-blue toggle) */
.switch {
    position: relative;
    display: inline-block;
    width: 60px;
    height: 34px;
}
.switch input {
    opacity: 0;
    width: 0;
    height: 0;
}
.slider {
    position: absolute;
    cursor: pointer;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background-color: #666; /* Dark gray for inactive state */
    transition: .4s;
    border-radius: 34px;
    border: 1px solid rgba(255, 255, 255, 0.1); /* Subtle white border */
}
.slider:before {
    position: absolute;
    content: "";
    height: 26px;
    width: 26px;
    left: 4px;
    bottom: 4px;
    background-color: white;
    transition: .4s;
    border-radius: 50%;
    box-shadow: 0 2px 5px rgba(0, 0, 0, 0.2);
}
input:checked + .slider {
    background-color: #1E90FF; /* Blue from your app’s colors for active state */
}
input:checked + .slider:before {
    transform: translateX(26px);
}
/* Current Balance */
.current-balance {
    display: flex;
    justify-content: space-between;
    padding: 0.75rem 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.1); /* Subtle white border */
    margin-bottom: 1rem;
}
.current-balance span {
    color: #B3D1FF; /* Lighter blue for secondary text */
    font-size: 0.95rem;
}
.balance-amount {
    color: #fff !important;
    font-weight: 600;
}
/* Top-up Settings */
.topup-settings p {
    color: #fff;
    font-size: 0.95rem;
    margin: 1rem 0 0.8rem;
    line-height: 1.4;
}
.amount-input-container {
    margin-bottom: 1.2rem;
}
.amount-input-container label {
    color: #B3D1FF;
    font-size: 0.9rem;
    display: block;
    margin-bottom: 0.5rem;
    font-weight: 500;
}
.amount-input {
    width: 100%;
    padding: 0.6rem;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    background: #333; /* Slightly lighter dark background for input */
    color: #fff;
    font-size: 0.9rem;
    transition: border-color 0.3s ease;
}
.amount-input:focus {
    border-color: #7EB2FF; /* Blue accent on focus, matching your app */
    outline: none;
    box-shadow: 0 0 5px rgba(126, 178, 255, 0.3);
}
.iq-equivalent {
    color: #7EB2FF;
    font-size: 0.9rem;
    margin-top: 0.5rem;
    display: block;
    font-weight: 500;
}
/* Save Button (matches image’s turquoise-blue) */
.save-button {
    background: #1E90FF; /* Solid blue, matching your app’s colors */
    color: white;
    padding: 0.8rem 1.5rem;
    border-radius: 8px;
    border: none;
    font-size: 0.9rem;
    cursor: pointer;
    transition: all 0.3s ease;
    width: 100%;
    font-weight: 500;
}
.save-button:hover {
    background: linear-gradient(45deg, #1E90FF, #4169E1); /* Gradient on hover, matching your app */
    transform: translateY(-2px);
    box-shadow: 0 6px 20px rgba(30, 144, 255, 0.4);
}
.customer-portal-button {
    background: linear-gradient(45deg, #1E90FF, #4169E1);
    color: white;
    margin-top: 1rem;
    padding: 0.75rem 1.5rem;
    border-radius: 8px;
    border: none;
    font-size: 0.9rem;
    cursor: pointer;
    transition: all 0.3s ease;
    position: relative;
    z-index: 100;
    margin-left: 1rem; /* Space between this and the auto-topup button */
}
.customer-portal-button:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 15px rgba(30, 144, 255, 0.3);
}
/* New Buy Credits Button */
.buy-credits-button {
    background: linear-gradient(45deg, #1E90FF, #4169E1);
    color: white;
    margin-top: 1rem;
    padding: 0.75rem 1.5rem;
    border-radius: 8px;
    border: none;
    font-size: 0.9rem;
    cursor: pointer;
    transition: all 0.3s ease;
    position: relative;
    z-index: 100;
    margin-left: 1rem; /* Space between this and the auto-topup button */
}
.buy-credits-button:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 15px rgba(30, 144, 255, 0.3);
}
.buy-credits-button.disabled {
    background: #666;
    cursor: not-allowed;
    opacity: 0.7;
}
.buy-credits-button.disabled:hover {
    transform: none;
    box-shadow: none;
}
.buy-credits-disabled {
    position: relative;
    display: inline-block;
}
.buy-credits-disabled .tooltip {
    width: 250px;
    background-color: rgba(0, 0, 0, 0.9);
    color: white;
    text-align: center;
    padding: 8px;
    border-radius: 6px;
    position: absolute;
    z-index: 1;
    bottom: 125%;
    left: 50%;
    transform: translateX(-50%);
    visibility: hidden;
    opacity: 0;
    transition: opacity 0.3s;
}
.buy-credits-disabled:hover .tooltip {
    visibility: visible;
    opacity: 1;
}
/* Buy Credits Modal */
.buy-credits-modal {
    position: absolute;
    background: #222; /* Dark background for the modal */
    border: 1px solid rgba(30, 144, 255, 0.1); /* Subtle blue border */
    border-radius: 12px;
    padding: 1.5rem;
    margin-top: 0.5rem;
    z-index: 90;
    box-shadow: 0 4px 15px rgba(0, 0, 0, 0.2); /* Slightly stronger shadow for depth */
    width: 340px; /* Matches width in your image */
    color: #fff; /* White text for contrast against dark background */
}
.buy-credits-modal h3 {
    color: #7EB2FF; /* Blue accent for title, matching your app’s colors */
    font-size: 1.2rem;
    margin-bottom: 1rem;
    font-weight: 500;
}
.buy-credits-modal .message {
    padding: 0.8rem;
    border-radius: 8px;
    margin-top: 1rem;
    width: 100%;
    text-align: center;
}
/* Modal Actions */
.modal-actions {
    display: flex;
    gap: 1rem;
    margin-top: 1.5rem;
}
.cancel-button {
    background: #666; /* Dark gray for Cancel button */
    color: white;
    padding: 0.8rem 1.5rem;
    border-radius: 8px;
    border: none;
    font-size: 0.9rem;
    cursor: pointer;
    transition: all 0.3s ease;
    flex: 1;
}
.cancel-button:hover {
    background: #555; /* Slightly darker gray on hover */
    transform: translateY(-2px);
    box-shadow: 0 4px 15px rgba(0, 0, 0, 0.2);
}
.buy-now-button {
    background: #1E90FF; /* Blue for Buy Now button, matching your app’s colors */
    color: white;
    padding: 0.8rem 1.5rem;
    border-radius: 8px;
    border: none;
    font-size: 0.9rem;
    cursor: pointer;
    transition: all 0.3s ease;
    flex: 1;
}
.buy-now-button:hover {
    background: linear-gradient(45deg, #1E90FF, #4169E1); /* Gradient on hover, matching your app */
    transform: translateY(-2px);
    box-shadow: 0 6px 20px rgba(30, 144, 255, 0.4);
}
/* Confirmation Modal */
.confirmation-modal {
    position: absolute;
    background: #222; /* Dark background for the modal */
    border: 1px solid rgba(30, 144, 255, 0.1); /* Subtle blue border */
    border-radius: 12px;
    padding: 1.5rem;
    margin-top: 0.5rem;
    z-index: 90;
    box-shadow: 0 4px 15px rgba(0, 0, 0, 0.2); /* Slightly stronger shadow for depth */
    width: 340px; /* Matches width in your image */
    color: #fff; /* White text for contrast against dark background */
}
.confirmation-modal h3 {
    color: #7EB2FF; /* Blue accent for title, matching your app’s colors */
    font-size: 1.2rem;
    margin-bottom: 1rem;
    font-weight: 500;
}
.confirmation-modal p {
    color: #B3D1FF; /* Lighter blue for text, readable on dark */
    font-size: 0.95rem;
    margin-bottom: 1.5rem;
    line-height: 1.4;
}
.confirmation-modal .message {
    padding: 0.8rem;
    border-radius: 8px;
    margin-top: 1rem;
    width: 100%;
    text-align: center;
}
/* Modal Actions (shared with buy-credits-modal) */
.modal-actions {
    display: flex;
    gap: 1rem;
    margin-top: 1.5rem;
}
.cancel-button {
    background: #666; /* Dark gray for Cancel button */
    color: white;
    padding: 0.8rem 1.5rem;
    border-radius: 8px;
    border: none;
    font-size: 0.9rem;
    cursor: pointer;
    transition: all 0.3s ease;
    flex: 1;
}
.cancel-button:hover {
    background: #555; /* Slightly darker gray on hover */
    transform: translateY(-2px);
    box-shadow: 0 4px 15px rgba(0, 0, 0, 0.2);
}
.confirm-button {
    background: #1E90FF; /* Blue for Confirm button, matching your app’s colors */
    color: white;
    padding: 0.8rem 1.5rem;
    border-radius: 8px;
    border: none;
    font-size: 0.9rem;
    cursor: pointer;
    transition: all 0.3s ease;
    flex: 1;
}
.confirm-button:hover {
    background: linear-gradient(45deg, #1E90FF, #4169E1); /* Gradient on hover, matching your app */
    transform: translateY(-2px);
    box-shadow: 0 6px 20px rgba(30, 144, 255, 0.4);
}
/* Subscription Tier Display */
.subscription-tier {
    border-radius: 8px;
    padding: 1rem;
    margin-bottom: 1rem;
    display: flex;
    justify-content: space-between;
    align-items: center;
}
.subscription-tier span {
    color: #B3D1FF;
    font-size: 0.95rem;
}
.tier-label {
    color: #1E90FF !important;
    font-weight: 600;
    text-transform: capitalize;
}
/* Payment Method Button */
.subscription-tier {
    border-radius: 8px;
    padding: 1rem;
    margin-bottom: 1rem;
    display: flex;
    justify-content: space-between;
    align-items: center;
}
.subscription-tier span {
    color: #B3D1FF;
    font-size: 0.95rem;
}
.tier-label {
    color: #1E90FF !important;
    font-weight: 600;
    text-transform: capitalize;
}
/* Payment Method Button */
.payment-method-button {
    background: linear-gradient(45deg, #1E90FF, #4169E1);
    color: white;
    padding: 0.75rem 1.5rem;
    border-radius: 8px;
    border: none;
    font-size: 0.9rem;
    cursor: pointer;
    transition: all 0.3s ease;
    position: relative;
    margin-left: 1rem; /* Space between this and other buttons */
}
.payment-method-button:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 15px rgba(30, 144, 255, 0.3);
}
/* Payment Method Container */
.payment-method-container {
    display: flex;
    align-items: center;
    margin-top: 1rem;
}
/* Stripe Modal */
.stripe-modal {
    position: absolute;
    background: #222; /* Dark background for the modal */
    border: 1px solid rgba(30, 144, 255, 0.1); /* Subtle blue border */
    border-radius: 12px;
    padding: 1.5rem;
    margin-top: 0.5rem;
    z-index: 90;
    box-shadow: 0 4px 15px rgba(0, 0, 0, 0.2); /* Slightly stronger shadow for depth */
    width: 340px; /* Matches width in your image */
    color: #fff; /* White text for contrast against dark background */
}
.stripe-modal p {
    color: #B3D1FF; /* Lighter blue for text, readable on dark */
    font-size: 0.95rem;
    margin-bottom: 1rem;
    line-height: 1.4;
}
.stripe-modal {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    background: white;
    padding: 20px;
    border-radius: 8px;
    box-shadow: 0 0 10px rgba(0, 0, 0, 0.3);
    z-index: 1000;
}
#card-element {
    margin: 10px 0;
    padding: 10px;
    border: 1px solid #ccc;
    border-radius: 4px;
}
#card-errors {
    color: red;
    font-size: 14px;
    margin-top: 10px;
}
#payment-form button[type="submit"] {
    margin-top: 10px;
    padding: 8px 16px;
    background: #007bff;
    color: white;
    border: none;
    border-radius: 4px;
    cursor: pointer;
}
#payment-form button[type="submit"]:hover {
    background: #0056b3;
}
.close-button {
    background: #666; /* Dark gray for Close button */
    color: white;
    padding: 0.8rem 1.5rem;
    border-radius: 8px;
    border: none;
    font-size: 0.9rem;
    cursor: pointer;
    transition: all 0.3s ease;
    width: 100%;
}
.close-button:hover {
    background: #555; /* Slightly darker gray on hover */
    transform: translateY(-2px);
    box-shadow: 0 4px 15px rgba(0, 0, 0, 0.2);
}
/* Success Message */
.success-message {
    background: #4CAF50; /* Green for success */
    color: white;
    padding: 0.8rem;
    border-radius: 8px;
    margin-top: 1rem;
    width: 100%;
    text-align: center;
}
                /* Tooltip Styles */
                .credits-card, .subscription-tier, .discount-status, .no-subscription {
                    position: relative;
                }
                .tooltip {
                    position: absolute;
                    visibility: hidden;
                    width: 300px;
                    background-color: rgba(0, 0, 0, 0.9);
                    color: white;
                    text-align: left;
                    padding: 12px;
                    border-radius: 8px;
                    font-size: 14px;
                    line-height: 1.4;
                    z-index: 1;
                    top: -10px;
                    left: 50%;
                    transform: translateX(-50%) translateY(-100%);
                    opacity: 0;
                    transition: all 0.3s ease;
                    border: 1px solid rgba(30, 144, 255, 0.2);
                    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
                    backdrop-filter: blur(10px);
                }
                .credits-card:hover .tooltip,
                .subscription-tier:hover .tooltip,
                .discount-status:hover .tooltip,
                .no-subscription:hover .tooltip {
                    visibility: visible;
                    opacity: 1;
                    z-index: 2;
                }
                /* Add a small arrow at the bottom of the tooltip */
                .tooltip::after {
                    content: "";
                    position: absolute;
                    top: 100%;
                    left: 50%;
                    margin-left: -5px;
                    border-width: 5px;
                    border-style: solid;
                    border-color: rgba(0, 0, 0, 0.9) transparent transparent transparent;
                }
                /* Adjust heading margins */
                h3 {
                    margin: 0;
                }

                /* Refund Section Styles */
                .refund-section {
                    margin-top: 2rem;
                    padding: 1.5rem;
                    background: linear-gradient(to bottom, rgba(100, 100, 100, 0.05), rgba(100, 100, 100, 0.02));
                    border-radius: 12px;
                    border: 1px solid rgba(100, 100, 100, 0.2);
                }
                .refund-title {
                    font-size: 1.1rem;
                    margin-bottom: 1rem;
                    color: #666;
                }
                .refund-loading {
                    color: #888;
                    font-style: italic;
                }
                .refund-content {
                    padding: 0.5rem 0;
                }
                .refund-status {
                    padding: 1rem;
                    border-radius: 8px;
                }
                .refund-eligible {
                    background: rgba(76, 175, 80, 0.1);
                    border: 1px solid rgba(76, 175, 80, 0.3);
                }
                .refund-ineligible {
                    background: rgba(158, 158, 158, 0.1);
                    border: 1px solid rgba(158, 158, 158, 0.2);
                }
                .refund-reason {
                    font-size: 0.95rem;
                    margin-bottom: 0.5rem;
                }
                .refund-details {
                    font-size: 0.85rem;
                    color: #666;
                    margin: 0.25rem 0;
                }
                .refund-contact {
                    font-size: 0.85rem;
                    color: #888;
                    margin-top: 1rem;
                }
                .refund-contact a {
                    color: #1e90ff;
                    text-decoration: none;
                }
                .refund-contact a:hover {
                    text-decoration: underline;
                }
                .refund-btn {
                    margin-top: 1rem;
                    padding: 0.75rem 1.5rem;
                    background: #4CAF50;
                    color: white;
                    border: none;
                    border-radius: 8px;
                    cursor: pointer;
                    font-size: 0.95rem;
                    transition: background 0.2s ease;
                }
                .refund-btn:hover {
                    background: #43a047;
                }
                .refund-confirm {
                    margin-top: 1rem;
                    padding: 1rem;
                    background: rgba(255, 152, 0, 0.1);
                    border: 1px solid rgba(255, 152, 0, 0.3);
                    border-radius: 8px;
                }
                .refund-confirm p {
                    margin-bottom: 1rem;
                    color: #e65100;
                }
                .refund-confirm-buttons {
                    display: flex;
                    gap: 1rem;
                }
                .refund-cancel-btn {
                    padding: 0.5rem 1rem;
                    background: #9e9e9e;
                    color: white;
                    border: none;
                    border-radius: 6px;
                    cursor: pointer;
                }
                .refund-cancel-btn:hover {
                    background: #757575;
                }
                .refund-confirm-btn {
                    padding: 0.5rem 1rem;
                    background: #f44336;
                    color: white;
                    border: none;
                    border-radius: 6px;
                    cursor: pointer;
                }
                .refund-confirm-btn:hover {
                    background: #d32f2f;
                }
                .refund-confirm-btn:disabled {
                    background: #bdbdbd;
                    cursor: not-allowed;
                }
                "#}
        </style>
        <style>
            {r#"
/* Rates Section Styling */
.rates-section {
    background: linear-gradient(to bottom, rgba(30, 144, 255, 0.05), rgba(30, 144, 255, 0.02));
    border-radius: 16px;
    padding: 2rem;
    margin-top: 3rem;
    border: 1px solid rgba(30, 144, 255, 0.2);
    transition: all 0.3s ease;
}
.rates-section:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 20px rgba(30, 144, 255, 0.15);
    border-color: rgba(30, 144, 255, 0.4);
}
.rates-container {
    margin-top: 1.5rem;
    display: flex;
    flex-direction: column;
    gap: 1.5rem;
}
.rate-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1rem;
    background: rgba(0, 0, 0, 0.2);
    border-radius: 12px;
    border: 1px solid rgba(30, 144, 255, 0.1);
    transition: all 0.3s ease;
}
.rate-item:hover {
    background: rgba(0, 0, 0, 0.3);
    border-color: rgba(30, 144, 255, 0.2);
}
.rate-label {
    display: flex;
    align-items: center;
    gap: 8px;
    color: #B3D1FF;
    font-size: 1rem;
}
.rate-value {
    color: #7EB2FF;
    font-size: 1rem;
    font-weight: 500;
}

/* Usage Projection Styles */
.usage-projection-section {
    margin-bottom: 2rem;
}

.usage-projection-card {
    background: linear-gradient(145deg, rgba(30, 144, 255, 0.08), rgba(30, 144, 255, 0.03));
    border-radius: 16px;
    padding: 1.5rem;
    border: 1px solid rgba(30, 144, 255, 0.2);
}

.usage-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1rem;
}

.usage-header h3 {
    color: #7EB2FF;
    margin: 0;
    font-size: 1.1rem;
}

.usage-percentage {
    font-size: 1.5rem;
    font-weight: 600;
    color: #e0e0e0;
}

.usage-bar-container {
    width: 100%;
    height: 12px;
    background: rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    overflow: hidden;
    margin-bottom: 1.5rem;
}

.usage-bar {
    height: 100%;
    border-radius: 6px;
    transition: width 0.5s ease, background-color 0.3s ease;
}

.usage-breakdown {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    margin-bottom: 1rem;
    padding: 1rem;
    background: rgba(0, 0, 0, 0.2);
    border-radius: 8px;
}

.usage-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
}

.usage-label {
    color: #B3D1FF;
    font-size: 0.95rem;
}

.usage-value {
    color: #e0e0e0;
    font-size: 0.95rem;
    font-weight: 500;
}

.capacity-info {
    margin-bottom: 1rem;
}

.capacity-remaining, .capacity-over {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.75rem 1rem;
    background: rgba(0, 0, 0, 0.15);
    border-radius: 8px;
}

.capacity-label {
    color: #B3D1FF;
    font-size: 0.9rem;
}

.capacity-value {
    font-weight: 600;
    font-size: 0.95rem;
}

.capacity-value.positive {
    color: #4CAF50;
}

.capacity-value.negative {
    color: #F44336;
}

.overage-info {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    padding: 0.75rem 1rem;
    background: rgba(76, 175, 80, 0.1);
    border: 1px solid rgba(76, 175, 80, 0.3);
    border-radius: 8px;
    margin-bottom: 1rem;
}

.overage-info.covered {
    background: rgba(76, 175, 80, 0.1);
    border-color: rgba(76, 175, 80, 0.3);
}

.overage-label {
    color: #81c784;
    font-size: 0.85rem;
}

.overage-value {
    color: #e0e0e0;
    font-size: 0.95rem;
    font-weight: 500;
}

.usage-tip {
    display: flex;
    align-items: flex-start;
    gap: 0.75rem;
    padding: 0.75rem 1rem;
    border-radius: 8px;
    margin-bottom: 1rem;
}

.usage-tip.warning {
    background: rgba(255, 193, 7, 0.1);
    border: 1px solid rgba(255, 193, 7, 0.3);
}

.tip-icon {
    background: #FFC107;
    color: #000;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-weight: bold;
    font-size: 0.85rem;
    flex-shrink: 0;
}

.tip-text {
    color: #FFE082;
    font-size: 0.9rem;
    line-height: 1.4;
}

.usage-status.ok {
    color: #81c784;
    font-size: 0.95rem;
    padding: 0.5rem 0;
    text-align: center;
    margin-bottom: 1rem;
}

.plan-info-footer {
    color: #888;
    font-size: 0.85rem;
    text-align: center;
    padding-top: 0.75rem;
    border-top: 1px solid rgba(255, 255, 255, 0.1);
}

.days-left {
    color: #7EB2FF;
}

/* Example data styles */
.example-badge {
    font-size: 0.75rem;
    color: #FFC107;
    margin-left: 0.5rem;
    font-weight: normal;
    font-style: italic;
}

.example-data-info {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
    padding: 0.75rem 1rem;
    background: rgba(100, 181, 246, 0.1);
    border: 1px solid rgba(100, 181, 246, 0.3);
    border-radius: 8px;
    margin-bottom: 1rem;
}

.info-icon {
    background: #64B5F6;
    color: #000;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-weight: bold;
    font-size: 0.75rem;
    flex-shrink: 0;
}

.info-text {
    color: #90CAF9;
    font-size: 0.85rem;
    line-height: 1.4;
}

.usage-item.usage-total {
    border-top: 1px solid rgba(255, 255, 255, 0.1);
    padding-top: 0.5rem;
    margin-top: 0.5rem;
}

.usage-item.usage-total .usage-label,
.usage-item.usage-total .usage-value {
    font-weight: 600;
}
            "#}
        </style>
        </>
    }
}
