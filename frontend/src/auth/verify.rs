use yew::prelude::*;
use yew_router::prelude::*;
use crate::Route;
use crate::config;
use crate::profile::billing_models::UserProfile;
use web_sys::{window, HtmlInputElement};
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};

#[derive(Serialize)]
struct UpdatePhoneRequest {
    email: String,
    phone_number: String,
    nickname: String,
    info: String,
    timezone: String,
    timezone_auto: bool,
    agent_language: String,
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
    let is_editing = use_state(|| false);
    let phone_number = use_state(String::new);
    let error = use_state(|| None::<String>);
    let success = use_state(|| None::<String>);
    let user_profile = use_state(|| None::<UserProfile>);

    // Polling effect for verification status
    {
        let navigator = navigator.clone();
        let phone_number = phone_number.clone();
        let user_profile = user_profile.clone();
        let is_editing = is_editing.clone();
        
        use_effect_with_deps(move |_| {
            let is_editing = is_editing.clone();
            let interval_handle = std::rc::Rc::new(std::cell::RefCell::new(None));
            let interval_handle_clone = interval_handle.clone();
            let phone_number = phone_number.clone();
            let user_profile = user_profile.clone();

            // Function to check verification status
            let check_verification = move || {
                let navigator = navigator.clone();
                let interval_handle = interval_handle.clone();
                let phone_number = phone_number.clone();
                let user_profile = user_profile.clone();
                let is_editing = is_editing.clone();

                wasm_bindgen_futures::spawn_local(async move {
                    let user_profile = user_profile.clone();
                    let phone_number = phone_number.clone();
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
                                    user_profile.set(Some(profile.clone()));
                                    if profile.verified {
                                        // Stop polling and redirect to home
                                        if let Some(interval) = interval_handle.borrow_mut().take() {
                                            drop(interval);
                                        }
                                        navigator.push(&Route::Home);
                                    } else {
                                        // Only set the phone number once when it's initially empty
                                        if phone_number.is_empty() {
                                            phone_number.set(profile.phone_number);
                                        }
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
        <>
        <div class="verification-container">
            <div class="verification-panel">
                <h1>{"Verify Your Account"}</h1>
                <p>{"Click to call one of the following numbers or type them manually to verify your account"}</p>
                <div class="phone-numbers-list">
                    { PHONE_NUMBERS.iter().map(|(country, number)| {
                        html! {
                            <a class="phone-number-call-link" href={format!("tel:{}", number)}>
                                <div class="phone-number-option">
                                    <span class="country-code">{country.to_uppercase()}</span>
                                    {"Call "}<span class="number-value">{number}</span>
                                </div>
                            </a>
                        }
                    }).collect::<Html>() }
                </div>
                <div class="phone-edit-section">
                    <div class="current-phone">
                        <span>{"Your phone number: "}</span>
                        <span class="phone-value">{(*phone_number).clone()}</span>
                    </div>
                    {
                        if *is_editing {
                            html! {
                                <>
                                    <div class="phone-edit-form">
                                        <input
                                            type="tel"
                                            class="phone-input"
                                            placeholder="+1234567890"
                                            onchange={let phone_number = phone_number.clone(); move |e: Event| {
                                                let input: HtmlInputElement = e.target_unchecked_into();
                                                phone_number.set(input.value());
                                            }}
                                        />
                                        <div class="button-group">
                                            <button 
                                                class="save-button"
                                                onclick={{
                                                let phone_number = phone_number.clone();
                                                let error = error.clone();
                                                let success = success.clone();
                                                let is_editing = is_editing.clone();
                                                
                                                let user_profile = user_profile.clone();
                                                Callback::from(move |_| {
                                                    let phone_number = phone_number.clone();
                                                    let error = error.clone();
                                                    let success = success.clone();
                                                    let is_editing = is_editing.clone();
                                                    let user_profile = user_profile.clone();
                                                    
                                                    if let Some(token) = window()
                                                        .and_then(|w| w.local_storage().ok())
                                                        .flatten()
                                                        .and_then(|storage| storage.get_item("token").ok())
                                                        .flatten()
                                                    {
                                                        wasm_bindgen_futures::spawn_local(async move {
                                                            if let Some(profile) = (*user_profile).clone() {
                                                                match Request::post(&format!("{}/api/profile/update", config::get_backend_url()))
                                                                    .header("Authorization", &format!("Bearer {}", token))
                                                                    .json(&UpdatePhoneRequest { 
                                                                        email: profile.email.clone(),
                                                                        phone_number: (*phone_number).clone(),
                                                                        nickname: profile.nickname.clone().unwrap_or_default(),
                                                                        info: profile.info.clone().unwrap_or_default(),
                                                                        timezone: profile.timezone.clone().unwrap_or_else(|| String::from("UTC")),
                                                                        timezone_auto: profile.timezone_auto.unwrap_or(true),
                                                                        agent_language: profile.agent_language.clone(),
                                                                    })
                                                                    .expect("Failed to build request")
                                                                    .send()
                                                                    .await 
                                                                {
                                                                    Ok(response) => {
                                                                        if response.ok() {
                                                                            success.set(Some("Phone number updated successfully".to_string()));
                                                                            error.set(None);
                                                                            is_editing.set(false);
                                                                        } else {
                                                                            error.set(Some("Failed to update phone number".to_string()));
                                                                        }
                                                                    }
                                                                    Err(_) => {
                                                                        error.set(Some("Failed to send request".to_string()));
                                                                    }
                                                                }
                                                            }
                                                        });
                                                    }
                                                    })
                                                }}
                                            >
                                            {"Save"}
                                        </button>
                                        <button 
                                            class="cancel-button"
                                            onclick={let is_editing = is_editing.clone(); move |_| is_editing.set(false)}
                                        >
                                            {"Cancel"}
                                        </button>
                                    </div>
                                </div>
                                </>
                            }
                        } else {
                            html! {
                                <button 
                                    class="edit-button"
                                    onclick={let is_editing = is_editing.clone(); move |_| is_editing.set(true)}
                                >
                                    {"Change Phone Number"}
                                </button>
                            }
                        }
                    }
                    
                {
                    if let Some(error_msg) = (*error).as_ref() {
                        html! {
                            <div class="error-message">{error_msg}</div>
                        }
                    } else if let Some(success_msg) = (*success).as_ref() {
                        html! {
                            <div class="success-message">{success_msg}</div>
                        }
                    } else {
                        html! {}
                    }
                }
                </div>

                <div class="verification-status">
                    <i class="verification-icon"></i>
                    <span>{"Waiting for verification..."}</span>
                </div>
                <p class="instruction-text">
                    {"Want a local phone number to call? Please send me an email(rasmus@ahtava.com) or telegram(@ahtavarasmus)"}
                </p>
                <p class="verification-help">
                    <span>{"Having trouble? "}</span>
                    <ul>
                    <li>{"Correct country code? (starting with country code +...)."}</li>
                    <li>{"Calling from correct number? (your phone could have multiple sims)"}</li>
                    </ul>
                </p>
            </div>
        </div>
        <style>
            {r#"
.phone-numbers-list {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 1rem;
    margin: 2rem 0;
}

.phone-number-option {
    background: rgba(30, 30, 30, 0.7);
    border: 1px solid rgba(30, 144, 255, 0.2);
    border-radius: 12px;
    padding: 1rem;
    display: flex;
    color: white;
    justify-content: space-between;
    align-items: center;
    transition: all 0.3s ease;
}

.phone-number-option:hover {
    transform: translateY(-2px);
    border-color: rgba(30, 144, 255, 0.4);
    box-shadow: 0 4px 20px rgba(30, 144, 255, 0.15);
}

.phone-number-call-link {
    text-decoration: none;
    color: inherit;
}

.country-code {
    font-size: 0.8rem;
    color: #7EB2FF;
    font-weight: 600;
    background: rgba(30, 144, 255, 0.1);
    padding: 0.3rem 0.6rem;
    border-radius: 4px;
}

.number-value {
    color: #e0e0e0;
    font-family: monospace;
    font-size: 1rem;
    letter-spacing: 0.5px;
}

.phone-edit-section {
    margin: 2rem 0;
    padding: 2rem;
    background: rgba(30, 30, 30, 0.7);
    border: 1px solid rgba(30, 144, 255, 0.2);
    border-radius: 16px;
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.2);
}

.current-phone {
    margin-bottom: 1.5rem;
    padding: 1.2rem;
    border-radius: 12px;
    background: rgba(0, 0, 0, 0.2);
    display: flex;
    align-items: center;
    gap: 1rem;
}

.current-phone span:first-child {
    color: #999;
}

.phone-value {
    font-weight: 600;
    color: #7EB2FF;
    font-family: monospace;
    letter-spacing: 1px;
    font-size: 1.1rem;
}

.phone-edit-form {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    background: rgba(0, 0, 0, 0.2);
    padding: 1.5rem;
    border-radius: 12px;
    border: 1px solid rgba(30, 144, 255, 0.1);
}

.phone-input {
    padding: 1rem;
    border-radius: 8px;
    border: 2px solid rgba(30, 144, 255, 0.3);
    background: rgba(0, 0, 0, 0.3);
    color: white;
    font-size: 1rem;
    font-family: monospace;
    letter-spacing: 1px;
    transition: all 0.3s ease;
}

.phone-input:focus {
    outline: none;
    border-color: rgba(30, 144, 255, 0.6);
    box-shadow: 0 0 0 2px rgba(30, 144, 255, 0.1);
}

.phone-input::placeholder {
    color: rgba(255, 255, 255, 0.3);
}

.button-group {
    display: flex;
    gap: 1rem;
    margin-top: 1rem;
}

.save-button, .cancel-button, .edit-button {
    padding: 0.8rem 1.5rem;
    border-radius: 8px;
    border: none;
    cursor: pointer;
    font-size: 0.9rem;
    transition: all 0.3s ease;
    font-weight: 500;
    flex: 1;
}

.save-button {
    background: linear-gradient(45deg, #1E90FF, #4169E1);
    color: white;
    box-shadow: 0 2px 10px rgba(30, 144, 255, 0.2);
}

.save-button:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 20px rgba(30, 144, 255, 0.3);
}

.cancel-button {
    background: rgba(255, 255, 255, 0.05);
    color: #999;
    border: 1px solid rgba(255, 255, 255, 0.1);
}

.cancel-button:hover {
    background: rgba(255, 255, 255, 0.1);
    color: white;
}

.edit-button {
    background: rgba(30, 144, 255, 0.1);
    color: #7EB2FF;
    border: 1px solid rgba(30, 144, 255, 0.3);
    width: 100%;
    position: relative;
    overflow: hidden;
}

.edit-button::before {
    content: '';
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    background: linear-gradient(
        45deg,
        transparent,
        rgba(30, 144, 255, 0.1),
        transparent
    );
    transform: translateX(-100%);
    transition: transform 0.6s;
}

.edit-button:hover::before {
    transform: translateX(100%);
}

.edit-button:hover {
    border-color: rgba(30, 144, 255, 0.5);
    color: #1E90FF;
}

@media (max-width: 768px) {
    .phone-numbers-list {
        grid-template-columns: 1fr;
    }

    .button-group {
        flex-direction: column;
    }

    .save-button, .cancel-button, .edit-button {
        width: 100%;
    }
}

                .save-button:hover, .cancel-button:hover, .edit-button:hover {
                    transform: translateY(-2px);
                }

                .error-message {
                    color: #ff4444;
                    font-size: 14px;
                    margin-top: 10px;
                    padding: 8px;
                    background: rgba(255, 68, 68, 0.1);
                    border-radius: 4px;
                }

                .success-message {
                    color: #00ff00;
                    font-size: 14px;
                    margin-top: 10px;
                    padding: 8px;
                    background: rgba(0, 255, 0, 0.1);
                    border-radius: 4px;
                }
            "#}
        </style>
            </>
    }
}
