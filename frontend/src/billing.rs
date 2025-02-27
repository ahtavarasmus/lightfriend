use yew::prelude::*;
use web_sys::{HtmlInputElement, window};
use yew_router::prelude::*;
use crate::Route;
use crate::config;
use crate::usage_graph::UsageGraph;
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use crate::money::CheckoutButton;
use chrono::{DateTime, TimeZone, Utc};
use wasm_bindgen_futures::spawn_local;
use std::str::FromStr;

#[derive(Deserialize, Clone, PartialEq)]
struct SubscriptionInfo {
    id: String,
    status: String,
    next_bill_date: i32,
    stage: String,
    is_scheduled_to_cancel: Option<bool>,
}

#[derive(Deserialize, Clone, PartialEq)]
struct PaddlePortalSessionResponse {
    portal_url: String,
}

#[derive(Deserialize, Clone, PartialEq)]
struct UserProfile {
    id: i32,
    email: String,
    time_to_live: i32,
    time_to_delete: bool,
    iq: i32,
    info: Option<String>,
    subscription: Option<SubscriptionInfo>,
}

const MAX_NICKNAME_LENGTH: usize = 30;
const MAX_INFO_LENGTH: usize = 500;

fn format_timestamp(timestamp: i32) -> String {
    match Utc.timestamp_opt(timestamp as i64, 0) {
        chrono::offset::LocalResult::Single(dt) => {
            dt.format("%B %d, %Y").to_string()
        },
        _ => "Unknown date".to_string(),
    }
}

#[derive(Serialize)]
struct UpdateProfileRequest {
    email: String,
    phone_number: String,
    nickname: String,
    info: String,
}

#[derive(Properties, PartialEq, Clone)]
struct BillingPageProps {
    pub user_profile: Option<UserProfile>,
    pub success: UseStateHandle<Option<String>>,
    pub error: UseStateHandle<Option<String>>
}

#[function_component]
pub fn BillingPage(props: &BillingPageProps) -> Html {
    
    let user_profile = props.user_profile;

    // Auto top-up related states
    let show_auto_topup_modal = use_state(|| false);
    let auto_topup_active = use_state(|| false);
    let auto_topup_amount = use_state(|| "5.00".to_string());
    let auto_topup_threshold = use_state(|| "2.00".to_string());



    html! {
        <div class="profile-info">
            <div class="billing-section">
            // turn true when billing comes
            if true {
                    {
                        if user_profile.iq < 0 {
                            html! {
                                <>
                                <h3>{"IQ Usage this month"}</h3>
                                <div class="iq-balance">
                                    <span class="iq-time">
                                        { 
                                            format!("{} IQ (approx. {:.2}€)", user_profile.iq.abs(), (user_profile.iq.abs() as f64 / 300.0))
                                        }
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
                                            format!("{} IQ ({} seconds)", user_profile.iq, user_profile.iq)
                                        }}
                                    </span>

                                </div>
                                
                                <button 
                                    class="auto-topup-button"
                                    onclick={{
                                        let show_modal = show_auto_topup_modal.clone();
                                        Callback::from(move |_| {
                                            show_modal.set(!*show_modal);
                                        })
                                    }}
                                >
                                    {"Automatic Top-up"}
                                </button>
                                
                                {
                                    if *show_auto_topup_modal {
                                        html! {
                                            <div class="auto-topup-modal">
                                                <div class="auto-topup-toggle">
                                                    <span>{"Automatic Top-up"}</span>
                                                    <label class="switch">
                                                        <input 
                                                            type="checkbox" 
                                                            checked={*auto_topup_active}
                                                            onchange={{
                                                                let auto_topup_active = auto_topup_active.clone();
                                                                Callback::from(move |e: Event| {
                                                                    let input: HtmlInputElement = e.target_unchecked_into();
                                                                    auto_topup_active.set(input.checked());
                                                                })
                                                            }}
                                                        />
                                                        <span class="slider round"></span>
                                                    </label>
                                                </div>
                                                
                                                <div class="current-balance">
                                                    <span>{"Currently: "}</span>
                                                    <span class="balance-amount">{format!("€{}", (user_profile.iq as f64 / 300.0).max(0.0).to_string())}</span>
                                                </div>
                                                
                                                {
                                                    if *auto_topup_active {
                                                        html! {
                                                            <>
                                                                <div class="topup-settings">
                                                                    <p>{"How much would you like to automatically top up when your balance drops below €2.00?"}</p>
                                                                    <div class="amount-input-container">
                                                                        <label for="amount">{"Amount"}</label>
                                                                        <input 
                                                                            id="amount"
                                                                            type="text" 
                                                                            class="amount-input"
                                                                            value={(*auto_topup_amount).clone()}
                                                                            onchange={{
                                                                                let auto_topup_amount = auto_topup_amount.clone();
                                                                                Callback::from(move |e: Event| {
                                                                                    let input: HtmlInputElement = e.target_unchecked_into();
                                                                                    auto_topup_amount.set(input.value());
                                                                                })
                                                                            }}
                                                                        />
                                                                    </div>
                                                                    <button 
                                                                        class="save-button"
                                                                        onclick={{
                                                                            let success = success.clone();
                                                                            let error = error.clone();
                                                                            let show_modal = show_auto_topup_modal.clone();
                                                                            Callback::from(move |_| {
                                                                                // Here you would implement the API call to save the settings
                                                                                success.set(Some("Auto top-up settings saved successfully".to_string()));
                                                                                
                                                                                // Clear success message after 3 seconds
                                                                                let success_clone = success.clone();
                                                                                wasm_bindgen_futures::spawn_local(async move {
                                                                                    gloo_timers::future::TimeoutFuture::new(3_000).await;
                                                                                    success_clone.set(None);
                                                                                });
                                                                            })
                                                                        }}
                                                                    >
                                                                        {"Save"}
                                                                    </button>
                                                                </div>
                                                            </>
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
                                </>
                            }
                        }
                    }

                <UsageGraph user_id={user_profile.id} />
            <div class="billing-info">
                {
                    if let Some(subscription) = &user_profile.subscription {
                        let next_bill_date = format_timestamp(subscription.next_bill_date);
                        html! {
                            <div class="subscription-info">
                                <h4>{"Active Subscription"}</h4>
                                <p>
                                    <span class="label">{"Status:"}</span>
                                    {
                                        if let Some(true) = subscription.is_scheduled_to_cancel {
                                            html! {
                                                <span class="value status-canceled">{"to be cancelled"}</span>
                                            }
                                        } else {
                                            html! {
                                                <span class={classes!("value", format!("status-{}", subscription.status.to_lowercase()))}>
                                                    {subscription.status.clone()}
                                                </span>
                                            }
                                        }
                                    }
                                </p>

                                <p>
                                    <span class="label">{"Next billing date:"}</span>
                                    <span class="value">{next_bill_date}</span>
                                </p>
                                <p>
                                    <span class="label">{"Subscription plan:"}</span>
                                    <span class="value">{subscription.stage.clone()}</span>
                                </p>
                                <div class="subscription-actions">
                                    <a 
                                        href={
                                            if let Some(url) = (*portal_url).clone() {
                                                url
                                            } else {
                                                "#".to_string()
                                            }
                                        }
                                        target="_blank" 
                                        class="paddle-dashboard-button"
                                        onclick={
                                            let portal_url = portal_url.clone();
                                            let user_id = user_profile.id;
                                            Callback::from(move |e: MouseEvent| {
                                                if (*portal_url).is_none() {
                                                    e.prevent_default();
                                                    let portal_url = portal_url.clone();
                                                    spawn_local(async move {
                                                        if let Some(token) = window()
                                                            .and_then(|w| w.local_storage().ok())
                                                            .flatten()
                                                            .and_then(|storage| storage.get_item("token").ok())
                                                            .flatten()
                                                        {
                                                            match Request::get(&format!("{}/api/profile/get-customer-portal-link/{}", config::get_backend_url(), user_id))
                                                                .header("Authorization", &format!("Bearer {}", token))
                                                                .send()
                                                                .await
                                                            {
                                                                Ok(response) => {
                                                                    if response.ok() {
                                                                        if let Ok(data) = response.json::<PaddlePortalSessionResponse>().await {
                                                                            portal_url.set(Some(data.portal_url.clone()));
                                                                            // Redirect to the portal URL
                                                                            if let Some(window) = window() {
                                                                                let _ = window.open_with_url(&data.portal_url);
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                                Err(_) => {
                                                                    // Handle error
                                                                }
                                                            }
                                                        }
                                                    });
                                                }
                                            })
                                        }
                                    >
                                        {"Manage Subscription"}
                                    </a>
                                    <CheckoutButton user_id={user_profile.id} user_email={user_profile.email} />
                                </div>
                            </div>
                        }
                    } else {
                        html! {
                            <>
                                <p>{"Subscribe to usage based billing, pay only for what you use monthly."}</p>
                                <CheckoutButton user_id={user_profile.id} user_email={user_profile.email} />
                            </>
                        }
                    }
                }
            </div>
            } else {
                // remove from here when paddle is ready
            <h3>{"IQ Balance"}</h3>
                <div class="iq-balance">
                    <span class="iq-amount">{user_profile.iq}</span>
                    <span class="iq-time">
                        {if user_profile.iq >= 60 { 
                            format!("({} minutes/messages)", user_profile.iq / 60)
                        } else { 
                            format!("({} seconds)", user_profile.iq)
                        }}
                    </span>
                </div>
                {
                    if user_profile.iq <= 0 {
                        let onclick = {
                            let profile = profile.clone();
                            let error = error.clone();
                            let success = success.clone();
                            Callback::from(move |_| {
                                let profile = profile.clone();
                                let error = error.clone();
                                let success = success.clone();
                                wasm_bindgen_futures::spawn_local(async move {
                                    if let Some(token) = window()
                                        .and_then(|w| w.local_storage().ok())
                                        .flatten()
                                        .and_then(|storage| storage.get_item("token").ok())
                                        .flatten()
                                    {
                                        match Request::post(&format!("{}/api/profile/increase-iq/{}", config::get_backend_url(), user_profile.id))
                                            .header("Authorization", &format!("Bearer {}", token))
                                            .send()
                                            .await
                                        {
                                            Ok(response) => {
                                                if response.ok() {
                                                    success.set(Some("IQ increased successfully".to_string()));
                                                    error.set(None);
                                                    
                                                    // Fetch updated profile
                                                    if let Ok(profile_response) = Request::get(&format!("{}/api/profile", config::get_backend_url()))
                                                        .header("Authorization", &format!("Bearer {}", token))
                                                        .send()
                                                        .await
                                                    {
                                                        if let Ok(updated_profile) = profile_response.json::<UserProfile>().await {
                                                            profile.set(Some(updated_profile));
                                                        }
                                                    }
                                                } else {
                                                    error.set(Some("Failed to increase IQ".to_string()));
                                                }
                                            }
                                            Err(_) => {
                                                error.set(Some("Failed to send request".to_string()));
                                            }
                                        }
                                    }
                                });
                            })
                        };
                        html! {
                            <button onclick={onclick} class="iq-button">
                                {"Get 500 IQ"}
                            </button>

                        }
                    } else {
                        html! {}
                    }
                }
            <div class="billing-info">
                <p>{"Purchase additional IQ soon... for now you can just add more IQ for free if they run out"}</p>
            </div>
            }
        </div>
    </div>
    }
}
