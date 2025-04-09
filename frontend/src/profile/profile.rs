use yew::prelude::*;
use web_sys::window;
use yew_router::prelude::*;
use crate::Route;
use crate::config;
use gloo_net::http::Request;
use wasm_bindgen_futures::spawn_local;
use crate::profile::billing_models::UserProfile;
use crate::profile::settings::SettingsPage;
use crate::profile::billing_credits::BillingPage;
use web_sys::UrlSearchParams;

#[derive(Clone, PartialEq)]
enum ProfileTab {
    Settings,
    Billing,
}

#[function_component]
pub fn Profile() -> Html {
    let profile = use_state(|| None::<UserProfile>);
    let error = use_state(|| None::<String>);
    let success = use_state(|| None::<String>);
    let active_tab = use_state(|| ProfileTab::Settings);
    let navigator = use_navigator().unwrap();
    let location = use_location().unwrap();

    // Check for subscription success parameter
    {
        let success = success.clone();
        let active_tab = active_tab.clone();
        use_effect_with_deps(move |_| {
            let query = location.query_str();
            if let Ok(params) = UrlSearchParams::new_with_str(query) {
                if params.has("subscription") && params.get("subscription").unwrap_or_default() == "success" {
                    success.set(Some("Subscription activated successfully! You can now configure your proactive filters in the dashboard 'Proavtive' tab.".to_string()));
                    active_tab.set(ProfileTab::Settings);
                    
                    // Clean up the URL after showing the message
                    if let Some(window) = window() {
                        if let Ok(history) = window.history() {
                            let _ = history.replace_state_with_url(
                                &wasm_bindgen::JsValue::NULL,
                                "",
                                Some("/profile")
                            );
                        }
                    }
                }
            }
            || ()
        }, ());
    }

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

    // Fetch user profile 
    {
        let profile = profile.clone();
        let error = error.clone();
        use_effect_with_deps(move |_| {
            spawn_local(async move {
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
        <>
            <style>
                {".success-message {
                    background-color: rgba(76, 175, 80, 0.1);
                    border: 1px solid rgba(76, 175, 80, 0.3);
                    border-radius: 8px;
                    padding: 1rem;
                    margin-bottom: 1.5rem;
                    animation: fadeIn 0.5s ease-in-out;
                }
                
                .success-content {
                    display: flex;
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
                }"}
            </style>
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
                            match *active_tab {
                                ProfileTab::Settings => html! {
                                    <SettingsPage 
                                        user_profile={user_profile.clone()}
                                        on_profile_update={{
                                            let profile = profile.clone();
                                            Callback::from(move |updated_profile: UserProfile| {
                                                profile.set(Some(updated_profile));
                                            })
                                        }}
                                    />
                                },
                                ProfileTab::Billing => html! {
                                    <BillingPage user_profile={user_profile.clone()} />
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

        </>
    }
}

