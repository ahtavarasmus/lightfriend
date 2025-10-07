pub mod login {
    use yew::prelude::*;
    use gloo_net::http::Request;
    use serde::{Deserialize, Serialize};
    use yew_router::prelude::*;
    use crate::Route;
    use crate::config;
    use gloo_timers::callback::Timeout;
    use wasm_bindgen_futures::spawn_local;
    use yew_hooks::prelude::*;
    use gloo_console::log;

    #[derive(Serialize)]
    pub struct TokenRequest {
        token: String,
    }

    #[derive(Deserialize)]
    pub struct LoginResponse {
        token: String,
    }

    #[derive(Deserialize)]
    struct ErrorResponse {
        error: String,
    }

    #[function_component]
    pub fn Login() -> Html {
        let token = use_search_param("token".to_string());
        let error = use_state(|| None::<String>);
        let success = use_state(|| None::<String>);
        let is_loading = use_state(|| false);

        // Auto-trigger login if token is present (runs on mount or param change)
        {
            let token = token.clone();
            let error_setter = error.clone();
            let success_setter = success.clone();
            let loading_setter = is_loading.clone();
            use_effect_with_deps(
                move |current_token| {
                    if let Some(ref t) = *current_token {
                        if !t.is_empty() {
                            loading_setter.set(true);
                            let error_setter = error_setter.clone();
                            let success_setter = success_setter.clone();
                            let loading_setter = loading_setter.clone();
                            let token_str = t.clone();
                            spawn_local(async move {
                                log!("Auto-logging in with token: {}", &token_str[..8.min(token_str.len())]); // Partial log for security, handle short strings
                                match Request::post(&format!("{}/api/self-hosted/login", config::get_backend_url()))
                                    .json(&TokenRequest { token: token_str })
                                    .unwrap()
                                    .send()
                                    .await
                                {
                                    Ok(response) => {
                                        if response.ok() {
                                            log!("Token login successful, parsing response...");
                                            match response.json::<LoginResponse>().await {
                                                Ok(resp) => {
                                                    let window = web_sys::window().unwrap();
                                                    if let Ok(Some(storage)) = window.local_storage() {
                                                        if storage.set_item("token", &resp.token).is_ok() {
                                                            log!("Token stored successfully");
                                                            error_setter.set(None);
                                                            success_setter.set(Some("Login successful! Redirecting...".to_string()));
                                                            loading_setter.set(false);
                                                            // Redirect after delay
                                                            let window_clone = window.clone();
                                                            spawn_local(async move {
                                                                gloo_timers::future::TimeoutFuture::new(1_000).await;
                                                                let _ = window_clone.location().set_href("/");
                                                            });
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    log!("Error parsing response: {}", e.to_string());
                                                    error_setter.set(Some("Failed to parse response".to_string()));
                                                    loading_setter.set(false);
                                                }
                                            }
                                        } else {
                                            log!("Login failed with status: {}", response.status());
                                            match response.json::<ErrorResponse>().await {
                                                Ok(err_resp) => {
                                                    log!("Server error: {}", &err_resp.error);
                                                    error_setter.set(Some(err_resp.error));
                                                }
                                                Err(_) => {
                                                    error_setter.set(Some("Login failed".to_string()));
                                                }
                                            }
                                            loading_setter.set(false);
                                        }
                                    }
                                    Err(e) => {
                                        log!("Network error: {}", e.to_string());
                                        error_setter.set(Some(format!("Request failed: {}", e)));
                                        loading_setter.set(false);
                                    }
                                }
                            });
                        }
                    }
                    || ()
                },
                token,
            );
        }

        let has_valid_token = token.as_deref().map_or(false, |t| !t.is_empty());

        html! {
            <div style="min-height: 100vh; display: flex; align-items: center; justify-content: center; padding: 2rem;">
                <style>
                {r#".login-container {
                    background: rgba(30, 30, 30, 0.7); /* Darker container */
                    border: 1px solid rgba(30, 144, 255, 0.1);
                    border-radius: 16px;
                    padding: 3rem;
                    width: 100%;
                    max-width: 480px;
                    backdrop-filter: blur(10px);
                    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
                }
                .login-container h1 {
                    font-size: 2rem;
                    margin-bottom: 1.5rem;
                    text-align: center;
                    background: linear-gradient(45deg, #fff, #7EB2FF);
                    -webkit-background-clip: text;
                    -webkit-text-fill-color: transparent;
                }
                .login-container p {
                    text-align: center;
                    color: rgba(255, 255, 255, 0.8);
                    font-size: 0.9rem;
                    margin-bottom: 1.5rem;
                }
                .no-token-message {
                    text-align: center;
                    color: rgba(255, 255, 255, 0.7);
                    font-size: 1rem;
                    margin-bottom: 1rem;
                }
                .loading-spinner {
                    display: inline-block;
                    width: 20px;
                    height: 20px;
                    border: 3px solid rgba(255,255,255,.3);
                    border-radius: 50%;
                    border-top-color: #fff;
                    animation: spin 1s ease-in-out infinite;
                }
                @keyframes spin { to { transform: rotate(360deg); } }
                @media (max-width: 768px) {
                    .login-container {
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
                    <h1>{"Login"}</h1>
                    if !has_valid_token {
                        <p class="no-token-message">
                            {"Get your magic login link from "}
                            <a href="https://lightfriend.ai" target="_blank" style="color: #7EB2FF;">{"lightfriend.ai"}</a>
                            {" and click it to access your self-hosted instance."}
                        </p>
                    } else {
                        <p>{"Verifying your magic link..."}</p>
                    }
                    {
                        if *is_loading {
                            html! { <div style="text-align: center;"><span class="loading-spinner"></span> {" Logging in..."}</div> }
                        } else if let Some(error_message) = (*error).as_ref() {
                            html! {
                                <div class="error-message" style="color: red; margin-bottom: 10px;">
                                    {error_message} {" "} <a href="https://lightfriend.ai" target="_blank" style="color: #7EB2FF;">{"Request a new link"}</a>
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
                </div>
            </div>
        }
    }
}
