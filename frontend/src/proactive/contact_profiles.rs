use yew::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlInputElement, HtmlSelectElement, Event};
use serde::{Deserialize, Serialize};
use crate::utils::api::Api;
use gloo_timers::future::TimeoutFuture;

#[derive(Clone, PartialEq)]
pub enum FieldSaveState {
    Idle,
    Saving,
    Success,
    Error(String),
}

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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ProfileException {
    pub platform: String,
    pub notification_mode: String,
    pub notification_type: String,
    pub notify_on_call: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ContactProfile {
    pub id: i32,
    pub nickname: String,
    pub whatsapp_chat: Option<String>,
    pub telegram_chat: Option<String>,
    pub signal_chat: Option<String>,
    pub email_addresses: Option<String>,
    pub notification_mode: String,
    pub notification_type: String,
    pub notify_on_call: bool,
    #[serde(default)]
    pub exceptions: Vec<ProfileException>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ContactProfilesResponse {
    pub profiles: Vec<ContactProfile>,
    pub default_mode: String,
    #[serde(default = "default_noti_type")]
    pub default_noti_type: String,
    #[serde(default = "default_notify_call")]
    pub default_notify_on_call: bool,
}

fn default_noti_type() -> String {
    "sms".to_string()
}

fn default_notify_call() -> bool {
    true
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Room {
    pub display_name: String,
    pub last_activity_formatted: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SearchResponse {
    pub results: Vec<Room>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExceptionRequest {
    pub platform: String,
    pub notification_mode: String,
    pub notification_type: String,
    pub notify_on_call: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateProfileRequest {
    pub nickname: String,
    pub whatsapp_chat: Option<String>,
    pub telegram_chat: Option<String>,
    pub signal_chat: Option<String>,
    pub email_addresses: Option<String>,
    pub notification_mode: String,
    pub notification_type: String,
    pub notify_on_call: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exceptions: Option<Vec<ExceptionRequest>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateDefaultModeRequest {
    pub mode: Option<String>,
    pub noti_type: Option<String>,
    pub notify_on_call: Option<bool>,
}

#[derive(Properties, PartialEq, Clone)]
pub struct ContactProfilesProps {
    #[prop_or(false)]
    pub critical_disabled: bool,
}

#[function_component(ContactProfilesSection)]
pub fn contact_profiles_section(props: &ContactProfilesProps) -> Html {
    let profiles = use_state(|| Vec::<ContactProfile>::new());
    let default_mode = use_state(|| "critical".to_string());
    let default_noti_type = use_state(|| "sms".to_string());
    let default_notify_on_call = use_state(|| true);
    let show_mode_info = use_state(|| false);
    let loading = use_state(|| true);
    let error_message = use_state(|| None::<String>);
    let show_modal = use_state(|| false);
    let editing_profile = use_state(|| None::<ContactProfile>);

    // Save states for various operations
    let default_save_state = use_state(|| FieldSaveState::Idle);
    let profile_save_state = use_state(|| FieldSaveState::Idle);

    // Modal form state
    let form_nickname = use_state(|| String::new());
    let form_whatsapp = use_state(|| String::new());
    let form_telegram = use_state(|| String::new());
    let form_signal = use_state(|| String::new());
    let form_email = use_state(|| String::new());
    let form_mode = use_state(|| "critical".to_string());
    let form_type = use_state(|| "sms".to_string());
    let form_notify_call = use_state(|| true);

    // Exception form state - which platforms have exceptions expanded
    let show_whatsapp_exception = use_state(|| false);
    let show_telegram_exception = use_state(|| false);
    let show_signal_exception = use_state(|| false);
    let show_email_exception = use_state(|| false);

    // Exception settings per platform (None = use profile default)
    let exc_whatsapp_mode = use_state(|| None::<String>);
    let exc_whatsapp_type = use_state(|| "sms".to_string());
    let exc_whatsapp_call = use_state(|| true);

    let exc_telegram_mode = use_state(|| None::<String>);
    let exc_telegram_type = use_state(|| "sms".to_string());
    let exc_telegram_call = use_state(|| true);

    let exc_signal_mode = use_state(|| None::<String>);
    let exc_signal_type = use_state(|| "sms".to_string());
    let exc_signal_call = use_state(|| true);

    let exc_email_mode = use_state(|| None::<String>);
    let exc_email_type = use_state(|| "sms".to_string());
    let exc_email_call = use_state(|| true);

    // Search state
    let whatsapp_results = use_state(|| Vec::<Room>::new());
    let telegram_results = use_state(|| Vec::<Room>::new());
    let signal_results = use_state(|| Vec::<Room>::new());
    let searching_whatsapp = use_state(|| false);
    let searching_telegram = use_state(|| false);
    let searching_signal = use_state(|| false);
    let show_whatsapp_suggestions = use_state(|| false);
    let show_telegram_suggestions = use_state(|| false);
    let show_signal_suggestions = use_state(|| false);
    let search_error_whatsapp = use_state(|| None::<String>);
    let search_error_telegram = use_state(|| None::<String>);
    let search_error_signal = use_state(|| None::<String>);

    // Load profiles on mount
    {
        let profiles = profiles.clone();
        let default_mode = default_mode.clone();
        let default_noti_type = default_noti_type.clone();
        let default_notify_on_call = default_notify_on_call.clone();
        let loading = loading.clone();
        let error_message = error_message.clone();

        use_effect_with_deps(move |_| {
            spawn_local(async move {
                if let Ok(response) = Api::get("/api/contact-profiles").send().await {
                    if let Ok(data) = response.json::<ContactProfilesResponse>().await {
                        profiles.set(data.profiles);
                        default_mode.set(data.default_mode);
                        default_noti_type.set(data.default_noti_type);
                        default_notify_on_call.set(data.default_notify_on_call);
                    } else {
                        error_message.set(Some("Failed to parse profiles".to_string()));
                    }
                } else {
                    error_message.set(Some("Failed to load profiles".to_string()));
                }
                loading.set(false);
            });
            || ()
        }, ());
    }

    // Search function for chats
    let search_chats = {
        let whatsapp_results = whatsapp_results.clone();
        let telegram_results = telegram_results.clone();
        let signal_results = signal_results.clone();
        let searching_whatsapp = searching_whatsapp.clone();
        let searching_telegram = searching_telegram.clone();
        let searching_signal = searching_signal.clone();
        let show_whatsapp_suggestions = show_whatsapp_suggestions.clone();
        let show_telegram_suggestions = show_telegram_suggestions.clone();
        let show_signal_suggestions = show_signal_suggestions.clone();
        let search_error_whatsapp = search_error_whatsapp.clone();
        let search_error_telegram = search_error_telegram.clone();
        let search_error_signal = search_error_signal.clone();

        Callback::from(move |(service, query): (String, String)| {
            if query.trim().len() < 2 {
                match service.as_str() {
                    "whatsapp" => { whatsapp_results.set(vec![]); show_whatsapp_suggestions.set(false); searching_whatsapp.set(false); search_error_whatsapp.set(None); }
                    "telegram" => { telegram_results.set(vec![]); show_telegram_suggestions.set(false); searching_telegram.set(false); search_error_telegram.set(None); }
                    "signal" => { signal_results.set(vec![]); show_signal_suggestions.set(false); searching_signal.set(false); search_error_signal.set(None); }
                    _ => {}
                }
                return;
            }

            let whatsapp_results = whatsapp_results.clone();
            let telegram_results = telegram_results.clone();
            let signal_results = signal_results.clone();
            let searching_whatsapp = searching_whatsapp.clone();
            let searching_telegram = searching_telegram.clone();
            let searching_signal = searching_signal.clone();
            let show_whatsapp_suggestions = show_whatsapp_suggestions.clone();
            let show_telegram_suggestions = show_telegram_suggestions.clone();
            let show_signal_suggestions = show_signal_suggestions.clone();
            let search_error_whatsapp = search_error_whatsapp.clone();
            let search_error_telegram = search_error_telegram.clone();
            let search_error_signal = search_error_signal.clone();
            let service = service.clone();
            let query = query.clone();

            // Set searching state immediately
            match service.as_str() {
                "whatsapp" => { searching_whatsapp.set(true); show_whatsapp_suggestions.set(true); search_error_whatsapp.set(None); }
                "telegram" => { searching_telegram.set(true); show_telegram_suggestions.set(true); search_error_telegram.set(None); }
                "signal" => { searching_signal.set(true); show_signal_suggestions.set(true); search_error_signal.set(None); }
                _ => {}
            }

            spawn_local(async move {
                let encoded_query = js_sys::encode_uri_component(&query);
                let url = format!("/api/contact-profiles/search/{}?q={}", service, encoded_query);
                match Api::get(&url).send().await {
                    Ok(response) => {
                        if response.ok() {
                            if let Ok(data) = response.json::<SearchResponse>().await {
                                match service.as_str() {
                                    "whatsapp" => {
                                        whatsapp_results.set(data.results);
                                        searching_whatsapp.set(false);
                                    }
                                    "telegram" => {
                                        telegram_results.set(data.results);
                                        searching_telegram.set(false);
                                    }
                                    "signal" => {
                                        signal_results.set(data.results);
                                        searching_signal.set(false);
                                    }
                                    _ => {}
                                }
                            }
                        } else {
                            // Try to get error message from response
                            let error_msg = if let Ok(text) = response.text().await {
                                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                                    json.get("error").and_then(|e| e.as_str()).unwrap_or("Search failed").to_string()
                                } else {
                                    "Search failed".to_string()
                                }
                            } else {
                                "Search failed".to_string()
                            };
                            match service.as_str() {
                                "whatsapp" => { searching_whatsapp.set(false); search_error_whatsapp.set(Some(error_msg)); }
                                "telegram" => { searching_telegram.set(false); search_error_telegram.set(Some(error_msg)); }
                                "signal" => { searching_signal.set(false); search_error_signal.set(Some(error_msg)); }
                                _ => {}
                            }
                        }
                    }
                    Err(_) => {
                        match service.as_str() {
                            "whatsapp" => { searching_whatsapp.set(false); search_error_whatsapp.set(Some("Network error".to_string())); }
                            "telegram" => { searching_telegram.set(false); search_error_telegram.set(Some("Network error".to_string())); }
                            "signal" => { searching_signal.set(false); search_error_signal.set(Some("Network error".to_string())); }
                            _ => {}
                        }
                    }
                }
            });
        })
    };

    // Open modal for new profile
    let open_new_modal = {
        let show_modal = show_modal.clone();
        let editing_profile = editing_profile.clone();
        let profile_save_state = profile_save_state.clone();
        let form_nickname = form_nickname.clone();
        let form_whatsapp = form_whatsapp.clone();
        let form_telegram = form_telegram.clone();
        let form_signal = form_signal.clone();
        let form_email = form_email.clone();
        let form_mode = form_mode.clone();
        let form_type = form_type.clone();
        let form_notify_call = form_notify_call.clone();
        // Exception state
        let show_whatsapp_exception = show_whatsapp_exception.clone();
        let show_telegram_exception = show_telegram_exception.clone();
        let show_signal_exception = show_signal_exception.clone();
        let show_email_exception = show_email_exception.clone();
        let exc_whatsapp_mode = exc_whatsapp_mode.clone();
        let exc_telegram_mode = exc_telegram_mode.clone();
        let exc_signal_mode = exc_signal_mode.clone();
        let exc_email_mode = exc_email_mode.clone();

        Callback::from(move |_| {
            editing_profile.set(None);
            profile_save_state.set(FieldSaveState::Idle);
            form_nickname.set(String::new());
            form_whatsapp.set(String::new());
            form_telegram.set(String::new());
            form_signal.set(String::new());
            form_email.set(String::new());
            form_mode.set("critical".to_string());
            form_type.set("sms".to_string());
            form_notify_call.set(true);
            // Reset exception state
            show_whatsapp_exception.set(false);
            show_telegram_exception.set(false);
            show_signal_exception.set(false);
            show_email_exception.set(false);
            exc_whatsapp_mode.set(None);
            exc_telegram_mode.set(None);
            exc_signal_mode.set(None);
            exc_email_mode.set(None);
            show_modal.set(true);
        })
    };

    // Open modal for editing
    let open_edit_modal = {
        let show_modal = show_modal.clone();
        let editing_profile = editing_profile.clone();
        let profile_save_state = profile_save_state.clone();
        let form_nickname = form_nickname.clone();
        let form_whatsapp = form_whatsapp.clone();
        let form_telegram = form_telegram.clone();
        let form_signal = form_signal.clone();
        let form_email = form_email.clone();
        let form_mode = form_mode.clone();
        let form_type = form_type.clone();
        let form_notify_call = form_notify_call.clone();
        // Exception state
        let show_whatsapp_exception = show_whatsapp_exception.clone();
        let show_telegram_exception = show_telegram_exception.clone();
        let show_signal_exception = show_signal_exception.clone();
        let show_email_exception = show_email_exception.clone();
        let exc_whatsapp_mode = exc_whatsapp_mode.clone();
        let exc_whatsapp_type = exc_whatsapp_type.clone();
        let exc_whatsapp_call = exc_whatsapp_call.clone();
        let exc_telegram_mode = exc_telegram_mode.clone();
        let exc_telegram_type = exc_telegram_type.clone();
        let exc_telegram_call = exc_telegram_call.clone();
        let exc_signal_mode = exc_signal_mode.clone();
        let exc_signal_type = exc_signal_type.clone();
        let exc_signal_call = exc_signal_call.clone();
        let exc_email_mode = exc_email_mode.clone();
        let exc_email_type = exc_email_type.clone();
        let exc_email_call = exc_email_call.clone();

        Callback::from(move |profile: ContactProfile| {
            profile_save_state.set(FieldSaveState::Idle);
            form_nickname.set(profile.nickname.clone());
            form_whatsapp.set(profile.whatsapp_chat.clone().unwrap_or_default());
            form_telegram.set(profile.telegram_chat.clone().unwrap_or_default());
            form_signal.set(profile.signal_chat.clone().unwrap_or_default());
            form_email.set(profile.email_addresses.clone().unwrap_or_default());
            form_mode.set(profile.notification_mode.clone());
            form_type.set(profile.notification_type.clone());
            form_notify_call.set(profile.notify_on_call);

            // Load exceptions from profile
            // Reset first
            show_whatsapp_exception.set(false);
            show_telegram_exception.set(false);
            show_signal_exception.set(false);
            show_email_exception.set(false);
            exc_whatsapp_mode.set(None);
            exc_telegram_mode.set(None);
            exc_signal_mode.set(None);
            exc_email_mode.set(None);

            // Load existing exceptions
            for exc in &profile.exceptions {
                match exc.platform.as_str() {
                    "whatsapp" => {
                        show_whatsapp_exception.set(true);
                        exc_whatsapp_mode.set(Some(exc.notification_mode.clone()));
                        exc_whatsapp_type.set(exc.notification_type.clone());
                        exc_whatsapp_call.set(exc.notify_on_call);
                    }
                    "telegram" => {
                        show_telegram_exception.set(true);
                        exc_telegram_mode.set(Some(exc.notification_mode.clone()));
                        exc_telegram_type.set(exc.notification_type.clone());
                        exc_telegram_call.set(exc.notify_on_call);
                    }
                    "signal" => {
                        show_signal_exception.set(true);
                        exc_signal_mode.set(Some(exc.notification_mode.clone()));
                        exc_signal_type.set(exc.notification_type.clone());
                        exc_signal_call.set(exc.notify_on_call);
                    }
                    "email" => {
                        show_email_exception.set(true);
                        exc_email_mode.set(Some(exc.notification_mode.clone()));
                        exc_email_type.set(exc.notification_type.clone());
                        exc_email_call.set(exc.notify_on_call);
                    }
                    _ => {}
                }
            }

            editing_profile.set(Some(profile));
            show_modal.set(true);
        })
    };

    // Close modal
    let close_modal = {
        let show_modal = show_modal.clone();
        Callback::from(move |_| {
            show_modal.set(false);
        })
    };

    // Save profile
    let save_profile = {
        let profiles = profiles.clone();
        let default_mode = default_mode.clone();
        let show_modal = show_modal.clone();
        let editing_profile = editing_profile.clone();
        let error_message = error_message.clone();
        let profile_save_state = profile_save_state.clone();
        let form_nickname = form_nickname.clone();
        let form_whatsapp = form_whatsapp.clone();
        let form_telegram = form_telegram.clone();
        let form_signal = form_signal.clone();
        let form_email = form_email.clone();
        let form_mode = form_mode.clone();
        let form_type = form_type.clone();
        let form_notify_call = form_notify_call.clone();
        // Exception state
        let exc_whatsapp_mode = exc_whatsapp_mode.clone();
        let exc_whatsapp_type = exc_whatsapp_type.clone();
        let exc_whatsapp_call = exc_whatsapp_call.clone();
        let exc_telegram_mode = exc_telegram_mode.clone();
        let exc_telegram_type = exc_telegram_type.clone();
        let exc_telegram_call = exc_telegram_call.clone();
        let exc_signal_mode = exc_signal_mode.clone();
        let exc_signal_type = exc_signal_type.clone();
        let exc_signal_call = exc_signal_call.clone();
        let exc_email_mode = exc_email_mode.clone();
        let exc_email_type = exc_email_type.clone();
        let exc_email_call = exc_email_call.clone();

        Callback::from(move |_| {
            let profiles = profiles.clone();
            let default_mode = default_mode.clone();
            let show_modal = show_modal.clone();
            let editing_profile_val = (*editing_profile).clone();
            let error_message = error_message.clone();
            let profile_save_state = profile_save_state.clone();

            // Build exceptions list from state
            let mut exceptions = Vec::new();
            if let Some(mode) = (*exc_whatsapp_mode).clone() {
                exceptions.push(ExceptionRequest {
                    platform: "whatsapp".to_string(),
                    notification_mode: mode,
                    notification_type: (*exc_whatsapp_type).clone(),
                    notify_on_call: *exc_whatsapp_call,
                });
            }
            if let Some(mode) = (*exc_telegram_mode).clone() {
                exceptions.push(ExceptionRequest {
                    platform: "telegram".to_string(),
                    notification_mode: mode,
                    notification_type: (*exc_telegram_type).clone(),
                    notify_on_call: *exc_telegram_call,
                });
            }
            if let Some(mode) = (*exc_signal_mode).clone() {
                exceptions.push(ExceptionRequest {
                    platform: "signal".to_string(),
                    notification_mode: mode,
                    notification_type: (*exc_signal_type).clone(),
                    notify_on_call: *exc_signal_call,
                });
            }
            if let Some(mode) = (*exc_email_mode).clone() {
                exceptions.push(ExceptionRequest {
                    platform: "email".to_string(),
                    notification_mode: mode,
                    notification_type: (*exc_email_type).clone(),
                    notify_on_call: *exc_email_call,
                });
            }

            let request = CreateProfileRequest {
                nickname: (*form_nickname).clone(),
                whatsapp_chat: if form_whatsapp.is_empty() { None } else { Some((*form_whatsapp).clone()) },
                telegram_chat: if form_telegram.is_empty() { None } else { Some((*form_telegram).clone()) },
                signal_chat: if form_signal.is_empty() { None } else { Some((*form_signal).clone()) },
                email_addresses: if form_email.is_empty() { None } else { Some((*form_email).clone()) },
                notification_mode: (*form_mode).clone(),
                notification_type: (*form_type).clone(),
                notify_on_call: *form_notify_call,
                exceptions: if exceptions.is_empty() { None } else { Some(exceptions) },
            };

            profile_save_state.set(FieldSaveState::Saving);
            spawn_local(async move {
                let result = if let Some(existing) = editing_profile_val {
                    Api::put(&format!("/api/contact-profiles/{}", existing.id))
                        .json(&request)
                        .ok()
                        .map(|r| r.send())
                } else {
                    Api::post("/api/contact-profiles")
                        .json(&request)
                        .ok()
                        .map(|r| r.send())
                };

                if let Some(future) = result {
                    if let Ok(response) = future.await {
                        if response.ok() {
                            // Reload profiles
                            if let Ok(resp) = Api::get("/api/contact-profiles").send().await {
                                if let Ok(data) = resp.json::<ContactProfilesResponse>().await {
                                    profiles.set(data.profiles);
                                    default_mode.set(data.default_mode);
                                }
                            }
                            profile_save_state.set(FieldSaveState::Idle);
                            show_modal.set(false);
                        } else {
                            profile_save_state.set(FieldSaveState::Error("Failed to save".to_string()));
                            error_message.set(Some("Failed to save profile".to_string()));
                        }
                    } else {
                        profile_save_state.set(FieldSaveState::Error("Network error".to_string()));
                        error_message.set(Some("Network error".to_string()));
                    }
                } else {
                    profile_save_state.set(FieldSaveState::Error("Failed to serialize".to_string()));
                    error_message.set(Some("Failed to serialize request".to_string()));
                }
            });
        })
    };

    // Delete profile
    let delete_profile = {
        let profiles = profiles.clone();
        let default_mode = default_mode.clone();
        let error_message = error_message.clone();

        Callback::from(move |profile_id: i32| {
            let profiles = profiles.clone();
            let default_mode = default_mode.clone();
            let error_message = error_message.clone();

            spawn_local(async move {
                if let Ok(response) = Api::delete(&format!("/api/contact-profiles/{}", profile_id)).send().await {
                    if response.ok() {
                        // Reload profiles
                        if let Ok(resp) = Api::get("/api/contact-profiles").send().await {
                            if let Ok(data) = resp.json::<ContactProfilesResponse>().await {
                                profiles.set(data.profiles);
                                default_mode.set(data.default_mode);
                            }
                        }
                    } else {
                        error_message.set(Some("Failed to delete profile".to_string()));
                    }
                } else {
                    error_message.set(Some("Network error".to_string()));
                }
            });
        })
    };

    // Update default mode
    let update_default_mode = {
        let default_mode = default_mode.clone();
        let default_save_state = default_save_state.clone();

        Callback::from(move |e: Event| {
            let target: HtmlSelectElement = e.target_unchecked_into();
            let new_mode = target.value();
            let default_mode = default_mode.clone();
            let default_save_state = default_save_state.clone();

            default_save_state.set(FieldSaveState::Saving);
            spawn_local(async move {
                let request = UpdateDefaultModeRequest { mode: Some(new_mode.clone()), noti_type: None, notify_on_call: None };
                if let Ok(req) = Api::put("/api/contact-profiles/default-mode").json(&request) {
                    if let Ok(response) = req.send().await {
                        if response.ok() {
                            default_mode.set(new_mode);
                            default_save_state.set(FieldSaveState::Success);
                            let state = default_save_state.clone();
                            spawn_local(async move {
                                TimeoutFuture::new(2000).await;
                                state.set(FieldSaveState::Idle);
                            });
                        } else {
                            default_save_state.set(FieldSaveState::Error("Failed to save".to_string()));
                        }
                    } else {
                        default_save_state.set(FieldSaveState::Error("Network error".to_string()));
                    }
                }
            });
        })
    };

    // Update default notification type
    let update_default_noti_type = {
        let default_noti_type = default_noti_type.clone();
        let default_save_state = default_save_state.clone();

        Callback::from(move |e: Event| {
            let target: HtmlSelectElement = e.target_unchecked_into();
            let new_type = target.value();
            let default_noti_type = default_noti_type.clone();
            let default_save_state = default_save_state.clone();

            default_save_state.set(FieldSaveState::Saving);
            spawn_local(async move {
                let request = UpdateDefaultModeRequest { mode: None, noti_type: Some(new_type.clone()), notify_on_call: None };
                if let Ok(req) = Api::put("/api/contact-profiles/default-mode").json(&request) {
                    if let Ok(response) = req.send().await {
                        if response.ok() {
                            default_noti_type.set(new_type);
                            default_save_state.set(FieldSaveState::Success);
                            let state = default_save_state.clone();
                            spawn_local(async move {
                                TimeoutFuture::new(2000).await;
                                state.set(FieldSaveState::Idle);
                            });
                        } else {
                            default_save_state.set(FieldSaveState::Error("Failed to save".to_string()));
                        }
                    } else {
                        default_save_state.set(FieldSaveState::Error("Network error".to_string()));
                    }
                }
            });
        })
    };

    // Update default notify on call
    let update_default_notify_on_call = {
        let default_notify_on_call = default_notify_on_call.clone();
        let default_save_state = default_save_state.clone();

        Callback::from(move |new_value: bool| {
            let default_notify_on_call = default_notify_on_call.clone();
            let default_save_state = default_save_state.clone();

            default_save_state.set(FieldSaveState::Saving);
            spawn_local(async move {
                let request = UpdateDefaultModeRequest { mode: None, noti_type: None, notify_on_call: Some(new_value) };
                if let Ok(req) = Api::put("/api/contact-profiles/default-mode").json(&request) {
                    if let Ok(response) = req.send().await {
                        if response.ok() {
                            default_notify_on_call.set(new_value);
                            default_save_state.set(FieldSaveState::Success);
                            let state = default_save_state.clone();
                            spawn_local(async move {
                                TimeoutFuture::new(2000).await;
                                state.set(FieldSaveState::Idle);
                            });
                        } else {
                            default_save_state.set(FieldSaveState::Error("Failed to save".to_string()));
                        }
                    } else {
                        default_save_state.set(FieldSaveState::Error("Network error".to_string()));
                    }
                }
            });
        })
    };

    let disabled_class = if props.critical_disabled { "section-disabled" } else { "" };

    html! {
        <div class={classes!("proactive-section", "contact-profiles-section", disabled_class)}>
            <style>
                {r#"
                    .contact-profiles-section {
                        padding: 1rem;
                    }
                    .contact-profiles-section .section-header {
                        display: flex;
                        align-items: center;
                        justify-content: space-between;
                        margin-bottom: 1rem;
                    }
                    .contact-profiles-section .section-header .header-left {
                        display: flex;
                        align-items: center;
                        gap: 0.5rem;
                    }
                    .contact-profiles-section .section-header h3 {
                        margin: 0;
                        color: #F59E0B;
                        font-size: 1.1rem;
                    }
                    .contact-profiles-section .disabled-badge {
                        color: #666;
                        font-size: 0.9rem;
                        margin-left: 0.5rem;
                    }
                    .contact-profiles-section .add-btn {
                        background: linear-gradient(45deg, #F59E0B, #D97706);
                        color: white;
                        border: none;
                        padding: 0.5rem 1rem;
                        border-radius: 6px;
                        cursor: pointer;
                        font-size: 0.9rem;
                        transition: all 0.3s ease;
                    }
                    .contact-profiles-section .add-btn:hover {
                        transform: translateY(-2px);
                        box-shadow: 0 4px 12px rgba(245, 158, 11, 0.3);
                    }
                    .contact-profiles-section .add-btn:disabled {
                        opacity: 0.5;
                        cursor: not-allowed;
                        transform: none;
                    }
                    .contact-profiles-section .error-message {
                        color: #FF6347;
                        background: rgba(255, 99, 71, 0.1);
                        padding: 0.75rem;
                        border-radius: 6px;
                        margin-bottom: 1rem;
                    }
                    .contact-profiles-section .loading {
                        color: #999;
                        text-align: center;
                        padding: 2rem;
                    }
                    .contact-profiles-section .empty-state {
                        text-align: center;
                        padding: 2rem;
                        color: #999;
                    }
                    .contact-profiles-section .empty-state .hint {
                        font-size: 0.85rem;
                        color: #666;
                    }
                    .contact-profiles-section .profiles-list {
                        display: flex;
                        flex-direction: column;
                        gap: 1rem;
                    }
                    .contact-profiles-section .profile-card {
                        background: rgba(0, 0, 0, 0.2);
                        border: 1px solid rgba(34, 197, 94, 0.4);
                        border-radius: 12px;
                        padding: 1rem;
                    }
                    .contact-profiles-section .profile-header {
                        display: flex;
                        justify-content: space-between;
                        align-items: center;
                    }
                    .contact-profiles-section .profile-name {
                        font-weight: 600;
                        font-size: 1rem;
                        color: #fff;
                    }
                    .contact-profiles-section .profile-actions {
                        display: flex;
                        gap: 0.5rem;
                    }
                    .contact-profiles-section .edit-btn {
                        background: rgba(245, 158, 11, 0.2);
                        color: #F59E0B;
                        border: 1px solid rgba(245, 158, 11, 0.3);
                        padding: 0.35rem 0.75rem;
                        border-radius: 6px;
                        cursor: pointer;
                        font-size: 0.85rem;
                    }
                    .contact-profiles-section .delete-btn {
                        background: rgba(255, 99, 71, 0.2);
                        color: #FF6347;
                        border: 1px solid rgba(255, 99, 71, 0.3);
                        padding: 0.35rem 0.75rem;
                        border-radius: 6px;
                        cursor: pointer;
                        font-size: 0.85rem;
                    }
                    .contact-profiles-section .profile-platforms {
                        margin-top: 0.75rem;
                        display: flex;
                        flex-wrap: wrap;
                        gap: 0.5rem;
                    }
                    .contact-profiles-section .platform-badge {
                        font-size: 0.8rem;
                        padding: 0.3rem 0.6rem;
                        border-radius: 6px;
                        display: inline-flex;
                        align-items: center;
                        gap: 0.4rem;
                    }
                    .contact-profiles-section .platform-badge i {
                        font-size: 0.9rem;
                    }
                    .contact-profiles-section .platform-badge.whatsapp {
                        background: rgba(37, 211, 102, 0.15);
                        color: #25D366;
                        border: 1px solid rgba(37, 211, 102, 0.3);
                    }
                    .contact-profiles-section .platform-badge.telegram {
                        background: rgba(0, 136, 204, 0.15);
                        color: #0088CC;
                        border: 1px solid rgba(0, 136, 204, 0.3);
                    }
                    .contact-profiles-section .platform-badge.signal {
                        background: rgba(58, 118, 241, 0.15);
                        color: #3A76F1;
                        border: 1px solid rgba(58, 118, 241, 0.3);
                    }
                    .contact-profiles-section .platform-badge.email {
                        background: rgba(126, 178, 255, 0.15);
                        color: #7EB2FF;
                        border: 1px solid rgba(126, 178, 255, 0.3);
                    }
                    .contact-profiles-section .platform-with-override {
                        display: flex;
                        flex-direction: column;
                        gap: 0.2rem;
                    }
                    .contact-profiles-section .platform-badge.has-override {
                        border-style: dashed;
                    }
                    .contact-profiles-section .override-icon {
                        font-size: 0.7rem;
                        margin-left: 0.2rem;
                        opacity: 0.7;
                    }
                    .contact-profiles-section .override-details {
                        font-size: 0.7rem;
                        color: #888;
                        padding-left: 0.3rem;
                        font-style: italic;
                    }
                    .contact-profiles-section .profile-settings {
                        margin-top: 0.75rem;
                        padding-top: 0.75rem;
                        border-top: 1px solid rgba(255, 255, 255, 0.1);
                        display: flex;
                        flex-wrap: wrap;
                        gap: 1rem;
                        align-items: center;
                    }
                    .contact-profiles-section .setting-item {
                        display: flex;
                        align-items: center;
                        gap: 0.3rem;
                    }
                    .contact-profiles-section .setting-label {
                        color: #888;
                        font-size: 0.8rem;
                    }
                    .contact-profiles-section .setting-value {
                        color: #ccc;
                        font-size: 0.85rem;
                    }
                    .contact-profiles-section .setting-value.mode.all {
                        color: #34D399;
                    }
                    .contact-profiles-section .setting-value.mode.critical {
                        color: #F59E0B;
                    }
                    .contact-profiles-section .setting-value.mode.digest {
                        color: #7EB2FF;
                    }
                    .contact-profiles-section .setting-value.calls {
                        color: #34D399;
                        display: flex;
                        align-items: center;
                        gap: 0.3rem;
                    }
                    .contact-profiles-section .setting-value.calls i {
                        font-size: 0.75rem;
                    }
                    .contact-profiles-section .default-mode-section {
                        margin-top: 1.5rem;
                        padding-top: 1rem;
                        border-top: 1px solid rgba(245, 158, 11, 0.1);
                    }
                    .contact-profiles-section .default-mode-header {
                        display: flex;
                        align-items: center;
                        flex-wrap: wrap;
                        gap: 0.5rem;
                    }
                    .contact-profiles-section .default-mode-section label {
                        color: #999;
                        font-size: 0.9rem;
                    }
                    .contact-profiles-section .default-mode-section select {
                        background: rgba(0, 0, 0, 0.3);
                        border: 1px solid rgba(245, 158, 11, 0.2);
                        color: #fff;
                        padding: 0.5rem;
                        border-radius: 4px;
                    }
                    .contact-profiles-section .info-btn {
                        background: none;
                        border: none;
                        color: #F59E0B;
                        font-size: 1rem;
                        cursor: pointer;
                        padding: 0.25rem;
                        border-radius: 50%;
                        width: 24px;
                        height: 24px;
                        display: inline-flex;
                        align-items: center;
                        justify-content: center;
                        transition: all 0.2s ease;
                        margin-left: 0.25rem;
                    }
                    .contact-profiles-section .info-btn:hover {
                        background: rgba(245, 158, 11, 0.1);
                    }
                    .contact-profiles-section .mode-info-panel {
                        margin-top: 0.75rem;
                        padding: 0.75rem;
                        background: rgba(0, 0, 0, 0.2);
                        border: 1px solid rgba(245, 158, 11, 0.1);
                        border-radius: 8px;
                        font-size: 0.8rem;
                        color: #999;
                        line-height: 1.5;
                    }
                    .contact-profiles-section .mode-info-panel strong {
                        color: #F59E0B;
                    }
                    .contact-profiles-section .mode-info-panel .mode-item {
                        margin-bottom: 0.5rem;
                    }
                    .contact-profiles-section .mode-info-panel .mode-item:last-child {
                        margin-bottom: 0;
                    }
                    .contact-profiles-section .mode-info-panel .example {
                        color: #666;
                        font-style: italic;
                        font-size: 0.75rem;
                    }
                    .contact-profiles-section .mode-info-panel .mode-section-header {
                        color: #F59E0B;
                        font-size: 0.7rem;
                        font-weight: 600;
                        letter-spacing: 0.5px;
                        margin-bottom: 0.25rem;
                        border-bottom: 1px solid rgba(245, 158, 11, 0.2);
                        padding-bottom: 0.25rem;
                    }
                    .contact-profiles-section .modal-overlay {
                        position: fixed;
                        top: 0;
                        left: 0;
                        right: 0;
                        bottom: 0;
                        background: rgba(0, 0, 0, 0.8);
                        display: flex;
                        align-items: center;
                        justify-content: center;
                        z-index: 9999;
                        transform: none !important;
                        contain: layout;
                    }
                    .contact-profiles-section .modal-content {
                        background: #1a1a1a;
                        border: 1px solid rgba(245, 158, 11, 0.2);
                        border-radius: 12px;
                        padding: 1.5rem;
                        max-width: 500px;
                        width: 90%;
                        max-height: 90vh;
                        overflow-y: auto;
                        transform: none !important;
                    }
                    .contact-profiles-section .modal-content h3 {
                        margin: 0 0 1rem 0;
                        color: #F59E0B;
                    }
                    .contact-profiles-section .form-group {
                        margin-bottom: 1rem;
                    }
                    .contact-profiles-section .form-group label {
                        display: block;
                        color: #999;
                        font-size: 0.9rem;
                        margin-bottom: 0.25rem;
                    }
                    .contact-profiles-section .form-group input,
                    .contact-profiles-section .form-group select {
                        width: 100%;
                        padding: 0.5rem;
                        background: rgba(0, 0, 0, 0.3);
                        border: 1px solid rgba(245, 158, 11, 0.2);
                        border-radius: 4px;
                        color: #fff;
                        box-sizing: border-box;
                    }
                    .contact-profiles-section .field-help {
                        color: #666;
                        font-size: 0.75rem;
                        margin-top: 0.25rem;
                        line-height: 1.4;
                    }
                    .contact-profiles-section .form-section {
                        margin-top: 1.5rem;
                        padding-top: 1rem;
                        border-top: 1px solid rgba(245, 158, 11, 0.1);
                    }
                    .contact-profiles-section .form-section h4 {
                        margin: 0 0 0.5rem 0;
                        color: #F59E0B;
                        font-size: 0.95rem;
                    }
                    .contact-profiles-section .form-section .hint {
                        color: #666;
                        font-size: 0.8rem;
                        margin-bottom: 1rem;
                    }
                    .contact-profiles-section .checkbox-group label {
                        display: flex;
                        align-items: center;
                        gap: 0.5rem;
                        cursor: pointer;
                    }
                    .contact-profiles-section .checkbox-group input {
                        width: auto;
                    }
                    .contact-profiles-section .modal-actions {
                        display: flex;
                        justify-content: flex-end;
                        gap: 0.75rem;
                        margin-top: 1.5rem;
                    }
                    .contact-profiles-section .cancel-btn {
                        background: transparent;
                        border: 1px solid rgba(255, 255, 255, 0.2);
                        color: #999;
                        padding: 0.5rem 1rem;
                        border-radius: 6px;
                        cursor: pointer;
                    }
                    .contact-profiles-section .save-btn {
                        background: linear-gradient(45deg, #F59E0B, #D97706);
                        color: white;
                        border: none;
                        padding: 0.5rem 1rem;
                        border-radius: 6px;
                        cursor: pointer;
                    }
                    .contact-profiles-section .input-with-suggestions {
                        position: relative;
                    }
                    .contact-profiles-section .suggestions-dropdown {
                        position: absolute;
                        top: 100%;
                        left: 0;
                        right: 0;
                        background: #2a2a2a;
                        border: 1px solid rgba(245, 158, 11, 0.2);
                        border-radius: 4px;
                        max-height: 200px;
                        overflow-y: auto;
                        z-index: 10;
                    }
                    .contact-profiles-section .suggestion-item {
                        padding: 0.5rem;
                        cursor: pointer;
                        color: #ccc;
                    }
                    .contact-profiles-section .suggestion-item:hover {
                        background: rgba(245, 158, 11, 0.1);
                    }
                    .contact-profiles-section .suggestion-item.searching,
                    .contact-profiles-section .suggestion-item.no-results {
                        color: #888;
                        font-style: italic;
                        cursor: default;
                    }
                    .contact-profiles-section .suggestion-item.searching:hover,
                    .contact-profiles-section .suggestion-item.no-results:hover {
                        background: transparent;
                    }
                    .contact-profiles-section .suggestion-item.error {
                        color: #FF6B6B;
                        font-style: italic;
                        cursor: default;
                        font-size: 0.85rem;
                    }
                    .contact-profiles-section .suggestion-item.error:hover {
                        background: transparent;
                    }
                    .contact-profiles-section .platform-input-row {
                        display: flex;
                        align-items: center;
                        gap: 0.5rem;
                    }
                    .contact-profiles-section .platform-input-row .input-with-suggestions {
                        flex: 1;
                    }
                    .contact-profiles-section .exception-toggle-btn {
                        background: rgba(245, 158, 11, 0.1);
                        border: 1px solid rgba(245, 158, 11, 0.2);
                        color: #999;
                        padding: 0.4rem 0.5rem;
                        border-radius: 4px;
                        cursor: pointer;
                        font-size: 0.9rem;
                        transition: all 0.2s ease;
                        min-width: 28px;
                        text-align: center;
                    }
                    .contact-profiles-section .exception-toggle-btn:hover {
                        background: rgba(245, 158, 11, 0.2);
                        color: #F59E0B;
                    }
                    .contact-profiles-section .exception-toggle-btn.active {
                        background: rgba(245, 158, 11, 0.3);
                        color: #F59E0B;
                        border-color: #F59E0B;
                    }
                    .contact-profiles-section .exception-panel {
                        margin-top: 0.5rem;
                        margin-left: 0;
                        padding: 0.75rem;
                        background: rgba(245, 158, 11, 0.05);
                        border: 1px solid rgba(245, 158, 11, 0.15);
                        border-radius: 6px;
                    }
                    .contact-profiles-section .exception-panel .exception-header {
                        display: flex;
                        justify-content: space-between;
                        align-items: center;
                        margin-bottom: 0.5rem;
                    }
                    .contact-profiles-section .exception-panel .exception-header span {
                        font-size: 0.8rem;
                        color: #F59E0B;
                    }
                    .contact-profiles-section .exception-panel .clear-exc-btn {
                        background: transparent;
                        border: none;
                        color: #888;
                        font-size: 0.75rem;
                        cursor: pointer;
                        text-decoration: underline;
                    }
                    .contact-profiles-section .exception-panel .clear-exc-btn:hover {
                        color: #FF6B6B;
                    }
                    .contact-profiles-section .exception-controls {
                        display: flex;
                        flex-wrap: wrap;
                        gap: 0.5rem;
                        align-items: center;
                    }
                    .contact-profiles-section .exception-controls select {
                        padding: 0.35rem 0.5rem;
                        font-size: 0.8rem;
                        background: rgba(0, 0, 0, 0.3);
                        border: 1px solid rgba(245, 158, 11, 0.2);
                        border-radius: 4px;
                        color: #fff;
                    }
                    .contact-profiles-section .exception-controls label {
                        display: flex;
                        align-items: center;
                        gap: 0.25rem;
                        font-size: 0.8rem;
                        color: #999;
                        cursor: pointer;
                    }
                    .contact-profiles-section .exception-controls input[type="checkbox"] {
                        width: auto;
                    }
                    .contact-profiles-section .exc-badge {
                        font-size: 0.6rem;
                        padding: 0.1rem 0.3rem;
                        border-radius: 3px;
                        background: rgba(245, 158, 11, 0.2);
                        color: #F59E0B;
                        margin-left: 0.25rem;
                    }
                    .contact-profiles-section .section-content {
                        transition: opacity 0.3s ease;
                    }
                    .contact-profiles-section .section-content.disabled {
                        opacity: 0.5;
                        pointer-events: none;
                    }
                    /* Save indicator styles */
                    .contact-profiles-section .save-indicator {
                        min-width: 20px;
                        height: 20px;
                        display: inline-flex;
                        align-items: center;
                        justify-content: center;
                        margin-left: 0.5rem;
                    }
                    .contact-profiles-section .save-spinner {
                        width: 14px;
                        height: 14px;
                        border: 2px solid rgba(245, 158, 11, 0.3);
                        border-top-color: #F59E0B;
                        border-radius: 50%;
                        animation: spin 1s linear infinite;
                    }
                    @keyframes spin {
                        to { transform: rotate(360deg); }
                    }
                    .contact-profiles-section .save-success {
                        color: #22C55E;
                        font-size: 16px;
                    }
                    .contact-profiles-section .save-error {
                        color: #EF4444;
                        cursor: help;
                        font-size: 16px;
                    }
                "#}
            </style>
            <div class={classes!("section-content", if props.critical_disabled { "disabled" } else { "" })}>
            <div class="section-header">
                <div class="header-left">
                    <button class="info-btn" onclick={{
                        let show_mode_info = show_mode_info.clone();
                        Callback::from(move |_| show_mode_info.set(!*show_mode_info))
                    }}>
                        {"ⓘ"}
                    </button>
                    <i class="fas fa-user-circle" style="color: #F59E0B; font-size: 1.1rem;"></i>
                    <h3>{"Contact Profiles"}</h3>
                </div>
                <button class="add-btn" onclick={open_new_modal.clone()} disabled={props.critical_disabled}>
                    {"+ Add Profile"}
                </button>
            </div>

            // Info panel right under header
            if *show_mode_info {
                <div class="mode-info-panel">
                    <div class="mode-item">
                        {"Create profiles for specific contacts or chats to customize how you're notified about their messages."}
                    </div>
                    <div class="mode-item">
                        {"Each profile can be linked to WhatsApp, Telegram, Signal chats, or email addresses. You can set different notification rules for each."}
                    </div>
                    <div class="mode-item">
                        {"The \"Everyone else\" setting below applies to any contact or chat that doesn't have a profile."}
                    </div>
                </div>
            }

            if let Some(error) = (*error_message).as_ref() {
                <div class="error-message">{error}</div>
            }

            if *loading {
                <div class="loading">{"Loading..."}</div>
            } else {
                <div class="profiles-list">
                    if profiles.is_empty() {
                        <div class="empty-state">
                            <p>{"No contact profiles yet. Add one to get started!"}</p>
                            <p class="hint">{"Contact profiles let you set notification preferences for specific people or group chats."}</p>
                        </div>
                    } else {
                        { for profiles.iter().map(|profile| {
                            let profile_clone = profile.clone();
                            let profile_id = profile.id;
                            let open_edit = open_edit_modal.clone();
                            let delete = delete_profile.clone();

                            // Pre-compute exception info for each platform
                            let wa_exc = profile.exceptions.iter().find(|e| e.platform == "whatsapp").cloned();
                            let tg_exc = profile.exceptions.iter().find(|e| e.platform == "telegram").cloned();
                            let sig_exc = profile.exceptions.iter().find(|e| e.platform == "signal").cloned();
                            let email_exc = profile.exceptions.iter().find(|e| e.platform == "email").cloned();

                            // Helper to format exception details
                            let format_exc = |exc: &ProfileException| -> String {
                                let mode = match exc.notification_mode.as_str() {
                                    "all" => "All",
                                    "critical" => "Critical",
                                    "digest" => "Digest",
                                    _ => &exc.notification_mode
                                };
                                if exc.notification_mode != "digest" {
                                    let via = match exc.notification_type.as_str() {
                                        "sms" => "SMS",
                                        "call" => "Call",
                                        "call_sms" => "Call+SMS",
                                        _ => &exc.notification_type
                                    };
                                    format!("{} via {}", mode, via)
                                } else {
                                    mode.to_string()
                                }
                            };

                            let wa_exc_text = wa_exc.as_ref().map(|e| format_exc(e));
                            let tg_exc_text = tg_exc.as_ref().map(|e| format_exc(e));
                            let sig_exc_text = sig_exc.as_ref().map(|e| format_exc(e));
                            let email_exc_text = email_exc.as_ref().map(|e| format_exc(e));

                            html! {
                                <div class="profile-card">
                                    <div class="profile-header">
                                        <span class="profile-name">{&profile.nickname}</span>
                                        <div class="profile-actions">
                                            <button class="edit-btn" onclick={Callback::from(move |_| open_edit.emit(profile_clone.clone()))}>{"Edit"}</button>
                                            <button class="delete-btn" onclick={Callback::from(move |_| delete.emit(profile_id))}>{"x"}</button>
                                        </div>
                                    </div>
                                    // Platforms row
                                    <div class="profile-platforms">
                                        if let Some(wa) = &profile.whatsapp_chat {
                                            <div class="platform-with-override">
                                                <span class={classes!("platform-badge", "whatsapp", wa_exc.is_some().then(|| "has-override"))}>
                                                    <i class="fab fa-whatsapp"></i>
                                                    {wa}
                                                    if wa_exc.is_some() {
                                                        <i class="fas fa-cog override-icon" title="Has custom settings"></i>
                                                    }
                                                </span>
                                                if let Some(text) = &wa_exc_text {
                                                    <span class="override-details">{text}</span>
                                                }
                                            </div>
                                        }
                                        if let Some(tg) = &profile.telegram_chat {
                                            <div class="platform-with-override">
                                                <span class={classes!("platform-badge", "telegram", tg_exc.is_some().then(|| "has-override"))}>
                                                    <i class="fab fa-telegram"></i>
                                                    {tg}
                                                    if tg_exc.is_some() {
                                                        <i class="fas fa-cog override-icon" title="Has custom settings"></i>
                                                    }
                                                </span>
                                                if let Some(text) = &tg_exc_text {
                                                    <span class="override-details">{text}</span>
                                                }
                                            </div>
                                        }
                                        if let Some(sig) = &profile.signal_chat {
                                            <div class="platform-with-override">
                                                <span class={classes!("platform-badge", "signal", sig_exc.is_some().then(|| "has-override"))}>
                                                    <i class="fas fa-comment-dots"></i>
                                                    {sig}
                                                    if sig_exc.is_some() {
                                                        <i class="fas fa-cog override-icon" title="Has custom settings"></i>
                                                    }
                                                </span>
                                                if let Some(text) = &sig_exc_text {
                                                    <span class="override-details">{text}</span>
                                                }
                                            </div>
                                        }
                                        if let Some(email) = &profile.email_addresses {
                                            <div class="platform-with-override">
                                                <span class={classes!("platform-badge", "email", email_exc.is_some().then(|| "has-override"))}>
                                                    <i class="fas fa-envelope"></i>
                                                    {email}
                                                    if email_exc.is_some() {
                                                        <i class="fas fa-cog override-icon" title="Has custom settings"></i>
                                                    }
                                                </span>
                                                if let Some(text) = &email_exc_text {
                                                    <span class="override-details">{text}</span>
                                                }
                                            </div>
                                        }
                                    </div>
                                    // Settings row
                                    <div class="profile-settings">
                                        <div class="setting-item">
                                            <span class="setting-label">{"Mode:"}</span>
                                            <span class={classes!("setting-value", "mode", &profile.notification_mode)}>
                                                {match profile.notification_mode.as_str() {
                                                    "all" => "All messages",
                                                    "critical" => "Critical only",
                                                    "digest" => "Digest only",
                                                    _ => &profile.notification_mode
                                                }}
                                            </span>
                                        </div>
                                        if profile.notification_mode == "all" || profile.notification_mode == "critical" {
                                            <div class="setting-item">
                                                <span class="setting-label">{"Via:"}</span>
                                                <span class="setting-value">
                                                    {match profile.notification_type.as_str() {
                                                        "sms" => "SMS",
                                                        "call" => "Call",
                                                        "call_sms" => "Call + SMS",
                                                        _ => &profile.notification_type
                                                    }}
                                                </span>
                                            </div>
                                        }
                                        if profile.notify_on_call {
                                            <div class="setting-item">
                                                <span class="setting-value calls">
                                                    <i class="fas fa-phone"></i>
                                                    {"Call alerts"}
                                                </span>
                                            </div>
                                        }
                                    </div>
                                </div>
                            }
                        })}
                    }
                </div>

                <div class="default-mode-section">
                    <div class="default-mode-header">
                        <label>{"Everyone else:"}</label>
                        {render_save_indicator(&*default_save_state)}
                        <select value={(*default_mode).clone()} onchange={update_default_mode}>
                            <option value="critical" selected={*default_mode == "critical"}>{"Critical"}</option>
                            <option value="digest" selected={*default_mode == "digest"}>{"Digest"}</option>
                            <option value="ignore" selected={*default_mode == "ignore"}>{"Ignore"}</option>
                        </select>
                        if *default_mode == "critical" {
                            <label style="margin-left: 0.5rem;">{"+"}</label>
                            <label
                                class="calls-checkbox"
                                style="display: inline-flex; align-items: center; gap: 0.25rem; cursor: pointer;"
                                title="Get notified when contacts call you on WhatsApp/Telegram/Signal"
                            >
                                <input
                                    type="checkbox"
                                    checked={*default_notify_on_call}
                                    onchange={{
                                        let update_default_notify_on_call = update_default_notify_on_call.clone();
                                        Callback::from(move |e: Event| {
                                            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                                            update_default_notify_on_call.emit(input.checked());
                                        })
                                    }}
                                    style="width: auto; margin: 0;"
                                />
                                {"Alert on incoming calls"}
                            </label>
                            <label style="margin-left: 0.5rem;">{"via"}</label>
                            <select value={(*default_noti_type).clone()} onchange={update_default_noti_type}>
                                <option value="sms" selected={*default_noti_type == "sms"}>{"SMS"}</option>
                                <option value="call" selected={*default_noti_type == "call"}>{"Call"}</option>
                                <option value="call_sms" selected={*default_noti_type == "call_sms"}>{"Call + SMS"}</option>
                            </select>
                        }
                    </div>
                </div>
            }
            </div> // Close section-content

            // Modal for add/edit
            if *show_modal {
                <div class="modal-overlay" onclick={close_modal.clone()}>
                    <div class="modal-content" onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}>
                        <h3>{if editing_profile.is_some() { "Edit Contact Profile" } else { "Add Contact Profile" }}</h3>

                        <div class="form-group">
                            <label>{"Nickname"}</label>
                            <input
                                type="text"
                                placeholder="e.g., Mom, Boss, Roommates"
                                value={(*form_nickname).clone()}
                                oninput={Callback::from({
                                    let form_nickname = form_nickname.clone();
                                    move |e: InputEvent| {
                                        let input: HtmlInputElement = e.target_unchecked_into();
                                        form_nickname.set(input.value());
                                    }
                                })}
                            />
                            <div class="field-help">{"A name to identify this profile. Could be a person, group, or category."}</div>
                        </div>

                        <div class="form-section">
                            <h4>{"Connected Platforms"}</h4>
                            <p class="hint">{"Link this profile to specific chats. Messages from these chats will use this profile's notification rules."}</p>

                            <div class="form-group">
                                <label>{"WhatsApp"}{if exc_whatsapp_mode.is_some() { html!{<span class="exc-badge">{"custom"}</span>} } else { html!{} }}</label>
                                <div class="platform-input-row">
                                    <div class="input-with-suggestions">
                                        <input
                                            type="text"
                                            placeholder="Search or type chat name"
                                            value={(*form_whatsapp).clone()}
                                            oninput={Callback::from({
                                                let form_whatsapp = form_whatsapp.clone();
                                                let search_chats = search_chats.clone();
                                                move |e: InputEvent| {
                                                    let input: HtmlInputElement = e.target_unchecked_into();
                                                    let value = input.value();
                                                    form_whatsapp.set(value.clone());
                                                    search_chats.emit(("whatsapp".to_string(), value));
                                                }
                                            })}
                                        />
                                        if *show_whatsapp_suggestions {
                                            <div class="suggestions-dropdown">
                                                if *searching_whatsapp {
                                                    <div class="suggestion-item searching">{"Searching..."}</div>
                                                } else if let Some(err) = (*search_error_whatsapp).as_ref() {
                                                    <div class="suggestion-item error">{err}</div>
                                                } else if whatsapp_results.is_empty() {
                                                    <div class="suggestion-item no-results">{"No chats found"}</div>
                                                } else {
                                                    { for whatsapp_results.iter().map(|room| {
                                                        let name = room.display_name.clone();
                                                        let form_whatsapp = form_whatsapp.clone();
                                                        let show_whatsapp_suggestions = show_whatsapp_suggestions.clone();
                                                        html! {
                                                            <div class="suggestion-item" onclick={Callback::from(move |_| {
                                                                form_whatsapp.set(name.clone());
                                                                show_whatsapp_suggestions.set(false);
                                                            })}>
                                                                {&room.display_name}
                                                            </div>
                                                        }
                                                    })}
                                                }
                                            </div>
                                        }
                                    </div>
                                    if !form_whatsapp.is_empty() {
                                        <button
                                            type="button"
                                            class={classes!("exception-toggle-btn", if *show_whatsapp_exception { "active" } else { "" })}
                                            onclick={{
                                                let show_whatsapp_exception = show_whatsapp_exception.clone();
                                                Callback::from(move |_| show_whatsapp_exception.set(!*show_whatsapp_exception))
                                            }}
                                            title="Custom notification rules for WhatsApp"
                                        >
                                            {"⚙"}
                                        </button>
                                    }
                                </div>
                                if *show_whatsapp_exception && !form_whatsapp.is_empty() {
                                    <div class="exception-panel">
                                        <div class="exception-header">
                                            <span>{"WhatsApp-specific rules"}</span>
                                            <button type="button" class="clear-exc-btn" onclick={{
                                                let exc_whatsapp_mode = exc_whatsapp_mode.clone();
                                                let show_whatsapp_exception = show_whatsapp_exception.clone();
                                                Callback::from(move |_| {
                                                    exc_whatsapp_mode.set(None);
                                                    show_whatsapp_exception.set(false);
                                                })
                                            }}>{"Use default"}</button>
                                        </div>
                                        <div class="exception-controls">
                                            <select
                                                value={(*exc_whatsapp_mode).clone().unwrap_or_else(|| (*form_mode).clone())}
                                                onchange={{
                                                    let exc_whatsapp_mode = exc_whatsapp_mode.clone();
                                                    Callback::from(move |e: Event| {
                                                        let target: HtmlSelectElement = e.target_unchecked_into();
                                                        exc_whatsapp_mode.set(Some(target.value()));
                                                    })
                                                }}
                                            >
                                                <option value="all">{"All"}</option>
                                                <option value="critical">{"Critical"}</option>
                                                <option value="digest">{"Digest"}</option>
                                            </select>
                                            <span style="color: #666; font-size: 0.8rem;">{"via"}</span>
                                            <select
                                                value={(*exc_whatsapp_type).clone()}
                                                onchange={{
                                                    let exc_whatsapp_type = exc_whatsapp_type.clone();
                                                    let exc_whatsapp_mode = exc_whatsapp_mode.clone();
                                                    let form_mode = form_mode.clone();
                                                    Callback::from(move |e: Event| {
                                                        let target: HtmlSelectElement = e.target_unchecked_into();
                                                        exc_whatsapp_type.set(target.value());
                                                        // Ensure mode is set if changing type
                                                        if exc_whatsapp_mode.is_none() {
                                                            exc_whatsapp_mode.set(Some((*form_mode).clone()));
                                                        }
                                                    })
                                                }}
                                            >
                                                <option value="sms">{"SMS"}</option>
                                                <option value="call">{"Call"}</option>
                                                <option value="call_sms">{"Call + SMS"}</option>
                                            </select>
                                            <label title="Alert when this contact calls you on WhatsApp/Telegram/Signal">
                                                <input
                                                    type="checkbox"
                                                    checked={*exc_whatsapp_call}
                                                    onchange={{
                                                        let exc_whatsapp_call = exc_whatsapp_call.clone();
                                                        let exc_whatsapp_mode = exc_whatsapp_mode.clone();
                                                        let form_mode = form_mode.clone();
                                                        Callback::from(move |e: Event| {
                                                            let input: HtmlInputElement = e.target_unchecked_into();
                                                            exc_whatsapp_call.set(input.checked());
                                                            if exc_whatsapp_mode.is_none() {
                                                                exc_whatsapp_mode.set(Some((*form_mode).clone()));
                                                            }
                                                        })
                                                    }}
                                                />
                                                {"Incoming calls"}
                                            </label>
                                        </div>
                                    </div>
                                }
                            </div>

                            <div class="form-group">
                                <label>{"Telegram"}{if exc_telegram_mode.is_some() { html!{<span class="exc-badge">{"custom"}</span>} } else { html!{} }}</label>
                                <div class="platform-input-row">
                                    <div class="input-with-suggestions">
                                        <input
                                            type="text"
                                            placeholder="Search or type chat name"
                                            value={(*form_telegram).clone()}
                                            oninput={Callback::from({
                                                let form_telegram = form_telegram.clone();
                                                let search_chats = search_chats.clone();
                                                move |e: InputEvent| {
                                                    let input: HtmlInputElement = e.target_unchecked_into();
                                                    let value = input.value();
                                                    form_telegram.set(value.clone());
                                                    search_chats.emit(("telegram".to_string(), value));
                                                }
                                            })}
                                        />
                                        if *show_telegram_suggestions {
                                            <div class="suggestions-dropdown">
                                                if *searching_telegram {
                                                    <div class="suggestion-item searching">{"Searching..."}</div>
                                                } else if let Some(err) = (*search_error_telegram).as_ref() {
                                                    <div class="suggestion-item error">{err}</div>
                                                } else if telegram_results.is_empty() {
                                                    <div class="suggestion-item no-results">{"No chats found"}</div>
                                                } else {
                                                    { for telegram_results.iter().map(|room| {
                                                        let name = room.display_name.clone();
                                                        let form_telegram = form_telegram.clone();
                                                        let show_telegram_suggestions = show_telegram_suggestions.clone();
                                                        html! {
                                                            <div class="suggestion-item" onclick={Callback::from(move |_| {
                                                                form_telegram.set(name.clone());
                                                                show_telegram_suggestions.set(false);
                                                            })}>
                                                                {&room.display_name}
                                                            </div>
                                                        }
                                                    })}
                                                }
                                            </div>
                                        }
                                    </div>
                                    if !form_telegram.is_empty() {
                                        <button
                                            type="button"
                                            class={classes!("exception-toggle-btn", if *show_telegram_exception { "active" } else { "" })}
                                            onclick={{
                                                let show_telegram_exception = show_telegram_exception.clone();
                                                Callback::from(move |_| show_telegram_exception.set(!*show_telegram_exception))
                                            }}
                                            title="Custom notification rules for Telegram"
                                        >
                                            {"⚙"}
                                        </button>
                                    }
                                </div>
                                if *show_telegram_exception && !form_telegram.is_empty() {
                                    <div class="exception-panel">
                                        <div class="exception-header">
                                            <span>{"Telegram-specific rules"}</span>
                                            <button type="button" class="clear-exc-btn" onclick={{
                                                let exc_telegram_mode = exc_telegram_mode.clone();
                                                let show_telegram_exception = show_telegram_exception.clone();
                                                Callback::from(move |_| {
                                                    exc_telegram_mode.set(None);
                                                    show_telegram_exception.set(false);
                                                })
                                            }}>{"Use default"}</button>
                                        </div>
                                        <div class="exception-controls">
                                            <select
                                                value={(*exc_telegram_mode).clone().unwrap_or_else(|| (*form_mode).clone())}
                                                onchange={{
                                                    let exc_telegram_mode = exc_telegram_mode.clone();
                                                    Callback::from(move |e: Event| {
                                                        let target: HtmlSelectElement = e.target_unchecked_into();
                                                        exc_telegram_mode.set(Some(target.value()));
                                                    })
                                                }}
                                            >
                                                <option value="all">{"All"}</option>
                                                <option value="critical">{"Critical"}</option>
                                                <option value="digest">{"Digest"}</option>
                                            </select>
                                            <span style="color: #666; font-size: 0.8rem;">{"via"}</span>
                                            <select
                                                value={(*exc_telegram_type).clone()}
                                                onchange={{
                                                    let exc_telegram_type = exc_telegram_type.clone();
                                                    let exc_telegram_mode = exc_telegram_mode.clone();
                                                    let form_mode = form_mode.clone();
                                                    Callback::from(move |e: Event| {
                                                        let target: HtmlSelectElement = e.target_unchecked_into();
                                                        exc_telegram_type.set(target.value());
                                                        if exc_telegram_mode.is_none() {
                                                            exc_telegram_mode.set(Some((*form_mode).clone()));
                                                        }
                                                    })
                                                }}
                                            >
                                                <option value="sms">{"SMS"}</option>
                                                <option value="call">{"Call"}</option>
                                                <option value="call_sms">{"Call + SMS"}</option>
                                            </select>
                                            <label title="Alert when this contact calls you on WhatsApp/Telegram/Signal">
                                                <input
                                                    type="checkbox"
                                                    checked={*exc_telegram_call}
                                                    onchange={{
                                                        let exc_telegram_call = exc_telegram_call.clone();
                                                        let exc_telegram_mode = exc_telegram_mode.clone();
                                                        let form_mode = form_mode.clone();
                                                        Callback::from(move |e: Event| {
                                                            let input: HtmlInputElement = e.target_unchecked_into();
                                                            exc_telegram_call.set(input.checked());
                                                            if exc_telegram_mode.is_none() {
                                                                exc_telegram_mode.set(Some((*form_mode).clone()));
                                                            }
                                                        })
                                                    }}
                                                />
                                                {"Incoming calls"}
                                            </label>
                                        </div>
                                    </div>
                                }
                            </div>

                            <div class="form-group">
                                <label>{"Signal"}{if exc_signal_mode.is_some() { html!{<span class="exc-badge">{"custom"}</span>} } else { html!{} }}</label>
                                <div class="platform-input-row">
                                    <div class="input-with-suggestions">
                                    <input
                                        type="text"
                                        placeholder="Search or type chat name"
                                        value={(*form_signal).clone()}
                                        oninput={Callback::from({
                                            let form_signal = form_signal.clone();
                                            let search_chats = search_chats.clone();
                                            move |e: InputEvent| {
                                                let input: HtmlInputElement = e.target_unchecked_into();
                                                let value = input.value();
                                                form_signal.set(value.clone());
                                                search_chats.emit(("signal".to_string(), value));
                                            }
                                        })}
                                    />
                                    if *show_signal_suggestions {
                                        <div class="suggestions-dropdown">
                                            if *searching_signal {
                                                <div class="suggestion-item searching">{"Searching..."}</div>
                                            } else if let Some(err) = (*search_error_signal).as_ref() {
                                                <div class="suggestion-item error">{err}</div>
                                            } else if signal_results.is_empty() {
                                                <div class="suggestion-item no-results">{"No chats found"}</div>
                                            } else {
                                                { for signal_results.iter().map(|room| {
                                                    let name = room.display_name.clone();
                                                    let form_signal = form_signal.clone();
                                                    let show_signal_suggestions = show_signal_suggestions.clone();
                                                    html! {
                                                        <div class="suggestion-item" onclick={Callback::from(move |_| {
                                                            form_signal.set(name.clone());
                                                            show_signal_suggestions.set(false);
                                                        })}>
                                                            {&room.display_name}
                                                        </div>
                                                    }
                                                })}
                                            }
                                        </div>
                                    }
                                    </div>
                                    if !form_signal.is_empty() {
                                        <button
                                            type="button"
                                            class={classes!("exception-toggle-btn", if *show_signal_exception { "active" } else { "" })}
                                            onclick={{
                                                let show_signal_exception = show_signal_exception.clone();
                                                Callback::from(move |_| show_signal_exception.set(!*show_signal_exception))
                                            }}
                                            title="Custom notification rules for Signal"
                                        >
                                            {"⚙"}
                                        </button>
                                    }
                                </div>
                                if *show_signal_exception && !form_signal.is_empty() {
                                    <div class="exception-panel">
                                        <div class="exception-header">
                                            <span>{"Signal-specific rules"}</span>
                                            <button type="button" class="clear-exc-btn" onclick={{
                                                let exc_signal_mode = exc_signal_mode.clone();
                                                let show_signal_exception = show_signal_exception.clone();
                                                Callback::from(move |_| {
                                                    exc_signal_mode.set(None);
                                                    show_signal_exception.set(false);
                                                })
                                            }}>{"Use default"}</button>
                                        </div>
                                        <div class="exception-controls">
                                            <select
                                                value={(*exc_signal_mode).clone().unwrap_or_else(|| (*form_mode).clone())}
                                                onchange={{
                                                    let exc_signal_mode = exc_signal_mode.clone();
                                                    Callback::from(move |e: Event| {
                                                        let target: HtmlSelectElement = e.target_unchecked_into();
                                                        exc_signal_mode.set(Some(target.value()));
                                                    })
                                                }}
                                            >
                                                <option value="all">{"All"}</option>
                                                <option value="critical">{"Critical"}</option>
                                                <option value="digest">{"Digest"}</option>
                                            </select>
                                            <span style="color: #666; font-size: 0.8rem;">{"via"}</span>
                                            <select
                                                value={(*exc_signal_type).clone()}
                                                onchange={{
                                                    let exc_signal_type = exc_signal_type.clone();
                                                    let exc_signal_mode = exc_signal_mode.clone();
                                                    let form_mode = form_mode.clone();
                                                    Callback::from(move |e: Event| {
                                                        let target: HtmlSelectElement = e.target_unchecked_into();
                                                        exc_signal_type.set(target.value());
                                                        if exc_signal_mode.is_none() {
                                                            exc_signal_mode.set(Some((*form_mode).clone()));
                                                        }
                                                    })
                                                }}
                                            >
                                                <option value="sms">{"SMS"}</option>
                                                <option value="call">{"Call"}</option>
                                                <option value="call_sms">{"Call + SMS"}</option>
                                            </select>
                                            <label title="Alert when this contact calls you on WhatsApp/Telegram/Signal">
                                                <input
                                                    type="checkbox"
                                                    checked={*exc_signal_call}
                                                    onchange={{
                                                        let exc_signal_call = exc_signal_call.clone();
                                                        let exc_signal_mode = exc_signal_mode.clone();
                                                        let form_mode = form_mode.clone();
                                                        Callback::from(move |e: Event| {
                                                            let input: HtmlInputElement = e.target_unchecked_into();
                                                            exc_signal_call.set(input.checked());
                                                            if exc_signal_mode.is_none() {
                                                                exc_signal_mode.set(Some((*form_mode).clone()));
                                                            }
                                                        })
                                                    }}
                                                />
                                                {"Incoming calls"}
                                            </label>
                                        </div>
                                    </div>
                                }
                            </div>

                            <div class="form-group">
                                <label>{"Email(s)"}{if exc_email_mode.is_some() { html!{<span class="exc-badge">{"custom"}</span>} } else { html!{} }}</label>
                                <div class="platform-input-row">
                                    <input
                                        type="text"
                                        placeholder="email@example.com, other@example.com"
                                        value={(*form_email).clone()}
                                        style="flex: 1;"
                                        oninput={Callback::from({
                                            let form_email = form_email.clone();
                                            move |e: InputEvent| {
                                                let input: HtmlInputElement = e.target_unchecked_into();
                                                form_email.set(input.value());
                                            }
                                        })}
                                    />
                                    if !form_email.is_empty() {
                                        <button
                                            type="button"
                                            class={classes!("exception-toggle-btn", if *show_email_exception { "active" } else { "" })}
                                            onclick={{
                                                let show_email_exception = show_email_exception.clone();
                                                Callback::from(move |_| show_email_exception.set(!*show_email_exception))
                                            }}
                                            title="Custom notification rules for Email"
                                        >
                                            {"⚙"}
                                        </button>
                                    }
                                </div>
                                if *show_email_exception && !form_email.is_empty() {
                                    <div class="exception-panel">
                                        <div class="exception-header">
                                            <span>{"Email-specific rules"}</span>
                                            <button type="button" class="clear-exc-btn" onclick={{
                                                let exc_email_mode = exc_email_mode.clone();
                                                let show_email_exception = show_email_exception.clone();
                                                Callback::from(move |_| {
                                                    exc_email_mode.set(None);
                                                    show_email_exception.set(false);
                                                })
                                            }}>{"Use default"}</button>
                                        </div>
                                        <div class="exception-controls">
                                            <select
                                                value={(*exc_email_mode).clone().unwrap_or_else(|| (*form_mode).clone())}
                                                onchange={{
                                                    let exc_email_mode = exc_email_mode.clone();
                                                    Callback::from(move |e: Event| {
                                                        let target: HtmlSelectElement = e.target_unchecked_into();
                                                        exc_email_mode.set(Some(target.value()));
                                                    })
                                                }}
                                            >
                                                <option value="all">{"All"}</option>
                                                <option value="critical">{"Critical"}</option>
                                                <option value="digest">{"Digest"}</option>
                                            </select>
                                            <span style="color: #666; font-size: 0.8rem;">{"via"}</span>
                                            <select
                                                value={(*exc_email_type).clone()}
                                                onchange={{
                                                    let exc_email_type = exc_email_type.clone();
                                                    let exc_email_mode = exc_email_mode.clone();
                                                    let form_mode = form_mode.clone();
                                                    Callback::from(move |e: Event| {
                                                        let target: HtmlSelectElement = e.target_unchecked_into();
                                                        exc_email_type.set(target.value());
                                                        if exc_email_mode.is_none() {
                                                            exc_email_mode.set(Some((*form_mode).clone()));
                                                        }
                                                    })
                                                }}
                                            >
                                                <option value="sms">{"SMS"}</option>
                                                <option value="call">{"Call"}</option>
                                                <option value="call_sms">{"Call + SMS"}</option>
                                            </select>
                                            <label title="Alert when this contact calls you on WhatsApp/Telegram/Signal">
                                                <input
                                                    type="checkbox"
                                                    checked={*exc_email_call}
                                                    onchange={{
                                                        let exc_email_call = exc_email_call.clone();
                                                        let exc_email_mode = exc_email_mode.clone();
                                                        let form_mode = form_mode.clone();
                                                        Callback::from(move |e: Event| {
                                                            let input: HtmlInputElement = e.target_unchecked_into();
                                                            exc_email_call.set(input.checked());
                                                            if exc_email_mode.is_none() {
                                                                exc_email_mode.set(Some((*form_mode).clone()));
                                                            }
                                                        })
                                                    }}
                                                />
                                                {"Incoming calls"}
                                            </label>
                                        </div>
                                    </div>
                                }
                            </div>
                        </div>

                        <div class="form-section">
                            <h4>{"Notification Rules"}</h4>

                            <div class="form-group">
                                <label>{"Mode"}</label>
                                <select
                                    value={(*form_mode).clone()}
                                    onchange={Callback::from({
                                        let form_mode = form_mode.clone();
                                        move |e: Event| {
                                            let target: HtmlSelectElement = e.target_unchecked_into();
                                            form_mode.set(target.value());
                                        }
                                    })}
                                >
                                    <option value="all" selected={*form_mode == "all"}>{"All messages"}</option>
                                    <option value="critical" selected={*form_mode == "critical"}>{"Critical only"}</option>
                                    <option value="digest" selected={*form_mode == "digest"}>{"Digest only"}</option>
                                </select>
                                <div class="field-help">
                                    {match (*form_mode).as_str() {
                                        "all" => "Notify immediately for every message from this contact.",
                                        "critical" => "AI filters messages - only urgent ones (needing action within 2 hours) notify instantly. Others go to digest.",
                                        "digest" => "No instant notifications. Messages appear in your daily digest summary.",
                                        _ => ""
                                    }}
                                </div>
                            </div>

                            <div class="form-group">
                                <label>{"Notify via"}</label>
                                <select
                                    value={(*form_type).clone()}
                                    onchange={Callback::from({
                                        let form_type = form_type.clone();
                                        move |e: Event| {
                                            let target: HtmlSelectElement = e.target_unchecked_into();
                                            form_type.set(target.value());
                                        }
                                    })}
                                >
                                    <option value="sms" selected={*form_type == "sms"}>{"SMS"}</option>
                                    <option value="call" selected={*form_type == "call"}>{"Phone Call"}</option>
                                    <option value="call_sms" selected={*form_type == "call_sms"}>{"Call + SMS"}</option>
                                </select>
                                <div class="field-help">
                                    {match (*form_type).as_str() {
                                        "sms" => "Text message with notification details.",
                                        "call" => "AI voice calls and reads the message to you.",
                                        "call_sms" => "Phone rings as loud alert + SMS backup. Only charged for call if answered.",
                                        _ => ""
                                    }}
                                </div>
                            </div>

                            <div class="form-group checkbox-group">
                                <label>
                                    <input
                                        type="checkbox"
                                        checked={*form_notify_call}
                                        onchange={Callback::from({
                                            let form_notify_call = form_notify_call.clone();
                                            move |e: Event| {
                                                let input: HtmlInputElement = e.target_unchecked_into();
                                                form_notify_call.set(input.checked());
                                            }
                                        })}
                                    />
                                    {" Alert on incoming calls"}
                                </label>
                                <div class="field-help">{"Get notified when this contact calls you on WhatsApp, Telegram, or Signal."}</div>
                            </div>
                        </div>

                        <div class="modal-actions">
                            <button class="cancel-btn" onclick={close_modal} disabled={matches!(*profile_save_state, FieldSaveState::Saving)}>{"Cancel"}</button>
                            <button class="save-btn" onclick={save_profile} disabled={matches!(*profile_save_state, FieldSaveState::Saving)}>
                                {if matches!(*profile_save_state, FieldSaveState::Saving) { "Saving..." } else { "Save Profile" }}
                                {render_save_indicator(&*profile_save_state)}
                            </button>
                        </div>
                    </div>
                </div>
            }
        </div>
    }
}
