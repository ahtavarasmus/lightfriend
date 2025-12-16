use yew::prelude::*;
use web_sys::HtmlInputElement;
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use crate::config;

#[derive(Serialize)]
struct SetPasswordRequest {
    token: String,
    password: String,
}

#[derive(Deserialize)]
struct MagicLinkResponse {
    needs_password: bool,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Deserialize)]
struct SessionTokenResponse {
    #[serde(default)]
    token: Option<String>,
    #[serde(default)]
    existing_user: bool,
    #[serde(default)]
    new_user_check_email: bool,
}

#[derive(Deserialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Properties, PartialEq, Clone)]
pub struct SetPasswordProps {
    #[prop_or_default]
    pub token: Option<String>,
}

#[function_component]
pub fn SetPassword(props: &SetPasswordProps) -> Html {
    let password = use_state(String::new);
    let error = use_state(|| None::<String>);
    let success = use_state(|| None::<String>);
    let loading = use_state(|| true);
    let needs_password = use_state(|| true);
    let check_email = use_state(|| false);
    let token = use_state(|| props.token.clone().unwrap_or_default());
    let is_submitting = use_state(|| false);

    // On mount: extract token from props or query string, then validate
    {
        let token = token.clone();
        let loading = loading.clone();
        let error = error.clone();
        let needs_password = needs_password.clone();
        let check_email = check_email.clone();
        let prop_token = props.token.clone();

        use_effect_with_deps(move |prop_token| {
            let token = token.clone();
            let loading = loading.clone();
            let error = error.clone();
            let needs_password = needs_password.clone();
            let check_email = check_email.clone();
            let prop_token = prop_token.clone();

            wasm_bindgen_futures::spawn_local(async move {
                // Get token from props or query string
                let final_token = if let Some(t) = prop_token {
                    t
                } else {
                    // Try to get session_id from query string
                    if let Some(window) = web_sys::window() {
                        if let Ok(search) = window.location().search() {
                            let params = web_sys::UrlSearchParams::new_with_str(&search).ok();
                            if let Some(params) = params {
                                if let Some(session_id) = params.get("session_id") {
                                    // Fetch token from session_id
                                    match Request::get(&format!(
                                        "{}/api/auth/session-token/{}",
                                        config::get_backend_url(),
                                        session_id
                                    ))
                                    .send()
                                    .await
                                    {
                                        Ok(response) => {
                                            if response.ok() {
                                                if let Ok(resp) = response.json::<SessionTokenResponse>().await {
                                                    // Check if this is an existing user checkout
                                                    if resp.existing_user {
                                                        // Redirect to login instead of auto-logging in
                                                        if let Some(window) = web_sys::window() {
                                                            let _ = window.location().set_href("/login?subscription=activated");
                                                        }
                                                        return;
                                                    }
                                                    // Check if this is a new user who needs to check email
                                                    if resp.new_user_check_email {
                                                        check_email.set(true);
                                                        loading.set(false);
                                                        return;
                                                    }
                                                    if let Some(token) = resp.token {
                                                        token
                                                    } else {
                                                        error.set(Some("No token in response".to_string()));
                                                        loading.set(false);
                                                        return;
                                                    }
                                                } else {
                                                    error.set(Some("Failed to parse session response".to_string()));
                                                    loading.set(false);
                                                    return;
                                                }
                                            } else {
                                                error.set(Some("Session not found. Try clicking the link in your email.".to_string()));
                                                loading.set(false);
                                                return;
                                            }
                                        }
                                        Err(e) => {
                                            error.set(Some(format!("Request failed: {}", e)));
                                            loading.set(false);
                                            return;
                                        }
                                    }
                                } else {
                                    error.set(Some("No token or session_id provided".to_string()));
                                    loading.set(false);
                                    return;
                                }
                            } else {
                                error.set(Some("Invalid URL parameters".to_string()));
                                loading.set(false);
                                return;
                            }
                        } else {
                            error.set(Some("Could not read URL".to_string()));
                            loading.set(false);
                            return;
                        }
                    } else {
                        error.set(Some("No window object".to_string()));
                        loading.set(false);
                        return;
                    }
                };

                token.set(final_token.clone());

                // Validate the token
                match Request::get(&format!(
                    "{}/api/auth/magic/{}",
                    config::get_backend_url(),
                    final_token
                ))
                .credentials(web_sys::RequestCredentials::Include)
                .send()
                .await
                {
                    Ok(response) => {
                        if response.ok() {
                            if let Ok(resp) = response.json::<MagicLinkResponse>().await {
                                if resp.needs_password {
                                    needs_password.set(true);
                                    loading.set(false);
                                } else {
                                    // Already has password - user is now logged in, redirect to home
                                    if let Some(window) = web_sys::window() {
                                        let _ = window.location().set_href("/");
                                    }
                                }
                            } else {
                                error.set(Some("Failed to parse response".to_string()));
                                loading.set(false);
                            }
                        } else {
                            if let Ok(err_resp) = response.json::<ErrorResponse>().await {
                                error.set(Some(err_resp.error));
                            } else {
                                error.set(Some("Invalid or expired link".to_string()));
                            }
                            loading.set(false);
                        }
                    }
                    Err(e) => {
                        error.set(Some(format!("Request failed: {}", e)));
                        loading.set(false);
                    }
                }
            });

            || ()
        }, prop_token);
    }

    let onsubmit = {
        let password = password.clone();
        let token = token.clone();
        let error = error.clone();
        let success = success.clone();
        let is_submitting = is_submitting.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();

            let pwd = (*password).clone();
            let tok = (*token).clone();
            let error = error.clone();
            let success = success.clone();
            let is_submitting = is_submitting.clone();

            if pwd.is_empty() {
                error.set(Some("Please enter a password".to_string()));
                return;
            }

            if pwd.len() < 8 {
                error.set(Some("Password must be at least 8 characters".to_string()));
                return;
            }

            is_submitting.set(true);

            wasm_bindgen_futures::spawn_local(async move {
                match Request::post(&format!("{}/api/auth/set-password", config::get_backend_url()))
                    .credentials(web_sys::RequestCredentials::Include)
                    .json(&SetPasswordRequest {
                        token: tok,
                        password: pwd,
                    })
                    .unwrap()
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.ok() {
                            error.set(None);
                            success.set(Some("Password set successfully! Redirecting...".to_string()));

                            // Redirect to home after success
                            if let Some(window) = web_sys::window() {
                                gloo_timers::callback::Timeout::new(1_500, move || {
                                    let _ = window.location().set_href("/");
                                })
                                .forget();
                            }
                        } else {
                            is_submitting.set(false);
                            if let Ok(err_resp) = response.json::<ErrorResponse>().await {
                                error.set(Some(err_resp.error));
                            } else {
                                error.set(Some("Failed to set password".to_string()));
                            }
                        }
                    }
                    Err(e) => {
                        is_submitting.set(false);
                        error.set(Some(format!("Request failed: {}", e)));
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
                <h1>{"Set Your Password"}</h1>

                {
                    if *loading {
                        html! {
                            <div style="text-align: center; color: rgba(255, 255, 255, 0.7);">
                                {"Validating your link..."}
                            </div>
                        }
                    } else if let Some(error_message) = (*error).as_ref() {
                        html! {
                            <div style="text-align: center;">
                                <div class="error-message" style="color: #ff6b6b; margin-bottom: 1.5rem;">
                                    {error_message}
                                </div>
                                <p style="color: rgba(255, 255, 255, 0.6); font-size: 0.9rem;">
                                    {"If you need a new link, check your email or contact support."}
                                </p>
                            </div>
                        }
                    } else if let Some(success_message) = (*success).as_ref() {
                        html! {
                            <div class="success-message" style="color: #4ecdc4; text-align: center;">
                                {success_message}
                            </div>
                        }
                    } else if *check_email {
                        html! {
                            <div style="text-align: center;">
                                <div style="color: #4ecdc4; font-size: 1.2rem; margin-bottom: 1.5rem;">
                                    {"Thank you for subscribing!"}
                                </div>
                                <p style="color: rgba(255, 255, 255, 0.7); margin-bottom: 1.5rem;">
                                    {"We've sent you an email with a link to set your password and access your account."}
                                </p>
                                <p style="color: rgba(255, 255, 255, 0.5); font-size: 0.9rem;">
                                    {"Please check your inbox (and spam folder) for the email from Lightfriend."}
                                </p>
                            </div>
                        }
                    } else if *needs_password {
                        html! {
                            <>
                                <p style="color: rgba(255, 255, 255, 0.7); margin-bottom: 1.5rem; text-align: center;">
                                    {"Welcome to Lightfriend! Please set a password for your account."}
                                </p>
                                <form onsubmit={onsubmit}>
                                    <input
                                        type="password"
                                        placeholder="Password (min 8 characters)"
                                        autocomplete="new-password"
                                        disabled={*is_submitting}
                                        onchange={let password = password.clone(); move |e: Event| {
                                            let input: HtmlInputElement = e.target_unchecked_into();
                                            password.set(input.value());
                                        }}
                                    />
                                    <button type="submit" disabled={*is_submitting}>
                                        {if *is_submitting { "Setting Password..." } else { "Set Password" }}
                                    </button>
                                </form>
                            </>
                        }
                    } else {
                        html! {}
                    }
                }
            </div>
        </div>
    }
}
