use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::handlers::auth_middleware::AuthUser;
use crate::models::ontology_models::NewOntRule;
use crate::proactive::rules::{compute_next_fire_at, ActionConfig, TriggerConfig};
use crate::repositories::user_core::UserCoreOps;
use crate::AppState;

#[derive(Deserialize)]
pub struct CreateRuleRequest {
    pub name: String,
    pub trigger_type: String,
    pub trigger_config: String,
    pub logic_type: String,
    pub logic_prompt: Option<String>,
    pub logic_fetch: Option<String>,
    pub action_type: String,
    pub action_config: String,
    pub expires_in_days: Option<f64>,
    pub flow_config: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateRuleStatusRequest {
    pub status: String,
}

pub async fn list_rules(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<serde_json::Value>, StatusCode> {
    state
        .ontology_repository
        .get_rules(auth_user.user_id)
        .map(|rules| Json(json!(rules)))
        .map_err(|e| {
            tracing::error!("Failed to list rules: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

pub async fn create_rule(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(req): Json<CreateRuleRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let user_id = auth_user.user_id;

    // Validate trigger_config
    let trigger: TriggerConfig = serde_json::from_str(&req.trigger_config).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("Invalid trigger_config: {}", e) })),
        )
    })?;

    // Validate action_config
    let _action: ActionConfig = serde_json::from_str(&req.action_config).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("Invalid action_config: {}", e) })),
        )
    })?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i32;

    // Compute next_fire_at for schedule rules
    let next_fire_at = if req.trigger_type == "schedule" {
        match trigger.schedule.as_deref() {
            Some("once") => trigger
                .at
                .as_ref()
                .and_then(|at| crate::proactive::utils::parse_iso_to_timestamp(at)),
            Some("recurring") => {
                if let Some(ref pattern) = trigger.pattern {
                    let user_tz = state
                        .user_core
                        .get_user_info(user_id)
                        .ok()
                        .and_then(|info| info.timezone)
                        .unwrap_or_else(|| "UTC".to_string());
                    compute_next_fire_at(pattern, &user_tz)
                } else {
                    None
                }
            }
            _ => None,
        }
    } else {
        None
    };

    let expires_at = req
        .expires_in_days
        .map(|days| now + (days * 86400.0) as i32);

    // Validate flow_config depth if provided
    if let Some(ref fc) = req.flow_config {
        match serde_json::from_str::<crate::proactive::rules::FlowNode>(fc) {
            Ok(node) => {
                if node.condition_depth() > 3 {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        Json(json!({ "error": "Flow config exceeds max depth of 3 conditions" })),
                    ));
                }
            }
            Err(e) => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "error": format!("Invalid flow_config: {}", e) })),
                ));
            }
        }
    }

    let new_rule = NewOntRule {
        user_id,
        name: req.name,
        trigger_type: req.trigger_type,
        trigger_config: req.trigger_config,
        logic_type: req.logic_type,
        logic_prompt: req.logic_prompt,
        logic_fetch: req.logic_fetch,
        action_type: req.action_type,
        action_config: req.action_config,
        status: "active".to_string(),
        next_fire_at,
        expires_at,
        created_at: now,
        updated_at: now,
        flow_config: req.flow_config,
    };

    match state.ontology_repository.create_rule(&new_rule) {
        Ok(rule) => Ok(Json(serde_json::to_value(rule).unwrap_or_default())),
        Err(e) => {
            tracing::error!("Failed to create rule: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Failed to create rule" })),
            ))
        }
    }
}

pub async fn get_rule(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(rule_id): Path<i32>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    state
        .ontology_repository
        .get_rule(auth_user.user_id, rule_id)
        .map(|rule| Json(serde_json::to_value(rule).unwrap_or_default()))
        .map_err(|e| {
            tracing::error!("Failed to get rule: {}", e);
            StatusCode::NOT_FOUND
        })
}

pub async fn update_rule(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(rule_id): Path<i32>,
    Json(req): Json<CreateRuleRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let user_id = auth_user.user_id;

    // Verify ownership
    state
        .ontology_repository
        .get_rule(user_id, rule_id)
        .map_err(|_| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "Rule not found" })),
            )
        })?;

    // Validate configs
    let trigger: TriggerConfig = serde_json::from_str(&req.trigger_config).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("Invalid trigger_config: {}", e) })),
        )
    })?;
    let _action: ActionConfig = serde_json::from_str(&req.action_config).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("Invalid action_config: {}", e) })),
        )
    })?;

    // Recompute next_fire_at for schedule rules
    let next_fire_at = if req.trigger_type == "schedule" {
        match trigger.schedule.as_deref() {
            Some("once") => trigger
                .at
                .as_ref()
                .and_then(|at| crate::proactive::utils::parse_iso_to_timestamp(at)),
            Some("recurring") => {
                if let Some(ref pattern) = trigger.pattern {
                    let user_tz = state
                        .user_core
                        .get_user_info(user_id)
                        .ok()
                        .and_then(|info| info.timezone)
                        .unwrap_or_else(|| "UTC".to_string());
                    compute_next_fire_at(pattern, &user_tz)
                } else {
                    None
                }
            }
            _ => None,
        }
    } else {
        None
    };

    // Validate flow_config depth if provided
    if let Some(ref fc) = req.flow_config {
        match serde_json::from_str::<crate::proactive::rules::FlowNode>(fc) {
            Ok(node) => {
                if node.condition_depth() > 3 {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        Json(json!({ "error": "Flow config exceeds max depth of 3 conditions" })),
                    ));
                }
            }
            Err(e) => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(json!({ "error": format!("Invalid flow_config: {}", e) })),
                ));
            }
        }
    }

    match state.ontology_repository.update_rule(
        user_id,
        rule_id,
        &req.name,
        &req.trigger_type,
        &req.trigger_config,
        &req.logic_type,
        req.logic_prompt.as_deref(),
        req.logic_fetch.as_deref(),
        &req.action_type,
        &req.action_config,
        next_fire_at,
        req.flow_config.as_deref(),
    ) {
        Ok(rule) => Ok(Json(serde_json::to_value(rule).unwrap_or_default())),
        Err(e) => {
            tracing::error!("Failed to update rule {}: {}", rule_id, e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Failed to update rule" })),
            ))
        }
    }
}

pub async fn update_rule_status(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(rule_id): Path<i32>,
    Json(req): Json<UpdateRuleStatusRequest>,
) -> Result<StatusCode, StatusCode> {
    // Verify ownership
    state
        .ontology_repository
        .get_rule(auth_user.user_id, rule_id)
        .map_err(|_| StatusCode::NOT_FOUND)?;

    state
        .ontology_repository
        .update_rule_status(rule_id, &req.status)
        .map_err(|e| {
            tracing::error!("Failed to update rule status: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_rule(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(rule_id): Path<i32>,
) -> Result<StatusCode, StatusCode> {
    state
        .ontology_repository
        .delete_rule(auth_user.user_id, rule_id)
        .map_err(|e| {
            tracing::error!("Failed to delete rule: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(StatusCode::NO_CONTENT)
}
