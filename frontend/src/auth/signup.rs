pub mod login {
    use yew::prelude::*;
    use web_sys::HtmlInputElement;
    use gloo_net::http::Request;
    use serde::{Deserialize, Serialize};
    use yew_router::prelude::*;
    use crate::Route;
    use crate::config;
    use crate::utils::webauthn;
    use gloo_console::log;

    #[derive(Serialize)]
    pub struct LoginRequest {
        email: String,
        password: String,
    }
    #[derive(Deserialize)]
    pub struct LoginResponse {
        token: String,
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
                                                show_totp_form.set(true);
                                                return;
                                            }
                                        }
                                    }
                                }

                                log!("Login request successful, cookies set by backend");
                                error_setter.set(None);
                                success_setter.set(Some("Login successful! Redirecting...".to_string()));

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

            Callback::from(move |e: SubmitEvent| {
                e.prevent_default();
                let token = (*login_token).clone();
                let code = (*totp_code).clone();
                let is_backup = *use_backup_code;
                let error_setter = error_setter.clone();
                let success_setter = success_setter.clone();
                let is_verifying = is_verifying.clone();

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

            Callback::from(move |_: MouseEvent| {
                let token = (*login_token).clone();
                let error_setter = error_setter.clone();
                let success_setter = success_setter.clone();
                let is_verifying = is_verifying.clone();

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
                                    {"Don't have an account? "}
                                    <Link<Route> to={Route::Register}>
                                        {"Register here"}
                                    </Link<Route>>
                                </div>
                                <div class="auth-redirect">
                                    <Link<Route> to={Route::PasswordReset}>
                                        {"Forgot password?"}
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
    use crate::auth::signup::register::ErrorResponse;
    use gloo_net::http::Request;
    use serde::{Deserialize, Serialize};
    use yew_router::prelude::*;
    use crate::Route;
    use crate::config;
    #[derive(Serialize)]
    struct PasswordResetRequest {
        email: String,
    }
    #[derive(Serialize)]
    struct VerifyPasswordResetRequest {
        email: String,
        otp: String,
        new_password: String,
    }
    #[derive(Deserialize)]
    struct PasswordResetResponse {
        message: String,
    }
    #[function_component]
    pub fn PasswordReset() -> Html {
        let navigator = use_navigator().unwrap();
        let email = use_state(String::new);
        let otp = use_state(String::new);
        let new_password = use_state(String::new);
        let error = use_state(|| None::<String>);
        let success = use_state(|| None::<String>);
        let otp_sent = use_state(|| false);
        let request_reset = {
            let email = email.clone();
            let error_setter = error.clone();
            let success_setter = success.clone();
            let otp_sent_setter = otp_sent.clone();
           
            Callback::from(move |e: SubmitEvent| {
                e.prevent_default();
                let email = (*email).clone();
                let error_setter = error_setter.clone();
                let success_setter = success_setter.clone();
                let otp_sent_setter = otp_sent_setter.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match Request::post(&format!("{}/api/password-reset/request", config::get_backend_url()))
                        .json(&PasswordResetRequest { email })
                        .unwrap()
                        .send()
                        .await
                    {
                        Ok(response) => {
                            if response.ok() {
                                match response.json::<PasswordResetResponse>().await {
                                    Ok(resp) => {
                                        error_setter.set(None);
                                        success_setter.set(Some(resp.message));
                                        otp_sent_setter.set(true);
                                    }
                                    Err(_) => {
                                        error_setter.set(Some("Failed to parse server response".to_string()));
                                    }
                                }
                            } else {
                                match response.json::<ErrorResponse>().await {
                                    Ok(error_response) => {
                                        error_setter.set(Some(error_response.error));
                                    }
                                    Err(_) => {
                                        error_setter.set(Some("Failed to request password reset".to_string()));
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error_setter.set(Some(format!("Request failed: {}", e)));
                        }
                    }
                });
            })
        };
        let verify_reset = {
            let email = email.clone();
            let otp = otp.clone();
            let new_password = new_password.clone();
            let error_setter = error.clone();
            let success_setter = success.clone();
            let navigator = navigator.clone();
           
            Callback::from(move |e: SubmitEvent| {
                e.prevent_default();
                let email = (*email).clone();
                let otp = (*otp).clone();
                let new_password = (*new_password).clone();
                let error_setter = error_setter.clone();
                let success_setter = success_setter.clone();
                let navigator = navigator.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match Request::post(&format!("{}/api/password-reset/verify", config::get_backend_url()))
                        .json(&VerifyPasswordResetRequest {
                            email,
                            otp,
                            new_password,
                        })
                        .unwrap()
                        .send()
                        .await
                    {
                        Ok(response) => {
                            if response.ok() {
                                match response.json::<PasswordResetResponse>().await {
                                    Ok(resp) => {
                                        println!("Password reset successful, preparing to redirect");
                                        error_setter.set(None);
                                        success_setter.set(Some(resp.message.clone()));
                                        // Use setTimeout to delay navigation
                                        let navigator = navigator.clone();
                                        let success_message = resp.message.clone();
                                        gloo_timers::callback::Timeout::new(2_000, move || {
                                            println!("Redirecting to login page after password reset");
                                            navigator.push(&Route::Login);
                                        }).forget();
                                    }
                                    Err(e) => {
                                        println!("Error parsing password reset response: {:?}", e);
                                        error_setter.set(Some("Failed to parse server response. Please try again.".to_string()));
                                    }
                                }
                            } else {
                                error_setter.set(Some("Failed to verify reset code".to_string()));
                            }
                        }
                        Err(e) => {
                            error_setter.set(Some(format!("Request failed: {}", e)));
                        }
                    }
                });
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
                    <h1>{"Password Reset"}</h1>
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
                        if !*otp_sent {
                            html! {
                                <form onsubmit={request_reset}>
                                    <input
                                        type="email"
                                        placeholder="Email"
                                        onchange={let email = email.clone(); move |e: Event| {
                                            let input: HtmlInputElement = e.target_unchecked_into();
                                            email.set(input.value());
                                        }}
                                    />
                                    <button type="submit">{"Send Reset Code"}</button>
                                </form>
                            }
                        } else {
                            html! {
                                <form onsubmit={verify_reset}>
                                    <input
                                        type="text"
                                        placeholder="Reset Code"
                                        onchange={let otp = otp.clone(); move |e: Event| {
                                            let input: HtmlInputElement = e.target_unchecked_into();
                                            otp.set(input.value());
                                        }}
                                    />
                                    <input
                                        type="password"
                                        placeholder="New Password"
                                        autocomplete="new-password"
                                        onchange={let new_password = new_password.clone(); move |e: Event| {
                                            let input: HtmlInputElement = e.target_unchecked_into();
                                            new_password.set(input.value());
                                        }}
                                    />
                                    <button type="submit">{"Reset Password"}</button>
                                </form>
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

pub mod register {
    use yew::prelude::*;
    use web_sys::HtmlInputElement;
    use gloo_net::http::Request;
    use serde::{Deserialize, Serialize};
    use yew_router::prelude::*;
    use crate::Route;
    use crate::config;
    use crate::SelfHostingStatus;
    use gloo_console::log;
    #[derive(Properties, PartialEq)]
    pub struct RegisterProps {
        #[prop_or(SelfHostingStatus::Normal)]
        pub self_hosting_status: SelfHostingStatus,
    }
    #[derive(Serialize)]
    pub struct RegisterRequest {
        email: String,
        password: String,
        phone_number: String,
    }
    #[derive(Serialize)]
    pub struct SelfHostedLoginRequest {
        email: String,
        password: String,
    }
    #[derive(Deserialize)]
    pub struct RegisterResponse {
        message: String,
        token: String,
    }
    #[derive(Deserialize)]
    pub struct ErrorResponse {
        pub error: String,
    }
    fn is_valid_email(email: &str) -> bool {
        // Basic email validation
        email.contains('@') && email.contains('.')
    }
    fn is_valid_phone(phone: &str) -> bool {
        // Check if phone number starts with +
        phone.starts_with('+')
    }
    #[function_component]
    pub fn Register(props: &RegisterProps) -> Html {
        let email = use_state(String::new);
        let password = use_state(String::new);
        let phone_number = use_state(String::new);
        let error = use_state(|| None::<String>);
        let success = use_state(|| None::<String>);
        let email_valid = use_state(|| true); // Track email validity
        let terms_accepted = use_state(|| false); // Track terms acceptance
        let self_hosted_login = {
            let email = email.clone();
            let password = password.clone();
            let error_setter = error.clone();
            let success_setter = success.clone();
          
            Callback::from(move |e: SubmitEvent| {
                e.prevent_default();
                let email = (*email).clone();
                let password = (*password).clone();
                let error_setter = error_setter.clone();
                let success_setter = success_setter.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match Request::post(&format!("{}/api/self-hosted/login", config::get_backend_url()))
                        .credentials(web_sys::RequestCredentials::Include)
                        .json(&SelfHostedLoginRequest { email: email.clone(), password })
                        .unwrap()
                        .send()
                        .await
                    {
                        Ok(response) => {
                            if response.ok() {
                                match response.json::<RegisterResponse>().await {
                                    Ok(resp) => {
                                        log!("Self-hosted login successful, cookies set by backend");
                                        error_setter.set(None);
                                        success_setter.set(Some(resp.message));

                                        // Redirect to home page after a short delay
                                        let window = web_sys::window().unwrap();
                                        wasm_bindgen_futures::spawn_local(async move {
                                            gloo_timers::future::TimeoutFuture::new(1_000).await;
                                            let _ = window.location().set_href("/");
                                        });
                                    }
                                    Err(_) => {
                                        error_setter.set(Some("Failed to parse server response".to_string()));
                                    }
                                }
                            } else {
                                match response.json::<ErrorResponse>().await {
                                    Ok(error_response) => {
                                        error_setter.set(Some(error_response.error));
                                    }
                                    Err(_) => {
                                        error_setter.set(Some("An unknown error occurred".to_string()));
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error_setter.set(Some(format!("Request failed: {}", e)));
                        }
                    }
                });
            })
        };
        let onsubmit = {
            let email = email.clone();
            let password = password.clone();
            let phone_number = phone_number.clone();
            let error_setter = error.clone();
            let success_setter = success.clone();
          
            Callback::from(move |e: SubmitEvent| {
                e.prevent_default();
                let email = (*email).clone();
                let password = (*password).clone();
                let phone_number = (*phone_number).clone();
                let error_setter = error_setter.clone();
                let success_setter = success_setter.clone();
                if !is_valid_email(&email) {
                    error_setter.set(Some("Please enter a valid email address".to_string()));
                    return;
                }
                if !is_valid_phone(&phone_number) {
                    error_setter.set(Some("Phone number must start with '+'".to_string()));
                    return;
                }
                wasm_bindgen_futures::spawn_local(async move {
                    match Request::post(&format!("{}/api/register", config::get_backend_url()))
                        .credentials(web_sys::RequestCredentials::Include)
                        .json(&RegisterRequest {
                            email,
                            password,
                            phone_number,
                        })
                        .unwrap()
                        .send()
                        .await
                    {
                        Ok(response) => {
                            if response.ok() {
                                match response.json::<RegisterResponse>().await {
                                    Ok(resp) => {
                                        log!("Registration successful, cookies set by backend");
                                        error_setter.set(None);
                                        success_setter.set(Some(resp.message));

                                        // Redirect to home page after a short delay
                                        let window = web_sys::window().unwrap();
                                        wasm_bindgen_futures::spawn_local(async move {
                                            gloo_timers::future::TimeoutFuture::new(1_000).await;
                                            let _ = window.location().set_href("/");
                                        });
                                    }
                                    Err(_) => {
                                        error_setter.set(Some("Failed to parse server response".to_string()));
                                    }
                                }
                            } else {
                                match response.json::<ErrorResponse>().await {
                                    Ok(error_response) => {
                                        error_setter.set(Some(error_response.error));
                                    }
                                    Err(_) => {
                                        error_setter.set(Some("An unknown error occurred".to_string()));
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error_setter.set(Some(format!("Request failed: {}", e)));
                        }
                    }
                });
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
                <div class="register-container">
                    {
                        match props.self_hosting_status {
                            SelfHostingStatus::SelfHostedLogin => html! {
                                <>
                                    <h1>{"Self-Hosted Login"}</h1>
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
                                    <form onsubmit={self_hosted_login}>
                                        <input
                                            type="email"
                                            placeholder="Email"
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
                                </>
                            },
                            SelfHostingStatus::Normal => html! {
                                <>
                                    <h1>{"Register"}</h1>
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
                                    <form onsubmit={onsubmit}>
                                        <input
                                            type="email"
                                            placeholder="Email"
                                            onchange={
                                                let email = email.clone();
                                                let email_valid = email_valid.clone();
                                                let error_setter = error.clone();
                                                move |e: Event| {
                                                    let input: HtmlInputElement = e.target_unchecked_into();
                                                    let value = input.value();
                                                    let is_valid = is_valid_email(&value);
                                                    email_valid.set(is_valid);
                                                    if !is_valid {
                                                        error_setter.set(Some("Please enter a valid email address".to_string()));
                                                    } else {
                                                        error_setter.set(None);
                                                    }
                                                    email.set(value);
                                                }
                                            }
                                            class={if !*email_valid {"invalid-input"} else {""}}
                                        />
                                        <input
                                            type="tel"
                                            placeholder="Phone Number"
                                            onchange={
                                                let phone_number = phone_number.clone();
                                                let error_setter = error.clone();
                                                move |e: Event| {
                                                    let input: HtmlInputElement = e.target_unchecked_into();
                                                    let value = input.value();
                                                    if !is_valid_phone(&value) {
                                                        error_setter.set(Some("Phone number must start with '+'".to_string()));
                                                    } else {
                                                        error_setter.set(None);
                                                    }
                                                    phone_number.set(value);
                                                }
                                            }
                                        />
                                        <input
                                            type="password"
                                            placeholder="Password"
                                            onchange={let password = password.clone(); move |e: Event| {
                                                let input: HtmlInputElement = e.target_unchecked_into();
                                                password.set(input.value());
                                            }}
                                        />
                                        <div id="terms-checkbox-container">
                                            <label>
                                                <input
                                                    type="checkbox"
                                                    checked={*terms_accepted}
                                                    onchange={
                                                        let terms_accepted = terms_accepted.clone();
                                                        move |e: Event| {
                                                            let input: HtmlInputElement = e.target_unchecked_into();
                                                            terms_accepted.set(input.checked());
                                                        }
                                                    }
                                                />
                                                <span>
                                                    {"By signing up you agree to our "}
                                                    <a href="/terms" target="_blank">{"terms of service"}</a>
                                                    {" and "}
                                                    <a href="/privacy" target="_blank">{"privacy policy"}</a>
                                                    {" and consent to receive automated SMS messages from Lightfriend. Message and data rates may apply. Message frequency varies. Reply STOP to opt out."}
                                                </span>
                                            </label>
                                        </div>
                                        <button
                                            type="submit"
                                            disabled={!*terms_accepted}
                                            style={if !*terms_accepted {
                                                "opacity: 0.5; cursor: not-allowed;"
                                            } else {
                                                ""
                                            }}
                                        >
                                            {"Register"}
                                        </button>
                                    </form>
                                    <div class="auth-redirect">
                                        {"Already have an account? "}
                                        <Link<Route> to={Route::Login}>
                                            {"Login here"}
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
