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
                                <SettingsPage user_profile={user_profile.clone()} />
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
    }
}

