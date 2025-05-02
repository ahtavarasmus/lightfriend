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

    let mut required_credits: f32 = 0.0; // for voice  
    if event_type == "message" {
        // Get message cost
        required_credits = std::env::var("MESSAGE_COST")
            .expect("MESSAGE_COST not set")
            .parse::<f32>()
            .unwrap_or(0.10);
    }

    // No sub credits left, check extra credits
    if user.credits < required_credits {
        return Err("Insufficient credits. You have used all your monthly credits and don't have enough extra credits.".to_string());
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

    let message_cost: f32 = std::env::var("MESSAGE_COST")
        .expect("MESSAGE_COST not set")
        .parse::<f32>()
        .unwrap_or(0.15); // default to message

    let mut credits_cost = message_cost;

    if event_type == "voice" {
        let voice_second_cost = std::env::var("VOICE_SECOND_COST")
            .expect("VOICE_SECOND_COST not set")
            .parse::<f32>()
            .unwrap_or(0.0033);
        credits_cost = voice_seconds.unwrap_or(0) as f32 * voice_second_cost;
    }

    let new_credits = user.credits - credits_cost;
    if let Err(e) = state.user_repository.update_user_credits(user_id, new_credits) {
        eprintln!("Failed to update user credits: {}", e);
        return Err("Failed to process credits".to_string());
    }

    Ok(())
}
