use yew::prelude::*;
use web_sys::{HtmlInputElement, window};
use crate::config;
use serde_json::{Value, json};
use crate::profile::usage_graph::UsageGraph;
use gloo_net::http::Request;
use crate::profile::billing_models::{ // Import from the new file
    AutoTopupSettings, 
    BuyCreditsRequest, 
    ApiResponse, 
    UserProfile,
    MIN_TOPUP_AMOUNT_CREDITS
};
use wasm_bindgen_futures::spawn_local;
use gloo_timers::future::TimeoutFuture;

#[derive(Properties, PartialEq, Clone)]
pub struct BillingPageProps {
    pub user_profile: UserProfile,
}

#[function_component]
pub fn BillingPage(props: &BillingPageProps) -> Html {
    let user_profile = &props.user_profile;
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

    // Rate constants (replace with actual values from crate::profile::billing_models)
    let voice_second_cost = crate::profile::billing_models::VOICE_SECOND_COST;
    let message_cost = crate::profile::billing_models::MESSAGE_COST;

    // Calculate usage estimates for one-time credits
    let one_time_credits = user_profile.credits;
    let one_time_minutes = if one_time_credits > 0.0 {
        (one_time_credits / voice_second_cost / 60.0).floor()
    } else {
        0.0
    };
    let one_time_seconds = if one_time_credits > 0.0 {
        (one_time_credits / voice_second_cost % 60.0).floor()
    } else {
        0.0
    };
    let one_time_messages = if one_time_credits > 0.0 {
        (one_time_credits / message_cost).floor()
    } else {
        0.0
    };

    // Calculate usage estimates for monthly quota
    let monthly_credits = user_profile.credits_left;
    let monthly_minutes = if monthly_credits > 0.0 {
        (monthly_credits / voice_second_cost / 60.0).floor()
    } else {
        0.0
    };
    let monthly_seconds = if monthly_credits > 0.0 {
        (monthly_credits / voice_second_cost % 60.0).floor()
    } else {
        0.0
    };
    let monthly_messages = if monthly_credits > 0.0 {
        (monthly_credits / message_cost).floor()
    } else {
        0.0
    };

    // Function to update auto top-up settings and refresh the profile
    let update_auto_topup = {
        let user_id = user_profile.id;
        let error = error.clone();
        let success = success.clone();
        let auto_topup_active = auto_topup_active.clone();
        let auto_topup_amount = auto_topup_amount.clone();
        let saved_auto_topup_amount = saved_auto_topup_amount.clone();
        
        Callback::from(move |settings: AutoTopupSettings| {
            let user_id = user_id;
            let error = error.clone();
            let success = success.clone();
            let auto_topup_active = auto_topup_active.clone();
            let auto_topup_amount = auto_topup_amount.clone();
            let saved_auto_topup_amount = saved_auto_topup_amount.clone();
            let settings = settings.clone();
            
            spawn_local(async move {
                if let Some(token) = window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|storage| storage.get_item("token").ok())
                    .flatten()
                {
                    // Update auto-topup settings
                    match Request::post(&format!("{}/api/billing/update-auto-topup/{}", config::get_backend_url(), user_id))
                        .header("Authorization", &format!("Bearer {}", token))
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
                                    match Request::get(&format!("{}/api/profile", config::get_backend_url()))
                                        .header("Authorization", &format!("Bearer {}", token))
                                        .send()
                                        .await
                                    {
                                        Ok(profile_response) => {
                                            if profile_response.ok() {
                                                match profile_response.json::<UserProfile>().await {
                                                    Ok(updated_profile) => {
                                                        // Update saved amount with the server's value
                                                        if let Some(new_amount) = updated_profile.charge_back_to {
                                                            saved_auto_topup_amount.set(new_amount);
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
                } else {
                    error.set(Some("Authentication token not found".to_string()));
                    // Clear error after 3 seconds
                    let error_clone = error.clone();
                    spawn_local(async move {
                        TimeoutFuture::new(3_000).await;
                        error_clone.set(None);
                    });
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
        
        Callback::from(move |_| {
            let user_id = user_id;
            let error = error.clone();
            let success = success.clone();
            let show_confirmation_modal = show_confirmation_modal.clone();
            let buy_credits_amount = buy_credits_amount.clone();
            
            spawn_local(async move {
                if let Some(token) = window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|storage| storage.get_item("token").ok())
                    .flatten()
                {
                    let amount_dollars = *buy_credits_amount; // Safely dereference the cloned handle
                    let request = BuyCreditsRequest { amount_dollars };

                    match Request::post(&format!("{}/api/stripe/checkout-session/{}", config::get_backend_url(), user_id))
                        .header("Authorization", &format!("Bearer {}", token))
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
                                error.set(Some("Failed to create Stripe Checkout session".to_string()));
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
                } else {
                    error.set(Some("Authentication token not found".to_string()));
                    // Clear error after 3 seconds
                    let error_clone = error.clone();
                    spawn_local(async move {
                        TimeoutFuture::new(3_000).await;
                        error_clone.set(None);
                    });
                }
            });
        })
    };

    // Handle redirect after successful payment
    let handle_successful_payment = {
        let user_id = user_profile.id;
        let success = success.clone();
        let error = error.clone();
        
        use_effect_with_deps(move |()| {
            let window = web_sys::window().unwrap();
            let search = window.location().search().unwrap_or_default();
            if search.contains("session_id=") {
                // Extract session_id from URL
                let session_id = search.split("session_id=").nth(1)
                    .and_then(|s| s.split('&').next())
                    .unwrap_or_default()
                    .to_string();

                spawn_local(async move {
                    if let Some(token) = window
                        .local_storage()
                        .ok()
                        .flatten()
                        .and_then(|storage| storage.get_item("token").ok())
                        .flatten()
                    {
                        match Request::post(&format!("{}/api/stripe/confirm-checkout", config::get_backend_url()))
                            .header("Authorization", &format!("Bearer {}", token))
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
                                    }
                                } else {
                                    error.set(Some("Failed to confirm Stripe payment".to_string()));
                                }
                            }
                            Err(e) => {
                                error.set(Some(format!("Network error confirming payment: {:?}", e)));
                            }
                        }
                        // Clear messages after 3 seconds
                        let success_clone = success.clone();
                        let error_clone = error.clone();
                        spawn_local(async move {
                            TimeoutFuture::new(3_000).await;
                            success_clone.set(None);
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
                if let Some(token) = window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|storage| storage.get_item("token").ok())
                    .flatten()
                {
                    match Request::get(&format!("{}/api/stripe/customer-portal/{}", config::get_backend_url(), user_id))
                        .header("Authorization", &format!("Bearer {}", token))
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
                } else {
                    error.set(Some("Authentication token not found".to_string()));
                    // Clear error after 3 seconds
                    let error_clone = error.clone();
                    spawn_local(async move {
                        TimeoutFuture::new(3_000).await;
                        error_clone.set(None);
                    });
                }
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
                            // Subscription or Discount Status
                            <div class="section-container status-section">
                                {
                                    if let Some(sub_tier) = &user_profile.sub_tier {
                                        html! {
                                            <div class="status">
                                                <div class="subscription-tier">
                                                <h3>{"Current Subscription"}</h3>
                                                <div class="tooltip">
                                                    {
                                                        if sub_tier == "hard_mode" {
                                                            "Basic Plan subscription gives your lightfriend access to Perplexity Search and Weather tool with 30 monthly message quota."
                                                        } else {
                                                            "Escape Plan subscription gives full capability to your lightfriend with 100 proactive notifications and 100 unit message/voice minute quota monthly."
                                                        }
                                                    }
                                                </div>
                                                    <span class="tier-label">
                                                        {
                                                            if sub_tier == "hard_mode" {
                                                                "Basic Plan"
                                                            } else {
                                                                "Escape Plan"
                                                            }
                                                        }
                                                    </span>
                                                </div>
                                            </div>
                                        }
                                    } else if user_profile.discount {
                                        html! {
                                            <div class="status">
                                                <div class="discount-status">
                                                <h3>{"Current Subscription"}</h3>
                                                <div class="tooltip">
                                                    {"Early adopters keep access to tools: Email, Calendar, Shazam, Perplexity and Weather regardless of their subscription status(although credits have to be bought to use them). Thank you for taking interest!"}
                                                </div>
                                                    <span>{"Early adopter"}</span>
                                                </div>
                                            </div>
                                        }
                                    } else {
                                        html! {
                                            <div class="status">
                                                <div class="discount-status">
                                                <h3>{"Current Subscription"}</h3>
                                                <div class="tooltip">
                                                    {"Upgrade to Basic Plan Mode for Perplexity Search and Weather tool with 30 monthly messages, or get Escape Plan for full access including Email, Calendar, and 100 monthly messages / voice minutes!"}
                                                </div>
                                                    <span>{"Inactive"}</span>
                                                </div>
                                            </div>
                                        }
                                    }
                                }
                            </div>

                            // Purchased Credits
                            <div class="section-container">
                                <div class="section-header">
                                    <h3>{"Credits"}</h3>
                                </div>
                                <div class="credits-grid">
                                    <div class="credits-card one-time-credits">
                                        <div class="credits-header">{"Purchased Overage Credits"}</div>
                                        <div class="tooltip">

                                            {"These are the overage credits you've purchased. They don't expire and can be used for voice calls or messages when monthly quota is used up."}
                                        </div>

                                    <div class="credits-amount">
                                        <span class="amount">{format!("{:.2}€", one_time_credits)}</span>
                                        {
                                            if one_time_credits < 0.0 {
                                                html! {
                                                    <span class="usage-estimate">{" (0 minutes/messages)"}</span>
                                                }
                                            } else {
                                                html! {
                                                    <div class="usage-estimate">
                                                        <span class="time-estimate">{format!("({:.0}min {:.0}s", one_time_minutes, one_time_seconds)}</span>
                                                        <span class="or">{"or"}</span>
                                                        <span class="message-estimate">{format!("{:.0} messages)", one_time_messages)}</span>
                                                    </div>
                                                }
                                            }
                                        }
                                    </div>
                                    </div>
                                    <div class="credits-card monthly-credits">
                                        <div class="credits-header">{"Monthly Quota Left"}</div>
                                        <div class="tooltip">
                                            {
                                                if let Some(sub_tier) = &user_profile.sub_tier {
                                                    if sub_tier == "hard_mode" {
                                                        "Your Basic Plan Mode subscription includes 30 monthly messages. When these run out, your purchased overage credits will be used."
                                                    } else {
                                                        "Your Escape Plan subscription includes 100 monthly messages or voice minutes. When these run out, your purchased overage credits will be used."
                                                    }
                                                } else {
                                                    "This is how much monthly quota you have left from your previous subscription. When these run out, your purchased overage credits will be used."
                                                }
                                            }
                                        </div>


                                    <div class="credits-amount">
                                        {
                                            if monthly_credits < 0.0 {
                                                html! {
                                                    <span class="usage-estimate">{"0 minutes/messages"}</span>
                                                }
                                            } else {
                                                html! {
                                                    <div class="usage-estimate">
                                                        <span class="time-estimate">{format!("{:.0}min {:.0}s", monthly_minutes, monthly_seconds)}</span>
                                                        <span class="or">{"or"}</span>
                                                        <span class="message-estimate">{format!("{:.0} messages", monthly_messages)}</span>
                                                    </div>
                                                }
                                            }
                                        }
                                    </div>
                                    </div>
                                    <div class="credits-card proactive-messages">
                                        <div class="credits-header">{"Monthly Proactive Messages"}</div>
                                        <div class="tooltip">
                                            {
                                                if let Some(sub_tier) = &user_profile.sub_tier {
                                                    if sub_tier == "hard_mode" {
                                                        "Proactive notifications are only available with the Escape Plan subscription. Upgrade to get access to email and calendar notifications!"
                                                    } else {
                                                        "Your Escape Plan includes 100 proactive notifications per month. Lightfriend uses these to notify you about important events, emails, and calendar updates."
                                                    }
                                                } else {
                                                    "Proactive notifications are available with the Escape Plan subscription. Subscribe to get notified about important events, emails, and calendar updates!"
                                                }
                                            }
                                        </div>

                                        <div class="credits-amount">
                                            <div class="usage-estimate">
                                                <span class="message-estimate">
                                                    {
                                                        if user_profile.msgs_left <= 0 {
                                                            "0 messages left".to_string()
                                                        } else {
                                                            format!("{} messages left", user_profile.msgs_left)
                                                        }
                                                    }
                                                </span>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            </div>
                            
                            <div class="auto-topup-container">
                                {
                                    if user_profile.sub_tier.is_some() || user_profile.time_to_live.unwrap_or(i32::MAX) < 1747170000 {
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
                                                    {"Subscribe to a plan to enable credit purchases. Credits allow you to make more voice calls and send more messages."}
                                                </div>
                                                    </>
                                        }
                                    }
                                }

                                if user_profile.stripe_payment_method_id.is_some() || user_profile.sub_tier.is_some() {
                                    <button 
                                        class="customer-portal-button"
                                        onclick={open_customer_portal.clone()}
                                    >
                                        {"Manage Payments"}
                                    </button>
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
                        </>
                    }
                }

                <div class="billing-info">
                    //<PaymentMethodButton user_id={user_profile.id} /> 
                </div>
                //<UsageGraph user_id={user_profile.id} />
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
    border-bottom: 1px solid rgba(30, 144, 255, 0.2);
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
    padding: 2rem;
    border-radius: 16px;
    transition: all 0.3s ease;
    border: 1px solid rgba(30, 144, 255, 0.2);
    backdrop-filter: blur(5px);
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
            "#}
        </style>
        </>
    }
}
