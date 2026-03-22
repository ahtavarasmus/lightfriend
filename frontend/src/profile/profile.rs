use crate::profile::billing_credits::BillingPage;
use crate::profile::billing_models::UserProfile;
use crate::utils::api::Api;
use crate::Route;
use wasm_bindgen_futures::spawn_local;
use web_sys::window;
use web_sys::UrlSearchParams;
use yew::prelude::*;
use yew_router::prelude::*;

#[function_component]
pub fn Billing() -> Html {
    let profile = use_state(|| None::<UserProfile>);
    let error = use_state(|| None::<String>);
    let success = use_state(|| None::<String>);
    let location = use_location().unwrap();

    // Check for subscription success parameter
    {
        let success = success.clone();
        use_effect_with_deps(
            move |_| {
                let query = location.query_str();
                if let Ok(params) = UrlSearchParams::new_with_str(query) {
                    if params.has("subscription")
                        && params.get("subscription").unwrap_or_default() == "success"
                    {
                        success.set(Some("Subscription activated successfully!".to_string()));

                        // Clean up the URL after showing the message
                        if let Some(window) = window() {
                            if let Ok(history) = window.history() {
                                let _ = history.replace_state_with_url(
                                    &wasm_bindgen::JsValue::NULL,
                                    "",
                                    Some("/billing"),
                                );
                            }
                        }
                    }
                    if params.has("subscription")
                        && params.get("subscription").unwrap_or_default() == "canceled"
                    {
                        success.set(Some("Subscription canceled successfully.".to_string()));

                        // Clean up the URL after showing the message
                        if let Some(window) = window() {
                            if let Ok(history) = window.history() {
                                let _ = history.replace_state_with_url(
                                    &wasm_bindgen::JsValue::NULL,
                                    "",
                                    Some("/billing"),
                                );
                            }
                        }
                    }
                }
                || ()
            },
            (),
        );
    }

    // Authentication is handled by the profile fetch - if 401, user will be redirected

    // Fetch user profile
    {
        let profile = profile.clone();
        let error = error.clone();
        use_effect_with_deps(
            move |_| {
                spawn_local(async move {
                    match Api::get("/api/profile").send().await {
                        Ok(response) => {
                            // Automatic retry handles 401 with token refresh and redirect
                            // We only need to handle successful responses
                            if response.ok() {
                                match response.json::<UserProfile>().await {
                                    Ok(data) => {
                                        profile.set(Some(data));
                                    }
                                    Err(_) => {
                                        error.set(Some("Failed to parse profile data".to_string()));
                                    }
                                }
                            } else {
                                error.set(Some("Failed to fetch profile".to_string()));
                            }
                        }
                        Err(_) => {
                            error.set(Some("Failed to fetch profile".to_string()));
                        }
                    }
                });
                || ()
            },
            (),
        );
    }

    let profile_data = (*profile).clone();

    html! {
        <>
        <style>

            {r#"
                    /* Profile Container Styles */
                    .profile-container {
                        max-width: 1200px;
                        margin: 0 auto;
                        padding: 2rem;
                        animation: fadeIn 0.5s ease-out;
                    }

                    @keyframes fadeIn {
                        from {
                            opacity: 0;
                            transform: translateY(10px);
                        }
                        to {
                            opacity: 1;
                            transform: translateY(0);
                        }
                    }

                    .profile-panel {
                        background: rgba(30, 30, 30, 0.7);
                        border: 1px solid rgba(30, 144, 255, 0.1);
                        border-radius: 24px;
                        padding: 2rem;
                        margin-top: 5rem;
                        backdrop-filter: blur(10px);
                        box-shadow: 0 8px 32px rgba(0, 0, 0, 0.2);
                    }

                    .profile-header {
                        display: flex;
                        justify-content: space-between;
                        align-items: center;
                        margin-bottom: 3rem;
                        padding-bottom: 1.5rem;
                        border-bottom: 1px solid rgba(30, 144, 255, 0.1);
                    }

                    .header-content {
                        display: flex;
                        flex-direction: column;
                        gap: 0.5rem;
                    }

                    .profile-title {
                        font-size: 2.5rem;
                        background: linear-gradient(45deg, #fff, #7EB2FF);
                        -webkit-background-clip: text;
                        -webkit-text-fill-color: transparent;
                        margin: 0;
                        font-weight: 700;
                    }

                    .profile-subtitle {
                        color: #999;
                        font-size: 1.1rem;
                        margin: 0;
                    }

                    .back-link {
                        display: flex;
                        align-items: center;
                        gap: 0.5rem;
                        color: #7EB2FF;
                        text-decoration: none;
                        font-size: 1rem;
                        transition: all 0.3s ease;
                        padding: 0.5rem 1rem;
                        border-radius: 8px;
                        border: 1px solid rgba(30, 144, 255, 0.2);
                        background: rgba(30, 144, 255, 0.05);
                    }

                    .back-link:hover {
                        transform: translateX(-5px);
                        background: rgba(30, 144, 255, 0.1);
                        border-color: rgba(30, 144, 255, 0.3);
                    }

                    .back-icon {
                        font-size: 1.2rem;
                    }

                    @media (max-width: 768px) {
                        .profile-container {
                            padding: 1rem;
                        }

                        .profile-panel {
                            padding: 1.5rem;
                            border-radius: 16px;
                        }

                        .profile-header {
                            flex-direction: column;
                            align-items: flex-start;
                            gap: 1rem;
                            margin-bottom: 2rem;
                        }

                        .profile-title {
                            font-size: 2rem;
                        }

                        .back-link {
                            align-self: flex-start;
                        }
                    }
                .success-message {
                    border: 1px solid rgba(76, 175, 80, 0.3);
                    background: none !important;
                    border-radius: 8px;
                    padding: 1rem;
                    margin-bottom: 1.5rem;
                    animation: fadeIn 0.5s ease-in-out;
                }
                
                .success-content {
                    display: flex;
                    background: none !important;
                    align-items: center;
                    gap: 1rem;
                }
                
                .success-icon {
                    background-color: rgba(76, 175, 80, 0.2);
                    border-radius: 50%;
                    width: 24px;
                    height: 24px;
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    color: #4CAF50;
                }
                
                .success-text {
                    color: #4CAF50;
                    flex: 1;
                }
                
                @keyframes fadeIn {
                    from { opacity: 0; transform: translateY(-10px); }
                    to { opacity: 1; transform: translateY(0); }
                "#}
            </style>
            <div class="profile-container">
                <div class="profile-panel">
                    <div class="profile-header">
                        <div class="header-content">
                            <h1 class="profile-title">{"Billing"}</h1>
                            <p class="profile-subtitle">{"Manage your subscription and credits"}</p>
                        </div>
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
                                <div class="message success-message">
                                    <div class="success-content">
                                        <span class="success-icon">{"✓"}</span>
                                        <div class="success-text">
                                            {success_msg}
                                        </div>
                                    </div>
                                </div>
                            }
                        } else {
                            html! {}
                        }
                    }

                    {
                        if let Some(user_profile) = profile_data {
                            html! {
                                <BillingPage user_profile={user_profile.clone()} />
                            }
                        } else {
                            html! {
                                <div class="loading-profile">{"Loading billing..."}</div>
                            }
                        }
                    }
                </div>
            </div>

        </>
    }
}
