use crate::config;
use crate::utils::datafast::track_goal;
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use web_sys::HtmlInputElement;
use yew::prelude::*;

#[derive(Serialize)]
struct SetPasswordRequest {
    token: String,
    password: String,
}

#[derive(Deserialize)]
struct MagicLinkResponse {
    needs_password: bool,
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
    let token = use_state(|| props.token.clone().unwrap_or_default());
    let is_submitting = use_state(|| false);

    // On mount: extract token from props or query string, then validate
    {
        let token = token.clone();
        let loading = loading.clone();
        let error = error.clone();
        let needs_password = needs_password.clone();
        let prop_token = props.token.clone();

        use_effect_with_deps(
            move |prop_token| {
                let token = token.clone();
                let loading = loading.clone();
                let error = error.clone();
                let needs_password = needs_password.clone();
                let prop_token = prop_token.clone();

                wasm_bindgen_futures::spawn_local(async move {
                    let final_token = if let Some(token) = prop_token {
                        token
                    } else {
                        error.set(Some(
                            "Invalid password setup link. Use the link sent to your email."
                                .to_string(),
                        ));
                        loading.set(false);
                        return;
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
                                            let _ = window.location().set_href("/app");
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
            },
            prop_token,
        );
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
                match Request::post(&format!(
                    "{}/api/auth/set-password",
                    config::get_backend_url()
                ))
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
                            track_goal(
                                "registration_complete",
                                &[("registration_type", "checkout_magic_link")],
                            );
                            error.set(None);
                            success.set(Some(
                                "Password set successfully! Redirecting...".to_string(),
                            ));

                            // Redirect to home after success
                            if let Some(window) = web_sys::window() {
                                gloo_timers::callback::Timeout::new(1_500, move || {
                                    let _ = window.location().set_href("/app");
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
        <div class="auth-page-shell">
            <style>
            {r#".login-container,
.register-container {
    background: rgba(255, 255, 255, 0.105);
    border: 1px solid rgba(255, 255, 255, 0.22);
    border-radius: 8px;
    padding: 3rem;
    width: 100%;
    max-width: 480px;
    backdrop-filter: blur(14px) saturate(1.1);
    -webkit-backdrop-filter: blur(14px) saturate(1.1);
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.18), 0 20px 70px rgba(20, 36, 48, 0.16);
}
.login-container h1,
.register-container h1 {
    font-size: 2rem;
    margin-bottom: 1.5rem;
    text-align: center;
    color: #fff;
    background: none;
    -webkit-text-fill-color: currentColor;
    text-shadow: 0 2px 18px rgba(0, 0, 0, 0.46), 0 1px 2px rgba(0, 0, 0, 0.42);
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
    inset: 0;
    height: 100vh;
    background-image: url('/assets/child-field-hero.png');
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
    inset: 0;
    background: linear-gradient(to bottom,
        rgba(13, 13, 13, 0.08) 0%,
        rgba(13, 13, 13, 0.18) 35%,
        rgba(13, 13, 13, 0.74) 100%
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
