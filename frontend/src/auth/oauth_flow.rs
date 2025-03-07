use yew::prelude::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use web_sys::window;
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};

#[derive(Properties, PartialEq)]
pub struct OAuthButtonProps {
    pub user_id: i32,
}

#[derive(Deserialize, Debug)]
struct AuthParams {
    auth_url: String,
    state: String,
}

#[function_component(GoogleCalendarConnect)]
pub fn google_calendar_connect(props: &OAuthButtonProps) -> Html {
    let connecting = use_state(|| false);
    
    let onclick = {
        let connecting = connecting.clone();
        let user_id = props.user_id;
        
        Callback::from(move |_| {
            connecting.set(true);
            let connecting = connecting.clone();
            
            spawn_local(async move {
                match Request::get("/api/oauth/auth-params")
                    .send()
                    .await {
                    Ok(response) => {
                        if let Ok(params) = response.json::<AuthParams>().await {
                            // Store state in localStorage for verification
                            if let Some(window) = window() {
                                if let Ok(storage) = window.local_storage() {
                                    if let Some(storage) = storage {
                                        let _ = storage.set_item("oauth_state", &params.state);
                                    }
                                }
                            }
                            
                            // Redirect to Google auth URL
                            if let Some(window) = window() {
                                let _ = window.location().set_href(&params.auth_url);
                            }
                        }
                    }
                    Err(_) => {
                        // Handle error
                        connecting.set(false);
                    }
                }
            });
        })
    };

    html! {
        <button 
            onclick={onclick}
            disabled={*connecting}
            class="btn btn-primary"
        >
            if *connecting {
                {"Connecting..."}
            } else {
                {"Connect Google Calendar"}
            }
        </button>
    }
}

