//! Centralized plan feature helpers.
//!
//! All plan_type gating should go through these two functions.
//! If tier names change in the future, only this file needs updating.

/// Does this plan include automatic LLM processing of incoming messages?
/// (message monitoring, critical filtering, auto item creation)
pub fn has_auto_features(plan_type: Option<&str>) -> bool {
    matches!(plan_type, Some("autopilot") | Some("byot"))
}

/// Does this plan use hosted messaging credits?
/// (not BYOT which pays Twilio directly)
pub fn uses_hosted_credits(plan_type: Option<&str>) -> bool {
    matches!(plan_type, Some("assistant") | Some("autopilot"))
}
