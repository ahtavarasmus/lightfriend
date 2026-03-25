use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use std::convert::Infallible;
use std::sync::Arc;

use crate::handlers::auth_middleware::AuthUser;
use crate::models::ontology_models::{NewOntRule, OntRule};
use crate::proactive::rules::{
    compute_next_fire_at, evaluate_flow_test, ActionConfig, FlowNode, RuleTestStep, TriggerConfig,
};
use crate::repositories::user_core::UserCoreOps;
use crate::repositories::user_repository::LogUsageParams;
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

// ---------------------------------------------------------------------------
// Rule test endpoints
// ---------------------------------------------------------------------------

const WEB_CHAT_COST_EUR: f32 = 0.01;
const WEB_CHAT_COST_US: f32 = 0.5;

#[derive(Deserialize)]
pub struct StartRuleTestRequest {
    pub flow_config: String,
    pub message: String,
    #[serde(default = "default_sender")]
    pub sender: String,
    #[serde(default)]
    pub rule_name: String,
}

fn default_sender() -> String {
    "Test Sender".to_string()
}

pub struct PendingRuleTest {
    pub flow_config: String,
    pub message: String,
    pub sender: String,
    pub rule_name: String,
    pub user_id: i32,
    pub created_at: std::time::Instant,
}

/// POST /api/rules/test - validate flow, deduct credits, store pending test
pub async fn start_rule_test(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(req): Json<StartRuleTestRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Validate flow_config parses
    let _node: FlowNode = serde_json::from_str(&req.flow_config).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("Invalid flow_config: {}", e) })),
        )
    })?;

    // Check user & credits
    let user = state
        .user_core
        .find_by_id(auth_user.user_id)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("DB error: {}", e) })),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "User not found" })),
            )
        })?;

    if user.sub_tier.is_none() {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Please subscribe to use rule testing" })),
        ));
    }

    let is_us_or_ca = user.phone_number.starts_with("+1");
    let (credits_left_cost, credits_cost) = if is_us_or_ca {
        (WEB_CHAT_COST_US, WEB_CHAT_COST_EUR)
    } else {
        (WEB_CHAT_COST_EUR, WEB_CHAT_COST_EUR)
    };

    let has_credits = user.credits_left >= credits_left_cost || user.credits >= credits_cost;
    if !has_credits {
        return Err((
            StatusCode::PAYMENT_REQUIRED,
            Json(json!({ "error": "Insufficient credits" })),
        ));
    }

    // Deduct
    let charged_amount = if user.credits_left >= credits_left_cost {
        let new_val = user.credits_left - credits_left_cost;
        state
            .user_repository
            .update_user_credits_left(auth_user.user_id, new_val)
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": format!("Credit deduction failed: {}", e) })),
                )
            })?;
        credits_left_cost
    } else {
        let new_val = user.credits - credits_cost;
        state
            .user_repository
            .update_user_credits(auth_user.user_id, new_val)
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": format!("Credit deduction failed: {}", e) })),
                )
            })?;
        credits_cost
    };

    let _ = state.user_repository.log_usage(LogUsageParams {
        user_id: auth_user.user_id,
        sid: None,
        activity_type: "rule_test".to_string(),
        credits: Some(charged_amount),
        time_consumed: None,
        success: Some(true),
        reason: None,
        status: None,
        recharge_threshold_timestamp: None,
        zero_credits_timestamp: None,
    });

    let test_id = uuid::Uuid::new_v4().to_string();
    state.pending_rule_tests.insert(
        test_id.clone(),
        PendingRuleTest {
            flow_config: req.flow_config,
            message: req.message,
            sender: req.sender,
            rule_name: req.rule_name,
            user_id: auth_user.user_id,
            created_at: std::time::Instant::now(),
        },
    );

    Ok(Json(json!({ "test_id": test_id })))
}

#[derive(Deserialize)]
pub struct TestStreamQuery {
    pub test_id: String,
}

/// GET /api/rules/test-stream?test_id=... - SSE stream of test steps
pub async fn test_rule_stream(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Query(query): Query<TestStreamQuery>,
) -> axum::response::sse::Sse<
    impl futures::stream::Stream<Item = Result<axum::response::sse::Event, Infallible>>,
> {
    let stream = async_stream::stream! {
        // Look up and remove pending test
        let pending = match state.pending_rule_tests.remove(&query.test_id) {
            Some((_, p)) => p,
            None => {
                yield Ok(axum::response::sse::Event::default().data(
                    json!({"step": "error", "message": "Test not found or expired"}).to_string(),
                ));
                yield Ok(axum::response::sse::Event::default().data(
                    json!({"step": "complete"}).to_string(),
                ));
                return;
            }
        };

        // Verify ownership
        if pending.user_id != auth_user.user_id {
            yield Ok(axum::response::sse::Event::default().data(
                json!({"step": "error", "message": "Unauthorized"}).to_string(),
            ));
            yield Ok(axum::response::sse::Event::default().data(
                json!({"step": "complete"}).to_string(),
            ));
            return;
        }

        // Check TTL (60s)
        if pending.created_at.elapsed().as_secs() > 60 {
            yield Ok(axum::response::sse::Event::default().data(
                json!({"step": "error", "message": "Test expired"}).to_string(),
            ));
            yield Ok(axum::response::sse::Event::default().data(
                json!({"step": "complete"}).to_string(),
            ));
            return;
        }

        // Parse flow
        let root: FlowNode = match serde_json::from_str(&pending.flow_config) {
            Ok(n) => n,
            Err(e) => {
                yield Ok(axum::response::sse::Event::default().data(
                    json!({"step": "error", "message": format!("Invalid flow: {}", e)}).to_string(),
                ));
                yield Ok(axum::response::sse::Event::default().data(
                    json!({"step": "complete"}).to_string(),
                ));
                return;
            }
        };

        let trigger_context = format!("Message from {}: {}", pending.sender, pending.message);

        // Build a synthetic OntRule
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i32;
        let rule = OntRule {
            id: 0,
            user_id: pending.user_id,
            name: if pending.rule_name.is_empty() { "Test Rule".to_string() } else { pending.rule_name },
            trigger_type: "test".to_string(),
            trigger_config: "{}".to_string(),
            logic_type: "flow".to_string(),
            logic_prompt: None,
            logic_fetch: None,
            action_type: "test".to_string(),
            action_config: "{}".to_string(),
            status: "active".to_string(),
            next_fire_at: None,
            expires_at: None,
            last_triggered_at: None,
            created_at: now,
            updated_at: now,
            flow_config: Some(pending.flow_config),
        };

        // Run evaluation with mpsc channel
        let (tx, mut rx) = tokio::sync::mpsc::channel::<RuleTestStep>(32);
        let state_clone = Arc::clone(&state);
        tokio::spawn(async move {
            evaluate_flow_test(&state_clone, &rule, &trigger_context, &root, &tx).await;
            let _ = tx.send(RuleTestStep::Complete).await;
        });

        while let Some(step) = rx.recv().await {
            let data = serde_json::to_string(&step).unwrap_or_default();
            yield Ok(axum::response::sse::Event::default().data(data));
        }
    };

    axum::response::sse::Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default())
}
