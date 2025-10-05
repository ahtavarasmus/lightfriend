use yew::prelude::*;
use log::{info, Level};
use web_sys::{HtmlInputElement, window};
use yew_router::prelude::*;
use crate::Route;
use crate::config;
use crate::profile::timezone_detector::TimezoneDetector;
use gloo_net::http::Request;
use serde::Serialize;
use wasm_bindgen_futures::spawn_local;
use crate::profile::billing_models::UserProfile;
use web_sys::js_sys::encode_uri_component;
const MAX_NICKNAME_LENGTH: usize = 30;
const MAX_INFO_LENGTH: usize = 500;
#[derive(Serialize)]
struct UpdateProfileRequest {
    phone_number: String,
    nickname: String,
    info: String,
    timezone: String,
    timezone_auto: bool,
    agent_language: String,
    notification_type: Option<String>,
    save_context: Option<i32>,
    location: String,
    nearby_places: String,
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
    let agent_language = use_state(|| (*user_profile).agent_language.clone());
    let notification_type = {
        let initial_value = (*user_profile).notification_type.clone();
        info!("Initial notification type from props: {:?}", initial_value);
        use_state(|| initial_value.or(Some("sms".to_string())))
    };
    let save_context = use_state(|| (*user_profile).save_context.unwrap_or(0));
    let location = use_state(|| (*user_profile).location.clone().unwrap_or_default());
    let nearby_places = use_state(|| (*user_profile).nearby_places.clone().unwrap_or_default());
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
        let agent_language = agent_language.clone();
        let user_profile_state = user_profile.clone();
        let notification_type = notification_type.clone();
        let save_context = save_context.clone();
        let location = location.clone();
        let nearby_places = nearby_places.clone();
        use_effect_with_deps(move |props_profile| {
            info!("{}", &format!("Props notification type: {:?}", props_profile.notification_type));
            email.set(props_profile.email.clone());
            phone_number.set(props_profile.phone_number.clone());
            nickname.set(props_profile.nickname.clone().unwrap_or_default());
            info.set(props_profile.info.clone().unwrap_or_default());
            timezone.set(props_profile.timezone.clone().unwrap_or_else(|| String::from("UTC")));
            agent_language.set(props_profile.agent_language.clone());
            notification_type.set(props_profile.notification_type.clone());
            location.set(props_profile.location.clone().unwrap_or_default());
            nearby_places.set(props_profile.nearby_places.clone().unwrap_or_default());
            user_profile_state.set(props_profile.clone());
            || ()
        }, props.user_profile.clone());
    }
    let is_editing_clone = is_editing.clone();
    let nearby_places_clone = nearby_places.clone();
    let error_clone = error.clone();
    use_effect_with_deps(
        move |location_handle| {
            let loc = (*location_handle).clone();
            let is_editing = *is_editing_clone;
            let nearby_places = nearby_places_clone.clone();
            let error = error_clone.clone();
            if !is_editing || loc.is_empty() {
                Box::new(|| ()) as Box<dyn FnOnce()>
            } else {
                let timer = gloo_timers::callback::Timeout::new(1000, move || {
                    spawn_local(async move {
                        if let Some(token) = window()
                            .and_then(|w| w.local_storage().ok())
                            .flatten()
                            .and_then(|storage| storage.get_item("token").ok())
                            .flatten()
                        {
                            let encoded_loc = encode_uri_component(&loc).to_string();
                            let url = format!("{}/api/get_nearby_places?location={}", config::get_backend_url(), encoded_loc);
                            match Request::get(&url)
                                .header("Authorization", &format!("Bearer {}", token))
                                .send()
                                .await
                            {
                                Ok(response) if response.ok() => {
                                    if let Ok(json) = response.json::<Vec<String>>().await {
                                        nearby_places.set(json.join(", "));
                                    } else {
                                        error.set(Some("Failed to parse nearby places".to_string()));
                                    }
                                }
                                _ => {
                                    error.set(Some("Failed to fetch nearby places".to_string()));
                                }
                            }
                        } else {
                            error.set(Some("No authentication token found".to_string()));
                        }
                    });
                });
                Box::new(move || { timer.cancel(); }) as Box<dyn FnOnce()>
            }
        },
        location.clone(),
    );
 
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
        let agent_language = agent_language.clone();
        let notification_type = notification_type.clone();
        let user_profile = user_profile.clone();
        let save_context = save_context.clone();
        let props = props.clone();
        let location = location.clone();
        let nearby_places = nearby_places.clone();
        Callback::from(move |_e: MouseEvent| {
            let email = email.clone();
            let phone_number = phone_number.clone();
            let nickname = nickname.clone();
            let info = info.clone();
            let timezone = timezone.clone();
            let timezone_auto = timezone_auto.clone(); // Clone the UseState handle instead of dereferencing
            let agent_language = agent_language.clone();
            let error = error.clone();
            let success = success.clone();
            let is_editing = is_editing.clone();
            let navigator = navigator.clone();
            let user_profile = user_profile.clone();
            let notification_type = notification_type.clone();
            let save_context = save_context.clone();
            let location = location.clone();
            let nearby_places = nearby_places.clone();
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
                            phone_number: (*phone_number).clone(),
                            nickname: (*nickname).clone(),
                            info: (*info).clone(),
                            timezone: (*timezone).clone(),
                            timezone_auto: *timezone_auto.clone(),
                            agent_language: (*agent_language).clone(),
                            notification_type: (*notification_type).clone(),
                            save_context: Some(*save_context),
                            location: (*location).clone(),
                            nearby_places: (*nearby_places).clone(),
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
                                    preferred_number: (*user_profile).preferred_number.clone(),
                                    timezone: Some((*timezone).clone()),
                                    timezone_auto: Some(*timezone_auto),
                                    agent_language: (*agent_language).clone(),
                                    notification_type: (*notification_type).clone(),
                                    verified: (*user_profile).verified,
                                    time_to_live: (*user_profile).time_to_live,
                                    time_to_delete: (*user_profile).time_to_delete,
                                    credits: (*user_profile).credits,
                                    charge_when_under: (*user_profile).charge_when_under,
                                    charge_back_to: (*user_profile).charge_back_to,
                                    stripe_payment_method_id: (*user_profile).stripe_payment_method_id.clone(),
                                    sub_tier: (*user_profile).sub_tier.clone(),
                                    credits_left: (*user_profile).credits_left,
                                    discount: (*user_profile).discount,
                                    notify: (*user_profile).notify,
                                    sub_country: (*user_profile).sub_country.clone(),
                                    save_context: Some(*save_context),
                                    days_until_billing: (*user_profile).days_until_billing.clone(),
                                    digests_reserved: (*user_profile).digests_reserved,
                                    pairing_code: (*user_profile).pairing_code.clone(),
                                    server_ip: (*user_profile).server_ip.clone(),
                                    twilio_sid: (*user_profile).twilio_sid.clone(),
                                    twilio_token: (*user_profile).twilio_token.clone(),
                                    openrouter_api_key: (*user_profile).openrouter_api_key.clone(),
                                    textbee_device_id: (*user_profile).textbee_device_id.clone(),
                                    textbee_api_key: (*user_profile).textbee_api_key.clone(),
                                    estimated_monitoring_cost: (*user_profile).estimated_monitoring_cost,
                                    location: Some((*location).clone()),
                                    nearby_places: Some((*nearby_places).clone()),
                                    phone_number_country: (*user_profile).phone_number_country.clone(),
                                };
                                // Notify parent component
                                props.on_profile_update.emit(updated_profile.clone());
                                // Check if phone number was changed
                                let phone_changed = (*user_profile).phone_number != (*phone_number).clone();
                             
                                if phone_changed {
                                    // If phone number changed, redirect to home for verification
                                    navigator.push(&Route::Home);
                                } else {
                                    success.set(Some("Profile updated successfully".to_string()));
                                    error.set(None);
                                    is_editing.set(false);
                                 
                                    // Clear success message after 3 seconds
                                    let success_clone = success.clone();
                                    spawn_local(async move {
                                        gloo_timers::future::TimeoutFuture::new(3_000).await;
                                        success_clone.set(None);
                                    });
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
        <>
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
         
            {
                if (*user_profile).sub_tier != Some("self_hosted".to_string()) {
                    html! {
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
                    }
                } else {
                    html! {}
                }
            }
         
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
                            {"What would you like the AI assistant to know about you? For example, preferred units (metric/imperial), language preferences, or any specific way you'd like the assistant to respond to you."}
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
                                {(*user_profile).info.clone().unwrap_or("".to_string())}
                            </span>
                        }
                    }
                }
            </div>
            <div class="profile-field">
                <div class="field-label-group">
                    <span class="field-label">{"Location"}</span>
                    <div class="tooltip">
                        <span class="tooltip-icon">{"?"}</span>
                        <span class="tooltip-text">
                            {"Enter your location as District, City, Country to help the AI with context and transcription."}
                        </span>
                    </div>
                </div>
                {
                    if *is_editing {
                        html! {
                            <input
                                type="text"
                                class="profile-input"
                                value={(*location).clone()}
                                placeholder="District, City, Country"
                                onchange={let location = location.clone(); move |e: Event| {
                                    let input: HtmlInputElement = e.target_unchecked_into();
                                    location.set(input.value());
                                }}
                            />
                        }
                    } else {
                        html! {
                            <span class="field-value">
                                {(*user_profile).location.clone().unwrap_or_default()}
                            </span>
                        }
                    }
                }
            </div>
            <div class="profile-field">
                <div class="field-label-group">
                    <span class="field-label">{"Nearby Places"}</span>
                    <div class="tooltip">
                        <span class="tooltip-icon">{"?"}</span>
                        <span class="tooltip-text">
                            {"Comma-separated list of nearby places to improve voice AI transcription accuracy. Automatically updated based on location, but you can add or remove places."}
                        </span>
                    </div>
                </div>
                {
                    if *is_editing {
                        let on_fill_click = {
                            let location = location.clone();
                            let nearby_places = nearby_places.clone();
                            let error = error.clone();
                            Callback::from(move |_e: MouseEvent| {
                                let loc = (*location).clone();
                                let nearby_places = nearby_places.clone();
                                let error = error.clone();
                                if loc.is_empty() {
                                    error.set(Some("Location is empty".to_string()));
                                    return;
                                }
                                spawn_local(async move {
                                    if let Some(token) = window()
                                        .and_then(|w| w.local_storage().ok())
                                        .flatten()
                                        .and_then(|storage| storage.get_item("token").ok())
                                        .flatten()
                                    {
                                        let encoded_loc = encode_uri_component(&loc).to_string();
                                        let url = format!("{}/api/profile/get_nearby_places?location={}", config::get_backend_url(), encoded_loc);
                                        match Request::get(&url)
                                            .header("Authorization", &format!("Bearer {}", token))
                                            .send()
                                            .await
                                        {
                                            Ok(response) if response.ok() => {
                                                if let Ok(json) = response.json::<Vec<String>>().await {
                                                    nearby_places.set(json.join(", "));
                                                } else {
                                                    error.set(Some("Failed to parse nearby places".to_string()));
                                                }
                                            }
                                            _ => {
                                                error.set(Some("Failed to fetch nearby places".to_string()));
                                            }
                                        }
                                    } else {
                                        error.set(Some("No authentication token found".to_string()));
                                    }
                                });
                            })
                        };
                        html! {
                            <div class="nearby-places-container">
                                <textarea
                                    class="profile-input"
                                    value={(*nearby_places).clone()}
                                    placeholder="Comma-separated places"
                                    onchange={let nearby_places = nearby_places.clone(); move |e: Event| {
                                        let input: HtmlInputElement = e.target_unchecked_into();
                                        nearby_places.set(input.value());
                                    }}
                                />
                                <button
                                    class="fill-button"
                                    onclick={on_fill_click}
                                >
                                    {"Fill from Location"}
                                </button>
                            </div>
                        }
                    } else {
                        html! {
                            <span class="field-value">
                                {(*user_profile).nearby_places.clone().unwrap_or_default()}
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
            <div class="profile-field">
                <div class="field-label-group">
                    <span class="field-label">{"Agent Language"}</span>
                    <div class="tooltip">
                        <span class="tooltip-icon">{"?"}</span>
                        <span class="tooltip-text">
                            {"Choose the language the AI assistant will use when speaking to you. This affects voice calls."}
                        </span>
                    </div>
                </div>
                {
                    if *is_editing {
                        html! {
                            <select
                                class="profile-input"
                                value={(*agent_language).clone()}
                                onchange={let agent_language = agent_language.clone(); move |e: Event| {
                                    let select: HtmlInputElement = e.target_unchecked_into();
                                    agent_language.set(select.value());
                                }}
                            >
                                <option value="en" selected={*agent_language == "en"}>
                                    {"English"}
                                </option>
                                <option value="fi" selected={*agent_language == "fi"}>
                                    {"Finnish"}
                                </option>
                                <option value="de" selected={*agent_language == "de"}>
                                    {"German"}
                                </option>
                            </select>
                        }
                    } else {
                        html! {
                            <span class="field-value">
                                {
                                    match (*user_profile).agent_language.as_str() {
                                        "en" => "English",
                                        "fi" => "Finnish",
                                        "de" => "German",
                                        _ => "English"
                                    }
                                }
                            </span>
                        }
                    }
                }
            </div>
            <div class="profile-field">
                <div class="field-label-group">
                    <span class="field-label">{"Notification Type"}</span>
                    <div class="tooltip">
                        <span class="tooltip-icon">{"?"}</span>
                        <span class="tooltip-text">
                            {"Choose how you want to receive notifications. You can choose between SMS and calling. Note that you won't receive anything unless you have proactive notifications enabled."}
                        </span>
                    </div>
                </div>
                {
                    if *is_editing {
                        html! {
                            <select
                                class="profile-input"
                                value={(*notification_type).clone().unwrap_or_else(|| "sms".to_string())}
                                onchange={let notification_type = notification_type.clone(); move |e: Event| {
                                    let select: HtmlInputElement = e.target_unchecked_into();
                                    info!("{}", &format!("Select changed to: {}", select.value()));
                                    let value = if select.value() == "none" { None } else { Some(select.value()) };
                                    notification_type.set(value);
                                }}
                            >
                                <option value="sms" selected={(*notification_type).as_deref().unwrap_or("sms") == "sms"}>
                                    {"Text me"}
                                </option>
                                <option value="call" selected={(*notification_type).as_deref() == Some("call")}>
                                    {"Call me"}
                                </option>
                            </select>
                        }
                    } else {
                        html! {
                            <span class="field-value">
                                {
                                    match (*user_profile).notification_type.as_deref() {
                                        Some("call") => "Voice call",
                                        Some("sms") => "SMS",
                                        Some(other) => {
                                            web_sys::console::log_1(&format!("Unexpected notification type: {:?}", other).into());
                                            other
                                        },
                                        None => "SMS" // Default to SMS if no preference is set
                                    }
                                }
                            </span>
                        }
                    }
                }
            </div>
            <div class="profile-field">
                <div class="field-label-group">
                    <span class="field-label">{"Conversation History"}</span>
                    <div class="tooltip">
                        <span class="tooltip-icon">{"?"}</span>
                        <span class="tooltip-text">
                            {"Choose how many back-and-forth messages Lightfriend remembers in SMS conversations. A value of 0 means no history is kept. The conversation history is securely encrypted when not in use, and only the specified number of recent exchanges is retained. More history means better context and no history means lightfriend responds to every query like it was the first one."}
                        </span>
                    </div>
                </div>
                {
                    if *is_editing {
                        html! {
                            <select
                                class="profile-input"
                                value={(*save_context).to_string()}
                                onchange={let save_context = save_context.clone(); move |e: Event| {
                                    let select: HtmlInputElement = e.target_unchecked_into();
                                    if let Ok(value) = select.value().parse::<i32>() {
                                        save_context.set(value);
                                    }
                                }}
                            >
                                {
                                    (1..=10).map(|i| {
                                        html! {
                                            <option value={i.to_string()} selected={*save_context == i}>
                                                {format!("{} {}", i, if i == 1 { "message" } else { "messages" })}
                                            </option>
                                        }
                                    }).collect::<Html>()
                                }
                                <option value="0" selected={*save_context == 0}>{"No history"}</option>
                            </select>
                        }
                    } else {
                        html! {
                            <span class="field-value">
                                {
                                    match (*user_profile).save_context {
                                        Some(0) | None => "No history".to_string(),
                                        Some(n) => format!("{} {}", n, if n == 1 { "message" } else { "messages" }).to_string()
                                    }
                                }
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
        <style>
                {r#"
.profile-input {
    background: rgba(0, 0, 0, 0.2);
    border: 1px solid rgba(30, 144, 255, 0.2);
    border-radius: 8px;
    padding: 0.75rem;
    color: #ffffff;
    font-size: 1rem;
    transition: all 0.3s ease;
    width: 100%;
}
.profile-input:focus {
    outline: none;
    border-color: rgba(30, 144, 255, 0.5);
    box-shadow: 0 0 0 2px rgba(30, 144, 255, 0.1);
}
.profile-input[ type="text" ], .profile-input[ type="email" ], .profile-input[ type="tel" ] {
    height: auto;
}
textarea.profile-input {
    height: 100px;
    resize: vertical;
}
.edit-button {
    background: linear-gradient(45deg, #1E90FF, #4169E1);
    color: white;
    border: none;
    padding: 0.75rem 1.5rem;
    border-radius: 8px;
    font-size: 1rem;
    cursor: pointer;
    transition: all 0.3s ease;
    margin-top: 1rem;
}
.edit-button:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 20px rgba(30, 144, 255, 0.3);
}
.edit-button.confirming {
    background: linear-gradient(45deg, #4CAF50, #45a049);
}
.field-label-group {
    display: flex;
    align-items: center;
    gap: 8px;
}
.tooltip {
    position: relative;
    display: inline-block;
}
.tooltip-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    background-color: #e0e0e0;
    border-radius: 50%;
    font-size: 12px;
    cursor: help;
    color: #666;
}
.tooltip-text {
    visibility: hidden;
    position: absolute;
    width: 300px;
    background-color: #333;
    color: white;
    text-align: left;
    padding: 8px;
    border-radius: 4px;
    font-size: 14px;
    line-height: 1.4;
    z-index: 1;
    bottom: 125%;
    left: 50%;
    transform: translateX(-50%);
    opacity: 0;
    transition: opacity 0.3s;
}
.tooltip:hover .tooltip-text {
    visibility: visible;
    opacity: 1;
}
/* Add a small arrow at the bottom of the tooltip */
.tooltip-text::after {
    content: "";
    position: absolute;
    top: 100%;
    left: 50%;
    margin-left: -5px;
    border-width: 5px;
    border-style: solid;
    border-color: #333 transparent transparent transparent;
}
/* Timezone section styling */
.timezone-section {
    display: flex;
    flex-direction: column;
    gap: 12px;
    width: 100%;
}
.timezone-auto-checkbox {
    margin-bottom: 8px;
}
.custom-checkbox {
    display: flex;
    align-items: center;
    position: relative;
    padding-left: 35px;
    cursor: pointer;
    font-size: 1rem;
    user-select: none;
    color: #ffffff;
    opacity: 0.9;
    transition: opacity 0.3s ease;
}
.custom-checkbox:hover {
    opacity: 1;
}
.custom-checkbox input {
    position: absolute;
    opacity: 0;
    cursor: pointer;
    height: 0;
    width: 0;
}
.checkmark {
    position: absolute;
    left: 0;
    height: 22px;
    width: 22px;
    background: rgba(0, 0, 0, 0.2);
    border: 2px solid rgba(30, 144, 255, 0.3);
    border-radius: 4px;
    transition: all 0.3s ease;
}
.custom-checkbox:hover .checkmark {
    border-color: rgba(30, 144, 255, 0.5);
    box-shadow: 0 0 0 2px rgba(30, 144, 255, 0.1);
}
.custom-checkbox input:checked ~ .checkmark {
    background: linear-gradient(45deg, #1E90FF, #4169E1);
    border-color: transparent;
}
.checkmark:after {
    content: "";
    position: absolute;
    display: none;
    left: 7px;
    top: 3px;
    width: 5px;
    height: 10px;
    border: solid white;
    border-width: 0 2px 2px 0;
    transform: rotate(45deg);
}
.custom-checkbox input:checked ~ .checkmark:after {
    display: block;
}
.custom-checkbox input:disabled ~ .checkmark {
    opacity: 0.5;
    cursor: not-allowed;
}
.custom-checkbox input:disabled ~ .checkmark:hover {
    border-color: rgba(30, 144, 255, 0.3);
    box-shadow: none;
}
.timezone-display {
    display: flex;
    align-items: center;
    gap: 8px;
}
.auto-tag {
    font-size: 0.85rem;
    color: #1E90FF;
    background: rgba(30, 144, 255, 0.1);
    padding: 2px 8px;
    border-radius: 12px;
    border: 1px solid rgba(30, 144, 255, 0.2);
}
/* Disabled select styling */
.profile-input:disabled {
    opacity: 0.5;
    cursor: not-allowed;
}
.nearby-places-container {
    display: flex;
    flex-direction: column;
    gap: 8px;
    width: 100%;
}
.fill-button {
    background: linear-gradient(45deg, #1E90FF, #4169E1);
    color: white;
    border: none;
    padding: 0.5rem 1rem;
    border-radius: 8px;
    font-size: 0.9rem;
    cursor: pointer;
    transition: all 0.3s ease;
    align-self: flex-start;
}
.fill-button:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 20px rgba(30, 144, 255, 0.3);
}
                "#}
        </style>
        </>
    }
}
