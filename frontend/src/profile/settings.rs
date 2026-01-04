use yew::prelude::*;
use log::info;
use web_sys::{HtmlInputElement, KeyboardEvent};
use yew_router::prelude::*;
use crate::Route;
use crate::utils::api::Api;
use crate::utils::webauthn;
use crate::profile::timezone_detector::TimezoneDetector;
use crate::profile::security::SecuritySettings;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;
use crate::profile::billing_models::UserProfile;
use web_sys::js_sys::encode_uri_component;

const MAX_NICKNAME_LENGTH: usize = 30;
const MAX_INFO_LENGTH: usize = 500;

// Request for patching individual fields
#[derive(Serialize)]
struct PatchFieldRequest {
    field: String,
    value: serde_json::Value,
}

// Request for updating sensitive fields (email, phone)
#[derive(Serialize)]
struct UpdateProfileRequest {
    email: String,
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
    preferred_number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    totp_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    passkey_response: Option<serde_json::Value>,
}

// Response for 2FA requirements check
#[derive(Deserialize, Clone, Debug)]
struct SensitiveChangeRequirements {
    requires_2fa: bool,
    has_passkeys: bool,
    has_totp: bool,
    passkey_options: Option<serde_json::Value>,
}

// State for the 2FA verification modal
#[derive(Clone, PartialEq)]
enum TwoFactorVerifyState {
    Hidden,
    Loading,
    ShowingOptions {
        has_passkeys: bool,
        has_totp: bool,
        passkey_options: Option<serde_json::Value>,
        change_type: SensitiveChangeType,
    },
    WaitingForPasskey,
    WaitingForTotp,
    Verifying,
    Error(String),
}

#[derive(Clone, PartialEq)]
enum SensitiveChangeType {
    Email(String),
    Phone(String),
}

#[derive(Clone, PartialEq)]
pub enum FieldSaveState {
    Idle,
    Saving,
    Success,
    Error(String),
}

#[derive(Properties, PartialEq, Clone)]
pub struct SettingsPageProps {
    pub user_profile: UserProfile,
    pub on_profile_update: Callback<UserProfile>,
}

// Helper function to perform email update
async fn perform_profile_update_email(
    new_email: String,
    totp_code: Option<String>,
    passkey_response: Option<serde_json::Value>,
    email_original: UseStateHandle<String>,
    save_state: UseStateHandle<FieldSaveState>,
    user_profile: UseStateHandle<UserProfile>,
    on_profile_update: Callback<UserProfile>,
    pending_email: UseStateHandle<Option<String>>,
) {
    save_state.set(FieldSaveState::Saving);
    let profile = (*user_profile).clone();
    let request = UpdateProfileRequest {
        email: new_email.clone(),
        phone_number: profile.phone_number.clone(),
        nickname: profile.nickname.clone().unwrap_or_default(),
        info: profile.info.clone().unwrap_or_default(),
        timezone: profile.timezone.clone().unwrap_or_else(|| "UTC".to_string()),
        timezone_auto: profile.timezone_auto.unwrap_or(true),
        agent_language: profile.agent_language.clone(),
        notification_type: profile.notification_type.clone(),
        save_context: profile.save_context,
        location: profile.location.clone().unwrap_or_default(),
        nearby_places: profile.nearby_places.clone().unwrap_or_default(),
        preferred_number: profile.preferred_number.clone(),
        totp_code,
        passkey_response,
    };
    match Api::post("/api/profile/update")
        .json(&request)
        .unwrap()
        .send()
        .await
    {
        Ok(response) if response.ok() => {
            email_original.set(new_email.clone());
            let mut profile = (*user_profile).clone();
            profile.email = new_email;
            on_profile_update.emit(profile);
            save_state.set(FieldSaveState::Success);
            let save_state_clone = save_state.clone();
            spawn_local(async move {
                gloo_timers::future::TimeoutFuture::new(2000).await;
                save_state_clone.set(FieldSaveState::Idle);
            });
        }
        Ok(response) => {
            // Try to get error message from response
            let error_msg = if let Ok(error_json) = response.json::<serde_json::Value>().await {
                error_json.get("error")
                    .and_then(|e| e.as_str())
                    .unwrap_or("Email already exists or invalid")
                    .to_string()
            } else {
                "Email already exists or invalid".to_string()
            };
            save_state.set(FieldSaveState::Error(error_msg));
        }
        Err(_) => {
            save_state.set(FieldSaveState::Error("Network error".to_string()));
        }
    }
    pending_email.set(None);
}

// Helper function to perform phone update
async fn perform_profile_update_phone(
    new_phone: String,
    totp_code: Option<String>,
    passkey_response: Option<serde_json::Value>,
    phone_number_original: UseStateHandle<String>,
    save_state: UseStateHandle<FieldSaveState>,
    user_profile: UseStateHandle<UserProfile>,
    on_profile_update: Callback<UserProfile>,
    pending_phone: UseStateHandle<Option<String>>,
    navigator: Navigator,
) {
    save_state.set(FieldSaveState::Saving);
    let profile = (*user_profile).clone();
    let request = UpdateProfileRequest {
        email: profile.email.clone(),
        phone_number: new_phone.clone(),
        nickname: profile.nickname.clone().unwrap_or_default(),
        info: profile.info.clone().unwrap_or_default(),
        timezone: profile.timezone.clone().unwrap_or_else(|| "UTC".to_string()),
        timezone_auto: profile.timezone_auto.unwrap_or(true),
        agent_language: profile.agent_language.clone(),
        notification_type: profile.notification_type.clone(),
        save_context: profile.save_context,
        location: profile.location.clone().unwrap_or_default(),
        nearby_places: profile.nearby_places.clone().unwrap_or_default(),
        preferred_number: profile.preferred_number.clone(),
        totp_code,
        passkey_response,
    };
    match Api::post("/api/profile/update")
        .json(&request)
        .unwrap()
        .send()
        .await
    {
        Ok(response) if response.ok() => {
            phone_number_original.set(new_phone.clone());
            let mut profile = (*user_profile).clone();
            profile.phone_number = new_phone;
            on_profile_update.emit(profile);
            save_state.set(FieldSaveState::Success);
            // Redirect to home for verification
            navigator.push(&Route::Home);
        }
        Ok(response) => {
            // Try to get error message from response
            let error_msg = if let Ok(error_json) = response.json::<serde_json::Value>().await {
                error_json.get("error")
                    .and_then(|e| e.as_str())
                    .unwrap_or("Phone number already exists or invalid")
                    .to_string()
            } else {
                "Phone number already exists or invalid".to_string()
            };
            save_state.set(FieldSaveState::Error(error_msg));
        }
        Err(_) => {
            save_state.set(FieldSaveState::Error("Network error".to_string()));
        }
    }
    pending_phone.set(None);
}

#[function_component]
pub fn SettingsPage(props: &SettingsPageProps) -> Html {
    let user_profile = use_state(|| props.user_profile.clone());
    let navigator = use_navigator().unwrap();

    // Field values
    let email = use_state(|| (*user_profile).email.clone());
    let email_original = use_state(|| (*user_profile).email.clone());
    let phone_number = use_state(|| (*user_profile).phone_number.clone());
    let phone_number_original = use_state(|| (*user_profile).phone_number.clone());
    let nickname = use_state(|| (*user_profile).nickname.clone().unwrap_or_default());
    let nickname_original = use_state(|| (*user_profile).nickname.clone().unwrap_or_default());
    let info = use_state(|| (*user_profile).info.clone().unwrap_or_default());
    let info_original = use_state(|| (*user_profile).info.clone().unwrap_or_default());
    let timezone = use_state(|| (*user_profile).timezone.clone().unwrap_or_else(|| String::from("UTC")));
    let timezone_auto = use_state(|| (*user_profile).timezone_auto.unwrap_or(true));
    let phone_service_active = use_state(|| (*user_profile).phone_service_active.unwrap_or(true));
    let agent_language = use_state(|| (*user_profile).agent_language.clone());
    let notification_type = use_state(|| (*user_profile).notification_type.clone().or(Some("sms".to_string())));
    let save_context = use_state(|| (*user_profile).save_context.unwrap_or(0));
    let llm_provider = use_state(|| (*user_profile).llm_provider.clone().unwrap_or_else(|| "openai".to_string()));
    let location = use_state(|| (*user_profile).location.clone().unwrap_or_default());
    let location_original = use_state(|| (*user_profile).location.clone().unwrap_or_default());
    let nearby_places = use_state(|| (*user_profile).nearby_places.clone().unwrap_or_default());
    let nearby_places_original = use_state(|| (*user_profile).nearby_places.clone().unwrap_or_default());

    // Per-field save states
    let nickname_save_state = use_state(|| FieldSaveState::Idle);
    let info_save_state = use_state(|| FieldSaveState::Idle);
    let location_save_state = use_state(|| FieldSaveState::Idle);
    let nearby_places_save_state = use_state(|| FieldSaveState::Idle);
    let timezone_save_state = use_state(|| FieldSaveState::Idle);
    let timezone_auto_save_state = use_state(|| FieldSaveState::Idle);
    let phone_service_active_save_state = use_state(|| FieldSaveState::Idle);
    let agent_language_save_state = use_state(|| FieldSaveState::Idle);
    let notification_type_save_state = use_state(|| FieldSaveState::Idle);
    let save_context_save_state = use_state(|| FieldSaveState::Idle);
    let llm_provider_save_state = use_state(|| FieldSaveState::Idle);

    // Confirmation dialog states for sensitive fields
    let show_email_confirm = use_state(|| false);
    let show_phone_confirm = use_state(|| false);
    let pending_email = use_state(|| None::<String>);
    let pending_phone = use_state(|| None::<String>);
    let email_save_state = use_state(|| FieldSaveState::Idle);
    let phone_save_state = use_state(|| FieldSaveState::Idle);

    // 2FA verification states
    let two_factor_state = use_state(|| TwoFactorVerifyState::Hidden);
    let totp_code_input = use_state(String::new);
    let pending_passkey_options = use_state(|| None::<serde_json::Value>);
    let pending_change_type = use_state(|| None::<SensitiveChangeType>);

    // Update local state when props change
    {
        let email = email.clone();
        let email_original = email_original.clone();
        let phone_number = phone_number.clone();
        let phone_number_original = phone_number_original.clone();
        let nickname = nickname.clone();
        let nickname_original = nickname_original.clone();
        let info = info.clone();
        let info_original = info_original.clone();
        let timezone = timezone.clone();
        let timezone_auto = timezone_auto.clone();
        let phone_service_active = phone_service_active.clone();
        let user_profile_state = user_profile.clone();
        let agent_language = agent_language.clone();
        let notification_type = notification_type.clone();
        let save_context = save_context.clone();
        let location = location.clone();
        let location_original = location_original.clone();
        let nearby_places = nearby_places.clone();
        let nearby_places_original = nearby_places_original.clone();
        use_effect_with_deps(move |props_profile| {
            email.set(props_profile.email.clone());
            email_original.set(props_profile.email.clone());
            phone_number.set(props_profile.phone_number.clone());
            phone_number_original.set(props_profile.phone_number.clone());
            nickname.set(props_profile.nickname.clone().unwrap_or_default());
            nickname_original.set(props_profile.nickname.clone().unwrap_or_default());
            info.set(props_profile.info.clone().unwrap_or_default());
            info_original.set(props_profile.info.clone().unwrap_or_default());
            timezone.set(props_profile.timezone.clone().unwrap_or_else(|| String::from("UTC")));
            timezone_auto.set(props_profile.timezone_auto.unwrap_or(true));
            phone_service_active.set(props_profile.phone_service_active.unwrap_or(true));
            agent_language.set(props_profile.agent_language.clone());
            notification_type.set(props_profile.notification_type.clone());
            location.set(props_profile.location.clone().unwrap_or_default());
            location_original.set(props_profile.location.clone().unwrap_or_default());
            nearby_places.set(props_profile.nearby_places.clone().unwrap_or_default());
            nearby_places_original.set(props_profile.nearby_places.clone().unwrap_or_default());
            save_context.set(props_profile.save_context.unwrap_or(0));
            user_profile_state.set(props_profile.clone());
            || ()
        }, props.user_profile.clone());
    }

    // Helper to save a field via PATCH
    fn save_field(
        field: &str,
        value: serde_json::Value,
        save_state: UseStateHandle<FieldSaveState>,
    ) {
        let field = field.to_string();
        save_state.set(FieldSaveState::Saving);
        spawn_local(async move {
            let request = PatchFieldRequest { field: field.clone(), value };
            match Api::patch("/api/profile/field")
                .json(&request)
                .unwrap()
                .send()
                .await
            {
                Ok(response) if response.ok() => {
                    save_state.set(FieldSaveState::Success);
                    let save_state_clone = save_state.clone();
                    spawn_local(async move {
                        gloo_timers::future::TimeoutFuture::new(2000).await;
                        save_state_clone.set(FieldSaveState::Idle);
                    });
                }
                Ok(response) => {
                    let error_msg = response.text().await.unwrap_or_else(|_| "Failed to save".to_string());
                    info!("Failed to save {}: {}", field, error_msg);
                    save_state.set(FieldSaveState::Error(error_msg));
                }
                Err(e) => {
                    info!("Network error saving {}: {:?}", field, e);
                    save_state.set(FieldSaveState::Error("Network error".to_string()));
                }
            }
        });
    }

    // Helper function to save nickname
    fn save_nickname(
        nickname: UseStateHandle<String>,
        nickname_original: UseStateHandle<String>,
        save_state: UseStateHandle<FieldSaveState>,
        user_profile: UseStateHandle<UserProfile>,
        on_profile_update: Callback<UserProfile>,
    ) {
        if *nickname != *nickname_original {
            let new_val = (*nickname).clone();
            let save_state = save_state.clone();
            let nickname_original = nickname_original.clone();
            let user_profile = user_profile.clone();
            let on_profile_update = on_profile_update.clone();
            save_state.set(FieldSaveState::Saving);
            spawn_local(async move {
                let request = PatchFieldRequest {
                    field: "nickname".to_string(),
                    value: serde_json::Value::String(new_val.clone())
                };
                match Api::patch("/api/profile/field")
                    .json(&request)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(response) if response.ok() => {
                        nickname_original.set(new_val.clone());
                        let mut profile = (*user_profile).clone();
                        profile.nickname = Some(new_val);
                        on_profile_update.emit(profile);
                        save_state.set(FieldSaveState::Success);
                        let save_state_clone = save_state.clone();
                        spawn_local(async move {
                            gloo_timers::future::TimeoutFuture::new(2000).await;
                            save_state_clone.set(FieldSaveState::Idle);
                        });
                    }
                    Ok(_) => {
                        save_state.set(FieldSaveState::Error("Failed to save".to_string()));
                    }
                    Err(_) => {
                        save_state.set(FieldSaveState::Error("Network error".to_string()));
                    }
                }
            });
        }
    }

    // Nickname blur handler
    let on_nickname_blur = {
        let nickname = nickname.clone();
        let nickname_original = nickname_original.clone();
        let save_state = nickname_save_state.clone();
        let user_profile = user_profile.clone();
        let on_profile_update = props.on_profile_update.clone();
        Callback::from(move |_: FocusEvent| {
            save_nickname(
                nickname.clone(),
                nickname_original.clone(),
                save_state.clone(),
                user_profile.clone(),
                on_profile_update.clone(),
            );
        })
    };

    // Nickname keypress handler (save on Enter)
    let on_nickname_keypress = {
        let nickname = nickname.clone();
        let nickname_original = nickname_original.clone();
        let save_state = nickname_save_state.clone();
        let user_profile = user_profile.clone();
        let on_profile_update = props.on_profile_update.clone();
        Callback::from(move |e: KeyboardEvent| {
            if e.key() == "Enter" {
                e.prevent_default();
                save_nickname(
                    nickname.clone(),
                    nickname_original.clone(),
                    save_state.clone(),
                    user_profile.clone(),
                    on_profile_update.clone(),
                );
            }
        })
    };

    // Helper function to save info
    fn save_info(
        info: UseStateHandle<String>,
        info_original: UseStateHandle<String>,
        save_state: UseStateHandle<FieldSaveState>,
        user_profile: UseStateHandle<UserProfile>,
        on_profile_update: Callback<UserProfile>,
    ) {
        if *info != *info_original {
            let new_val = (*info).clone();
            let save_state = save_state.clone();
            let info_original = info_original.clone();
            let user_profile = user_profile.clone();
            let on_profile_update = on_profile_update.clone();
            save_state.set(FieldSaveState::Saving);
            spawn_local(async move {
                let request = PatchFieldRequest {
                    field: "info".to_string(),
                    value: serde_json::Value::String(new_val.clone())
                };
                match Api::patch("/api/profile/field")
                    .json(&request)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(response) if response.ok() => {
                        info_original.set(new_val.clone());
                        let mut profile = (*user_profile).clone();
                        profile.info = Some(new_val);
                        on_profile_update.emit(profile);
                        save_state.set(FieldSaveState::Success);
                        let save_state_clone = save_state.clone();
                        spawn_local(async move {
                            gloo_timers::future::TimeoutFuture::new(2000).await;
                            save_state_clone.set(FieldSaveState::Idle);
                        });
                    }
                    Ok(_) => {
                        save_state.set(FieldSaveState::Error("Failed to save".to_string()));
                    }
                    Err(_) => {
                        save_state.set(FieldSaveState::Error("Network error".to_string()));
                    }
                }
            });
        }
    }

    // Info blur handler
    let on_info_blur = {
        let info = info.clone();
        let info_original = info_original.clone();
        let save_state = info_save_state.clone();
        let user_profile = user_profile.clone();
        let on_profile_update = props.on_profile_update.clone();
        Callback::from(move |_: FocusEvent| {
            save_info(
                info.clone(),
                info_original.clone(),
                save_state.clone(),
                user_profile.clone(),
                on_profile_update.clone(),
            );
        })
    };

    // Info keypress handler (save on Ctrl+Enter for textarea)
    let on_info_keypress = {
        let info = info.clone();
        let info_original = info_original.clone();
        let save_state = info_save_state.clone();
        let user_profile = user_profile.clone();
        let on_profile_update = props.on_profile_update.clone();
        Callback::from(move |e: KeyboardEvent| {
            // Use Ctrl+Enter or Cmd+Enter for textarea to allow regular Enter for newlines
            if e.key() == "Enter" && (e.ctrl_key() || e.meta_key()) {
                e.prevent_default();
                save_info(
                    info.clone(),
                    info_original.clone(),
                    save_state.clone(),
                    user_profile.clone(),
                    on_profile_update.clone(),
                );
            }
        })
    };

    // Helper function to save location
    fn save_location(
        location: UseStateHandle<String>,
        location_original: UseStateHandle<String>,
        save_state: UseStateHandle<FieldSaveState>,
        user_profile: UseStateHandle<UserProfile>,
        on_profile_update: Callback<UserProfile>,
    ) {
        if *location != *location_original {
            let new_val = (*location).clone();
            let save_state = save_state.clone();
            let location_original = location_original.clone();
            let user_profile = user_profile.clone();
            let on_profile_update = on_profile_update.clone();
            save_state.set(FieldSaveState::Saving);
            spawn_local(async move {
                let request = PatchFieldRequest {
                    field: "location".to_string(),
                    value: serde_json::Value::String(new_val.clone())
                };
                match Api::patch("/api/profile/field")
                    .json(&request)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(response) if response.ok() => {
                        location_original.set(new_val.clone());
                        let mut profile = (*user_profile).clone();
                        profile.location = Some(new_val);
                        on_profile_update.emit(profile);
                        save_state.set(FieldSaveState::Success);
                        let save_state_clone = save_state.clone();
                        spawn_local(async move {
                            gloo_timers::future::TimeoutFuture::new(2000).await;
                            save_state_clone.set(FieldSaveState::Idle);
                        });
                    }
                    Ok(_) => {
                        save_state.set(FieldSaveState::Error("Failed to save".to_string()));
                    }
                    Err(_) => {
                        save_state.set(FieldSaveState::Error("Network error".to_string()));
                    }
                }
            });
        }
    }

    // Location blur handler
    let on_location_blur = {
        let location = location.clone();
        let location_original = location_original.clone();
        let save_state = location_save_state.clone();
        let user_profile = user_profile.clone();
        let on_profile_update = props.on_profile_update.clone();
        Callback::from(move |_: FocusEvent| {
            save_location(
                location.clone(),
                location_original.clone(),
                save_state.clone(),
                user_profile.clone(),
                on_profile_update.clone(),
            );
        })
    };

    // Location keypress handler (save on Enter)
    let on_location_keypress = {
        let location = location.clone();
        let location_original = location_original.clone();
        let save_state = location_save_state.clone();
        let user_profile = user_profile.clone();
        let on_profile_update = props.on_profile_update.clone();
        Callback::from(move |e: KeyboardEvent| {
            if e.key() == "Enter" {
                e.prevent_default();
                save_location(
                    location.clone(),
                    location_original.clone(),
                    save_state.clone(),
                    user_profile.clone(),
                    on_profile_update.clone(),
                );
            }
        })
    };

    // Helper function to save nearby_places
    fn save_nearby_places(
        nearby_places: UseStateHandle<String>,
        nearby_places_original: UseStateHandle<String>,
        save_state: UseStateHandle<FieldSaveState>,
        user_profile: UseStateHandle<UserProfile>,
        on_profile_update: Callback<UserProfile>,
    ) {
        if *nearby_places != *nearby_places_original {
            let new_val = (*nearby_places).clone();
            let save_state = save_state.clone();
            let nearby_places_original = nearby_places_original.clone();
            let user_profile = user_profile.clone();
            let on_profile_update = on_profile_update.clone();
            save_state.set(FieldSaveState::Saving);
            spawn_local(async move {
                let request = PatchFieldRequest {
                    field: "nearby_places".to_string(),
                    value: serde_json::Value::String(new_val.clone())
                };
                match Api::patch("/api/profile/field")
                    .json(&request)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(response) if response.ok() => {
                        nearby_places_original.set(new_val.clone());
                        let mut profile = (*user_profile).clone();
                        profile.nearby_places = Some(new_val);
                        on_profile_update.emit(profile);
                        save_state.set(FieldSaveState::Success);
                        let save_state_clone = save_state.clone();
                        spawn_local(async move {
                            gloo_timers::future::TimeoutFuture::new(2000).await;
                            save_state_clone.set(FieldSaveState::Idle);
                        });
                    }
                    Ok(_) => {
                        save_state.set(FieldSaveState::Error("Failed to save".to_string()));
                    }
                    Err(_) => {
                        save_state.set(FieldSaveState::Error("Network error".to_string()));
                    }
                }
            });
        }
    }

    // Nearby places blur handler
    let on_nearby_places_blur = {
        let nearby_places = nearby_places.clone();
        let nearby_places_original = nearby_places_original.clone();
        let save_state = nearby_places_save_state.clone();
        let user_profile = user_profile.clone();
        let on_profile_update = props.on_profile_update.clone();
        Callback::from(move |_: FocusEvent| {
            save_nearby_places(
                nearby_places.clone(),
                nearby_places_original.clone(),
                save_state.clone(),
                user_profile.clone(),
                on_profile_update.clone(),
            );
        })
    };

    // Nearby places keypress handler (save on Ctrl+Enter for textarea)
    let on_nearby_places_keypress = {
        let nearby_places = nearby_places.clone();
        let nearby_places_original = nearby_places_original.clone();
        let save_state = nearby_places_save_state.clone();
        let user_profile = user_profile.clone();
        let on_profile_update = props.on_profile_update.clone();
        Callback::from(move |e: KeyboardEvent| {
            // Use Ctrl+Enter or Cmd+Enter for textarea to allow regular Enter for newlines
            if e.key() == "Enter" && (e.ctrl_key() || e.meta_key()) {
                e.prevent_default();
                save_nearby_places(
                    nearby_places.clone(),
                    nearby_places_original.clone(),
                    save_state.clone(),
                    user_profile.clone(),
                    on_profile_update.clone(),
                );
            }
        })
    };

    // Timezone change handler (saves immediately on selection)
    let on_timezone_change = {
        let timezone = timezone.clone();
        let save_state = timezone_save_state.clone();
        let user_profile = user_profile.clone();
        let on_profile_update = props.on_profile_update.clone();
        Callback::from(move |e: Event| {
            let select: HtmlInputElement = e.target_unchecked_into();
            let new_val = select.value();
            timezone.set(new_val.clone());
            let save_state = save_state.clone();
            let user_profile = user_profile.clone();
            let on_profile_update = on_profile_update.clone();
            save_state.set(FieldSaveState::Saving);
            spawn_local(async move {
                let request = PatchFieldRequest {
                    field: "timezone".to_string(),
                    value: serde_json::Value::String(new_val.clone())
                };
                match Api::patch("/api/profile/field")
                    .json(&request)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(response) if response.ok() => {
                        let mut profile = (*user_profile).clone();
                        profile.timezone = Some(new_val);
                        on_profile_update.emit(profile);
                        save_state.set(FieldSaveState::Success);
                        let save_state_clone = save_state.clone();
                        spawn_local(async move {
                            gloo_timers::future::TimeoutFuture::new(2000).await;
                            save_state_clone.set(FieldSaveState::Idle);
                        });
                    }
                    Ok(_) => {
                        save_state.set(FieldSaveState::Error("Failed to save".to_string()));
                    }
                    Err(_) => {
                        save_state.set(FieldSaveState::Error("Network error".to_string()));
                    }
                }
            });
        })
    };

    // Timezone auto change handler
    let on_timezone_auto_change = {
        let timezone_auto = timezone_auto.clone();
        let save_state = timezone_auto_save_state.clone();
        let user_profile = user_profile.clone();
        let on_profile_update = props.on_profile_update.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let new_val = input.checked();
            timezone_auto.set(new_val);
            let save_state = save_state.clone();
            let user_profile = user_profile.clone();
            let on_profile_update = on_profile_update.clone();
            save_state.set(FieldSaveState::Saving);
            spawn_local(async move {
                let request = PatchFieldRequest {
                    field: "timezone_auto".to_string(),
                    value: serde_json::Value::Bool(new_val)
                };
                match Api::patch("/api/profile/field")
                    .json(&request)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(response) if response.ok() => {
                        let mut profile = (*user_profile).clone();
                        profile.timezone_auto = Some(new_val);
                        on_profile_update.emit(profile);
                        save_state.set(FieldSaveState::Success);
                        let save_state_clone = save_state.clone();
                        spawn_local(async move {
                            gloo_timers::future::TimeoutFuture::new(2000).await;
                            save_state_clone.set(FieldSaveState::Idle);
                        });
                    }
                    Ok(_) => {
                        save_state.set(FieldSaveState::Error("Failed to save".to_string()));
                    }
                    Err(_) => {
                        save_state.set(FieldSaveState::Error("Network error".to_string()));
                    }
                }
            });
        })
    };

    // Phone service active toggle handler
    let on_phone_service_toggle = {
        let phone_service_active = phone_service_active.clone();
        let save_state = phone_service_active_save_state.clone();
        let user_profile = user_profile.clone();
        let on_profile_update = props.on_profile_update.clone();
        Callback::from(move |e: Event| {
            let input: HtmlInputElement = e.target_unchecked_into();
            let new_val = input.checked();
            phone_service_active.set(new_val);
            let save_state = save_state.clone();
            let user_profile = user_profile.clone();
            let on_profile_update = on_profile_update.clone();
            save_state.set(FieldSaveState::Saving);
            spawn_local(async move {
                let request = PatchFieldRequest {
                    field: "phone_service_active".to_string(),
                    value: serde_json::Value::Bool(new_val)
                };
                match Api::patch("/api/profile/field")
                    .json(&request)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(response) if response.ok() => {
                        let mut profile = (*user_profile).clone();
                        profile.phone_service_active = Some(new_val);
                        on_profile_update.emit(profile);
                        save_state.set(FieldSaveState::Success);
                        let save_state_clone = save_state.clone();
                        spawn_local(async move {
                            gloo_timers::future::TimeoutFuture::new(2000).await;
                            save_state_clone.set(FieldSaveState::Idle);
                        });
                    }
                    Ok(_) => {
                        save_state.set(FieldSaveState::Error("Failed to save".to_string()));
                    }
                    Err(_) => {
                        save_state.set(FieldSaveState::Error("Network error".to_string()));
                    }
                }
            });
        })
    };

    // Agent language change handler
    let on_agent_language_change = {
        let agent_language = agent_language.clone();
        let save_state = agent_language_save_state.clone();
        let user_profile = user_profile.clone();
        let on_profile_update = props.on_profile_update.clone();
        Callback::from(move |e: Event| {
            let select: HtmlInputElement = e.target_unchecked_into();
            let new_val = select.value();
            agent_language.set(new_val.clone());
            let save_state = save_state.clone();
            let user_profile = user_profile.clone();
            let on_profile_update = on_profile_update.clone();
            save_state.set(FieldSaveState::Saving);
            spawn_local(async move {
                let request = PatchFieldRequest {
                    field: "agent_language".to_string(),
                    value: serde_json::Value::String(new_val.clone())
                };
                match Api::patch("/api/profile/field")
                    .json(&request)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(response) if response.ok() => {
                        let mut profile = (*user_profile).clone();
                        profile.agent_language = new_val;
                        on_profile_update.emit(profile);
                        save_state.set(FieldSaveState::Success);
                        let save_state_clone = save_state.clone();
                        spawn_local(async move {
                            gloo_timers::future::TimeoutFuture::new(2000).await;
                            save_state_clone.set(FieldSaveState::Idle);
                        });
                    }
                    Ok(_) => {
                        save_state.set(FieldSaveState::Error("Failed to save".to_string()));
                    }
                    Err(_) => {
                        save_state.set(FieldSaveState::Error("Network error".to_string()));
                    }
                }
            });
        })
    };

    // Notification type change handler
    let on_notification_type_change = {
        let notification_type = notification_type.clone();
        let save_state = notification_type_save_state.clone();
        let user_profile = user_profile.clone();
        let on_profile_update = props.on_profile_update.clone();
        Callback::from(move |e: Event| {
            let select: HtmlInputElement = e.target_unchecked_into();
            let new_val = select.value();
            let value = if new_val == "none" { None } else { Some(new_val.clone()) };
            notification_type.set(value.clone());
            let save_state = save_state.clone();
            let user_profile = user_profile.clone();
            let on_profile_update = on_profile_update.clone();
            save_state.set(FieldSaveState::Saving);
            spawn_local(async move {
                let request = PatchFieldRequest {
                    field: "notification_type".to_string(),
                    value: serde_json::Value::String(new_val.clone())
                };
                match Api::patch("/api/profile/field")
                    .json(&request)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(response) if response.ok() => {
                        let mut profile = (*user_profile).clone();
                        profile.notification_type = value;
                        on_profile_update.emit(profile);
                        save_state.set(FieldSaveState::Success);
                        let save_state_clone = save_state.clone();
                        spawn_local(async move {
                            gloo_timers::future::TimeoutFuture::new(2000).await;
                            save_state_clone.set(FieldSaveState::Idle);
                        });
                    }
                    Ok(_) => {
                        save_state.set(FieldSaveState::Error("Failed to save".to_string()));
                    }
                    Err(_) => {
                        save_state.set(FieldSaveState::Error("Network error".to_string()));
                    }
                }
            });
        })
    };

    // Save context change handler
    let on_save_context_change = {
        let save_context = save_context.clone();
        let save_state = save_context_save_state.clone();
        let user_profile = user_profile.clone();
        let on_profile_update = props.on_profile_update.clone();
        Callback::from(move |e: Event| {
            let select: HtmlInputElement = e.target_unchecked_into();
            if let Ok(new_val) = select.value().parse::<i32>() {
                save_context.set(new_val);
                let save_state = save_state.clone();
                let user_profile = user_profile.clone();
                let on_profile_update = on_profile_update.clone();
                save_state.set(FieldSaveState::Saving);
                spawn_local(async move {
                    let request = PatchFieldRequest {
                        field: "save_context".to_string(),
                        value: serde_json::Value::Number(new_val.into())
                    };
                    match Api::patch("/api/profile/field")
                        .json(&request)
                        .unwrap()
                        .send()
                        .await
                    {
                        Ok(response) if response.ok() => {
                            let mut profile = (*user_profile).clone();
                            profile.save_context = Some(new_val);
                            on_profile_update.emit(profile);
                            save_state.set(FieldSaveState::Success);
                            let save_state_clone = save_state.clone();
                            spawn_local(async move {
                                gloo_timers::future::TimeoutFuture::new(2000).await;
                                save_state_clone.set(FieldSaveState::Idle);
                            });
                        }
                        Ok(_) => {
                            save_state.set(FieldSaveState::Error("Failed to save".to_string()));
                        }
                        Err(_) => {
                            save_state.set(FieldSaveState::Error("Network error".to_string()));
                        }
                    }
                });
            }
        })
    };

    // LLM provider change handler
    let on_llm_provider_change = {
        let llm_provider = llm_provider.clone();
        let save_state = llm_provider_save_state.clone();
        let user_profile = user_profile.clone();
        let on_profile_update = props.on_profile_update.clone();
        Callback::from(move |e: Event| {
            let select: HtmlInputElement = e.target_unchecked_into();
            let new_val = select.value();
            llm_provider.set(new_val.clone());
            let save_state = save_state.clone();
            let user_profile = user_profile.clone();
            let on_profile_update = on_profile_update.clone();
            save_state.set(FieldSaveState::Saving);
            spawn_local(async move {
                let request = PatchFieldRequest {
                    field: "llm_provider".to_string(),
                    value: serde_json::Value::String(new_val.clone())
                };
                match Api::patch("/api/profile/field")
                    .json(&request)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(response) if response.ok() => {
                        let mut profile = (*user_profile).clone();
                        profile.llm_provider = Some(new_val);
                        on_profile_update.emit(profile);
                        save_state.set(FieldSaveState::Success);
                        let save_state_clone = save_state.clone();
                        spawn_local(async move {
                            gloo_timers::future::TimeoutFuture::new(2000).await;
                            save_state_clone.set(FieldSaveState::Idle);
                        });
                    }
                    Ok(_) => {
                        save_state.set(FieldSaveState::Error("Failed to save".to_string()));
                    }
                    Err(_) => {
                        save_state.set(FieldSaveState::Error("Network error".to_string()));
                    }
                }
            });
        })
    };

    // Email blur handler - shows confirmation dialog
    let on_email_blur = {
        let email = email.clone();
        let email_original = email_original.clone();
        let show_email_confirm = show_email_confirm.clone();
        let pending_email = pending_email.clone();
        Callback::from(move |_: FocusEvent| {
            if *email != *email_original {
                pending_email.set(Some((*email).clone()));
                show_email_confirm.set(true);
            }
        })
    };

    // Email keypress handler - shows confirmation dialog on Enter
    let on_email_keypress = {
        let email = email.clone();
        let email_original = email_original.clone();
        let show_email_confirm = show_email_confirm.clone();
        let pending_email = pending_email.clone();
        Callback::from(move |e: KeyboardEvent| {
            if e.key() == "Enter" {
                e.prevent_default();
                if *email != *email_original {
                    pending_email.set(Some((*email).clone()));
                    show_email_confirm.set(true);
                }
            }
        })
    };

    // Phone blur handler - shows confirmation dialog
    let on_phone_blur = {
        let phone_number = phone_number.clone();
        let phone_number_original = phone_number_original.clone();
        let show_phone_confirm = show_phone_confirm.clone();
        let pending_phone = pending_phone.clone();
        Callback::from(move |_: FocusEvent| {
            if *phone_number != *phone_number_original {
                pending_phone.set(Some((*phone_number).clone()));
                show_phone_confirm.set(true);
            }
        })
    };

    // Phone keypress handler - shows confirmation dialog on Enter
    let on_phone_keypress = {
        let phone_number = phone_number.clone();
        let phone_number_original = phone_number_original.clone();
        let show_phone_confirm = show_phone_confirm.clone();
        let pending_phone = pending_phone.clone();
        Callback::from(move |e: KeyboardEvent| {
            if e.key() == "Enter" {
                e.prevent_default();
                if *phone_number != *phone_number_original {
                    pending_phone.set(Some((*phone_number).clone()));
                    show_phone_confirm.set(true);
                }
            }
        })
    };

    // Email confirm save - checks for 2FA first
    let on_email_confirm = {
        let pending_email = pending_email.clone();
        let show_email_confirm = show_email_confirm.clone();
        let two_factor_state = two_factor_state.clone();
        let pending_change_type = pending_change_type.clone();
        let pending_passkey_options = pending_passkey_options.clone();
        let email_original = email_original.clone();
        let save_state = email_save_state.clone();
        let user_profile = user_profile.clone();
        let on_profile_update = props.on_profile_update.clone();
        Callback::from(move |_: MouseEvent| {
            if let Some(new_email) = (*pending_email).clone() {
                show_email_confirm.set(false);
                let two_factor_state = two_factor_state.clone();
                let pending_change_type = pending_change_type.clone();
                let pending_passkey_options = pending_passkey_options.clone();
                let email_original = email_original.clone();
                let save_state = save_state.clone();
                let user_profile = user_profile.clone();
                let on_profile_update = on_profile_update.clone();
                let pending_email = pending_email.clone();

                // First check if 2FA is required
                two_factor_state.set(TwoFactorVerifyState::Loading);
                spawn_local(async move {
                    match Api::get("/api/profile/sensitive-change-requirements").send().await {
                        Ok(resp) if resp.ok() => {
                            if let Ok(requirements) = resp.json::<SensitiveChangeRequirements>().await {
                                if requirements.requires_2fa {
                                    // Store the change type and show 2FA modal
                                    pending_change_type.set(Some(SensitiveChangeType::Email(new_email.clone())));
                                    pending_passkey_options.set(requirements.passkey_options.clone());
                                    two_factor_state.set(TwoFactorVerifyState::ShowingOptions {
                                        has_passkeys: requirements.has_passkeys,
                                        has_totp: requirements.has_totp,
                                        passkey_options: requirements.passkey_options,
                                        change_type: SensitiveChangeType::Email(new_email),
                                    });
                                } else {
                                    // No 2FA required, proceed with update
                                    two_factor_state.set(TwoFactorVerifyState::Hidden);
                                    perform_profile_update_email(
                                        new_email,
                                        None,
                                        None,
                                        email_original,
                                        save_state,
                                        user_profile,
                                        on_profile_update,
                                        pending_email,
                                    ).await;
                                }
                            } else {
                                two_factor_state.set(TwoFactorVerifyState::Error("Failed to parse 2FA requirements".to_string()));
                            }
                        }
                        Ok(_) => {
                            two_factor_state.set(TwoFactorVerifyState::Error("Failed to check 2FA requirements".to_string()));
                        }
                        Err(_) => {
                            two_factor_state.set(TwoFactorVerifyState::Error("Network error".to_string()));
                        }
                    }
                });
            }
        })
    };

    // Email cancel
    let on_email_cancel = {
        let email = email.clone();
        let email_original = email_original.clone();
        let show_email_confirm = show_email_confirm.clone();
        let pending_email = pending_email.clone();
        Callback::from(move |_: MouseEvent| {
            email.set((*email_original).clone());
            show_email_confirm.set(false);
            pending_email.set(None);
        })
    };

    // Phone confirm save - checks for 2FA first
    let on_phone_confirm = {
        let pending_phone = pending_phone.clone();
        let show_phone_confirm = show_phone_confirm.clone();
        let two_factor_state = two_factor_state.clone();
        let pending_change_type = pending_change_type.clone();
        let pending_passkey_options = pending_passkey_options.clone();
        let phone_number_original = phone_number_original.clone();
        let save_state = phone_save_state.clone();
        let user_profile = user_profile.clone();
        let on_profile_update = props.on_profile_update.clone();
        let navigator = navigator.clone();
        Callback::from(move |_: MouseEvent| {
            if let Some(new_phone) = (*pending_phone).clone() {
                show_phone_confirm.set(false);
                let two_factor_state = two_factor_state.clone();
                let pending_change_type = pending_change_type.clone();
                let pending_passkey_options = pending_passkey_options.clone();
                let phone_number_original = phone_number_original.clone();
                let save_state = save_state.clone();
                let user_profile = user_profile.clone();
                let on_profile_update = on_profile_update.clone();
                let pending_phone = pending_phone.clone();
                let navigator = navigator.clone();

                // First check if 2FA is required
                two_factor_state.set(TwoFactorVerifyState::Loading);
                spawn_local(async move {
                    match Api::get("/api/profile/sensitive-change-requirements").send().await {
                        Ok(resp) if resp.ok() => {
                            if let Ok(requirements) = resp.json::<SensitiveChangeRequirements>().await {
                                if requirements.requires_2fa {
                                    // Store the change type and show 2FA modal
                                    pending_change_type.set(Some(SensitiveChangeType::Phone(new_phone.clone())));
                                    pending_passkey_options.set(requirements.passkey_options.clone());
                                    two_factor_state.set(TwoFactorVerifyState::ShowingOptions {
                                        has_passkeys: requirements.has_passkeys,
                                        has_totp: requirements.has_totp,
                                        passkey_options: requirements.passkey_options,
                                        change_type: SensitiveChangeType::Phone(new_phone),
                                    });
                                } else {
                                    // No 2FA required, proceed with update
                                    two_factor_state.set(TwoFactorVerifyState::Hidden);
                                    perform_profile_update_phone(
                                        new_phone,
                                        None,
                                        None,
                                        phone_number_original,
                                        save_state,
                                        user_profile,
                                        on_profile_update,
                                        pending_phone,
                                        navigator,
                                    ).await;
                                }
                            } else {
                                two_factor_state.set(TwoFactorVerifyState::Error("Failed to parse 2FA requirements".to_string()));
                            }
                        }
                        Ok(_) => {
                            two_factor_state.set(TwoFactorVerifyState::Error("Failed to check 2FA requirements".to_string()));
                        }
                        Err(_) => {
                            two_factor_state.set(TwoFactorVerifyState::Error("Network error".to_string()));
                        }
                    }
                });
            }
        })
    };

    // Phone cancel
    let on_phone_cancel = {
        let phone_number = phone_number.clone();
        let phone_number_original = phone_number_original.clone();
        let show_phone_confirm = show_phone_confirm.clone();
        let pending_phone = pending_phone.clone();
        Callback::from(move |_: MouseEvent| {
            phone_number.set((*phone_number_original).clone());
            show_phone_confirm.set(false);
            pending_phone.set(None);
        })
    };

    // 2FA verification handlers
    let on_2fa_cancel = {
        let two_factor_state = two_factor_state.clone();
        let pending_change_type = pending_change_type.clone();
        let totp_code_input = totp_code_input.clone();
        let email = email.clone();
        let email_original = email_original.clone();
        let phone_number = phone_number.clone();
        let phone_number_original = phone_number_original.clone();
        let pending_email = pending_email.clone();
        let pending_phone = pending_phone.clone();
        Callback::from(move |_: MouseEvent| {
            // Reset values to original
            email.set((*email_original).clone());
            phone_number.set((*phone_number_original).clone());
            two_factor_state.set(TwoFactorVerifyState::Hidden);
            pending_change_type.set(None);
            totp_code_input.set(String::new());
            pending_email.set(None);
            pending_phone.set(None);
        })
    };

    // Use passkey for 2FA
    let on_use_passkey = {
        let two_factor_state = two_factor_state.clone();
        let pending_passkey_options = pending_passkey_options.clone();
        let pending_change_type = pending_change_type.clone();
        let email_original = email_original.clone();
        let phone_number_original = phone_number_original.clone();
        let email_save_state = email_save_state.clone();
        let phone_save_state = phone_save_state.clone();
        let user_profile = user_profile.clone();
        let on_profile_update = props.on_profile_update.clone();
        let pending_email = pending_email.clone();
        let pending_phone = pending_phone.clone();
        let navigator = navigator.clone();
        Callback::from(move |_: MouseEvent| {
            let two_factor_state = two_factor_state.clone();
            let pending_passkey_options = pending_passkey_options.clone();
            let pending_change_type = pending_change_type.clone();
            let email_original = email_original.clone();
            let phone_number_original = phone_number_original.clone();
            let email_save_state = email_save_state.clone();
            let phone_save_state = phone_save_state.clone();
            let user_profile = user_profile.clone();
            let on_profile_update = on_profile_update.clone();
            let pending_email = pending_email.clone();
            let pending_phone = pending_phone.clone();
            let navigator = navigator.clone();

            two_factor_state.set(TwoFactorVerifyState::WaitingForPasskey);

            spawn_local(async move {
                if let Some(options) = (*pending_passkey_options).clone() {
                    // Get the inner options if wrapped
                    let auth_options = options.get("options").cloned().unwrap_or(options);

                    match webauthn::get_credential(&auth_options).await {
                        Ok(credential) => {
                            two_factor_state.set(TwoFactorVerifyState::Verifying);

                            // Now submit the profile update with passkey response
                            if let Some(change_type) = (*pending_change_type).clone() {
                                match change_type {
                                    SensitiveChangeType::Email(new_email) => {
                                        perform_profile_update_email(
                                            new_email,
                                            None,
                                            Some(credential),
                                            email_original,
                                            email_save_state,
                                            user_profile,
                                            on_profile_update,
                                            pending_email,
                                        ).await;
                                        two_factor_state.set(TwoFactorVerifyState::Hidden);
                                    }
                                    SensitiveChangeType::Phone(new_phone) => {
                                        perform_profile_update_phone(
                                            new_phone,
                                            None,
                                            Some(credential),
                                            phone_number_original,
                                            phone_save_state,
                                            user_profile,
                                            on_profile_update,
                                            pending_phone,
                                            navigator,
                                        ).await;
                                        two_factor_state.set(TwoFactorVerifyState::Hidden);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            two_factor_state.set(TwoFactorVerifyState::Error(format!("Passkey verification failed: {}", e)));
                        }
                    }
                } else {
                    two_factor_state.set(TwoFactorVerifyState::Error("No passkey options available".to_string()));
                }
            });
        })
    };

    // Use TOTP for 2FA
    let on_use_totp = {
        let two_factor_state = two_factor_state.clone();
        Callback::from(move |_: MouseEvent| {
            two_factor_state.set(TwoFactorVerifyState::WaitingForTotp);
        })
    };

    // Submit TOTP code
    let on_submit_totp = {
        let two_factor_state = two_factor_state.clone();
        let totp_code_input = totp_code_input.clone();
        let pending_change_type = pending_change_type.clone();
        let email_original = email_original.clone();
        let phone_number_original = phone_number_original.clone();
        let email_save_state = email_save_state.clone();
        let phone_save_state = phone_save_state.clone();
        let user_profile = user_profile.clone();
        let on_profile_update = props.on_profile_update.clone();
        let pending_email = pending_email.clone();
        let pending_phone = pending_phone.clone();
        let navigator = navigator.clone();
        Callback::from(move |_: MouseEvent| {
            let code = (*totp_code_input).clone();
            if code.is_empty() {
                return;
            }

            let two_factor_state = two_factor_state.clone();
            let pending_change_type = pending_change_type.clone();
            let email_original = email_original.clone();
            let phone_number_original = phone_number_original.clone();
            let email_save_state = email_save_state.clone();
            let phone_save_state = phone_save_state.clone();
            let user_profile = user_profile.clone();
            let on_profile_update = on_profile_update.clone();
            let pending_email = pending_email.clone();
            let pending_phone = pending_phone.clone();
            let navigator = navigator.clone();
            let totp_code_input = totp_code_input.clone();

            two_factor_state.set(TwoFactorVerifyState::Verifying);

            spawn_local(async move {
                if let Some(change_type) = (*pending_change_type).clone() {
                    match change_type {
                        SensitiveChangeType::Email(new_email) => {
                            perform_profile_update_email(
                                new_email,
                                Some(code),
                                None,
                                email_original,
                                email_save_state,
                                user_profile,
                                on_profile_update,
                                pending_email,
                            ).await;
                            two_factor_state.set(TwoFactorVerifyState::Hidden);
                            totp_code_input.set(String::new());
                        }
                        SensitiveChangeType::Phone(new_phone) => {
                            perform_profile_update_phone(
                                new_phone,
                                Some(code),
                                None,
                                phone_number_original,
                                phone_save_state,
                                user_profile,
                                on_profile_update,
                                pending_phone,
                                navigator,
                            ).await;
                            two_factor_state.set(TwoFactorVerifyState::Hidden);
                            totp_code_input.set(String::new());
                        }
                    }
                }
            });
        })
    };

    // Timezone detector callback
    let on_timezone_update = {
        let timezone = timezone.clone();
        let user_profile = user_profile.clone();
        let on_profile_update = props.on_profile_update.clone();
        let timezone_auto = timezone_auto.clone();
        Callback::from(move |new_timezone: String| {
            if *timezone_auto {
                timezone.set(new_timezone.clone());
                let mut updated_profile = (*user_profile).clone();
                updated_profile.timezone = Some(new_timezone.clone());
                updated_profile.timezone_auto = Some(*timezone_auto);
                user_profile.set(updated_profile.clone());
                on_profile_update.emit(updated_profile);
            }
        })
    };

    // Fill nearby places from location and auto-save
    let on_fill_nearby_places = {
        let location = location.clone();
        let nearby_places = nearby_places.clone();
        let nearby_places_original = nearby_places_original.clone();
        let save_state = nearby_places_save_state.clone();
        let user_profile = user_profile.clone();
        let on_profile_update = props.on_profile_update.clone();
        Callback::from(move |_e: MouseEvent| {
            let loc = (*location).clone();
            let nearby_places = nearby_places.clone();
            let nearby_places_original = nearby_places_original.clone();
            let save_state = save_state.clone();
            let user_profile = user_profile.clone();
            let on_profile_update = on_profile_update.clone();
            if loc.is_empty() {
                return;
            }
            spawn_local(async move {
                let encoded_loc = encode_uri_component(&loc).to_string();
                match Api::get(&format!("/api/profile/get_nearby_places?location={}", encoded_loc))
                    .send()
                    .await
                {
                    Ok(response) if response.ok() => {
                        if let Ok(json) = response.json::<Vec<String>>().await {
                            let new_val = json.join(", ");
                            nearby_places.set(new_val.clone());

                            // Auto-save the filled value
                            save_state.set(FieldSaveState::Saving);
                            let request = PatchFieldRequest {
                                field: "nearby_places".to_string(),
                                value: serde_json::Value::String(new_val.clone())
                            };
                            match Api::patch("/api/profile/field")
                                .json(&request)
                                .unwrap()
                                .send()
                                .await
                            {
                                Ok(resp) if resp.ok() => {
                                    nearby_places_original.set(new_val.clone());
                                    let mut profile = (*user_profile).clone();
                                    profile.nearby_places = Some(new_val);
                                    on_profile_update.emit(profile);
                                    save_state.set(FieldSaveState::Success);
                                    let save_state_clone = save_state.clone();
                                    spawn_local(async move {
                                        gloo_timers::future::TimeoutFuture::new(2000).await;
                                        save_state_clone.set(FieldSaveState::Idle);
                                    });
                                }
                                Ok(_) => {
                                    save_state.set(FieldSaveState::Error("Failed to save".to_string()));
                                }
                                Err(_) => {
                                    save_state.set(FieldSaveState::Error("Network error".to_string()));
                                }
                            }
                        }
                    }
                    _ => {}
                }
            });
        })
    };

    // Render save indicator
    fn render_save_indicator(state: &FieldSaveState) -> Html {
        match state {
            FieldSaveState::Idle => html! {},
            FieldSaveState::Saving => html! {
                <span class="save-indicator">
                    <span class="save-spinner"></span>
                </span>
            },
            FieldSaveState::Success => html! {
                <span class="save-indicator save-success">{"✓"}</span>
            },
            FieldSaveState::Error(msg) => html! {
                <span class="save-indicator save-error" title={msg.clone()}>{"✗"}</span>
            },
        }
    }

    html! {
        <>
        <div class="profile-info">
            <TimezoneDetector on_timezone_update={on_timezone_update} />

            // Email confirmation dialog
            {
                if *show_email_confirm {
                    html! {
                        <div class="confirm-dialog-overlay">
                            <div class="confirm-dialog">
                                <h3>{"Confirm Email Change"}</h3>
                                <p>{"Changing your email will update your login credentials. Continue?"}</p>
                                <div class="confirm-dialog-buttons">
                                    <button class="confirm-btn cancel" onclick={on_email_cancel.clone()}>{"Cancel"}</button>
                                    <button class="confirm-btn confirm" onclick={on_email_confirm.clone()}>{"Confirm"}</button>
                                </div>
                            </div>
                        </div>
                    }
                } else {
                    html! {}
                }
            }

            // Phone confirmation dialog
            {
                if *show_phone_confirm {
                    html! {
                        <div class="confirm-dialog-overlay">
                            <div class="confirm-dialog">
                                <h3>{"Confirm Phone Change"}</h3>
                                <p>{"Changing your phone number will require re-verification. You'll be redirected to verify the new number."}</p>
                                <div class="confirm-dialog-buttons">
                                    <button class="confirm-btn cancel" onclick={on_phone_cancel.clone()}>{"Cancel"}</button>
                                    <button class="confirm-btn confirm" onclick={on_phone_confirm.clone()}>{"Confirm"}</button>
                                </div>
                            </div>
                        </div>
                    }
                } else {
                    html! {}
                }
            }

            // 2FA verification modal
            {
                match &*two_factor_state {
                    TwoFactorVerifyState::Hidden => html! {},
                    TwoFactorVerifyState::Loading => html! {
                        <div class="confirm-dialog-overlay">
                            <div class="confirm-dialog">
                                <h3>{"Checking security requirements..."}</h3>
                                <div class="loading-spinner"></div>
                            </div>
                        </div>
                    },
                    TwoFactorVerifyState::ShowingOptions { has_passkeys, has_totp, .. } => html! {
                        <div class="confirm-dialog-overlay">
                            <div class="confirm-dialog">
                                <h3>{"Verify Your Identity"}</h3>
                                <p>{"This change requires verification. Choose your method:"}</p>
                                <div class="two-factor-options">
                                    {
                                        if *has_passkeys {
                                            html! {
                                                <button
                                                    class="confirm-btn confirm two-factor-btn"
                                                    onclick={on_use_passkey.clone()}
                                                >
                                                    {"Use Passkey"}
                                                </button>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }
                                    {
                                        if *has_totp {
                                            html! {
                                                <button
                                                    class="confirm-btn confirm two-factor-btn"
                                                    onclick={on_use_totp.clone()}
                                                >
                                                    {"Use Authenticator Code"}
                                                </button>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }
                                </div>
                                <div class="confirm-dialog-buttons">
                                    <button class="confirm-btn cancel" onclick={on_2fa_cancel.clone()}>{"Cancel"}</button>
                                </div>
                            </div>
                        </div>
                    },
                    TwoFactorVerifyState::WaitingForPasskey => html! {
                        <div class="confirm-dialog-overlay">
                            <div class="confirm-dialog">
                                <h3>{"Passkey Verification"}</h3>
                                <p>{"Please use your passkey to verify..."}</p>
                                <div class="loading-spinner"></div>
                                <div class="confirm-dialog-buttons">
                                    <button class="confirm-btn cancel" onclick={on_2fa_cancel.clone()}>{"Cancel"}</button>
                                </div>
                            </div>
                        </div>
                    },
                    TwoFactorVerifyState::WaitingForTotp => {
                        let totp_code_input_clone = totp_code_input.clone();
                        html! {
                            <div class="confirm-dialog-overlay">
                                <div class="confirm-dialog">
                                    <h3>{"Enter Verification Code"}</h3>
                                    <p>{"Enter the 6-digit code from your authenticator app:"}</p>
                                    <input
                                        type="text"
                                        class="profile-input totp-input"
                                        placeholder="000000"
                                        maxlength="6"
                                        value={(*totp_code_input).clone()}
                                        oninput={move |e: InputEvent| {
                                            let input: HtmlInputElement = e.target_unchecked_into();
                                            totp_code_input_clone.set(input.value());
                                        }}
                                    />
                                    <div class="confirm-dialog-buttons">
                                        <button class="confirm-btn cancel" onclick={on_2fa_cancel.clone()}>{"Cancel"}</button>
                                        <button class="confirm-btn confirm" onclick={on_submit_totp.clone()}>{"Verify"}</button>
                                    </div>
                                </div>
                            </div>
                        }
                    },
                    TwoFactorVerifyState::Verifying => html! {
                        <div class="confirm-dialog-overlay">
                            <div class="confirm-dialog">
                                <h3>{"Verifying..."}</h3>
                                <div class="loading-spinner"></div>
                            </div>
                        </div>
                    },
                    TwoFactorVerifyState::Error(msg) => html! {
                        <div class="confirm-dialog-overlay">
                            <div class="confirm-dialog">
                                <h3>{"Verification Failed"}</h3>
                                <p class="error-message">{msg}</p>
                                <div class="confirm-dialog-buttons">
                                    <button class="confirm-btn cancel" onclick={on_2fa_cancel.clone()}>{"Close"}</button>
                                </div>
                            </div>
                        </div>
                    },
                }
            }

            // Email field (hidden for self-hosted)
            {
                if (*user_profile).sub_tier != Some("self_hosted".to_string()) {
                    html! {
                        <div class="profile-field">
                            <span class="field-label">{"Email"}</span>
                            <div class="field-input-container">
                                <input
                                    type="email"
                                    class="profile-input"
                                    value={(*email).clone()}
                                    placeholder="your@email.com"
                                    oninput={let email = email.clone(); move |e: InputEvent| {
                                        let input: HtmlInputElement = e.target_unchecked_into();
                                        email.set(input.value());
                                    }}
                                    onblur={on_email_blur.clone()}
                                    onkeypress={on_email_keypress.clone()}
                                />
                                {render_save_indicator(&*email_save_state)}
                            </div>
                        </div>
                    }
                } else {
                    html! {}
                }
            }

            // Phone field
            <div class="profile-field">
                <span class="field-label">{"Phone"}</span>
                <div class="field-input-container">
                    <input
                        type="tel"
                        class="profile-input"
                        value={(*phone_number).clone()}
                        placeholder="+1234567890"
                        oninput={let phone_number = phone_number.clone(); move |e: InputEvent| {
                            let input: HtmlInputElement = e.target_unchecked_into();
                            phone_number.set(input.value());
                        }}
                        onblur={on_phone_blur.clone()}
                        onkeypress={on_phone_keypress.clone()}
                    />
                    {render_save_indicator(&*phone_save_state)}
                </div>
            </div>

            // Phone Service Active toggle
            <div class="profile-field">
                <div class="field-label-group">
                    <span class="field-label">{"Phone Service"}</span>
                    <div class="tooltip">
                        <span class="tooltip-icon">{"?"}</span>
                        <span class="tooltip-text">
                            {"Disable this if your phone is lost or stolen to prevent any SMS or calls from being processed."}
                        </span>
                    </div>
                </div>
                <div class="phone-service-toggle">
                    <label class={classes!(
                        "custom-checkbox",
                        if !*phone_service_active { "service-deactivated" } else { "" }
                    )}>
                        <input
                            type="checkbox"
                            checked={*phone_service_active}
                            onchange={on_phone_service_toggle.clone()}
                        />
                        <span class="checkmark"></span>
                        {if *phone_service_active { "Service active" } else { "Service DEACTIVATED" }}
                    </label>
                    {render_save_indicator(&*phone_service_active_save_state)}
                </div>
                {if !*phone_service_active {
                    html! {
                        <div class="warning-message">
                            {"All SMS and calls to this number are currently blocked."}
                        </div>
                    }
                } else {
                    html! {}
                }}
            </div>

            // Nickname field
            <div class="profile-field">
                <div class="field-label-group">
                    <span class="field-label">{"Nickname"}</span>
                    <div class="tooltip">
                        <span class="tooltip-icon">{"?"}</span>
                        <span class="tooltip-text">
                            {"This is how the AI assistant will address you in conversations."}
                        </span>
                    </div>
                </div>
                <div class="field-input-container">
                    <div class="input-with-limit">
                        <input
                            type="text"
                            class="profile-input"
                            value={(*nickname).clone()}
                            maxlength={MAX_NICKNAME_LENGTH.to_string()}
                            oninput={let nickname = nickname.clone(); move |e: InputEvent| {
                                let input: HtmlInputElement = e.target_unchecked_into();
                                let value = input.value();
                                if value.chars().count() <= MAX_NICKNAME_LENGTH {
                                    nickname.set(value);
                                }
                            }}
                            onblur={on_nickname_blur.clone()}
                            onkeypress={on_nickname_keypress.clone()}
                        />
                        <span class="char-count">
                            {format!("{}/{}", (*nickname).chars().count(), MAX_NICKNAME_LENGTH)}
                        </span>
                    </div>
                    {render_save_indicator(&*nickname_save_state)}
                </div>
            </div>

            // Info field
            <div class="profile-field">
                <div class="field-label-group">
                    <span class="field-label">{"Info"}</span>
                    <div class="tooltip">
                        <span class="tooltip-icon">{"?"}</span>
                        <span class="tooltip-text">
                            {"What would you like the AI assistant to know about you?"}
                        </span>
                    </div>
                </div>
                <div class="field-input-container">
                    <div class="input-with-limit">
                        <textarea
                            class="profile-input"
                            value={(*info).clone()}
                            maxlength={MAX_INFO_LENGTH.to_string()}
                            placeholder="Tell something about yourself"
                            oninput={let info = info.clone(); move |e: InputEvent| {
                                let input: HtmlInputElement = e.target_unchecked_into();
                                let value = input.value();
                                if value.chars().count() <= MAX_INFO_LENGTH {
                                    info.set(value);
                                }
                            }}
                            onblur={on_info_blur.clone()}
                            onkeypress={on_info_keypress.clone()}
                        />
                        <span class="char-count">
                            {format!("{}/{}", (*info).chars().count(), MAX_INFO_LENGTH)}
                        </span>
                    </div>
                    {render_save_indicator(&*info_save_state)}
                </div>
            </div>

            // Location field
            <div class="profile-field">
                <div class="field-label-group">
                    <span class="field-label">{"Location"}</span>
                    <div class="tooltip">
                        <span class="tooltip-icon">{"?"}</span>
                        <span class="tooltip-text">
                            {"Enter your location as District, City, Country."}
                        </span>
                    </div>
                </div>
                <div class="field-input-container">
                    <input
                        type="text"
                        class="profile-input"
                        value={(*location).clone()}
                        placeholder="District, City, Country"
                        oninput={let location = location.clone(); move |e: InputEvent| {
                            let input: HtmlInputElement = e.target_unchecked_into();
                            location.set(input.value());
                        }}
                        onblur={on_location_blur.clone()}
                        onkeypress={on_location_keypress.clone()}
                    />
                    {render_save_indicator(&*location_save_state)}
                </div>
            </div>

            // Nearby Places field
            <div class="profile-field">
                <div class="field-label-group">
                    <span class="field-label">{"Nearby Places"}</span>
                    <div class="tooltip">
                        <span class="tooltip-icon">{"?"}</span>
                        <span class="tooltip-text">
                            {"Comma-separated list of nearby places to improve voice AI transcription accuracy."}
                        </span>
                    </div>
                </div>
                <div class="field-input-container nearby-places-row">
                    <div class="nearby-places-container">
                        <textarea
                            class="profile-input"
                            value={(*nearby_places).clone()}
                            placeholder="Comma-separated places"
                            oninput={let nearby_places = nearby_places.clone(); move |e: InputEvent| {
                                let input: HtmlInputElement = e.target_unchecked_into();
                                nearby_places.set(input.value());
                            }}
                            onblur={on_nearby_places_blur.clone()}
                            onkeypress={on_nearby_places_keypress.clone()}
                        />
                        <button class="fill-button" onclick={on_fill_nearby_places.clone()}>
                            {"Fill from Location"}
                        </button>
                    </div>
                    {render_save_indicator(&*nearby_places_save_state)}
                </div>
            </div>

            // Timezone field
            <div class="profile-field">
                <div class="field-label-group">
                    <span class="field-label">{"Timezone"}</span>
                    <div class="tooltip">
                        <span class="tooltip-icon">{"?"}</span>
                        <span class="tooltip-text">
                            {"Choose your timezone for time-sensitive responses."}
                        </span>
                    </div>
                </div>
                <div class="timezone-section">
                    <div class="timezone-auto-checkbox">
                        <label class="custom-checkbox">
                            <input
                                type="checkbox"
                                checked={*timezone_auto}
                                onchange={on_timezone_auto_change.clone()}
                            />
                            <span class="checkmark"></span>
                            {"Automatically detect timezone"}
                        </label>
                        {render_save_indicator(&*timezone_auto_save_state)}
                    </div>
                    <div class="field-input-container">
                        <select
                            class="profile-input"
                            value={(*timezone).clone()}
                            disabled={*timezone_auto}
                            onchange={on_timezone_change.clone()}
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
                        {render_save_indicator(&*timezone_save_state)}
                    </div>
                </div>
            </div>

            // Agent Language field
            <div class="profile-field">
                <div class="field-label-group">
                    <span class="field-label">{"Agent Language"}</span>
                    <div class="tooltip">
                        <span class="tooltip-icon">{"?"}</span>
                        <span class="tooltip-text">
                            {"Choose the language the AI assistant will use for voice calls."}
                        </span>
                    </div>
                </div>
                <div class="field-input-container">
                    <select
                        class="profile-input"
                        value={(*agent_language).clone()}
                        onchange={on_agent_language_change.clone()}
                    >
                        <option value="en" selected={*agent_language == "en"}>{"English"}</option>
                        <option value="fi" selected={*agent_language == "fi"}>{"Finnish"}</option>
                        <option value="de" selected={*agent_language == "de"}>{"German"}</option>
                    </select>
                    {render_save_indicator(&*agent_language_save_state)}
                </div>
            </div>

            // LLM Provider field (only show to subscribers, not self-hosted)
            {
                if (*user_profile).sub_tier.is_some() && (*user_profile).sub_tier != Some("self_hosted".to_string()) {
                    html! {
                        <div class="profile-field">
                            <div class="field-label-group">
                                <span class="field-label">{"AI Provider"}</span>
                                <div class="tooltip">
                                    <span class="tooltip-icon">{"?"}</span>
                                    <span class="tooltip-text">
                                        {"Choose which AI provider to use for SMS and chat messages. OpenAI is faster and smarter. Tinfoil is privacy-focused but slower."}
                                    </span>
                                </div>
                            </div>
                            <div class="field-input-container">
                                <select
                                    class="profile-input"
                                    value={(*llm_provider).clone()}
                                    onchange={on_llm_provider_change.clone()}
                                >
                                    <option value="openai" selected={*llm_provider == "openai"}>{"OpenAI (faster, smarter)"}</option>
                                    <option value="tinfoil" selected={*llm_provider == "tinfoil"}>{"Tinfoil (privacy-focused)"}</option>
                                </select>
                                {render_save_indicator(&*llm_provider_save_state)}
                            </div>
                        </div>
                    }
                } else {
                    html! {}
                }
            }

            // Notification Type field
            <div class="profile-field">
                <div class="field-label-group">
                    <span class="field-label">{"Notification Type"}</span>
                    <div class="tooltip">
                        <span class="tooltip-icon">{"?"}</span>
                        <span class="tooltip-text">
                            {"Choose how you want to receive notifications. Call+SMS rings your phone as a loud alert, plus sends SMS with details. Only charged for call if answered."}
                        </span>
                    </div>
                </div>
                <div class="field-input-container">
                    <select
                        class="profile-input"
                        value={(*notification_type).clone().unwrap_or_else(|| "sms".to_string())}
                        onchange={on_notification_type_change.clone()}
                    >
                        <option value="sms" selected={(*notification_type).as_deref().unwrap_or("sms") == "sms"}>
                            {"Text me"}
                        </option>
                        <option value="call" selected={(*notification_type).as_deref() == Some("call")}>
                            {"Call me"}
                        </option>
                        <option value="call_sms" selected={(*notification_type).as_deref() == Some("call_sms")}>
                            {"Call + SMS (loud alert)"}
                        </option>
                    </select>
                    {render_save_indicator(&*notification_type_save_state)}
                </div>
            </div>

            // Conversation History field
            <div class="profile-field">
                <div class="field-label-group">
                    <span class="field-label">{"Conversation History"}</span>
                    <div class="tooltip">
                        <span class="tooltip-icon">{"?"}</span>
                        <span class="tooltip-text">
                            {"Choose how many messages Lightfriend remembers in SMS conversations."}
                        </span>
                    </div>
                </div>
                <div class="field-input-container">
                    <select
                        class="profile-input"
                        value={(*save_context).to_string()}
                        onchange={on_save_context_change.clone()}
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
                    {render_save_indicator(&*save_context_save_state)}
                </div>
            </div>

            // Security section (2FA)
            <SecuritySettings />
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
textarea.profile-input {
    height: 100px;
    resize: vertical;
}
.field-input-container {
    position: relative;
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
}
.field-input-container .input-with-limit {
    flex: 1;
}
.save-indicator {
    min-width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
}
.save-spinner {
    width: 16px;
    height: 16px;
    border: 2px solid rgba(30, 144, 255, 0.3);
    border-top-color: #1E90FF;
    border-radius: 50%;
    animation: spin 1s linear infinite;
}
@keyframes spin {
    to { transform: rotate(360deg); }
}
.save-success {
    color: #22C55E;
    font-size: 18px;
}
.save-error {
    color: #EF4444;
    cursor: help;
    font-size: 18px;
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
.timezone-section {
    display: flex;
    flex-direction: column;
    gap: 12px;
    width: 100%;
}
.timezone-auto-checkbox {
    display: flex;
    align-items: center;
    gap: 8px;
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
.profile-input:disabled {
    opacity: 0.5;
    cursor: not-allowed;
}
.nearby-places-container {
    display: flex;
    flex-direction: column;
    gap: 8px;
    flex: 1;
}
.nearby-places-row {
    align-items: flex-start;
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
.confirm-dialog-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
}
.confirm-dialog {
    background: #1a1a2e;
    border-radius: 12px;
    padding: 24px;
    max-width: 400px;
    width: 90%;
    border: 1px solid rgba(30, 144, 255, 0.3);
}
.confirm-dialog h3 {
    margin: 0 0 16px 0;
    color: #ffffff;
}
.confirm-dialog p {
    margin: 0 0 24px 0;
    color: rgba(255, 255, 255, 0.8);
    line-height: 1.5;
}
.confirm-dialog-buttons {
    display: flex;
    gap: 12px;
    justify-content: flex-end;
}
.confirm-btn {
    padding: 10px 20px;
    border-radius: 8px;
    border: none;
    cursor: pointer;
    font-size: 1rem;
    transition: all 0.3s ease;
}
.confirm-btn.cancel {
    background: rgba(255, 255, 255, 0.1);
    color: #ffffff;
}
.confirm-btn.cancel:hover {
    background: rgba(255, 255, 255, 0.2);
}
.confirm-btn.confirm {
    background: linear-gradient(45deg, #1E90FF, #4169E1);
    color: white;
}
.confirm-btn.confirm:hover {
    transform: translateY(-2px);
    box-shadow: 0 4px 20px rgba(30, 144, 255, 0.3);
}
.phone-service-toggle {
    display: flex;
    align-items: center;
    gap: 12px;
}
.phone-service-toggle .custom-checkbox.service-deactivated {
    color: #ff6b6b;
}
.phone-service-toggle .custom-checkbox.service-deactivated .checkmark {
    border-color: #ff6b6b;
}
.warning-message {
    margin-top: 8px;
    padding: 10px 12px;
    background: rgba(255, 107, 107, 0.15);
    border: 1px solid rgba(255, 107, 107, 0.4);
    border-radius: 6px;
    color: #ff6b6b;
    font-size: 0.9rem;
}
            "#}
        </style>
        </>
    }
}
