use crate::AppState;
use std::sync::Arc;
use serde::Deserialize;
use axum::Json;
use chrono::{DateTime, FixedOffset, Local, NaiveDate};
use chrono_tz;
use serde_json::Value;

pub fn get_fetch_calendar_event_tool() -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;

    let mut calendar_properties = HashMap::new();
    calendar_properties.insert(
        "start".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Start time in RFC3339 format (e.g., '2024-03-16T00:00:00Z')".to_string()),
            ..Default::default()
        }),
    );
    calendar_properties.insert(
        "end".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("End time in RFC3339 format (e.g., '2024-03-16T00:00:00Z')".to_string()),
            ..Default::default()
        }),
    );

    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("fetch_calendar_events"),
            description: Some(String::from(
                "Fetches the user's calendar events for the specified time frame, or defaults to today if no time frame is provided. If the current time is after 6 PM in the user's timezone (assumed PDT unless specified), also include tomorrow's events when no time frame is given. CRITICAL: Return the tool's output EXACTLY as received, with NO additional text, NO numbering, NO markdown formatting (no **, -, etc.), NO HTML, and NO modifications. The tool formats events as 'summary: HH:MM AM/PM - HH:MM AM/PM date' for timed events or 'summary: All day, date' for all-day events, joined by '|'. Example output: 'Meeting: 1:00 PM - 2:00 PM today|Vacation: All day, Jun 16 - Jun 17'. DO NOT add 'Today's events:', numbering, bullet points, or any other text or formatting. Pass through the raw event list exactly as provided. Never use markdown formatting like **, -, or other special characters for formatting."
            )),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(calendar_properties),
                required: Some(vec![String::from("start"), String::from("end")]),
            },
        },
    }
}

pub fn get_create_calendar_event_tool() -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;
// Add calendar event properties
    let mut calendar_event_properties = HashMap::new();
    calendar_event_properties.insert(
        "summary".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The title/summary of the calendar event".to_string()),
            ..Default::default()
        }),
    );
    calendar_event_properties.insert(
        "start_time".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Start time in RFC3339 format in UTC (e.g., '2024-03-23T14:30:00Z')".to_string()),
            ..Default::default()
        }),
    );
    calendar_event_properties.insert(
        "duration_minutes".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Duration of the event in minutes".to_string()),
            ..Default::default()
        }),
    );
    calendar_event_properties.insert(
        "description".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Optional description of the event. Do not add unless user asks specifically.".to_string()),
            ..Default::default()
        }),
    );
    calendar_event_properties.insert(
        "add_notification".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Boolean),
            description: Some("Whether to add a notification reminder (defaults to true unless specified)".to_string()),
            ..Default::default()
        }),
    );


    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("create_calendar_event"),
            description: Some(String::from("Creates a new Google Calendar event. Invoke this tool immediately without extra confirmation if the user has explicitly provided the required parameters (summary, start_time, and duration_minutes). If any required parameters are missing or unclear, ask the user for clarification in a follow-up response, then call the tool once the information is obtained. Only include optional parameters like description or add_notification if the user specifies them.")),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(calendar_event_properties),
                required: Some(vec![String::from("summary"), String::from("start_time"), String::from("duration_minutes")]),
            },
        },
    }
}

#[derive(Deserialize)]
pub struct CalendarTimeFrame {
    pub start: String,
    pub end: String,
}


pub async fn handle_fetch_calendar_events(
    state: &Arc<AppState>,
    user_id: i32,
    args: &str,
) -> String {
    let c: CalendarTimeFrame = match serde_json::from_str(args) {
        Ok(q) => q,
        Err(e) => {
            eprintln!("Failed to parse calendar time frame: {}", e);
            return "Failed to parse calendar request.".to_string();
        }
    };

    match crate::handlers::google_calendar::handle_calendar_fetching(&state, user_id, &c.start, &c.end).await {
        Ok(Json(response)) => {
            if let Some(events) = response.get("events") {
                let empty_vec = Vec::new();
                let events_array = events.as_array().unwrap_or(&empty_vec);
                let mut formatted_events = Vec::new();
                let today = Local::today().with_timezone(&FixedOffset::west_opt(7 * 3600).unwrap()).naive_local();

                // Collect events with their start times for sorting
                let mut events_with_time: Vec<(Value, Option<DateTime<FixedOffset>>)> = events_array.iter().map(|event| {
                    let start_str = event.get("start").and_then(Value::as_str).unwrap_or("");
                    let start_time = DateTime::parse_from_rfc3339(start_str).ok();
                    (event.clone(), start_time)
                }).collect();

                // Sort by start time, placing events without times (all-day) at the end
                events_with_time.sort_by(|a, b| {
                    match (a.1, b.1) {
                        (Some(t1), Some(t2)) => t1.cmp(&t2),
                        (Some(_), None) => std::cmp::Ordering::Less,
                        (None, Some(_)) => std::cmp::Ordering::Greater,
                        (None, None) => std::cmp::Ordering::Equal,
                    }
                });

                for (event, _) in events_with_time.iter() {
                    let summary = event.get("summary").and_then(|s| s.as_str()).unwrap_or("Untitled");
                    let duration = event.get("duration_minutes").and_then(|d| d.as_i64()).unwrap_or(0);
                    let start_str = event.get("start").and_then(|s| s.as_str()).unwrap_or("");
                    let end_str = event.get("end").and_then(|s| s.as_str()).unwrap_or("");

                    let formatted_event = if duration == 0 || start_str.len() == 10 { // All-day event
                        let start_date = NaiveDate::parse_from_str(start_str, "%Y-%m-%d")
                            .or_else(|_| NaiveDate::parse_from_str(end_str, "%Y-%m-%d"))
                            .unwrap_or(today);
                        let end_date = NaiveDate::parse_from_str(end_str, "%Y-%m-%d")
                            .or_else(|_| NaiveDate::parse_from_str(start_str, "%Y-%m-%d"))
                            .unwrap_or(start_date);
                        
                        if start_date == end_date {
                            format!("{}: All day, {}", summary, start_date.format("%b %d"))
                        } else {
                            format!(
                                "{}: All day, {} - {}",
                                summary, start_date.format("%b %d"), end_date.format("%b %d")
                            )
                        }
                    } else { // Timed event
                        match (DateTime::parse_from_rfc3339(start_str),
                              DateTime::parse_from_rfc3339(end_str)) {
                            (Ok(start_dt), Ok(end_dt)) => {
                                let date_str = if start_dt.date_naive() == today {
                                    "today".to_string()
                                } else {
                                    start_dt.format("%b %d").to_string()
                                };
                                format!(
                                    "{}: {} - {} {}",
                                    summary,
                                    start_dt.format("%l:%M %p"),
                                    end_dt.format("%l:%M %p"),
                                    date_str
                                )
                            },
                            _ => continue, // Skip invalid timed events
                        }
                    };

                    formatted_events.push(formatted_event);
                }

                if formatted_events.is_empty() {
                    "No events scheduled.".to_string()
                } else {
                    formatted_events.join("|")
                }
            } else {
                "No events scheduled.".to_string()
            }
        }
        Err((status, _)) => {
            match status {
                axum::http::StatusCode::BAD_REQUEST => "No active Google Calendar connection found. Visit the website to connect.".to_string(),
                axum::http::StatusCode::UNAUTHORIZED => "Your calendar connection needs to be renewed. Please reconnect on the website.".to_string(),
                _ => "Failed to fetch calendar events. Please try again later.".to_string(),
            }
        }
    }
}


#[derive(Deserialize)]
struct CalendarEventArgs {
    summary: String,
    start_time: String,
    duration_minutes: String,  // Changed to String to handle quoted numbers
    description: Option<String>,
    add_notification: Option<bool>,
}

pub async fn handle_create_calendar_event(
    state: &Arc<AppState>,
    user_id: i32,
    args: &str,
    user: &crate::models::user_models::User,
) -> Result<(axum::http::StatusCode, [(axum::http::HeaderName, &'static str); 1], axum::Json<crate::api::twilio_sms::TwilioResponse>), Box<dyn std::error::Error>> {
    let mut args: CalendarEventArgs = serde_json::from_str(args)?;
    
    // Parse duration_minutes from String to i32
    let duration_minutes: i32 = args.duration_minutes.parse().map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
    args.duration_minutes = duration_minutes.to_string();  // Optional: store back as string if needed, but not necessary
    
    let user_info = state.user_core.get_user_info(user_id)?;
    let timezone = user_info.timezone.unwrap_or_else(|| String::from("UTC"));
   
    // Parse the start time
    let start_time = chrono::DateTime::parse_from_rfc3339(&args.start_time)
        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
   
    // Convert to user's timezone
    let user_tz: chrono_tz::Tz = timezone.parse()
        .unwrap_or(chrono_tz::UTC);
    let local_time = start_time.with_timezone(&user_tz);
   
    // Format the date and time
    let formatted_time = local_time.format("%B %d at %I:%M %p %Z").to_string();
    // Create the event directly
    let event_request = crate::handlers::google_calendar::CreateEventRequest {
        start_time: start_time.with_timezone(&chrono::Utc),
        duration_minutes,
        summary: args.summary.clone(),
        description: args.description.clone(),
        add_notification: args.add_notification.unwrap_or(true),
    };
    match crate::handlers::google_calendar::create_calendar_event(
        axum::extract::State(state.clone()),
        crate::handlers::auth_middleware::AuthUser { user_id, is_admin: false },
        axum::Json(event_request)
    ).await {
        Ok(_) => {
            let success_msg = format!("Calendar event '{}' created for {}", args.summary, formatted_time);
            if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                &state,
                &success_msg,
                None,
                user,
            ).await {
                eprintln!("Failed to send success message: {}", e);
            }
            return Ok((
                axum::http::StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                axum::Json(crate::api::twilio_sms::TwilioResponse {
                    message: success_msg,
                })
            ));
        }
        Err((status, error_json)) => {
            let error_msg = format!("Failed to create calendar event: {}", error_json.0.get("error").and_then(|v| v.as_str()).unwrap_or("Unknown error"));
            if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                &state,
                &error_msg,
                None,
                user,
            ).await {
                eprintln!("Failed to send error message: {}", e);
            }
            return Ok((
                axum::http::StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                axum::Json(crate::api::twilio_sms::TwilioResponse {
                    message: error_msg,
                })
            ));
        }
    }
}

