use crate::AppState;
use crate::utils::country::{is_local_number_country, is_notification_only_country, get_country_code_from_phone};
use std::sync::Arc;

/// Helper to check if a phone number is US/CA
fn is_us_or_ca(phone: &str) -> bool {
    phone.starts_with("+1")
}

/// Checks if a user has sufficient credits to perform an action.
///
/// Credits interpretation differs by region:
/// - US/CA: credits_left is message COUNT (1 = 1 message)
/// - Euro countries: credits_left is EURO VALUE (deduct actual cost)
///
/// Returns Ok(()) if the user has enough credits, or Err with an appropriate error message if not.
/// Also handles automatic recharging if enabled.
pub async fn check_user_credits(
    state: &Arc<AppState>,
    user: &crate::models::user_models::User,
    event_type: &str,
    amount: Option<i32>,
) -> Result<(), String> {

    // Check if phone service is deactivated (e.g., stolen phone scenario)
    if let Ok(false) = state.user_core.get_phone_service_active(user.id) {
        return Err("Phone service is currently deactivated for this number.".to_string());
    }

    // BYOT users with their own Twilio credentials pay Twilio directly - no credit check
    if state.user_core.has_twilio_credentials(user.id) {
        return Ok(());
    }

    // Check if the event type is free based on country
    // Local number countries and notification-only countries are charged through Lightfriend
    // Other countries pay Twilio directly (require their own credentials)
    let messages_are_included = !is_local_number_country(&user.phone_number)
        && !is_notification_only_country(&user.phone_number);

    if messages_are_included {
        return Ok(());
    }

    // Get required amounts based on region (dual interpretation)
    let (required_credits_left, required_credits) = if is_us_or_ca(&user.phone_number) {
        // US/CA: credits_left is message COUNT, credits is dollar value
        let credits_left_cost = match event_type {
            "message" => 1.0,
            "voice" => 0.0, // voice uses credits, not credits_left
            "web_call" => 1.0, // 1 message credit per minute for web calls
            "noti_msg" => 0.5,
            "noti_call" => 0.5,
            "digest" => amount.unwrap_or(1) as f32,
            _ => return Err("Invalid event type".to_string()),
        };
        let credits_cost = match event_type {
            "message" => 0.075,
            "voice" => amount.unwrap_or(0) as f32 * 0.0033,
            "web_call" => 0.0, // web_call uses credits_left (message units)
            "noti_msg" => 0.075,
            "noti_call" => 0.15,
            "digest" => amount.unwrap_or(1) as f32 * 0.075,
            _ => return Err("Invalid event type".to_string()),
        };
        (credits_left_cost, credits_cost)
    } else {
        // Euro countries: credits_left is EURO VALUE with segment-based pricing
        let country_code = get_country_code_from_phone(&user.phone_number);
        let pricing = if let Some(code) = country_code {
            crate::api::twilio_pricing::get_notification_only_pricing(state, &code).await.ok()
        } else {
            None
        };

        let (noti_price, msg_price, digest_price, voice_price) = match pricing {
            Some(p) => (p.notification_price, p.regular_message_price, p.digest_price, p.calculated_voice_price),
            None => {
                // Fallback: assume ~€0.10 raw price
                (0.195, 0.39, 0.39, 0.13) // 0.10 × 1.5/3/3 × 1.3
            }
        };

        // ElevenLabs voice AI cost: $0.11 per minute (added to voice calls)
        const ELEVENLABS_COST_PER_MIN: f32 = 0.11;

        // Web call cost: 0.15 EUR per minute (ElevenLabs only, no Twilio)
        const WEB_CALL_COST_PER_MIN: f32 = 0.15;

        // Both credits_left and credits use actual euro cost
        let cost = match event_type {
            "message" => msg_price,
            "voice" => {
                // Voice: first minute always charged, then per additional minute
                // Includes Twilio voice + ElevenLabs AI voice cost
                let minutes = (amount.unwrap_or(60) as f32 / 60.0).ceil().max(1.0);
                minutes * (voice_price + ELEVENLABS_COST_PER_MIN)
            },
            "web_call" => {
                // Web call: 0.15 EUR per minute (no Twilio, just ElevenLabs)
                let minutes = (amount.unwrap_or(60) as f32 / 60.0).ceil().max(1.0);
                minutes * WEB_CALL_COST_PER_MIN
            },
            "noti_msg" => noti_price,
            "noti_call" => voice_price + ELEVENLABS_COST_PER_MIN, // First minute always charged + ElevenLabs
            "digest" => digest_price * amount.unwrap_or(1) as f32,
            _ => return Err("Invalid event type".to_string()),
        };
        (cost, cost) // Same value for both since both are euro-based
    };

    // Check if user has sufficient balance
    if user.credits_left < required_credits_left && user.credits < required_credits {
        // Check if enough time has passed since the last notification (24 hours)
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;

        let should_notify = match user.last_credits_notification {
            None => true,
            Some(last_time) => (current_time - last_time) >= 24 * 3600 // 24 hours in seconds
        };

        if should_notify && event_type != "digest" {
            // Send notification about depleted credits and monthly quota

            // Update the last notification timestamp
            if let Err(e) = state.user_core.update_last_credits_notification(user.id, current_time) {
                eprintln!("Failed to update last_credits_notification: {}", e);
            }

            let user_clone = user.clone();
            let state_clone = state.clone();

            tokio::spawn(async move {
                let _ = crate::api::twilio_utils::send_conversation_message(
                    &state_clone,
                    "Your credits and monthly quota have been depleted. Please recharge your credits to continue using the service.",
                    None,
                    &user_clone,
                ).await;
            });
        }
        return Err("Insufficient credits. You have used all your monthly quota and don't have enough extra credits.".to_string());
    }

    // Check credits threshold and handle automatic charging
    match state.user_repository.is_credits_under_threshold(user.id) {
        Ok(is_under) => {
            if is_under && user.charge_when_under {
                println!("User {} credits is under threshold, attempting automatic charge", user.id);
                use axum::extract::{State, Path};
                let state_clone = Arc::clone(state);
                let user_id = user.id; // Clone the user ID
                tokio::spawn(async move {
                    let _ = crate::handlers::stripe_handlers::automatic_charge(
                        State(state_clone),
                        Path(user_id),
                    ).await;
                });
                println!("Initiated automatic recharge for user");
            }
        },
        Err(e) => eprintln!("Failed to check if user credits is under threshold: {}", e),
    }

    Ok(())
}

/// Deducts credits from a user's account, using monthly credits (credits_left) first before using regular credits.
///
/// Credits interpretation differs by region:
/// - US/CA: credits_left is message COUNT (1 = 1 message)
/// - Euro countries: credits_left is EURO VALUE (deduct actual cost with segment-based pricing)
///
/// Returns Ok(()) if credits were successfully deducted, or Err with an appropriate error message if not.
pub fn deduct_user_credits(
    state: &Arc<AppState>,
    user_id: i32,
    event_type: &str,
    amount: Option<i32>,
) -> Result<(), String> {
    let user = match state.user_core.find_by_id(user_id) {
        Ok(Some(user)) => user,
        Ok(None) => return Err("User not found".to_string()),
        Err(e) => {
            eprintln!("Database error while finding user {}: {}", user_id, e);
            return Err("Database error occurred".to_string());
        }
    };

    // BYOT users with their own Twilio credentials pay Twilio directly - no credit deduction
    if state.user_core.has_twilio_credentials(user_id) {
        return Ok(());
    }

    // For tier 3 self-hosted users, check if they have US/CA unlimited
    let is_tier3 = user.sub_tier.as_deref() == Some("tier 3");
    let user_settings = if is_tier3 {
        match state.user_core.get_user_settings(user_id) {
            Ok(settings) => Some(settings),
            Err(e) => {
                eprintln!("Failed to get user settings for tier 3 user {}: {}", user_id, e);
                None
            }
        }
    } else {
        None
    };

    // Check if the event type is free based on country
    // Local number countries and notification-only countries are charged through Lightfriend
    // Other countries pay Twilio directly (require their own credentials)
    let messages_are_included = !is_local_number_country(&user.phone_number)
        && !is_notification_only_country(&user.phone_number);

    if messages_are_included {
        return Ok(());
    }

    // Calculate deduction amounts based on region (dual interpretation)
    let (credits_left_deduction, credits_deduction) = if is_tier3 {
        // Tier 3: use outbound_message_pricing from user_settings
        let pricing = user_settings.as_ref().and_then(|s| s.outbound_message_pricing).unwrap_or(0.075);
        let cost = match event_type {
            "message" => pricing,
            "voice" => amount.unwrap_or(0) as f32 * 0.005,
            "noti_msg" => pricing,
            "noti_call" => pricing * 2.0,
            "digest" => pricing,
            _ => return Err("Invalid event type".to_string()),
        };
        (0.0, cost) // Tier 3 doesn't use credits_left
    } else if is_us_or_ca(&user.phone_number) {
        // US/CA: credits_left is message COUNT, credits is dollar value
        let credits_left_cost = match event_type {
            "message" => 1.0,
            "voice" => 0.0, // voice uses credits, not credits_left
            "web_call" => {
                // Web call: 1 message credit per minute (round up)
                
                (amount.unwrap_or(60) as f32 / 60.0).ceil().max(1.0)
            },
            "noti_msg" => 0.5,
            "noti_call" => 0.5,
            "digest" => 1.0,
            _ => return Err("Invalid event type".to_string()),
        };
        let credits_cost = match event_type {
            "message" => 0.075,
            "voice" => amount.unwrap_or(0) as f32 * 0.0033,
            "web_call" => 0.0, // web_call uses credits_left (message units)
            "noti_msg" => 0.075,
            "noti_call" => 0.15,
            "digest" => 0.075,
            _ => return Err("Invalid event type".to_string()),
        };
        (credits_left_cost, credits_cost)
    } else {
        // Euro countries: credits_left is EURO VALUE with segment-based pricing
        let country_code = get_country_code_from_phone(&user.phone_number);
        let pricing = country_code.and_then(|code| {
            crate::api::twilio_pricing::get_cached_euro_pricing_sync(state, &code)
        });

        let (noti_price, msg_price, digest_price, voice_price) = match pricing {
            Some(p) => (p.notification_price, p.regular_message_price, p.digest_price, p.calculated_voice_price),
            None => {
                tracing::warn!("No cached pricing for euro country, using fallback");
                // Fallback: assume ~€0.10 raw price
                (0.195, 0.39, 0.39, 0.13) // 0.10 × 1.5/3/3 × 1.3
            }
        };

        // ElevenLabs voice AI cost: $0.11 per minute (added to voice calls)
        const ELEVENLABS_COST_PER_MIN: f32 = 0.11;

        // Web call cost: 0.15 EUR per minute (ElevenLabs only, no Twilio)
        const WEB_CALL_COST_PER_MIN: f32 = 0.15;

        // Both credits_left and credits use actual euro cost
        let cost = match event_type {
            "message" => msg_price,
            "voice" => {
                // Voice: first minute always charged, then per additional minute
                // Includes Twilio voice + ElevenLabs AI voice cost
                let minutes = (amount.unwrap_or(60) as f32 / 60.0).ceil().max(1.0);
                minutes * (voice_price + ELEVENLABS_COST_PER_MIN)
            },
            "web_call" => {
                // Web call: 0.15 EUR per minute (no Twilio, just ElevenLabs)
                let minutes = (amount.unwrap_or(60) as f32 / 60.0).ceil().max(1.0);
                minutes * WEB_CALL_COST_PER_MIN
            },
            "noti_msg" => noti_price,
            "noti_call" => voice_price + ELEVENLABS_COST_PER_MIN, // First minute always charged + ElevenLabs
            "digest" => digest_price,
            _ => return Err("Invalid event type".to_string()),
        };
        (cost, cost) // Same value for both since both are euro-based
    };

    // Verify sufficient balance before deducting (prevents race condition where check passed but balance changed)
    // User must have enough in EITHER credits_left OR credits
    if user.credits_left < credits_left_deduction && user.credits < credits_deduction {
        eprintln!("Insufficient credits at deduction time for user {}: credits_left={}, credits={}, needed={}/{}",
            user_id, user.credits_left, user.credits, credits_left_deduction, credits_deduction);
        return Err("Insufficient credits".to_string());
    }

    // Deduct credits: prefer credits_left, fall back to credits
    if user.credits_left >= credits_left_deduction && credits_left_deduction > 0.0 {
        // Deduct from credits_left
        let new_credits_left = user.credits_left - credits_left_deduction;
        if let Err(e) = state.user_repository.update_user_credits_left(user_id, new_credits_left) {
            eprintln!("Failed to update user credits_left: {}", e);
            return Err("Failed to process credits".to_string());
        }
    } else {
        // Deduct from regular credits
        let new_credits = user.credits - credits_deduction;
        if let Err(e) = state.user_repository.update_user_credits(user_id, new_credits) {
            eprintln!("Failed to update user credits: {}", e);
            return Err("Failed to process credits".to_string());
        }
    }

    // For tier 3 US/CA users: Increment monthly message count and monitor for 1000 limit
    if is_tier3 && event_type == "message" {
        if let Some(ref settings) = user_settings {
            // Check if this is a US/CA subaccount (unlimited messaging with monitoring)
            // US/CA users have outbound_message_pricing of None or very low (we monitor all tier 3 messages)
            if let Err(e) = state.user_core.increment_monthly_message_count(user_id) {
                eprintln!("Failed to increment monthly message count for user {}: {}", user_id, e);
            }

            // Get updated settings to check current count
            if let Ok(updated_settings) = state.user_core.get_user_settings(user_id) {
                let count = updated_settings.monthly_message_count;

                // Send email alert when hitting 1000 messages (only send once when crossing threshold)
                if count >= 1000 && settings.monthly_message_count < 1000 {
                    tracing::warn!("Tier 3 user {} has reached {} messages this month", user_id, count);

                    // Send email notification to admin
                    let state_clone = state.clone();
                    let user_email = user.email.clone();
                    tokio::spawn(async move {
                        if let Err(e) = send_tier3_usage_alert(&state_clone, user_id, &user_email, count).await {
                            tracing::error!("Failed to send tier 3 usage alert: {}", e);
                        }
                    });
                }
            }
        }
    }

    Ok(())
}

/// Sends an email alert when a tier 3 user exceeds 1000 messages per month
async fn send_tier3_usage_alert(
    state: &Arc<AppState>,
    user_id: i32,
    user_email: &str,
    message_count: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    use axum::extract::{Json, State as AxumState};

    let body = format!(
        "Tier 3 Usage Alert - 1000 Messages Reached\n\
        ==========================================\n\n\
        User ID: {}\n\
        User Email: {}\n\
        Monthly Message Count: {}\n\n\
        This tier 3 self-hosted user has reached 1000 outbound messages this month.\n\
        This is a monitoring alert to track usage patterns.\n\
        ",
        user_id,
        user_email,
        message_count
    );

    let email_request = crate::handlers::imap_handlers::SendEmailRequest {
        to: "rasmus@ahtava.com".to_string(),
        subject: format!("Tier 3 Usage Alert - User {} - 1000 Messages", user_id),
        body: body.replace("\n", "\r\n"),
    };

    let auth_user = crate::handlers::auth_middleware::AuthUser {
        user_id: 1,
        is_admin: true,
    };

    match crate::handlers::imap_handlers::send_email(
        AxumState(state.clone()),
        auth_user,
        Json(email_request),
    ).await {
        Ok(_) => {
            tracing::info!("Successfully sent tier 3 usage alert for user {}", user_id);
            Ok(())
        }
        Err((status, err)) => {
            let error_msg = format!("Failed to send tier 3 usage alert: {:?} - {:?}", status, err);
            tracing::error!("{}", error_msg);
            Err(error_msg.into())
        }
    }
}
