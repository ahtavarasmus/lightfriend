use yew::prelude::*;
use chrono::{Utc, TimeZone, Duration};
use crate::profile::billing_models::format_timestamp;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct UsageLog {
    pub id: i32,
    pub user_id: i32,
    pub activity_type: String,
    pub timestamp: i32,
    pub sid: Option<String>,
    pub status: Option<String>,
    pub success: Option<bool>,
    pub credits: Option<f32>,
    pub time_consumed: Option<i32>,
    pub reason: Option<String>,
    pub recharge_threshold_timestamp: Option<i32>,
    pub zero_credits_timestamp: Option<i32>,
}

#[derive(Properties, PartialEq)]
pub struct UsageLogsProps {
    pub usage_logs: Vec<UsageLog>,
    pub activity_filter: Option<String>,
    pub on_filter_change: Callback<Option<String>>,
}

#[function_component(UsageLogs)]
pub fn usage_logs(props: &UsageLogsProps) -> Html {
    html! {
        <div class="filter-section">
            <h3>{"Usage Logs"}</h3>
            <div class="usage-filter">
                <button 
                    class={classes!(
                        "filter-button",
                        (props.activity_filter.is_none()).then_some("active")
                    )}
                    onclick={
                        let on_filter_change = props.on_filter_change.clone();
                        Callback::from(move |_| on_filter_change.emit(None))
                    }
                >
                    {"All"}
                </button>
                // SMS
                <button 
                    class={classes!(
                        "filter-button",
                        (props.activity_filter.as_deref() == Some("sms")).then_some("active")
                    )}
                    onclick={
                        let on_filter_change = props.on_filter_change.clone();
                        Callback::from(move |_| on_filter_change.emit(Some("sms".to_string())))
                    }
                >
                    {"SMS"}
                </button>
                
                // Call
                <button 
                    class={classes!(
                        "filter-button",
                        (props.activity_filter.as_deref() == Some("call")).then_some("active")
                    )}
                    onclick={
                        let on_filter_change = props.on_filter_change.clone();
                        Callback::from(move |_| on_filter_change.emit(Some("call".to_string())))
                    }
                >
                    {"Calls"}
                </button>

                // Calendar Notifications
                <button 
                    class={classes!(
                        "filter-button",
                        (props.activity_filter.as_deref() == Some("calendar_notification")).then_some("active")
                    )}
                    onclick={
                        let on_filter_change = props.on_filter_change.clone();
                        Callback::from(move |_| on_filter_change.emit(Some("calendar_notification".to_string())))
                    }
                >
                    {"Calendar"}
                </button>

                // Email Categories
                <button 
                    class={classes!(
                        "filter-button",
                        (props.activity_filter.as_deref() == Some("email_priority")).then_some("active")
                    )}
                    onclick={
                        let on_filter_change = props.on_filter_change.clone();
                        Callback::from(move |_| on_filter_change.emit(Some("email_priority".to_string())))
                    }
                >
                    {"Email Priority"}
                </button>

                <button 
                    class={classes!(
                        "filter-button",
                        (props.activity_filter.as_deref() == Some("email_waiting_check")).then_some("active")
                    )}
                    onclick={
                        let on_filter_change = props.on_filter_change.clone();
                        Callback::from(move |_| on_filter_change.emit(Some("email_waiting_check".to_string())))
                    }
                >
                    {"Email Waiting"}
                </button>

                <button 
                    class={classes!(
                        "filter-button",
                        (props.activity_filter.as_deref() == Some("email_critical")).then_some("active")
                    )}
                    onclick={
                        let on_filter_change = props.on_filter_change.clone();
                        Callback::from(move |_| on_filter_change.emit(Some("email_critical".to_string())))
                    }
                >
                    {"Email Critical"}
                </button>

                // WhatsApp Categories
                <button 
                    class={classes!(
                        "filter-button",
                        (props.activity_filter.as_deref() == Some("whatsapp_critical")).then_some("active")
                    )}
                    onclick={
                        let on_filter_change = props.on_filter_change.clone();
                        Callback::from(move |_| on_filter_change.emit(Some("whatsapp_critical".to_string())))
                    }
                >
                    {"WhatsApp Critical"}
                </button>

                <button 
                    class={classes!(
                        "filter-button",
                        (props.activity_filter.as_deref() == Some("whatsapp_priority")).then_some("active")
                    )}
                    onclick={
                        let on_filter_change = props.on_filter_change.clone();
                        Callback::from(move |_| on_filter_change.emit(Some("whatsapp_priority".to_string())))
                    }
                >
                    {"WhatsApp Priority"}
                </button>

                <button 
                    class={classes!(
                        "filter-button",
                        (props.activity_filter.as_deref() == Some("whatsapp_waiting_check")).then_some("active")
                    )}
                    onclick={
                        let on_filter_change = props.on_filter_change.clone();
                        Callback::from(move |_| on_filter_change.emit(Some("whatsapp_waiting_check".to_string())))
                    }
                >
                    {"WhatsApp Waiting"}
                </button>

                <button 
                    class={classes!(
                        "filter-button",
                        (props.activity_filter.as_deref() == Some("failed")).then_some("active")
                    )}
                    onclick={
                        let on_filter_change = props.on_filter_change.clone();
                        Callback::from(move |_| on_filter_change.emit(Some("failed".to_string())))
                    }
                >
                    {"Failed"}
                </button>
            </div>

            <div class="usage-logs">
                {
                    props.usage_logs.iter()
                        .filter(|log| {
                            if let Some(filter) = props.activity_filter.as_ref() {
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
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UserWaitingStats {
    pub user_id: i32,
    pub total_waiting_messages: usize,
    pub days_active: usize,
    pub average_per_day: f64,
    pub first_activity: i32,
    pub last_activity: i32,
}

#[derive(Properties, PartialEq)]
pub struct WaitingMessageStatsProps {
    pub usage_logs: Vec<UsageLog>,
}

fn calculate_waiting_stats(usage_logs: &[UsageLog]) -> Vec<UserWaitingStats> {
    let mut user_stats: HashMap<i32, (Vec<i32>, usize)> = HashMap::new();
    
    // Filter waiting messages and group by user
    for log in usage_logs.iter() {
        if log.activity_type.contains("waiting") {
            let entry = user_stats.entry(log.user_id).or_insert((Vec::new(), 0));
            entry.0.push(log.timestamp);
            entry.1 += 1;
        }
    }
    
    let mut stats = Vec::new();
    
    for (user_id, (timestamps, total_count)) in user_stats {
        if timestamps.is_empty() {
            continue;
        }
        
        let mut sorted_timestamps = timestamps;
        sorted_timestamps.sort();
        
        let first_activity = *sorted_timestamps.first().unwrap();
        let last_activity = *sorted_timestamps.last().unwrap();
        
        // Calculate unique days
        let mut unique_days = std::collections::HashSet::new();
        for &timestamp in &sorted_timestamps {
            let dt = Utc.timestamp_opt(timestamp as i64, 0).single().unwrap();
            let day_key = dt.format("%Y-%m-%d").to_string();
            unique_days.insert(day_key);
        }
        
        let days_active = unique_days.len();
        let average_per_day = if days_active > 0 {
            total_count as f64 / days_active as f64
        } else {
            0.0
        };
        
        stats.push(UserWaitingStats {
            user_id,
            total_waiting_messages: total_count,
            days_active,
            average_per_day,
            first_activity,
            last_activity,
        });
    }
    
    // Sort by average per day (descending)
    stats.sort_by(|a, b| b.average_per_day.partial_cmp(&a.average_per_day).unwrap());
    
    stats
}

#[function_component(WaitingMessageStats)]
pub fn waiting_message_stats(props: &WaitingMessageStatsProps) -> Html {
    let stats = calculate_waiting_stats(&props.usage_logs);
    
    html! {
        <div class="waiting-stats-section">
            <h3>{"Waiting Check Message Statistics by User"}</h3>
            <div class="stats-table">
                <div class="stats-header">
                    <div class="stats-cell">{"User ID"}</div>
                    <div class="stats-cell">{"Total Waiting Checks"}</div>
                    <div class="stats-cell">{"Days Active"}</div>
                    <div class="stats-cell">{"Average per Day"}</div>
                    <div class="stats-cell">{"First Activity"}</div>
                    <div class="stats-cell">{"Last Activity"}</div>
                </div>
                {
                    if stats.is_empty() {
                        html! {
                            <div class="no-stats">
                                {"No waiting check activity found"}
                            </div>
                        }
                    } else {
                        stats.iter().map(|stat| {
                            html! {
                                <div class="stats-row">
                                    <div class="stats-cell">
                                        <strong>{stat.user_id}</strong>
                                    </div>
                                    <div class="stats-cell">
                                        {stat.total_waiting_messages}
                                    </div>
                                    <div class="stats-cell">
                                        {stat.days_active}
                                    </div>
                                    <div class="stats-cell">
                                        <strong>{format!("{:.2}", stat.average_per_day)}</strong>
                                    </div>
                                    <div class="stats-cell">
                                        {
                                            if let Some(dt) = Utc.timestamp_opt(stat.first_activity as i64, 0).single() {
                                                dt.format("%Y-%m-%d").to_string()
                                            } else {
                                                "Invalid".to_string()
                                            }
                                        }
                                    </div>
                                    <div class="stats-cell">
                                        {
                                            if let Some(dt) = Utc.timestamp_opt(stat.last_activity as i64, 0).single() {
                                                dt.format("%Y-%m-%d").to_string()
                                            } else {
                                                "Invalid".to_string()
                                            }
                                        }
                                    </div>
                                </div>
                            }
                        }).collect::<Html>()
                    }
                }
            </div>
        </div>
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UserCriticalStats {
    pub user_id: i32,
    pub total_critical_messages: usize,
    pub days_active: usize,
    pub average_per_day: f64,
    pub first_activity: i32,
    pub last_activity: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UserCalendarStats {
    pub user_id: i32,
    pub total_calendar_messages: usize,
    pub days_active: usize,
    pub average_per_day: f64,
    pub first_activity: i32,
    pub last_activity: i32,
}

#[derive(Properties, PartialEq)]
pub struct CriticalMessageStatsProps {
    pub usage_logs: Vec<UsageLog>,
}

#[derive(Properties, PartialEq)]
pub struct CalendarMessageStatsProps {
    pub usage_logs: Vec<UsageLog>,
}

fn calculate_critical_stats(usage_logs: &[UsageLog]) -> Vec<UserCriticalStats> {
    let mut user_stats: HashMap<i32, (Vec<i32>, usize)> = HashMap::new();
    
    // Filter critical messages and group by user
    for log in usage_logs.iter() {
        if log.activity_type.contains("critical") {
            let entry = user_stats.entry(log.user_id).or_insert((Vec::new(), 0));
            entry.0.push(log.timestamp);
            entry.1 += 1;
        }
    }
    
    let mut stats = Vec::new();
    
    for (user_id, (timestamps, total_count)) in user_stats {
        if timestamps.is_empty() {
            continue;
        }
        
        let mut sorted_timestamps = timestamps;
        sorted_timestamps.sort();
        
        let first_activity = *sorted_timestamps.first().unwrap();
        let last_activity = *sorted_timestamps.last().unwrap();
        
        // Calculate unique days
        let mut unique_days = std::collections::HashSet::new();
        for &timestamp in &sorted_timestamps {
            let dt = Utc.timestamp_opt(timestamp as i64, 0).single().unwrap();
            let day_key = dt.format("%Y-%m-%d").to_string();
            unique_days.insert(day_key);
        }
        
        let days_active = unique_days.len();
        let average_per_day = if days_active > 0 {
            total_count as f64 / days_active as f64
        } else {
            0.0
        };
        
        stats.push(UserCriticalStats {
            user_id,
            total_critical_messages: total_count,
            days_active,
            average_per_day,
            first_activity,
            last_activity,
        });
    }
    
    // Sort by average per day (descending)
    stats.sort_by(|a, b| b.average_per_day.partial_cmp(&a.average_per_day).unwrap());
    
    stats
}

fn calculate_calendar_stats(usage_logs: &[UsageLog]) -> Vec<UserCalendarStats> {
    let mut user_stats: HashMap<i32, (Vec<i32>, usize)> = HashMap::new();
    
    // Filter calendar messages and group by user
    for log in usage_logs.iter() {
        if log.activity_type.contains("calendar") {
            let entry = user_stats.entry(log.user_id).or_insert((Vec::new(), 0));
            entry.0.push(log.timestamp);
            entry.1 += 1;
        }
    }
    
    let mut stats = Vec::new();
    
    for (user_id, (timestamps, total_count)) in user_stats {
        if timestamps.is_empty() {
            continue;
        }
        
        let mut sorted_timestamps = timestamps;
        sorted_timestamps.sort();
        
        let first_activity = *sorted_timestamps.first().unwrap();
        let last_activity = *sorted_timestamps.last().unwrap();
        
        // Calculate unique days
        let mut unique_days = std::collections::HashSet::new();
        for &timestamp in &sorted_timestamps {
            let dt = Utc.timestamp_opt(timestamp as i64, 0).single().unwrap();
            let day_key = dt.format("%Y-%m-%d").to_string();
            unique_days.insert(day_key);
        }
        
        let days_active = unique_days.len();
        let average_per_day = if days_active > 0 {
            total_count as f64 / days_active as f64
        } else {
            0.0
        };
        
        stats.push(UserCalendarStats {
            user_id,
            total_calendar_messages: total_count,
            days_active,
            average_per_day,
            first_activity,
            last_activity,
        });
    }
    
    // Sort by average per day (descending)
    stats.sort_by(|a, b| b.average_per_day.partial_cmp(&a.average_per_day).unwrap());
    
    stats
}

#[function_component(CriticalMessageStats)]
pub fn critical_message_stats(props: &CriticalMessageStatsProps) -> Html {
    let stats = calculate_critical_stats(&props.usage_logs);
    
    html! {
        <div class="critical-stats-section">
            <h3>{"Critical Message Statistics by User"}</h3>
            <div class="stats-table">
                <div class="stats-header">
                    <div class="stats-cell">{"User ID"}</div>
                    <div class="stats-cell">{"Total Critical Messages"}</div>
                    <div class="stats-cell">{"Days Active"}</div>
                    <div class="stats-cell">{"Average per Day"}</div>
                    <div class="stats-cell">{"First Activity"}</div>
                    <div class="stats-cell">{"Last Activity"}</div>
                </div>
                {
                    if stats.is_empty() {
                        html! {
                            <div class="no-stats">
                                {"No critical message activity found"}
                            </div>
                        }
                    } else {
                        stats.iter().map(|stat| {
                            html! {
                                <div class="stats-row">
                                    <div class="stats-cell">
                                        <strong>{stat.user_id}</strong>
                                    </div>
                                    <div class="stats-cell">
                                        {stat.total_critical_messages}
                                    </div>
                                    <div class="stats-cell">
                                        {stat.days_active}
                                    </div>
                                    <div class="stats-cell">
                                        <strong>{format!("{:.2}", stat.average_per_day)}</strong>
                                    </div>
                                    <div class="stats-cell">
                                        {
                                            if let Some(dt) = Utc.timestamp_opt(stat.first_activity as i64, 0).single() {
                                                dt.format("%Y-%m-%d").to_string()
                                            } else {
                                                "Invalid".to_string()
                                            }
                                        }
                                    </div>
                                    <div class="stats-cell">
                                        {
                                            if let Some(dt) = Utc.timestamp_opt(stat.last_activity as i64, 0).single() {
                                                dt.format("%Y-%m-%d").to_string()
                                            } else {
                                                "Invalid".to_string()
                                            }
                                        }
                                    </div>
                                </div>
                            }
                        }).collect::<Html>()
                    }
                }
            </div>
        </div>
    }
}

#[function_component(CalendarMessageStats)]
pub fn calendar_message_stats(props: &CalendarMessageStatsProps) -> Html {
    let stats = calculate_calendar_stats(&props.usage_logs);
    
    html! {
        <div class="calendar-stats-section">
            <h3>{"Calendar Message Statistics by User"}</h3>
            <div class="stats-table">
                <div class="stats-header">
                    <div class="stats-cell">{"User ID"}</div>
                    <div class="stats-cell">{"Total Calendar Messages"}</div>
                    <div class="stats-cell">{"Days Active"}</div>
                    <div class="stats-cell">{"Average per Day"}</div>
                    <div class="stats-cell">{"First Activity"}</div>
                    <div class="stats-cell">{"Last Activity"}</div>
                </div>
                {
                    if stats.is_empty() {
                        html! {
                            <div class="no-stats">
                                {"No calendar message activity found"}
                            </div>
                        }
                    } else {
                        stats.iter().map(|stat| {
                            html! {
                                <div class="stats-row">
                                    <div class="stats-cell">
                                        <strong>{stat.user_id}</strong>
                                    </div>
                                    <div class="stats-cell">
                                        {stat.total_calendar_messages}
                                    </div>
                                    <div class="stats-cell">
                                        {stat.days_active}
                                    </div>
                                    <div class="stats-cell">
                                        <strong>{format!("{:.2}", stat.average_per_day)}</strong>
                                    </div>
                                    <div class="stats-cell">
                                        {
                                            if let Some(dt) = Utc.timestamp_opt(stat.first_activity as i64, 0).single() {
                                                dt.format("%Y-%m-%d").to_string()
                                            } else {
                                                "Invalid".to_string()
                                            }
                                        }
                                    </div>
                                    <div class="stats-cell">
                                        {
                                            if let Some(dt) = Utc.timestamp_opt(stat.last_activity as i64, 0).single() {
                                                dt.format("%Y-%m-%d").to_string()
                                            } else {
                                                "Invalid".to_string()
                                            }
                                        }
                                    </div>
                                </div>
                            }
                        }).collect::<Html>()
                    }
                }
            </div>
        </div>
    }
}

