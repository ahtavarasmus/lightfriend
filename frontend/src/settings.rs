use yew::prelude::*;
use web_sys::{HtmlInputElement, window};
use yew_router::prelude::*;
use crate::Route;
use crate::config;
use gloo_net::http::Request;
use serde::Serialize;
use wasm_bindgen_futures::spawn_local;
use gloo_timers::future::TimeoutFuture;
use crate::billing::UserProfile;

const MAX_NICKNAME_LENGTH: usize = 30;
const MAX_INFO_LENGTH: usize = 500;

#[derive(Serialize)]
struct UpdateProfileRequest {
    email: String,
    phone_number: String,
    nickname: String,
    info: String,
}

#[derive(Properties, PartialEq, Clone)]
pub struct SettingsPageProps {
    pub user_profile: UserProfile,
}



#[function_component]
pub fn SettingsPage(props: &SettingsPageProps) -> Html {
    let user_profile = &props.user_profile;
    let email = use_state(|| user_profile.email.clone());
    let phone_number = use_state(|| user_profile.phone_number.clone());
    let nickname = use_state(|| user_profile.nickname.clone().unwrap_or_default());
    let info = use_state(|| user_profile.info.clone().unwrap_or_default());
    let error = use_state(|| None::<String>);
    let success = use_state(|| None::<String>);
    let is_editing = use_state(|| false);
    let navigator = use_navigator().unwrap();
    
    let on_edit = {
        let email = email.clone();
        let phone_number = phone_number.clone();
        let nickname = nickname.clone();
        let info = info.clone();
        let error = error.clone();
        let success = success.clone();
        let is_editing = is_editing.clone();
        let navigator = navigator.clone();

        Callback::from(move |_e: MouseEvent| {
            let email_str = (*email).clone();
            let phone = (*phone_number).clone();
            let nick = (*nickname).clone();
            let user_info = (*info).clone();
            let error = error.clone();
            let success = success.clone();
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

            spawn_local(async move {
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
                                spawn_local(async move {
                                    gloo_timers::future::TimeoutFuture::new(3_000).await;
                                    success_clone.set(None);
                                });
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


    html! {
        <div class="profile-info">
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
                                {&user_profile.phone_number}
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
    }
}
