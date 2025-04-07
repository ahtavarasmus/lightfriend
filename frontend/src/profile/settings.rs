use yew::prelude::*;
use web_sys::{HtmlInputElement, window};
use yew_router::prelude::*;
use crate::Route;
use crate::config;
use crate::profile::timezone_detector::TimezoneDetector;
use gloo_net::http::Request;
use serde::Serialize;
use wasm_bindgen_futures::spawn_local;
use crate::profile::billing_models::UserProfile;

const MAX_NICKNAME_LENGTH: usize = 30;
const MAX_INFO_LENGTH: usize = 500;

#[derive(Serialize)]
struct UpdateProfileRequest {
    email: String,
    phone_number: String,
    nickname: String,
    info: String,
    timezone: String,
    timezone_auto: bool,
}

#[derive(Properties, PartialEq, Clone)]
pub struct SettingsPageProps {
    pub user_profile: UserProfile,
    pub on_profile_update: Callback<UserProfile>,
}


#[function_component]
pub fn SettingsPage(props: &SettingsPageProps) -> Html {
    let user_profile = use_state(|| props.user_profile.clone());
    let email = use_state(|| (*user_profile).email.clone());
    let phone_number = use_state(|| (*user_profile).phone_number.clone());
    let nickname = use_state(|| (*user_profile).nickname.clone().unwrap_or_default());
    let info = use_state(|| (*user_profile).info.clone().unwrap_or_default());
    let timezone = use_state(|| (*user_profile).timezone.clone().unwrap_or_else(|| String::from("UTC")));
    let timezone_auto = use_state(|| (*user_profile).timezone_auto.unwrap_or(true));
    let error = use_state(|| None::<String>);
    let success = use_state(|| None::<String>);
    let is_editing = use_state(|| false);
    let navigator = use_navigator().unwrap();

    // Update local state when props change
    {
        let email = email.clone();
        let phone_number = phone_number.clone();
        let nickname = nickname.clone();
        let info = info.clone();
        let timezone = timezone.clone();
        let user_profile_state = user_profile.clone();
        
        use_effect_with_deps(move |props_profile| {
            email.set(props_profile.email.clone());
            phone_number.set(props_profile.phone_number.clone());
            nickname.set(props_profile.nickname.clone().unwrap_or_default());
            info.set(props_profile.info.clone().unwrap_or_default());
            timezone.set(props_profile.timezone.clone().unwrap_or_else(|| String::from("UTC")));
            user_profile_state.set(props_profile.clone());
            || ()
        }, props.user_profile.clone());
    }
    
    let on_edit = {
        let email = email.clone();
        let phone_number = phone_number.clone();
        let nickname = nickname.clone();
        let info = info.clone();
        let error = error.clone();
        let success = success.clone();
        let is_editing = is_editing.clone();
        let navigator = navigator.clone();
        let timezone = timezone.clone();
        let timezone_auto = timezone_auto.clone();
        let user_profile = user_profile.clone();
        let props = props.clone();

        Callback::from(move |_e: MouseEvent| {
            let email = email.clone();
            let phone_number = phone_number.clone();
            let nickname = nickname.clone();
            let info = info.clone();
            let timezone = timezone.clone();
            let timezone_auto = timezone_auto.clone();  // Clone the UseState handle instead of dereferencing
            let error = error.clone();
            let success = success.clone();
            let is_editing = is_editing.clone();
            let navigator = navigator.clone();
            let user_profile = user_profile.clone();

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
            let props = props.clone();

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
                            email: (*email).clone(),
                            phone_number: (*phone_number).clone(),
                            nickname: (*nickname).clone(),
                            info: (*info).clone(),
                            timezone: (*timezone).clone(),
                            timezone_auto: *timezone_auto.clone(),
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
                                // Create updated profile
                                let updated_profile = UserProfile {
                                    id: (*user_profile).id,
                                    email: (*email).clone(),
                                    phone_number: (*phone_number).clone(),
                                    nickname: Some((*nickname).clone()),
                                    info: Some((*info).clone()),
                                    timezone: Some((*timezone).clone()),
                                    timezone_auto: Some(*timezone_auto),
                                    verified: (*user_profile).verified,
                                    time_to_live: (*user_profile).time_to_live,
                                    time_to_delete: (*user_profile).time_to_delete,
                                    credits: (*user_profile).credits,
                                    charge_when_under: (*user_profile).charge_when_under,
                                    charge_back_to: (*user_profile).charge_back_to,
                                    stripe_payment_method_id: (*user_profile).stripe_payment_method_id.clone(),
                                    sub_tier: (*user_profile).sub_tier.clone(),
                                    msgs_left: (*user_profile).msgs_left,
                                };

                                // Notify parent component
                                props.on_profile_update.emit(updated_profile.clone());


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


let on_timezone_update = {
    let timezone = timezone.clone();
    let user_profile = user_profile.clone();
    let props = props.clone();
    let timezone_auto = timezone_auto.clone();
    
    Callback::from(move |new_timezone: String| {
        // Only update if automatic timezone is enabled
        if *timezone_auto {
            timezone.set(new_timezone.clone());
            
            // Update the user_profile state with the new timezone
            let mut updated_profile = (*user_profile).clone();
            updated_profile.timezone = Some(new_timezone.clone());
            updated_profile.timezone_auto = Some(*timezone_auto);
            user_profile.set(updated_profile.clone());
            
            // Notify parent component
            props.on_profile_update.emit(updated_profile);
        }
    })
};

    html! {
        <div class="profile-info">
            <TimezoneDetector on_timezone_update={on_timezone_update} />
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
                value={(*email).to_string()}
                                placeholder="your@email.com"
                onchange={let email = email.clone(); move |e: Event| {
                    let input: HtmlInputElement = e.target_unchecked_into();
                    email.set(input.value());
                }}
                            />
                        }
                    } else {
                        html! {
                            <span class="field-value">{&(*user_profile).email}</span>
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
                                {&(*user_profile).phone_number}
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
                                {(*user_profile).nickname.clone().unwrap_or_default()}
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
                                {(*user_profile).info.clone().unwrap_or("I'm from finland, always use Celsious and metric system, etc...".to_string())}
                            </span>
                        }
                    }
                }
            </div>

            <div class="profile-field">
                <div class="field-label-group">
                    <span class="field-label">{"Timezone"}</span>
                    <div class="tooltip">
                        <span class="tooltip-icon">{"?"}</span>
                        <span class="tooltip-text">
                            {"Choose your timezone. This helps the AI assistant provide time-sensitive responses and schedule events in your local time."}
                        </span>
                    </div>
                </div>
                <div class="timezone-section">
                    {
                        if *is_editing {
                            html! {
                                <>
                                <div class="timezone-auto-checkbox">
                                    <label class="custom-checkbox">
                                        <input
                                            type="checkbox"
                                            id="timezone-auto"
                                            checked={*timezone_auto}
                                            disabled={!*is_editing}
                                            onchange={let timezone_auto = timezone_auto.clone(); move |e: Event| {
                                                let input: HtmlInputElement = e.target_unchecked_into();
                                                timezone_auto.set(input.checked());
                                            }}
                                        />
                                        <span class="checkmark"></span>
                                        {"Automatically detect timezone"}
                                    </label>
                                </div>


                                <select
                                    class="profile-input"
                                    value={(*timezone).clone()}
                                    disabled={*timezone_auto}
                                    onchange={let timezone = timezone.clone(); move |e: Event| {
                                        let select: HtmlInputElement = e.target_unchecked_into();
                                        timezone.set(select.value());
                                    }}
                                >
                                    {
                                    chrono_tz::TZ_VARIANTS.iter().map(|tz| {
                                            html! {
                                                <option value={tz.name()} selected={tz.name() == (*timezone)}>
                                                    {tz.name()}
                                                </option>
                                            }
                                        }).collect::<Html>()
                                    }
                                </select>
                                </>
                            }
                        } else {
                            html! {
                                <div class="timezone-display">
                                    <span class="field-value">
                                        {(*user_profile).timezone.clone().unwrap_or_else(|| String::from("UTC"))}
                                    </span>
                                    {
                                        if *timezone_auto {
                                            html! {
                                                <span class="auto-tag">{"(Auto)"}</span>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }
                                </div>
                            }
                        }
                    }
                </div>
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
