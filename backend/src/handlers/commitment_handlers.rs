//! Dashboard endpoints for managing the commitment-detection signal state:
//! the per-user mute / always-track sender rules, and the recent SMS-prompt
//! history (so users can see what was asked and what they replied).

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;
use serde_json::json;

use crate::handlers::auth_middleware::AuthUser;
use crate::repositories::commitment_repository::{RULE_ALWAYS_TRACK, RULE_MUTE};
use crate::AppState;

#[derive(Serialize)]
pub struct SenderRuleView {
    pub id: i32,
    pub platform: String,
    pub sender_key: String,
    pub rule_type: String,
    pub source: String,
    pub created_at: i32,
}

#[derive(Serialize)]
pub struct SenderRulesResponse {
    pub muted: Vec<SenderRuleView>,
    pub always_track: Vec<SenderRuleView>,
}

/// GET /api/commitment/sender-rules
/// Returns the user's active mute and always-track sender rules grouped by
/// type so the dashboard can render them in two sections.
pub async fn list_sender_rules(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<SenderRulesResponse>, StatusCode> {
    let user_id = auth_user.user_id;

    let muted = state
        .commitment_repository
        .list_active_rules(user_id, RULE_MUTE)
        .map_err(|e| {
            tracing::error!("list_active_rules(mute) user={}: {}", user_id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    let always_track = state
        .commitment_repository
        .list_active_rules(user_id, RULE_ALWAYS_TRACK)
        .map_err(|e| {
            tracing::error!("list_active_rules(always) user={}: {}", user_id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let to_view = |r: crate::models::commitment_models::CommitmentSenderRule| SenderRuleView {
        id: r.id,
        platform: r.platform,
        sender_key: r.sender_key,
        rule_type: r.rule_type,
        source: r.source,
        created_at: r.created_at,
    };

    Ok(Json(SenderRulesResponse {
        muted: muted.into_iter().map(to_view).collect(),
        always_track: always_track.into_iter().map(to_view).collect(),
    }))
}

/// DELETE /api/commitment/sender-rules/:id
/// Deactivates a rule. Scoped to the auth_user so users can't delete each
/// other's rules; an unknown id returns 200 with `removed=0` rather than
/// 404 because the rule may already be inactive.
pub async fn deactivate_sender_rule(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(rule_id): Path<i32>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let user_id = auth_user.user_id;
    let removed = state
        .commitment_repository
        .deactivate_rule(user_id, rule_id)
        .map_err(|e| {
            tracing::error!(
                "deactivate_rule user={} rule_id={}: {}",
                user_id,
                rule_id,
                e
            );
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(json!({ "removed": removed })))
}

#[derive(Serialize)]
pub struct PromptView {
    pub id: i32,
    pub platform: String,
    pub sender_display_name: String,
    pub commitment_description: String,
    pub sent_at: i32,
    pub user_label: Option<String>,
    pub labeled_at: Option<i32>,
    pub resulting_event_id: Option<i32>,
    pub resolved_at: Option<i32>,
}

/// GET /api/commitment/recent-prompts
/// Returns the most recent commitment prompts (capped at 50) for the
/// transparency panel. Lets a user see what we asked them about, what they
/// answered, and which event was created.
pub async fn list_recent_prompts(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<Vec<PromptView>>, StatusCode> {
    let user_id = auth_user.user_id;
    let rows = state
        .commitment_repository
        .list_recent_prompts(user_id, 50)
        .map_err(|e| {
            tracing::error!("list_recent_prompts user={}: {}", user_id, e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(
        rows.into_iter()
            .map(|p| PromptView {
                id: p.id,
                platform: p.platform,
                sender_display_name: p.sender_display_name,
                commitment_description: p.commitment_description,
                sent_at: p.sent_at,
                user_label: p.user_label,
                labeled_at: p.labeled_at,
                resulting_event_id: p.resulting_event_id,
                resolved_at: p.resolved_at,
            })
            .collect(),
    ))
}
