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
        let email= use_state(String::new);
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
                    println!("Attempting login for email: {}", &email);
                    match Request::post(&format!("{}/api/login", config::get_backend_url()))
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
        <div class="min-h-screen gradient-bg">
            <div class="login-container">
                <h1>{"Login"}</h1>
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
            <div class="min-h-screen gradient-bg">
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

    #[derive(Serialize)]
    pub struct RegisterRequest {
        email: String,
        password: String,
        phone_number: String,
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
    pub fn Register() -> Html {
        let email = use_state(String::new);
        let password = use_state(String::new);
        let phone_number = use_state(String::new);
        let error = use_state(|| None::<String>);
        let success = use_state(|| None::<String>);
        let email_valid = use_state(|| true); // Track email validity
        let terms_accepted = use_state(|| false); // Track terms acceptance

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
                        .json(&RegisterRequest { 
                            email, 
                            password,
                            phone_number,
                        })
                        .unwrap()
                        .send()
                        .await 
                    {
                        Ok(resp) => {
                            if resp.ok() {
                                match resp.json::<RegisterResponse>().await {
                                    Ok(resp) => {
                                        let window = web_sys::window().unwrap();
                                        if let Ok(Some(storage)) = window.local_storage() {
                                            if storage.set_item("token", &resp.token).is_ok() {
                                                error_setter.set(None);
                                                success_setter.set(Some(resp.message));
                                                
                                                // Redirect to pricing page after a short delay
                                                let window_clone = window.clone();
                                                wasm_bindgen_futures::spawn_local(async move {
                                                    gloo_timers::future::TimeoutFuture::new(1_000).await;
                                                    let _ = window_clone.location().set_href("/");
                                                });
                                            }
                                        }
                                    }
                                    Err(_) => {
                                        error_setter.set(Some("Failed to parse server response".to_string()));
                                    }
                                }
                            } else {
                                match resp.json::<ErrorResponse>().await {
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

        <div class="min-h-screen gradient-bg">
            <div class="register-container">
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
                                <a href="/terms" target="_blank" style="color: #007bff; text-decoration: underline;">{"terms of service"}</a>
                                {" and "}
                                <a href="/privacy" target="_blank" style="color: #007bff; text-decoration: underline;">{"privacy policy"}</a>
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
            </div>
        </div>
        }
    }
}


