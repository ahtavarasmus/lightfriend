pub mod login {
    use yew::prelude::*;
    use web_sys::HtmlInputElement;
    use gloo_net::http::Request;
    use serde::{Deserialize, Serialize};
    use yew_router::prelude::*;
    use crate::Route;
    use crate::config;
    use crate::utils::webauthn;
    use crate::utils::backup_crypto;
    use crate::utils::api::Api;
    use crate::profile::billing_models::UserProfile;
    use gloo_console::log;

    #[derive(Serialize)]
    pub struct LoginRequest {
        email: String,
        password: String,
    }
    // New 2FA response format
    #[derive(Deserialize)]
    struct TwoFaRequiredResponse {
        requires_2fa: bool,
        totp_enabled: bool,
        webauthn_enabled: bool,
        login_token: String,
    }
    // Legacy TOTP response (for backwards compatibility)
    #[derive(Deserialize)]
    struct TotpRequiredResponse {
        requires_totp: bool,
        totp_token: String,
    }
    #[derive(Serialize)]
    struct TotpVerifyRequest {
        totp_token: String,
        code: String,
        is_backup_code: bool,
    }
    #[derive(Serialize)]
    struct WebAuthnLoginStartRequest {
        login_token: String,
    }
    #[derive(Serialize)]
    struct WebAuthnVerifyRequest {
        login_token: String,
        response: serde_json::Value,
    }
    #[derive(Deserialize)]
    struct ErrorResponse {
        error: String,
    }

    #[function_component]
    pub fn Login() -> Html {
        let email = use_state(String::new);
        let password = use_state(String::new);
        let error = use_state(|| None::<String>);
        let success = use_state(|| None::<String>);
        // 2FA states
        let requires_2fa = use_state(|| false);
        let totp_enabled = use_state(|| false);
        let webauthn_enabled = use_state(|| false);
        let login_token = use_state(String::new);
        let totp_code = use_state(String::new);
        let use_backup_code = use_state(|| false);
        let is_verifying = use_state(|| false);
        let show_totp_form = use_state(|| false); // Which 2FA method is currently shown
        let webauthn_supported = use_state(|| webauthn::is_webauthn_supported());
        let password_for_backup = use_state(String::new); // Preserve password for backup init after 2FA

        let onsubmit = {
            let email = email.clone();
            let password = password.clone();
            let error_setter = error.clone();
            let success_setter = success.clone();
            let requires_2fa = requires_2fa.clone();
            let totp_enabled = totp_enabled.clone();
            let webauthn_enabled = webauthn_enabled.clone();
            let login_token = login_token.clone();
            let show_totp_form = show_totp_form.clone();
            let password_for_backup = password_for_backup.clone();

            Callback::from(move |e: SubmitEvent| {
                e.prevent_default();
                let email = (*email).clone();
                let password = (*password).clone();
                let error_setter = error_setter.clone();
                let success_setter = success_setter.clone();
                let requires_2fa = requires_2fa.clone();
                let totp_enabled = totp_enabled.clone();
                let webauthn_enabled = webauthn_enabled.clone();
                let login_token = login_token.clone();
                let show_totp_form = show_totp_form.clone();
                let password_for_backup = password_for_backup.clone();
                let password_clone = password.clone(); // For backup initialization

                wasm_bindgen_futures::spawn_local(async move {
                    log!("Attempting login for email:", &email);
                    match Request::post(&format!("{}/api/login", config::get_backend_url()))
                        .credentials(web_sys::RequestCredentials::Include)
                        .json(&LoginRequest { email, password })
                        .unwrap()
                        .send()
                        .await
                    {
                        Ok(response) => {
                            if response.ok() {
                                // Check if 2FA is required
                                if let Ok(text) = response.text().await {
                                    // Try new 2FA format first
                                    if text.contains("requires_2fa") {
                                        if let Ok(resp) = serde_json::from_str::<TwoFaRequiredResponse>(&text) {
                                            if resp.requires_2fa {
                                                requires_2fa.set(true);
                                                totp_enabled.set(resp.totp_enabled);
                                                webauthn_enabled.set(resp.webauthn_enabled);
                                                login_token.set(resp.login_token);
                                                // Save password for backup initialization after 2FA
                                                password_for_backup.set(password_clone.clone());
                                                // Default to TOTP form if both are enabled and webauthn not supported
                                                // Otherwise prefer WebAuthn
                                                show_totp_form.set(!resp.webauthn_enabled || (resp.totp_enabled && !webauthn::is_webauthn_supported()));
                                                return;
                                            }
                                        }
                                    }
                                    // Try legacy TOTP format
                                    else if text.contains("requires_totp") {
                                        if let Ok(totp_resp) = serde_json::from_str::<TotpRequiredResponse>(&text) {
                                            if totp_resp.requires_totp {
                                                requires_2fa.set(true);
                                                totp_enabled.set(true);
                                                webauthn_enabled.set(false);
                                                login_token.set(totp_resp.totp_token);
                                                // Save password for backup initialization after 2FA
                                                password_for_backup.set(password_clone.clone());
                                                show_totp_form.set(true);
                                                return;
                                            }
                                        }
                                    }
                                }

                                log!("Login request successful, cookies set by backend");
                                error_setter.set(None);
                                success_setter.set(Some("Login successful! Redirecting...".to_string()));

                                // Initialize backup encryption with password
                                let backup_password = password_clone.clone();
                                wasm_bindgen_futures::spawn_local(async move {
                                    // Get user profile to get user_id
                                    if let Ok(response) = Api::get("/api/profile").send().await {
                                        if let Ok(profile) = response.json::<UserProfile>().await {
                                            // Initialize backup with password
                                            if let Ok(session_key) = backup_crypto::initialize_backup_with_password(&backup_password, profile.id).await {
                                                log!("Backup initialized, sending session key to backend");
                                                let _ = Api::post("/api/backup/establish-key")
                                                    .json(&serde_json::json!({ "session_key": session_key }))
                                                    .ok()
                                                    .map(|r| r.send());
                                            }
                                        }
                                    }
                                });

                                let window = web_sys::window().unwrap();
                                wasm_bindgen_futures::spawn_local(async move {
                                    gloo_timers::future::TimeoutFuture::new(2_000).await;
                                    let _ = window.location().set_href("/");
                                });
                            } else {
                                log!("Login request failed with status:", response.status());
                                match response.json::<ErrorResponse>().await {
                                    Ok(error_response) => {
                                        log!("Server error response:", &error_response.error);
                                        error_setter.set(Some(error_response.error));
                                    }
                                    Err(_) => {
                                        error_setter.set(Some("Login failed".to_string()));
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            log!("Network request failed:", e.to_string());
                            error_setter.set(Some(format!("Request failed: {}", e)));
                        }
                    }
                });
            })
        };
        // TOTP verification handler
        let on_totp_verify = {
            let login_token = login_token.clone();
            let totp_code = totp_code.clone();
            let use_backup_code = use_backup_code.clone();
            let error_setter = error.clone();
            let success_setter = success.clone();
            let is_verifying = is_verifying.clone();
            let password_for_backup = password_for_backup.clone();

            Callback::from(move |e: SubmitEvent| {
                e.prevent_default();
                let token = (*login_token).clone();
                let code = (*totp_code).clone();
                let is_backup = *use_backup_code;
                let error_setter = error_setter.clone();
                let success_setter = success_setter.clone();
                let is_verifying = is_verifying.clone();
                let backup_password = (*password_for_backup).clone();

                if code.is_empty() {
                    error_setter.set(Some("Please enter a code".to_string()));
                    return;
                }

                is_verifying.set(true);
                wasm_bindgen_futures::spawn_local(async move {
                    match Request::post(&format!("{}/api/totp/verify", config::get_backend_url()))
                        .credentials(web_sys::RequestCredentials::Include)
                        .json(&TotpVerifyRequest {
                            totp_token: token,
                            code,
                            is_backup_code: is_backup
                        })
                        .unwrap()
                        .send()
                        .await
                    {
                        Ok(response) => {
                            if response.ok() {
                                log!("TOTP verification successful");
                                error_setter.set(None);
                                success_setter.set(Some("Login successful! Redirecting...".to_string()));

                                // Initialize backup encryption with password
                                let backup_pwd = backup_password.clone();
                                wasm_bindgen_futures::spawn_local(async move {
                                    if let Ok(response) = Api::get("/api/profile").send().await {
                                        if let Ok(profile) = response.json::<UserProfile>().await {
                                            if let Ok(session_key) = backup_crypto::initialize_backup_with_password(&backup_pwd, profile.id).await {
                                                log!("Backup initialized after TOTP, sending session key");
                                                let _ = Api::post("/api/backup/establish-key")
                                                    .json(&serde_json::json!({ "session_key": session_key }))
                                                    .ok()
                                                    .map(|r| r.send());
                                            }
                                        }
                                    }
                                });

                                let window = web_sys::window().unwrap();
                                wasm_bindgen_futures::spawn_local(async move {
                                    gloo_timers::future::TimeoutFuture::new(2_000).await;
                                    let _ = window.location().set_href("/");
                                });
                            } else {
                                match response.json::<ErrorResponse>().await {
                                    Ok(error_response) => {
                                        error_setter.set(Some(error_response.error));
                                    }
                                    Err(_) => {
                                        error_setter.set(Some("Invalid code".to_string()));
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error_setter.set(Some(format!("Request failed: {}", e)));
                        }
                    }
                    is_verifying.set(false);
                });
            })
        };
        // WebAuthn verification handler
        let on_webauthn_verify = {
            let login_token = login_token.clone();
            let error_setter = error.clone();
            let success_setter = success.clone();
            let is_verifying = is_verifying.clone();
            let password_for_backup = password_for_backup.clone();

            Callback::from(move |_: MouseEvent| {
                let token = (*login_token).clone();
                let error_setter = error_setter.clone();
                let success_setter = success_setter.clone();
                let is_verifying = is_verifying.clone();
                let backup_password = (*password_for_backup).clone();

                is_verifying.set(true);
                error_setter.set(None);

                wasm_bindgen_futures::spawn_local(async move {
                    // Step 1: Get WebAuthn options from server
                    let options_result = match Request::post(&format!("{}/api/webauthn/login/start", config::get_backend_url()))
                        .credentials(web_sys::RequestCredentials::Include)
                        .json(&WebAuthnLoginStartRequest {
                            login_token: token.clone(),
                        })
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
                                },
                                Err(e) => Err(format!("Failed to parse options: {:?}", e)),
                            }
                        }
                        Ok(resp) => {
                            let err = resp.json::<ErrorResponse>().await
                                .map(|e| e.error)
                                .unwrap_or_else(|_| format!("Server error: {}", resp.status()));
                            Err(err)
                        }
                        Err(e) => Err(format!("Network error: {:?}", e)),
                    };

                    let options = match options_result {
                        Ok(o) => o,
                        Err(e) => {
                            error_setter.set(Some(e));
                            is_verifying.set(false);
                            return;
                        }
                    };

                    // Step 2: Get credential from browser
                    let credential = match webauthn::get_credential(&options).await {
                        Ok(c) => c,
                        Err(e) => {
                            error_setter.set(Some(format!("WebAuthn error: {}", e)));
                            is_verifying.set(false);
                            return;
                        }
                    };

                    // Step 3: Verify with server
                    match Request::post(&format!("{}/api/webauthn/verify-login", config::get_backend_url()))
                        .credentials(web_sys::RequestCredentials::Include)
                        .json(&WebAuthnVerifyRequest {
                            login_token: token,
                            response: credential,
                        })
                        .unwrap()
                        .send()
                        .await
                    {
                        Ok(response) => {
                            if response.ok() {
                                log!("WebAuthn verification successful");
                                error_setter.set(None);
                                success_setter.set(Some("Login successful! Redirecting...".to_string()));

                                // Initialize backup encryption with password
                                let backup_pwd = backup_password.clone();
                                wasm_bindgen_futures::spawn_local(async move {
                                    if let Ok(response) = Api::get("/api/profile").send().await {
                                        if let Ok(profile) = response.json::<UserProfile>().await {
                                            if let Ok(session_key) = backup_crypto::initialize_backup_with_password(&backup_pwd, profile.id).await {
                                                log!("Backup initialized after WebAuthn, sending session key");
                                                let _ = Api::post("/api/backup/establish-key")
                                                    .json(&serde_json::json!({ "session_key": session_key }))
                                                    .ok()
                                                    .map(|r| r.send());
                                            }
                                        }
                                    }
                                });

                                let window = web_sys::window().unwrap();
                                wasm_bindgen_futures::spawn_local(async move {
                                    gloo_timers::future::TimeoutFuture::new(2_000).await;
                                    let _ = window.location().set_href("/");
                                });
                            } else {
                                match response.json::<ErrorResponse>().await {
                                    Ok(error_response) => {
                                        error_setter.set(Some(error_response.error));
                                    }
                                    Err(_) => {
                                        error_setter.set(Some("Authentication failed".to_string()));
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error_setter.set(Some(format!("Request failed: {}", e)));
                        }
                    }
                    is_verifying.set(false);
                });
            })
        };

        // Toggle between TOTP and WebAuthn
        let on_switch_to_totp = {
            let show_totp_form = show_totp_form.clone();
            Callback::from(move |_: MouseEvent| {
                show_totp_form.set(true);
            })
        };

        let on_switch_to_webauthn = {
            let show_totp_form = show_totp_form.clone();
            Callback::from(move |_: MouseEvent| {
                show_totp_form.set(false);
            })
        };

        // Toggle backup code mode
        let on_toggle_backup = {
            let use_backup_code = use_backup_code.clone();
            let totp_code = totp_code.clone();
            Callback::from(move |_: MouseEvent| {
                use_backup_code.set(!*use_backup_code);
                totp_code.set(String::new());
            })
        };

        // Cancel 2FA and go back to login
        let on_cancel_2fa = {
            let requires_2fa = requires_2fa.clone();
            let login_token = login_token.clone();
            let totp_code = totp_code.clone();
            let use_backup_code = use_backup_code.clone();
            let error_setter = error.clone();
            let show_totp_form = show_totp_form.clone();
            Callback::from(move |_: MouseEvent| {
                requires_2fa.set(false);
                login_token.set(String::new());
                totp_code.set(String::new());
                use_backup_code.set(false);
                show_totp_form.set(false);
                error_setter.set(None);
            })
        };
        html! {
        <div style="min-height: 100vh; display: flex; align-items: center; justify-content: center; padding: 2rem;">
            <style>
            {r#".login-container,
.register-container {
    background: rgba(30, 30, 30, 0.7); /* Darker container */
    border: 1px solid rgba(30, 144, 255, 0.1);
    border-radius: 16px;
    padding: 3rem;
    width: 100%;
    max-width: 480px;
    backdrop-filter: blur(10px);
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
}
.login-container h1,
.register-container h1 {
    font-size: 2rem;
    margin-bottom: 1.5rem;
    text-align: center;
    background: linear-gradient(45deg, #fff, #7EB2FF);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
}
@media (max-width: 768px) {
    .login-container,
    .register-container {
        padding: 2rem;
        margin: 1rem;
    }
}
.auth-redirect {
    margin-top: 2rem;
    text-align: center;
    color: rgba(255, 255, 255, 0.6); /* Dimmer text */
    font-size: 0.9rem;
}
.auth-redirect a {
    color: #1E90FF;
    text-decoration: none;
    transition: color 0.3s ease;
    margin-left: 0.25rem;
}
.auth-redirect a:hover {
    color: #7EB2FF;
    text-decoration: underline;
}
/* Custom checkbox styling */
#terms-checkbox-container {
    margin: 15px 0;
}
#terms-checkbox-container label {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    cursor: pointer;
    font-size: 0.9em;
    color: rgba(255, 255, 255, 0.8);
    line-height: 1.4;
}
#terms-checkbox-container input[type="checkbox"] {
    appearance: none !important;
    -webkit-appearance: none !important;
    width: 1px !important;
    height: 1px !important;
    border: 2px solid rgba(30, 144, 255, 0.5) !important;
    border-radius: 4px !important;
    background: rgba(30, 30, 30, 0.7) !important;
    cursor: pointer !important;
    position: relative !important;
    margin-top: 2px !important;
    transition: all 0.2s ease !important;
    display: inline-block !important;
    vertical-align: middle !important;
    transform: scale(0.6) !important;
    transform-origin: left center !important;
}
#terms-checkbox-container input[type="checkbox"]:checked {
    background: #1E90FF !important;
    border-color: #1E90FF !important;
}
#terms-checkbox-container input[type="checkbox"]:checked::after {
    content: "✓" !important;
    position: absolute !important;
    color: white !important;
    font-size: 30px !important;
    left: 2px !important;
    top: -1px !important;
    display: block !important;
}
#terms-checkbox-container input[type="checkbox"]:hover {
    border-color: #1E90FF !important;
}
#terms-checkbox-container a {
    color: #1E90FF;
    text-decoration: none;
    transition: color 0.3s ease;
}
#terms-checkbox-container a:hover {
    color: #7EB2FF;
    text-decoration: underline;
}
.hero-background {
        position: fixed;
        top: 0;
        left: 0;
        width: 100%;
        height: 100vh;
        background-image: url('/assets/rain.gif');
        background-size: cover;
        background-position: center;
        background-repeat: no-repeat;
        opacity: 1;
        z-index: -2;
        pointer-events: none;
    }
    .hero-background::after {
        content: '';
        position: absolute;
        bottom: 0;
        left: 0;
        width: 100%;
        height: 50%;
        background: linear-gradient(to bottom,
            rgba(26, 26, 26, 0) 0%,
            rgba(26, 26, 26, 1) 100%
        );
    }"#}
            </style>
            <div class="hero-background"></div>
            <div class="login-container">
                <h1>{if *requires_2fa { "Two-Factor Authentication" } else { "Login" }}</h1>
                {
                    if let Some(error_message) = (*error).as_ref() {
                        html! {
                            <div class="error-message" style="color: red; margin-bottom: 10px;">
                                {error_message}
                            </div>
                        }
                    } else if let Some(success_message) = (*success).as_ref() {
                        html! {
                            <div class="success-message" style="color: green; margin-bottom: 10px;">
                                {success_message}
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }
                {
                    if *requires_2fa {
                        // 2FA is required - show either WebAuthn or TOTP form
                        if *show_totp_form {
                            // Show TOTP form
                            html! {
                                <>
                                    <p style="color: rgba(255, 255, 255, 0.8); margin-bottom: 20px; text-align: center;">
                                        {if *use_backup_code {
                                            "Enter one of your backup codes"
                                        } else {
                                            "Enter the 6-digit code from your authenticator app"
                                        }}
                                    </p>
                                    <form onsubmit={on_totp_verify}>
                                        <input
                                            type="text"
                                            placeholder={if *use_backup_code { "Backup code" } else { "000000" }}
                                            maxlength={if *use_backup_code { "10" } else { "6" }}
                                            value={(*totp_code).clone()}
                                            style="text-align: center; font-size: 1.5rem; letter-spacing: 0.5rem;"
                                            oninput={let totp_code = totp_code.clone(); let use_backup = *use_backup_code; move |e: InputEvent| {
                                                let input: HtmlInputElement = e.target_unchecked_into();
                                                let value = if use_backup {
                                                    input.value()
                                                } else {
                                                    input.value().chars().filter(|c| c.is_numeric()).collect::<String>()
                                                };
                                                totp_code.set(value);
                                            }}
                                        />
                                        <button type="submit" disabled={*is_verifying}>
                                            {if *is_verifying { "Verifying..." } else { "Verify" }}
                                        </button>
                                    </form>
                                    <div class="auth-redirect">
                                        <a href="#" onclick={on_toggle_backup} style="color: #1E90FF; text-decoration: none;">
                                            {if *use_backup_code { "Use authenticator code instead" } else { "Use backup code instead" }}
                                        </a>
                                    </div>
                                    {
                                        if *webauthn_enabled && *webauthn_supported {
                                            html! {
                                                <div class="auth-redirect">
                                                    <a href="#" onclick={on_switch_to_webauthn.clone()} style="color: #1E90FF; text-decoration: none;">
                                                        {"Use passkey instead"}
                                                    </a>
                                                </div>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }
                                    <div class="auth-redirect">
                                        <a href="#" onclick={on_cancel_2fa.clone()} style="color: rgba(255, 255, 255, 0.6); text-decoration: none;">
                                            {"Back to login"}
                                        </a>
                                    </div>
                                </>
                            }
                        } else {
                            // Show WebAuthn form
                            html! {
                                <>
                                    <p style="color: rgba(255, 255, 255, 0.8); margin-bottom: 20px; text-align: center;">
                                        {"Use your passkey to complete login"}
                                    </p>
                                    <button
                                        onclick={on_webauthn_verify}
                                        disabled={*is_verifying}
                                        style="width: 100%; padding: 1rem; font-size: 1rem; display: flex; align-items: center; justify-content: center; gap: 0.5rem;"
                                    >
                                        {if *is_verifying { "Verifying..." } else { "Authenticate with Passkey" }}
                                    </button>
                                    {
                                        if *totp_enabled {
                                            html! {
                                                <div class="auth-redirect">
                                                    <a href="#" onclick={on_switch_to_totp.clone()} style="color: #1E90FF; text-decoration: none;">
                                                        {"Use authenticator app instead"}
                                                    </a>
                                                </div>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }
                                    <div class="auth-redirect">
                                        <a href="#" onclick={on_cancel_2fa} style="color: rgba(255, 255, 255, 0.6); text-decoration: none;">
                                            {"Back to login"}
                                        </a>
                                    </div>
                                </>
                            }
                        }
                    } else {
                        html! {
                            <>
                                <form onsubmit={onsubmit}>
                                    <input
                                        type="text"
                                        placeholder="Email or username"
                                        onchange={let email = email.clone(); move |e: Event| {
                                            let input: HtmlInputElement = e.target_unchecked_into();
                                            email.set(input.value());
                                        }}
                                    />
                                    <input
                                        type="password"
                                        placeholder="Password"
                                        onchange={let password = password.clone(); move |e: Event| {
                                            let input: HtmlInputElement = e.target_unchecked_into();
                                            password.set(input.value());
                                        }}
                                    />
                                    <button type="submit">{"Login"}</button>
                                </form>
                                <div class="auth-redirect">
                                    <Link<Route> to={Route::PasswordReset}>
                                        {"Forgot password?"}
                                    </Link<Route>>
                                </div>
                                <div class="auth-redirect">
                                    {"New? "}
                                    <Link<Route> to={Route::Pricing}>
                                        {"Subscribe here →"}
                                    </Link<Route>>
                                </div>
                            </>
                        }
                    }
                }
            </div>
        </div>
        }
    }
}
pub mod password_reset {
    use yew::prelude::*;
    use web_sys::HtmlInputElement;
    use gloo_net::http::Request;
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize)]
    struct ErrorResponse {
        pub error: String,
    }
    use yew_router::prelude::*;
    use crate::Route;
    use crate::config;

    #[derive(Serialize)]
    struct CompletePasswordResetRequest {
        token: String,
        new_password: String,
    }

    #[derive(Deserialize)]
    struct PasswordResetResponse {
        message: String,
    }

    // Shared CSS styles
    const AUTH_STYLES: &str = r#".login-container,
.register-container {
    background: rgba(30, 30, 30, 0.7);
    border: 1px solid rgba(30, 144, 255, 0.1);
    border-radius: 16px;
    padding: 3rem;
    width: 100%;
    max-width: 480px;
    backdrop-filter: blur(10px);
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
}
.login-container h1,
.register-container h1 {
    font-size: 2rem;
    margin-bottom: 1.5rem;
    text-align: center;
    background: linear-gradient(45deg, #fff, #7EB2FF);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
}
@media (max-width: 768px) {
    .login-container,
    .register-container {
        padding: 2rem;
        margin: 1rem;
    }
}
.auth-redirect {
    margin-top: 2rem;
    text-align: center;
    color: rgba(255, 255, 255, 0.6);
    font-size: 0.9rem;
}
.auth-redirect a {
    color: #1E90FF;
    text-decoration: none;
    transition: color 0.3s ease;
    margin-left: 0.25rem;
}
.auth-redirect a:hover {
    color: #7EB2FF;
    text-decoration: underline;
}
.hero-background {
    position: fixed;
    top: 0;
    left: 0;
    width: 100%;
    height: 100vh;
    background-image: url('/assets/rain.gif');
    background-size: cover;
    background-position: center;
    background-repeat: no-repeat;
    opacity: 1;
    z-index: -2;
    pointer-events: none;
}
.hero-background::after {
    content: '';
    position: absolute;
    bottom: 0;
    left: 0;
    width: 100%;
    height: 50%;
    background: linear-gradient(to bottom,
        rgba(26, 26, 26, 0) 0%,
        rgba(26, 26, 26, 1) 100%
    );
}
.contact-info {
    text-align: center;
    color: rgba(255, 255, 255, 0.8);
    line-height: 1.6;
    margin: 1.5rem 0;
}
.contact-info a {
    color: #1E90FF;
    text-decoration: none;
}
.contact-info a:hover {
    color: #7EB2FF;
    text-decoration: underline;
}"#;

    /// Password Reset page - shows contact instructions
    /// (No token = user needs to contact admin for reset)
    #[function_component]
    pub fn PasswordReset() -> Html {
        html! {
            <div style="min-height: 100vh; display: flex; align-items: center; justify-content: center; padding: 2rem;">
                <style>{AUTH_STYLES}</style>
                <div class="hero-background"></div>
                <div class="login-container">
                    <h1>{"Password Reset"}</h1>
                    <div class="contact-info">
                        <p>{"To reset your password, please contact:"}</p>
                        <p style="margin-top: 1rem;">
                            <a href="mailto:rasmus@ahtava.com">{"rasmus@ahtava.com"}</a>
                        </p>
                        <p style="margin-top: 1rem; font-size: 0.9rem; color: rgba(255, 255, 255, 0.6);">
                            {"We'll verify your identity and send you a secure reset link."}
                        </p>
                    </div>
                    <div class="auth-redirect">
                        <Link<Route> to={Route::Login}>
                            {"Back to Login"}
                        </Link<Route>>
                    </div>
                </div>
            </div>
        }
    }

    /// Password Reset with Token - shows form to set new password
    #[derive(Properties, PartialEq)]
    pub struct PasswordResetWithTokenProps {
        pub token: String,
    }

    #[function_component]
    pub fn PasswordResetWithToken(props: &PasswordResetWithTokenProps) -> Html {
        let navigator = use_navigator().unwrap();
        let token = props.token.clone();
        let new_password = use_state(String::new);
        let confirm_password = use_state(String::new);
        let error = use_state(|| None::<String>);
        let success = use_state(|| None::<String>);
        let loading = use_state(|| true);
        let token_valid = use_state(|| false);

        // Validate token on mount
        {
            let token = token.clone();
            let error = error.clone();
            let loading = loading.clone();
            let token_valid = token_valid.clone();

            use_effect_with_deps(move |_| {
                wasm_bindgen_futures::spawn_local(async move {
                    match Request::get(&format!("{}/api/password-reset/validate/{}", config::get_backend_url(), token))
                        .send()
                        .await
                    {
                        Ok(response) => {
                            if response.ok() {
                                token_valid.set(true);
                            } else {
                                match response.json::<ErrorResponse>().await {
                                    Ok(err) => error.set(Some(err.error)),
                                    Err(_) => error.set(Some("Invalid or expired reset link.".to_string())),
                                }
                            }
                        }
                        Err(e) => {
                            error.set(Some(format!("Failed to validate token: {}", e)));
                        }
                    }
                    loading.set(false);
                });
                || ()
            }, ());
        }

        let submit_reset = {
            let token = token.clone();
            let new_password = new_password.clone();
            let confirm_password = confirm_password.clone();
            let error = error.clone();
            let success = success.clone();
            let navigator = navigator.clone();

            Callback::from(move |e: SubmitEvent| {
                e.prevent_default();

                let password = (*new_password).clone();
                let confirm = (*confirm_password).clone();

                if password.len() < 8 {
                    error.set(Some("Password must be at least 8 characters long.".to_string()));
                    return;
                }

                if password != confirm {
                    error.set(Some("Passwords do not match.".to_string()));
                    return;
                }

                let token = token.clone();
                let error = error.clone();
                let success = success.clone();
                let navigator = navigator.clone();

                wasm_bindgen_futures::spawn_local(async move {
                    match Request::post(&format!("{}/api/password-reset/complete", config::get_backend_url()))
                        .json(&CompletePasswordResetRequest {
                            token,
                            new_password: password,
                        })
                        .unwrap()
                        .send()
                        .await
                    {
                        Ok(response) => {
                            if response.ok() {
                                match response.json::<PasswordResetResponse>().await {
                                    Ok(resp) => {
                                        error.set(None);
                                        success.set(Some(resp.message));
                                        // Redirect to login after 2 seconds
                                        gloo_timers::callback::Timeout::new(2_000, move || {
                                            navigator.push(&Route::Login);
                                        }).forget();
                                    }
                                    Err(_) => {
                                        error.set(Some("Password reset successful! Redirecting to login...".to_string()));
                                        gloo_timers::callback::Timeout::new(2_000, move || {
                                            navigator.push(&Route::Login);
                                        }).forget();
                                    }
                                }
                            } else {
                                match response.json::<ErrorResponse>().await {
                                    Ok(err) => error.set(Some(err.error)),
                                    Err(_) => error.set(Some("Failed to reset password.".to_string())),
                                }
                            }
                        }
                        Err(e) => {
                            error.set(Some(format!("Request failed: {}", e)));
                        }
                    }
                });
            })
        };

        html! {
            <div style="min-height: 100vh; display: flex; align-items: center; justify-content: center; padding: 2rem;">
                <style>{AUTH_STYLES}</style>
                <div class="hero-background"></div>
                <div class="login-container">
                    <h1>{"Reset Password"}</h1>

                    {
                        if *loading {
                            html! { <p style="text-align: center;">{"Validating reset link..."}</p> }
                        } else if !*token_valid {
                            html! {
                                <>
                                    <div class="error-message" style="color: #ff6b6b; margin-bottom: 1rem; text-align: center;">
                                        {(*error).as_ref().unwrap_or(&"Invalid or expired reset link.".to_string())}
                                    </div>
                                    <div class="contact-info">
                                        <p>{"Please contact "}<a href="mailto:rasmus@ahtava.com">{"rasmus@ahtava.com"}</a>{" for a new reset link."}</p>
                                    </div>
                                </>
                            }
                        } else {
                            html! {
                                <>
                                    {
                                        if let Some(error_msg) = (*error).as_ref() {
                                            html! {
                                                <div class="error-message" style="color: #ff6b6b; margin-bottom: 1rem; text-align: center;">
                                                    {error_msg}
                                                </div>
                                            }
                                        } else if let Some(success_msg) = (*success).as_ref() {
                                            html! {
                                                <div class="success-message" style="color: #4ade80; margin-bottom: 1rem; text-align: center;">
                                                    {success_msg}
                                                </div>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }

                                    <form onsubmit={submit_reset}>
                                        <input
                                            type="password"
                                            placeholder="New Password"
                                            autocomplete="new-password"
                                            onchange={let new_password = new_password.clone(); move |e: Event| {
                                                let input: HtmlInputElement = e.target_unchecked_into();
                                                new_password.set(input.value());
                                            }}
                                        />
                                        <input
                                            type="password"
                                            placeholder="Confirm Password"
                                            autocomplete="new-password"
                                            onchange={let confirm_password = confirm_password.clone(); move |e: Event| {
                                                let input: HtmlInputElement = e.target_unchecked_into();
                                                confirm_password.set(input.value());
                                            }}
                                        />
                                        <button type="submit">{"Set New Password"}</button>
                                    </form>
                                </>
                            }
                        }
                    }

                    <div class="auth-redirect">
                        <Link<Route> to={Route::Login}>
                            {"Back to Login"}
                        </Link<Route>>
                    </div>
                </div>
            </div>
        }
    }
}
