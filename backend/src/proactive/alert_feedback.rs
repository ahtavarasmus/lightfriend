//! Incoming SMS handler for important-alert feedback.
//!
//! After Lightfriend interrupts the user for an urgent message, the alert SMS
//! asks for a tiny rating: 1 = worth it, 2 = should have waited. We store that
//! as usage-log signal against the latest recent important alert, which gives
//! the dashboard an early quality ratio without needing a new table yet.

use std::sync::Arc;

use tracing::{info, warn};

use crate::models::user_models::User;
use crate::repositories::user_repository::{
    LogUsageParams, SYSTEM_ALERT_FEEDBACK_SHOULD_WAIT, SYSTEM_ALERT_FEEDBACK_WORTH_IT,
};
use crate::AppState;

const FEEDBACK_WINDOW_SECS: i32 = 6 * 3600;

pub async fn try_handle_reply(state: &Arc<AppState>, user: &User, body: &str) -> Option<String> {
    let worth_it = parse_reply(body)?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i32;

    let alert = match state
        .user_repository
        .latest_system_alert_for_feedback(user.id, now - FEEDBACK_WINDOW_SECS)
    {
        Ok(Some(alert)) => alert,
        Ok(None) => {
            info!(
                "alert_feedback user={} got '{}' but no recent important alert - ignoring as agent input",
                user.id, body
            );
            return None;
        }
        Err(e) => {
            warn!("alert_feedback lookup failed user={}: {}", user.id, e);
            return None;
        }
    };

    match state
        .user_repository
        .has_system_alert_feedback_after(user.id, alert.created_at)
    {
        Ok(true) => return Some("Already got your rating for that alert.".to_string()),
        Ok(false) => {}
        Err(e) => {
            warn!(
                "alert_feedback duplicate check failed user={} alert={}: {}",
                user.id, alert.id, e
            );
            return None;
        }
    }

    let activity_type = if worth_it {
        SYSTEM_ALERT_FEEDBACK_WORTH_IT
    } else {
        SYSTEM_ALERT_FEEDBACK_SHOULD_WAIT
    };
    let reason = format!(
        "alert_id={} alert_type={} alert_created_at={}",
        alert.id, alert.activity_type, alert.created_at
    );

    if let Err(e) = state.user_repository.log_usage(LogUsageParams {
        user_id: user.id,
        sid: None,
        activity_type: activity_type.to_string(),
        credits: None,
        time_consumed: None,
        success: Some(true),
        reason: Some(reason),
        status: Some("recorded".to_string()),
        recharge_threshold_timestamp: None,
        zero_credits_timestamp: None,
    }) {
        warn!(
            "alert_feedback failed to record user={} alert={}: {}",
            user.id, alert.id, e
        );
        return Some("I saw that rating, but couldn't save it right now.".to_string());
    }

    Some(if worth_it {
        "Thanks. I'll keep interrupting for messages like that.".to_string()
    } else {
        "Got it. I'll be more careful before interrupting for messages like that.".to_string()
    })
}

fn parse_reply(body: &str) -> Option<bool> {
    match body.trim() {
        "1" => Some(true),
        "2" => Some(false),
        _ => None,
    }
}
