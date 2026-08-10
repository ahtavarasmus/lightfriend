use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::convert::Infallible;
use std::sync::Arc;

use crate::handlers::auth_middleware::AuthUser;
use crate::handlers::dashboard_handlers::Contact;
use crate::models::ontology_models::{NewOntRule, OntRule};
use crate::proactive::rules::{
    compute_next_fire_at, evaluate_flow_test, ActionConfig, FlowNode, RuleTestStep, TriggerConfig,
};
use crate::repositories::user_core::UserCoreOps;
use crate::repositories::user_repository::LogUsageParams;
use crate::AppState;

pub const ALWAYS_SHOW_LOGIC_TYPE: &str = "always_show";

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CreateAlwaysShowRequest {
    Platform {
        contact_id: String,
        #[serde(default)]
        group_mode: Option<String>,
    },
    Email {
        email: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AlwaysShowEntry {
    pub id: i32,
    pub platform: String,
    pub display_name: String,
    pub subtitle: String,
}

pub fn is_always_show_rule(rule: &OntRule) -> bool {
    rule.logic_type == ALWAYS_SHOW_LOGIC_TYPE
}

fn always_show_metadata(rule: &OntRule) -> Option<(String, String, String)> {
    if !is_always_show_rule(rule) {
        return None;
    }
    let config: serde_json::Value = serde_json::from_str(&rule.trigger_config).ok()?;
    Some((
        config.get("always_show_platform")?.as_str()?.to_string(),
        config
            .get("always_show_display_name")?
            .as_str()?
            .to_string(),
        config.get("always_show_identity")?.as_str()?.to_string(),
    ))
}

fn always_show_entry(rule: &OntRule) -> Option<AlwaysShowEntry> {
    let (platform, display_name, _) = always_show_metadata(rule)?;
    let config: serde_json::Value = serde_json::from_str(&rule.trigger_config).ok()?;
    let subtitle = if platform == "email" {
        "Email address".to_string()
    } else if config.get("always_show_is_group").and_then(|v| v.as_bool()) == Some(true)
        && config.get("group_mode").and_then(|v| v.as_str()) == Some("mention_only")
    {
        format!("Mentions only · {}", platform)
    } else {
        format!("Always shown from {}", platform)
    };
    Some(AlwaysShowEntry {
        id: rule.id,
        platform,
        display_name,
        subtitle,
    })
}

struct AlwaysShowRuleInput<'a> {
    user_id: i32,
    platform: &'a str,
    display_name: &'a str,
    identity: &'a str,
    room_id: Option<&'a str>,
    person_id: Option<i32>,
    sender_key: Option<&'a str>,
    is_group: Option<bool>,
    group_mode: Option<&'a str>,
    now: i32,
}

fn build_always_show_rule(input: AlwaysShowRuleInput<'_>) -> NewOntRule {
    let AlwaysShowRuleInput {
        user_id,
        platform,
        display_name,
        identity,
        room_id,
        person_id,
        sender_key,
        is_group,
        group_mode,
        now,
    } = input;
    let mut trigger = json!({
        "entity_type": "Message",
        "change": "created",
        "filters": {
            "sender": display_name,
            "platform": platform,
        },
        "delay_seconds": 0,
        "incoming_only": true,
        "always_show_platform": platform,
        "always_show_display_name": display_name,
        "always_show_identity": identity,
    });
    if let Some(room_id) = room_id {
        trigger["resolved_room_id"] = json!(room_id);
    }
    if let Some(person_id) = person_id {
        trigger["resolved_person_id"] = json!(person_id);
    }
    if let Some(sender_key) = sender_key {
        trigger["resolved_sender_key"] = json!(sender_key);
    }
    if let Some(is_group) = is_group {
        trigger["always_show_is_group"] = json!(is_group);
    }
    if let Some(group_mode) = group_mode {
        trigger["group_mode"] = json!(group_mode);
    }

    let action = json!({ "method": "sms" });
    let flow = json!({
        "type": "action",
        "action_type": "notify",
        "config": action,
    });

    NewOntRule {
        user_id,
        name: format!("Always show: {}", display_name),
        trigger_type: "ontology_change".to_string(),
        trigger_config: trigger.to_string(),
        logic_type: ALWAYS_SHOW_LOGIC_TYPE.to_string(),
        logic_prompt: None,
        logic_fetch: None,
        action_type: "notify".to_string(),
        action_config: action.to_string(),
        status: "active".to_string(),
        next_fire_at: None,
        expires_at: None,
        created_at: now,
        updated_at: now,
        flow_config: Some(flow.to_string()),
    }
}

pub fn build_platform_always_show_rule(
    user_id: i32,
    contact: &Contact,
    now: i32,
) -> Result<NewOntRule, &'static str> {
    build_platform_always_show_rule_with_mode(user_id, contact, None, now)
}

pub fn platform_supports_authoritative_mentions(platform: &str) -> bool {
    // mautrix-whatsapp exposes source-platform tags as Matrix m.mentions.
    // Do not advertise mention-only for bridges whose native-to-Matrix
    // mention mapping has not been verified here.
    platform == "whatsapp"
}

pub fn build_platform_always_show_rule_with_mode(
    user_id: i32,
    contact: &Contact,
    requested_group_mode: Option<&str>,
    now: i32,
) -> Result<NewOntRule, &'static str> {
    let platform = contact
        .platform
        .as_deref()
        .ok_or("Contact has no platform")?;
    if platform == "email" {
        return Err("Use the email entry form for email addresses");
    }
    let group_mode = if contact.is_group {
        let mode = requested_group_mode.unwrap_or("all");
        if mode != "all" && mode != "mention_only" {
            return Err("Choose all messages or mentions only");
        }
        if mode == "mention_only" && !platform_supports_authoritative_mentions(platform) {
            return Err("Mentions-only is not reliably available for this platform");
        }
        Some(mode)
    } else {
        if requested_group_mode.is_some() {
            return Err("Delivery mode is only available for group chats");
        }
        None
    };
    Ok(build_always_show_rule(AlwaysShowRuleInput {
        user_id,
        platform,
        display_name: contact.display_name.trim(),
        identity: &contact.id,
        room_id: contact.room_id.as_deref(),
        person_id: contact.person_id,
        sender_key: None,
        is_group: Some(contact.is_group),
        group_mode,
        now,
    }))
}

pub fn build_email_always_show_rule(
    user_id: i32,
    email: &str,
    now: i32,
) -> Result<NewOntRule, &'static str> {
    let normalized = email.trim().to_lowercase();
    if !crate::handlers::imap_auth::is_valid_email(&normalized) {
        return Err("Enter a valid email address");
    }
    Ok(build_always_show_rule(AlwaysShowRuleInput {
        user_id,
        platform: "email",
        display_name: &normalized,
        identity: &format!("email:{}", normalized),
        room_id: None,
        person_id: None,
        sender_key: Some(&normalized),
        is_group: Some(false),
        group_mode: None,
        now,
    }))
}

pub async fn list_always_show(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<Vec<AlwaysShowEntry>>, StatusCode> {
    state
        .ontology_repository
        .get_rules(auth_user.user_id)
        .map(|rules| {
            Json(
                rules
                    .iter()
                    .filter(|rule| rule.status == "active")
                    .filter_map(always_show_entry)
                    .collect(),
            )
        })
        .map_err(|e| {
            tracing::error!("Failed to list always-show entries: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

pub async fn create_always_show(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(req): Json<CreateAlwaysShowRequest>,
) -> Result<Json<AlwaysShowEntry>, (StatusCode, Json<serde_json::Value>)> {
    let user_id = auth_user.user_id;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i32;

    let new_rule = match req {
        CreateAlwaysShowRequest::Platform {
            contact_id,
            group_mode,
        } => {
            let contact = state
                .rule_builder_contact_cache
                .get(&user_id)
                .and_then(|entry| {
                    entry
                        .value()
                        .1
                        .iter()
                        .find(|contact| contact.id == contact_id)
                        .cloned()
                })
                .ok_or_else(|| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(
                            json!({ "error": "Select a contact or chat from the search results" }),
                        ),
                    )
                })?;
            build_platform_always_show_rule_with_mode(user_id, &contact, group_mode.as_deref(), now)
        }
        CreateAlwaysShowRequest::Email { email } => {
            build_email_always_show_rule(user_id, &email, now)
        }
    }
    .map_err(|message| (StatusCode::BAD_REQUEST, Json(json!({ "error": message }))))?;

    let identity = always_show_metadata_from_new(&new_rule).2;
    let existing = state
        .ontology_repository
        .get_active_rules(user_id)
        .map_err(|e| {
            tracing::error!("Failed to check always-show entries: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Failed to save entry" })),
            )
        })?
        .into_iter()
        .find(|rule| {
            always_show_metadata(rule)
                .is_some_and(|(_, _, existing_identity)| existing_identity == identity)
        });

    let rule = if let Some(existing) = existing {
        // Re-adding the same group is how the compact UI changes its single
        // delivery mode. Update in place so duplicate modes cannot coexist.
        state
            .ontology_repository
            .update_rule(
                user_id,
                existing.id,
                &new_rule.name,
                &new_rule.trigger_type,
                &new_rule.trigger_config,
                &new_rule.logic_type,
                new_rule.logic_prompt.as_deref(),
                new_rule.logic_fetch.as_deref(),
                &new_rule.action_type,
                &new_rule.action_config,
                new_rule.next_fire_at,
                new_rule.flow_config.as_deref(),
            )
            .map_err(|e| {
                tracing::error!("Failed to update always-show entry: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": "Failed to save entry" })),
                )
            })?
    } else {
        state
            .ontology_repository
            .create_rule(&new_rule)
            .map_err(|e| {
                tracing::error!("Failed to create always-show entry: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": "Failed to save entry" })),
                )
            })?
    };

    Ok(Json(
        always_show_entry(&rule).expect("new always-show rule has metadata"),
    ))
}

fn always_show_metadata_from_new(rule: &NewOntRule) -> (String, String, String) {
    let config: serde_json::Value =
        serde_json::from_str(&rule.trigger_config).expect("always-show trigger is valid JSON");
    (
        config["always_show_platform"].as_str().unwrap().to_string(),
        config["always_show_display_name"]
            .as_str()
            .unwrap()
            .to_string(),
        config["always_show_identity"].as_str().unwrap().to_string(),
    )
}

pub async fn delete_always_show(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(rule_id): Path<i32>,
) -> Result<StatusCode, StatusCode> {
    let rule = state
        .ontology_repository
        .get_rule(auth_user.user_id, rule_id)
        .map_err(|_| StatusCode::NOT_FOUND)?;
    if !is_always_show_rule(&rule) {
        return Err(StatusCode::NOT_FOUND);
    }
    state
        .ontology_repository
        .delete_rule(auth_user.user_id, rule_id)
        .map_err(|e| {
            tracing::error!("Failed to delete always-show entry: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(StatusCode::NO_CONTENT)
}

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

    // Rules require autopilot or byot plan
    let user = state
        .user_core
        .find_by_id(user_id)
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Failed to fetch user" })),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "User not found" })),
            )
        })?;
    if !crate::utils::plan_features::has_auto_features(user.plan_type.as_deref()) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Rules require Autopilot plan" })),
        ));
    }

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
    let tz_offset = crate::proactive::utils::user_tz_offset_secs(&state, user_id);
    let next_fire_at = if req.trigger_type == "schedule" {
        match trigger.schedule.as_deref() {
            Some("once") => trigger
                .at
                .as_ref()
                .and_then(|at| crate::proactive::utils::parse_iso_to_timestamp(at, tz_offset)),
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

    // Rules require autopilot or byot plan
    let user = state
        .user_core
        .find_by_id(user_id)
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "Failed to fetch user" })),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "User not found" })),
            )
        })?;
    if !crate::utils::plan_features::has_auto_features(user.plan_type.as_deref()) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Rules require Autopilot plan" })),
        ));
    }

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
    let tz_offset = crate::proactive::utils::user_tz_offset_secs(&state, user_id);
    let next_fire_at = if req.trigger_type == "schedule" {
        match trigger.schedule.as_deref() {
            Some("once") => trigger
                .at
                .as_ref()
                .and_then(|at| crate::proactive::utils::parse_iso_to_timestamp(at, tz_offset)),
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

    let charged_amount = if crate::services::metronome_billing::metronome_enabled() {
        let entitled = crate::services::metronome_billing::has_usage_entitlement(&state, user.id)
            .await
            .map_err(|error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": format!("Billing check failed: {}", error) })),
                )
            })?;
        if !entitled {
            return Err((
                StatusCode::PAYMENT_REQUIRED,
                Json(
                    json!({ "error": "Included usage depleted. Enable overage billing to continue." }),
                ),
            ));
        }
        crate::services::metronome_billing::enqueue_usage(
            &state,
            user.id,
            "rule_test",
            WEB_CHAT_COST_EUR,
            None,
        )
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to queue usage: {}", error) })),
            )
        })?;
        WEB_CHAT_COST_EUR
    } else {
        let user = crate::utils::usage::ensure_current_included_usage_window(&state, &user)
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": e })),
                )
            })?;
        let credits_left_cost = if user.phone_number.starts_with("+1") {
            WEB_CHAT_COST_US
        } else {
            WEB_CHAT_COST_EUR
        };
        if user.credits_left >= credits_left_cost {
            state
                .user_repository
                .update_user_credits_left(user.id, user.credits_left - credits_left_cost)
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({ "error": format!("Credit deduction failed: {}", e) })),
                    )
                })?;
            credits_left_cost
        } else if user.credits >= WEB_CHAT_COST_EUR {
            state
                .user_repository
                .update_user_credits(user.id, user.credits - WEB_CHAT_COST_EUR)
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({ "error": format!("Credit deduction failed: {}", e) })),
                    )
                })?;
            WEB_CHAT_COST_EUR
        } else {
            return Err((
                StatusCode::PAYMENT_REQUIRED,
                Json(json!({ "error": "Insufficient credits" })),
            ));
        }
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
