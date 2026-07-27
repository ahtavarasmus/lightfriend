use super::light_phone_panel::LightPhonePanel;
use crate::profile::billing_models::UserProfile;
use crate::utils::api::Api;
use serde::Serialize;
use wasm_bindgen_futures::spawn_local;
use web_sys::{HtmlInputElement, HtmlSelectElement};
use yew::prelude::*;

const BYOT_NUMBER_OPTION: &str = "__byot";

const PHONE_DEVICE_STYLES: &str = r#"
.phone-device-panel {
    display: flex;
    flex-direction: column;
    padding: 0 0.9rem 0.25rem;
}
.connection-card {
    padding: 0;
    border: 0;
    background: transparent;
}
.connection-card + .connection-card {
    border-top: 1px solid rgba(255, 255, 255, 0.08);
}
.connection-field {
    display: grid;
    grid-template-columns: minmax(120px, 0.6fr) minmax(180px, 1fr);
    align-items: center;
    gap: 0.75rem 1.25rem;
    min-height: 64px;
    padding: 0.8rem 0;
}
.connection-field + .connection-field {
    border-top: 1px solid rgba(255, 255, 255, 0.08);
}
.connection-label-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
}
.connection-label {
    color: #ddd;
    font-size: 0.82rem;
    font-weight: 600;
}
.connection-value {
    color: #b9d3ff;
    font-size: 0.82rem;
    font-variant-numeric: tabular-nums;
    justify-self: end;
}
.connection-control {
    width: 100%;
    max-width: 300px;
    min-height: 40px;
    padding: 0.55rem 0.65rem;
    border: 1px solid rgba(255, 255, 255, 0.14);
    border-radius: 6px;
    background: transparent;
    color: #ddd;
    color-scheme: dark;
    font-size: 0.84rem;
    justify-self: end;
}
.connection-control option {
    background: #222;
    color: #ddd;
}
.connection-control:focus {
    border-color: rgba(126, 178, 255, 0.7);
    outline: 2px solid rgba(126, 178, 255, 0.16);
    outline-offset: 1px;
}
.connection-status {
    flex: 0 0 auto;
    min-width: 1rem;
    color: #78d69b;
    font-size: 0.78rem;
    text-align: right;
}
.connection-status.error {
    color: #f88;
}
.connection-error {
    margin: 0;
    color: #f88;
    font-size: 0.76rem;
    line-height: 1.4;
    grid-column: 2;
}
.connection-spinner {
    display: inline-block;
    width: 12px;
    height: 12px;
    border: 2px solid rgba(126, 178, 255, 0.25);
    border-top-color: #7eb2ff;
    border-radius: 50%;
    animation: connectionSpin 0.75s linear infinite;
}
@keyframes connectionSpin {
    to { transform: rotate(360deg); }
}
.byot-settings {
    display: flex;
    flex-direction: column;
    gap: 0.55rem;
    grid-column: 1 / -1;
    width: min(100%, 300px);
    margin-left: auto;
}
.connection-action {
    min-height: 40px;
    padding: 0.55rem 0.8rem;
    border: 1px solid rgba(126, 178, 255, 0.35);
    border-radius: 6px;
    background: rgba(126, 178, 255, 0.14);
    color: #b9d3ff;
    cursor: pointer;
    font-size: 0.82rem;
    font-weight: 600;
    transition: background 160ms ease, transform 160ms ease;
}
.connection-action:hover:not(:disabled) {
    background: rgba(126, 178, 255, 0.22);
}
.connection-action:active:not(:disabled) {
    transform: scale(0.98);
}
.connection-action:disabled {
    cursor: wait;
    opacity: 0.55;
}
.light-phone-control {
    min-width: 0;
}
@media (max-width: 560px) {
    .phone-device-panel {
        padding-left: 0.75rem;
        padding-right: 0.75rem;
    }
    .connection-field {
        grid-template-columns: 1fr;
        gap: 0.55rem;
        padding: 0.9rem 0;
    }
    .connection-control,
    .connection-value,
    .byot-settings {
        justify-self: stretch;
        max-width: none;
        margin-left: 0;
    }
    .connection-error {
        grid-column: 1;
    }
}
"#;

#[derive(Clone, PartialEq)]
enum SaveState {
    Idle,
    Saving,
    Success,
    Error(String),
}

#[derive(Serialize)]
struct PatchFieldRequest {
    field: String,
    value: serde_json::Value,
}

#[derive(Properties, PartialEq, Clone)]
pub struct PhoneDevicePanelProps {
    pub user_profile: UserProfile,
    pub on_profile_update: Callback<UserProfile>,
}

fn save_status(state: &SaveState) -> Html {
    match state {
        SaveState::Idle => html! {},
        SaveState::Saving => {
            html! { <span class="connection-status" aria-label="Saving"><span class="connection-spinner"></span></span> }
        }
        SaveState::Success => {
            html! { <span class="connection-status" role="status">{"Saved"}</span> }
        }
        SaveState::Error(message) => {
            html! { <span class="connection-status error" title={message.clone()}>{"Not saved"}</span> }
        }
    }
}

fn save_error(state: &SaveState) -> Html {
    match state {
        SaveState::Error(message) => {
            html! { <p class="connection-error" role="alert">{message}</p> }
        }
        _ => html! {},
    }
}

fn clear_success_later(state: UseStateHandle<SaveState>) {
    spawn_local(async move {
        gloo_timers::future::TimeoutFuture::new(3_000).await;
        state.set(SaveState::Idle);
    });
}

#[function_component(PhoneDevicePanel)]
pub fn phone_device_panel(props: &PhoneDevicePanelProps) -> Html {
    let user_profile = use_state(|| props.user_profile.clone());
    let show_number_selector = use_state(|| false);
    let available_numbers = use_state(Vec::<serde_json::Value>::new);
    let selected_number = use_state(|| {
        if props.user_profile.own_twilio_enabled {
            BYOT_NUMBER_OPTION.to_string()
        } else {
            props
                .user_profile
                .preferred_number
                .clone()
                .unwrap_or_default()
        }
    });
    let notification_type = use_state(|| {
        props
            .user_profile
            .notification_type
            .clone()
            .unwrap_or_else(|| "sms".to_string())
    });
    let byot_phone_number = use_state(|| {
        props
            .user_profile
            .preferred_number
            .clone()
            .unwrap_or_default()
    });
    let byot_account_sid = use_state(|| props.user_profile.twilio_sid.clone().unwrap_or_default());
    let byot_auth_token = use_state(|| props.user_profile.twilio_token.clone().unwrap_or_default());
    let number_save_state = use_state(|| SaveState::Idle);
    let notification_save_state = use_state(|| SaveState::Idle);

    {
        let user_profile = user_profile.clone();
        let selected_number = selected_number.clone();
        let notification_type = notification_type.clone();
        let byot_phone_number = byot_phone_number.clone();
        let byot_account_sid = byot_account_sid.clone();
        let byot_auth_token = byot_auth_token.clone();
        use_effect_with_deps(
            move |profile| {
                user_profile.set(profile.clone());
                selected_number.set(if profile.own_twilio_enabled {
                    BYOT_NUMBER_OPTION.to_string()
                } else {
                    profile.preferred_number.clone().unwrap_or_default()
                });
                notification_type.set(
                    profile
                        .notification_type
                        .clone()
                        .unwrap_or_else(|| "sms".to_string()),
                );
                byot_phone_number.set(profile.preferred_number.clone().unwrap_or_default());
                byot_account_sid.set(profile.twilio_sid.clone().unwrap_or_default());
                byot_auth_token.set(profile.twilio_token.clone().unwrap_or_default());
                || ()
            },
            props.user_profile.clone(),
        );
    }

    {
        let show_number_selector = show_number_selector.clone();
        let available_numbers = available_numbers.clone();
        let selected_number = selected_number.clone();
        use_effect_with_deps(
            move |_| {
                spawn_local(async move {
                    if let Ok(response) = Api::get("/api/profile/available-sending-numbers")
                        .send()
                        .await
                    {
                        if response.ok() {
                            if let Ok(data) = response.json::<serde_json::Value>().await {
                                show_number_selector.set(
                                    data.get("show_selector")
                                        .and_then(|value| value.as_bool())
                                        .unwrap_or(false),
                                );
                                if let Some(numbers) = data
                                    .get("available_numbers")
                                    .and_then(|value| value.as_array())
                                {
                                    available_numbers.set(numbers.clone());
                                }
                                let uses_own_twilio = data
                                    .get("own_twilio_enabled")
                                    .and_then(|value| value.as_bool())
                                    .unwrap_or(false);
                                if uses_own_twilio {
                                    selected_number.set(BYOT_NUMBER_OPTION.to_string());
                                } else if let Some(current) = data
                                    .get("current_preferred")
                                    .and_then(|value| value.as_str())
                                {
                                    selected_number.set(current.to_string());
                                }
                            }
                        }
                    }
                });
                || ()
            },
            (),
        );
    }

    let on_notification_change = {
        let notification_type = notification_type.clone();
        let save_state = notification_save_state.clone();
        let user_profile = user_profile.clone();
        let on_profile_update = props.on_profile_update.clone();
        Callback::from(move |event: Event| {
            let select: HtmlSelectElement = event.target_unchecked_into();
            let value = select.value();
            notification_type.set(value.clone());
            save_state.set(SaveState::Saving);

            let save_state = save_state.clone();
            let user_profile = user_profile.clone();
            let on_profile_update = on_profile_update.clone();
            spawn_local(async move {
                let request = PatchFieldRequest {
                    field: "notification_type".to_string(),
                    value: serde_json::Value::String(value.clone()),
                };
                match Api::patch("/api/profile/field").json(&request) {
                    Ok(request) => match request.send().await {
                        Ok(response) if response.ok() => {
                            let mut profile = (*user_profile).clone();
                            profile.notification_type = Some(value);
                            user_profile.set(profile.clone());
                            on_profile_update.emit(profile);
                            save_state.set(SaveState::Success);
                            clear_success_later(save_state);
                        }
                        Ok(_) => save_state
                            .set(SaveState::Error("Could not save alert method.".to_string())),
                        Err(_) => save_state.set(SaveState::Error("Network error.".to_string())),
                    },
                    Err(_) => save_state.set(SaveState::Error(
                        "Could not encode the request.".to_string(),
                    )),
                }
            });
        })
    };

    let on_number_change = {
        let selected_number = selected_number.clone();
        let save_state = number_save_state.clone();
        let user_profile = user_profile.clone();
        let on_profile_update = props.on_profile_update.clone();
        Callback::from(move |event: Event| {
            let select: HtmlSelectElement = event.target_unchecked_into();
            let value = select.value();
            selected_number.set(value.clone());
            if value == BYOT_NUMBER_OPTION {
                save_state.set(SaveState::Idle);
                return;
            }
            save_state.set(SaveState::Saving);

            let save_state = save_state.clone();
            let user_profile = user_profile.clone();
            let on_profile_update = on_profile_update.clone();
            spawn_local(async move {
                let request = PatchFieldRequest {
                    field: "preferred_number".to_string(),
                    value: serde_json::Value::String(value.clone()),
                };
                match Api::patch("/api/profile/field").json(&request) {
                    Ok(request) => match request.send().await {
                        Ok(response) if response.ok() => {
                            let mut profile = (*user_profile).clone();
                            profile.preferred_number = Some(value.clone());
                            profile.sms_from_number = Some(value);
                            profile.own_twilio_enabled = false;
                            user_profile.set(profile.clone());
                            on_profile_update.emit(profile);
                            save_state.set(SaveState::Success);
                            clear_success_later(save_state);
                        }
                        Ok(_) => save_state
                            .set(SaveState::Error("Could not save this number.".to_string())),
                        Err(_) => save_state.set(SaveState::Error("Network error.".to_string())),
                    },
                    Err(_) => save_state.set(SaveState::Error(
                        "Could not encode the request.".to_string(),
                    )),
                }
            });
        })
    };

    let on_save_byot = {
        let byot_phone_number = byot_phone_number.clone();
        let byot_account_sid = byot_account_sid.clone();
        let byot_auth_token = byot_auth_token.clone();
        let selected_number = selected_number.clone();
        let save_state = number_save_state.clone();
        let user_profile = user_profile.clone();
        let on_profile_update = props.on_profile_update.clone();
        Callback::from(move |_: MouseEvent| {
            let phone = (*byot_phone_number).trim().to_string();
            let sid = (*byot_account_sid).trim().to_string();
            let token = (*byot_auth_token).trim().to_string();
            let using_saved_credentials = sid.starts_with("...") && token.starts_with("...");

            if phone.is_empty()
                || !phone.starts_with('+')
                || phone.len() < 10
                || !phone[1..]
                    .chars()
                    .all(|character| character.is_ascii_digit())
                || phone.starts_with("...")
            {
                save_state.set(SaveState::Error(
                    "Enter the Twilio number in E.164 format.".to_string(),
                ));
                return;
            }
            if !using_saved_credentials
                && (sid.len() != 34
                    || !sid.starts_with("AC")
                    || !sid[2..]
                        .chars()
                        .all(|character| character.is_ascii_hexdigit()))
            {
                save_state.set(SaveState::Error(
                    "Enter a valid Twilio Account SID.".to_string(),
                ));
                return;
            }
            if !using_saved_credentials
                && (token.len() != 32
                    || !token.chars().all(|character| character.is_ascii_hexdigit()))
            {
                save_state.set(SaveState::Error(
                    "Enter a valid Twilio Auth Token.".to_string(),
                ));
                return;
            }

            save_state.set(SaveState::Saving);
            let selected_number = selected_number.clone();
            let save_state = save_state.clone();
            let user_profile = user_profile.clone();
            let on_profile_update = on_profile_update.clone();
            spawn_local(async move {
                let phone_result = Api::post("/api/profile/twilio-phone")
                    .header("Content-Type", "application/json")
                    .body(
                        serde_json::to_string(&serde_json::json!({ "twilio_phone": phone }))
                            .unwrap(),
                    )
                    .send()
                    .await;
                if !matches!(phone_result, Ok(ref response) if response.ok()) {
                    save_state.set(SaveState::Error(
                        "Could not save the Twilio number.".to_string(),
                    ));
                    return;
                }

                if !using_saved_credentials {
                    let credentials_result = Api::post("/api/profile/twilio-creds")
                        .header("Content-Type", "application/json")
                        .body(
                            serde_json::to_string(&serde_json::json!({
                                "account_sid": sid,
                                "auth_token": token
                            }))
                            .unwrap(),
                        )
                        .send()
                        .await;
                    if !matches!(credentials_result, Ok(ref response) if response.ok()) {
                        save_state.set(SaveState::Error(
                            "Could not save the Twilio credentials.".to_string(),
                        ));
                        return;
                    }
                }

                match Api::post("/api/profile/own-twilio")
                    .header("Content-Type", "application/json")
                    .body(serde_json::to_string(&serde_json::json!({ "enabled": true })).unwrap())
                    .send()
                    .await
                {
                    Ok(response) if response.ok() => {
                        selected_number.set(BYOT_NUMBER_OPTION.to_string());
                        let mut profile = (*user_profile).clone();
                        profile.preferred_number = Some(phone.clone());
                        profile.sms_from_number = Some(phone);
                        profile.own_twilio_enabled = true;
                        if !using_saved_credentials {
                            profile.twilio_sid = Some("...".to_string());
                            profile.twilio_token = Some("...".to_string());
                        }
                        user_profile.set(profile.clone());
                        on_profile_update.emit(profile);
                        save_state.set(SaveState::Success);
                        clear_success_later(save_state);
                    }
                    Ok(response) => {
                        let message = response
                            .json::<serde_json::Value>()
                            .await
                            .ok()
                            .and_then(|body| {
                                body.get("error")
                                    .and_then(|error| error.as_str())
                                    .map(str::to_string)
                            })
                            .unwrap_or_else(|| "Could not enable your Twilio number.".to_string());
                        save_state.set(SaveState::Error(message));
                    }
                    Err(_) => save_state.set(SaveState::Error("Network error.".to_string())),
                }
            });
        })
    };

    let shown_number = if user_profile.own_twilio_enabled {
        user_profile
            .preferred_number
            .clone()
            .unwrap_or_else(|| "Your Twilio number".to_string())
    } else {
        user_profile
            .sms_from_number
            .clone()
            .or_else(|| user_profile.preferred_number.clone())
            .unwrap_or_else(|| "Assigned automatically".to_string())
    };
    let show_byot_fields = *selected_number == BYOT_NUMBER_OPTION;

    html! {
        <>
            <style>{PHONE_DEVICE_STYLES}</style>
            <div class="phone-device-panel">
                <div class="connection-card">
                    <div class="connection-field">
                        <div class="connection-label-row">
                            <span class="connection-label">{"Lightfriend number"}</span>
                            {save_status(&number_save_state)}
                        </div>
                        {
                            if *show_number_selector {
                                let numbers = (*available_numbers).clone();
                                html! {
                                    <>
                                        <select
                                            class="connection-control"
                                            value={(*selected_number).clone()}
                                            onchange={on_number_change}
                                            aria-label="Lightfriend number"
                                        >
                                            {for numbers.iter().map(|item| {
                                                let number = item
                                                    .get("number")
                                                    .and_then(|value| value.as_str())
                                                    .unwrap_or("");
                                                let label = item
                                                    .get("label")
                                                    .and_then(|value| value.as_str())
                                                    .unwrap_or("Unknown");
                                                let option_label = if number.is_empty() {
                                                    label.to_string()
                                                } else {
                                                    format!("{label} ({number})")
                                                };
                                                html! {
                                                    <option
                                                        value={number.to_string()}
                                                        selected={*selected_number == number}
                                                    >
                                                        {option_label}
                                                    </option>
                                                }
                                            })}
                                            <option
                                                value={BYOT_NUMBER_OPTION}
                                                selected={show_byot_fields}
                                            >
                                                {"Bring your own Twilio number"}
                                            </option>
                                        </select>
                                        {
                                            if show_byot_fields {
                                                html! {
                                                    <div class="byot-settings">
                                                        <input
                                                            type="tel"
                                                            class="connection-control"
                                                            value={(*byot_phone_number).clone()}
                                                            placeholder="+1234567890"
                                                            aria-label="Twilio phone number"
                                                            oninput={{
                                                                let value = byot_phone_number.clone();
                                                                Callback::from(move |event: InputEvent| {
                                                                    let input: HtmlInputElement =
                                                                        event.target_unchecked_into();
                                                                    value.set(input.value());
                                                                })
                                                            }}
                                                        />
                                                        <input
                                                            type="text"
                                                            class="connection-control"
                                                            value={(*byot_account_sid).clone()}
                                                            placeholder="Twilio Account SID"
                                                            aria-label="Twilio Account SID"
                                                            oninput={{
                                                                let value = byot_account_sid.clone();
                                                                Callback::from(move |event: InputEvent| {
                                                                    let input: HtmlInputElement =
                                                                        event.target_unchecked_into();
                                                                    value.set(input.value());
                                                                })
                                                            }}
                                                        />
                                                        <input
                                                            type="password"
                                                            class="connection-control"
                                                            value={(*byot_auth_token).clone()}
                                                            placeholder="Twilio Auth Token"
                                                            aria-label="Twilio Auth Token"
                                                            oninput={{
                                                                let value = byot_auth_token.clone();
                                                                Callback::from(move |event: InputEvent| {
                                                                    let input: HtmlInputElement =
                                                                        event.target_unchecked_into();
                                                                    value.set(input.value());
                                                                })
                                                            }}
                                                        />
                                                        <button
                                                            class="connection-action"
                                                            onclick={on_save_byot}
                                                            disabled={*number_save_state == SaveState::Saving}
                                                        >
                                                            {"Connect Twilio number"}
                                                        </button>
                                                    </div>
                                                }
                                            } else {
                                                html! {}
                                            }
                                        }
                                    </>
                                }
                            } else {
                                html! { <span class="connection-value">{shown_number}</span> }
                            }
                        }
                        {save_error(&number_save_state)}
                    </div>

                    <div class="connection-field">
                        <div class="connection-label-row">
                            <span class="connection-label">{"Alerts"}</span>
                            {save_status(&notification_save_state)}
                        </div>
                        <select
                            class="connection-control"
                            value={(*notification_type).clone()}
                            onchange={on_notification_change}
                            aria-label="Alert method"
                        >
                            <option value="sms" selected={*notification_type == "sms"}>
                                {"Text me"}
                            </option>
                            <option value="call" selected={*notification_type == "call"}>
                                {"Call me"}
                            </option>
                        </select>
                        {save_error(&notification_save_state)}
                    </div>
                </div>

                <div class="connection-card">
                    <div class="connection-field">
                        <span class="connection-label">{"Light Phone"}</span>
                        <div class="light-phone-control">
                            <LightPhonePanel />
                        </div>
                    </div>
                </div>
            </div>
        </>
    }
}
