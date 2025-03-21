use yew::prelude::*;
use web_sys::window;
use crate::config;
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use yew_router::prelude::*;
use crate::Route;
use chrono::{Utc, TimeZone};

#[derive(Serialize)]
struct BroadcastMessage {
    message: String,
}


#[derive(Deserialize, Clone, Debug)]
struct UserInfo {
    id: i32,
    email: String,
    phone_number: String,
    time_to_live: Option<i32>,
    verified: bool,
    credits: f32,
    notify: bool,
    preferred_number: Option<String>,
}

#[derive(Clone, Debug)]
struct DeleteModalState {
    show: bool,
    user_id: Option<i32>,
    user_email: Option<String>,
}

#[function_component]
pub fn AdminDashboard() -> Html {
    let users = use_state(|| Vec::new());
    let error = use_state(|| None::<String>);
    let selected_user_id = use_state(|| None::<i32>);
    let message = use_state(|| String::new());
    let delete_modal = use_state(|| DeleteModalState {
        show: false,
        user_id: None,
        user_email: None,
    });

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
                                            <th>{"Credits"}</th>
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
                                                                <td>{format!("{:.2}", user.credits)}</td>
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
                                                                            <p><strong>{"Notify: "}</strong>{if user.notify { "Yes" } else { "No" }}</p>
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
                                                                                            match Request::post(&format!("{}/api/billing/increase-credits/{}", config::get_backend_url(), user_id))
                                                                                                .header("Authorization", &format!("Bearer {}", token))
                                                                                                .send()
                                                                                                .await
                                                                                            {
                                                                                                Ok(response) => {
                                                                                                    if response.ok() {
                                                                                                        // Refresh the users list after increasing credits 
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
                                                                                                        error.set(Some("Failed to increase Credits".to_string()));
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
                                                                            {"Get €2.00 credits"}
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
                                                                                            match Request::post(&format!("{}/api/billing/reset-credits/{}", config::get_backend_url(), user_id))
                                                                                                .header("Authorization", &format!("Bearer {}", token))
                                                                                                .send()
                                                                                                .await
                                                                                            {
                                                                                                Ok(response) => {
                                                                                                    if response.ok() {
                                                                                                        // Refresh the users list after resetting credits
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
                                                                                                        error.set(Some("Failed to reset credits".to_string()));
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
                                                                            {"Reset credits"}
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
                                                                                            match Request::post(&format!("{}/api/admin/set-preferred-number-default/{}", config::get_backend_url(), user_id))
                                                                                                .header("Authorization", &format!("Bearer {}", token))
                                                                                                .send()
                                                                                                .await
                                                                                            {
                                                                                                Ok(response) => {
                                                                                                    if response.ok() {
                                                                                                        // Refresh the users list after setting preferred number
                                                                                                        if let Ok(response) = Request::get(&format!("{}/api/admin/users", config::get_backend_url()))
                                                                                                            .header("Authorization", &format!("Bearer {}", token))
                                                                                                            .send()
                                                                                                            .await
                                                                                                        {
                                                                                                            if let Ok(updated_users) = response.json::<Vec<UserInfo>>().await {
                                                                                                                users.set(updated_users);
                                                                                                                error.set(None);
                                                                                                            }
                                                                                                        }
                                                                                                    } else {
                                                                                                        error.set(Some("Failed to set preferred number".to_string()));
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
                                                                            {"Set Default Number"}
                                                                        </button>
                                                                        <button 
                                                                            onclick={{
                                                                                let delete_modal = delete_modal.clone();
                                                                                let user_id = user.id;
                                                                                let user_email = user.email.clone();
                                                                                Callback::from(move |_| {
                                                                                    delete_modal.set(DeleteModalState {
                                                                                        show: true,
                                                                                        user_id: Some(user_id),
                                                                                        user_email: Some(user_email.clone()),
                                                                                    });
                                                                                })
                                                                            }}
                                                                            class="iq-button delete"
                                                                        >
                                                                            {"Delete User"}
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
            {
                if delete_modal.show {
                    html! {
                        <div class="modal-overlay">
                            <div class="modal-content">
                                <h2>{"Confirm Delete"}</h2>
                                <p>{format!("Are you sure you want to delete user {}?", delete_modal.user_email.clone().unwrap_or_default())}</p>
                                <p class="warning">{"This action cannot be undone!"}</p>
                                <div class="modal-buttons">
                                    <button 
                                        onclick={{
                                            let delete_modal = delete_modal.clone();
                                            Callback::from(move |_| {
                                                delete_modal.set(DeleteModalState {
                                                    show: false,
                                                    user_id: None,
                                                    user_email: None,
                                                });
                                            })
                                        }}
                                        class="modal-button cancel"
                                    >
                                        {"Cancel"}
                                    </button>
                                    <button 
                                        onclick={{
                                            let delete_modal = delete_modal.clone();
                                            let users = users.clone();
                                            let error = error.clone();
                                            Callback::from(move |_| {
                                                let users = users.clone();
                                                let error = error.clone();
                                                let delete_modal = delete_modal.clone();
                                                let user_id = delete_modal.user_id.unwrap();
                                                
                                                wasm_bindgen_futures::spawn_local(async move {
                                                    if let Some(token) = window()
                                                        .and_then(|w| w.local_storage().ok())
                                                        .flatten()
                                                        .and_then(|storage| storage.get_item("token").ok())
                                                        .flatten()
                                                    {
                                                        match Request::delete(&format!("{}/api/profile/delete/{}", config::get_backend_url(), user_id))
                                                            .header("Authorization", &format!("Bearer {}", token))
                                                            .send()
                                                            .await
                                                        {
                                                            Ok(response) => {
                                                                if response.ok() {
                                                                    // Remove the deleted user from the users list
                                                                    users.set((*users).clone().into_iter().filter(|u| u.id != user_id).collect());
                                                                    delete_modal.set(DeleteModalState {
                                                                        show: false,
                                                                        user_id: None,
                                                                        user_email: None,
                                                                    });
                                                                    error.set(Some("User deleted successfully".to_string()));
                                                                } else {
                                                                    error.set(Some("Failed to delete user".to_string()));
                                                                }
                                                            }
                                                            Err(_) => {
                                                                error.set(Some("Failed to send delete request".to_string()));
                                                            }
                                                        }
                                                    }
                                                });
                                            })
                                        }}
                                        class="modal-button delete"
                                    >
                                        {"Delete"}
                                    </button>
                                </div>
                            </div>
                        </div>
                    }
                } else {
                    html! {}
                }
            }
        </div>
    }
}
                }
            </div>
        </div>
    }
}
