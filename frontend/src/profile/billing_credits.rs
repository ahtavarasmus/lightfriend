use yew::prelude::*;
use web_sys::{HtmlInputElement, window};
use crate::config;
use crate::profile::usage_graph::UsageGraph;
use gloo_net::http::Request;
use crate::profile::billing_models::{ // Import from the new file
    AutoTopupSettings, BuyCreditsRequest, ApiResponse, UserProfile, StripeSetupIntentResponse,
    IQ_TO_EURO_RATE, MIN_TOPUP_AMOUNT_DOLLARS, MIN_TOPUP_AMOUNT_IQ, format_timestamp,
};
use crate::profile::billing_payments::PaymentMethodButton;
use serde_json::{Value, json};
use chrono::Utc;
use wasm_bindgen_futures::spawn_local;
use gloo_timers::future::TimeoutFuture;
use wasm_bindgen::JsValue; // For debugging/logging

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
    let auto_topup_amount = use_state(|| user_profile.charge_back_to.unwrap_or(0)); // Default to 0 (empty) for the input

    // State to track the saved auto-topup amount for display in "Currently:"
    let saved_auto_topup_amount = use_state(|| user_profile.charge_back_to.unwrap_or(0));

    // Buy credits related states
    let show_buy_credits_modal = use_state(|| false);
    let buy_credits_amount = use_state(|| MIN_TOPUP_AMOUNT_DOLLARS); // Default to $5 (minimum)
    let show_confirmation_modal = use_state(|| false); // New state for confirmation modal

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

    // Function to trigger test purchase
    let test_purchase = {
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
                    match Request::post(&format!("{}/api/stripe/automatic-charge/{}", config::get_backend_url(), user_id))
                        .header("Authorization", &format!("Bearer {}", token))
                        .header("Content-Type", "application/json")
                        .send()
                        .await
                    {
                        Ok(response) => {
                            if response.ok() {
                                if let Ok(data) = response.json::<Value>().await {
                                    if let Some(message) = data.get("message").and_then(|v| v.as_str()) {
                                        success.set(Some(message.to_string()));
                                    }
                                } else {
                                    error.set(Some("Failed to parse test purchase response".to_string()));
                                }
                            } else {
                                error.set(Some("Failed to process test purchase".to_string()));
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
        <div class="profile-info">
            <div class="billing-section">
                {
                    if user_profile.iq < 0 {
                        html! {
                            <>
                                <h3>{"IQ Usage this month"}</h3>
                                <div class="iq-balance">
                                    <span class="iq-time">
                                        {format!("{} IQ (approx. {:.2}€)", user_profile.iq.abs(), (user_profile.iq.abs() as f64 / IQ_TO_EURO_RATE))}
                                    </span>
                                </div>
                            </>
                        }
                    } else {
                        html! {
                            <>
                                <h3>{"Available credits"}</h3>
                                <div class="iq-balance">
                                    <span class="iq-time">
                                        {if user_profile.iq >= 60 { 
                                            format!("{} IQ ({} minutes/messages)", user_profile.iq, user_profile.iq / 60)
                                        } else { 
                                            format!("{} IQ", user_profile.iq)
                                        }}
                                    </span>
                                </div>
                                
                                <div class="auto-topup-container">
                                    <button 
                                        class="auto-topup-button"
                                        onclick={{
                                            let show_modal = show_auto_topup_modal.clone();
                                            Callback::from(move |_| show_modal.set(!*show_modal))
                                        }}
                                    >
                                        {"Automatic Top-up"}
                                    </button>
                                    <button 
                                        class="buy-credits-button"
                                        onclick={toggle_buy_credits_modal.clone()}
                                    >
                                        {"Buy Credits"}
                                    </button>
                                    <button 
                                        class="test-purchase-button"
                                        onclick={test_purchase.clone()}
                                    >
                                        {"Test 5 EUR Purchase"}
                                    </button>
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
                                                        <span class="balance-amount">{format!("${:.2}", (*saved_auto_topup_amount as f64 / IQ_TO_EURO_RATE).max(0.0))}</span>
                                                    </div>
                                                    
                                                    {
                                                        if *auto_topup_active {
                                                            html! {
                                                                <div class="topup-settings">
                                                                    <p>{"How much would you like to automatically top up when your balance drops below $2.00?"}</p>
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
                                                                                    if let Ok(dollars) = input.value().parse::<f64>() {
                                                                                        // Enforce minimum of $5 (300 IQ)
                                                                                        let final_dollars = dollars.max(MIN_TOPUP_AMOUNT_DOLLARS);
                                                                                        if dollars < MIN_TOPUP_AMOUNT_DOLLARS {
                                                                                            error.set(Some("Minimum amount is $5".to_string()));
                                                                                            // Clear error after 3 seconds
                                                                                            let error_clone = error.clone();
                                                                                            spawn_local(async move {
                                                                                                TimeoutFuture::new(3_000).await;
                                                                                                error_clone.set(None);
                                                                                            });
                                                                                        }
                                                                                        // Convert dollars to IQ credits
                                                                                        let iq_amount = (final_dollars * IQ_TO_EURO_RATE).round() as i32;
                                                                                        auto_topup_amount.set(iq_amount);
                                                                                        // Update the input value to reflect the enforced minimum
                                                                                        input.set_value(&format!("{:.2}", final_dollars));
                                                                                    } else {
                                                                                        // If parsing fails (e.g., empty or invalid input), set to minimum
                                                                                        auto_topup_amount.set(MIN_TOPUP_AMOUNT_IQ);
                                                                                        input.set_value(&format!("{:.2}", MIN_TOPUP_AMOUNT_DOLLARS));
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
                                                            min="5"
                                                            class="amount-input"
                                                            value={format!("{:.2}", *buy_credits_amount)}
                                                            onchange={{
                                                                let buy_credits_amount = buy_credits_amount.clone();
                                                                let error = error.clone();
                                                                Callback::from(move |e: Event| {
                                                                    let input: HtmlInputElement = e.target_unchecked_into();
                                                                    if let Ok(dollars) = input.value().parse::<f64>() {
                                                                        // Enforce minimum of $5
                                                                        let final_dollars = dollars.max(MIN_TOPUP_AMOUNT_DOLLARS);
                                                                        if dollars < MIN_TOPUP_AMOUNT_DOLLARS {
                                                                            error.set(Some("Minimum amount is $5".to_string()));
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
                                                                        buy_credits_amount.set(MIN_TOPUP_AMOUNT_DOLLARS);
                                                                        input.set_value(&format!("{:.2}", MIN_TOPUP_AMOUNT_DOLLARS));
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
                }

                <div class="billing-info">
                    <PaymentMethodButton user_id={user_profile.id} /> 
                </div>
                <UsageGraph user_id={user_profile.id} />
            </div>
        </div>
    }
}
