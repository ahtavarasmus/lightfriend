pub mod home {
    use yew::prelude::*;
    use yew_router::prelude::*;
    use crate::Route;
    use web_sys::window;

    fn is_logged_in() -> bool {
        if let Some(window) = window() {
            if let Ok(Some(storage)) = window.local_storage() {
                if let Ok(Some(_token)) = storage.get_item("token") {
                    return true;
                }
            }
        }
        false
    }

    #[function_component]
    pub fn Home() -> Html {
        let logged_in = is_logged_in();

        let handle_logout = {
            Callback::from(move |_| {
                if let Some(window) = window() {
                    if let Ok(Some(storage)) = window.local_storage() {
                        let _ = storage.remove_item("token");
                        // Reload the page to reflect the logged out state
                        let _ = window.location().reload();
                    }
                }
            })
        };

        html! {
            <div class="home-container">
                <h1>{"Welcome to Our App"}</h1>
                {
                    if logged_in {
                        html! {
                            <div class="user-panel">
                                <p>{"You are logged in!"}</p>
                                <button onclick={handle_logout}>{"Logout"}</button>
                            </div>
                        }
                    } else {
                        html! {
                            <div class="auth-links">
                                <Link<Route> to={Route::Login}>
                                    {"Login"}
                                </Link<Route>>
                                {" or "}
                                <Link<Route> to={Route::Register}>
                                    {"Register"}
                                </Link<Route>>
                            </div>
                        }
                    }
                }
            </div>
        }
    }
}


pub mod profile {
    use yew::prelude::*;
    use web_sys::{HtmlInputElement, window};
    use gloo_net::http::Request;
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize)]
    struct UserProfile {
        username: String,
        email: String,
        phone_number: Option<String>,
    }

    #[derive(Serialize)]
    struct UpdateProfileRequest {
        phone_number: String,
    }

    #[function_component]
    pub fn Profile() -> Html {
            let profile = use_state(|| None::<UserProfile>);
            let phone_number = use_state(String::new);
            let error = use_state(|| None::<String>);
            let success = use_state(|| None::<String>);
            let is_editing = use_state(|| false);

        // Fetch user profile 
        {
            let profile = profile.clone();
            let error = error.clone();

            use_effect_with_deps(move |_| {
                wasm_bindgen_futures::spawn_local(async move {
                    if let Some(token) = window()
                        .and_then(|w| w.local_storage().ok())
                        .flatten()
                        .and_then(|storage| storage.get_item("token").ok())
                        .flatten()
                    {
                        match Request::get("http://localhost:3000/api/profile")
                            .header("Authorization", &format!("Bearer {}", token))
                            .send()
                            .await
                        {
                            Ok(response) => {
                                if response.ok() {
                                    match response.json::<UserProfile>().await {
                                        Ok(data) => {
                                            profile.set(Some(data));
                                        }
                                        Err(_) => {
                                            error.set(Some("Failed to parse profile data".to_string()));
                                        }
                                    }
                                }
                            }
                            Err(_) => {
                                error.set(Some("Failed to fetch profile".to_string()));
                            }
                        }
                    }
                });
                || ()
            }, ());
        }

        let on_edit = {
            let phone_number = phone_number.clone();
            let error = error.clone();
            let success = success.clone();
            let profile = profile.clone();
            let is_editing = is_editing.clone();

            Callback::from(move |_e: MouseEvent| {
                let phone = (*phone_number).clone();
                let error = error.clone();
                let success = success.clone();
                let profile = profile.clone();
                let is_editing = is_editing.clone();

                wasm_bindgen_futures::spawn_local(async move {
                    if let Some(token) = window()
                        .and_then(|w| w.local_storage().ok())
                        .flatten()
                        .and_then(|storage| storage.get_item("token").ok())
                        .flatten()
                    {
                        match Request::post("http://localhost:3000/api/profile/update")
                            .header("Authorization", &format!("Bearer {}", token))
                            .json(&UpdateProfileRequest { phone_number: phone })
                            .unwrap()
                            .send()
                            .await
                        {
                            Ok(response) => {
                                if response.ok() {
                                    success.set(Some("Profile updated successfully".to_string()));
                                    error.set(None);
                                    is_editing.set(false);
                                    
                                    // Clear success message after 3 seconds
                                    let success_clone = success.clone();
                                    wasm_bindgen_futures::spawn_local(async move {
                                        gloo_timers::future::TimeoutFuture::new(3_000).await;
                                        success_clone.set(None);
                                    });
                                    
                                    // Fetch updated profile data after successful update
                                    if let Ok(profile_response) = Request::get("http://localhost:3000/api/profile")
                                        .header("Authorization", &format!("Bearer {}", token))
                                        .send()
                                        .await
                                    {
                                        if let Ok(updated_profile) = profile_response.json::<UserProfile>().await {
                                            profile.set(Some(updated_profile));
                                        }
                                    }
                                } else {
                                    error.set(Some("Failed to update profile".to_string()));
                                }
                            }
                            Err(_) => {
                                error.set(Some("Failed to send request".to_string()));
                            }
                        }
                    }
                });
            })
        };
        html! {
            <div class="profile-container">
                <h1>{"Your Profile"}</h1>
                {
                    if let Some(error_msg) = (*error).as_ref() {
                        html! {
                            <div class="error-message" style="color: red;">
                                {error_msg}
                            </div>
                        }
                    } else if let Some(success_msg) = (*success).as_ref() {
                        html! {
                            <div class="success-message" style="color: green;">
                                {success_msg}
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }
                {
                    if let Some(user_profile) = (*profile).as_ref() {
                        html! {
                            <div class="profile-info">
                                <p><strong>{"Username: "}</strong>{&user_profile.username}</p>
                                <p><strong>{"Email: "}</strong>{&user_profile.email}</p>
                                <p>
                                    <strong>{"Phone: "}</strong>
                                    {
                                        if *is_editing {
                                            html! {
                                                <input
                                                    type="tel"
                                                    value={user_profile.phone_number.clone().unwrap_or_default()}
                                                    onchange={let phone_number = phone_number.clone(); move |e: Event| {
                                                        let input: HtmlInputElement = e.target_unchecked_into();
                                                        phone_number.set(input.value());
                                                    }}
                                                />
                                            }
                                        } else {
                                            html! {
                                                <span>{user_profile.phone_number.clone().unwrap_or_default()}</span>
                                            }
                                        }
                                    }
                                    <button onclick={
                                        let is_editing = is_editing.clone();
                                        if *is_editing {
                                            on_edit
                                        } else {
                                            Callback::from(move |_| is_editing.set(true))
                                        }
                                    }>
                                        {if *is_editing { "Confirm" } else { "Edit" }}
                                    </button>
                                </p>
                            </div>
                        }
                    } else {
                        html! {
                            <p>{"Loading profile..."}</p>
                        }
                    }
                }
            </div>
        }
    }
}

pub mod admin {
    use yew::prelude::*;
    use web_sys::window;
    use gloo_net::http::Request;
    use serde::Deserialize;

    #[derive(Deserialize, Clone, Debug)]
    struct UserInfo {
        id: i32,
        username: String,
        email: String,
    }

    #[function_component]
    pub fn Admin() -> Html {
        let users = use_state(|| Vec::new());
        let error = use_state(|| None::<String>);

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
                    match Request::get("http://localhost:3000/api/admin/users")
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

        html! {
            <div class="admin-container">
                <h1>{"Admin Dashboard"}</h1>
                {
                    if let Some(error_msg) = (*error).as_ref().clone() {
                        html! {
                            <div class="error-message">
                                {error_msg}
                            </div>
                        }
                    } else {
                        html! {
                            <div class="users-list">
                                <h2>{"Users List"}</h2>
                                <table>
                                    <thead>
                                        <tr>
                                            <th>{"ID"}</th>
                                            <th>{"Username"}</th>
                                            <th>{"Email"}</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {
                                            users.iter().map(|user| {
                                                html! {
                                                    <tr key={user.id}>
                                                        <td>{user.id}</td>
                                                        <td>{&user.username}</td>
                                                        <td>{&user.email}</td>
                                                    </tr>
                                                }
                                            }).collect::<Html>()
                                        }
                                    </tbody>
                                </table>
                            </div>
                        }
                    }
                }
            </div>
        }
    }
}
