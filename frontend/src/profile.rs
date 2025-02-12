use yew::prelude::*;
use web_sys::{HtmlInputElement, window, InputEvent};
use yew_router::prelude::*;
use crate::Route;
use crate::config;
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Clone, PartialEq)]
struct UserProfile {
    id: i32,
    username: String,
    phone_number: String,
    nickname: Option<String>,
    verified: bool,
    time_to_live: i32,
    time_to_delete: bool,
    iq: i32,
}

#[derive(Serialize)]
struct UpdateProfileRequest {
    phone_number: String,
    nickname: String,
}

#[derive(Clone, PartialEq)]
enum ProfileTab {
    Settings,
    Billing,
}

#[derive(Serialize)]
struct BuyIqRequest {
    amount: i32,
    user_id: i32,
}

#[function_component]
pub fn Profile() -> Html {
    let profile = use_state(|| None::<UserProfile>);
    let phone_number = use_state(String::new);
    let nickname = use_state(String::new);
    let error = use_state(|| None::<String>);
    let success = use_state(|| None::<String>);
    let is_editing = use_state(|| false);
    let active_tab = use_state(|| ProfileTab::Settings);
    let iq_amount = use_state(|| String::new());
    let navigator = use_navigator().unwrap();

    // Check for purchase success parameter
    {
        let success = success.clone();
        let active_tab = active_tab.clone();
        use_effect_with_deps(move |_| {
            if let Some(window) = window() {
                if let Ok(location) = window.location().search() {
                    if location.contains("purchase=success") {
                        success.set(Some("IQ successfully added to your account!".to_string()));
                        active_tab.set(ProfileTab::Billing);
                        
                        // Clear success message after 5 seconds
                        let success_clone = success.clone();
                        wasm_bindgen_futures::spawn_local(async move {
                            gloo_timers::future::TimeoutFuture::new(5_000).await;
                            success_clone.set(None);
                        });
                    }
                }
            }
            || ()
        }, ());
    }

    // Check authentication immediately
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
        let phone_number = phone_number.clone();
        let nickname = nickname.clone();
        let profile = profile.clone();
        use_effect_with_deps(move |profile| {
            if let Some(user_profile) = (**profile).as_ref() {
                phone_number.set(user_profile.phone_number.clone());
                if let Some(nick) = &user_profile.nickname {
                    nickname.set(nick.clone());
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

    let on_edit = {
        let phone_number = phone_number.clone();
        let nickname = nickname.clone();
        let error = error.clone();
        let success = success.clone();
        let profile = profile.clone();
        let is_editing = is_editing.clone();
        let navigator = navigator.clone();

        Callback::from(move |_e: MouseEvent| {
            let phone = (*phone_number).clone();
            let nick = (*nickname).clone();
            let error = error.clone();
            let success = success.clone();
            let profile = profile.clone();
            let is_editing = is_editing.clone();
            let navigator = navigator.clone();

            // Check authentication first
            let is_authenticated = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|storage| storage.get_item("token").ok())
                .flatten()
                .is_some();

            if !is_authenticated {
                navigator.push(&Route::Home);
                return;
            }

            wasm_bindgen_futures::spawn_local(async move {
                if let Some(token) = window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|storage| storage.get_item("token").ok())
                    .flatten()
                {
                    match Request::post(&format!("{}/api/profile/update", config::get_backend_url()))
                        .header("Authorization", &format!("Bearer {}", token))
                        .json(&UpdateProfileRequest { phone_number: phone,
                            nickname: nick,
                        })
                        .expect("Failed to build request")
                        .send()
                        .await 
                    {
                        Ok(response) => {
                            if response.status() == 401 {
                                // Token is invalid or expired
                                if let Some(window) = window() {
                                    if let Ok(Some(storage)) = window.local_storage() {
                                        let _ = storage.remove_item("token");
                                        navigator.push(&Route::Home);
                                        return;
                                    }
                                }
                            } else if response.ok() {
                                success.set(Some("Profile updated successfully".to_string()));
                                error.set(None);
                                is_editing.set(false);
                                
                                // Clear success message after 3 seconds
                                let success_clone = success.clone();
                                wasm_bindgen_futures::spawn_local(async move {
                                    gloo_timers::future::TimeoutFuture::new(3_000).await;
                                    success_clone.set(None);
                                });
                                
                                // Fetch updated profile data after successful update
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
                                error.set(Some("Failed to update profile. Phone number already exists?".to_string()));
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
                            <div class="profile-info">
                                <div class="profile-field">
                                    <span class="field-label">{"Username"}</span>
                                    <span class="field-value">{&user_profile.username}</span>
                                </div>
                                
                                <div class="profile-field">
                                    <span class="field-label">{"Phone"}</span>
                                    {
                                        if *is_editing {
                                            html! {
                                                <input
                                                    type="tel"
                                                    class="profile-input"
                                                    value={(*phone_number).clone()}
                                                    onchange={let phone_number = phone_number.clone(); move |e: Event| {
                                                        let input: HtmlInputElement = e.target_unchecked_into();
                                                        phone_number.set(input.value());
                                                    }}
                                                />
                                            }
                                        } else {
                                            html! {
                                                <span class="field-value">
                                                    {user_profile.phone_number.clone()}
                                                </span>
                                            }
                                        }
                                    }
                                </div>

                                <div class="profile-field">
                                    <span class="field-label">{"Nickname"}</span>
                                    {
                                        if *is_editing {
                                            html! {
                                                <input
                                                    type="text"
                                                    class="profile-input"
                                                    value={(*nickname).clone()}
                                                    onchange={let nickname = nickname.clone(); move |e: Event| {
                                                        let input: HtmlInputElement = e.target_unchecked_into();
                                                        nickname.set(input.value());
                                                    }}
                                                />
                                            }
                                        } else {
                                            html! {
                                                <span class="field-value">
                                                    {user_profile.nickname.clone().unwrap_or_default()}
                                                </span>
                                            }
                                        }
                                    }
                                </div>
                                
                                <button 
                                    onclick={
                                        let is_editing = is_editing.clone();
                                        if *is_editing {
                                            on_edit
                                        } else {
                                            Callback::from(move |_| is_editing.set(true))
                                        }
                                    }
                                    class={classes!("edit-button", (*is_editing).then(|| "confirming"))}
                                >
                                    {if *is_editing { "Save Changes" } else { "Edit Profile" }}
                                </button>
                            </div>
                        },
                        ProfileTab::Billing => html! {
                            <div class="profile-info">
                                <div class="billing-section">
                                    <h3>{"IQ Balance"}</h3>
                                    <div class="iq-balance">
                                        <span class="iq-amount">{user_profile.iq}</span>
                                        <span class="iq-time">
                                            {if user_profile.iq >= 60 { 
                                                format!("({} minutes)", user_profile.iq / 60)
                                            } else { 
                                                format!("({} seconds)", user_profile.iq)
                                            }}
                                        </span>
                                    </div>
                                    <div class="billing-info">
                                        <div class="iq-purchase-form">
                                            <input
                                                type="number"
                                                min="1500"
                                                step="1"
                                                class="iq-amount-input"
                                                placeholder="Enter IQ amount (min 1500)"
                                                value={(*iq_amount).clone()}
                                                oninput={
                                                    let iq_amount = iq_amount.clone();
                                                    move |e: InputEvent| {
                                                        let input: HtmlInputElement = e.target_unchecked_into();
                                                        // Only allow numeric values
                                                        if let Ok(num) = input.value().parse::<i32>() {
                                                            iq_amount.set(input.value());
                                                        }
                                                    }
                                                }
                                            />
                                            {
                                                if let Ok(amount) = (*iq_amount).parse::<i32>() {
                                                    if amount > 0 {
                                                        let hours = amount / 3600;
                                                        let minutes = (amount % 3600) / 60;
                                                        let seconds = amount % 60;
                                                        let cost = (amount as f64 / 60.0) * 0.2;
                                                        html! {
                                                            <div class="iq-conversion-info">
                                                                <p class="time-conversion">
                                                                    {
                                                                        if hours > 0 {
                                                                            if minutes > 0 && seconds > 0 {
                                                                                format!("= {} hours {} minutes {} seconds", hours, minutes, seconds)
                                                                            } else if minutes > 0 {
                                                                                format!("= {} hours {} minutes", hours, minutes)
                                                                            } else if seconds > 0 {
                                                                                format!("= {} hours {} seconds", hours, seconds)
                                                                            } else {
                                                                                format!("= {} hours", hours)
                                                                            }
                                                                        } else if minutes > 0 {
                                                                            if seconds > 0 {
                                                                                format!("= {} minutes {} seconds", minutes, seconds)
                                                                            } else {
                                                                                format!("= {} minutes", minutes)
                                                                            }
                                                                        } else {
                                                                            format!("= {} seconds", seconds)
                                                                        }
                                                                    }
                                                                </p>
                                                                <p class="cost-info">
                                                                    {format!("Cost: {:.2}€", cost)}
                                                                </p>
                                                            </div>
                                                        }
                                                    } else {
                                                        html! {}
                                                    }
                                                } else {
                                                    html! {}
                                                }
                                            }
                                            <button 
                                                class="iq-button"
                                                onclick={{
                                                    let profile = profile.clone();
                                                    let error = error.clone();
                                                    let success = success.clone();
                                                    let iq_amount = iq_amount.clone();
                                                    
                                                    Callback::from(move |_| {
                                                        let profile = profile.clone();
                                                        let error = error.clone();
                                                        let success = success.clone();
                                                        let amount_str = (*iq_amount).clone();
                                                        
                                                        // Parse the amount, show error if invalid
                                                        let amount = match amount_str.parse::<i32>() {
                                                            Ok(n) if n >= 1500 => n,
                                                            _ => {
                                                                error.set(Some("Please enter a valid amount (minimum 1500 IQ / 5€)".to_string()));
                                                                return;
                                                            }
                                                        };

                                                        wasm_bindgen_futures::spawn_local(async move {
                                                            if let Some(token) = window()
                                                                .and_then(|w| w.local_storage().ok())
                                                                .flatten()
                                                                .and_then(|storage| storage.get_item("token").ok())
                                                                .flatten()
                                                            {
                                                                match Request::post(&format!("{}/api/profile/buy-iq", config::get_backend_url()))
                                                                    .header("Authorization", &format!("Bearer {}", token))
                                                                    .json(&BuyIqRequest { 
                                                                        amount,
                                                                        user_id: profile.as_ref().map(|p| p.id).unwrap_or_default()
                                                                    })
                                                                    .expect("Failed to build request")
                                                                    .send()
                                                                    .await
                                                                {
                                                                    Ok(response) => {
                                                                        if response.status() == 401 {
                                                                            // Token is invalid or expired
                                                                            if let Some(window) = window() {
                                                                                if let Ok(Some(storage)) = window.local_storage() {
                                                                                    let _ = storage.remove_item("token");
                                                                                    // Redirect to home page
                                                                                    let _ = window.location().set_href("/");
                                                                                    return;
                                                                                }
                                                                            }
                                                                        } else if response.ok() {
                                                                            match response.json::<serde_json::Value>().await {
                                                                                Ok(data) => {
                                                                                    if let Some(checkout_url) = data.get("checkout_url").and_then(|u| u.as_str()) {
                                                                                        // Redirect to the Lemon Squeezy checkout
                                                                                        if let Some(window) = window() {
                                                                                            let _ = window.location().set_href(checkout_url);
                                                                                        }
                                                                                    } else {
                                                                                        error.set(Some("Invalid response from server".to_string()));
                                                                                    }
                                                                                }
                                                                                Err(_) => {
                                                                                    error.set(Some("Failed to parse server response".to_string()));
                                                                                }
                                                                            }
                                                                        } else {
                                                                            // Try to get detailed error message from response
                                                                            match response.json::<serde_json::Value>().await {
                                                                                Ok(error_data) => {
                                                                                    let error_msg = error_data.get("error")
                                                                                        .and_then(|e| e.as_str())
                                                                                        .unwrap_or("Failed to process payment request");
                                                                                    error.set(Some(error_msg.to_string()));
                                                                                }
                                                                                Err(_) => {
                                                                                    error.set(Some("Failed to process payment request".to_string()));
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                    Err(_) => {
                                                                        error.set(Some("Failed to connect to server".to_string()));
                                                                    }
                                                                }
                             
                                                            }
                                                        });
                                                    })
                                                }}
                                            >
                                                {"Buy IQ"}
                                            </button>
                                        </div>
                                    </div>
                                </div>
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
    </div>
    }
}


