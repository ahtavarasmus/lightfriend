pub mod login {
    use yew::prelude::*;
    use web_sys::HtmlInputElement;
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
    pub struct LoginRequest {
        email: String,
        password: String,
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
        let email = use_state(String::new);
        let password = use_state(String::new);
        let error = use_state(|| None::<String>);
        let success = use_state(|| None::<String>);

        let onsubmit = {
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
                    log!("Attempting login for email: {}", &email);
                    match Request::post(&format!("{}/api/self-hosted/login", config::get_backend_url()))
                        .json(&LoginRequest { email, password })
                        .unwrap()
                        .send()
                        .await
                    {
                        Ok(response) => {
                            if response.ok() {
                                log!("Login request successful, parsing response...");
                                match response.json::<LoginResponse>().await {
                                    Ok(resp) => {
                                        let window = web_sys::window().unwrap();
                                        if let Ok(Some(storage)) = window.local_storage() {
                                            if storage.set_item("token", &resp.token).is_ok() {
                                                log!("Token stored successfully in localStorage");
                                                error_setter.set(None);
                                                success_setter.set(Some("Login successful! Redirecting...".to_string()));

                                                // Redirect after a short delay to show the success message
                                                let window_clone = window.clone();
                                                wasm_bindgen_futures::spawn_local(async move {
                                                    gloo_timers::future::TimeoutFuture::new(1_000).await;
                                                    let _ = window_clone.location().set_href("/");
                                                });
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        log!("Error parsing login response:", e.to_string());
                                        error_setter.set(Some("Failed to parse server response".to_string()));
                                    }
                                }
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
                    <p>{"Login with same credentials as on lightfriend.ai"}</p>
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
                </div>
            </div>
        }
    }
}
