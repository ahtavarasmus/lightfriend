use yew::prelude::*;
use yew_router::prelude::*;
use crate::Route;
use crate::config;
use web_sys::window;
use gloo_net::http::Request;
use serde::Deserialize;

#[derive(Deserialize, Clone)]
struct UserProfile {
    verified: bool,
}

const PHONE_NUMBERS: &[(&str, &str)] = &[
    ("usa", "+18153684737"),
    ("fin", "+358454901522"),
    ("aus", "+61489260976"),
    ("gbr", "+447383240344"),
];

#[function_component]
pub fn Verify() -> Html {
    let navigator = use_navigator().unwrap();

    // Polling effect for verification status
    {
        let navigator = navigator.clone();
        
        use_effect_with_deps(move |_| {
            let interval_handle = std::rc::Rc::new(std::cell::RefCell::new(None));
            let interval_handle_clone = interval_handle.clone();

            // Function to check verification status
            let check_verification = move || {
                let navigator = navigator.clone();
                let interval_handle = interval_handle.clone();

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
                                
                                if let Ok(profile) = response.json::<UserProfile>().await {
                                    if profile.verified {
                                        // Stop polling and redirect to home
                                        if let Some(interval) = interval_handle.borrow_mut().take() {
                                            drop(interval);
                                        }
                                        navigator.push(&Route::Home);
                                    }
                                }
                            }
                            Err(_) => {
                                gloo_console::error!("Failed to fetch profile");
                            }
                        }
                    }
                });
            };

            // Initial check
            check_verification();
            
            // Set up polling interval
            let interval = gloo_timers::callback::Interval::new(5000, move || {
                check_verification();
            });

            *interval_handle_clone.borrow_mut() = Some(interval);

            move || {
                if let Some(interval) = interval_handle_clone.borrow_mut().take() {
                    drop(interval);
                }
            }
        }, ());
    }

    html! {
        <div class="verification-container">
            <div class="verification-panel">
                <h1>{"Verify Your Account"}</h1>
                <p>{"Call one of the following numbers to verify your account"}</p>
                <div class="phone-numbers-list">
                    { PHONE_NUMBERS.iter().map(|(country, number)| {
                        html! {
                            <div class="phone-number">{format!("{} {}", country, number)}</div>
                        }
                    }).collect::<Html>() }
                </div>
                <div class="verification-status">
                    <i class="verification-icon"></i>
                    <span>{"Waiting for verification..."}</span>
                </div>
                <p class="instruction-text">
                    {"Want a local phone number to call? Please send me an email(rasmus@ahtava.com) or telegram(@ahtavarasmus)"}
                </p>
                <p class="verification-help">
                    <span>{"Having trouble? Make sure you typed your number correctly. You can change it in the profile."}</span>
                    <Link<Route> to={Route::Profile}>
                        {"profile"}
                    </Link<Route>>
                </p>
            </div>
        </div>
    }
}
