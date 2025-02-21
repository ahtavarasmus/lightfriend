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
        info: Option<String>,
    }

    const MAX_NICKNAME_LENGTH: usize = 30;
    const MAX_INFO_LENGTH: usize = 500;

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

        let on_edit = {
            let email = email.clone();
            let phone_number = phone_number.clone();
            let nickname = nickname.clone();
            let info = info.clone();
            let error = error.clone();
            let success = success.clone();
            let profile = profile.clone();
            let is_editing = is_editing.clone();
            let navigator = navigator.clone();

            Callback::from(move |_e: MouseEvent| {
                let email_str = (*email).clone();
                let phone = (*phone_number).clone();
                let nick = (*nickname).clone();
                let user_info = (*info).clone();
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
                            .json(&UpdateProfileRequest { 
                                email: email_str,
                                phone_number: phone,
                                nickname: nick,
                                info: user_info,
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
                                    error.set(Some("Failed to update profile. Phone number/email already exists?".to_string()));
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
                                        {
                                            if *is_editing {
                                                html! {
                                                    <input
                                                        type="email"
                                                        class="profile-input"
                                                        value={(*email).clone()}
                                                        placeholder="your@email.com"
                                                        onchange={let email = email.clone(); move |e: Event| {
                                                            let input: HtmlInputElement = e.target_unchecked_into();
                                                            email.set(input.value());
                                                        }}
                                                    />
                                                }
                                            } else {
                                                html! {
                                                    <span class="field-value">{&user_profile.email}</span>
                                                }
                                            }
                                        }
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
                                        <div class="field-label-group">
                                            <span class="field-label">{"Nickname"}</span>
                                            <div class="tooltip">
                                                <span class="tooltip-icon">{"?"}</span>
                                                <span class="tooltip-text">
                                                    {"This is how the AI assistant will address you in conversations. It will use this name to greet you and make interactions more personal."}
                                                </span>
                                            </div>
                                        </div>
                                        {
                                            if *is_editing {
                                                html! {
                                                    <div class="input-with-limit">
                                                        <input
                                                            type="text"
                                                            class="profile-input"
                                                            value={(*nickname).clone()}
                                                            maxlength={MAX_NICKNAME_LENGTH.to_string()}
                                                            onchange={let nickname = nickname.clone(); move |e: Event| {
                                                                let input: HtmlInputElement = e.target_unchecked_into();
                                                                let value = input.value();
                                                                if value.chars().count() <= MAX_NICKNAME_LENGTH {
                                                                    nickname.set(value);
                                                                }
                                                            }}
                                                        />
                                                        <span class="char-count">
                                                            {format!("{}/{}", (*nickname).chars().count(), MAX_NICKNAME_LENGTH)}
                                                        </span>
                                                    </div>
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

                                    <div class="profile-field">
                                        <div class="field-label-group">
                                            <span class="field-label">{"Info"}</span>
                                            <div class="tooltip">
                                                <span class="tooltip-icon">{"?"}</span>
                                                <span class="tooltip-text">
                                                    {"What would you like the AI assistant to know about you? For example, your location, preferred units (metric/imperial), language preferences, or any specific way you'd like the assistant to respond to you."}
                                                </span>
                                            </div>
                                        </div>
                                        {
                                            if *is_editing {
                                                html! {
                                                    <div class="input-with-limit">
                                                        <textarea
                                                            class="profile-input"
                                                            value={(*info).clone()}
                                                            maxlength={MAX_INFO_LENGTH.to_string()}
                                                            placeholder="Tell something about yourself or how the assistant should respond to you"
                                                            onchange={let info = info.clone(); move |e: Event| {
                                                                let input: HtmlInputElement = e.target_unchecked_into();
                                                                let value = input.value();
                                                                if value.chars().count() <= MAX_INFO_LENGTH {
                                                                    info.set(value);
                                                                }
                                                            }}
                                                        />
                                                        <span class="char-count">
                                                            {format!("{}/{}", (*info).chars().count(), MAX_INFO_LENGTH)}
                                                        </span>
                                                    </div>
                                                }
                                            } else {
                                                html! {
                                                    <span class="field-value">
                                                        {user_profile.info.clone().unwrap_or("I'm from finland, always use Celsious and metric system, etc...".to_string())}
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


