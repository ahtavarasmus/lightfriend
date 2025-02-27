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
    phone_number: String,
    nickname: Option<String>,
    verified: bool,
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

#[derive(Clone, PartialEq)]
enum ProfileTab {
    Settings,
    Billing,
}

#[function_component]
pub fn Profile() -> Html {
    let profile = use_state(|| None::<UserProfile>);
    let email = use_state(String::new);
    let phone_number = use_state(String::new);
    let nickname = use_state(String::new);
    let info = use_state(String::new);
    let error = use_state(|| None::<String>);
    let success = use_state(|| None::<String>);
    let is_editing = use_state(|| false);
    let active_tab = use_state(|| ProfileTab::Settings);
    let navigator = use_navigator().unwrap();
    let portal_url = use_state(|| None::<String>);

       // Check authentication
    {
        let navigator = navigator.clone();
        use_effect_with_deps(move |_| {
            let is_authenticated = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|storage| storage.get_item("token").ok())
                .flatten()
                .is_some();

            if !is_authenticated {
                navigator.push(&Route::Home);
            }
            || ()
        }, ());
    }

    // Initialize phone_number state when profile is loaded
    {
        let email = email.clone();
        let phone_number = phone_number.clone();
        let nickname = nickname.clone();
        let info = info.clone();
        let profile = profile.clone();
        use_effect_with_deps(move |profile| {
            if let Some(user_profile) = (**profile).as_ref() {
                email.set(user_profile.email.clone());
                phone_number.set(user_profile.phone_number.clone());
                if let Some(nick) = &user_profile.nickname {
                    nickname.set(nick.clone());
                }
                if let Some(user_info) = &user_profile.info {
                    info.set(user_info.clone());
                }
            }
            || ()
        }, profile.clone());
    }

    // Fetch user profile 
    {
        let profile = profile.clone();
        let error = error.clone();
        use_effect_with_deps(move |_| {
            wasm_bindgen_futures::spawn_local(async move {
                if let Some(token) = window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|storage| storage.get_item("token").ok())
                    .flatten()
                {
                    match Request::get(&format!("{}/api/profile", config::get_backend_url()))
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await
                    {
                        Ok(response) => {
                            if response.status() == 401 {
                                // Handle unauthorized access
                                if let Some(window) = window() {
                                    if let Ok(Some(storage)) = window.local_storage() {
                                        let _ = storage.remove_item("token");
                                        let _ = window.location().set_href("/login");
                                    }
                                }
                                return;
                            } else if response.ok() {
                                match response.json::<UserProfile>().await {
                                    Ok(data) => {
                                        profile.set(Some(data));
                                    }
                                    Err(_) => {
                                        error.set(Some("Failed to parse profile data".to_string()));
                                    }
                                }
                            }
                        }
                        Err(_) => {
                            error.set(Some("Failed to fetch profile".to_string()));
                        }
                    }
                }
            });
            || ()
        }, ());
    }

    let profile_data = (*profile).clone();

    html! {
        <div class="profile-container">
            <div class="profile-panel">
                <div class="profile-header">
                    <h1 class="profile-title">{"Profile"}</h1>
                    <Link<Route> to={Route::Home} classes="back-link">
                        {"Back to Home"}
                    </Link<Route>>
                </div>
                <div class="profile-tabs">
                    <button 
                        class={classes!("tab-button", (*active_tab == ProfileTab::Settings).then(|| "active"))}
                        onclick={{
                            let active_tab = active_tab.clone();
                            Callback::from(move |_| active_tab.set(ProfileTab::Settings))
                        }}
                    >
                        {"Settings"}
                    </button>
                    <button 
                        class={classes!("tab-button", (*active_tab == ProfileTab::Billing).then(|| "active"))}
                        onclick={{
                            let active_tab = active_tab.clone();
                            Callback::from(move |_| active_tab.set(ProfileTab::Billing))
                        }}
                    >
                        {"Billing"}
                    </button>
                </div>
                {
                    if let Some(error_msg) = (*error).as_ref() {
                        html! {
                            <div class="message error-message">{error_msg}</div>
                        }
                    } else if let Some(success_msg) = (*success).as_ref() {
                        html! {
                            <div class="message success-message">{success_msg}</div>
                        }
                    } else {
                        html! {}
                    }
                }

                {
                    if let Some(user_profile) = profile_data {
                        match *active_tab {
                            ProfileTab::Settings => html! {
                                // add here the settings
                                <SettingsPage />
                            },
                        ProfileTab::Billing => html! {
                            // add here the billing
                            <BillingPage user_profile={user_profile}/>
                        }
                    }
                } else {
                    html! {
                        <div class="loading-profile">{"Loading profile..."}</div>
                    }
                }
            }
        </div>
    </div>
    }
}

