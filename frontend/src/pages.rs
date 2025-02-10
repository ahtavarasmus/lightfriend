pub mod home {
    use yew::prelude::*;
    use yew_router::prelude::*;
    use crate::Route;
    use crate::config;
    use web_sys::window;
    use gloo_net::http::Request;
    use serde::Deserialize;

    #[derive(Deserialize, Clone)]
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

    pub fn is_logged_in() -> bool {
        if let Some(window) = window() {
            if let Ok(Some(storage)) = window.local_storage() {
                if let Ok(Some(_token)) = storage.get_item("token") {
                    return true;
                }
            }
        }
        false
    }


    #[function_component(Landing)]
    pub fn landing() -> Html {
        html! {
            <div class="landing-page">
                // Hero Section
                <section class="hero">
                    <h1>{"Smart Tools for Dumbphones"}</h1>
                    <p class="hero-subtitle">
                        {"Use your dumbphone smarter with AI-powered voice and text assistance."}
                    </p>
                    <button class="hero-cta">
                        <Link<Route> to={Route::Register} classes="forward-link">
                        {"Get Started"}
                        </Link<Route>>
                    </button>
                </section>

                // Features Section
                <section class="features">
                    <h2>{"Essential Tools, Minimal Distractions"}</h2>
                    <p>{"Access everything you need through your dumbphone, while staying focused and present."}</p>
                    
                    <div class="features-grid">
                        <div class="feature-item">
                            <i class="calendar-icon"></i>
                            <h3>{"Calendar Access"}</h3>
                            <p>{"Check and manage your schedule through simple voice calls or text messages."}</p>
                        </div>
                        
                        <div class="feature-item">
                            <i class="email-icon"></i>
                            <h3>{"Email Integration"}</h3>
                            <p>{"Stay on top of important emails without the constant notifications."}</p>
                        </div>

                        <div class="feature-item">
                            <i class="message-icon"></i>
                            <h3>{"Smart Messaging"}</h3>
                            <p>{"Access your messages across platforms through your dumbphone."}</p>
                        </div>

                        <div class="feature-item">
                            <i class="search-icon"></i>
                            <h3>{"Perplexity Search"}</h3>
                            <p>{"Get instant answers and information via SMS or voice call."}</p>
                        </div>
                    </div>
                </section>

                // How It Works section
                <section class="how-it-works">
                    <h2>{"How Lightfriend Works"}</h2>
                    <p>{"Three simple steps to digital freedom"}</p>

                    <div class="steps-grid">
                        <div class="step">
                            <h3>{"Connect Your Services"}</h3>
                            <p>{"Link your calendar, email, and messaging accounts through our secure web interface."}</p>
                        </div>

                        <div class="step">
                            <h3>{"Use Your Dumbphone"}</h3>
                            <p>{"Call or text Lightfriend to access your connected services anytime, anywhere."}</p>
                        </div>

                        <div class="step">
                            <h3>{"Stay Present"}</h3>
                            <p>{"Enjoy life without digital distractions, knowing essential information is just a call away."}</p>
                        </div>
                    </div>
                </section>

                <section class="footer-cta">
                    <div class="footer-content">
                        <h2>{"Ready to Reclaim Your Focus?"}</h2>
                        <p class="subtitle">
                            {"Join the digital minimalism movement without sacrificing essential connectivity."}
                        </p>
                        <button class="hero-cta">
                            <Link<Route> to={Route::Register} classes="forward-link">
                                {"Get Started Now"}
                            </Link<Route>>
                        </button>
                        <p class="disclaimer">
                            {"No smartphone required. Works with any basic phone."}
                        </p>
                    </div>
                </section>
            </div>
        }
    }

    // Separate the deletion logic
    fn delete_unverified_account(profile_id: i32, token: String) {
        wasm_bindgen_futures::spawn_local(async move {
            let _ = Request::delete(&format!("{}/api/profile/delete/{}", config::get_backend_url(), profile_id))
                .header("Authorization", &format!("Bearer {}", token))
                .send()
                .await;
            
            if let Some(window) = window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    let _ = storage.remove_item("token");
                    let _ = window.location().set_href("/");
                }
            }
        });
    }

    #[function_component]
    pub fn Home() -> Html {
        let logged_in = is_logged_in();
        let profile_data = use_state(|| None::<UserProfile>);
        let user_verified = use_state(|| false);
        let error = use_state(|| None::<String>);

        // Polling effect
        {
            let profile_data = profile_data.clone();
            let user_verified = user_verified.clone();
            let error = error.clone();
            
            use_effect_with_deps(move |_| {
                let profile_data = profile_data.clone();
                let user_verified = user_verified.clone();
                let error = error.clone();

                // Create a handle to store the interval
                let interval_handle: std::rc::Rc<std::cell::RefCell<Option<gloo_timers::callback::Interval>>> = 
                std::rc::Rc::new(std::cell::RefCell::new(None));
                let interval_handle_clone = interval_handle.clone();

                // Function to fetch profile
                let fetch_profile = move || {
                    let profile_data = profile_data.clone();
                    let user_verified = user_verified.clone();
                    let error = error.clone();
let interval_handle = interval_handle.clone();

                    gloo_console::log!("Fetching profile...");
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
                                        if let Some(window) = window() {
                                            if let Ok(Some(storage)) = window.local_storage() {
                                                let _ = storage.remove_item("token");
                                                let _ = window.location().set_href("/");
                                            }
                                        }
                                        return;
                                    }
                                    
                                    match response.json::<UserProfile>().await {
                                        Ok(profile) => {
                                            gloo_console::log!("Profile fetched successfully:", format!("verified: {}", profile.verified));
                                            // Check if unverified profile has expired
                                            if !profile.verified && profile.time_to_delete {
                                                // Profile has expired, delete account and logout
                                                delete_unverified_account(profile.id, token.clone());
                                                return;
                                            }
                                            
                                            // If user becomes verified, clear the interval
                                            if profile.verified {
                                                if let Some(interval) = interval_handle.borrow_mut().take() {
                                                    gloo_console::log!("User verified, stopping polling");
                                                    drop(interval); // This will stop the interval
                                                }
                                            }
                                            
                                            user_verified.set(profile.verified);
                                            error.set(None);
                                        }
                                        Err(_) => {
                                            gloo_console::error!("Failed to parse profile data");
                                            error.set(Some("Failed to parse profile data".to_string()));
                                        }
                                    }
                                }
                                Err(_) => {
                                    gloo_console::error!("Failed to fetch profile");
                                    error.set(Some("Failed to fetch profile".to_string()));
                                }
                            }
                        }
                    });
                };

                // Initial fetch
                fetch_profile();

                
                // Set up interval for polling
                let interval = gloo_timers::callback::Interval::new(5000, move || {
                    fetch_profile();
                });

                // Store the interval in our handle
                *interval_handle_clone.borrow_mut() = Some(interval);

                move || {
                    // Clean up interval on component unmount
                    if let Some(interval) = interval_handle_clone.borrow_mut().take() {
                        drop(interval);
                    }
                }
            }, ());
        }

        if !logged_in {
            html! { <Landing /> }
        } else {
            if !*user_verified {
                html! {
                    <div class="verification-container">
                        <div class="verification-panel">
                            <h1>{"Verify Your Account"}</h1>
                            <p>{"Call the following number to verify your account"}</p>
                            <div class="phone-display">
                                <span class="phone-number">{"+358454901522"}</span>
                            </div>
                            <div class="verification-status">
                                <i class="verification-icon"></i>
                                <span>{"Waiting for verification..."}</span>
                            </div>
                            <p class="verification-help">
                                <span>{"Having trouble? Make sure you typed your number correctly. You can change it in the profile."}</span>
                                <Link<Route> to={Route::Profile}>
                                    {"profile"}
                                </Link<Route>>

                            </p>
                        </div>
                    </div>
                }
            } else {
                html! {
                    <div class="dashboard-container">
                        <div class="dashboard-panel">
                            <div class="panel-header">
                                <h1 class="panel-title">{"Your Lightfriend Dashboard"}</h1>
                            </div>
                            <div class="info-section">
                                <h2 class="section-title">{"Your Lightfriend is Ready!"}</h2>
                                <div class="phone-display">
                                    <span class="phone-number">{"+358454901522"}</span>
                                </div>
                                <p class="instruction-text">
                                    {"Call or text this number to access your services"}
                                </p>
                            </div>
                        </div>
                    </div>
                }
            }
        }
    }
}



pub mod profile {
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

    #[function_component]
    pub fn Profile() -> Html {
        let profile = use_state(|| None::<UserProfile>);
        let phone_number = use_state(String::new);
        let nickname = use_state(String::new);
        let error = use_state(|| None::<String>);
        let success = use_state(|| None::<String>);
        let is_editing = use_state(|| false);
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
                            html! {
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
                                    <div class="profile-field">
                                        <span class="field-label">{"IQ"}</span>
                                        <span class="field-value">{user_profile.iq}{" "}<span class="time-note">{"("}{if user_profile.iq >= 60 { format!("{} minutes", user_profile.iq / 60) } else { format!("{} seconds", user_profile.iq) }}{")"}</span></span>
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
}

pub mod admin {
    use yew::prelude::*;
    use web_sys::window;
    use crate::config;
    use gloo_net::http::Request;
    use serde::{Deserialize, Serialize};
    use yew_router::prelude::*;
    use crate::Route;

    #[derive(Deserialize, Clone, Debug)]
    struct UserInfo {
        id: i32,
        username: String,
        phone_number: String,
        nickname: Option<String>,
        time_to_live: Option<i32>,
        verified: bool,
        iq: i32,
    }

    #[derive(Serialize)]
    struct UpdateUserRequest {
        username: String,
        phone_number: String,
        nickname: Option<String>,
        time_to_live: Option<i32>,
        verified: bool,
    }

    #[function_component]
    pub fn Admin() -> Html {
        let users = use_state(|| Vec::new());
        let error = use_state(|| None::<String>);
        let selected_user_id = use_state(|| None::<i32>);

        // Clone state handles for the effect
        let users_effect = users.clone();
        let error_effect = error.clone();

        use_effect_with_deps(move |_| {
            let users = users_effect;
            let error = error_effect;
            wasm_bindgen_futures::spawn_local(async move {
                // Get token from localStorage
                let token = window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|storage| storage.get_item("token").ok())
                    .flatten();

                if let Some(token) = token {
                    match Request::get(&format!("{}/api/admin/users", config::get_backend_url()))
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await
                    {
                        Ok(response) => {
                            if response.ok() {
                                match response.json::<Vec<UserInfo>>().await {
                                    Ok(data) => {
                                        users.set(data);
                                    }
                                    Err(_) => {
                                        error.set(Some("Failed to parse users data".to_string()));
                                    }
                                }
                            } else {
                                error.set(Some("Not authorized to view this page".to_string()));
                            }
                        }
                        Err(_) => {
                            error.set(Some("Failed to fetch users".to_string()));
                        }
                    }
                }
            });
            || ()
        }, ());

        let toggle_user_details = {
            let selected_user_id = selected_user_id.clone();
            Callback::from(move |user_id: i32| {
                selected_user_id.set(Some(match *selected_user_id {
                    Some(current_id) if current_id == user_id => return selected_user_id.set(None),
                    _ => user_id
                }));
            })
        };

        html! {
            <div class="dashboard-container">
                <div class="dashboard-panel">
                    <div class="panel-header">
                        <h1 class="panel-title">{"Admin Dashboard"}</h1>
                        <Link<Route> to={Route::Home} classes="back-link">
                            {"Back to Home"}
                        </Link<Route>>
                    </div>

                    {
                        if let Some(error_msg) = (*error).as_ref() {
                            html! {
                                <div class="info-section error">
                                    <span class="error-message">{error_msg}</span>
                                </div>
                            }
                        } else {
                            html! {
                                <div class="info-section">
                                    <h2 class="section-title">{"Users List"}</h2>
                                    <div class="users-table-container">
                                        <table class="users-table">
                                            <thead>
                                                <tr>
                                                    <th>{"ID"}</th>
                                                    <th>{"Username"}</th>
                                                <th>{"IQ"}</th>
                                                <th>{"Actions"}</th>
                                                </tr>
                                            </thead>
                                            <tbody>
                                                {
                                                    users.iter().map(|user| {
                                                        let is_selected = selected_user_id.as_ref() == Some(&user.id);
                                                        let user_id = user.id;
                                                        let onclick = toggle_user_details.reform(move |_| user_id);
                                                        
                                                        html! {
                                                            <>
                                                                <tr key={user.id} class={classes!("user-row", is_selected.then(|| "selected"))}>
                                                                    <td>{user.id}</td>
                                                                    <td>{&user.username}</td>
                                                                    <td>{user.iq}</td>
                                                                    <td>
                                                                        <button onclick={onclick} class="details-button">
                                                                            {if is_selected { "Hide Details" } else { "Show Details" }}
                                                                        </button>
                                                                    </td>
                                                                </tr>
                                                                if is_selected {
                                                                    <tr class="details-row">
                                                                        <td colspan="4">
                                                                            <div class="user-details">
                                                                                <p><strong>{"Phone Number: "}</strong>{&user.phone_number}</p>
                                                                                <p><strong>{"Nickname: "}</strong>{user.nickname.as_ref().map_or("None", |n| n)}</p>
                                                                                <p><strong>{"Time to Live: "}</strong>{user.time_to_live.map_or("N/A".to_string(), |ttl| ttl.to_string())}</p>
                                                                                <p><strong>{"Verified: "}</strong>{if user.verified { "Yes" } else { "No" }}</p>
                                                                            <button 
                                                                                onclick={{
                                                                                    let users = users.clone();
                                                                                    let error = error.clone();
                                                                                    let user_id = user.id;
                                                                                    Callback::from(move |_| {
                                                                                        let users = users.clone();
                                                                                        let error = error.clone();
                                                                                        wasm_bindgen_futures::spawn_local(async move {
                                                                                            if let Some(token) = window()
                                                                                                .and_then(|w| w.local_storage().ok())
                                                                                                .flatten()
                                                                                                .and_then(|storage| storage.get_item("token").ok())
                                                                                                .flatten()
                                                                                            {
                                                                                                match Request::post(&format!("{}/api/profile/increase-iq/{}", config::get_backend_url(), user_id))
                                                                                                    .header("Authorization", &format!("Bearer {}", token))
                                                                                                    .send()
                                                                                                    .await
                                                                                                {
                                                                                                    Ok(response) => {
                                                                                                        if response.ok() {
                                                                                                            // Refresh the users list after increasing IQ
                                                                                                            if let Ok(response) = Request::get(&format!("{}/api/admin/users", config::get_backend_url()))
                                                                                                                .header("Authorization", &format!("Bearer {}", token))
                                                                                                                .send()
                                                                                                                .await
                                                                                                            {
                                                                                                                if let Ok(updated_users) = response.json::<Vec<UserInfo>>().await {
                                                                                                                    users.set(updated_users);
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
                                                                                }}
                                                                                class="iq-button"
                                                                            >
                                                                                {"Get 500 IQ"}
                                                                            </button>
                                                                            <button 
                                                                                onclick={{
                                                                                    let users = users.clone();
                                                                                    let error = error.clone();
                                                                                    let user_id = user.id;
                                                                                    Callback::from(move |_| {
                                                                                        let users = users.clone();
                                                                                        let error = error.clone();
                                                                                        wasm_bindgen_futures::spawn_local(async move {
                                                                                            if let Some(token) = window()
                                                                                                .and_then(|w| w.local_storage().ok())
                                                                                                .flatten()
                                                                                                .and_then(|storage| storage.get_item("token").ok())
                                                                                                .flatten()
                                                                                            {
                                                                                                match Request::post(&format!("{}/api/profile/reset-iq/{}", config::get_backend_url(), user_id))
                                                                                                    .header("Authorization", &format!("Bearer {}", token))
                                                                                                    .send()
                                                                                                    .await
                                                                                                {
                                                                                                    Ok(response) => {
                                                                                                        if response.ok() {
                                                                                                            // Refresh the users list after resetting IQ
                                                                                                            if let Ok(response) = Request::get(&format!("{}/api/admin/users", config::get_backend_url()))
                                                                                                                .header("Authorization", &format!("Bearer {}", token))
                                                                                                                .send()
                                                                                                                .await
                                                                                                            {
                                                                                                                if let Ok(updated_users) = response.json::<Vec<UserInfo>>().await {
                                                                                                                    users.set(updated_users);
                                                                                                                }
                                                                                                            }
                                                                                                        } else {
                                                                                                            error.set(Some("Failed to reset IQ".to_string()));
                                                                                                        }
                                                                                                    }
                                                                                                    Err(_) => {
                                                                                                        error.set(Some("Failed to send request".to_string()));
                                                                                                    }
                                                                                                }
                                                                                            }
                                                                                        });
                                                                                    })
                                                                                }}
                                                                                class="iq-button reset"
                                                                            >
                                                                                {"Reset IQ"}
                                                                            </button>
                                                                            </div>
                                                                        </td>
                                                                    </tr>
                                                                }
                                                            </>
                                                        }
                                                    }).collect::<Html>()
                                                }
                                            </tbody>
                                        </table>
                                    </div>
                                </div>
                            }
                        }
                    }
                </div>
            </div>
        }
    }
}
