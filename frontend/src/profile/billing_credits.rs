use crate::profile::billing_models::{
    ApiResponse, AutoTopupSettings, BuyCreditsRequest, UsageLogEntry, UserProfile,
    MIN_TOPUP_AMOUNT_CREDITS,
};
use crate::profile::stripe::StripePricingTable;
use crate::utils::api::Api;
use gloo_timers::future::TimeoutFuture;
use serde::Deserialize;
use serde_json::{json, Value};
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use yew::prelude::*;

#[derive(Properties, PartialEq, Clone)]
pub struct BillingPageProps {
    pub user_profile: UserProfile,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct SubscriptionMigrationStatus {
    show_current_plans: bool,
    has_canceling_subscription: bool,
    has_legacy_subscription: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct OverageStatus {
    billing_system: String,
    provisioned: bool,
    overage_enabled: bool,
    payment_ready: bool,
    usage_entitled: bool,
    charge_threshold_usd: i32,
    invoice_cadence: String,
    consent_version: String,
    available_usage_usd: Option<f64>,
    resets_at: Option<String>,
}

fn format_reset_date(value: &str) -> String {
    let date = js_sys::Date::new(&JsValue::from_str(value));
    if date.get_time().is_nan() {
        return value.to_string();
    }
    let months = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov",
        "Dec",
    ];
    format!(
        "{} {}",
        months.get(date.get_month() as usize).unwrap_or(&""),
        date.get_date()
    )
}

fn format_activity_type(activity_type: &str) -> &str {
    match activity_type {
        "_critical" | "_priority_sms" | "noti_msg" => "SMS notification",
        "_priority_call" | "noti_call" => "Call notification",
        "sms" | "message" => "Message",
        "voice" | "call" => "Voice call",
        "digest" => "Digest",
        other => other,
    }
}

fn format_timestamp_time(ts: i32) -> String {
    // Convert unix timestamp to HH:MM using js_sys
    let date = js_sys::Date::new_0();
    date.set_time((ts as f64) * 1000.0);
    let hours = date.get_hours();
    let minutes = date.get_minutes();
    format!("{:02}:{:02}", hours, minutes)
}

fn format_timestamp_date(ts: i32) -> String {
    let date = js_sys::Date::new_0();
    date.set_time((ts as f64) * 1000.0);
    let month = date.get_month(); // 0-indexed
    let day = date.get_date();
    let month_name = match month {
        0 => "Jan",
        1 => "Feb",
        2 => "Mar",
        3 => "Apr",
        4 => "May",
        5 => "Jun",
        6 => "Jul",
        7 => "Aug",
        8 => "Sep",
        9 => "Oct",
        10 => "Nov",
        11 => "Dec",
        _ => "???",
    };
    format!("{} {}", month_name, day)
}

fn get_day_label(ts: i32) -> String {
    let now = js_sys::Date::new_0();
    let entry_date = js_sys::Date::new_0();
    entry_date.set_time((ts as f64) * 1000.0);

    let now_day = (now.get_time() / 86400000.0).floor() as i64;
    let entry_day = (entry_date.get_time() / 86400000.0).floor() as i64;

    if now_day == entry_day {
        "Today".to_string()
    } else if now_day - entry_day == 1 {
        "Yesterday".to_string()
    } else {
        format_timestamp_date(ts)
    }
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
    let saved_auto_topup_amount = use_state(|| user_profile.charge_back_to.unwrap_or(5.00));

    // Buy credits related states
    let show_buy_credits_modal = use_state(|| false);
    let buy_credits_amount = use_state(|| 5.00);
    let show_confirmation_modal = use_state(|| false);
    let enable_auto_topup_with_purchase = use_state(|| true);

    // Recent usage feed state
    let usage_feed = use_state(|| None::<Vec<UsageLogEntry>>);
    let migration_status = use_state(|| None::<SubscriptionMigrationStatus>);
    let overage_status = use_state(|| None::<OverageStatus>);
    let overage_updating = use_state(|| false);

    // Fetch recent usage on mount
    {
        let usage_feed = usage_feed.clone();
        use_effect_with_deps(
            move |_| {
                spawn_local(async move {
                    match Api::get("/api/billing/recent-usage").send().await {
                        Ok(response) => {
                            if response.ok() {
                                if let Ok(data) = response.json::<Vec<UsageLogEntry>>().await {
                                    usage_feed.set(Some(data));
                                }
                            }
                        }
                        Err(e) => {
                            web_sys::console::log_1(
                                &format!("Failed to fetch recent usage: {:?}", e).into(),
                            );
                        }
                    }
                });
                || ()
            },
            (),
        );
    }

    {
        let overage_status = overage_status.clone();
        use_effect_with_deps(
            move |_| {
                spawn_local(async move {
                    if let Ok(response) = Api::get("/api/billing/overage").send().await {
                        if response.ok() {
                            if let Ok(status) = response.json::<OverageStatus>().await {
                                overage_status.set(Some(status));
                            }
                        }
                    }
                });
                || ()
            },
            (),
        );
    }

    {
        let migration_status = migration_status.clone();
        let user_id = user_profile.id;
        let has_plan = user_profile.sub_tier.is_some();
        use_effect_with_deps(
            move |_| {
                let migration_status = migration_status.clone();
                spawn_local(async move {
                    if !has_plan {
                        migration_status.set(None);
                        return;
                    }

                    match Api::get(&format!(
                        "/api/stripe/subscription-migration-status/{}",
                        user_id
                    ))
                    .send()
                    .await
                    {
                        Ok(response) if response.ok() => {
                            if let Ok(status) = response.json::<SubscriptionMigrationStatus>().await
                            {
                                migration_status.set(Some(status));
                            }
                        }
                        Ok(_) | Err(_) => migration_status.set(None),
                    }
                });
                || ()
            },
            (user_id, has_plan),
        );
    }

    let one_time_credits = user_profile.credits;

    // Function to update auto top-up settings
    let update_auto_topup = {
        let user_id = user_profile.id;
        let error = error.clone();
        let success = success.clone();
        let auto_topup_active = auto_topup_active.clone();
        let auto_topup_amount = auto_topup_amount.clone();
        let saved_auto_topup_amount = saved_auto_topup_amount.clone();
        let user_profile_state = user_profile_state.clone();

        Callback::from(move |settings: AutoTopupSettings| {
            let user_id = user_id;
            let error = error.clone();
            let success = success.clone();
            let auto_topup_active = auto_topup_active.clone();
            let auto_topup_amount = auto_topup_amount.clone();
            let saved_auto_topup_amount = saved_auto_topup_amount.clone();
            let user_profile_state = user_profile_state.clone();
            let settings = settings.clone();

            spawn_local(async move {
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
                                auto_topup_active.set(settings.active);
                                if let Some(amount) = settings.amount {
                                    auto_topup_amount.set(amount);
                                    saved_auto_topup_amount.set(amount);
                                }
                                // Refresh profile
                                if let Ok(profile_response) = Api::get("/api/profile").send().await
                                {
                                    if profile_response.ok() {
                                        if let Ok(updated_profile) =
                                            profile_response.json::<UserProfile>().await
                                        {
                                            if let Some(new_amount) = updated_profile.charge_back_to
                                            {
                                                saved_auto_topup_amount.set(new_amount);
                                            }
                                            user_profile_state.set(updated_profile);
                                        }
                                    }
                                }
                                TimeoutFuture::new(3_000).await;
                                success.set(None);
                            } else {
                                error.set(Some("Failed to parse response".to_string()));
                                let error_clone = error.clone();
                                spawn_local(async move {
                                    TimeoutFuture::new(3_000).await;
                                    error_clone.set(None);
                                });
                            }
                        } else {
                            error.set(Some("Failed to update auto top-up settings".to_string()));
                            let error_clone = error.clone();
                            spawn_local(async move {
                                TimeoutFuture::new(3_000).await;
                                error_clone.set(None);
                            });
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Network error occurred: {:?}", e)));
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

    let toggle_buy_credits_modal = {
        let show_buy_credits_modal = show_buy_credits_modal.clone();
        Callback::from(move |_| show_buy_credits_modal.set(!*show_buy_credits_modal))
    };

    let show_confirmation = {
        let show_confirmation_modal = show_confirmation_modal.clone();
        let show_buy_credits_modal = show_buy_credits_modal.clone();
        Callback::from(move |_| {
            show_buy_credits_modal.set(false);
            show_confirmation_modal.set(true);
        })
    };

    let confirm_buy_credits = {
        let user_id = user_profile.id;
        let error = error.clone();
        let show_confirmation_modal = show_confirmation_modal.clone();
        let buy_credits_amount = buy_credits_amount.clone();
        let enable_auto_topup_with_purchase = enable_auto_topup_with_purchase.clone();
        let auto_topup_active = auto_topup_active.clone();

        Callback::from(move |_| {
            let user_id = user_id;
            let error = error.clone();
            let show_confirmation_modal = show_confirmation_modal.clone();
            let buy_credits_amount = buy_credits_amount.clone();
            let enable_auto_topup = *enable_auto_topup_with_purchase && !*auto_topup_active;

            spawn_local(async move {
                if enable_auto_topup {
                    let settings = AutoTopupSettings {
                        active: true,
                        amount: Some(*buy_credits_amount),
                    };
                    let _ = Api::post(&format!("/api/billing/update-auto-topup/{}", user_id))
                        .header("Content-Type", "application/json")
                        .json(&settings)
                        .expect("Failed to serialize auto top-up settings")
                        .send()
                        .await;
                }

                let amount_dollars = *buy_credits_amount;
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
                                    web_sys::window()
                                        .unwrap()
                                        .location()
                                        .set_href(url)
                                        .unwrap_or_else(|e| {
                                            error.set(Some(format!(
                                                "Failed to redirect to Stripe: {:?}",
                                                e
                                            )));
                                        });
                                    show_confirmation_modal.set(false);
                                } else {
                                    error.set(Some("No URL in Stripe response".to_string()));
                                }
                            } else {
                                error.set(Some("Failed to parse Stripe response".to_string()));
                            }
                        } else {
                            if let Ok(data) = response.json::<Value>().await {
                                if data
                                    .get("upgrade_required")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false)
                                {
                                    error.set(Some("Credit top-ups are only available on the Digest plan. Upgrade to Digest for more credits and top-up ability.".to_string()));
                                } else if let Some(msg) = data.get("error").and_then(|v| v.as_str())
                                {
                                    error.set(Some(msg.to_string()));
                                } else {
                                    error.set(Some(
                                        "Failed to create Stripe Checkout session".to_string(),
                                    ));
                                }
                            } else {
                                error.set(Some(
                                    "Failed to create Stripe Checkout session".to_string(),
                                ));
                            }
                        }
                        let error_clone = error.clone();
                        spawn_local(async move {
                            TimeoutFuture::new(3_000).await;
                            error_clone.set(None);
                        });
                    }
                    Err(e) => {
                        error.set(Some(format!("Network error occurred: {:?}", e)));
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
        use_effect_with_deps(
            move |_| {
                let window = web_sys::window().unwrap();
                let search = window.location().search().unwrap_or_default();
                let mut need_refresh = false;
                let session_id_opt = if search.contains("session_id=") {
                    let sid = search
                        .split("session_id=")
                        .nth(1)
                        .and_then(|s| s.split('&').next())
                        .unwrap_or_default()
                        .to_string();
                    need_refresh = true;
                    Some(sid)
                } else {
                    None
                };
                if search.contains("subscription=success")
                    || search.contains("subscription=changed")
                    || search.contains("credits=success")
                {
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
                                            error.set(Some(
                                                "Failed to parse confirmation response".to_string(),
                                            ));
                                            refresh_success = false;
                                        }
                                    } else {
                                        error.set(Some(
                                            "Failed to confirm Stripe payment".to_string(),
                                        ));
                                        refresh_success = false;
                                    }
                                }
                                Err(e) => {
                                    error.set(Some(format!(
                                        "Network error confirming payment: {:?}",
                                        e
                                    )));
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
                            history
                                .replace_state_with_url(&JsValue::NULL, "", Some("/billing"))
                                .expect("replace state failed");
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
                || ()
            },
            (),
        )
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
                                    web_sys::window()
                                        .unwrap()
                                        .location()
                                        .set_href(url)
                                        .unwrap_or_else(|e| {
                                            error.set(Some(format!(
                                                "Failed to redirect to Stripe Customer Portal: {:?}",
                                                e
                                            )));
                                        });
                                    success.set(Some(
                                        "Redirecting to Stripe Customer Portal".to_string(),
                                    ));
                                } else {
                                    error.set(Some(
                                        "No URL in Customer Portal response".to_string(),
                                    ));
                                }
                            } else {
                                error.set(Some(
                                    "Failed to parse Customer Portal response".to_string(),
                                ));
                            }
                        } else {
                            error.set(Some("Failed to create Customer Portal session".to_string()));
                        }
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

    let toggle_overage = {
        let overage_status = overage_status.clone();
        let overage_updating = overage_updating.clone();
        let error = error.clone();
        let success = success.clone();
        Callback::from(move |_| {
            let Some(current) = (*overage_status).clone() else {
                return;
            };
            let enabled = !current.overage_enabled;
            let overage_status = overage_status.clone();
            let overage_updating = overage_updating.clone();
            let error = error.clone();
            let success = success.clone();
            overage_updating.set(true);
            spawn_local(async move {
                let request = json!({
                    "enabled": enabled,
                    "consent_version": if enabled {
                        Some(current.consent_version.as_str())
                    } else {
                        None
                    }
                });
                match Api::put("/api/billing/overage")
                    .json(&request)
                    .expect("Failed to serialize overage setting")
                    .send()
                    .await
                {
                    Ok(response) if response.ok() => {
                        if let Ok(updated) = response.json::<OverageStatus>().await {
                            let message = if updated.overage_enabled {
                                "Overage billing enabled"
                            } else {
                                "Overage billing disabled"
                            };
                            overage_status.set(Some(updated));
                            success.set(Some(message.to_string()));
                        }
                    }
                    Ok(response) => {
                        let message = response
                            .json::<Value>()
                            .await
                            .ok()
                            .and_then(|body| body["error"].as_str().map(ToString::to_string))
                            .unwrap_or_else(|| "Could not update overage billing".to_string());
                        error.set(Some(message));
                    }
                    Err(network_error) => {
                        error.set(Some(format!("Network error: {:?}", network_error)));
                    }
                }
                overage_updating.set(false);
            });
        })
    };

    // Determine plan display name
    let plan_name = match user_profile.plan_type.as_deref() {
        Some("assistant") | Some("autopilot") => "Autopilot",
        _ => {
            if user_profile.sub_tier.is_some() {
                "Active"
            } else {
                ""
            }
        }
    };
    let has_plan = user_profile.sub_tier.is_some();
    let show_current_plan_picker = !has_plan
        || migration_status
            .as_ref()
            .map(|status| status.show_current_plans)
            .unwrap_or(false);
    let uses_own_twilio = user_profile.own_twilio_enabled;
    let uses_metronome = overage_status
        .as_ref()
        .map(|status| status.billing_system == "metronome")
        .unwrap_or(false);

    html! {
        <>
        <div class="profile-info">
            <div class="billing-section">
                // Success/error messages
                {
                    if let Some(success_msg) = (*success).as_ref() {
                        html! {
                            <div class="message success-message" style="margin-bottom: 16px;">
                                {success_msg}
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }
                {
                    if let Some(error_msg) = (*error).as_ref() {
                        html! {
                            <div class="message error-message" style="margin-bottom: 16px;">
                                {error_msg}
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }

                if !has_plan {
                    <div class="usage-projection-card billing-pricing-table-card" style="margin-bottom: 16px;">
                        <div class="usage-header">
                            <h3>{"Choose a plan"}</h3>
                        </div>
                        <StripePricingTable user_id={Some(user_profile.id)} />
                    </div>
                } else if show_current_plan_picker {
                    <div class="usage-projection-card current-plan-card" style="margin-bottom: 16px;">
                        <div class="usage-header">
                            <h3>{"Switch to a current plan"}</h3>
                        </div>
                        <div class="billing-note">
                            {"Your Stripe portal still shows an old plan. Choose a current billing timeline here to replace it."}
                        </div>
                        <StripePricingTable user_id={Some(user_profile.id)} />
                    </div>
                }

                // Section A: Plan + Credits Summary
                <div class="usage-projection-card" style="margin-bottom: 16px;">
                    <div class="usage-header">
                        <h3>
                            {
                                if has_plan {
                                    format!("{} Plan", plan_name)
                                } else {
                                    "No active plan".to_string()
                                }
                            }
                        </h3>
                    </div>

                    {
                        if uses_own_twilio {
                            html! {
                                <div style="color: #B3D1FF; font-size: 0.95rem;">
                                    {"Own Twilio enabled - you pay Twilio directly for phone usage"}
                                </div>
                            }
                        } else if has_plan && uses_metronome {
                            html! {
                                <>
                                    <div style="color: #4ade80; font-size: 1.1rem; font-weight: 600; margin-bottom: 6px;">
                                        {
                                            overage_status
                                                .as_ref()
                                                .and_then(|status| status.available_usage_usd)
                                                .map(|amount| format!("${:.2} usage available", amount))
                                                .unwrap_or_else(|| "$25 included usage each month".to_string())
                                        }
                                    </div>
                                    <div style="color: #888; font-size: 0.85rem;">
                                        {
                                            overage_status
                                                .as_ref()
                                                .and_then(|status| status.resets_at.as_deref())
                                                .map(|date| format!("Monthly allowance resets {}.", format_reset_date(date)))
                                                .unwrap_or_else(|| "Usage and allowance are metered by Stripe Metronome.".to_string())
                                        }
                                    </div>
                                </>
                            }
                        } else if has_plan {
                            html! {
                                <>
                                    <div style="display: flex; gap: 24px; flex-wrap: wrap; margin-bottom: 12px;">
                                        <div>
                                            <div style="color: #888; font-size: 0.8rem; margin-bottom: 4px;">{"Included usage this month"}</div>
                                            <div style="color: #4ade80; font-size: 1.4rem; font-weight: 600;">
                                                {format!("${:.2}", user_profile.credits_left)}
                                            </div>
                                        </div>
                                        <div>
                                            <div style="color: #888; font-size: 0.8rem; margin-bottom: 4px;">{"Overage credits"}</div>
                                            <div style="color: #7EB2FF; font-size: 1.4rem; font-weight: 600;">
                                                {format!("${:.2}", one_time_credits)}
                                            </div>
                                        </div>
                                    </div>
                                    {
                                        if let Some(renews_at) = user_profile.included_usage_renews_at {
                                            let reset_date = chrono::DateTime::<chrono::Utc>::from_timestamp(renews_at as i64, 0)
                                                .unwrap_or_else(chrono::Utc::now);
                                            let formatted_date = reset_date.format("%B %d, %Y").to_string();
                                            let days = user_profile.days_until_usage_reset.unwrap_or(0);
                                            html! {
                                                <div style="color: #888; font-size: 0.85rem;">
                                                    {"Included usage renews "}
                                                    <span style="color: #ccc;">{formatted_date}</span>
                                                    {format!(" ({} days)", days)}
                                                </div>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }
                                </>
                            }
                        } else {
                            html! {
                                <div style="color: #888; font-size: 0.9rem;">
                                    {"Subscribe to a plan to start using Lightfriend."}
                                </div>
                            }
                        }
                    }
                </div>

                // Section B: Recent Usage Feed
                <div class="usage-projection-card" style="margin-bottom: 16px;">
                    <div class="usage-header">
                        <h3>{"Recent Usage"}</h3>
                    </div>
                    {
                        if let Some(entries) = (*usage_feed).as_ref() {
                            if entries.is_empty() {
                                html! {
                                    <div style="color: #666; font-size: 0.9rem; padding: 8px 0;">
                                        {"No usage recorded in this included-usage window."}
                                    </div>
                                }
                            } else {
                                // Group entries by day
                                let mut current_day = String::new();
                                html! {
                                    <div class="usage-feed">
                                        { for entries.iter().map(|entry| {
                                            let day = get_day_label(entry.created_at);
                                            let show_header = day != current_day;
                                            current_day = day.clone();

                                            let label = format_activity_type(&entry.activity_type);
                                            let time = format_timestamp_time(entry.created_at);
                                            let cost_str = match entry.credits {
                                                Some(c) if c > 0.0 => format!("-${:.3}", c),
                                                Some(c) if c < 0.0 => format!("-${:.3}", c.abs()),
                                                _ => "free".to_string(),
                                            };
                                            let duration_str = entry.call_duration.map(|d| {
                                                if d >= 60 {
                                                    format!(" ({}m{}s)", d / 60, d % 60)
                                                } else {
                                                    format!(" ({}s)", d)
                                                }
                                            }).unwrap_or_default();

                                            html! {
                                                <>
                                                    { if show_header {
                                                        html! {
                                                            <div class="usage-feed-day">
                                                                {&day}
                                                            </div>
                                                        }
                                                    } else {
                                                        html! {}
                                                    }}
                                                    <div class="usage-feed-entry">
                                                        <span class="usage-feed-label">
                                                            {label}{duration_str}
                                                        </span>
                                                        <span class="usage-feed-meta">
                                                            <span class="usage-feed-cost">{cost_str}</span>
                                                            <span class="usage-feed-time">{time}</span>
                                                        </span>
                                                    </div>
                                                </>
                                            }
                                        })}
                                    </div>
                                }
                            }
                        } else {
                            html! {
                                <div style="color: #666; font-size: 0.9rem; padding: 8px 0;">
                                    {"Loading..."}
                                </div>
                            }
                        }
                    }
                </div>

                if uses_metronome && has_plan && !uses_own_twilio {
                    <div class="usage-projection-card overage-billing-card" style="margin-bottom: 16px;">
                        <div class="usage-header">
                            <h3>{"Overage billing"}</h3>
                            <span class={classes!(
                                "overage-status",
                                overage_status.as_ref().map(|status| status.overage_enabled).unwrap_or(false).then_some("enabled")
                            )}>
                                {
                                    if overage_status.as_ref().map(|status| status.overage_enabled).unwrap_or(false) {
                                        "Enabled"
                                    } else {
                                        "Off"
                                    }
                                }
                            </span>
                        </div>
                        <p class="overage-copy">
                            {format!(
                                "After your included $25 is used, continue on usage-based billing. Your saved payment method is charged whenever overage reaches ${} or {}, whichever comes first.",
                                overage_status.as_ref().map(|status| status.charge_threshold_usd).unwrap_or(10),
                                overage_status.as_ref().map(|status| status.invoice_cadence.as_str()).unwrap_or("weekly")
                            )}
                        </p>
                        {
                            if let Some(status) = overage_status.as_ref() {
                                html! {
                                    <>
                                        if !status.provisioned {
                                            <div class="billing-note">{"Finishing billing setup…"}</div>
                                        } else if !status.payment_ready {
                                            <div class="billing-note">{"A valid default payment method is required. Use Manage billing below to update it."}</div>
                                        }
                                        <button
                                            class={classes!("overage-toggle-button", status.overage_enabled.then_some("danger"))}
                                            disabled={*overage_updating || !status.provisioned || (!status.payment_ready && !status.overage_enabled)}
                                            onclick={toggle_overage.clone()}
                                        >
                                            {
                                                if *overage_updating {
                                                    "Saving…"
                                                } else if status.overage_enabled {
                                                    "Disable overage billing"
                                                } else {
                                                    "Enable overage billing"
                                                }
                                            }
                                        </button>
                                        <div class="overage-terms">
                                            {"By enabling, you authorize off-session usage charges to your saved payment method. You can disable overage at any time; usage already incurred remains billable."}
                                        </div>
                                    </>
                                }
                            } else {
                                html! { <div class="billing-note">{"Loading billing settings…"}</div> }
                            }
                        }
                    </div>
                }

                // Legacy credits UI remains available only until the Metronome cutover flag is enabled.
                <div class="usage-projection-card" style={format!("margin-bottom: 16px;{}{}", if uses_own_twilio { " opacity: 0.6;" } else { "" }, if uses_metronome { " display: none;" } else { "" })}>
                    <div class="usage-header">
                        <h3>{"Overage Credits"}</h3>
                        <span class="usage-percentage" style="font-size: 1.2rem;">{format!("${:.2}", one_time_credits)}</span>
                    </div>
                    <div style="margin-bottom: 12px; color: #888; font-size: 0.8rem;">
                        {"One-time purchased credits that don't expire. Used when monthly quota is exhausted."}
                    </div>

                    <div class="auto-topup-container" style="margin-top: 12px; padding: 0;">
                    {
                        if uses_own_twilio {
                            html! {
                                <>
                                    <div class="buy-credits-disabled">
                                        <button
                                            class="buy-credits-button disabled"
                                            title="Not needed with own Twilio enabled"
                                            disabled=true
                                            style="opacity: 0.5; cursor: not-allowed;"
                                        >
                                            {"Buy Credits"}
                                        </button>
                                    </div>
                                    <div class="tooltip" style="color: #888; font-size: 0.85rem;">
                                        {"With own Twilio enabled, you pay Twilio directly for phone usage."}
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
                                    {"Subscribe to a plan to enable overage credit purchases."}
                                </div>
                                </>
                            }
                        }
                    }
                    // Auto top-up modal
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
                                                            value=""
                                                            onchange={{
                                                                let auto_topup_amount = auto_topup_amount.clone();
                                                                let error = error.clone();
                                                                Callback::from(move |e: Event| {
                                                                    let input: HtmlInputElement = e.target_unchecked_into();
                                                                    if let Ok(dollars) = input.value().parse::<f32>() {
                                                                        let final_dollars = dollars.max(MIN_TOPUP_AMOUNT_CREDITS);
                                                                        if dollars < MIN_TOPUP_AMOUNT_CREDITS {
                                                                            error.set(Some("Minimum amount is $5".to_string()));
                                                                            let error_clone = error.clone();
                                                                            spawn_local(async move {
                                                                                TimeoutFuture::new(3_000).await;
                                                                                error_clone.set(None);
                                                                            });
                                                                        }
                                                                        auto_topup_amount.set(final_dollars);
                                                                        input.set_value(&format!("{:.2}", final_dollars));
                                                                    } else {
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
                    // Buy credits modal
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
                                                        let final_dollars = dollars.max(MIN_TOPUP_AMOUNT_CREDITS);
                                                        if dollars < MIN_TOPUP_AMOUNT_CREDITS {
                                                            error.set(Some("Minimum amount is $3".to_string()));
                                                            let error_clone = error.clone();
                                                            spawn_local(async move {
                                                                TimeoutFuture::new(3_000).await;
                                                                error_clone.set(None);
                                                            });
                                                        }
                                                        buy_credits_amount.set(final_dollars);
                                                        input.set_value(&format!("{:.2}", final_dollars));
                                                    } else {
                                                        buy_credits_amount.set(MIN_TOPUP_AMOUNT_CREDITS);
                                                        input.set_value(&format!("{:.2}", MIN_TOPUP_AMOUNT_CREDITS));
                                                    }
                                                })
                                            }}
                                        />
                                    </div>
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
                    // Confirmation modal
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

                // Manage billing button
                if has_plan {
                    <button
                        class="customer-portal-button"
                        onclick={open_customer_portal.clone()}
                    >
                        {
                            if show_current_plan_picker {
                                "Manage old plan / payment methods"
                            } else {
                                "Manage billing"
                            }
                        }
                    </button>
                }
            </div>
        </div>
        <style>
            {r#"
.billing-section {
    padding: 1rem 0;
}

.billing-pricing-table-card .stripe-pricing-table-wrap {
    margin-top: 0.5rem;
}

.current-plan-card .usage-header {
    margin-bottom: 0.5rem;
}

.billing-note {
    color: #a8b3c7;
    font-size: 0.9rem;
    line-height: 1.4;
    margin-bottom: 1rem;
}

.overage-status {
    color: #9ca3af;
    font-size: 0.82rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
}

.overage-status.enabled {
    color: #4ade80;
}

.overage-copy {
    color: #cbd5e1;
    line-height: 1.55;
    margin: 0 0 1rem;
}

.overage-toggle-button {
    min-height: 44px;
    border: 0;
    border-radius: 8px;
    padding: 0.7rem 1rem;
    background: linear-gradient(45deg, #1e90ff, #4169e1);
    color: white;
    font-weight: 650;
    cursor: pointer;
}

.overage-toggle-button.danger {
    background: rgba(239, 68, 68, 0.12);
    border: 1px solid rgba(239, 68, 68, 0.4);
    color: #fca5a5;
}

.overage-toggle-button:disabled {
    cursor: not-allowed;
    opacity: 0.55;
}

.overage-terms {
    color: #7f8b9d;
    font-size: 0.78rem;
    line-height: 1.45;
    margin-top: 0.75rem;
}

.plan-switch-buttons {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 0.75rem;
}

.plan-switch-button {
    min-height: 44px;
    border-radius: 8px;
    border: 1px solid rgba(126, 178, 255, 0.35);
    background: rgba(126, 178, 255, 0.08);
    color: #dbeafe;
    font-size: 0.95rem;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.2s ease;
}

.plan-switch-button.primary {
    background: linear-gradient(45deg, #1E90FF, #4169E1);
    border-color: transparent;
    color: #fff;
}

.plan-switch-button:hover:not(:disabled) {
    transform: translateY(-1px);
    border-color: rgba(126, 178, 255, 0.6);
}

.plan-switch-button:disabled {
    opacity: 0.6;
    cursor: wait;
}

@media (max-width: 640px) {
    .plan-switch-buttons {
        grid-template-columns: 1fr;
    }
}

.stripe-pricing-loading,
.stripe-pricing-error {
    min-height: 120px;
    display: grid;
    place-items: center;
    color: #aaa;
    font-size: 0.95rem;
}

.stripe-pricing-error {
    color: #ffb4a8;
}

/* Usage Projection Card (reused for all cards) */
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

/* Usage Feed */
.usage-feed {
    max-height: 400px;
    overflow-y: auto;
}

.usage-feed-day {
    color: #7EB2FF;
    font-size: 0.8rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    padding: 8px 0 4px 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    margin-bottom: 4px;
}

.usage-feed-day:first-child {
    padding-top: 0;
}

.usage-feed-entry {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 6px 8px;
    border-radius: 6px;
    transition: background 0.15s;
}

.usage-feed-entry:hover {
    background: rgba(255, 255, 255, 0.03);
}

.usage-feed-label {
    color: #ccc;
    font-size: 0.9rem;
}

.usage-feed-meta {
    display: flex;
    align-items: center;
    gap: 12px;
}

.usage-feed-cost {
    color: #ef4444;
    font-size: 0.85rem;
    font-weight: 500;
    font-family: monospace;
}

.usage-feed-time {
    color: #666;
    font-size: 0.8rem;
    min-width: 40px;
    text-align: right;
}

/* Buttons */
.auto-topup-button, .buy-credits-button {
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
    margin-left: 1rem;
}

.auto-topup-button:hover, .buy-credits-button:hover {
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
    margin-left: 1rem;
}

.customer-portal-button:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 15px rgba(30, 144, 255, 0.3);
}

/* Modals */
.auto-topup-modal, .buy-credits-modal, .confirmation-modal {
    position: absolute;
    background: #222;
    border: 1px solid rgba(30, 144, 255, 0.1);
    border-radius: 12px;
    padding: 1.5rem;
    margin-top: 0.5rem;
    z-index: 90;
    box-shadow: 0 4px 15px rgba(0, 0, 0, 0.2);
    width: 340px;
    color: #fff;
}

.auto-topup-modal h3, .buy-credits-modal h3, .confirmation-modal h3 {
    color: #7EB2FF;
    font-size: 1.2rem;
    margin-bottom: 1rem;
    font-weight: 500;
}

.confirmation-modal p {
    color: #B3D1FF;
    font-size: 0.95rem;
    margin-bottom: 1.5rem;
    line-height: 1.4;
}

/* Toggle */
.auto-topup-toggle {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 1.2rem;
}

.auto-topup-toggle span {
    color: #B3D1FF;
    font-size: 1rem;
}

.toggle-status {
    color: #B3D1FF;
    font-size: 1rem;
    margin-left: 1rem;
    font-weight: 500;
}

.current-balance {
    display: flex;
    justify-content: space-between;
    padding: 0.75rem 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
    margin-bottom: 1rem;
}

.current-balance span {
    color: #B3D1FF;
    font-size: 0.95rem;
}

.balance-amount {
    color: #fff !important;
    font-weight: 600;
}

/* Switch */
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
    background-color: #666;
    transition: .4s;
    border-radius: 34px;
    border: 1px solid rgba(255, 255, 255, 0.1);
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
    background-color: #1E90FF;
}

input:checked + .slider:before {
    transform: translateX(26px);
}

/* Inputs */
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
    background: #333;
    color: #fff;
    font-size: 0.9rem;
    transition: border-color 0.3s ease;
}

.amount-input:focus {
    border-color: #7EB2FF;
    outline: none;
    box-shadow: 0 0 5px rgba(126, 178, 255, 0.3);
}

.topup-settings p {
    color: #fff;
    font-size: 0.95rem;
    margin: 1rem 0 0.8rem;
    line-height: 1.4;
}

/* Action buttons */
.save-button {
    background: #1E90FF;
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
    background: linear-gradient(45deg, #1E90FF, #4169E1);
    transform: translateY(-2px);
    box-shadow: 0 6px 20px rgba(30, 144, 255, 0.4);
}

.modal-actions {
    display: flex;
    gap: 1rem;
    margin-top: 1.5rem;
}

.cancel-button {
    background: #666;
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
    background: #555;
    transform: translateY(-2px);
    box-shadow: 0 4px 15px rgba(0, 0, 0, 0.2);
}

.buy-now-button, .confirm-button {
    background: #1E90FF;
    color: white;
    padding: 0.8rem 1.5rem;
    border-radius: 8px;
    border: none;
    font-size: 0.9rem;
    cursor: pointer;
    transition: all 0.3s ease;
    flex: 1;
}

.buy-now-button:hover, .confirm-button:hover {
    background: linear-gradient(45deg, #1E90FF, #4169E1);
    transform: translateY(-2px);
    box-shadow: 0 6px 20px rgba(30, 144, 255, 0.4);
}

/* Messages */
.success-message {
    background: #4CAF50;
    color: white;
    padding: 0.8rem;
    border-radius: 8px;
    text-align: center;
}

.error-message {
    background: rgba(244, 67, 54, 0.2);
    color: #ef4444;
    padding: 0.8rem;
    border-radius: 8px;
    text-align: center;
}

/* Tooltip */
.tooltip {
    color: #888;
    font-size: 0.85rem;
    margin-top: 8px;
}

h3 {
    margin: 0;
}
            "#}
        </style>
        </>
    }
}
