use crate::utils::country::get_country_code_from_phone;
use crate::utils::plan_features::TWILIO_COST_MARGIN;
use crate::AppState;
use crate::UserCoreOps;
use std::sync::Arc;

/// Checks if a user has sufficient credits to perform an action.
///
/// For SMS events (message, noti_msg, digest): just checks credits > 0.
/// Actual SMS cost is deducted later at Twilio status callback.
///
/// For voice/web events: estimates cost upfront since these commit resources.
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

    // Calculate minimum required credits
    let required = match event_type {
        // SMS events: just check > 0 (actual cost deducted at Twilio callback)
        "message" | "noti_msg" | "digest" => 0.01,
        // Voice/web events: estimate upfront cost (these commit resources)
        "voice" | "noti_call" => {
            get_voice_cost_estimate(state, &user.phone_number, event_type, amount).await
        }
        "web_call" => {
            // Web call: 0.15 per minute (ElevenLabs only, no Twilio)
            let minutes = (amount.unwrap_or(60) as f32 / 60.0).ceil().max(1.0);
            minutes * 0.15
        }
        _ => return Err("Invalid event type".to_string()),
    };

    // Check if user has sufficient balance in either pool
    if user.credits_left < required && user.credits < required {
        // Only send notification once ever (when last_credits_notification is None)
        let should_notify = user.last_credits_notification.is_none();

        if should_notify && event_type != "digest" {
            let current_time = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i32;

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
                tracing::debug!(
                    "User {} credits is under threshold, attempting automatic charge",
                    user.id
                );
                use axum::extract::{Path, State};
                let state_clone = Arc::clone(state);
                let user_id = user.id;
                tokio::spawn(async move {
                    let _ = crate::handlers::stripe_handlers::automatic_charge(
                        State(state_clone),
                        Path(user_id),
                    )
                    .await;
                });
            }
        }
        Err(e) => eprintln!("Failed to check if user credits is under threshold: {}", e),
    }

    Ok(())
}

/// Estimate voice call cost for pre-send credit check.
async fn get_voice_cost_estimate(
    state: &Arc<AppState>,
    phone_number: &str,
    event_type: &str,
    amount: Option<i32>,
) -> f32 {
    const ELEVENLABS_COST_PER_MIN: f32 = 0.11;

    let country_code = get_country_code_from_phone(phone_number);
    let pricing = if let Some(code) = country_code {
        crate::api::twilio_pricing::get_notification_only_pricing(state, &code)
            .await
            .ok()
    } else {
        None
    };

    let voice_price = match pricing {
        Some(p) => p.calculated_voice_price,
        None => 0.13, // fallback
    };

    match event_type {
        "voice" => {
            let minutes = (amount.unwrap_or(60) as f32 / 60.0).ceil().max(1.0);
            minutes * (voice_price + ELEVENLABS_COST_PER_MIN)
        }
        "noti_call" => voice_price + ELEVENLABS_COST_PER_MIN,
        _ => 0.0,
    }
}

/// Deducts credits from a user's account for voice/web events.
///
/// SMS events are now deducted at Twilio status callback via deduct_from_twilio_price().
/// This function is only called for: voice, noti_call, web_call.
///
/// Returns Ok(()) if credits were successfully deducted, or Err with an appropriate error message.
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

    // Calculate deduction for voice/web events
    let cost = get_activity_cost(state, &user.phone_number, event_type, amount);
    if cost <= 0.0 {
        return Ok(());
    }

    // Verify sufficient balance
    if user.credits_left < cost && user.credits < cost {
        eprintln!(
            "Insufficient credits at deduction time for user {}: credits_left={}, credits={}, needed={}",
            user_id, user.credits_left, user.credits, cost
        );
        return Err("Insufficient credits".to_string());
    }

    // Deduct credits: prefer credits_left, fall back to credits
    if user.credits_left >= cost {
        let new_credits_left = user.credits_left - cost;
        if let Err(e) = state
            .user_repository
            .update_user_credits_left(user_id, new_credits_left)
        {
            eprintln!("Failed to update user credits_left: {}", e);
            return Err("Failed to process credits".to_string());
        }
    } else if user.credits >= cost {
        let new_credits = user.credits - cost;
        if let Err(e) = state
            .user_repository
            .update_user_credits(user_id, new_credits)
        {
            eprintln!("Failed to update user credits: {}", e);
            return Err("Failed to process credits".to_string());
        }
    }

    Ok(())
}

/// Calculate cost for voice/web events using cached pricing.
fn get_activity_cost(
    state: &Arc<AppState>,
    phone_number: &str,
    event_type: &str,
    amount: Option<i32>,
) -> f32 {
    const ELEVENLABS_COST_PER_MIN: f32 = 0.11;
    const WEB_CALL_COST_PER_MIN: f32 = 0.15;

    let country_code = get_country_code_from_phone(phone_number);
    let pricing = country_code.and_then(|code| {
        crate::api::twilio_pricing::get_cached_notification_pricing_sync(state, &code)
    });

    let voice_price = match &pricing {
        Some(p) => p.calculated_voice_price,
        None => 0.13, // fallback
    };

    match event_type {
        "voice" => {
            let minutes = (amount.unwrap_or(60) as f32 / 60.0).ceil().max(1.0);
            minutes * (voice_price + ELEVENLABS_COST_PER_MIN)
        }
        "noti_call" => voice_price + ELEVENLABS_COST_PER_MIN,
        "web_call" => {
            let minutes = (amount.unwrap_or(60) as f32 / 60.0).ceil().max(1.0);
            minutes * WEB_CALL_COST_PER_MIN
        }
        _ => 0.0, // SMS events should not reach here
    }
}

/// Deduct credits based on actual Twilio price from StatusCallback.
/// Called when Twilio reports the final cost of an SMS.
/// Price is always in USD from Twilio. We apply our margin and deduct directly.
pub fn deduct_from_twilio_price(
    state: &Arc<AppState>,
    user_id: i32,
    twilio_price_usd: f32,
) -> Result<f32, String> {
    let user = match state.user_core.find_by_id(user_id) {
        Ok(Some(user)) => user,
        Ok(None) => return Err("User not found".to_string()),
        Err(e) => return Err(format!("Database error: {}", e)),
    };

    // BYOT users pay Twilio directly
    if state.user_core.is_byot_user(user_id) {
        return Ok(0.0);
    }

    let abs_price = twilio_price_usd.abs();
    if abs_price == 0.0 {
        return Ok(0.0);
    }

    // Apply margin. Twilio price already reflects country-specific costs.
    let cost = abs_price * TWILIO_COST_MARGIN;

    // Deduct: prefer credits_left, fall back to credits
    if user.credits_left >= cost {
        state
            .user_repository
            .update_user_credits_left(user_id, user.credits_left - cost)
            .map_err(|e| format!("Failed to update credits_left: {}", e))?;
    } else if user.credits >= cost {
        state
            .user_repository
            .update_user_credits(user_id, user.credits - cost)
            .map_err(|e| format!("Failed to update credits: {}", e))?;
    } else {
        // Not enough credits - deduct what we can (user went slightly negative)
        // Acceptable since we only do basic checks at send time
        if user.credits_left > 0.0 {
            state
                .user_repository
                .update_user_credits_left(user_id, 0.0)
                .map_err(|e| format!("Failed to zero credits_left: {}", e))?;
        } else {
            state
                .user_repository
                .update_user_credits(user_id, 0.0)
                .map_err(|e| format!("Failed to zero credits: {}", e))?;
        }
    }

    Ok(cost)
}
