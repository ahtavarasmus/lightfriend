use yew::prelude::*;
use chrono::{Utc, TimeZone};
use crate::profile::billing_models::format_timestamp;
use serde::{Deserialize, Serialize};

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

