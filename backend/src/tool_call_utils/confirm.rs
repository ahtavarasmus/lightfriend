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
}

pub async fn handle_confirmation(
    state: &Arc<AppState>,
    user: &User,
    event_type: &str,
    user_message: &str,
    is_test: bool,
) -> ConfirmationResult {
    // Default values
    let mut should_continue = false;
    let mut response = None;

    let user_response = user_message.trim().to_lowercase();

    if event_type == "calendar" {
        // Handle calendar event confirmation
        // Get the calendar event details from temp variables
        let (summary, start_time, duration, description) = match state.user_core.get_temp_variable(user.id, "calendar") {
            Ok(Some((_, summary, description, start_time, duration_str, _, _))) => (
                summary.unwrap_or_default(),
                start_time.unwrap_or_default(),
                duration_str.unwrap_or_default().parse::<i32>().unwrap_or(30),
                description
            ),
            _ => {
                response = Some((
                    StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    Json(TwilioResponse {
                        message: "Failed to create calendar event due to internal error.".to_string(),
                    })
                ));

                // Clear the confirmation state
                if let Err(e) = state.user_core.clear_confirm_send_event(user.id) {
                    tracing::error!("Failed to clear confirmation state: {}", e);
                }
                return ConfirmationResult {
                    should_continue,
                    response,
                };
            }
        };
        match user_response.as_str() {
            "yes" => {

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
                            // Clear the confirmation state
                            if let Err(e) = state.user_core.clear_confirm_send_event(user.id) {
                                tracing::error!("Failed to clear confirmation state: {}", e);
                            }
                            return ConfirmationResult {
                                should_continue,
                                response,
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
                        if !is_test {
                            if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                                &state,
                                confirmation_msg,
                                None,
                                user,
                            ).await {
                                tracing::error!("Failed to send confirmation message: {}", e);
                            }
                        }

                        response = Some((
                            StatusCode::OK,
                            [(axum::http::header::CONTENT_TYPE, "application/json")],
                            Json(TwilioResponse {
                                message: confirmation_msg.to_string(),
                            })
                        ));
                    }
                    Err((status, Json(error))) => {
                        let error_msg = format!("Failed to create calendar event: {} (not charged)", error["error"].as_str().unwrap_or("Unknown error"));
                        if !is_test {
                            if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                                &state,
                                &error_msg,
                                None,
                                user,
                            ).await {
                                tracing::error!("Failed to send error message: {}", e);
                            }
                        }
                        response = Some((
                            StatusCode::OK,
                            [(axum::http::header::CONTENT_TYPE, "application/json")],
                            Json(TwilioResponse {
                                message: error_msg,
                            })
                        ));
                    }
                }
            }
            _ => {
                should_continue = true;
            }
        }
    } else if event_type == "telegram" || event_type == "whatsapp" || event_type == "signal" {
        // Get the message details from temp variables
        let details = match state.user_core.get_temp_variable(user.id, event_type) {
            Ok(Some((recipient, _, message_content, _, _, _, image_url))) => {
                (recipient.unwrap_or_default(), message_content.unwrap_or_default(), image_url.unwrap_or("".to_string()))
            },
            _ => {
                response = Some((
                    StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    Json(TwilioResponse {
                        message: format!("Failed to send {} message due to internal error.", event_type),
                    })
                ));
                // Clear the confirmation state
                if let Err(e) = state.user_core.clear_confirm_send_event(user.id) {
                    tracing::error!("Failed to clear confirmation state: {}", e);
                }
                return ConfirmationResult {
                    should_continue,
                    response,
                };
            }
        };

        let (recipient, message_content, image_url) = details;
        let image_url_option = if !image_url.is_empty() {
            Some(image_url)
        } else {
            None
        };

        match user_response.as_str() {
            "yes" => {
                // Send the message
                match crate::utils::bridge::send_bridge_message(
                    event_type,
                    state,
                    user.id,
                    &recipient,
                    &message_content,
                    image_url_option,
                ).await {
                    Ok(_) => {
                        // Send confirmation via Twilio
                        let confirmation_msg = format!("Message sent successfully to {}", recipient);
                        if !is_test {
                            if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                                &state,
                                &confirmation_msg,
                                None,
                                user,
                            ).await {
                                tracing::error!("Failed to send confirmation message: {}", e);
                            }
                        }

                        response = Some((
                            StatusCode::OK,
                            [(axum::http::header::CONTENT_TYPE, "application/json")],
                            Json(TwilioResponse {
                                message: confirmation_msg,
                            })
                        ));
                    }
                    Err(e) => {
                        tracing::debug!("sending failed to send the message");
                        let error_msg = format!("Failed to send message: {}", e);
                        if !is_test {
                            if let Err(send_err) = crate::api::twilio_utils::send_conversation_message(
                                &state,
                                &error_msg,
                                None,
                                user,
                            ).await {
                                tracing::error!("Failed to send error message: {}", send_err);
                            }
                        }

                        response = Some((
                            StatusCode::OK,
                            [(axum::http::header::CONTENT_TYPE, "application/json")],
                            Json(TwilioResponse {
                                message: error_msg,
                            })
                        ));
                    }
                }
            }
            _ => {
                should_continue = true;
            }
        }
    } else if event_type == "email" {
        // Get the email response details from temp variables
        let email_details = match state.user_core.get_temp_variable(user.id, "email") {
            Ok(Some((recipient, subject, response_text, _, _, email_id, _))) => {
                (
                    recipient.unwrap_or_default(),
                    subject.unwrap_or_default(),
                    response_text.unwrap_or_default(),
                    email_id.unwrap_or_default()
                )
            },
            _ => {
                response = Some((
                    StatusCode::OK,
                    [(axum::http::header::CONTENT_TYPE, "application/json")],
                    Json(TwilioResponse {
                        message: "Failed to send email response due to internal error.".to_string(),
                    })
                ));

                // Clear the confirmation state
                if let Err(e) = state.user_core.clear_confirm_send_event(user.id) {
                    tracing::error!("Failed to clear confirmation state: {}", e);
                }
                return ConfirmationResult {
                    should_continue,
                    response,
                };
            }
        };

        let (recipient, subject, response_text, email_id) = email_details;

        match user_response.as_str() {
            "yes" => {

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
                        if !is_test {
                            if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                                &state,
                                &confirmation_msg,
                                None,
                                user,
                            ).await {
                                tracing::error!("Failed to send confirmation message: {}", e);
                            }
                        }

                        response = Some((
                            StatusCode::OK,
                            [(axum::http::header::CONTENT_TYPE, "application/json")],
                            Json(TwilioResponse {
                                message: confirmation_msg,
                            })
                        ));
                    }
                    Err((status, Json(error))) => {
                        let error_msg = format!("Failed to send email response: {} (not charged)", error["error"].as_str().unwrap_or("Unknown error"));
                        if !is_test {
                            if let Err(e) = crate::api::twilio_utils::send_conversation_message(
                                &state,
                                &error_msg,
                                None,
                                user,
                            ).await {
                                tracing::error!("Failed to send error message: {}", e);
                            }
                        }
                        response = Some((
                            StatusCode::OK,
                            [(axum::http::header::CONTENT_TYPE, "application/json")],
                            Json(TwilioResponse {
                                message: error_msg,
                            })
                        ));
                    }
                }
            }
            _ => {
                should_continue = true;
            }
        }
    }

    // Clear the confirmation state
    if let Err(e) = state.user_core.clear_confirm_send_event(user.id) {
        tracing::error!("Failed to clear confirmation state: {}", e);
    }

    ConfirmationResult {
        should_continue,
        response,
    }
}
