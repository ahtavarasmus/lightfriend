use yew::prelude::*;
use web_sys::window;
use crate::config;
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use yew_router::prelude::*;
use serde_json::json;
use crate::Route;
use chrono::{DateTime, Utc, TimeZone};
use wasm_bindgen::JsCast;

#[derive(Serialize)]
struct BroadcastMessage {
    message: String,
}

#[derive(Deserialize, Clone, Debug)]
struct PhoneNumbers {
    usa: String,
    fin: String,
    nld: String,
    chz: String,
}

#[derive(Deserialize, Clone, Debug)]
struct UserInfo {
    id: i32,
    email: String,
    phone_number: String,
    nickname: Option<String>,
    time_to_live: Option<i32>,
    verified: bool,
    iq: i32,
    notify_credits: bool,
    preferred_number: Option<String>,
}

#[derive(Serialize)]
struct UpdateUserRequest {
    email: String,
    phone_number: String,
    nickname: Option<String>,
    time_to_live: Option<i32>,
    verified: bool,
}

#[function_component]
pub fn Admin() -> Html {
    let users = use_state(|| Vec::new());
    let error = use_state(|| None::<String>);
    let selected_user_id = use_state(|| None::<i32>);
    let message = use_state(|| String::new());
    let phone_numbers = use_state(|| None::<PhoneNumbers>);

    // Clone state handles for the phone numbers effect
    let phone_numbers_effect = phone_numbers.clone();
    
    // Fetch phone numbers from environment
    use_effect_with_deps(move |_| {
        wasm_bindgen_futures::spawn_local(async move {
            if let Some(token) = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|storage| storage.get_item("token").ok())
                .flatten()
            {
                match Request::get(&format!("{}/api/admin/phone-numbers", config::get_backend_url()))
                    .header("Authorization", &format!("Bearer {}", token))
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.ok() {
                            if let Ok(numbers) = response.json::<PhoneNumbers>().await {
                                phone_numbers_effect.set(Some(numbers));
                            }
                        }
                    }
                    Err(_) => {}
                }
            }
        });
        || ()
    }, ());

    // Clone state handles for the effect
    let users_effect = users.clone();
    let error_effect = error.clone();

    use_effect_with_deps(move |_| {
        let users = users_effect;
        let error = error_effect;
        wasm_bindgen_futures::spawn_local(async move {
            // Get token from localStorage
            let token = window()
                .and_then(|w| w.local_storage().ok())
                .flatten()
                .and_then(|storage| storage.get_item("token").ok())
                .flatten();

            if let Some(token) = token {
                match Request::get(&format!("{}/api/admin/users", config::get_backend_url()))
                    .header("Authorization", &format!("Bearer {}", token))
                    .send()
                    .await
                {
                    Ok(response) => {
                        if response.ok() {
                            match response.json::<Vec<UserInfo>>().await {
                                Ok(data) => {
                                    users.set(data);
                                }
                                Err(_) => {
                                    error.set(Some("Failed to parse users data".to_string()));
                                }
                            }
                        } else {
                            error.set(Some("Not authorized to view this page".to_string()));
                        }
                    }
                    Err(_) => {
                        error.set(Some("Failed to fetch users".to_string()));
                    }
                }
            }
        });
        || ()
    }, ());

    let toggle_user_details = {
        let selected_user_id = selected_user_id.clone();
        Callback::from(move |user_id: i32| {
            selected_user_id.set(Some(match *selected_user_id {
                Some(current_id) if current_id == user_id => return selected_user_id.set(None),
                _ => user_id
            }));
        })
    };

    html! {
        <div class="dashboard-container">
            <div class="dashboard-panel">
                <div class="panel-header">
                    <h1 class="panel-title">{"Admin Dashboard"}</h1>
                    <Link<Route> to={Route::Home} classes="back-link">
                        {"Back to Home"}
                    </Link<Route>>
                </div>
                
                <div class="broadcast-section">
                    <h2>{"Broadcast Message"}</h2>
                    <textarea
                        value={(*message).clone()}
                        onchange={{
                            let message = message.clone();
                            Callback::from(move |e: Event| {
                                let input: web_sys::HtmlTextAreaElement = e.target_unchecked_into();
                                message.set(input.value());
                            })
                        }}
                        placeholder="Enter message to broadcast..."
                        class="broadcast-textarea"
                    />
                    <button
                        onclick={{
                            let message = message.clone();
                            let error = error.clone();
                            Callback::from(move |_| {
                                let message = message.clone();
                                let error = error.clone();
                                
                                if message.is_empty() {
                                    error.set(Some("Message cannot be empty".to_string()));
                                    return;
                                }
                                
                                wasm_bindgen_futures::spawn_local(async move {
                                    if let Some(token) = window()
                                        .and_then(|w| w.local_storage().ok())
                                        .flatten()
                                        .and_then(|storage| storage.get_item("token").ok())
                                        .flatten()
                                    {
                                        let broadcast_message = BroadcastMessage {
                                            message: (*message).clone(),
                                        };
                                        
                                        match Request::post(&format!("{}/api/admin/broadcast", config::get_backend_url()))
                                            .header("Authorization", &format!("Bearer {}", token))
                                            .json(&broadcast_message)
                                            .unwrap()
                                            .send()
                                            .await
                                        {
                                            Ok(response) => {
                                                if response.ok() {
                                                    message.set(String::new());
                                                    error.set(Some("Message sent successfully".to_string()));
                                                } else {
                                                    error.set(Some("Failed to send message".to_string()));
                                                }
                                            }
                                            Err(_) => {
                                                error.set(Some("Failed to send request".to_string()));
                                            }
                                        }
                                    }
                                });
                            })
                        }}
                        class="broadcast-button"
                    >
                        {"Send Broadcast(only works with admin)"}
                    </button>
                </div>

                {
                    if let Some(error_msg) = (*error).as_ref() {
                        html! {
                            <div class="info-section error">
                                <span class="error-message">{error_msg}</span>
                            </div>
                        }
                    } else {
                        html! {
                            <div class="info-section">
                                <h2 class="section-title">{"Users List"}</h2>
                                <div class="users-table-container">
                                    <table class="users-table">
                                        <thead>
                                            <tr>
                                                <th>{"ID"}</th>
                                                <th>{"Email"}</th>
                                            <th>{"IQ"}</th>
                                            </tr>
                                        </thead>
                                        <tbody>
                                            {
                                                users.iter().map(|user| {
                                                    let is_selected = selected_user_id.as_ref() == Some(&user.id);
                                                    let user_id = user.id;
                                                    let onclick = toggle_user_details.reform(move |_| user_id);
                                                    
                                                    html! {
                                                        <>
                                                            <tr onclick={onclick} key={user.id} class={classes!("user-row", is_selected.then(|| "selected"))}>
                                                                <td>{user.id}</td>
                                                                <td>{&user.email}</td>
                                                                <td>{user.iq}</td>
                                                            </tr>
                                                            if is_selected {
                                                                <tr class="details-row">
                                                                    <td colspan="4">
                                                                        <div class="user-details">
                                                                            <p><strong>{"Phone Number: "}</strong>{&user.phone_number}</p>
                                                                            <p><strong>{"Joined at: "}</strong>{
                                                                                user.time_to_live.map_or("N/A".to_string(), |ttl| {
                                                                                    Utc.timestamp_opt(ttl as i64, 0)
                                                                                        .single()
                                                                                        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                                                                                        .unwrap_or_else(|| "Invalid timestamp".to_string())
                                                                                })
                                                                            }</p>
                                                                            <p><strong>{"Notify: "}</strong>{if user.notify_credits { "Yes" } else { "No" }}</p>
                                                                            <div class="preferred-number-section">
                                                                                <p><strong>{"Current Preferred Number: "}</strong>{user.preferred_number.clone().unwrap_or_else(|| "Not set".to_string())}</p>
                                                                            </div>
                                                                            
                                                                        <button 
                                                                            onclick={{
                                                                                let users = users.clone();
                                                                                let error = error.clone();
                                                                                let user_id = user.id;
                                                                                Callback::from(move |_| {
                                                                                    let users = users.clone();
                                                                                    let error = error.clone();
                                                                                    wasm_bindgen_futures::spawn_local(async move {
                                                                                        if let Some(token) = window()
                                                                                            .and_then(|w| w.local_storage().ok())
                                                                                            .flatten()
                                                                                            .and_then(|storage| storage.get_item("token").ok())
                                                                                            .flatten()
                                                                                        {
                                                                                            match Request::post(&format!("{}/api/profile/increase-iq/{}", config::get_backend_url(), user_id))
                                                                                                .header("Authorization", &format!("Bearer {}", token))
                                                                                                .send()
                                                                                                .await
                                                                                            {
                                                                                                Ok(response) => {
                                                                                                    if response.ok() {
                                                                                                        // Refresh the users list after increasing IQ
                                                                                                        if let Ok(response) = Request::get(&format!("{}/api/admin/users", config::get_backend_url()))
                                                                                                            .header("Authorization", &format!("Bearer {}", token))
                                                                                                            .send()
                                                                                                            .await
                                                                                                        {
                                                                                                            if let Ok(updated_users) = response.json::<Vec<UserInfo>>().await {
                                                                                                                users.set(updated_users);
                                                                                                            }
                                                                                                        }
                                                                                                    } else {
                                                                                                        error.set(Some("Failed to increase IQ".to_string()));
                                                                                                    }
                                                                                                }
                                                                                                Err(_) => {
                                                                                                    error.set(Some("Failed to send request".to_string()));
                                                                                                }
                                                                                            }
                                                                                        }
                                                                                    });
                                                                                })
                                                                            }}
                                                                            class="iq-button"
                                                                        >
                                                                            {"Get 500 IQ"}
                                                                        </button>
                                                                        <button 
                                                                            onclick={{
                                                                                let users = users.clone();
                                                                                let error = error.clone();
                                                                                let user_id = user.id;
                                                                                Callback::from(move |_| {
                                                                                    let users = users.clone();
                                                                                    let error = error.clone();
                                                                                    wasm_bindgen_futures::spawn_local(async move {
                                                                                        if let Some(token) = window()
                                                                                            .and_then(|w| w.local_storage().ok())
                                                                                            .flatten()
                                                                                            .and_then(|storage| storage.get_item("token").ok())
                                                                                            .flatten()
                                                                                        {
                                                                                            match Request::post(&format!("{}/api/profile/reset-iq/{}", config::get_backend_url(), user_id))
                                                                                                .header("Authorization", &format!("Bearer {}", token))
                                                                                                .send()
                                                                                                .await
                                                                                            {
                                                                                                Ok(response) => {
                                                                                                    if response.ok() {
                                                                                                        // Refresh the users list after resetting IQ
                                                                                                        if let Ok(response) = Request::get(&format!("{}/api/admin/users", config::get_backend_url()))
                                                                                                            .header("Authorization", &format!("Bearer {}", token))
                                                                                                            .send()
                                                                                                            .await
                                                                                                        {
                                                                                                            if let Ok(updated_users) = response.json::<Vec<UserInfo>>().await {
                                                                                                                users.set(updated_users);
                                                                                                            }
                                                                                                        }
                                                                                                    } else {
                                                                                                        error.set(Some("Failed to reset IQ".to_string()));
                                                                                                    }
                                                                                                }
                                                                                                Err(_) => {
                                                                                                    error.set(Some("Failed to send request".to_string()));
                                                                                                }
                                                                                            }
                                                                                        }
                                                                                    });
                                                                                })
                                                                            }}
                                                                            class="iq-button reset"
                                                                        >
                                                                            {"Reset IQ"}
                                                                        </button>
                                                                        <button 
                                                                            onclick={{
                                                                                let users = users.clone();
                                                                                let error = error.clone();
                                                                                let user_id = user.id;
                                                                                Callback::from(move |_| {
                                                                                    let users = users.clone();
                                                                                    let error = error.clone();
                                                                                    wasm_bindgen_futures::spawn_local(async move {
                                                                                        if let Some(token) = window()
                                                                                            .and_then(|w| w.local_storage().ok())
                                                                                            .flatten()
                                                                                            .and_then(|storage| storage.get_item("token").ok())
                                                                                            .flatten()
                                                                                        {
                                                                                            match Request::post(&format!("{}/api/admin/verify/{}", config::get_backend_url(), user_id))
                                                                                                .header("Authorization", &format!("Bearer {}", token))
                                                                                                .send()
                                                                                                .await
                                                                                            {
                                                                                                Ok(response) => {
                                                                                                    if response.ok() {
                                                                                                        // Refresh the users list after verifying
                                                                                                        if let Ok(response) = Request::get(&format!("{}/api/admin/users", config::get_backend_url()))
                                                                                                            .header("Authorization", &format!("Bearer {}", token))
                                                                                                            .send()
                                                                                                            .await
                                                                                                        {
                                                                                                            if let Ok(updated_users) = response.json::<Vec<UserInfo>>().await {
                                                                                                                users.set(updated_users);
                                                                                                            }
                                                                                                        }
                                                                                                    } else {
                                                                                                        error.set(Some("Failed to verify user".to_string()));
                                                                                                    }
                                                                                                }
                                                                                                Err(_) => {
                                                                                                    error.set(Some("Failed to send verification request".to_string()));
                                                                                                }
                                                                                            }
                                                                                        }
                                                                                    });
                                                                                })
                                                                            }}
                                                                            class="iq-button"
                                                                            disabled={user.verified}
                                                                        >
                                                                            {if user.verified { "Verified" } else { "Verify User" }}
                                                                        </button>
                                                                        </div>
                                                                    </td>
                                                                </tr>
                                                            }
                                                        </>
                                                    }
                                                }).collect::<Html>()
                                            }
                                        </tbody>
                                    </table>
                                </div>
                            </div>
                        }
                    }
                }
            </div>
        </div>
    }
}
