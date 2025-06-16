use crate::AppState;
use crate::models::user_models::User;
use axum::http::StatusCode;
use axum::Json;
use std::sync::Arc;
use regex::Regex;
use crate::api::twilio_sms::TwilioResponse;
use crate::api::twilio_sms::TwilioMessageResponse;

pub struct ConfirmationResult {
    pub should_continue: bool,
    pub response: Option<(StatusCode, [(axum::http::HeaderName, &'static str); 1], Json<TwilioResponse>)>,
    pub redact_body: bool,
}

pub async fn handle_confirmation(
    state: &Arc<AppState>,
    user: &User,
    conversation_sid: &str,
    twilio_number: &String,
    user_message: &str,
    last_ai_message: Option<&TwilioMessageResponse>,
) -> ConfirmationResult {
    // Default values
    let mut should_continue = true;
    let mut redact_body = true;
    let mut response = None;

    if !user.confirm_send_event {
        return ConfirmationResult {
            should_continue,
            response,
            redact_body,
        };
    }

    let last_ai_message = match last_ai_message {
        Some(msg) => msg,
        None => return ConfirmationResult {
            should_continue,
            response,
            redact_body,
        },
    };

    let user_response = user_message.trim().to_lowercase();

    // Handle calendar event confirmation
    if let Some(captures) = Regex::new(r"Confirm creating calendar event: '([^']+)' starting at '([^']+)' for (\d+) minutes(\s*with description: '([^']+)')?.*")
        .ok()
        .and_then(|re| re.captures(&last_ai_message.body)) {
        
        let summary = captures.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
        let start_time = captures.get(2).map(|m| m.as_str().to_string()).unwrap_or_default();
        let duration = captures.get(3).and_then(|m| m.as_str().parse::<i32>().ok()).unwrap_or_default();
        let description = captures.get(5).map(|m| m.as_str().to_string());

        // Redact the confirmation message
        if let Err(e) = crate::api::twilio_utils::redact_message(
            conversation_sid,
            &last_ai_message.sid,
            "Calendar event confirmation message redacted",
            user,
        ).await {
            tracing::error!("Failed to redact calendar confirmation message: {}", e);
        }

        match user_response.as_str() {
            "yes" => {
                // Reset the confirmation flag
                if let Err(e) = state.user_repository.set_confirm_send_event(user.id, false) {
                    tracing::error!("Failed to reset confirm_send_event flag: {}", e);
                }

                // Create the calendar event
                let event_request = crate::handlers::google_calendar::CreateEventRequest {
                    start_time: match chrono::DateTime::parse_from_rfc3339(&start_time) {
                        Ok(dt) => dt.with_timezone(&chrono::Utc),
                        Err(e) => {
                            tracing::error!("Failed to parse start time: {}", e);
                            response = Some((
                                StatusCode::OK,
                                [(axum::http::header::CONTENT_TYPE, "application/json")],
                                Json(TwilioResponse {
                                    message: "Failed to create calendar event due to invalid start time.".to_string(),
                                })
                            ));
                            should_continue = false;
                            return ConfirmationResult {
                                should_continue,
                                response,
                                redact_body,
                            };
                        }
                    },
                    duration_minutes: duration,
                    summary,
                    description,
                    add_notification: true,
                };

                let auth_user = crate::handlers::auth_middleware::AuthUser {
                    user_id: user.id,
                    is_admin: false,
                };

                match crate::handlers::google_calendar::create_calendar_event(
                    axum::extract::State(state.clone()),
                    auth_user,
                    Json(event_request),
                ).await {
                    Ok(_) => {
                        let confirmation_msg = "Calendar event created successfully!";
                        if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                            conversation_sid,
                            twilio_number,
                            confirmation_msg,
                            true,
                            user,
                        ).await {
                            tracing::error!("Failed to send confirmation message: {}", e);
                        }

                        response = Some((
                            StatusCode::OK,
                            [(axum::http::header::CONTENT_TYPE, "application/json")],
                            Json(TwilioResponse {
                                message: confirmation_msg.to_string(),
                            })
                        ));
                        should_continue = false;
                    }
                    Err((status, Json(error))) => {
                        let error_msg = format!("Failed to create calendar event: {} (not charged)", error["error"].as_str().unwrap_or("Unknown error"));
                        if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                            conversation_sid,
                            twilio_number,
                            &error_msg,
                            true,
                            user,
                        ).await {
                            tracing::error!("Failed to send error message: {}", e);
                        }
                        response = Some((
                            StatusCode::OK,
                            [(axum::http::header::CONTENT_TYPE, "application/json")],
                            Json(TwilioResponse {
                                message: error_msg,
                            })
                        ));
                        should_continue = false;
                    }
                }
            }
            "no" => {
                // Reset the confirmation flag
                if let Err(e) = state.user_repository.set_confirm_send_event(user.id, false) {
                    tracing::error!("Failed to reset confirm_send_event flag: {}", e);
                }

                let cancel_msg = "Calendar event creation cancelled.";
                if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                    conversation_sid,
                    twilio_number,
                    cancel_msg,
                    true,
                    user,
                ).await {
                    tracing::error!("Failed to send cancellation confirmation: {}", e);
                }

                response = Some((
                    StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    Json(TwilioResponse {
                        message: cancel_msg.to_string(),
                    })
                ));
                should_continue = false;
            }
            _ => {
                // Reset the confirmation flag since we're treating this as a new message
                if let Err(e) = state.user_repository.set_confirm_send_event(user.id, false) {
                    tracing::error!("Failed to reset confirm_send_event flag: {}", e);
                }
            }
        }
    }

    // Handle email response confirmation
    if let Some(captures) = Regex::new(r"Confirm sending email response to '([^']+)' regarding '([^']+)' with content: '([^']+)' id:\((\d+)\)")
        .ok()
        .and_then(|re| re.captures(&last_ai_message.body)) {

        let recipient = captures.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
        let subject = captures.get(2).map(|m| m.as_str().to_string()).unwrap_or_default();
        let response_text = captures.get(3).map(|m| m.as_str().to_string()).unwrap_or_default();
        let email_id = captures.get(4).map(|m| m.as_str().to_string()).unwrap_or_default();

        // Redact the confirmation message
        if let Err(e) = crate::api::twilio_utils::redact_message(
            conversation_sid,
            &last_ai_message.sid,
            &format!("Confirm sending email response to '[RECIPIENT_REDACTED]' regarding '[SUBJECT_REDACTED]' with content: '[CONTENT_REDACTED]' id:[ID_REDACTED]"),
            user,
        ).await {
            tracing::error!("Failed to redact email confirmation message: {}", e);
        }

        match user_response.as_str() {
            "yes" => {
                // Reset the confirmation flag
                if let Err(e) = state.user_repository.set_confirm_send_event(user.id, false) {
                    tracing::error!("Failed to reset confirm_send_event flag: {}", e);
                }

                let email_request = crate::handlers::imap_handlers::EmailResponseRequest {
                    email_id: email_id.clone(),
                    response_text: response_text.clone(),
                };

                let auth_user = crate::handlers::auth_middleware::AuthUser {
                    user_id: user.id,
                    is_admin: false,
                };

                match crate::handlers::imap_handlers::respond_to_email(
                    axum::extract::State(state.clone()),
                    auth_user,
                    Json(email_request),
                ).await {
                    Ok(_) => {
                        let confirmation_msg = format!("Email response sent successfully to {} regarding '{}'", recipient, subject);
                        if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                            conversation_sid,
                            twilio_number,
                            &confirmation_msg,
                            true,
                            user,
                        ).await {
                            tracing::error!("Failed to send confirmation message: {}", e);
                        }

                        response = Some((
                            StatusCode::OK,
                            [(axum::http::header::CONTENT_TYPE, "application/json")],
                            Json(TwilioResponse {
                                message: confirmation_msg,
                            })
                        ));
                        should_continue = false;
                    }
                    Err((status, Json(error))) => {
                        let error_msg = format!("Failed to send email response: {} (not charged)", error["error"].as_str().unwrap_or("Unknown error"));
                        if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                            conversation_sid,
                            twilio_number,
                            &error_msg,
                            true,
                            user,
                        ).await {
                            tracing::error!("Failed to send error message: {}", e);
                        }
                        response = Some((
                            StatusCode::OK,
                            [(axum::http::header::CONTENT_TYPE, "application/json")],
                            Json(TwilioResponse {
                                message: error_msg,
                            })
                        ));
                        should_continue = false;
                    }
                }
            }
            "no" => {
                // Reset the confirmation flag
                if let Err(e) = state.user_repository.set_confirm_send_event(user.id, false) {
                    tracing::error!("Failed to reset confirm_send_event flag: {}", e);
                }

                let cancel_msg = "Email response cancelled.";
                if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                    conversation_sid,
                    twilio_number,
                    cancel_msg,
                    true,
                    user,
                ).await {
                    tracing::error!("Failed to send cancellation confirmation: {}", e);
                }

                response = Some((
                    StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    Json(TwilioResponse {
                        message: cancel_msg.to_string(),
                    })
                ));
                should_continue = false;
            }
            _ => {
                // Reset the confirmation flag since we're treating this as a new message
                if let Err(e) = state.user_repository.set_confirm_send_event(user.id, false) {
                    tracing::error!("Failed to reset confirm_send_event flag: {}", e);
                }
            }
        }
    }

    redact_body = should_continue;

    ConfirmationResult {
        should_continue,
        response,
        redact_body,
    }
}

