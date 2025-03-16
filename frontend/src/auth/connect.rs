use yew::prelude::*;
use yew_hooks::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::{window, Window, History};
use wasm_bindgen::JsValue;
use crate::config;
use gloo_net::http::Request;
use web_sys::UrlSearchParams;

#[derive(Properties, PartialEq)]
pub struct ConnectProps {
    pub user_id: i32,
}

#[function_component(Connect)]
pub fn connect(props: &ConnectProps) -> Html {
    let error = use_state(|| None::<String>);
    let connecting = use_state(|| false);
    let is_connected = use_state(|| false);

    // Check connection status on component mount
    {
        let is_connected = is_connected.clone();
        use_effect_with_deps(
            move |_| {
                if let Some(window) = web_sys::window() {
                    if let Ok(Some(storage)) = window.local_storage() {
                        if let Ok(Some(token)) = storage.get_item("token") {
                            spawn_local(async move {
                                let request = Request::get(&format!("{}/api/auth/google/status", config::get_backend_url()))
                                    .header("Authorization", &format!("Bearer {}", token))
                                    .send()
                                    .await;

                                if let Ok(response) = request {
                                    if let Ok(data) = response.json::<serde_json::Value>().await {
                                        if let Some(connected) = data.get("connected").and_then(|v| v.as_bool()) {
                                            is_connected.set(connected);
                                        }
                                    }
                                }
                            });
                        }
                    }
                }
                || ()
            },
            (),
        );
    }

    // Check token on component mount
    use_effect_with_deps(
        |_| {
            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    if let Ok(Some(token)) = storage.get_item("token") {
                        web_sys::console::log_1(&format!("Token found in localStorage: {}", token).into());
                    } else {
                        web_sys::console::log_1(&"No token found in localStorage".into());
                    }
                }
            }
            || ()
        },
        (),
    );

    // Clean URL parameters if present (post-callback)
    use_effect_with_deps(
        move |_| {
            if let Some(window) = web_sys::window() {
                if let Ok(search) = window.location().search() {
                    if !search.is_empty() {
                        let params = UrlSearchParams::new_with_str(&search).unwrap();
                        if params.get("code").is_some() || params.get("state").is_some() {
                            web_sys::console::log_1(&"Detected callback parameters, cleaning URL".into());
                            if let Ok(history) = window.history() {
                                let _ = history.push_state_with_url(
                                    &JsValue::NULL,
                                    "",
                                    Some(&window.location().pathname().unwrap_or_default()),
                                );
                            }
                        }
                    }
                }
            }
            || ()
        },
        (),
    );

    let onclick = {
        let connecting = connecting.clone();
        let error = error.clone();
        Callback::from(move |_| {
            let connecting = connecting.clone();
            let error = error.clone();

            connecting.set(true);
            error.set(None);

            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    if let Ok(Some(token)) = storage.get_item("token") {
                        web_sys::console::log_1(&format!("Initiating OAuth flow with token: {}", token).into());
                        spawn_local(async move {
                            let request = Request::get(&format!("{}/api/auth/google/login", config::get_backend_url()))
                                .header("Authorization", &format!("Bearer {}", token))
                                .header("Content-Type", "application/json");

                            match request.send().await {
                                Ok(response) => {
                                    if response.ok() {
                                        match response.json::<serde_json::Value>().await {
                                            Ok(data) => {
                                                if let Some(auth_url) = data.get("auth_url").and_then(|u| u.as_str()) {
                                                    web_sys::console::log_1(&format!("Redirecting to auth_url: {}", auth_url).into());
                                                    if let Some(window) = web_sys::window() {
                                                        let _ = window.location().set_href(auth_url);
                                                    }
                                                } else {
                                                    web_sys::console::log_1(&"Missing auth_url in response".into());
                                                    error.set(Some("Invalid response format: missing auth_url".to_string()));
                                                }
                                            }
                                            Err(e) => {
                                                web_sys::console::log_1(&format!("Failed to parse response: {}", e).into());
                                                error.set(Some(format!("Failed to parse response: {}", e)));
                                            }
                                        }
                                    } else {
                                        match response.json::<serde_json::Value>().await {
                                            Ok(error_data) => {
                                                if let Some(error_msg) = error_data.get("error").and_then(|e| e.as_str()) {
                                                    web_sys::console::log_1(&format!("Server error: {}", error_msg).into());
                                                    error.set(Some(error_msg.to_string()));
                                                } else {
                                                    web_sys::console::log_1(&format!("Server error: Status {}", response.status()).into());
                                                    error.set(Some(format!("Server error: {}", response.status())));
                                                }
                                            }
                                            Err(_) => {
                                                web_sys::console::log_1(&format!("Server error: Status {}", response.status()).into());
                                                error.set(Some(format!("Server error: {}", response.status())));
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    web_sys::console::log_1(&format!("Network error: {}", e).into());
                                    error.set(Some(format!("Network error: {}", e)));
                                }
                            }
                            connecting.set(false);
                        });
                    } else {
                        web_sys::console::log_1(&"No token found in localStorage".into());
                        error.set(Some("Not authenticated".to_string()));
                        connecting.set(false);
                    }
                }
            }
        })
    };

    let onclick_delete = {
        let is_connected = is_connected.clone();
        let error = error.clone();
        Callback::from(move |_| {
            let is_connected = is_connected.clone();
            let error = error.clone();

            if let Some(window) = web_sys::window() {
                if let Ok(Some(storage)) = window.local_storage() {
                    if let Ok(Some(token)) = storage.get_item("token") {
                        spawn_local(async move {
                            let request = Request::delete(&format!("{}/api/auth/google/connection", config::get_backend_url()))
                                .header("Authorization", &format!("Bearer {}", token))
                                .send()
                                .await;

                            match request {
                                Ok(response) => {
                                    if response.ok() {
                                        is_connected.set(false);
                                    } else {
                                        if let Ok(error_data) = response.json::<serde_json::Value>().await {
                                            if let Some(error_msg) = error_data.get("error").and_then(|e| e.as_str()) {
                                                error.set(Some(error_msg.to_string()));
                                            } else {
                                                error.set(Some(format!("Failed to delete connection: {}", response.status())));
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    error.set(Some(format!("Network error: {}", e)));
                                }
                            }
                        });
                    }
                }
            }
        })
    };

            html! {
                <div class="connect">
                    if *is_connected {
                        <div class="calendar-controls">
                            <button 
                                onclick={onclick.clone()} 
                                class="connect-button connected"
                            >
                                {"Connected to Google Calendar ✓"}
                            </button>
                            <button 
                                onclick={onclick_delete}
                                class="disconnect-button"
                            >
                                {"Disconnect"}
                            </button>
                        </div>
                    } else {
                        <button 
                            onclick={onclick.clone()} 
                            class="connect-button"
                        >
                            if *connecting {
                                {"Connecting to Google Calendar..."}
                            } else {
                                {"Connect Google Calendar"}
                            }
                        </button>
                    }
                    <button 
                        onclick={onclick} 
                        /*disabled={*connecting || *is_connected}*/
                        class={classes!("connect-button", if *is_connected { "connected" } else { "" })}
                    >
                        if *connecting {
                            {"Connecting to Google Calendar..."}
                        } else if *is_connected {
                            {"Connected to Google Calendar ✓"}
                        } else {
                            {"Connect Google Calendar"}
                        }
                    </button>
                    if let Some(err) = (*error).as_ref() {
                        <div class="error-message">
                            {err}
                        </div>
                    }
                </div>
            }

}
