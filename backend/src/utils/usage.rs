use crate::AppState;
use std::sync::Arc;

/// Checks if a user has sufficient credits to perform an action.
/// Returns Ok(()) if the user has enough credits, or Err with an appropriate error message if not.
/// Also handles automatic recharging if enabled.
pub async fn check_user_credits(
    state: &Arc<AppState>,
    user: &crate::models::user_models::User,
    event_type: &str,
) -> Result<(), String> {
    // If user has credits_left, use 1.0 credit per message or voice_seconds/60 for voice
    let (message_cost, voice_second_cost) = if user.credits_left > 0.0 {
        (1.0, 1.0/60.0)
    } else {
        // Otherwise use country-based pricing
        let msg_cost = if user.phone_number.starts_with("+1") {
            0.10 // US
        } else if user.phone_number.starts_with("+358") {
            0.30 // Finland
        } else if user.phone_number.starts_with("+44") {
            0.30 // UK
        } else if user.phone_number.starts_with("+61") {
            0.40 // Australia
        } else if user.phone_number.starts_with("+972") {
            0.90 // Israel
        } else {
            0.30 // Default to Finland/UK rate
        };

        let voice_cost = if user.phone_number.starts_with("+1") {
            0.0033 // US: 0.20/minute
        } else if user.phone_number.starts_with("+358") {
            0.0042 // Finland: 0.25/minute
        } else if user.phone_number.starts_with("+44") {
            0.0042 // UK: 0.25/minute
        } else if user.phone_number.starts_with("+61") {
            0.0042 // Australia: 0.25/minute
        } else if user.phone_number.starts_with("+972") {
            0.0033 // Israel: 0.20/minute
        } else {
            0.0042 // Default to Finland/UK/AU rate
        };
        (msg_cost, voice_cost)
    };


    // Check if the event type is free based on discount_tier
    let is_free = match user.discount_tier.as_deref() {
        Some("full") => true,
        Some("msg") if event_type == "message" => true,
        Some("voice") if event_type != "message" => true,
        _ => false,
    };

    if is_free {
        return Ok(());
    }

    // Check credits_left first, then overage credits
    let required_credits = if event_type == "message" { message_cost } else { 0.0 };
    
    if (user.credits_left < 0.0 || user.credits_left < required_credits) && (user.credits < 0.0 || user.credits < required_credits) {
        // Check if enough time has passed since the last notification (24 hours)
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;
        
        let should_notify = match user.last_credits_notification {
            None => true,
            Some(last_time) => (current_time - last_time) >= 24 * 3600 // 24 hours in seconds
        };

        if should_notify {
            // Send notification about depleted credits and monthly quota
            if let Ok(conversation) = state.user_conversations.get_conversation(&user, user.preferred_number.clone().unwrap_or_else(|| std::env::var("SHAZAM_PHONE_NUMBER").expect("SHAZAM_PHONE_NUMBER not set"))).await {
                let conversation_sid = conversation.conversation_sid.clone();
                let twilio_number = conversation.twilio_number.clone();
                
                // Update the last notification timestamp
                if let Err(e) = state.user_repository.update_last_credits_notification(user.id, current_time) {
                    eprintln!("Failed to update last_credits_notification: {}", e);
                }

                let user_clone = user.clone();
                
                tokio::spawn(async move {
                    let _ = crate::api::twilio_utils::send_conversation_message(
                        &conversation_sid,
                        &twilio_number,
                        "Your credits and monthly quota have been depleted. Please recharge your credits to continue using the service.",
                        false,
                        &user_clone,
                    ).await;
                });
            }
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
/// Returns Ok(()) if credits were successfully deducted, or Err with an appropriate error message if not.
pub fn deduct_user_credits(
    state: &Arc<AppState>,
    user_id: i32,
    event_type: &str,
    voice_seconds: Option<i32>,
) -> Result<(), String> {

    let user = match state.user_repository.find_by_id(user_id) {
        Ok(Some(user)) => user,
        Ok(None) => return Err("User not found".to_string()),
        Err(e) => {
            eprintln!("Database error while finding user {}: {}", user_id, e);
            return Err("Database error occurred".to_string());
        }
    };

    // If user has credits_left, use 1.0 credit per message or voice_seconds/60 for voice
    // First check if the event type is free based on discount_tier
    let is_free = match user.discount_tier.as_deref() {
        Some("full") => true,
        Some("msg") if event_type == "message" => true,
        Some("voice") if event_type != "message" => true,
        _ => false,
    };

    if is_free {
        return Ok(());
    }

    let (message_cost, voice_second_cost) = if user.credits_left > 0.0 {
        (1.0, 1.0/60.0) // message quota is calculated at 1e per msg and 1e per voice minute(just easy to calculate) and there are 40e worth for every month on every subscription
    } else {
        // Otherwise use country-based pricing
        let msg_cost = if user.phone_number.starts_with("+1") {
            0.10 // US
        } else if user.phone_number.starts_with("+358") {
            0.30 // Finland
        } else if user.phone_number.starts_with("+44") {
            0.30 // UK
        } else if user.phone_number.starts_with("+61") {
            0.40 // Australia
        } else if user.phone_number.starts_with("+972") {
            0.90 // Israel
        } else {
            0.30 // Default to Finland/UK rate
        };

        let voice_cost = if user.phone_number.starts_with("+1") {
            0.0033 // US: 0.20/minute
        } else if user.phone_number.starts_with("+358") {
            0.0042 // Finland: 0.25/minute
        } else if user.phone_number.starts_with("+44") {
            0.0042 // UK: 0.25/minute
        } else if user.phone_number.starts_with("+61") {
            0.0042 // Australia: 0.25/minute
        } else if user.phone_number.starts_with("+972") {
            0.0033 // Israel: 0.20/minute
        } else {
            0.0042 // Default to Finland/UK/AU rate
        };
        (msg_cost, voice_cost)
    };

    let mut credits_cost = if event_type == "message" {
        message_cost
    } else {
        voice_seconds.unwrap_or(0) as f32 * voice_second_cost
    };

    // Use credits_left first, then overage credits
    if user.credits_left >= credits_cost {
        // Deduct from credits_left
        let new_credits_left = user.credits_left - credits_cost;
        if let Err(e) = state.user_repository.update_user_credits_left(user_id, new_credits_left) {
            eprintln!("Failed to update user credits_left: {}", e);
            return Err("Failed to process credits".to_string());
        }
    } else {
        // Use remaining credits_left and deduct rest from regular credits
        let remaining_cost = if user.credits_left > 0.0 {
            credits_cost - user.credits_left
        } else {
            credits_cost
        };

        println!("User credits left: {}", user.credits_left);
        // Set credits_left to 0 if there were any left
        if user.credits_left > 0.0 {
            if let Err(e) = state.user_repository.update_user_credits_left(user_id, 0.0) {
                eprintln!("Failed to update user credits_left: {}", e);
                return Err("Failed to process credits".to_string());
            }
        }

        // Deduct remaining cost from regular credits
        let new_credits = (user.credits - remaining_cost).max(0.0);  // Ensure credits don't go below 0
        if let Err(e) = state.user_repository.update_user_credits(user_id, new_credits) {
            eprintln!("Failed to update user credits: {}", e);
            return Err("Failed to process credits".to_string());
        }
    }

    Ok(())
}
