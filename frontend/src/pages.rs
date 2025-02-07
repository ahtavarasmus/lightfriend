pub mod home {
    use yew::prelude::*;
    use yew_router::prelude::*;
    use crate::Route;
    use web_sys::window;
    use gloo_net::http::Request;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct UserProfile {
        phone_number: Option<String>,
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
                    <h1>{"Digital Freedom Through Simplicity"}</h1>
                    <p class="hero-subtitle">
                        {"Stay connected to what matters, free from digital distractions. Use your dumbphone smarter with AI-powered voice and text assistance."}
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


    #[function_component]
    pub fn Home() -> Html {
        let logged_in = is_logged_in();
        let missing_phone = use_state(|| false);
        let profile_checked = use_state(|| false);

        // Fetch profile data if logged in
        {
            let missing_phone = missing_phone.clone();
            let profile_checked = profile_checked.clone();
            use_effect_with_deps(move |_| {
                if is_logged_in() {
                    wasm_bindgen_futures::spawn_local(async move {
                        if let Some(token) = window()
                            .and_then(|w| w.local_storage().ok())
                            .flatten()
                            .and_then(|storage| storage.get_item("token").ok())
                            .flatten()
                        {
                            match Request::get("/api/profile")
                                .header("Authorization", &format!("Bearer {}", token))
                                .send()
                                .await
                            {
                                Ok(response) => {
                                    if response.ok() {
                                        match response.json::<UserProfile>().await {
                                            Ok(profile) => {
                                                let is_missing = profile.phone_number.is_none() || 
                                                        profile.phone_number.as_ref()
                                                        .map_or(true, |p| p.trim().is_empty());
                                                missing_phone.set(is_missing);
                                                profile_checked.set(true);
                                            }
                                            Err(e) => {
                                                web_sys::console::log_1(&format!("Failed to parse profile: {:?}", e).into());
                                                profile_checked.set(true);
                                            }
                                        }
                                    } else {
                                        web_sys::console::log_1(&format!("Response not OK: {}", response.status()).into());
                                        profile_checked.set(true);
                                    }
                                }
                                Err(e) => {
                                    web_sys::console::log_1(&format!("Request failed: {:?}", e).into());
                                    profile_checked.set(true);
                                }
                            }
                        }
                    });
                } else {
                    profile_checked.set(true);
                }
                || ()
            }, ());
        }

        let handle_logout = {
            Callback::from(move |_| {
                if let Some(window) = window() {
                    if let Ok(Some(storage)) = window.local_storage() {
                        let _ = storage.remove_item("token");
                        // Reload the page to reflect the logged out state
                        let _ = window.location().reload();
                    }
                }
            })
        };

        html! {
            {
                if !logged_in {
                    html! { <Landing /> }
                } else if *profile_checked {
                    html! {
                        <div class="dashboard-container">
                            <div class="dashboard-panel">
                                <div class="panel-header">
                                    <h1 class="panel-title">{"Your Lightfriend Dashboard"}</h1>
                                    <Link<Route> to={Route::Profile} classes="back-link">{"Back to Home"}</Link<Route>>
                                </div>

                                {
                                    if *missing_phone {
                                        html! {
                                            <div class="warning-card">
                                                <span class="warning-icon">{"⚠️"}</span>
                                                <Link<Route> to={Route::Profile}>
                                                    {"Complete your setup by adding a phone number"}
                                                </Link<Route>>
                                            </div>
                                        }
                                    } else {
                                        html! {
                                            <div class="info-section">
                                                <h2 class="section-title">{"Your Lightfriend is Ready!"}</h2>
                                                <div class="phone-display">
                                                    <span class="phone-number">{"+358 45 4901522"}</span>
                                                </div>
                                                <p class="instruction-text">
                                                    {"Call or text this number to access your services"}
                                                </p>
                                            </div>
                                        }
                                    }
                                }
                                
                                <button 
                                    onclick={handle_logout}
                                    class="action-button"
                                >
                                    {"Logout"}
                                </button>
                            </div>
                        </div>
                    }

                    
                } else {
                    html! {}
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
    use gloo_net::http::Request;
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize, PartialEq)]
    struct UserProfile {
        username: String,
        email: String,
        phone_number: Option<String>,
        nickname: Option<String>,
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
                    if let Some(phone) = &user_profile.phone_number {
                        phone_number.set(phone.clone());
                    }
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
                        match Request::get("/api/profile")
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
                        match Request::post("/api/profile/update")
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
                                    if let Ok(profile_response) = Request::get("/api/profile")
                                        .header("Authorization", &format!("Bearer {}", token))
                                        .send()
                                        .await
                                    {
                                        if let Ok(updated_profile) = profile_response.json::<UserProfile>().await {
                                            profile.set(Some(updated_profile));
                                        }
                                    }
                                } else {
                                    error.set(Some("Failed to update profile".to_string()));
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
                        if let Some(user_profile) = (*profile).as_ref() {
                            html! {
                                <div class="profile-info">
                                    <div class="profile-field">
                                        <span class="field-label">{"Username"}</span>
                                        <span class="field-value">{&user_profile.username}</span>
                                    </div>
                                    
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
                                                        onchange={let phone_number = phone_number.clone(); move |e: Event| {
                                                            let input: HtmlInputElement = e.target_unchecked_into();
                                                            phone_number.set(input.value());
                                                        }}
                                                    />
                                                }
                                            } else {
                                                html! {
                                                    <span class="field-value">
                                                        {user_profile.phone_number.clone().unwrap_or_default()}
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
    use gloo_net::http::Request;
    use serde::Deserialize;

    #[derive(Deserialize, Clone, Debug)]
    struct UserInfo {
        id: i32,
        username: String,
        email: String,
    }

    #[function_component]
    pub fn Admin() -> Html {
        let users = use_state(|| Vec::new());
        let error = use_state(|| None::<String>);

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
                    match Request::get("/api/admin/users")
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

        html! {
            <div class="admin-container">
                <h1>{"Admin Dashboard"}</h1>
                {
                    if let Some(error_msg) = (*error).as_ref().clone() {
                        html! {
                            <div class="error-message">
                                {error_msg}
                            </div>
                        }
                    } else {
                        html! {
                            <div class="users-list">
                                <h2>{"Users List"}</h2>
                                <table>
                                    <thead>
                                        <tr>
                                            <th>{"ID"}</th>
                                            <th>{"Username"}</th>
                                            <th>{"Email"}</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {
                                            users.iter().map(|user| {
                                                html! {
                                                    <tr key={user.id}>
                                                        <td>{user.id}</td>
                                                        <td>{&user.username}</td>
                                                        <td>{&user.email}</td>
                                                    </tr>
                                                }
                                            }).collect::<Html>()
                                        }
                                    </tbody>
                                </table>
                            </div>
                        }
                    }
                }
            </div>
        }
    }
}
