use crate::utils::api::Api;
use crate::utils::webauthn;
use serde::{Deserialize, Serialize};
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use yew::prelude::*;

#[derive(Clone, PartialEq, Debug)]
pub enum TwoFactorState {
    Loading,
    Disabled,
    Enabled { remaining_backup_codes: i64 },
    Setting { qr_code_url: String, secret: String },
    ShowingBackupCodes { codes: Vec<String> },
    Error(String),
}

#[derive(Clone, PartialEq, Debug)]
pub enum PasskeyState {
    Loading,
    Ready,
    Registering { device_name: String },
}

#[derive(Deserialize, Debug, Clone)]
pub struct TotpStatusResponse {
    pub enabled: bool,
    pub remaining_backup_codes: i64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct TotpSetupResponse {
    pub qr_code_data_url: String,
    pub secret: String,
}

#[derive(Serialize)]
pub struct TotpVerifyRequest {
    pub code: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct TotpVerifyResponse {
    pub backup_codes: Vec<String>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct RegenerateBackupCodesResponse {
    pub backup_codes: Vec<String>,
}

// Passkey types - matches backend's PasskeyInfo
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct Passkey {
    pub credential_id: String,
    pub device_name: String,
    pub created_at: i32,
    pub last_used_at: Option<i32>,
}

#[derive(Serialize)]
pub struct RegisterPasskeyStartRequest {
    pub device_name: String,
}

#[derive(Serialize)]
pub struct RegisterPasskeyFinishRequest {
    pub device_name: String,
    pub response: serde_json::Value,
}

#[derive(Serialize)]
pub struct DeletePasskeyRequest {
    pub credential_id: String,
}

#[derive(Serialize)]
pub struct RenamePasskeyRequest {
    pub credential_id: String,
    pub new_name: String,
}

#[function_component]
pub fn SecuritySettings() -> Html {
    // TOTP state
    let state = use_state(|| TwoFactorState::Loading);
    let verification_code = use_state(String::new);
    let disable_code = use_state(String::new);
    let regenerate_code = use_state(String::new);
    let show_secret = use_state(|| false);
    let error_message = use_state(|| None::<String>);
    let is_saving = use_state(|| false);
    let show_disable_modal = use_state(|| false);
    let show_regenerate_modal = use_state(|| false);
    let codes_copied = use_state(|| false);

    // Passkey state
    let passkey_state = use_state(|| PasskeyState::Loading);
    let passkeys = use_state(Vec::<Passkey>::new);
    let new_passkey_name = use_state(String::new);
    let passkey_error = use_state(|| None::<String>);
    let passkey_loading = use_state(|| false);
    let editing_passkey_id = use_state(|| None::<String>);
    let editing_passkey_name = use_state(String::new);
    let delete_passkey_id = use_state(|| None::<String>);
    let webauthn_supported = use_state(|| webauthn::is_webauthn_supported());

    // Logout state
    let logout_loading = use_state(|| false);

    // Load TOTP status on mount
    {
        let state = state.clone();
        use_effect_with_deps(
            move |_| {
                spawn_local(async move {
                    match Api::get("/api/totp/status").send().await {
                        Ok(resp) if resp.ok() => {
                            if let Ok(status) = resp.json::<TotpStatusResponse>().await {
                                if status.enabled {
                                    state.set(TwoFactorState::Enabled {
                                        remaining_backup_codes: status.remaining_backup_codes,
                                    });
                                } else {
                                    state.set(TwoFactorState::Disabled);
                                }
                            }
                        }
                        _ => {
                            state.set(TwoFactorState::Error(
                                "Failed to load 2FA status".to_string(),
                            ));
                        }
                    }
                });
                || ()
            },
            (),
        );
    }

    // Load passkeys on mount
    {
        let passkey_state = passkey_state.clone();
        let passkeys = passkeys.clone();
        use_effect_with_deps(
            move |_| {
                spawn_local(async move {
                    match Api::get("/api/webauthn/passkeys").send().await {
                        Ok(resp) if resp.ok() => {
                            // Backend returns Vec<PasskeyInfo> directly (not wrapped)
                            if let Ok(list) = resp.json::<Vec<Passkey>>().await {
                                passkeys.set(list);
                                passkey_state.set(PasskeyState::Ready);
                            } else {
                                passkey_state.set(PasskeyState::Ready);
                            }
                        }
                        _ => {
                            passkey_state.set(PasskeyState::Ready);
                        }
                    }
                });
                || ()
            },
            (),
        );
    }

    // Start 2FA setup
    let on_enable_click = {
        let state = state.clone();
        let error_message = error_message.clone();
        Callback::from(move |_: MouseEvent| {
            let state = state.clone();
            let error_message = error_message.clone();
            spawn_local(async move {
                match Api::post("/api/totp/setup/start")
                    .header("Content-Type", "application/json")
                    .body("{}")
                    .send()
                    .await
                {
                    Ok(resp) if resp.ok() => {
                        if let Ok(setup) = resp.json::<TotpSetupResponse>().await {
                            state.set(TwoFactorState::Setting {
                                qr_code_url: setup.qr_code_data_url,
                                secret: setup.secret,
                            });
                        }
                    }
                    Ok(resp) => {
                        error_message.set(Some(format!(
                            "Failed to start 2FA setup: {}",
                            resp.status()
                        )));
                    }
                    Err(e) => {
                        error_message.set(Some(format!("Network error: {:?}", e)));
                    }
                }
            });
        })
    };

    // Verify setup code
    let on_verify_setup = {
        let state = state.clone();
        let verification_code = verification_code.clone();
        let error_message = error_message.clone();
        let is_saving = is_saving.clone();
        Callback::from(move |_: MouseEvent| {
            let code = (*verification_code).clone();
            if code.len() != 6 {
                error_message.set(Some("Please enter a 6-digit code".to_string()));
                return;
            }
            let state = state.clone();
            let error_message = error_message.clone();
            let is_saving = is_saving.clone();
            let verification_code = verification_code.clone();
            is_saving.set(true);
            spawn_local(async move {
                let request = TotpVerifyRequest { code };
                match Api::post("/api/totp/setup/verify")
                    .json(&request)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(resp) if resp.ok() => {
                        if let Ok(result) = resp.json::<TotpVerifyResponse>().await {
                            verification_code.set(String::new());
                            state.set(TwoFactorState::ShowingBackupCodes {
                                codes: result.backup_codes,
                            });
                        }
                    }
                    Ok(_) => {
                        error_message.set(Some("Invalid verification code".to_string()));
                    }
                    Err(e) => {
                        error_message.set(Some(format!("Network error: {:?}", e)));
                    }
                }
                is_saving.set(false);
            });
        })
    };

    // Finish viewing backup codes
    let on_backup_codes_done = {
        let state = state.clone();
        Callback::from(move |_: MouseEvent| {
            state.set(TwoFactorState::Enabled {
                remaining_backup_codes: 10,
            });
        })
    };

    // Cancel setup
    let on_cancel_setup = {
        let state = state.clone();
        let verification_code = verification_code.clone();
        let show_secret = show_secret.clone();
        Callback::from(move |_: MouseEvent| {
            verification_code.set(String::new());
            show_secret.set(false);
            state.set(TwoFactorState::Disabled);
        })
    };

    // Show disable modal
    let on_disable_click = {
        let show_disable_modal = show_disable_modal.clone();
        Callback::from(move |_: MouseEvent| {
            show_disable_modal.set(true);
        })
    };

    // Cancel disable
    let on_cancel_disable = {
        let show_disable_modal = show_disable_modal.clone();
        let disable_code = disable_code.clone();
        Callback::from(move |_: MouseEvent| {
            disable_code.set(String::new());
            show_disable_modal.set(false);
        })
    };

    // Confirm disable
    let on_confirm_disable = {
        let state = state.clone();
        let disable_code = disable_code.clone();
        let show_disable_modal = show_disable_modal.clone();
        let error_message = error_message.clone();
        let is_saving = is_saving.clone();
        Callback::from(move |_: MouseEvent| {
            let code = (*disable_code).clone();
            if code.is_empty() {
                error_message.set(Some("Please enter your 2FA code".to_string()));
                return;
            }
            let state = state.clone();
            let disable_code = disable_code.clone();
            let show_disable_modal = show_disable_modal.clone();
            let error_message = error_message.clone();
            let is_saving = is_saving.clone();
            is_saving.set(true);
            spawn_local(async move {
                let request = TotpVerifyRequest { code };
                match Api::post("/api/totp/disable")
                    .json(&request)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(resp) if resp.ok() => {
                        disable_code.set(String::new());
                        show_disable_modal.set(false);
                        state.set(TwoFactorState::Disabled);
                    }
                    Ok(_) => {
                        error_message.set(Some("Invalid code".to_string()));
                    }
                    Err(e) => {
                        error_message.set(Some(format!("Network error: {:?}", e)));
                    }
                }
                is_saving.set(false);
            });
        })
    };

    // Show regenerate modal
    let on_regenerate_click = {
        let show_regenerate_modal = show_regenerate_modal.clone();
        Callback::from(move |_: MouseEvent| {
            show_regenerate_modal.set(true);
        })
    };

    // Cancel regenerate
    let on_cancel_regenerate = {
        let show_regenerate_modal = show_regenerate_modal.clone();
        let regenerate_code = regenerate_code.clone();
        Callback::from(move |_: MouseEvent| {
            regenerate_code.set(String::new());
            show_regenerate_modal.set(false);
        })
    };

    // Confirm regenerate
    let on_confirm_regenerate = {
        let state = state.clone();
        let regenerate_code = regenerate_code.clone();
        let show_regenerate_modal = show_regenerate_modal.clone();
        let error_message = error_message.clone();
        let is_saving = is_saving.clone();
        Callback::from(move |_: MouseEvent| {
            let code = (*regenerate_code).clone();
            if code.is_empty() {
                error_message.set(Some("Please enter your 2FA code".to_string()));
                return;
            }
            let state = state.clone();
            let regenerate_code = regenerate_code.clone();
            let show_regenerate_modal = show_regenerate_modal.clone();
            let error_message = error_message.clone();
            let is_saving = is_saving.clone();
            is_saving.set(true);
            spawn_local(async move {
                let request = TotpVerifyRequest { code };
                match Api::post("/api/totp/backup-codes/regenerate")
                    .json(&request)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(resp) if resp.ok() => {
                        if let Ok(result) = resp.json::<RegenerateBackupCodesResponse>().await {
                            regenerate_code.set(String::new());
                            show_regenerate_modal.set(false);
                            state.set(TwoFactorState::ShowingBackupCodes {
                                codes: result.backup_codes,
                            });
                        }
                    }
                    Ok(_) => {
                        error_message.set(Some("Invalid code".to_string()));
                    }
                    Err(e) => {
                        error_message.set(Some(format!("Network error: {:?}", e)));
                    }
                }
                is_saving.set(false);
            });
        })
    };

    // Copy backup codes to clipboard
    let on_copy_codes = {
        let codes_copied = codes_copied.clone();
        let state = state.clone();
        Callback::from(move |_: MouseEvent| {
            if let TwoFactorState::ShowingBackupCodes { codes } = &*state {
                let codes_text = codes.join("\n");
                if let Some(window) = web_sys::window() {
                    let navigator = window.navigator();
                    let clipboard = navigator.clipboard();
                    let codes_copied = codes_copied.clone();
                    spawn_local(async move {
                        let _ =
                            wasm_bindgen_futures::JsFuture::from(clipboard.write_text(&codes_text))
                                .await;
                        codes_copied.set(true);
                    });
                }
            }
        })
    };

    // Toggle show secret
    let on_toggle_secret = {
        let show_secret = show_secret.clone();
        Callback::from(move |_: MouseEvent| {
            show_secret.set(!*show_secret);
        })
    };

    // Clear error
    let on_clear_error = {
        let error_message = error_message.clone();
        Callback::from(move |_: MouseEvent| {
            error_message.set(None);
        })
    };

    // Passkey callbacks

    // Start registering a passkey
    let on_add_passkey_click = {
        let passkey_state = passkey_state.clone();
        let new_passkey_name = new_passkey_name.clone();
        Callback::from(move |_: MouseEvent| {
            new_passkey_name.set(String::new());
            passkey_state.set(PasskeyState::Registering {
                device_name: String::new(),
            });
        })
    };

    // Cancel passkey registration
    let on_cancel_passkey = {
        let passkey_state = passkey_state.clone();
        let new_passkey_name = new_passkey_name.clone();
        Callback::from(move |_: MouseEvent| {
            new_passkey_name.set(String::new());
            passkey_state.set(PasskeyState::Ready);
        })
    };

    // Register passkey
    let on_register_passkey = {
        let passkey_state = passkey_state.clone();
        let passkeys = passkeys.clone();
        let new_passkey_name = new_passkey_name.clone();
        let passkey_error = passkey_error.clone();
        let passkey_loading = passkey_loading.clone();
        Callback::from(move |_: MouseEvent| {
            let device_name = (*new_passkey_name).clone();
            if device_name.is_empty() {
                passkey_error.set(Some("Please enter a device name".to_string()));
                return;
            }

            let passkey_state = passkey_state.clone();
            let passkeys = passkeys.clone();
            let new_passkey_name = new_passkey_name.clone();
            let passkey_error = passkey_error.clone();
            let passkey_loading = passkey_loading.clone();
            passkey_loading.set(true);
            passkey_error.set(None);

            spawn_local(async move {
                // Step 1: Start registration to get options from server
                let start_request = RegisterPasskeyStartRequest {
                    device_name: device_name.clone(),
                };

                let options_result = match Api::post("/api/webauthn/register/start")
                    .json(&start_request)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(resp) if resp.ok() => {
                        match resp.json::<serde_json::Value>().await {
                            Ok(json) => {
                                // The backend returns { "options": { "publicKey": { ... } } }
                                // Extract the inner "options" field for the WebAuthn API
                                if let Some(opts) = json.get("options") {
                                    Ok(opts.clone())
                                } else {
                                    // Fallback: maybe the response is already the options directly
                                    Ok(json)
                                }
                            }
                            Err(e) => Err(format!("Failed to parse options: {:?}", e)),
                        }
                    }
                    Ok(resp) => Err(format!("Server error: {}", resp.status())),
                    Err(e) => Err(format!("Network error: {:?}", e)),
                };

                let options = match options_result {
                    Ok(o) => o,
                    Err(e) => {
                        passkey_error.set(Some(e));
                        passkey_loading.set(false);
                        return;
                    }
                };

                // Step 2: Create credential with browser WebAuthn API
                let credential = match webauthn::create_credential(&options).await {
                    Ok(c) => c,
                    Err(e) => {
                        passkey_error.set(Some(format!("WebAuthn error: {}", e)));
                        passkey_loading.set(false);
                        return;
                    }
                };

                // Step 3: Send credential to server to finish registration
                let finish_request = RegisterPasskeyFinishRequest {
                    device_name: device_name.clone(),
                    response: credential,
                };

                match Api::post("/api/webauthn/register/finish")
                    .json(&finish_request)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(resp) if resp.ok() => {
                        // Reload passkey list
                        if let Ok(list_resp) = Api::get("/api/webauthn/passkeys").send().await {
                            if let Ok(list) = list_resp.json::<Vec<Passkey>>().await {
                                passkeys.set(list);
                            }
                        }
                        new_passkey_name.set(String::new());
                        passkey_state.set(PasskeyState::Ready);
                    }
                    Ok(resp) => {
                        passkey_error.set(Some(format!("Failed to register: {}", resp.status())));
                    }
                    Err(e) => {
                        passkey_error.set(Some(format!("Network error: {:?}", e)));
                    }
                }
                passkey_loading.set(false);
            });
        })
    };

    // Delete passkey
    let on_confirm_delete_passkey = {
        let passkeys = passkeys.clone();
        let delete_passkey_id = delete_passkey_id.clone();
        let passkey_error = passkey_error.clone();
        let passkey_loading = passkey_loading.clone();
        Callback::from(move |_: MouseEvent| {
            let credential_id = match (*delete_passkey_id).clone() {
                Some(id) => id,
                None => return,
            };

            let passkeys = passkeys.clone();
            let delete_passkey_id = delete_passkey_id.clone();
            let passkey_error = passkey_error.clone();
            let passkey_loading = passkey_loading.clone();
            passkey_loading.set(true);

            spawn_local(async move {
                let cred_id_clone = credential_id.clone();
                let request = DeletePasskeyRequest { credential_id };
                match Api::delete("/api/webauthn/passkey")
                    .json(&request)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(resp) if resp.ok() => {
                        // Remove from local list
                        let current = (*passkeys).clone();
                        passkeys.set(
                            current
                                .into_iter()
                                .filter(|p| p.credential_id != cred_id_clone)
                                .collect(),
                        );
                        delete_passkey_id.set(None);
                    }
                    Ok(resp) => {
                        passkey_error.set(Some(format!("Failed to delete: {}", resp.status())));
                    }
                    Err(e) => {
                        passkey_error.set(Some(format!("Network error: {:?}", e)));
                    }
                }
                passkey_loading.set(false);
            });
        })
    };

    // Rename passkey
    let on_confirm_rename_passkey = {
        let passkeys = passkeys.clone();
        let editing_passkey_id = editing_passkey_id.clone();
        let editing_passkey_name = editing_passkey_name.clone();
        let passkey_error = passkey_error.clone();
        let passkey_loading = passkey_loading.clone();
        Callback::from(move |_: MouseEvent| {
            let credential_id = match (*editing_passkey_id).clone() {
                Some(id) => id,
                None => return,
            };
            let new_name = (*editing_passkey_name).clone();
            if new_name.is_empty() {
                passkey_error.set(Some("Device name cannot be empty".to_string()));
                return;
            }

            let passkeys = passkeys.clone();
            let editing_passkey_id = editing_passkey_id.clone();
            let editing_passkey_name = editing_passkey_name.clone();
            let passkey_error = passkey_error.clone();
            let passkey_loading = passkey_loading.clone();
            passkey_loading.set(true);

            spawn_local(async move {
                let cred_id_clone = credential_id.clone();
                let request = RenamePasskeyRequest {
                    credential_id,
                    new_name: new_name.clone(),
                };
                match Api::patch("/api/webauthn/passkey/rename")
                    .json(&request)
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(resp) if resp.ok() => {
                        // Update local list
                        let current = (*passkeys).clone();
                        passkeys.set(
                            current
                                .into_iter()
                                .map(|mut p| {
                                    if p.credential_id == cred_id_clone {
                                        p.device_name = new_name.clone();
                                    }
                                    p
                                })
                                .collect(),
                        );
                        editing_passkey_id.set(None);
                        editing_passkey_name.set(String::new());
                    }
                    Ok(resp) => {
                        passkey_error.set(Some(format!("Failed to rename: {}", resp.status())));
                    }
                    Err(e) => {
                        passkey_error.set(Some(format!("Network error: {:?}", e)));
                    }
                }
                passkey_loading.set(false);
            });
        })
    };

    // Clear passkey error
    let on_clear_passkey_error = {
        let passkey_error = passkey_error.clone();
        Callback::from(move |_: MouseEvent| {
            passkey_error.set(None);
        })
    };

    // Logout handler
    let on_logout_click = {
        let logout_loading = logout_loading.clone();
        Callback::from(move |_: MouseEvent| {
            let logout_loading = logout_loading.clone();
            logout_loading.set(true);
            spawn_local(async move {
                // Call backend logout endpoint to clear cookies
                let _ = Api::post("/api/logout").send().await;

                // Reload the page to reset state
                if let Some(window) = web_sys::window() {
                    let _ = window.location().reload();
                }
            });
        })
    };

    // Format timestamp
    let format_time = |ts: i32| -> String {
        let date = chrono::DateTime::from_timestamp(ts as i64, 0)
            .map(|d| d.format("%b %d, %Y").to_string())
            .unwrap_or_else(|| "Unknown".to_string());
        date
    };

    // Check if TOTP is enabled - required before passkeys can be added
    let totp_is_enabled = if let TwoFactorState::Enabled { .. } = &*state {
        true
    } else {
        false
    };

    html! {
        <div class="security-container">
            // TOTP Section
            <div class="security-settings">
                <h3 class="security-title">{"Authenticator App (TOTP)"}</h3>

                // Error message
                {
                    if let Some(error) = &*error_message {
                        html! {
                            <div class="security-error">
                                <span>{error}</span>
                                <button onclick={on_clear_error}>{"×"}</button>
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }

                // Main content based on state
                {
                    match &*state {
                        TwoFactorState::Loading => html! {
                            <div class="security-loading">{"Loading..."}</div>
                        },
                        TwoFactorState::Disabled => html! {
                            <div class="security-disabled">
                                <p class="security-description">
                                    {"Protect your account with two-factor authentication. "}
                                    {"You'll need an authenticator app like Google Authenticator or Authy."}
                                </p>
                                <button class="security-btn primary" onclick={on_enable_click}>
                                    {"Enable 2FA"}
                                </button>
                            </div>
                        },
                        TwoFactorState::Enabled { remaining_backup_codes } => html! {
                            <div class="security-enabled">
                                <div class="security-status">
                                    <span class="status-badge enabled">{"Enabled"}</span>
                                    <span class="backup-count">
                                        {format!("{} backup codes remaining", remaining_backup_codes)}
                                    </span>
                                </div>
                                <div class="security-actions">
                                    <button class="security-btn secondary" onclick={on_regenerate_click}>
                                        {"Regenerate Backup Codes"}
                                    </button>
                                    <button class="security-btn danger" onclick={on_disable_click}>
                                        {"Disable 2FA"}
                                    </button>
                                </div>
                            </div>
                        },
                        TwoFactorState::Setting { qr_code_url, secret } => html! {
                            <div class="security-setup">
                                <p class="setup-instruction">
                                    {"Scan this QR code with your authenticator app:"}
                                </p>
                                <img class="qr-code" src={qr_code_url.clone()} alt="QR Code" />

                                <div class="secret-section">
                                    <button class="show-secret-btn" onclick={on_toggle_secret.clone()}>
                                        {if *show_secret { "Hide secret key" } else { "Can't scan? Show secret key" }}
                                    </button>
                                    {
                                        if *show_secret {
                                            html! {
                                                <div class="secret-display">
                                                    <code>{secret.clone()}</code>
                                                </div>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }
                                </div>

                                <div class="verify-section">
                                    <label>{"Enter the 6-digit code from your app:"}</label>
                                    <input
                                        type="text"
                                        class="verify-input"
                                        maxlength="6"
                                        placeholder="000000"
                                        value={(*verification_code).clone()}
                                        oninput={
                                            let verification_code = verification_code.clone();
                                            move |e: InputEvent| {
                                                let input: HtmlInputElement = e.target_unchecked_into();
                                                let value = input.value().chars().filter(|c| c.is_numeric()).collect::<String>();
                                                verification_code.set(value);
                                            }
                                        }
                                    />
                                </div>

                                <div class="setup-buttons">
                                    <button class="security-btn secondary" onclick={on_cancel_setup}>
                                        {"Cancel"}
                                    </button>
                                    <button
                                        class="security-btn primary"
                                        onclick={on_verify_setup}
                                        disabled={*is_saving || verification_code.len() != 6}
                                    >
                                        {if *is_saving { "Verifying..." } else { "Verify & Enable" }}
                                    </button>
                                </div>
                            </div>
                        },
                        TwoFactorState::ShowingBackupCodes { codes } => html! {
                            <div class="backup-codes-display">
                                <h4>{"Save Your Backup Codes"}</h4>
                                <p class="backup-warning">
                                    {"These codes can be used to access your account if you lose your authenticator. "}
                                    {"Each code can only be used once. Save them somewhere safe!"}
                                </p>
                                <div class="backup-codes-grid">
                                    {
                                        codes.iter().map(|code| {
                                            html! { <code class="backup-code">{code}</code> }
                                        }).collect::<Html>()
                                    }
                                </div>
                                <div class="backup-codes-actions">
                                    <button class="security-btn secondary" onclick={on_copy_codes.clone()}>
                                        {if *codes_copied { "Copied!" } else { "Copy All" }}
                                    </button>
                                    <button class="security-btn primary" onclick={on_backup_codes_done}>
                                        {"I've Saved These Codes"}
                                    </button>
                                </div>
                            </div>
                        },
                        TwoFactorState::Error(msg) => html! {
                            <div class="security-error-state">
                                <p>{msg}</p>
                                <button class="security-btn primary" onclick={on_enable_click}>
                                    {"Retry"}
                                </button>
                            </div>
                        },
                    }
                }

                // Disable modal
                {
                    if *show_disable_modal {
                        html! {
                            <div class="modal-overlay">
                                <div class="modal-content">
                                    <h4>{"Disable Two-Factor Authentication"}</h4>
                                    <p>{"Enter your current 2FA code to disable:"}</p>
                                    <input
                                        type="text"
                                        class="verify-input"
                                        maxlength="6"
                                        placeholder="000000"
                                        value={(*disable_code).clone()}
                                        oninput={
                                            let disable_code = disable_code.clone();
                                            move |e: InputEvent| {
                                                let input: HtmlInputElement = e.target_unchecked_into();
                                                let value = input.value().chars().filter(|c| c.is_numeric()).collect::<String>();
                                                disable_code.set(value);
                                            }
                                        }
                                    />
                                    <div class="modal-buttons">
                                        <button class="security-btn secondary" onclick={on_cancel_disable}>
                                            {"Cancel"}
                                        </button>
                                        <button
                                            class="security-btn danger"
                                            onclick={on_confirm_disable}
                                            disabled={*is_saving}
                                        >
                                            {if *is_saving { "Disabling..." } else { "Disable 2FA" }}
                                        </button>
                                    </div>
                                </div>
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }

                // Regenerate modal
                {
                    if *show_regenerate_modal {
                        html! {
                            <div class="modal-overlay">
                                <div class="modal-content">
                                    <h4>{"Regenerate Backup Codes"}</h4>
                                    <p>{"This will invalidate your old backup codes. Enter your current 2FA code to continue:"}</p>
                                    <input
                                        type="text"
                                        class="verify-input"
                                        maxlength="6"
                                        placeholder="000000"
                                        value={(*regenerate_code).clone()}
                                        oninput={
                                            let regenerate_code = regenerate_code.clone();
                                            move |e: InputEvent| {
                                                let input: HtmlInputElement = e.target_unchecked_into();
                                                let value = input.value().chars().filter(|c| c.is_numeric()).collect::<String>();
                                                regenerate_code.set(value);
                                            }
                                        }
                                    />
                                    <div class="modal-buttons">
                                        <button class="security-btn secondary" onclick={on_cancel_regenerate}>
                                            {"Cancel"}
                                        </button>
                                        <button
                                            class="security-btn primary"
                                            onclick={on_confirm_regenerate}
                                            disabled={*is_saving}
                                        >
                                            {if *is_saving { "Regenerating..." } else { "Regenerate Codes" }}
                                        </button>
                                    </div>
                                </div>
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }
            </div>

            // Passkeys Section
            <div class="security-settings passkeys-section">
                <h3 class="security-title">{"Passkeys (Touch ID / Face ID)"}</h3>

                // Passkey error message
                {
                    if let Some(error) = &*passkey_error {
                        html! {
                            <div class="security-error">
                                <span>{error}</span>
                                <button onclick={on_clear_passkey_error}>{"×"}</button>
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }

                {
                    if !totp_is_enabled {
                        html! {
                            <div class="totp-required-notice">
                                <p>{"You must enable the authenticator app (TOTP) above before adding passkeys."}</p>
                                <p class="notice-reason">{"This ensures you have a fallback authentication method if you lose access to your passkey device."}</p>
                            </div>
                        }
                    } else if !*webauthn_supported {
                        html! {
                            <div class="webauthn-unsupported">
                                <p>{"Your browser doesn't support passkeys. Please use a modern browser like Chrome, Safari, or Firefox."}</p>
                            </div>
                        }
                    } else {
                        match &*passkey_state {
                            PasskeyState::Loading => html! {
                                <div class="security-loading">{"Loading passkeys..."}</div>
                            },
                            PasskeyState::Ready => html! {
                                <div class="passkeys-ready">
                                    <p class="security-description">
                                        {"Use Touch ID, Face ID, or your device's biometric authentication as a second factor. "}
                                        {"This is required for sensitive actions like unlocking your Tesla."}
                                    </p>

                                    // List of passkeys
                                    {
                                        if passkeys.is_empty() {
                                            html! {
                                                <p class="no-passkeys">{"No passkeys registered yet."}</p>
                                            }
                                        } else {
                                            html! {
                                                <div class="passkey-list">
                                                    {
                                                        passkeys.iter().map(|passkey| {
                                                            let pk_id = passkey.credential_id.clone();
                                                            let is_editing = *editing_passkey_id == Some(pk_id.clone());
                                                            let is_deleting = *delete_passkey_id == Some(pk_id.clone());

                                                            if is_editing {
                                                                let editing_passkey_name = editing_passkey_name.clone();
                                                                let editing_passkey_id = editing_passkey_id.clone();
                                                                let on_confirm_rename_passkey = on_confirm_rename_passkey.clone();
                                                                html! {
                                                                    <div class="passkey-item editing">
                                                                        <input
                                                                            type="text"
                                                                            class="passkey-name-input"
                                                                            value={(*editing_passkey_name).clone()}
                                                                            oninput={
                                                                                let editing_passkey_name = editing_passkey_name.clone();
                                                                                move |e: InputEvent| {
                                                                                    let input: HtmlInputElement = e.target_unchecked_into();
                                                                                    editing_passkey_name.set(input.value());
                                                                                }
                                                                            }
                                                                        />
                                                                        <div class="passkey-actions">
                                                                            <button
                                                                                class="passkey-btn save"
                                                                                onclick={on_confirm_rename_passkey}
                                                                                disabled={*passkey_loading}
                                                                            >
                                                                                {"Save"}
                                                                            </button>
                                                                            <button
                                                                                class="passkey-btn cancel"
                                                                                onclick={
                                                                                    let editing_passkey_id = editing_passkey_id.clone();
                                                                                    move |_: MouseEvent| {
                                                                                        editing_passkey_id.set(None);
                                                                                    }
                                                                                }
                                                                            >
                                                                                {"Cancel"}
                                                                            </button>
                                                                        </div>
                                                                    </div>
                                                                }
                                                            } else if is_deleting {
                                                                let delete_passkey_id = delete_passkey_id.clone();
                                                                let on_confirm_delete_passkey = on_confirm_delete_passkey.clone();
                                                                html! {
                                                                    <div class="passkey-item deleting">
                                                                        <span class="passkey-name">{&passkey.device_name}</span>
                                                                        <span class="delete-confirm">{"Delete this passkey?"}</span>
                                                                        <div class="passkey-actions">
                                                                            <button
                                                                                class="passkey-btn danger"
                                                                                onclick={on_confirm_delete_passkey}
                                                                                disabled={*passkey_loading}
                                                                            >
                                                                                {"Yes, Delete"}
                                                                            </button>
                                                                            <button
                                                                                class="passkey-btn cancel"
                                                                                onclick={
                                                                                    let delete_passkey_id = delete_passkey_id.clone();
                                                                                    move |_: MouseEvent| {
                                                                                        delete_passkey_id.set(None);
                                                                                    }
                                                                                }
                                                                            >
                                                                                {"Cancel"}
                                                                            </button>
                                                                        </div>
                                                                    </div>
                                                                }
                                                            } else {
                                                                let editing_passkey_id = editing_passkey_id.clone();
                                                                let editing_passkey_name = editing_passkey_name.clone();
                                                                let delete_passkey_id = delete_passkey_id.clone();
                                                                let device_name = passkey.device_name.clone();
                                                                html! {
                                                                    <div class="passkey-item">
                                                                        <div class="passkey-info">
                                                                            <span class="passkey-name">{&passkey.device_name}</span>
                                                                            <span class="passkey-meta">
                                                                                {"Added "}{format_time(passkey.created_at)}
                                                                                {
                                                                                    if let Some(last_used) = passkey.last_used_at {
                                                                                        format!(" • Last used {}", format_time(last_used))
                                                                                    } else {
                                                                                        String::new()
                                                                                    }
                                                                                }
                                                                            </span>
                                                                        </div>
                                                                        <div class="passkey-actions">
                                                                            <button
                                                                                class="passkey-btn edit"
                                                                                onclick={
                                                                                    let editing_passkey_id = editing_passkey_id.clone();
                                                                                    let editing_passkey_name = editing_passkey_name.clone();
                                                                                    let device_name = device_name.clone();
                                                                                    let pk_id = pk_id.clone();
                                                                                    move |_: MouseEvent| {
                                                                                        editing_passkey_id.set(Some(pk_id.clone()));
                                                                                        editing_passkey_name.set(device_name.clone());
                                                                                    }
                                                                                }
                                                                            >
                                                                                {"Rename"}
                                                                            </button>
                                                                            <button
                                                                                class="passkey-btn delete"
                                                                                onclick={
                                                                                    let delete_passkey_id = delete_passkey_id.clone();
                                                                                    let pk_id = pk_id.clone();
                                                                                    move |_: MouseEvent| {
                                                                                        delete_passkey_id.set(Some(pk_id.clone()));
                                                                                    }
                                                                                }
                                                                            >
                                                                                {"Delete"}
                                                                            </button>
                                                                        </div>
                                                                    </div>
                                                                }
                                                            }
                                                        }).collect::<Html>()
                                                    }
                                                </div>
                                            }
                                        }
                                    }

                                    <button class="security-btn primary" onclick={on_add_passkey_click}>
                                        {"Add Passkey"}
                                    </button>
                                </div>
                            },
                            PasskeyState::Registering { .. } => {
                                let new_passkey_name = new_passkey_name.clone();
                                html! {
                                    <div class="passkey-registration">
                                        <p class="setup-instruction">
                                            {"Enter a name for this passkey (e.g., \"MacBook Touch ID\" or \"iPhone Face ID\"):"}
                                        </p>
                                        <input
                                            type="text"
                                            class="passkey-name-input full"
                                            placeholder="e.g., MacBook Touch ID"
                                            value={(*new_passkey_name).clone()}
                                            oninput={
                                                let new_passkey_name = new_passkey_name.clone();
                                                move |e: InputEvent| {
                                                    let input: HtmlInputElement = e.target_unchecked_into();
                                                    new_passkey_name.set(input.value());
                                                }
                                            }
                                        />
                                        <div class="setup-buttons">
                                            <button class="security-btn secondary" onclick={on_cancel_passkey}>
                                                {"Cancel"}
                                            </button>
                                            <button
                                                class="security-btn primary"
                                                onclick={on_register_passkey}
                                                disabled={*passkey_loading || new_passkey_name.is_empty()}
                                            >
                                                {if *passkey_loading { "Registering..." } else { "Continue" }}
                                            </button>
                                        </div>
                                        <p class="passkey-hint">
                                            {"After clicking Continue, your browser will prompt you to authenticate with Touch ID, Face ID, or your device's biometric."}
                                        </p>
                                    </div>
                                }
                            },
                        }
                    }
                }
            </div>

            // Session Section
            <div class="security-settings session-section">
                <h3 class="security-title">{"Session"}</h3>
                <p class="security-description">
                    {"Sign out of your account on this device."}
                </p>
                <button
                    class="security-btn logout-btn"
                    onclick={on_logout_click}
                    disabled={*logout_loading}
                >
                    {if *logout_loading { "Signing out..." } else { "Sign Out" }}
                </button>
            </div>

            <style>
            {r#"
                .security-container {
                    display: flex;
                    flex-direction: column;
                    gap: 24px;
                }
                .security-settings {
                    padding: 16px;
                    border: 1px solid #e0e0e0;
                    border-radius: 8px;
                    background: #fafafa;
                }
                .passkeys-section {
                    margin-top: 0;
                }
                .security-title {
                    margin: 0 0 16px 0;
                    font-size: 18px;
                    color: #1a1a1a !important;
                }
                .security-description {
                    color: #666;
                    margin-bottom: 16px;
                    line-height: 1.5;
                }
                .security-error {
                    background: #fee2e2;
                    border: 1px solid #ef4444;
                    color: #dc2626;
                    padding: 12px;
                    border-radius: 6px;
                    margin-bottom: 16px;
                    display: flex;
                    justify-content: space-between;
                    align-items: center;
                }
                .security-error button {
                    background: none;
                    border: none;
                    color: #dc2626;
                    cursor: pointer;
                    font-size: 18px;
                }
                .security-loading {
                    color: #666;
                    padding: 20px;
                    text-align: center;
                }
                .security-btn {
                    padding: 10px 20px;
                    border: none;
                    border-radius: 6px;
                    cursor: pointer;
                    font-size: 14px;
                    font-weight: 500;
                    transition: background-color 0.2s;
                }
                .security-btn:disabled {
                    opacity: 0.6;
                    cursor: not-allowed;
                }
                .security-btn.primary {
                    background: #1E90FF;
                    color: white;
                }
                .security-btn.primary:hover:not(:disabled) {
                    background: #1a7ae0;
                }
                .security-btn.secondary {
                    background: #e0e0e0;
                    color: #333;
                }
                .security-btn.secondary:hover:not(:disabled) {
                    background: #d0d0d0;
                }
                .security-btn.danger {
                    background: #ef4444;
                    color: white;
                }
                .security-btn.danger:hover:not(:disabled) {
                    background: #dc2626;
                }
                .security-status {
                    display: flex;
                    align-items: center;
                    gap: 12px;
                    margin-bottom: 16px;
                }
                .status-badge {
                    padding: 4px 12px;
                    border-radius: 20px;
                    font-size: 14px;
                    font-weight: 500;
                }
                .status-badge.enabled {
                    background: #dcfce7;
                    color: #166534;
                }
                .backup-count {
                    color: #666;
                    font-size: 14px;
                }
                .security-actions {
                    display: flex;
                    gap: 12px;
                }
                .qr-code {
                    width: 200px;
                    height: 200px;
                    display: block;
                    margin: 16px auto;
                    border: 1px solid #e0e0e0;
                    border-radius: 8px;
                }
                .secret-section {
                    text-align: center;
                    margin: 16px 0;
                }
                .show-secret-btn {
                    background: none;
                    border: none;
                    color: #1E90FF;
                    cursor: pointer;
                    font-size: 14px;
                    text-decoration: underline;
                }
                .secret-display {
                    margin-top: 12px;
                    padding: 12px;
                    background: #f5f5f5;
                    border-radius: 6px;
                }
                .secret-display code {
                    font-family: monospace;
                    font-size: 14px;
                    word-break: break-all;
                }
                .verify-section {
                    margin: 20px 0;
                }
                .verify-section label {
                    display: block;
                    margin-bottom: 8px;
                    color: #333;
                }
                .verify-input {
                    width: 100%;
                    max-width: 200px;
                    padding: 12px;
                    font-size: 24px;
                    text-align: center;
                    letter-spacing: 8px;
                    border: 2px solid #e0e0e0;
                    border-radius: 8px;
                    font-family: monospace;
                }
                .verify-input:focus {
                    outline: none;
                    border-color: #1E90FF;
                }
                .setup-buttons {
                    display: flex;
                    gap: 12px;
                    justify-content: center;
                    margin-top: 20px;
                }
                .backup-codes-display h4 {
                    margin: 0 0 12px 0;
                }
                .backup-warning {
                    background: #fef3c7;
                    border: 1px solid #f59e0b;
                    color: #92400e;
                    padding: 12px;
                    border-radius: 6px;
                    margin-bottom: 16px;
                    line-height: 1.5;
                }
                .backup-codes-grid {
                    display: grid;
                    grid-template-columns: repeat(2, 1fr);
                    gap: 8px;
                    margin-bottom: 16px;
                }
                .backup-code {
                    background: #f5f5f5;
                    padding: 8px 12px;
                    border-radius: 4px;
                    font-family: monospace;
                    font-size: 14px;
                    text-align: center;
                }
                .backup-codes-actions {
                    display: flex;
                    gap: 12px;
                    justify-content: center;
                }
                .modal-overlay {
                    position: fixed;
                    top: 0;
                    left: 0;
                    right: 0;
                    bottom: 0;
                    background: rgba(0, 0, 0, 0.5);
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    z-index: 1000;
                }
                .modal-content {
                    background: white;
                    padding: 24px;
                    border-radius: 12px;
                    max-width: 400px;
                    width: 90%;
                }
                .modal-content h4 {
                    margin: 0 0 12px 0;
                }
                .modal-content p {
                    color: #666;
                    margin-bottom: 16px;
                }
                .modal-buttons {
                    display: flex;
                    gap: 12px;
                    justify-content: flex-end;
                    margin-top: 16px;
                }
                /* Passkey styles */
                .totp-required-notice {
                    background: #e0f2fe;
                    border: 1px solid #0ea5e9;
                    color: #0369a1;
                    padding: 16px;
                    border-radius: 6px;
                }
                .totp-required-notice p {
                    margin: 0 0 8px 0;
                }
                .totp-required-notice p:last-child {
                    margin-bottom: 0;
                }
                .totp-required-notice .notice-reason {
                    font-size: 13px;
                    opacity: 0.85;
                }
                .webauthn-unsupported {
                    background: #fef3c7;
                    border: 1px solid #f59e0b;
                    color: #92400e;
                    padding: 12px;
                    border-radius: 6px;
                }
                .no-passkeys {
                    color: #666;
                    font-style: italic;
                    margin-bottom: 16px;
                }
                .passkey-list {
                    margin-bottom: 16px;
                }
                .passkey-item {
                    display: flex;
                    align-items: center;
                    justify-content: space-between;
                    padding: 12px;
                    background: white;
                    border: 1px solid #e0e0e0;
                    border-radius: 6px;
                    margin-bottom: 8px;
                }
                .passkey-item.editing,
                .passkey-item.deleting {
                    background: #f5f5f5;
                }
                .passkey-info {
                    display: flex;
                    flex-direction: column;
                    gap: 4px;
                }
                .passkey-name {
                    font-weight: 500;
                    color: #333;
                }
                .passkey-meta {
                    font-size: 12px;
                    color: #888;
                }
                .passkey-actions {
                    display: flex;
                    gap: 8px;
                }
                .passkey-btn {
                    padding: 6px 12px;
                    border: none;
                    border-radius: 4px;
                    cursor: pointer;
                    font-size: 12px;
                }
                .passkey-btn:disabled {
                    opacity: 0.6;
                    cursor: not-allowed;
                }
                .passkey-btn.edit {
                    background: #e0e0e0;
                    color: #333;
                }
                .passkey-btn.delete {
                    background: #fee2e2;
                    color: #dc2626;
                }
                .passkey-btn.save {
                    background: #1E90FF;
                    color: white;
                }
                .passkey-btn.cancel {
                    background: #e0e0e0;
                    color: #333;
                }
                .passkey-btn.danger {
                    background: #ef4444;
                    color: white;
                }
                .delete-confirm {
                    color: #dc2626;
                    font-size: 12px;
                    margin: 0 12px;
                }
                .passkey-name-input {
                    padding: 8px 12px;
                    border: 2px solid #e0e0e0;
                    border-radius: 6px;
                    font-size: 14px;
                    flex: 1;
                    margin-right: 12px;
                }
                .passkey-name-input:focus {
                    outline: none;
                    border-color: #1E90FF;
                }
                .passkey-name-input.full {
                    width: 100%;
                    max-width: 400px;
                    display: block;
                    margin-bottom: 16px;
                }
                .passkey-registration {
                    text-align: center;
                }
                .passkey-hint {
                    font-size: 12px;
                    color: #888;
                    margin-top: 16px;
                }
                .setup-instruction {
                    margin-bottom: 16px;
                    color: #333;
                }
                /* Session/Logout section */
                .session-section {
                    margin-top: 0;
                }
                .logout-btn {
                    background: #ef4444;
                    color: white;
                }
                .logout-btn:hover:not(:disabled) {
                    background: #dc2626;
                }
            "#}
            </style>
        </div>
    }
}
