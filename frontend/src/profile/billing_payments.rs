use yew::prelude::*;
use web_sys::{HtmlInputElement, window};
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;
use gloo_timers::future::TimeoutFuture;
use wasm_bindgen::JsValue; // For debugging/logging

#[derive(Properties, PartialEq, Clone)]
pub struct PaymentMethodButtonProps {
    pub user_id: i32, // User ID for the Stripe customer
}

#[derive(Deserialize, Clone, PartialEq)]
pub struct StripeSetupIntentResponse {
    pub client_secret: String, // Client secret for the SetupIntent
}

#[derive(Deserialize, Clone, PartialEq)]
pub struct ApiResponse {
    pub success: bool,
    pub message: String,
}

#[function_component]
pub fn PaymentMethodButton(props: &PaymentMethodButtonProps) -> Html {
    let error = use_state(|| None::<String>);
    let success = use_state(|| None::<String>);
    let show_stripe_modal = use_state(|| false); // State for showing Stripe modal or redirect

    // Function to open Stripe payment method setup
    let open_stripe_portal = {
        let user_id = props.user_id;
        let error = error.clone();
        let success = success.clone();
        let show_stripe_modal = show_stripe_modal.clone();

        Callback::from(move |_| {
            let user_id = user_id;
            let error = error.clone();
            let success = success.clone();
            let show_stripe_modal = show_stripe_modal.clone();

            spawn_local(async move {
                if let Some(token) = window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|storage| storage.get_item("token").ok())
                    .flatten()
                {
                    // Create a SetupIntent on the backend to collect payment method
                    match Request::post(&format!("{}/api/stripe/setup-intent/{}", "https://your-backend-url", user_id)) // Replace with your backend URL
                        .header("Authorization", &format!("Bearer {}", token))
                        .header("Content-Type", "application/json")
                        .send()
                        .await
                    {
                        Ok(response) => {
                            if response.ok() {
                                match response.json::<StripeSetupIntentResponse>().await {
                                    Ok(data) => {
                                        // Here, you would typically use Stripe.js to handle the SetupIntent
                                        // This is a simplified example; in a real app, you'd integrate Stripe.js
                                        success.set(Some("Redirecting to Stripe to add payment method...".to_string()));
                                        show_stripe_modal.set(true); // Show modal or redirect logic

                                        // Simulate redirect or modal with Stripe.js (you'd need to include Stripe.js in your HTML)
                                        if let Some(window) = window() {
                                            let _ = window.alert(/*&format!("Use Stripe client_secret: {} to add a payment method for automatic charges.", data.client_secret)*/);
                                            // In a real implementation, use Stripe.js:
                                            // stripe.confirmCardSetup(data.client_secret, {
                                            //     payment_method: { card: cardElement },
                                            // }).then(result => { /* Handle result */ });
                                        }

                                        // Clear success message after 3 seconds
                                        let success_clone = success.clone();
                                        spawn_local(async move {
                                            TimeoutFuture::new(3_000).await;
                                            success_clone.set(None);
                                        });
                                    }
                                    Err(e) => {
                                        error.set(Some(format!("Failed to create SetupIntent: {:?}", e)));
                                        // Clear error after 3 seconds
                                        let error_clone = error.clone();
                                        spawn_local(async move {
                                            TimeoutFuture::new(3_000).await;
                                            error_clone.set(None);
                                        });
                                    }
                                }
                            } else {
                                error.set(Some("Failed to connect to Stripe API".to_string()));
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

    html! {
        <div class="payment-method-container">
            <button 
                class="payment-method-button"
                onclick={open_stripe_portal}
            >
                {"Add Payment Method"}
            </button>
            {
                if *show_stripe_modal {
                    html! {
                        <div class="stripe-modal">
                            <p>{"Please complete the payment method setup in the Stripe portal to enable automatic charges."}</p>
                            <button 
                                class="close-button"
                                onclick={{
                                    let show_stripe_modal = show_stripe_modal.clone();
                                    Callback::from(move |_| show_stripe_modal.set(false))
                                }}
                            >
                                {"Close"}
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
                            {
                                if let Some(success_msg) = (*success).as_ref() {
                                    html! {
                                        <div class="message success-message" style="margin-top: 1rem;">
                                            {success_msg}
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
}
