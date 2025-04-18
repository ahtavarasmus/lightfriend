use yew::prelude::*;
use web_sys::window;
use crate::config;
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use yew_router::prelude::*;
use crate::Route;
use chrono::{Utc, TimeZone};
use crate::profile::billing_models::format_timestamp;

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
    sub_tier: Option<String>,
    msgs_left: i32,
}

#[derive(Clone, Debug)]
struct DeleteModalState {
    show: bool,
    user_id: Option<i32>,
    user_email: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct EmailJudgmentResponse {
    id: i32,
    email_timestamp: i32,
    processed_at: i32,
    should_notify: bool,
    score: i32,
    reason: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct UsageLog {
    id: i32,
    activity_type: String,
    timestamp: i32,
    sid: Option<String>,
    status: Option<String>,
    success: Option<bool>,
    credits: Option<f32>,
    time_consumed: Option<i32>,
    reason: Option<String>,
    recharge_threshold_timestamp: Option<i32>,
    zero_credits_timestamp: Option<i32>,
}



#[function_component]
pub fn AdminDashboard() -> Html {
    let users = use_state(|| Vec::new());
    let error = use_state(|| None::<String>);
    let usage_logs = use_state(|| Vec::<UsageLog>::new());
    let activity_filter = use_state(|| None::<String>);
    let selected_user_id = use_state(|| None::<i32>);
    let message = use_state(|| String::new());
    let delete_modal = use_state(|| DeleteModalState {
        show: false,
        user_id: None,
        user_email: None,
    });

    let users_effect = users.clone();
    let error_effect = error.clone();

    // Fetch usage logs
    {
        let usage_logs = usage_logs.clone();
        let error = error.clone();
        let activity_filter = activity_filter.clone();

        use_effect_with_deps(move |_| {
            let usage_logs = usage_logs.clone();
            let error = error.clone();
            
            wasm_bindgen_futures::spawn_local(async move {
                if let Some(token) = window()
                    .and_then(|w| w.local_storage().ok())
                    .flatten()
                    .and_then(|storage| storage.get_item("token").ok())
                    .flatten()
                {
                    // This endpoint doesn't exist yet - we'll implement it later
                    match Request::get(&format!("{}/api/admin/usage-logs", config::get_backend_url()))
                        .header("Authorization", &format!("Bearer {}", token))
                        .send()
                        .await
                    {
                        Ok(response) => {
                            if response.ok() {
                                match response.json::<Vec<UsageLog>>().await {
                                    Ok(logs) => {
                                        usage_logs.set(logs);
                                    }
                                    Err(_) => {
                                        error.set(Some("Failed to parse usage logs data".to_string()));
                                    }
                                }
                            } else {
                                error.set(Some("Failed to fetch usage logs".to_string()));
                            }
                        }
                        Err(_) => {
                            error.set(Some("Failed to fetch usage logs".to_string()));
                        }
                    }
                }
            });
            || ()
        }, [activity_filter]);
    }

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
                // Usage Logs Section
                <div class="filter-section">
                    <h3>{"Usage Logs"}</h3>
                    <div class="usage-filter">
                        <button 
                            class={classes!(
                                "filter-button",
                                (activity_filter.is_none()).then_some("active")
                            )}
                            onclick={
                                let activity_filter = activity_filter.clone();
                                Callback::from(move |_| activity_filter.set(None))
                            }
                        >
                            {"All"}
                        </button>
                        <button 
                            class={classes!(
                                "filter-button",
                                (activity_filter.as_deref() == Some("sms")).then_some("active")
                            )}
                            onclick={
                                let activity_filter = activity_filter.clone();
                                Callback::from(move |_| activity_filter.set(Some("sms".to_string())))
                            }
                        >
                            {"SMS"}
                        </button>
                        <button 
                            class={classes!(
                                "filter-button",
                                (activity_filter.as_deref() == Some("call")).then_some("active")
                            )}
                            onclick={
                                let activity_filter = activity_filter.clone();
                                Callback::from(move |_| activity_filter.set(Some("call".to_string())))
                            }
                        >
                            {"Calls"}
                        </button>
                        <button 
                            class={classes!(
                                "filter-button",
                                (activity_filter.as_deref() == Some("failed")).then_some("active")
                            )}
                            onclick={
                                let activity_filter = activity_filter.clone();
                                Callback::from(move |_| activity_filter.set(Some("failed".to_string())))
                            }
                        >
                            {"Failed"}
                        </button>
                    </div>

                    <div class="usage-logs">
                        {
                            (*usage_logs).iter()
                                .filter(|log| {
                                    if let Some(filter) = (*activity_filter).as_ref() {
                                        match filter.as_str() {
                                            "failed" => !log.success.unwrap_or(true),
                                            _ => log.activity_type == *filter
                                        }
                                    } else {
                                        true
                                    }
                                })
                                .map(|log| {
                                    html! {
                                        <div class={classes!("usage-log-item", log.activity_type.clone())}>
                                                <div class="usage-log-header">
                                                    <span class="usage-type">{&log.activity_type}</span>
                                                    <span class="usage-date">
                                                        {
                                                            // Format timestamp with date and time
                                                            if let Some(dt) = Utc.timestamp_opt(log.timestamp as i64, 0).single() {
                                                                dt.format("%Y-%m-%d %H:%M:%S UTC").to_string()
                                                            } else {
                                                                "Invalid timestamp".to_string()
                                                            }
                                                        }
                                                    </span>
                                                </div>
                                                <div class="usage-details">
                                                    {
                                                        if let Some(status) = &log.status {
                                                            html! {
                                                                <div>
                                                                    <span class="label">{"Status"}</span>
                                                                    <span class={classes!("value", if log.success.unwrap_or(false) { "success" } else { "failure" })}>
                                                                        {status}
                                                                    </span>
                                                                </div>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }
                                                    }
                                                    {
                                                        // Add success field display
                                                        if let Some(success) = log.success {
                                                            html! {
                                                                <div>
                                                                    <span class="label">{"Success"}</span>
                                                                    <span class={classes!("value", if success { "success" } else { "failure" })}>
                                                                        {if success { "Yes" } else { "No" }}
                                                                    </span>
                                                                </div>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }
                                                    }

                                                    {
                                                        if let Some(credits) = log.credits {
                                                            html! {
                                                                <div>
                                                                    <span class="label">{"Credits Used"}</span>
                                                                    <span class="value">{format!("{:.2}€", credits)}</span>
                                                                </div>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }

                                                    }
                                                    {
                                                        if let Some(time) = log.time_consumed {
                                                            html! {
                                                                <div>
                                                                    <span class="label">{"Duration"}</span>
                                                                    <span class="value">{format!("{}s", time)}</span>
                                                                </div>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }

                                                    }
                                                    {
                                                        if let Some(reason) = &log.reason {
                                                            html! {
                                                                <div class="usage-reason">
                                                                    <span class="label">{"Reason"}</span>
                                                                    <span class="value">{reason}</span>
                                                                </div>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }

                                                    }
                                                    {
                                                        if let Some(sid) = &log.sid {
                                                            html! {
                                                                <div class="usage-sid">
                                                                    <span class="label">{"SID"}</span>
                                                                    <span class="value">{sid}</span>
                                                                </div>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }

                                                    }
                                                    {
                                                        if let Some(threshold) = log.recharge_threshold_timestamp {
                                                            html! {
                                                                <div>
                                                                    <span class="label">{"Recharge Threshold"}</span>
                                                                    <span class="value">{format_timestamp(threshold)}</span>
                                                                </div>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }

                                                    }
                                                    {
                                                        if let Some(zero) = log.zero_credits_timestamp {
                                                            html! {
                                                                <div>
                                                                    <span class="label">{"Zero Credits At"}</span>
                                                                    <span class="value">{format_timestamp(zero)}</span>
                                                                </div>
                                                            }
                                                        } else {
                                                            html! {}
                                                        }

                                                    }
                                                </div>
                                        </div>
                                    }
                                })
                                .collect::<Html>()
                        }
                    </div>
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
                                                                            <p><strong>{"Subscription Tier: "}</strong>{user.sub_tier.clone().unwrap_or_else(|| "None".to_string())}</p>
                                                                            <p><strong>{"Messages Left: "}</strong>{user.msgs_left}</p>
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
                                                                            {"Add 1€ credits"}
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
                                                                        {
                                                                            if !user.verified {
                                                                                html! {
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
                                                                                    >
                                                                                        {"Verify User"}
                                                                                    </button>
                                                                                }
                                                                            } else {
                                                                                html! {}
                                                                            }
                                                                        }
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
                                                                                let users = users.clone();
                                                                                let error = error.clone();
                                                                                let user_id = user.id;
                                                                                let current_tier = user.sub_tier.clone();
                                                                                Callback::from(move |_| {
                                                                                    let users = users.clone();
                                                                                    let error = error.clone();
                                                                                    let new_tier = if current_tier.as_deref() == Some("tier 1") {
                                                                                        "tier 0"
                                                                                    } else {
                                                                                        "tier 1"
                                                                                    };
                                                                                    
                                                                                    wasm_bindgen_futures::spawn_local(async move {
                                                                                        if let Some(token) = window()
                                                                                            .and_then(|w| w.local_storage().ok())
                                                                                            .flatten()
                                                                                            .and_then(|storage| storage.get_item("token").ok())
                                                                                            .flatten()
                                                                                        {
match Request::post(&format!("{}/api/admin/subscription/{}/{}", config::get_backend_url(), user_id, urlencoding::encode(new_tier).trim_end_matches('/')))
                                                                                                .header("Authorization", &format!("Bearer {}", token))
                                                                                                .send()
                                                                                                .await
                                                                                            {
                                                                                                Ok(response) => {
                                                                                                    if response.ok() {
                                                                                                        // Refresh the users list
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
                                                                                                        error.set(Some("Failed to update subscription tier".to_string()));
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
                                                                            {if user.sub_tier.as_deref() == Some("tier 1") {
                                                                                "Remove Tier 1"
                                                                            } else {
                                                                                "Set Tier 1"
                                                                            }}
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
                                                                                            match Request::post(&format!("{}/api/admin/messages/{}/10", config::get_backend_url(), user_id))
                                                                                                .header("Authorization", &format!("Bearer {}", token))
                                                                                                .send()
                                                                                                .await
                                                                                            {
                                                                                                Ok(response) => {
                                                                                                    if response.ok() {
                                                                                                        // Refresh the users list
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
                                                                                                        error.set(Some("Failed to add messages".to_string()));
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
                                                                            {"Add 10 Messages"}
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
                                                                                            match Request::post(&format!("{}/api/admin/messages/{}/{}", config::get_backend_url(), user_id, -10))
                                                                                                .header("Authorization", &format!("Bearer {}", token))
                                                                                                .send()
                                                                                                .await
                                                                                            {
                                                                                                Ok(response) => {
                                                                                                    if response.ok() {
                                                                                                        // Refresh the users list
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
                                                                                                        error.set(Some("Failed to remove messages".to_string()));
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
                                                                            {"Remove 10 Messages"}
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
            <style>
                {r#"
                .judgment-processed {
                    font-size: 0.8rem;
                    color: #666;
                }

                /* Usage Logs Styles */
                .usage-filter {
                    display: flex;
                    gap: 1rem;
                    margin-bottom: 1.5rem;
                }

                .filter-button {
                    padding: 0.5rem 1.5rem;
                    background: rgba(30, 144, 255, 0.1);
                    border: 1px solid rgba(30, 144, 255, 0.2);
                    border-radius: 20px;
                    color: #fff;
                    cursor: pointer;
                    transition: all 0.3s ease;
                }

                .filter-button:hover {
                    background: rgba(30, 144, 255, 0.2);
                }

                .filter-button.active {
                    background: #1E90FF;
                    border-color: #1E90FF;
                }

                .usage-logs {
                    display: flex;
                    flex-direction: column;
                    gap: 1rem;
                    max-height: 500px;
                    overflow-y: auto;
                    padding-right: 0.5rem;
                }

                .usage-log-item {
                    background: rgba(30, 30, 30, 0.7);
                    border: 1px solid rgba(30, 144, 255, 0.1);
                    border-radius: 8px;
                    padding: 1rem;
                    transition: all 0.3s ease;
                }

                .usage-log-item:hover {
                    transform: translateY(-2px);
                    box-shadow: 0 4px 20px rgba(30, 144, 255, 0.1);
                }

                .usage-log-item.sms {
                    border-left: 4px solid #4CAF50;
                }

                .usage-log-item.call {
                    border-left: 4px solid #FF9800;
                }

                .usage-log-header {
                    display: flex;
                    justify-content: space-between;
                    align-items: center;
                    margin-bottom: 1rem;
                }

                .usage-type {
                    font-size: 0.9rem;
                    padding: 0.3rem 0.8rem;
                    border-radius: 12px;
                    font-weight: 500;
                    text-transform: uppercase;
                }

                .usage-log-item.sms .usage-type {
                    background: rgba(76, 175, 80, 0.1);
                    color: #4CAF50;
                }

                .usage-log-item.call .usage-type {
                    background: rgba(255, 152, 0, 0.1);
                    color: #FF9800;
                }

                .usage-date {
                    color: #7EB2FF;
                    font-size: 0.9rem;
                }

                .usage-details {
                    display: grid;
                    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
                    gap: 0.8rem;
                }

                .usage-details > div {
                    display: flex;
                    flex-direction: column;
                    gap: 0.3rem;
                }

                .usage-details .label {
                    color: #999;
                    font-size: 0.8rem;
                }

                .usage-details .value {
                    color: #fff;
                    font-size: 0.9rem;
                }

                .usage-details .value.success {
                    color: #4CAF50;
                }

                .usage-details .value.failure {
                    color: #ff4757;
                }

                .usage-reason {
                    grid-column: 1 / -1;
                }

                .usage-reason .value {
                    font-style: italic;
                }

                .usage-sid {
                    grid-column: 1 / -1;
                }

                .usage-sid .value {
                    font-family: monospace;
                    font-size: 0.8rem;
                    color: #7EB2FF;
                }

                @media (max-width: 768px) {
                    .usage-filter {
                        flex-wrap: wrap;
                    }

                    .filter-button {
                        flex: 1;
                        text-align: center;
                    }

                    .usage-details {
                        grid-template-columns: 1fr;
                    }
                }
                    .iq-button {
                        background: linear-gradient(45deg, #FFD700, #FFA500);
                        color: #000;
                        border: none;
                        padding: 0.75rem 1.5rem;
                        border-radius: 8px;
                        font-size: 0.9rem;
                        font-weight: 600;
                        cursor: pointer;
                        transition: all 0.3s ease;
                        text-transform: uppercase;
                        letter-spacing: 0.5px;
                        display: inline-flex;
                        align-items: center;
                        justify-content: center;
                        gap: 0.5rem;
                        margin-left: 1rem;
                        position: relative;
                        overflow: hidden;
                        box-shadow: 0 2px 10px rgba(255, 215, 0, 0.2);
                    }

                    .iq-button:hover {
                        transform: translateY(-2px);
                        box-shadow: 0 4px 15px rgba(255, 215, 0, 0.4);
                        background: linear-gradient(45deg, #FFE44D, #FFB347);
                    }

                    .iq-button:active {
                        transform: translateY(0);
                    }

                    .iq-button::before {
                        content: '';
                        position: absolute;
                        top: 0;
                        left: 0;
                        width: 100%;
                        height: 100%;
                        background: linear-gradient(45deg, transparent, rgba(255, 255, 255, 0.2), transparent);
                        transform: translateX(-100%);
                        transition: transform 0.6s;
                    }

                    .iq-button:hover::before {
                        transform: translateX(100%);
                    }

                    .iq-button.reset {
                        background: linear-gradient(45deg, #FF6B6B, #FF4757);
                        color: white;
                        box-shadow: 0 2px 10px rgba(255, 107, 107, 0.2);
                    }

                    .iq-button.reset:hover {
                        background: linear-gradient(45deg, #FF8787, #FF6B6B);
                        box-shadow: 0 4px 15px rgba(255, 107, 107, 0.4);
                    }
                    .iq-button.delete {
                        background: linear-gradient(45deg, #FF6B6B, #FF4757);
                        color: white;
                    }

                    .iq-button.delete:hover {
                        background: linear-gradient(45deg, #FF8787, #FF6B6B);
                        box-shadow: 0 4px 15px rgba(255, 107, 107, 0.4);
                    }

                    .modal-overlay {
                        position: fixed;
                        top: 0;
                        left: 0;
                        right: 0;
                        bottom: 0;
                        background-color: rgba(0, 0, 0, 0.5);
                        display: flex;
                        justify-content: center;
                        align-items: center;
                        z-index: 1000;
                    }

                    .modal-content {
                        background-color: white;
                        padding: 2rem;
                        border-radius: 8px;
                        max-width: 500px;
                        width: 90%;
                    }

                    .modal-content h2 {
                        margin-top: 0;
                        color: #333;
                    }

                    .modal-content p {
                        margin: 1rem 0;
                    }

                    .modal-content p.warning {
                        color: #dc3545;
                        font-weight: bold;
                    }

                    .modal-buttons {
                        display: flex;
                        justify-content: flex-end;
                        gap: 1rem;
                        margin-top: 2rem;
                    }

                    .modal-button {
                        padding: 0.5rem 1rem;
                        border: none;
                        border-radius: 4px;
                        cursor: pointer;
                        font-weight: bold;
                    }

                    .modal-button.cancel {
                        background-color: #6c757d;
                        color: white;
                    }

                    .modal-button.delete {
                        background-color: #dc3545;
                        color: white;
                    }

                    .modal-button.cancel:hover {
                        background-color: #5a6268;
                    }

                    .modal-button.delete:hover {
                        background-color: #c82333;
                    }

                "#}
            </style>
        </div>
    }
}
