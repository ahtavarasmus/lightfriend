use yew::prelude::*;
use crate::utils::api::Api;
use serde::{Deserialize, Serialize};
use yew_router::prelude::*;
use crate::Route;
use chrono::{Utc, TimeZone};
use std::collections::HashMap;

#[derive(Serialize)]
struct EmailBroadcastMessage {
    subject: String,
    message: String,
}

#[derive(Deserialize, Clone, Debug)]
#[allow(dead_code)]
struct MessageStatusLog {
    id: Option<i32>,
    message_sid: String,
    user_id: i32,
    direction: String,
    to_number: String,
    from_number: Option<String>,
    status: String,
    error_code: Option<String>,
    error_message: Option<String>,
    price: Option<f32>,
    price_unit: Option<String>,
    created_at: i32,
    updated_at: i32,
}

#[derive(Deserialize, Clone, Debug)]
#[allow(dead_code)]
struct MessageStatsResponse {
    user_id: i32,
    total_messages: i64,
    delivered: i64,
    failed: i64,
    undelivered: i64,
    queued: i64,
    sent: i64,
    recent_messages: Vec<MessageStatusLog>,
}

#[derive(Deserialize, Clone, Debug)]
#[allow(dead_code)]
struct MessageStatusLogWithUser {
    id: Option<i32>,
    message_sid: String,
    user_id: i32,
    user_email: Option<String>,
    user_phone: Option<String>,
    direction: String,
    to_number: String,
    from_number: Option<String>,
    status: String,
    error_code: Option<String>,
    error_message: Option<String>,
    price: Option<f32>,
    price_unit: Option<String>,
    created_at: i32,
    updated_at: i32,
}

#[derive(Deserialize, Clone, Debug)]
#[allow(dead_code)]
struct GlobalMessageStatsResponse {
    total_messages: i64,
    delivered: i64,
    failed: i64,
    undelivered: i64,
    queued: i64,
    sent: i64,
    recent_failed: Vec<MessageStatusLogWithUser>,
}

// Cost Stats Response
#[derive(Deserialize, Clone, Debug)]
#[allow(dead_code)]
struct CostStatsResponse {
    // Key metrics
    avg_cost_per_intl_user_30d: f32,
    avg_cost_per_us_ca_user_30d: f32,
    avg_cost_per_intl_user_7d_projected: f32,
    avg_cost_per_us_ca_user_7d_projected: f32,
    // Counts
    intl_user_count: i64,
    us_ca_user_count: i64,
    // Totals
    total_cost: f32,
    total_sms_cost: f32,
    total_voice_cost: f32,
    international_sms_cost: f32,
    us_ca_sms_cost: f32,
    // Per-user
    costs_per_user: Vec<UserCostEntry>,
}

#[derive(Deserialize, Clone, Debug)]
struct UserCostEntry {
    user_id: i32,
    country_code: String,
    sms_cost: f32,
    sms_count: i64,
    is_international: bool,
}

// Usage Stats Response
#[derive(Deserialize, Clone, Debug)]
#[allow(dead_code)]
struct UsageStatsResponse {
    daily_stats: Vec<DailyUsageStat>,
    total_messages_7d: i64,
    total_messages_30d: i64,
    growth_rate_7d: f32,
    growth_rate_30d: f32,
    active_users_7d: i64,
    active_users_30d: i64,
    breakdown_by_type: Vec<ActivityTypeBreakdown>,
}

#[derive(Deserialize, Clone, Debug)]
struct DailyUsageStat {
    date: String,
    sms_count: i64,
    call_count: i64,
    total_cost: f32,
}

#[derive(Deserialize, Clone, Debug)]
struct ActivityTypeBreakdown {
    activity_type: String,
    count: i64,
    total_credits: f32,
}

// Admin Alert types
#[derive(Deserialize, Clone, Debug)]
struct AdminAlert {
    id: Option<i32>,
    alert_type: String,
    severity: String,
    message: String,
    location: String,
    #[allow(dead_code)]
    module: String,
    acknowledged: i32,
    created_at: i32,
}

#[derive(Deserialize, Clone, Debug)]
struct AlertsResponse {
    alerts: Vec<AdminAlert>,
    #[allow(dead_code)]
    total: i64,
    unacknowledged_count: i64,
}

#[derive(Deserialize, Clone, Debug)]
struct DisabledAlertType {
    #[allow(dead_code)]
    id: Option<i32>,
    alert_type: String,
    #[allow(dead_code)]
    disabled_at: i32,
}

#[derive(Deserialize, Clone, Debug)]
struct DisabledTypesResponse {
    disabled_types: Vec<DisabledAlertType>,
}

#[derive(Serialize)]
struct ChangePasswordRequest {
    new_password: String,
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
    credits_left: f32,
    discount_tier: Option<String>,
    plan_type: Option<String>,
    has_twilio_credentials: bool,
}

#[derive(Clone, Debug)]
struct DeleteModalState {
    show: bool,
    user_id: Option<i32>,
    user_email: Option<String>,
}

fn render_message_stats(
    user_id: i32,
    stats: Option<&MessageStatsResponse>,
    is_loading: bool,
    show_all: bool,
    message_stats: UseStateHandle<HashMap<i32, MessageStatsResponse>>,
    loading_stats: UseStateHandle<Option<i32>>,
    show_all_messages: UseStateHandle<bool>,
) -> Html {
    if stats.is_none() && !is_loading {
        let message_stats = message_stats.clone();
        let loading_stats = loading_stats.clone();
        html! {
            <div class="message-stats-section">
                <h3>{"SMS Delivery Stats"}</h3>
                <button
                    onclick={Callback::from(move |_| {
                        let message_stats = message_stats.clone();
                        let loading_stats = loading_stats.clone();
                        loading_stats.set(Some(user_id));
                        wasm_bindgen_futures::spawn_local(async move {
                            match Api::get(&format!("/api/admin/users/{}/message-stats", user_id))
                                .send()
                                .await
                            {
                                Ok(response) => {
                                    if response.ok() {
                                        if let Ok(data) = response.json::<MessageStatsResponse>().await {
                                            let mut new_stats = (*message_stats).clone();
                                            new_stats.insert(user_id, data);
                                            message_stats.set(new_stats);
                                        }
                                    }
                                }
                                Err(_) => {}
                            }
                            loading_stats.set(None);
                        });
                    })}
                    class="iq-button stats-button"
                >
                    {"Load Message Stats"}
                </button>
            </div>
        }
    } else if is_loading {
        html! {
            <div class="message-stats-section">
                <h3>{"SMS Delivery Stats"}</h3>
                <p class="loading">{"Loading stats..."}</p>
            </div>
        }
    } else if let Some(stats) = stats {
        let filtered_messages: Vec<_> = if show_all {
            stats.recent_messages.clone()
        } else {
            stats.recent_messages.iter()
                .filter(|m| m.status == "failed" || m.status == "undelivered")
                .cloned()
                .collect()
        };
        html! {
            <div class="message-stats-section">
                <h3>{"SMS Delivery Stats"}</h3>
                <div class="stats-summary">
                    <div class="stat-card total">
                        <span class="stat-number">{stats.total_messages}</span>
                        <span class="stat-label">{"Total"}</span>
                    </div>
                    <div class="stat-card delivered">
                        <span class="stat-number">{stats.delivered}</span>
                        <span class="stat-label">{"Delivered"}</span>
                    </div>
                    <div class="stat-card failed">
                        <span class="stat-number">{stats.failed}</span>
                        <span class="stat-label">{"Failed"}</span>
                    </div>
                    <div class="stat-card undelivered">
                        <span class="stat-number">{stats.undelivered}</span>
                        <span class="stat-label">{"Undelivered"}</span>
                    </div>
                </div>
                <div class="filter-toggle">
                    <label>
                        <input
                            type="checkbox"
                            checked={show_all}
                            onchange={Callback::from(move |_| {
                                show_all_messages.set(!show_all);
                            })}
                        />
                        {" Show all messages (not just failed)"}
                    </label>
                </div>
                {
                    if filtered_messages.is_empty() {
                        html! { <p class="no-messages">{"No failed/undelivered messages found"}</p> }
                    } else {
                        html! {
                            <table class="message-log-table">
                                <thead>
                                    <tr>
                                        <th>{"Status"}</th>
                                        <th>{"To"}</th>
                                        <th>{"From"}</th>
                                        <th>{"Price"}</th>
                                        <th>{"Error"}</th>
                                        <th>{"Time"}</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {
                                        filtered_messages.iter().map(|msg| {
                                            let status_class = match msg.status.as_str() {
                                                "delivered" => "status-delivered",
                                                "failed" => "status-failed",
                                                "undelivered" => "status-undelivered",
                                                "sent" => "status-sent",
                                                _ => "status-queued",
                                            };
                                            let from_str = msg.from_number.clone().unwrap_or_else(|| "-".to_string());
                                            let price_str = msg.price
                                                .map(|p| format!("{:.4} {}", p.abs(), msg.price_unit.as_deref().unwrap_or("USD")))
                                                .unwrap_or_else(|| "-".to_string());
                                            let error_str = msg.error_code.clone()
                                                .map(|c| format!("{}: {}", c, msg.error_message.as_deref().unwrap_or("")))
                                                .unwrap_or_else(|| "-".to_string());
                                            let time_str = Utc.timestamp_opt(msg.created_at as i64, 0)
                                                .single()
                                                .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                                                .unwrap_or_else(|| "Invalid".to_string());
                                            html! {
                                                <tr key={msg.message_sid.clone()}>
                                                    <td><span class={classes!("status-badge", status_class)}>{&msg.status}</span></td>
                                                    <td>{&msg.to_number}</td>
                                                    <td>{from_str}</td>
                                                    <td>{price_str}</td>
                                                    <td class="error-cell">{error_str}</td>
                                                    <td>{time_str}</td>
                                                </tr>
                                            }
                                        }).collect::<Html>()
                                    }
                                </tbody>
                            </table>
                        }
                    }
                }
            </div>
        }
    } else {
        html! {}
    }
}

#[function_component(AdminDashboard)]
pub fn admin_dashboard() -> Html {
    let users = use_state(|| Vec::new());
    let error = use_state(|| None::<String>);
    let selected_user_id = use_state(|| None::<i32>);
    let email_subject = use_state(|| String::new());
    let email_message = use_state(|| String::new());
    let delete_modal = use_state(|| DeleteModalState {
        show: false,
        user_id: None,
        user_email: None,
    });
    let reset_link_status = use_state(|| None::<(i32, String)>); // (user_id, message)
    let new_password = use_state(|| String::new());
    let password_status = use_state(|| None::<String>);
    // Message stats state
    let message_stats: UseStateHandle<HashMap<i32, MessageStatsResponse>> = use_state(|| HashMap::new());
    let loading_stats = use_state(|| None::<i32>);
    let show_all_messages = use_state(|| false); // Default: show only failed/undelivered
    // Global message stats state
    let global_stats: UseStateHandle<Option<GlobalMessageStatsResponse>> = use_state(|| None);
    // Cost and usage stats state
    let cost_stats: UseStateHandle<Option<CostStatsResponse>> = use_state(|| None);
    let usage_stats: UseStateHandle<Option<UsageStatsResponse>> = use_state(|| None);
    let stats_days = use_state(|| 30i32);
    // Admin alerts state
    let admin_alerts: UseStateHandle<Vec<AdminAlert>> = use_state(|| Vec::new());
    let disabled_alert_types: UseStateHandle<Vec<DisabledAlertType>> = use_state(|| Vec::new());
    let alert_count = use_state(|| 0i64);
    let show_alerts_section = use_state(|| false);
    // Collapsible section states
    let show_stats_section = use_state(|| false);
    let show_broadcast_section = use_state(|| false);
    let show_password_section = use_state(|| false);

    let users_effect = users.clone();
    let error_effect = error.clone();
    let global_stats_effect = global_stats.clone();
    let cost_stats_effect = cost_stats.clone();
    let usage_stats_effect = usage_stats.clone();
    let stats_days_effect = (*stats_days).clone();
    let admin_alerts_effect = admin_alerts.clone();
    let disabled_types_effect = disabled_alert_types.clone();
    let alert_count_effect = alert_count.clone();

    use_effect_with_deps(move |_| {
        let users = users_effect;
        let error = error_effect;
        let global_stats = global_stats_effect;
        let cost_stats = cost_stats_effect;
        let usage_stats = usage_stats_effect;
        let stats_days = stats_days_effect;
        let admin_alerts = admin_alerts_effect;
        let disabled_types = disabled_types_effect;
        let alert_count = alert_count_effect;

        wasm_bindgen_futures::spawn_local(async move {
            // Fetch users
            match Api::get("/api/admin/users")
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

            // Fetch global message stats
            if let Ok(response) = Api::get("/api/admin/global-message-stats")
                .send()
                .await
            {
                if response.ok() {
                    if let Ok(data) = response.json::<GlobalMessageStatsResponse>().await {
                        global_stats.set(Some(data));
                    }
                }
            }

            // Fetch cost stats
            if let Ok(response) = Api::get(&format!("/api/admin/stats/costs?days={}", stats_days))
                .send()
                .await
            {
                if response.ok() {
                    if let Ok(data) = response.json::<CostStatsResponse>().await {
                        cost_stats.set(Some(data));
                    }
                }
            }

            // Fetch usage stats
            if let Ok(response) = Api::get(&format!("/api/admin/stats/usage?days={}", stats_days))
                .send()
                .await
            {
                if response.ok() {
                    if let Ok(data) = response.json::<UsageStatsResponse>().await {
                        usage_stats.set(Some(data));
                    }
                }
            }

            // Fetch admin alerts
            if let Ok(response) = Api::get("/api/admin/alerts?limit=50")
                .send()
                .await
            {
                if response.ok() {
                    if let Ok(data) = response.json::<AlertsResponse>().await {
                        admin_alerts.set(data.alerts);
                        alert_count.set(data.unacknowledged_count);
                    }
                }
            }

            // Fetch disabled alert types
            if let Ok(response) = Api::get("/api/admin/alerts/disabled-types")
                .send()
                .await
            {
                if response.ok() {
                    if let Ok(data) = response.json::<DisabledTypesResponse>().await {
                        disabled_types.set(data.disabled_types);
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

                <div class="collapsible-section broadcast-section">
                    <div class="collapsible-header" onclick={{
                        let show_broadcast_section = show_broadcast_section.clone();
                        Callback::from(move |_| {
                            show_broadcast_section.set(!*show_broadcast_section);
                        })
                    }}>
                        <h2>{"Email Broadcast"}</h2>
                        <span class="toggle-indicator">{if *show_broadcast_section { "▼" } else { "▶" }}</span>
                    </div>
                    {
                        if *show_broadcast_section {
                            html! {
                                <div class="collapsible-content">
                                    <input
                                        type="text"
                                        value={(*email_subject).clone()}
                                        onchange={{
                                            let email_subject = email_subject.clone();
                                            Callback::from(move |e: Event| {
                                                let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                                                email_subject.set(input.value());
                                            })
                                        }}
                                        placeholder="Enter email subject..."
                                        class="email-subject-input"
                                    />
                                    <textarea
                                        value={(*email_message).clone()}
                                        onchange={{
                                            let email_message = email_message.clone();
                                            Callback::from(move |e: Event| {
                                                let input: web_sys::HtmlTextAreaElement = e.target_unchecked_into();
                                                email_message.set(input.value());
                                            })
                                        }}
                                        placeholder="Enter email message to broadcast..."
                                        class="broadcast-textarea"
                                    />
                                    <button
                                        onclick={{
                                            let email_subject = email_subject.clone();
                                            let email_message = email_message.clone();
                                            let error = error.clone();
                                            Callback::from(move |_| {
                                                let email_subject = email_subject.clone();
                                                let email_message = email_message.clone();
                                                let error = error.clone();

                                                if email_subject.is_empty() || email_message.is_empty() {
                                                    error.set(Some("Subject and message cannot be empty".to_string()));
                                                    return;
                                                }

                                                wasm_bindgen_futures::spawn_local(async move {
                                                    let broadcast_message = EmailBroadcastMessage {
                                                        subject: (*email_subject).clone(),
                                                        message: (*email_message).clone(),
                                                    };

                                                    match Api::post("/api/admin/broadcast-email")
                                                        .json(&broadcast_message)
                                                        .unwrap()
                                                        .send()
                                                        .await
                                                    {
                                                        Ok(response) => {
                                                            if response.ok() {
                                                                email_subject.set(String::new());
                                                                email_message.set(String::new());
                                                                error.set(Some("Email broadcast sent successfully".to_string()));
                                                            } else {
                                                                error.set(Some("Failed to send email broadcast".to_string()));
                                                            }
                                                        }
                                                        Err(_) => {
                                                            error.set(Some("Failed to send email broadcast request".to_string()));
                                                        }
                                                    }
                                                });
                                            })
                                        }}
                                        class="broadcast-button email"
                                    >
                                        {"Send Email Broadcast"}
                                    </button>
                                </div>
                            }
                        } else {
                            html! {}
                        }
                    }
                </div>

                // Change Password Section
                <div class="collapsible-section password-section">
                    <div class="collapsible-header" onclick={{
                        let show_password_section = show_password_section.clone();
                        Callback::from(move |_| {
                            show_password_section.set(!*show_password_section);
                        })
                    }}>
                        <h2>{"Change Admin Password"}</h2>
                        <span class="toggle-indicator">{if *show_password_section { "▼" } else { "▶" }}</span>
                    </div>
                    {
                        if *show_password_section {
                            html! {
                                <div class="collapsible-content">
                                    <div class="password-form">
                                        <input
                                            type="password"
                                            value={(*new_password).clone()}
                                            onchange={{
                                                let new_password = new_password.clone();
                                                Callback::from(move |e: Event| {
                                                    let input: web_sys::HtmlInputElement = e.target_unchecked_into();
                                                    new_password.set(input.value());
                                                })
                                            }}
                                            placeholder="Enter new password (min 6 characters)..."
                                            class="password-input"
                                        />
                                        <button
                                            onclick={{
                                                let new_password = new_password.clone();
                                                let password_status = password_status.clone();
                                                Callback::from(move |_| {
                                                    let password_value = (*new_password).clone();
                                                    let new_password = new_password.clone();
                                                    let password_status = password_status.clone();

                                                    if password_value.len() < 6 {
                                                        password_status.set(Some("Password must be at least 6 characters".to_string()));
                                                        return;
                                                    }

                                                    wasm_bindgen_futures::spawn_local(async move {
                                                        let request = ChangePasswordRequest {
                                                            new_password: password_value,
                                                        };

                                                        match Api::post("/api/admin/change-password")
                                                            .json(&request)
                                                            .unwrap()
                                                            .send()
                                                            .await
                                                        {
                                                            Ok(response) => {
                                                                if response.ok() {
                                                                    new_password.set(String::new());
                                                                    password_status.set(Some("Password updated successfully!".to_string()));
                                                                    let password_status = password_status.clone();
                                                                    gloo_timers::callback::Timeout::new(3000, move || {
                                                                        password_status.set(None);
                                                                    }).forget();
                                                                } else {
                                                                    password_status.set(Some("Failed to update password".to_string()));
                                                                }
                                                            }
                                                            Err(_) => {
                                                                password_status.set(Some("Failed to send request".to_string()));
                                                            }
                                                        }
                                                    });
                                                })
                                            }}
                                            class="broadcast-button"
                                        >
                                            {"Change Password"}
                                        </button>
                                        {
                                            if let Some(status) = (*password_status).as_ref() {
                                                html! {
                                                    <span class={classes!(
                                                        "password-status",
                                                        if status.contains("success") { "success" } else { "error" }
                                                    )}>
                                                        {status}
                                                    </span>
                                                }
                                            } else {
                                                html! {}
                                            }
                                        }
                                    </div>
                                </div>
                            }
                        } else {
                            html! {}
                        }
                    }
                </div>

                // Cost & Usage Statistics Section
                <div class="collapsible-section cost-usage-stats-section">
                    <div class="collapsible-header" onclick={{
                        let show_stats_section = show_stats_section.clone();
                        Callback::from(move |_| {
                            show_stats_section.set(!*show_stats_section);
                        })
                    }}>
                        <h2>
                            {"Cost & Usage Stats "}
                            {
                                if let Some(costs) = (*cost_stats).as_ref() {
                                    html! {
                                        <span class="header-stat">
                                            {format!("(Avg: ${:.4}/user Intl, ${:.4}/user US)",
                                                costs.avg_cost_per_intl_user_30d,
                                                costs.avg_cost_per_us_ca_user_30d
                                            )}
                                        </span>
                                    }
                                } else {
                                    html! {}
                                }
                            }
                        </h2>
                        <span class="toggle-indicator">{if *show_stats_section { "▼" } else { "▶" }}</span>
                    </div>
                    {
                        if *show_stats_section {
                            html! {
                                <div class="collapsible-content">
                    <div class="stats-period-selector">
                        <span>{"Period: "}</span>
                        <select
                            value={format!("{}", *stats_days)}
                            onchange={{
                                let stats_days = stats_days.clone();
                                let cost_stats = cost_stats.clone();
                                let usage_stats = usage_stats.clone();
                                Callback::from(move |e: Event| {
                                    let select: web_sys::HtmlSelectElement = e.target_unchecked_into();
                                    let days: i32 = select.value().parse().unwrap_or(30);
                                    stats_days.set(days);
                                    // Refetch stats with new period
                                    let cost_stats = cost_stats.clone();
                                    let usage_stats = usage_stats.clone();
                                    wasm_bindgen_futures::spawn_local(async move {
                                        if let Ok(response) = Api::get(&format!("/api/admin/stats/costs?days={}", days))
                                            .send()
                                            .await
                                        {
                                            if response.ok() {
                                                if let Ok(data) = response.json::<CostStatsResponse>().await {
                                                    cost_stats.set(Some(data));
                                                }
                                            }
                                        }
                                        if let Ok(response) = Api::get(&format!("/api/admin/stats/usage?days={}", days))
                                            .send()
                                            .await
                                        {
                                            if response.ok() {
                                                if let Ok(data) = response.json::<UsageStatsResponse>().await {
                                                    usage_stats.set(Some(data));
                                                }
                                            }
                                        }
                                    });
                                })
                            }}
                        >
                            <option value="7">{"7 days"}</option>
                            <option value="30" selected=true>{"30 days"}</option>
                            <option value="90">{"90 days"}</option>
                        </select>
                    </div>

                    // Cost Stats
                    {
                        if let Some(costs) = (*cost_stats).as_ref() {
                            html! {
                                <div class="cost-stats">
                                    <h3>{"Avg Cost Per User (30 days)"}</h3>
                                    <div class="key-metrics-row">
                                        <div class="key-metric intl">
                                            <span class="key-label">{"International"}</span>
                                            <span class="key-number">{format!("${:.4}", costs.avg_cost_per_intl_user_30d)}</span>
                                            <span class="key-context">{format!("{} users", costs.intl_user_count)}</span>
                                        </div>
                                        <div class="key-metric us-ca">
                                            <span class="key-label">{"US/CA"}</span>
                                            <span class="key-number">{format!("${:.4}", costs.avg_cost_per_us_ca_user_30d)}</span>
                                            <span class="key-context">{format!("{} users", costs.us_ca_user_count)}</span>
                                        </div>
                                    </div>

                                    {
                                        if !costs.costs_per_user.is_empty() {
                                            let max_cost = costs.costs_per_user.iter()
                                                .map(|u| u.sms_cost)
                                                .fold(0.0f32, |a, b| a.max(b));

                                            html! {
                                                <>
                                                    <h4>{"Cost Per User"}</h4>
                                                    <div class="user-cost-chart">
                                                        {
                                                            costs.costs_per_user.iter().map(|u| {
                                                                let bar_width = if max_cost > 0.0 {
                                                                    (u.sms_cost / max_cost * 100.0) as i32
                                                                } else {
                                                                    0
                                                                };
                                                                let bar_class = if u.is_international { "bar intl" } else { "bar us-ca" };
                                                                html! {
                                                                    <div class="chart-row" key={u.user_id}>
                                                                        <span class="chart-label">{format!("#{} ({})", u.user_id, u.country_code)}</span>
                                                                        <div class="chart-bar-container">
                                                                            <div class={bar_class} style={format!("width: {}%", bar_width)}></div>
                                                                        </div>
                                                                        <span class="chart-value">{format!("${:.4} ({} msgs)", u.sms_cost, u.sms_count)}</span>
                                                                    </div>
                                                                }
                                                            }).collect::<Html>()
                                                        }
                                                    </div>
                                                </>
                                            }
                                        } else {
                                            html! { <p class="no-data">{"No user cost data"}</p> }
                                        }
                                    }

                                    // Collapsible details section
                                    <details class="cost-details">
                                        <summary>{"Details"}</summary>
                                        <div class="details-content">
                                            <div class="detail-row">
                                                <span>{"7d projected (Intl):"}</span>
                                                <span>{format!("${:.4}", costs.avg_cost_per_intl_user_7d_projected)}</span>
                                            </div>
                                            <div class="detail-row">
                                                <span>{"7d projected (US/CA):"}</span>
                                                <span>{format!("${:.4}", costs.avg_cost_per_us_ca_user_7d_projected)}</span>
                                            </div>
                                            <div class="detail-row">
                                                <span>{"Total Intl SMS cost:"}</span>
                                                <span>{format!("${:.4}", costs.international_sms_cost)}</span>
                                            </div>
                                            <div class="detail-row">
                                                <span>{"Total US/CA SMS cost:"}</span>
                                                <span>{format!("${:.4}", costs.us_ca_sms_cost)}</span>
                                            </div>
                                            <div class="detail-row">
                                                <span>{"Total cost (SMS+Voice):"}</span>
                                                <span>{format!("${:.4}", costs.total_cost)}</span>
                                            </div>
                                        </div>
                                    </details>
                                </div>
                            }
                        } else {
                            html! { <p class="loading">{"Loading cost stats..."}</p> }
                        }
                    }

                    // Usage Stats
                    {
                        if let Some(usage) = (*usage_stats).as_ref() {
                            html! {
                                <div class="usage-stats">
                                    <h3>{"Usage Trends"}</h3>
                                    <div class="stats-summary usage-summary">
                                        <div class="stat-card growth">
                                            <span class="stat-number">{usage.total_messages_7d}</span>
                                            <span class="stat-label">{format!("Messages (7d) {}%", if usage.growth_rate_7d >= 0.0 { format!("+{:.1}", usage.growth_rate_7d) } else { format!("{:.1}", usage.growth_rate_7d) })}</span>
                                        </div>
                                        <div class="stat-card growth">
                                            <span class="stat-number">{usage.total_messages_30d}</span>
                                            <span class="stat-label">{format!("Messages (30d) {}%", if usage.growth_rate_30d >= 0.0 { format!("+{:.1}", usage.growth_rate_30d) } else { format!("{:.1}", usage.growth_rate_30d) })}</span>
                                        </div>
                                        <div class="stat-card active-users">
                                            <span class="stat-number">{usage.active_users_7d}</span>
                                            <span class="stat-label">{"Active Users (7d)"}</span>
                                        </div>
                                        <div class="stat-card active-users">
                                            <span class="stat-number">{usage.active_users_30d}</span>
                                            <span class="stat-label">{"Active Users (30d)"}</span>
                                        </div>
                                    </div>

                                    {
                                        // Only show activity breakdown if any item has non-zero credits
                                        if usage.breakdown_by_type.iter().any(|a| a.total_credits > 0.0) {
                                            html! {
                                                <>
                                                    <h4>{"Activity Breakdown"}</h4>
                                                    <div class="activity-breakdown">
                                                        {
                                                            usage.breakdown_by_type.iter().map(|a| {
                                                                html! {
                                                                    <span class="activity-item" key={a.activity_type.clone()}>
                                                                        {format!("{}: {} ({:.4} credits)", a.activity_type, a.count, a.total_credits)}
                                                                    </span>
                                                                }
                                                            }).collect::<Html>()
                                                        }
                                                    </div>
                                                </>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }

                                    {
                                        if !usage.daily_stats.is_empty() {
                                            html! {
                                                <>
                                                    <h4>{"Daily Stats (Recent)"}</h4>
                                                    <table class="daily-stats-table">
                                                        <thead>
                                                            <tr>
                                                                <th>{"Date"}</th>
                                                                <th>{"SMS"}</th>
                                                                <th>{"Calls"}</th>
                                                                <th>{"Cost"}</th>
                                                            </tr>
                                                        </thead>
                                                        <tbody>
                                                            {
                                                                usage.daily_stats.iter().rev().take(14).map(|d| {
                                                                    html! {
                                                                        <tr key={d.date.clone()}>
                                                                            <td>{&d.date}</td>
                                                                            <td>{d.sms_count}</td>
                                                                            <td>{d.call_count}</td>
                                                                            <td>{format!("${:.4}", d.total_cost)}</td>
                                                                        </tr>
                                                                    }
                                                                }).collect::<Html>()
                                                            }
                                                        </tbody>
                                                    </table>
                                                </>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }
                                </div>
                            }
                        } else {
                            html! { <p class="loading">{"Loading usage stats..."}</p> }
                        }
                    }
                                </div>
                            }
                        } else {
                            html! {}
                        }
                    }
                </div>

                // Admin Alerts Section
                <div class="alerts-section">
                    <div class="alerts-header" onclick={{
                        let show_alerts_section = show_alerts_section.clone();
                        Callback::from(move |_| {
                            show_alerts_section.set(!*show_alerts_section);
                        })
                    }}>
                        <h2>
                            {"System Alerts "}
                            {
                                if *alert_count > 0 {
                                    html! { <span class="alert-badge">{*alert_count}</span> }
                                } else {
                                    html! {}
                                }
                            }
                        </h2>
                        <span class="toggle-indicator">{if *show_alerts_section { "▼" } else { "▶" }}</span>
                    </div>

                    {
                        if *show_alerts_section {
                            let admin_alerts_for_actions = admin_alerts.clone();
                            let disabled_types_for_actions = disabled_alert_types.clone();
                            let alert_count_for_actions = alert_count.clone();

                            html! {
                                <div class="alerts-content">
                                    // Acknowledge all button
                                    {
                                        if *alert_count > 0 {
                                            let admin_alerts_ack = admin_alerts_for_actions.clone();
                                            let alert_count_ack = alert_count_for_actions.clone();
                                            html! {
                                                <button class="acknowledge-all-btn" onclick={Callback::from(move |_| {
                                                    let admin_alerts = admin_alerts_ack.clone();
                                                    let alert_count = alert_count_ack.clone();
                                                    wasm_bindgen_futures::spawn_local(async move {
                                                        if let Ok(response) = Api::post("/api/admin/alerts/acknowledge-all")
                                                            .send()
                                                            .await
                                                        {
                                                            if response.ok() {
                                                                // Mark all as acknowledged locally
                                                                let updated: Vec<AdminAlert> = (*admin_alerts).iter().map(|a| {
                                                                    let mut new_alert = a.clone();
                                                                    new_alert.acknowledged = 1;
                                                                    new_alert
                                                                }).collect();
                                                                admin_alerts.set(updated);
                                                                alert_count.set(0);
                                                            }
                                                        }
                                                    });
                                                })}>
                                                    {"Acknowledge All"}
                                                </button>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }

                                    // Disabled alert types section
                                    {
                                        if !(*disabled_types_for_actions).is_empty() {
                                            let disabled_types_display = disabled_types_for_actions.clone();
                                            html! {
                                                <div class="disabled-types-section">
                                                    <h3>{"Disabled Alert Types"}</h3>
                                                    <div class="disabled-types-list">
                                                        {
                                                            (*disabled_types_display).iter().map(|dt| {
                                                                let alert_type = dt.alert_type.clone();
                                                                let disabled_types_enable = disabled_types_for_actions.clone();
                                                                let encoded_type = urlencoding::encode(&alert_type).into_owned();
                                                                html! {
                                                                    <div class="disabled-type-item" key={alert_type.clone()}>
                                                                        <span class="disabled-type-name">{&alert_type}</span>
                                                                        <button class="enable-btn" onclick={Callback::from(move |_| {
                                                                            let disabled_types = disabled_types_enable.clone();
                                                                            let encoded = encoded_type.clone();
                                                                            let alert_type_to_remove = alert_type.clone();
                                                                            wasm_bindgen_futures::spawn_local(async move {
                                                                                if let Ok(response) = Api::post(&format!("/api/admin/alerts/enable/{}", encoded))
                                                                                    .send()
                                                                                    .await
                                                                                {
                                                                                    if response.ok() {
                                                                                        let updated: Vec<DisabledAlertType> = (*disabled_types)
                                                                                            .iter()
                                                                                            .filter(|d| d.alert_type != alert_type_to_remove)
                                                                                            .cloned()
                                                                                            .collect();
                                                                                        disabled_types.set(updated);
                                                                                    }
                                                                                }
                                                                            });
                                                                        })}>
                                                                            {"Enable"}
                                                                        </button>
                                                                    </div>
                                                                }
                                                            }).collect::<Html>()
                                                        }
                                                    </div>
                                                </div>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }

                                    // Alerts table
                                    {
                                        if (*admin_alerts_for_actions).is_empty() {
                                            html! { <p class="no-alerts">{"No alerts recorded."}</p> }
                                        } else {
                                            let alerts_display = admin_alerts_for_actions.clone();
                                            let disabled_types_for_disable = disabled_alert_types.clone();
                                            let admin_alerts_ack = admin_alerts.clone();
                                            let alert_count_ack = alert_count.clone();
                                            html! {
                                                <table class="alerts-table">
                                                    <thead>
                                                        <tr>
                                                            <th>{"Severity"}</th>
                                                            <th>{"Type"}</th>
                                                            <th>{"Location"}</th>
                                                            <th>{"Time"}</th>
                                                            <th>{"Actions"}</th>
                                                        </tr>
                                                    </thead>
                                                    <tbody>
                                                        {
                                                            (*alerts_display).iter().map(|alert| {
                                                                let alert_id = alert.id.unwrap_or(0);
                                                                let alert_type = alert.alert_type.clone();
                                                                let severity_class = match alert.severity.as_str() {
                                                                    "Critical" => "severity-critical",
                                                                    "Error" => "severity-error",
                                                                    "Warning" => "severity-warning",
                                                                    _ => "severity-info",
                                                                };
                                                                let row_class = if alert.acknowledged == 0 {
                                                                    "alert-row unacknowledged"
                                                                } else {
                                                                    "alert-row acknowledged"
                                                                };
                                                                let timestamp = Utc.timestamp_opt(alert.created_at as i64, 0)
                                                                    .single()
                                                                    .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                                                                    .unwrap_or_else(|| "Unknown".to_string());

                                                                let admin_alerts_row = admin_alerts_ack.clone();
                                                                let alert_count_row = alert_count_ack.clone();
                                                                let disabled_types_row = disabled_types_for_disable.clone();
                                                                let encoded_type = urlencoding::encode(&alert_type).into_owned();

                                                                html! {
                                                                    <tr class={row_class} key={alert_id}>
                                                                        <td class={severity_class}>{&alert.severity}</td>
                                                                        <td class="alert-type-cell" title={alert.message.clone()}>{&alert.alert_type}</td>
                                                                        <td class="location-cell">{&alert.location}</td>
                                                                        <td>{timestamp}</td>
                                                                        <td class="actions-cell">
                                                                            {
                                                                                if alert.acknowledged == 0 {
                                                                                    let admin_alerts_ack_btn = admin_alerts_row.clone();
                                                                                    let alert_count_ack_btn = alert_count_row.clone();
                                                                                    html! {
                                                                                        <button class="ack-btn" onclick={Callback::from(move |_| {
                                                                                            let admin_alerts = admin_alerts_ack_btn.clone();
                                                                                            let alert_count = alert_count_ack_btn.clone();
                                                                                            wasm_bindgen_futures::spawn_local(async move {
                                                                                                if let Ok(response) = Api::post(&format!("/api/admin/alerts/{}/acknowledge", alert_id))
                                                                                                    .send()
                                                                                                    .await
                                                                                                {
                                                                                                    if response.ok() {
                                                                                                        let updated: Vec<AdminAlert> = (*admin_alerts).iter().map(|a| {
                                                                                                            if a.id == Some(alert_id) {
                                                                                                                let mut new_alert = a.clone();
                                                                                                                new_alert.acknowledged = 1;
                                                                                                                new_alert
                                                                                                            } else {
                                                                                                                a.clone()
                                                                                                            }
                                                                                                        }).collect();
                                                                                                        admin_alerts.set(updated);
                                                                                                        alert_count.set((*alert_count - 1).max(0));
                                                                                                    }
                                                                                                }
                                                                                            });
                                                                                        })}>
                                                                                            {"Ack"}
                                                                                        </button>
                                                                                    }
                                                                                } else {
                                                                                    html! {}
                                                                                }
                                                                            }
                                                                            <button class="disable-btn" onclick={Callback::from(move |_| {
                                                                                let disabled_types = disabled_types_row.clone();
                                                                                let encoded = encoded_type.clone();
                                                                                let alert_type_clone = alert_type.clone();
                                                                                wasm_bindgen_futures::spawn_local(async move {
                                                                                    if let Ok(response) = Api::post(&format!("/api/admin/alerts/disable/{}", encoded))
                                                                                        .send()
                                                                                        .await
                                                                                    {
                                                                                        if response.ok() {
                                                                                            let current_time = chrono::Utc::now().timestamp() as i32;
                                                                                            let mut updated = (*disabled_types).clone();
                                                                                            updated.push(DisabledAlertType {
                                                                                                id: None,
                                                                                                alert_type: alert_type_clone,
                                                                                                disabled_at: current_time,
                                                                                            });
                                                                                            disabled_types.set(updated);
                                                                                        }
                                                                                    }
                                                                                });
                                                                            })}>
                                                                                {"Disable"}
                                                                            </button>
                                                                        </td>
                                                                    </tr>
                                                                }
                                                            }).collect::<Html>()
                                                        }
                                                    </tbody>
                                                </table>
                                            }
                                        }
                                    }
                                </div>
                            }
                        } else {
                            html! {}
                        }
                    }
                </div>

                // Global SMS Stats Section
                <div class="global-stats-section">
                    <h2>{"Global SMS Delivery Stats"}</h2>
                    {
                        if let Some(stats) = (*global_stats).as_ref() {
                            html! {
                                <>
                                    <div class="stats-summary global">
                                        <div class="stat-card total">
                                            <span class="stat-number">{stats.total_messages}</span>
                                            <span class="stat-label">{"Total"}</span>
                                        </div>
                                        <div class="stat-card delivered">
                                            <span class="stat-number">{stats.delivered}</span>
                                            <span class="stat-label">{"Delivered"}</span>
                                        </div>
                                        <div class="stat-card failed">
                                            <span class="stat-number">{stats.failed}</span>
                                            <span class="stat-label">{"Failed"}</span>
                                        </div>
                                        <div class="stat-card undelivered">
                                            <span class="stat-number">{stats.undelivered}</span>
                                            <span class="stat-label">{"Undelivered"}</span>
                                        </div>
                                        <div class="stat-card sent">
                                            <span class="stat-number">{stats.sent}</span>
                                            <span class="stat-label">{"Sent"}</span>
                                        </div>
                                    </div>
                                    {
                                        if !stats.recent_failed.is_empty() {
                                            html! {
                                                <>
                                                    <h3>{"Recent Failed/Undelivered Messages"}</h3>
                                                    <table class="message-log-table global-failed">
                                                        <thead>
                                                            <tr>
                                                                <th>{"User"}</th>
                                                                <th>{"Status"}</th>
                                                                <th>{"To"}</th>
                                                                <th>{"From"}</th>
                                                                <th>{"Error"}</th>
                                                                <th>{"Time"}</th>
                                                            </tr>
                                                        </thead>
                                                        <tbody>
                                                            {
                                                                stats.recent_failed.iter().map(|msg| {
                                                                    let status_class = match msg.status.as_str() {
                                                                        "failed" => "status-failed",
                                                                        "undelivered" => "status-undelivered",
                                                                        _ => "status-queued",
                                                                    };
                                                                    let user_str = msg.user_email.clone()
                                                                        .unwrap_or_else(|| format!("User {}", msg.user_id));
                                                                    let error_str = msg.error_code.clone()
                                                                        .map(|c| format!("{}: {}", c, msg.error_message.as_deref().unwrap_or("")))
                                                                        .unwrap_or_else(|| "-".to_string());
                                                                    let time_str = Utc.timestamp_opt(msg.created_at as i64, 0)
                                                                        .single()
                                                                        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                                                                        .unwrap_or_else(|| "Invalid".to_string());
                                                                    let from_str = msg.from_number.clone().unwrap_or_else(|| "-".to_string());
                                                                    html! {
                                                                        <tr key={msg.message_sid.clone()}>
                                                                            <td class="user-cell">{user_str}</td>
                                                                            <td><span class={classes!("status-badge", status_class)}>{&msg.status}</span></td>
                                                                            <td>{&msg.to_number}</td>
                                                                            <td>{from_str}</td>
                                                                            <td class="error-cell">{error_str}</td>
                                                                            <td>{time_str}</td>
                                                                        </tr>
                                                                    }
                                                                }).collect::<Html>()
                                                            }
                                                        </tbody>
                                                    </table>
                                                </>
                                            }
                                        } else {
                                            html! { <p class="no-messages">{"No failed messages - all messages delivered successfully!"}</p> }
                                        }
                                    }
                                </>
                            }
                        } else {
                            html! { <p class="loading">{"Loading global stats..."}</p> }
                        }
                    }
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
                                                <th>{"Phone"}</th>
                                                <th>{"Overage Credits"}</th>
                                                <th>{"Monthly Credits"}</th>
                                                <th>{"Tier"}</th>
                                                <th>{"Plan"}</th>
                                                <th>{"Twilio"}</th>
                                                <th>{"Notify"}</th>
                                                <th>{"Joined"}</th>
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
                                                            <tr onclick={onclick} key={user.id} class={classes!(
                                                                "user-row",
                                                                is_selected.then(|| "selected"),
                                                                match user.sub_tier.as_deref() {
                                                                    Some("tier 2") => "gold-user",
                                                                    _ => ""
                                                                }
                                                            )}>
                                                                <td>{user.id}</td>
                                                                <td>
                                                                    <div class="user-email-container">
                                                                        {&user.email}
                                                                        {
                                                                            match user.sub_tier.as_deref() {
                                                                                Some("tier 2") => html! {
                                                                                    <span class="gold-badge">{"★"}</span>
                                                                                },
                                                                                _ => html! {}
                                                                            }
                                                                        }
                                                                        {
                                                                            match user.discount_tier.as_deref() {
                                                                                Some("msg") => html! {
                                                                                    <span class="discount-badge msg">{"msg✦"}</span>
                                                                                },
                                                                                Some("voice") => html! {
                                                                                    <span class="discount-badge voice">{"voice✧"}</span>
                                                                                },
                                                                                Some("full") => html! {
                                                                                    <span class="discount-badge full">{"full✶"}</span>
                                                                                },
                                                                                _ => html! {}
                                                                            }
                                                                        }
                                                                    </div>
                                                                </td>
                                                                <td>{&user.phone_number}</td>
                                                                <td>{format!("{:.2}€", user.credits)}</td>
                                                                <td>{format!("{:.2}€", user.credits_left)}</td>
                                                                <td>
                                                                    <span class={classes!(
                                                                        "tier-badge",
                                                                        match user.sub_tier.as_deref() {
                                                                            Some("tier 2") => "gold",
                                                                            _ => "none"
                                                                        }
                                                                    )}>
                                                                        {user.sub_tier.clone().unwrap_or_else(|| "None".to_string())}
                                                                    </span>
                                                                </td>
                                                                <td>
                                                                    <span class={classes!(
                                                                        "plan-badge",
                                                                        match user.plan_type.as_deref() {
                                                                            Some("byot") => "byot",
                                                                            Some("digest") => "digest",
                                                                            Some("monitor") => "monitor",
                                                                            _ => "none"
                                                                        }
                                                                    )}>
                                                                        {user.plan_type.clone().unwrap_or_else(|| "None".to_string()).to_uppercase()}
                                                                    </span>
                                                                </td>
                                                                <td>
                                                                    {
                                                                        // Only show Twilio status for BYOT users
                                                                        if user.plan_type.as_deref() == Some("byot") {
                                                                            html! {
                                                                                <span class={classes!(
                                                                                    "status-badge",
                                                                                    if user.has_twilio_credentials { "verified" } else { "unverified" }
                                                                                )}>
                                                                                    {if user.has_twilio_credentials { "Yes" } else { "No" }}
                                                                                </span>
                                                                            }
                                                                        } else {
                                                                            html! { <span class="status-badge disabled">{"-"}</span> }
                                                                        }
                                                                    }
                                                                </td>
                                                                <td>
                                                                    <span class={classes!(
                                                                        "status-badge",
                                                                        if user.notify { "enabled" } else { "disabled" }
                                                                    )}>
                                                                        {if user.notify { "Yes" } else { "No" }}
                                                                    </span>
                                                                </td>
                                                                <td>
                                                                    {
                                                                        user.time_to_live.map_or("N/A".to_string(), |ttl| {
                                                                            Utc.timestamp_opt(ttl as i64, 0)
                                                                                .single()
                                                                                .map(|dt| dt.format("%Y-%m-%d").to_string())
                                                                                .unwrap_or_else(|| "Invalid".to_string())
                                                                        })
                                                                    }
                                                                </td>
                                                            </tr>
                                                            if is_selected {
                                                                <tr class="details-row">
                                                                    <td colspan="4">
                                                                        <div class="user-details">
                                                                            <div class="preferred-number-section">
                                                                                <p><strong>{"Current Preferred Number: "}</strong>{user.preferred_number.clone().unwrap_or_else(|| "Not set".to_string())}</p>
                                                                            </div>

                                                                            // Message Stats Section
                                                                            {render_message_stats(
                                                                                user.id,
                                                                                message_stats.get(&user.id),
                                                                                *loading_stats == Some(user.id),
                                                                                *show_all_messages,
                                                                                message_stats.clone(),
                                                                                loading_stats.clone(),
                                                                                show_all_messages.clone(),
                                                                            )}

                                                                        <button 
                                                                            onclick={{
                                                                                let users = users.clone();
                                                                                let error = error.clone();
                                                                                let user_id = user.id;
                                                                                Callback::from(move |_| {
                                                                                    let users = users.clone();
                                                                                    let error = error.clone();
                                                                                    wasm_bindgen_futures::spawn_local(async move {
                                                                                        match Api::post(&format!("/api/billing/increase-credits/{}", user_id))
                                                                                            .send()
                                                                                            .await
                                                                                        {
                                                                                            Ok(response) => {
                                                                                                if response.ok() {
                                                                                                    // Refresh the users list after increasing credits
                                                                                                    if let Ok(response) = Api::get("/api/admin/users")
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
                                                                                    });
                                                                                })
                                                                            }}
                                                                            class="iq-button"
                                                                        >
                                                                            {"+1€ Credits"}
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
                                                                                        match Api::post(&format!("/api/billing/reset-credits/{}", user_id))
                                                                                            .send()
                                                                                            .await
                                                                                        {
                                                                                            Ok(response) => {
                                                                                                if response.ok() {
                                                                                                    // Refresh the users list after resetting credits
                                                                                                    if let Ok(response) = Api::get("/api/admin/users")
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
                                                                                    });
                                                                                })
                                                                            }}
                                                                            class="iq-button reset"
                                                                        >
                                                                            {"Reset Credits"}
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
                                                                                        match Api::post(&format!("/api/admin/monthly-credits/{}/{}", user_id, 10.0))
                                                                                            .send()
                                                                                            .await
                                                                                        {
                                                                                            Ok(response) => {
                                                                                                if response.ok() {
                                                                                                    // Refresh the users list
                                                                                                    if let Ok(response) = Api::get("/api/admin/users")
                                                                                                        .send()
                                                                                                        .await
                                                                                                    {
                                                                                                        if let Ok(updated_users) = response.json::<Vec<UserInfo>>().await {
                                                                                                            users.set(updated_users);
                                                                                                        }
                                                                                                    }
                                                                                                } else {
                                                                                                    error.set(Some("Failed to add monthly credits".to_string()));
                                                                                                }
                                                                                            }
                                                                                            Err(_) => {
                                                                                                error.set(Some("Failed to send request".to_string()));
                                                                                            }
                                                                                        }
                                                                                    });
                                                                                })
                                                                            }}
                                                                            class="iq-button"
                                                                        >
                                                                            {"+10 Messages"}
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
                                                                                        match Api::post(&format!("/api/admin/monthly-credits/{}/{}", user_id, -10.0))
                                                                                            .send()
                                                                                            .await
                                                                                        {
                                                                                            Ok(response) => {
                                                                                                if response.ok() {
                                                                                                    // Refresh the users list
                                                                                                    if let Ok(response) = Api::get("/api/admin/users")
                                                                                                        .send()
                                                                                                        .await
                                                                                                    {
                                                                                                        if let Ok(updated_users) = response.json::<Vec<UserInfo>>().await {
                                                                                                            users.set(updated_users);
                                                                                                        }
                                                                                                    }
                                                                                                } else {
                                                                                                    error.set(Some("Failed to remove monthly credits".to_string()));
                                                                                                }
                                                                                            }
                                                                                            Err(_) => {
                                                                                                error.set(Some("Failed to send request".to_string()));
                                                                                            }
                                                                                        }
                                                                                    });
                                                                                })
                                                                            }}
                                                                            class="iq-button reset"
                                                                        >
                                                                            {"-10 Messages"}
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
                                                                                                    match Api::post(&format!("/api/admin/verify/{}", user_id))
                                                                                                        .send()
                                                                                                        .await
                                                                                                    {
                                                                                                        Ok(response) => {
                                                                                                            if response.ok() {
                                                                                                                // Refresh the users list after verifying
                                                                                                                if let Ok(response) = Api::get("/api/admin/users")
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
                                                                                let current_discount_tier = user.discount_tier.clone();
                                                                                Callback::from(move |_| {
                                                                                    let users = users.clone();
                                                                                    let error = error.clone();
                                                                                    let new_tier = match current_discount_tier.as_deref() {
                                                                                        None => "msg",
                                                                                        Some("msg") => "voice",
                                                                                        Some("voice") => "full",
                                                                                        Some("full") | _ => "none",
                                                                                    };
                                                                                    
                                                                                    wasm_bindgen_futures::spawn_local(async move {
                                                                                        match Api::post(&format!("/api/admin/discount-tier/{}/{}", user_id, new_tier))
                                                                                            .send()
                                                                                            .await
                                                                                        {
                                                                                            Ok(response) => {
                                                                                                if response.ok() {
                                                                                                    // Refresh the users list
                                                                                                    if let Ok(response) = Api::get("/api/admin/users")
                                                                                                        .send()
                                                                                                        .await
                                                                                                    {
                                                                                                        if let Ok(updated_users) = response.json::<Vec<UserInfo>>().await {
                                                                                                            users.set(updated_users);
                                                                                                        }
                                                                                                    }
                                                                                                } else {
                                                                                                    error.set(Some("Failed to update discount tier".to_string()));
                                                                                                }
                                                                                            }
                                                                                            Err(_) => {
                                                                                                error.set(Some("Failed to send request".to_string()));
                                                                                            }
                                                                                        }
                                                                                    });
                                                                                })
                                                                            }}
                                                                            class="iq-button discount-tier"
                                                                        >
                                                                            {match user.discount_tier.as_deref() {
                                                                                None => "Set MSG Discount",
                                                                                Some("msg") => "Set Voice Discount",
                                                                                Some("voice") => "Set Full Discount",
                                                                                Some("full") => "Remove Discount",
                                                                                _ => "Set MSG Discount",
                                                                            }}
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
                                                                                let new_tier = match current_tier.as_deref() {
                                                                                    None => "tier 2",
                                                                                    Some("tier 2") | _ => "tier 0",
                                                                                };
                                                                                    
                                                                                    wasm_bindgen_futures::spawn_local(async move {
                                                                                        match Api::post(&format!("/api/admin/subscription/{}/{}", user_id, urlencoding::encode(new_tier).trim_end_matches('/')))
                                                                                            .send()
                                                                                            .await
                                                                                        {
                                                                                            Ok(response) => {
                                                                                                if response.ok() {
                                                                                                    // Refresh the users list
                                                                                                    if let Ok(response) = Api::get("/api/admin/users")
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
                                                                                    });
                                                                                })
                                                                            }}
                                                                            class="iq-button"
                                                                        >
                                                                            {match user.sub_tier.as_deref() {
                                                                                None => "Set Tier 2",
                                                                                Some("tier 2") => "Remove Subscription",
                                                                                _ => "Set Tier 2"
                                                                            }}
                                                                        </button>
                                                                        <button
                                                                            onclick={{
                                                                                let users = users.clone();
                                                                                let error = error.clone();
                                                                                let user_id = user.id;
                                                                                let current_plan = user.plan_type.clone();
                                                                                Callback::from(move |_| {
                                                                                    let users = users.clone();
                                                                                    let error = error.clone();
                                                                                    // Cycle: None -> byot -> digest -> monitor -> None
                                                                                    let new_plan = match current_plan.as_deref() {
                                                                                        None => "byot",
                                                                                        Some("byot") => "digest",
                                                                                        Some("digest") => "monitor",
                                                                                        Some("monitor") | _ => "none",
                                                                                    };

                                                                                    wasm_bindgen_futures::spawn_local(async move {
                                                                                        match Api::post(&format!("/api/admin/plan-type/{}/{}", user_id, new_plan))
                                                                                            .send()
                                                                                            .await
                                                                                        {
                                                                                            Ok(response) => {
                                                                                                if response.ok() {
                                                                                                    // Refresh the users list
                                                                                                    if let Ok(response) = Api::get("/api/admin/users")
                                                                                                        .send()
                                                                                                        .await
                                                                                                    {
                                                                                                        if let Ok(updated_users) = response.json::<Vec<UserInfo>>().await {
                                                                                                            users.set(updated_users);
                                                                                                        }
                                                                                                    }
                                                                                                } else {
                                                                                                    error.set(Some("Failed to update plan type".to_string()));
                                                                                                }
                                                                                            }
                                                                                            Err(_) => {
                                                                                                error.set(Some("Failed to send request".to_string()));
                                                                                            }
                                                                                        }
                                                                                    });
                                                                                })
                                                                            }}
                                                                            class="iq-button plan-type"
                                                                        >
                                                                            {match user.plan_type.as_deref() {
                                                                                None => "Set BYOT",
                                                                                Some("byot") => "Set Digest",
                                                                                Some("digest") => "Set Monitor",
                                                                                Some("monitor") => "Remove Plan",
                                                                                _ => "Set BYOT"
                                                                            }}
                                                                        </button>

                                                                        // Send Password Reset Link button
                                                                        <button
                                                                            onclick={{
                                                                                let user_id = user.id;
                                                                                let user_email = user.email.clone();
                                                                                let reset_link_status = reset_link_status.clone();
                                                                                Callback::from(move |_| {
                                                                                    let user_id = user_id;
                                                                                    let user_email = user_email.clone();
                                                                                    let reset_link_status = reset_link_status.clone();
                                                                                    reset_link_status.set(Some((user_id, "Sending...".to_string())));
                                                                                    wasm_bindgen_futures::spawn_local(async move {
                                                                                        match Api::post(&format!("/api/admin/send-password-reset/{}", user_id))
                                                                                            .send()
                                                                                            .await
                                                                                        {
                                                                                            Ok(response) => {
                                                                                                if response.ok() {
                                                                                                    reset_link_status.set(Some((user_id, format!("Reset link sent to {}", user_email))));
                                                                                                    // Clear message after 3 seconds
                                                                                                    let reset_link_status = reset_link_status.clone();
                                                                                                    gloo_timers::callback::Timeout::new(3000, move || {
                                                                                                        reset_link_status.set(None);
                                                                                                    }).forget();
                                                                                                } else {
                                                                                                    reset_link_status.set(Some((user_id, "Failed to send reset link".to_string())));
                                                                                                }
                                                                                            }
                                                                                            Err(e) => {
                                                                                                reset_link_status.set(Some((user_id, format!("Error: {}", e))));
                                                                                            }
                                                                                        }
                                                                                    });
                                                                                })
                                                                            }}
                                                                            class="iq-button"
                                                                            style="background: #4ade80;"
                                                                        >
                                                                            {"Send Reset Link"}
                                                                        </button>
                                                                        // Show reset link status for this user
                                                                        {
                                                                            if let Some((status_user_id, status_msg)) = (*reset_link_status).as_ref() {
                                                                                if *status_user_id == user.id {
                                                                                    html! {
                                                                                        <span style="margin-left: 8px; font-size: 0.85rem; color: #4ade80;">
                                                                                            {status_msg}
                                                                                        </span>
                                                                                    }
                                                                                } else {
                                                                                    html! {}
                                                                                }
                                                                            } else {
                                                                                html! {}
                                                                            }
                                                                        }

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
                if (*delete_modal).show {
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
                                                    match Api::delete(&format!("/api/profile/delete/{}", user_id))
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
                .password-section {
                    margin: 2rem 0;
                    padding: 1.5rem;
                    background: rgba(30, 30, 30, 0.7);
                    border-radius: 8px;
                    border: 1px solid rgba(30, 144, 255, 0.2);
                }

                .password-section h2 {
                    margin-bottom: 1rem;
                    color: #1E90FF;
                }

                .password-form {
                    display: flex;
                    align-items: center;
                    gap: 1rem;
                    flex-wrap: wrap;
                }

                .password-input {
                    flex: 1;
                    min-width: 200px;
                    padding: 0.75rem;
                    border: 1px solid rgba(30, 144, 255, 0.2);
                    border-radius: 4px;
                    background: rgba(0, 0, 0, 0.3);
                    color: #fff;
                    font-size: 1rem;
                }

                .password-input:focus {
                    outline: none;
                    border-color: #1E90FF;
                }

                .password-status {
                    padding: 0.5rem 1rem;
                    border-radius: 4px;
                    font-size: 0.9rem;
                }

                .password-status.success {
                    background: rgba(76, 175, 80, 0.2);
                    color: #4CAF50;
                }

                .password-status.error {
                    background: rgba(255, 107, 107, 0.2);
                    color: #FF6B6B;
                }
                "#}
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

                    .email-broadcast {
                        margin-top: 2rem;
                        border-top: 1px solid rgba(30, 144, 255, 0.2);
                        padding-top: 2rem;
                    }

                    .email-subject-input {
                        width: 100%;
                        padding: 0.75rem;
                        margin-bottom: 1rem;
                        border: 1px solid rgba(30, 144, 255, 0.2);
                        border-radius: 4px;
                        background: rgba(0, 0, 0, 0.3);
                        color: #fff;
                        font-size: 1rem;
                    }

                    .email-subject-input:focus {
                        outline: none;
                        border-color: #1E90FF;
                    }

                    .broadcast-button.email {
                        background: linear-gradient(45deg, #1E90FF, #4169E1);
                        color: white;
                    }

                    .broadcast-button.email:hover {
                        background: linear-gradient(45deg, #4169E1, #1E90FF);
                        box-shadow: 0 4px 15px rgba(30, 144, 255, 0.4);
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

                    .user-email-container {
                        display: flex;
                        align-items: center;
                        gap: 0.5rem;
                    }

                    .gold-badge {
                        font-size: 1.2rem;
                    }

                    .blue-badge {
                        color: #1E90FF;
                    }

                    .gold-badge {
                        color: #FFD700;
                    }

                    .silver-badge {
                        color: #C0C0C0;
                    }

                    .bronze-badge {
                        color: #CD7F32;
                    }

                    .discount-badge {
                        font-size: 1.2rem;
                        margin-left: 0.2rem;
                    }

                    .discount-badge.msg {
                        color: #4CAF50;
                    }

                    .discount-badge.voice {
                        color: #FFC107;
                    }

                    .discount-badge.full {
                        color: #E91E63;
                    }

                    .blue-user {
                        background: linear-gradient(90deg, rgba(30, 144, 255, 0.05), transparent);
                        border-left: 3px solid #1E90FF;
                    }

                    .gold-user {
                        background: linear-gradient(90deg, rgba(255, 215, 0, 0.05), transparent);
                        border-left: 3px solid #FFD700;
                    }

                    .silver-user {
                        background: linear-gradient(90deg, rgba(192, 192, 192, 0.05), transparent);
                        border-left: 3px solid #C0C0C0;
                    }

                    .bronze-user {
                        background: linear-gradient(90deg, rgba(205, 127, 50, 0.05), transparent);
                        border-left: 3px solid #CD7F32;
                    }

                    .tier-badge {
                        padding: 0.25rem 0.5rem;
                        border-radius: 4px;
                        font-size: 0.8rem;
                        font-weight: 500;
                    }

                    .tier-badge.blue {
                        background: rgba(30, 144, 255, 0.1);
                        color: #1E90FF;
                        border: 1px solid rgba(30, 144, 255, 0.2);
                    }

                    .tier-badge.gold {
                        background: rgba(255, 215, 0, 0.1);
                        color: #FFD700;
                        border: 1px solid rgba(255, 215, 0, 0.2);
                    }

                    .tier-badge.silver {
                        background: rgba(192, 192, 192, 0.1);
                        color: #C0C0C0;
                        border: 1px solid rgba(192, 192, 192, 0.2);
                    }
                    .tier-badge.bronze {
                        background: rgba(205, 127, 50, 0.1);
                        color: #CD7F32;
                        border: 1px solid rgba(205, 127, 50, 0.2);
                    }
                    .tier-badge.none {
                        background: rgba(128, 128, 128, 0.1);
                        color: #808080;
                        border: 1px solid rgba(128, 128, 128, 0.2);
                    }

                    .status-badge {
                        padding: 0.25rem 0.5rem;
                        border-radius: 4px;
                        font-size: 0.8rem;
                        font-weight: 500;
                    }

                    .status-badge.verified {
                        background: rgba(76, 175, 80, 0.1);
                        color: #4CAF50;
                        border: 1px solid rgba(76, 175, 80, 0.2);
                    }

                    .status-badge.unverified {
                        background: rgba(255, 152, 0, 0.1);
                        color: #FF9800;
                        border: 1px solid rgba(255, 152, 0, 0.2);
                    }

                    .status-badge.enabled {
                        background: rgba(33, 150, 243, 0.1);
                        color: #2196F3;
                        border: 1px solid rgba(33, 150, 243, 0.2);
                    }

                    .status-badge.disabled {
                        background: rgba(158, 158, 158, 0.1);
                        color: #9E9E9E;
                        border: 1px solid rgba(158, 158, 158, 0.2);
                    }

                    .status-badge.discount-msg {
                        background: rgba(76, 175, 80, 0.1);
                        color: #4CAF50;
                        border: 1px solid rgba(76, 175, 80, 0.2);
                    }

                    .status-badge.discount-voice {
                        background: rgba(255, 193, 7, 0.1);
                        color: #FFC107;
                        border: 1px solid rgba(255, 193, 7, 0.2);
                    }

                    .status-badge.discount-full {
                        background: rgba(233, 30, 99, 0.1);
                        color: #E91E63;
                        border: 1px solid rgba(233, 30, 99, 0.2);
                    }

                    .iq-button.discount-tier {
                        background: linear-gradient(45deg, #4CAF50, #81C784);
                    }

                    .iq-button.discount-tier:hover {
                        background: linear-gradient(45deg, #81C784, #4CAF50);
                        box-shadow: 0 4px 15px rgba(76, 175, 80, 0.4);
                    }

                    .iq-button.plan-type {
                        background: linear-gradient(45deg, #00BCD4, #26C6DA);
                    }

                    .iq-button.plan-type:hover {
                        background: linear-gradient(45deg, #26C6DA, #00BCD4);
                        box-shadow: 0 4px 15px rgba(0, 188, 212, 0.4);
                    }

                    .plan-badge {
                        padding: 0.25rem 0.5rem;
                        border-radius: 4px;
                        font-size: 0.8rem;
                        font-weight: 500;
                    }

                    .plan-badge.byot {
                        background: rgba(156, 39, 176, 0.1);
                        color: #9C27B0;
                        border: 1px solid rgba(156, 39, 176, 0.2);
                    }

                    .plan-badge.digest {
                        background: rgba(0, 188, 212, 0.1);
                        color: #00BCD4;
                        border: 1px solid rgba(0, 188, 212, 0.2);
                    }

                    .plan-badge.monitor {
                        background: rgba(255, 152, 0, 0.1);
                        color: #FF9800;
                        border: 1px solid rgba(255, 152, 0, 0.2);
                    }

                    .plan-badge.none {
                        background: rgba(128, 128, 128, 0.1);
                        color: #808080;
                        border: 1px solid rgba(128, 128, 128, 0.2);
                    }

                    .iq-button.migrate {
                        background: linear-gradient(45deg, #9C27B0, #673AB7);
                        color: white;
                    }

                    .iq-button.migrate:hover {
                        background: linear-gradient(45deg, #673AB7, #9C27B0);
                        box-shadow: 0 4px 15px rgba(156, 39, 176, 0.4);
                    }

                    .users-table th {
                        padding: 0.75rem;
                        text-align: left;
                        border-bottom: 2px solid rgba(30, 144, 255, 0.2);
                        color: #1E90FF;
                        font-weight: 600;
                        white-space: nowrap;
                    }

                    .users-table td {
                        padding: 0.75rem;
                        border-bottom: 1px solid rgba(30, 144, 255, 0.1);
                        white-space: nowrap;
                    }

                    .users-table-container {
                        overflow-x: auto;
                        margin: 1rem 0;
                    }

                    /* Critical Message Stats Styles */
                    .critical-stats-section {
                        margin: 2rem 0;
                        padding: 1.5rem;
                        background: rgba(30, 30, 30, 0.7);
                        border-radius: 8px;
                        border: 1px solid rgba(30, 144, 255, 0.2);
                    }

                    .critical-stats-section h3 {
                        color: #1E90FF;
                        margin-bottom: 1.5rem;
                        font-size: 1.2rem;
                        font-weight: 600;
                    }

                    .stats-table {
                        width: 100%;
                        border-collapse: collapse;
                        background: rgba(0, 0, 0, 0.3);
                        border-radius: 8px;
                        overflow: hidden;
                    }

                    .stats-header {
                        display: grid;
                        grid-template-columns: 1fr 2fr 1.5fr 1.5fr 1.5fr 1.5fr;
                        background: rgba(30, 144, 255, 0.1);
                        border-bottom: 2px solid rgba(30, 144, 255, 0.3);
                    }

                    .stats-row {
                        display: grid;
                        grid-template-columns: 1fr 2fr 1.5fr 1.5fr 1.5fr 1.5fr;
                        border-bottom: 1px solid rgba(30, 144, 255, 0.1);
                        transition: all 0.3s ease;
                    }

                    .stats-row:hover {
                        background: rgba(30, 144, 255, 0.05);
                    }

                    .stats-cell {
                        padding: 0.75rem;
                        color: #fff;
                        font-size: 0.9rem;
                        display: flex;
                        align-items: center;
                    }

                    .stats-header .stats-cell {
                        color: #1E90FF;
                        font-weight: 600;
                        font-size: 0.85rem;
                        text-transform: uppercase;
                        letter-spacing: 0.5px;
                    }

                    .stats-cell strong {
                        color: #FFD700;
                    }

                    .no-stats {
                        padding: 2rem;
                        text-align: center;
                        color: #999;
                        font-style: italic;
                    }

                    @media (max-width: 768px) {
                        .stats-header,
                        .stats-row {
                            grid-template-columns: 1fr;
                            gap: 0.5rem;
                        }

                        .stats-cell {
                            padding: 0.5rem;
                            border-bottom: 1px solid rgba(30, 144, 255, 0.1);
                        }

                        .stats-header .stats-cell {
                            background: rgba(30, 144, 255, 0.2);
                            margin-bottom: 0.25rem;
                        }
                    }

                    /* Message Stats Section */
                    .message-stats-section {
                        margin: 1.5rem 0;
                        padding: 1rem;
                        background: rgba(30, 144, 255, 0.05);
                        border-radius: 8px;
                        border: 1px solid rgba(30, 144, 255, 0.2);
                    }

                    .message-stats-section h3 {
                        margin: 0 0 1rem 0;
                        color: #FFD700;
                        font-size: 1.1rem;
                    }

                    .stats-button {
                        background: linear-gradient(135deg, #1E90FF 0%, #4169E1 100%);
                    }

                    .stats-summary {
                        display: flex;
                        gap: 1rem;
                        margin-bottom: 1rem;
                        flex-wrap: wrap;
                    }

                    .stat-card {
                        flex: 1;
                        min-width: 80px;
                        padding: 0.75rem;
                        border-radius: 6px;
                        text-align: center;
                        background: rgba(255, 255, 255, 0.1);
                    }

                    .stat-card.total {
                        background: rgba(100, 100, 100, 0.3);
                    }

                    .stat-card.delivered {
                        background: rgba(40, 167, 69, 0.3);
                        border: 1px solid rgba(40, 167, 69, 0.5);
                    }

                    .stat-card.failed {
                        background: rgba(220, 53, 69, 0.3);
                        border: 1px solid rgba(220, 53, 69, 0.5);
                    }

                    .stat-card.undelivered {
                        background: rgba(255, 165, 0, 0.3);
                        border: 1px solid rgba(255, 165, 0, 0.5);
                    }

                    .stat-card.sent {
                        background: rgba(100, 149, 237, 0.3);
                        border: 1px solid rgba(100, 149, 237, 0.5);
                    }

                    .stat-card.cost {
                        background: rgba(30, 144, 255, 0.2);
                        border: 1px solid rgba(30, 144, 255, 0.4);
                    }

                    .stat-card.total-cost {
                        background: rgba(255, 215, 0, 0.25);
                        border: 1px solid rgba(255, 215, 0, 0.5);
                    }

                    .stat-card.growth {
                        background: rgba(138, 43, 226, 0.25);
                        border: 1px solid rgba(138, 43, 226, 0.5);
                    }

                    .stat-card.active-users {
                        background: rgba(0, 191, 165, 0.25);
                        border: 1px solid rgba(0, 191, 165, 0.5);
                    }

                    .stat-card.international {
                        background: rgba(255, 87, 51, 0.3);
                        border: 1px solid rgba(255, 87, 51, 0.6);
                    }

                    .stat-card.international-avg {
                        background: rgba(255, 165, 0, 0.3);
                        border: 1px solid rgba(255, 165, 0, 0.6);
                    }

                    .stat-card.us-ca {
                        background: rgba(40, 167, 69, 0.2);
                        border: 1px solid rgba(40, 167, 69, 0.4);
                    }

                    .stat-number {
                        display: block;
                        font-size: 1.5rem;
                        font-weight: bold;
                        color: white;
                    }

                    .stat-label {
                        display: block;
                        font-size: 0.75rem;
                        color: #ccc;
                        text-transform: uppercase;
                    }

                    .filter-toggle {
                        margin-bottom: 1rem;
                    }

                    .filter-toggle label {
                        color: #ccc;
                        cursor: pointer;
                        display: flex;
                        align-items: center;
                        gap: 0.5rem;
                    }

                    .filter-toggle input[type="checkbox"] {
                        width: 16px;
                        height: 16px;
                    }

                    .message-log-table {
                        width: 100%;
                        border-collapse: collapse;
                        font-size: 0.85rem;
                    }

                    .message-log-table th,
                    .message-log-table td {
                        padding: 0.5rem;
                        text-align: left;
                        border-bottom: 1px solid rgba(255, 255, 255, 0.1);
                    }

                    .message-log-table th {
                        color: #FFD700;
                        font-weight: 600;
                        background: rgba(0, 0, 0, 0.2);
                    }

                    .message-log-table tr:hover {
                        background: rgba(255, 255, 255, 0.05);
                    }

                    .message-log-table .error-cell {
                        max-width: 200px;
                        overflow: hidden;
                        text-overflow: ellipsis;
                        white-space: nowrap;
                        color: #ff6b6b;
                    }

                    .status-badge {
                        padding: 0.25rem 0.5rem;
                        border-radius: 4px;
                        font-size: 0.75rem;
                        font-weight: 600;
                        text-transform: uppercase;
                    }

                    .status-delivered {
                        background: rgba(40, 167, 69, 0.8);
                        color: white;
                    }

                    .status-failed {
                        background: rgba(220, 53, 69, 0.8);
                        color: white;
                    }

                    .status-undelivered {
                        background: rgba(255, 165, 0, 0.8);
                        color: black;
                    }

                    .status-sent {
                        background: rgba(30, 144, 255, 0.8);
                        color: white;
                    }

                    .status-queued {
                        background: rgba(128, 128, 128, 0.8);
                        color: white;
                    }

                    .no-messages {
                        color: #999;
                        font-style: italic;
                        text-align: center;
                        padding: 1rem;
                    }

                    .loading {
                        color: #FFD700;
                        font-style: italic;
                    }

                    /* Cost & Usage Stats Section */
                    .cost-usage-stats-section {
                        margin: 2rem 0;
                        padding: 1.5rem;
                        background: rgba(30, 144, 255, 0.05);
                        border-radius: 12px;
                        border: 1px solid rgba(30, 144, 255, 0.2);
                    }

                    .cost-usage-stats-section h2 {
                        color: #FFD700;
                        margin: 0 0 1rem 0;
                    }

                    .cost-usage-stats-section h3 {
                        color: #7EB2FF;
                        margin: 1.5rem 0 0.75rem 0;
                        font-size: 1.1rem;
                    }

                    .cost-usage-stats-section h4 {
                        color: #ccc;
                        margin: 1rem 0 0.5rem 0;
                        font-size: 0.95rem;
                    }

                    /* Key metrics row - side by side */
                    .key-metrics-row {
                        display: flex;
                        gap: 1rem;
                        margin-bottom: 1rem;
                    }

                    .key-metric {
                        flex: 1;
                        text-align: center;
                        padding: 1rem;
                        border-radius: 12px;
                    }

                    .key-metric.intl {
                        background: rgba(255, 87, 51, 0.15);
                        border: 2px solid rgba(255, 87, 51, 0.5);
                    }

                    .key-metric.us-ca {
                        background: rgba(40, 167, 69, 0.15);
                        border: 2px solid rgba(40, 167, 69, 0.5);
                    }

                    .key-label {
                        display: block;
                        font-size: 0.85rem;
                        color: #aaa;
                        margin-bottom: 0.25rem;
                    }

                    .key-number {
                        display: block;
                        font-size: 2rem;
                        font-weight: bold;
                    }

                    .key-metric.intl .key-number {
                        color: #FF5733;
                    }

                    .key-metric.us-ca .key-number {
                        color: #28a745;
                    }

                    .key-context {
                        display: block;
                        font-size: 0.8rem;
                        color: #888;
                        margin-top: 0.25rem;
                    }

                    /* Collapsible details */
                    .cost-details {
                        margin-top: 1rem;
                        border: 1px solid rgba(255, 255, 255, 0.1);
                        border-radius: 8px;
                    }

                    .cost-details summary {
                        padding: 0.75rem 1rem;
                        cursor: pointer;
                        color: #888;
                        font-size: 0.9rem;
                    }

                    .cost-details summary:hover {
                        color: #aaa;
                    }

                    .details-content {
                        padding: 0.5rem 1rem 1rem;
                        border-top: 1px solid rgba(255, 255, 255, 0.1);
                    }

                    .detail-row {
                        display: flex;
                        justify-content: space-between;
                        padding: 0.25rem 0;
                        font-size: 0.85rem;
                    }

                    .detail-row span:first-child {
                        color: #888;
                    }

                    .detail-row span:last-child {
                        color: #ddd;
                    }

                    /* User cost chart */
                    .user-cost-chart {
                        margin-top: 0.5rem;
                    }

                    .chart-row {
                        display: flex;
                        align-items: center;
                        gap: 0.5rem;
                        margin-bottom: 0.4rem;
                    }

                    .chart-label {
                        width: 100px;
                        font-size: 0.8rem;
                        color: #aaa;
                        flex-shrink: 0;
                    }

                    .chart-bar-container {
                        flex: 1;
                        height: 20px;
                        background: rgba(255, 255, 255, 0.1);
                        border-radius: 3px;
                        overflow: hidden;
                    }

                    .chart-bar-container .bar {
                        height: 100%;
                        border-radius: 3px;
                        transition: width 0.3s ease;
                    }

                    .chart-bar-container .bar.intl {
                        background: linear-gradient(90deg, #FF5733, #FF8C5A);
                    }

                    .chart-bar-container .bar.us-ca {
                        background: linear-gradient(90deg, #28a745, #5cb85c);
                    }

                    .chart-value {
                        width: 60px;
                        font-size: 0.85rem;
                        color: #fff;
                        text-align: right;
                        flex-shrink: 0;
                    }

                    .no-data {
                        color: #666;
                        font-style: italic;
                    }

                    .daily-stats-table {
                        width: 100%;
                        border-collapse: collapse;
                        font-size: 0.85rem;
                        margin-top: 0.5rem;
                    }

                    .daily-stats-table th,
                    .daily-stats-table td {
                        padding: 0.5rem;
                        text-align: left;
                        border-bottom: 1px solid rgba(255, 255, 255, 0.1);
                    }

                    .daily-stats-table th {
                        color: #7EB2FF;
                        font-weight: 600;
                        background: rgba(0, 0, 0, 0.2);
                    }

                    .daily-stats-table td {
                        color: #ddd;
                    }

                    .daily-stats-table tr:hover {
                        background: rgba(255, 255, 255, 0.05);
                    }

                    .activity-breakdown {
                        display: flex;
                        flex-wrap: wrap;
                        gap: 0.75rem;
                        margin-top: 0.5rem;
                    }

                    .activity-item {
                        background: rgba(100, 100, 100, 0.3);
                        padding: 0.4rem 0.8rem;
                        border-radius: 4px;
                        font-size: 0.85rem;
                        color: #ddd;
                    }

                    /* Collapsible Sections */
                    .collapsible-section {
                        background: linear-gradient(135deg, #1a1a2e 0%, #16213e 100%);
                        border-radius: 12px;
                        padding: 1.5rem;
                        margin-bottom: 1.5rem;
                        border: 1px solid rgba(255, 255, 255, 0.1);
                    }

                    .collapsible-header {
                        display: flex;
                        justify-content: space-between;
                        align-items: center;
                        cursor: pointer;
                        padding: 0.5rem;
                        margin: -0.5rem;
                        border-radius: 8px;
                        transition: background 0.2s;
                    }

                    .collapsible-header:hover {
                        background: rgba(255, 255, 255, 0.05);
                    }

                    .collapsible-header h2 {
                        color: #7EB2FF;
                        margin: 0;
                        display: flex;
                        align-items: center;
                        gap: 0.75rem;
                        flex-wrap: wrap;
                    }

                    .header-stat {
                        font-size: 0.85rem;
                        color: #28a745;
                        font-weight: normal;
                    }

                    .collapsible-content {
                        margin-top: 1rem;
                    }

                    /* Admin Alerts Section */
                    .alerts-section {
                        background: linear-gradient(135deg, #1a1a2e 0%, #16213e 100%);
                        border-radius: 12px;
                        padding: 1.5rem;
                        margin-bottom: 1.5rem;
                        border: 1px solid rgba(255, 200, 100, 0.3);
                    }

                    .alerts-header {
                        display: flex;
                        justify-content: space-between;
                        align-items: center;
                        cursor: pointer;
                        padding: 0.5rem;
                        margin: -0.5rem;
                        border-radius: 8px;
                        transition: background 0.2s;
                    }

                    .alerts-header:hover {
                        background: rgba(255, 255, 255, 0.05);
                    }

                    .alerts-header h2 {
                        color: #FFD700;
                        margin: 0;
                        display: flex;
                        align-items: center;
                        gap: 0.75rem;
                    }

                    .alert-badge {
                        background: #ff4444;
                        color: white;
                        font-size: 0.8rem;
                        padding: 0.2rem 0.6rem;
                        border-radius: 12px;
                        font-weight: bold;
                    }

                    .toggle-indicator {
                        color: #888;
                        font-size: 1rem;
                    }

                    .alerts-content {
                        margin-top: 1rem;
                    }

                    .acknowledge-all-btn {
                        background: #28a745;
                        color: white;
                        border: none;
                        padding: 0.5rem 1rem;
                        border-radius: 6px;
                        cursor: pointer;
                        margin-bottom: 1rem;
                        font-size: 0.9rem;
                    }

                    .acknowledge-all-btn:hover {
                        background: #218838;
                    }

                    .disabled-types-section {
                        background: rgba(255, 100, 100, 0.1);
                        border: 1px solid rgba(255, 100, 100, 0.3);
                        border-radius: 8px;
                        padding: 1rem;
                        margin-bottom: 1rem;
                    }

                    .disabled-types-section h3 {
                        color: #ff8888;
                        margin: 0 0 0.75rem 0;
                        font-size: 1rem;
                    }

                    .disabled-types-list {
                        display: flex;
                        flex-wrap: wrap;
                        gap: 0.5rem;
                    }

                    .disabled-type-item {
                        display: flex;
                        align-items: center;
                        gap: 0.5rem;
                        background: rgba(0, 0, 0, 0.3);
                        padding: 0.4rem 0.8rem;
                        border-radius: 6px;
                    }

                    .disabled-type-name {
                        color: #ddd;
                        font-size: 0.85rem;
                    }

                    .enable-btn {
                        background: #28a745;
                        color: white;
                        border: none;
                        padding: 0.2rem 0.5rem;
                        border-radius: 4px;
                        cursor: pointer;
                        font-size: 0.75rem;
                    }

                    .enable-btn:hover {
                        background: #218838;
                    }

                    .no-alerts {
                        color: #888;
                        font-style: italic;
                        text-align: center;
                        padding: 1rem;
                    }

                    .alerts-table {
                        width: 100%;
                        border-collapse: collapse;
                        font-size: 0.85rem;
                    }

                    .alerts-table th,
                    .alerts-table td {
                        padding: 0.6rem;
                        text-align: left;
                        border-bottom: 1px solid rgba(255, 255, 255, 0.1);
                    }

                    .alerts-table th {
                        color: #FFD700;
                        font-weight: 600;
                        background: rgba(0, 0, 0, 0.3);
                    }

                    .alert-row {
                        transition: background 0.2s;
                    }

                    .alert-row:hover {
                        background: rgba(255, 255, 255, 0.05);
                    }

                    .alert-row.unacknowledged {
                        background: rgba(255, 200, 100, 0.1);
                    }

                    .alert-row.acknowledged {
                        opacity: 0.7;
                    }

                    .severity-critical {
                        color: #ff4444;
                        font-weight: bold;
                    }

                    .severity-error {
                        color: #ff8800;
                        font-weight: bold;
                    }

                    .severity-warning {
                        color: #ffcc00;
                    }

                    .severity-info {
                        color: #7EB2FF;
                    }

                    .alert-type-cell {
                        color: #ddd;
                        max-width: 250px;
                        overflow: hidden;
                        text-overflow: ellipsis;
                        white-space: nowrap;
                    }

                    .location-cell {
                        color: #888;
                        font-family: monospace;
                        font-size: 0.8rem;
                    }

                    .actions-cell {
                        display: flex;
                        gap: 0.5rem;
                    }

                    .ack-btn {
                        background: #007bff;
                        color: white;
                        border: none;
                        padding: 0.25rem 0.5rem;
                        border-radius: 4px;
                        cursor: pointer;
                        font-size: 0.75rem;
                    }

                    .ack-btn:hover {
                        background: #0056b3;
                    }

                    .disable-btn {
                        background: #6c757d;
                        color: white;
                        border: none;
                        padding: 0.25rem 0.5rem;
                        border-radius: 4px;
                        cursor: pointer;
                        font-size: 0.75rem;
                    }

                    .disable-btn:hover {
                        background: #545b62;
                    }

                "#}
            </style>
        </div>
    }
}
