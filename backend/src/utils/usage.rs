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
    let message_cost = if user.discount || user.sub_tier == Some("tier 1".to_string()) {
        // For discounted/tier 1 users, use phone number based pricing
        if user.phone_number.starts_with("+1") {
            std::env::var("MESSAGE_COST_US")
                .unwrap_or_else(|_| std::env::var("MESSAGE_COST").expect("MESSAGE_COST not set"))
                .parse::<f32>()
                .unwrap_or(0.10)
        } else {
            std::env::var("MESSAGE_COST")
                .expect("MESSAGE_COST not set")
                .parse::<f32>()
                .unwrap_or(0.20)
        }
    } else {
        // For regular users, use flat rate
        std::env::var("MESSAGE_COST")
            .expect("MESSAGE_COST not set")
            .parse::<f32>()
            .unwrap_or(0.20)
    };

    let voice_second_cost = std::env::var("VOICE_SECOND_COST")
        .expect("VOICE_SECOND_COST not set")
        .parse::<f32>()
        .unwrap_or(0.0033);

    // Check if user has subscription or discount
    let use_regular_credits = user.discount || user.sub_tier == Some("tier 1".to_string());

    if use_regular_credits {
        // For discounted/tier 1 users, just check regular credits
        let required_credits = if event_type == "message" { message_cost } else { 0.0 };
        if user.credits < required_credits {
            return Err("Insufficient credits.".to_string());
        }
    } else {
        // For regular users, check credits_left first, then regular credits
        let required_credits = if event_type == "message" { message_cost } else { 0.0 };
        
        if user.credits_left < required_credits && user.credits < required_credits {
            return Err("Insufficient credits. You have used all your monthly quota and don't have enough extra credits.".to_string());
        }
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

    let message_cost = if user.discount || user.sub_tier == Some("tier 1".to_string()) {
        // For discounted/tier 1 users, use phone number based pricing
        if user.phone_number.starts_with("+1") {
            std::env::var("MESSAGE_COST_US")
                .unwrap_or_else(|_| std::env::var("MESSAGE_COST").expect("MESSAGE_COST not set"))
                .parse::<f32>()
                .unwrap_or(0.10)
        } else {
            std::env::var("MESSAGE_COST")
                .expect("MESSAGE_COST not set")
                .parse::<f32>()
                .unwrap_or(0.20)
        }
    } else {
        // For regular users, use flat rate
        std::env::var("MESSAGE_COST")
            .expect("MESSAGE_COST not set")
            .parse::<f32>()
            .unwrap_or(0.20)
    };


    let mut credits_cost = if event_type == "message" {
        message_cost
    } else {
        let voice_second_cost = std::env::var("VOICE_SECOND_COST")
            .expect("VOICE_SECOND_COST not set")
            .parse::<f32>()
            .unwrap_or(0.0033);
        voice_seconds.unwrap_or(0) as f32 * voice_second_cost
    };

    // Check if user has subscription or discount
    let use_regular_credits = (user.discount || user.sub_tier == Some("tier 1".to_string())) && user.sub_tier != Some("tier 2".to_string());

    println!("use regular credits: {}, sub_tier={:#?}", use_regular_credits, user.sub_tier);
    if use_regular_credits {
        // For discounted/tier 1 users, just deduct from regular credits
        let new_credits = user.credits - credits_cost;
        if let Err(e) = state.user_repository.update_user_credits(user_id, new_credits) {
            eprintln!("Failed to update user credits: {}", e);
            return Err("Failed to process credits".to_string());
        }
    } else {
        // For regular users, use credits_left first, then regular credits
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
            let new_credits = user.credits - remaining_cost;
            if let Err(e) = state.user_repository.update_user_credits(user_id, new_credits) {
                eprintln!("Failed to update user credits: {}", e);
                return Err("Failed to process credits".to_string());
            }
        }
    }

    Ok(())
}
