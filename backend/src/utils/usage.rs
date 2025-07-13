use crate::AppState;
use std::sync::Arc;

/// Checks if a user has sufficient credits to perform an action.
/// Returns Ok(()) if the user has enough credits, or Err with an appropriate error message if not.
/// Also handles automatic recharging if enabled.
pub async fn check_user_credits(
    state: &Arc<AppState>,
    user: &crate::models::user_models::User,
    event_type: &str,
    amount: Option<i32>,
) -> Result<(), String> {
    // Get country from user settings or phone number
    let country = match state.user_core.get_user_settings(user.id) {
        Ok(settings) => settings.sub_country,
        Err(e) => {
            eprintln!("Failed to get user settings: {}", e);
            None
        }
    };

    // Define costs for each country
    let (message_cost, voice_second_cost, notification_cost) = match country.as_deref() {
        Some("US") => (0.15, 0.0025, 0.075),  // US: $0.20/msg, $0.20/minute, $0.075/notification
        Some("FI") => (0.30, 0.005, 0.15),  // Finland: €0.30/msg, €0.25/minute, €0.10/notification
        Some("UK") => (0.30, 0.005, 0.15),  // UK: £0.30/msg, £0.25/minute, £0.10/notification
        Some("AU") => (0.30, 0.005, 0.15),  // Australia: A$0.30/msg, A$0.25/minute, A$0.10/notification
        Some(_) => (0.70, 0.005, 0.35),     // Default rate for unrecognized country codes
        None => {
            // Fallback to phone number based detection
            if user.phone_number.starts_with("+1") {
                (0.15, 0.0033, 0.075)  // US
            } else if user.phone_number.starts_with("+358") {
                (0.30, 0.005, 0.15)  // Finland
            } else if user.phone_number.starts_with("+44") {
                (0.30, 0.005, 0.15)  // UK
            } else if user.phone_number.starts_with("+61") {
                (0.30, 0.005, 0.15)  // Australia
            } else {
                (0.70, 0.005, 0.35)  // Default to higher rate for unknown regions
            }
        }
    };


    // Check if the event type is free based on discount_tier
    let is_free = match user.discount_tier.as_deref() {
        Some("full") => true,
        Some("msg") if event_type == "message" => true,
        Some("voice") if event_type != "message" => true,
        _ => false,
    };

    let is_self_hosted= std::env::var("ENVIRONMENT") == Ok("self_hosted".to_string());

    if is_free || is_self_hosted {
        return Ok(());
    }

    // Calculate cost based on event type
    let required_credits = match event_type {
        "message" => message_cost,
        "voice" => amount.unwrap_or(0) as f32 * voice_second_cost,
        "notification" => notification_cost,
        "digest" => amount.unwrap_or(0) as f32 * message_cost,
        _ => return Err("Invalid event type".to_string()),
    };

    let required_credits_left= match event_type {
        "message" => 1.00,
        "voice" => 0.00,
        "notification" => 1.00 / 2.00,
        "digest" => 1.00 * amount.unwrap_or(0) as f32,
        _ => return Err("Invalid event type".to_string()),
    };

    
    if (user.credits_left < 0.00 || user.credits_left < required_credits_left) && (user.credits < 0.0 || user.credits < required_credits) {
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
            if let Ok(conversation) = state.user_conversations.get_conversation(&state, &user, user.preferred_number.clone().unwrap_or_else(|| std::env::var("SHAZAM_PHONE_NUMBER").expect("SHAZAM_PHONE_NUMBER not set"))).await {
                let conversation_sid = conversation.conversation_sid.clone();
                let twilio_number = conversation.twilio_number.clone();
                
                // Update the last notification timestamp
                if let Err(e) = state.user_core.update_last_credits_notification(user.id, current_time) {
                    eprintln!("Failed to update last_credits_notification: {}", e);
                }

                let user_clone = user.clone();
                let state_clone = state.clone();
                
                tokio::spawn(async move {
                    let _ = crate::api::twilio_utils::send_conversation_message(
                        &state_clone,
                        &conversation_sid,
                        &twilio_number,
                        "Your credits and monthly quota have been depleted. Please recharge your credits to continue using the service.",
                        false,
                        None,
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


    // Check if the event type is free based on discount_tier
    let is_free = match user.discount_tier.as_deref() {
        Some("full") => true,
        Some("msg") if event_type == "message" => true,
        Some("voice") if event_type != "message" => true,
        _ => false,
    };

    let is_self_hosted= std::env::var("ENVIRONMENT") == Ok("self_hosted".to_string());

    if is_free || is_self_hosted {
        return Ok(());
    }

    // Get country from user settings or phone number
    let country = match state.user_core.get_user_settings(user_id) {
        Ok(settings) => settings.sub_country,
        Err(e) => {
            eprintln!("Failed to get user settings: {}", e);
            None
        }
    };

    // Define costs for each country
    let (message_cost, voice_second_cost, notification_cost) = match country.as_deref() {
        Some("US") => (0.15, 0.0025, 0.075),  // US: $0.10/msg, $0.20/minute, $0.05/notification
        Some("FI") => (0.30, 0.005, 0.15),  // Finland: €0.30/msg, €0.25/minute, €0.15/notification
        Some("UK") => (0.30, 0.005, 0.15),  // UK: £0.30/msg, £0.25/minute, £0.15/notification
        Some("AU") => (0.30, 0.005, 0.15),  // Australia: A$0.30/msg, A$0.25/minute, A$0.20/notification
        Some(_) => (0.70, 0.005, 0.35),     // Default rate for unrecognized country codes
        None => {
            // Fallback to phone number based detection
            if user.phone_number.starts_with("+1") {
                (0.15, 0.0033, 0.075)  // US
            } else if user.phone_number.starts_with("+358") {
                (0.30, 0.005, 0.15)  // Finland
            } else if user.phone_number.starts_with("+44") {
                (0.30, 0.005, 0.15)  // UK
            } else if user.phone_number.starts_with("+61") {
                (0.30, 0.005, 0.15)  // Australia
            } else {
                (0.70, 0.005, 0.35)  // Default to Finland/UK rate
            }
        }
    };

    // Calculate cost based on event type
    let cost = match event_type {
        "message" => message_cost,
        "voice" => amount.unwrap_or(0) as f32 * voice_second_cost,
        "notification" => notification_cost,
        "digest" => amount.unwrap_or(0) as f32 * message_cost,
        _ => return Err("Invalid event type".to_string()),
    };

    let cost_credits_left = match event_type {
        "message" => 1.00,
        "voice" => amount.unwrap_or(0) as f32 / 60.00,
        "notification" => 1.00 / 2.00,
        "digest" => 1.00 * amount.unwrap_or(0) as f32,
        _ => return Err("Invalid event type".to_string()),
    };

    // Deduct credits based on available credits_left
    if user.credits_left >= cost_credits_left {
        // Deduct from credits_left only
        if let Err(e) = state.user_repository.update_user_credits_left(user_id, (user.credits_left - cost_credits_left).max(0.0)) {
            eprintln!("Failed to update user credits_left: {}", e);
            return Err("Failed to process credits".to_string());
        }
    } else {
        // Deduct from regular credits only
        let new_credits = (user.credits - cost).max(0.0);
        if let Err(e) = state.user_repository.update_user_credits(user_id, new_credits) {
            eprintln!("Failed to update user credits: {}", e);
            return Err("Failed to process credits".to_string());
        }
    }

    Ok(())
}
