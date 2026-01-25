use crate::utils::country::get_country_code_from_phone;
use crate::AppState;
use crate::UserCoreOps;
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

    // Check if user has an active subscription (tier 2 required)
    if user.sub_tier.as_deref() != Some("tier 2") {
        return Err(
            "Active subscription required. Please subscribe to continue using the service."
                .to_string(),
        );
    }

    // BYOT users pay Twilio directly - no credit check
    if state.user_core.is_byot_user(user.id) {
        return Ok(());
    }

    // All other users are charged through Lightfriend:
    // - Local number countries (US, CA, FI, NL, GB, AU) use hardcoded pricing
    // - Notification-only countries (all others worldwide) use dynamic Twilio API pricing

    // Get required amounts based on region (dual interpretation)
    let (required_credits_left, required_credits) = if is_us_or_ca(&user.phone_number) {
        // US/CA: credits_left is message COUNT, credits is dollar value
        let credits_left_cost = match event_type {
            "message" => 1.0,
            "voice" => 0.0,    // voice uses credits, not credits_left
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
            crate::api::twilio_pricing::get_notification_only_pricing(state, &code)
                .await
                .ok()
        } else {
            None
        };

        let (noti_price, msg_price, digest_price, voice_price) = match pricing {
            Some(p) => (
                p.notification_price,
                p.regular_message_price,
                p.digest_price,
                p.calculated_voice_price,
            ),
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
            }
            "web_call" => {
                // Web call: 0.15 EUR per minute (no Twilio, just ElevenLabs)
                let minutes = (amount.unwrap_or(60) as f32 / 60.0).ceil().max(1.0);
                minutes * WEB_CALL_COST_PER_MIN
            }
            "noti_msg" => noti_price,
            "noti_call" => voice_price + ELEVENLABS_COST_PER_MIN, // First minute always charged + ElevenLabs
            "digest" => digest_price * amount.unwrap_or(1) as f32,
            _ => return Err("Invalid event type".to_string()),
        };
        (cost, cost) // Same value for both since both are euro-based
    };

    // Check if user has sufficient balance
    if user.credits_left < required_credits_left && user.credits < required_credits {
        // Only send notification once ever (when last_credits_notification is None)
        // The notification flag is cleared when user adds credits
        let should_notify = user.last_credits_notification.is_none();

        if should_notify && event_type != "digest" {
            // Send notification about depleted credits and monthly quota
            let current_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i32;

            // Update the last notification timestamp to prevent repeat notifications
            if let Err(e) = state
                .user_core
                .update_last_credits_notification(user.id, current_time)
            {
                eprintln!("Failed to update last_credits_notification: {}", e);
            }

            let user_clone = user.clone();
            let state_clone = state.clone();

            tokio::spawn(async move {
                let _ = state_clone
                    .twilio_message_service
                    .send_sms(
                        "Your credits and monthly quota have been depleted. Please recharge your credits to continue using the service.",
                        None,
                        &user_clone,
                    )
                    .await;
            });
        }
        return Err("Insufficient credits. You have used all your monthly quota and don't have enough extra credits.".to_string());
    }

    // Check credits threshold and handle automatic charging
    match state.user_repository.is_credits_under_threshold(user.id) {
        Ok(is_under) => {
            if is_under && user.charge_when_under {
                println!(
                    "User {} credits is under threshold, attempting automatic charge",
                    user.id
                );
                use axum::extract::{Path, State};
                let state_clone = Arc::clone(state);
                let user_id = user.id; // Clone the user ID
                tokio::spawn(async move {
                    let _ = crate::handlers::stripe_handlers::automatic_charge(
                        State(state_clone),
                        Path(user_id),
                    )
                    .await;
                });
                println!("Initiated automatic recharge for user");
            }
        }
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

    // BYOT users pay Twilio directly - no credit deduction
    if state.user_core.is_byot_user(user_id) {
        return Ok(());
    }

    // All other users are charged through Lightfriend:
    // - Local number countries (US, CA, FI, NL, GB, AU) use hardcoded pricing
    // - Notification-only countries (all others worldwide) use dynamic Twilio API pricing

    // Calculate deduction amounts based on region (dual interpretation)
    let (credits_left_deduction, credits_deduction) = if is_us_or_ca(&user.phone_number) {
        // US/CA: credits_left is message COUNT, credits is dollar value
        let credits_left_cost = match event_type {
            "message" => 1.0,
            "voice" => 0.0, // voice uses credits, not credits_left
            "web_call" => {
                // Web call: 1 message credit per minute (round up)

                (amount.unwrap_or(60) as f32 / 60.0).ceil().max(1.0)
            }
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
            Some(p) => (
                p.notification_price,
                p.regular_message_price,
                p.digest_price,
                p.calculated_voice_price,
            ),
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
            }
            "web_call" => {
                // Web call: 0.15 EUR per minute (no Twilio, just ElevenLabs)
                let minutes = (amount.unwrap_or(60) as f32 / 60.0).ceil().max(1.0);
                minutes * WEB_CALL_COST_PER_MIN
            }
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
    // IMPORTANT: Must check balance before deducting to prevent negative credits
    if user.credits_left >= credits_left_deduction && credits_left_deduction > 0.0 {
        // Deduct from credits_left
        let new_credits_left = user.credits_left - credits_left_deduction;
        if let Err(e) = state
            .user_repository
            .update_user_credits_left(user_id, new_credits_left)
        {
            eprintln!("Failed to update user credits_left: {}", e);
            return Err("Failed to process credits".to_string());
        }
    } else if user.credits >= credits_deduction && credits_deduction > 0.0 {
        // Deduct from regular credits only if we have enough
        let new_credits = user.credits - credits_deduction;
        if let Err(e) = state
            .user_repository
            .update_user_credits(user_id, new_credits)
        {
            eprintln!("Failed to update user credits: {}", e);
            return Err("Failed to process credits".to_string());
        }
    } else if credits_deduction > 0.0 {
        // Not enough credits - should have been caught earlier but log and fail gracefully
        eprintln!(
            "Insufficient credits at deduction for user {}: credits={}, needed={}",
            user_id, user.credits, credits_deduction
        );
        return Err("Insufficient credits".to_string());
    }
    // If both deductions are 0.0, nothing to deduct (free event)

    Ok(())
}
