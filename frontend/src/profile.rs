    use yew::prelude::*;
    use web_sys::{HtmlInputElement, window};
    use yew_router::prelude::*;
    use crate::Route;
    use crate::config;
    use gloo_net::http::Request;
    use serde::{Deserialize, Serialize};

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
    
    #[function_component]
    pub fn Profile() -> Html {
        let profile = use_state(|| None::<UserProfile>);
        let phone_number = use_state(String::new);
        let nickname = use_state(String::new);
        let error = use_state(|| None::<String>);
        let success = use_state(|| None::<String>);
        let is_editing = use_state(|| false);
        let active_tab = use_state(|| ProfileTab::Settings);
        let navigator = use_navigator().unwrap();
    
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

                // Validate phone number format
                if !phone.starts_with('+') {
                    error.set(Some("Phone number must start with '+'".to_string()));
                    return;
                }

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
                                        <span class="field-label">{"Email"}</span>
                                        <span class="field-value">{&user_profile.email}</span>
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
                                                        placeholder="+1234567890"
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
                                        {
                                            if user_profile.iq == 0 {
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


