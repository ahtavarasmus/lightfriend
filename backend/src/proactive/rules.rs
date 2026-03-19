//! Rule evaluation engine: flow-based evaluation tree
//!
//! Every rule has a `flow_config` JSON column containing a FlowNode tree.
//! Node types: llm_condition, keyword_condition, action.
//! The engine recursively walks the tree, evaluating conditions and executing actions.

use std::sync::Arc;

use chrono::Datelike;
use openai_api_rs::v1::{chat_completion, types};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{error, info, warn};

use crate::context::ContextBuilder;
use crate::models::ontology_models::OntRule;
use crate::proactive::utils::send_notification;
use crate::repositories::user_core::UserCoreOps;
use crate::AppState;

// ---------------------------------------------------------------------------
// FetchSource: typed prefetch source configuration
// ---------------------------------------------------------------------------

fn default_platform() -> String {
    "all".to_string()
}
fn default_limit() -> i64 {
    50
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FetchSource {
    Email,
    Chat {
        #[serde(default = "default_platform")]
        platform: String,
        #[serde(default = "default_limit")]
        limit: i64,
    },
    Weather {
        #[serde(default)]
        location: String,
    },
    Internet {
        query: String,
    },
    Tesla,
    Mcp {
        server: String,
        tool: String,
        #[serde(default)]
        args: String,
    },
    Pinned,
}

// ---------------------------------------------------------------------------
// FlowNode: the evaluation tree
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FlowNode {
    LlmCondition {
        prompt: String,
        #[serde(default)]
        fetch: Vec<FetchSource>,
        true_branch: Box<Option<FlowNode>>,
        false_branch: Box<Option<FlowNode>>,
    },
    KeywordCondition {
        keyword: String,
        true_branch: Box<Option<FlowNode>>,
        false_branch: Box<Option<FlowNode>>,
    },
    Action {
        action_type: String,
        config: serde_json::Value,
    },
}

impl FlowNode {
    /// Returns the depth of the deepest condition chain (actions don't count).
    pub fn condition_depth(&self) -> usize {
        match self {
            FlowNode::LlmCondition {
                true_branch,
                false_branch,
                ..
            }
            | FlowNode::KeywordCondition {
                true_branch,
                false_branch,
                ..
            } => {
                let t = true_branch
                    .as_ref()
                    .as_ref()
                    .map(|n| n.condition_depth())
                    .unwrap_or(0);
                let f = false_branch
                    .as_ref()
                    .as_ref()
                    .map(|n| n.condition_depth())
                    .unwrap_or(0);
                1 + t.max(f)
            }
            FlowNode::Action { .. } => 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Config types (deserialized from JSON columns)
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct TriggerConfig {
    // For ontology_change triggers
    pub entity_type: Option<String>,
    pub change: Option<String>,
    pub filters: Option<HashMap<String, String>>,
    // For schedule triggers
    pub schedule: Option<String>,
    pub pattern: Option<String>,
    pub at: Option<String>,
    // Common: fire once then auto-complete
    #[serde(default)]
    pub fire_once: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ActionConfig {
    pub method: Option<String>,
    pub message: Option<String>,
    pub tool: Option<String>,
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct LogicResult {
    should_act: bool,
    message: Option<String>,
    #[serde(flatten)]
    extra: HashMap<String, serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Trigger matching
// ---------------------------------------------------------------------------

/// Check if an ontology change event matches a rule's trigger config.
pub fn matches_trigger(
    rule: &OntRule,
    entity_type: &str,
    change_type: &str,
    entity_snapshot: &serde_json::Value,
) -> bool {
    let config: TriggerConfig = match serde_json::from_str(&rule.trigger_config) {
        Ok(c) => c,
        Err(_) => return false,
    };

    // Must match entity_type
    if let Some(ref expected_type) = config.entity_type {
        if !expected_type.eq_ignore_ascii_case(entity_type) {
            return false;
        }
    } else {
        return false;
    }

    // Must match change type
    if let Some(ref expected_change) = config.change {
        if !expected_change.eq_ignore_ascii_case(change_type) {
            return false;
        }
    }

    // Check filters against entity snapshot
    if let Some(ref filters) = config.filters {
        for (key, expected_value) in filters {
            let actual = entity_snapshot
                .get(key)
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if !actual
                .to_lowercase()
                .contains(&expected_value.to_lowercase())
            {
                return false;
            }
        }
    }

    true
}

// ---------------------------------------------------------------------------
// Rule evaluation entry point
// ---------------------------------------------------------------------------

/// Evaluate a rule's flow_config tree and execute actions.
pub async fn evaluate_and_execute(
    state: &Arc<AppState>,
    rule: &OntRule,
    trigger_context: &str,
    trigger_snapshot: Option<&serde_json::Value>,
) {
    // Mark as triggered
    let _ = state
        .ontology_repository
        .update_rule_last_triggered(rule.id);

    // Check expiry
    if let Some(expires) = rule.expires_at {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i32;
        if now > expires {
            let _ = state
                .ontology_repository
                .update_rule_status(rule.id, "expired");
            return;
        }
    }

    // All rules use flow_config
    if let Some(ref flow_json) = rule.flow_config {
        match serde_json::from_str::<FlowNode>(flow_json) {
            Ok(root) => {
                if let Err(e) = evaluate_flow(
                    state,
                    rule,
                    trigger_context,
                    trigger_snapshot,
                    &root,
                    None,
                    None,
                )
                .await
                {
                    error!("Rule {} flow evaluation failed: {}", rule.id, e);
                }
            }
            Err(e) => {
                error!("Rule {} has invalid flow_config JSON: {}", rule.id, e);
            }
        }
    } else {
        warn!("Rule {} has no flow_config, skipping", rule.id);
    }

    // For one-shot rules (schedule "once" or fire_once flag), mark completed
    let trigger: TriggerConfig = serde_json::from_str(&rule.trigger_config).unwrap_or_default();
    if trigger.schedule.as_deref() == Some("once") || trigger.fire_once {
        let _ = state
            .ontology_repository
            .update_rule_status(rule.id, "completed");
    }
}

// ---------------------------------------------------------------------------
// Recursive flow evaluation
// ---------------------------------------------------------------------------

/// Walk the flow tree recursively.
/// `prev_message` carries the LLM-generated message from a parent condition
/// so that downstream action nodes can use it.
/// `prev_extras` carries extra LLM-generated params for tool_call actions.
async fn evaluate_flow(
    state: &Arc<AppState>,
    rule: &OntRule,
    trigger_context: &str,
    trigger_snapshot: Option<&serde_json::Value>,
    node: &FlowNode,
    prev_message: Option<&str>,
    prev_extras: Option<&HashMap<String, serde_json::Value>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match node {
        FlowNode::LlmCondition {
            prompt,
            fetch,
            true_branch,
            false_branch,
        } => {
            // Peek at true_branch to extract tool params for LLM
            let extra_params = extract_tool_params(state, true_branch.as_ref());
            let prefetched = prefetch_sources(state, rule, fetch).await;
            let result = call_llm_condition(
                state,
                rule,
                trigger_context,
                prompt,
                &prefetched,
                extra_params.as_ref(),
            )
            .await?;
            let (next, msg, extras) = if result.should_act {
                let extras = if result.extra.is_empty() {
                    None
                } else {
                    Some(result.extra)
                };
                (
                    true_branch,
                    result.message.as_deref().map(|s| s.to_string()),
                    extras,
                )
            } else {
                (false_branch, None, None)
            };
            if let Some(branch) = next.as_ref() {
                let effective_msg = msg.as_deref().or(prev_message);
                let effective_extras = extras.as_ref().or(prev_extras);
                Box::pin(evaluate_flow(
                    state,
                    rule,
                    trigger_context,
                    trigger_snapshot,
                    branch,
                    effective_msg,
                    effective_extras,
                ))
                .await?;
            }
        }
        FlowNode::KeywordCondition {
            keyword,
            true_branch,
            false_branch,
        } => {
            let matched = !keyword.is_empty()
                && trigger_context
                    .to_lowercase()
                    .contains(&keyword.to_lowercase());
            let next = if matched { true_branch } else { false_branch };
            if let Some(branch) = next.as_ref() {
                Box::pin(evaluate_flow(
                    state,
                    rule,
                    trigger_context,
                    trigger_snapshot,
                    branch,
                    prev_message,
                    prev_extras,
                ))
                .await?;
            } else if matched {
                info!(
                    "Rule {} ({}): keyword matched but no true_branch",
                    rule.id, rule.name
                );
            }
        }
        FlowNode::Action {
            action_type,
            config,
        } => {
            let message = prev_message.unwrap_or(trigger_context);
            execute_flow_action(
                state,
                rule,
                trigger_snapshot,
                action_type,
                config,
                message,
                prev_extras,
            )
            .await;
        }
    }
    Ok(())
}

/// Peek at a branch node: if it's a tool_call Action, look up the tool's
/// parameter schema and return params that the LLM should fill (excluding
/// auto-injected ones).
fn extract_tool_params(
    state: &Arc<AppState>,
    branch: &Option<FlowNode>,
) -> Option<HashMap<String, Box<types::JSONSchemaDefine>>> {
    let node = branch.as_ref()?;
    if let FlowNode::Action {
        action_type,
        config,
    } = node
    {
        if action_type == "tool_call" {
            let tool_name = config.get("tool")?.as_str()?;
            let handler = state.tool_registry.get(tool_name)?;
            let auto_injected = handler.auto_injected_params();
            let def = handler.definition();
            let mut props = def.function.parameters.properties?;
            for key in &auto_injected {
                props.remove(*key);
            }
            if props.is_empty() {
                return None;
            }
            return Some(props);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Prefetch sources helper
// ---------------------------------------------------------------------------

async fn prefetch_sources(
    state: &Arc<AppState>,
    rule: &OntRule,
    sources: &[FetchSource],
) -> String {
    let mut prefetched = String::new();
    for source in sources {
        match source {
            FetchSource::Email => {
                let emails =
                    crate::tool_call_utils::email::handle_fetch_emails(state, rule.user_id).await;
                if !emails.is_empty() {
                    prefetched.push_str(&format!("\n\n--- Recent emails ---\n{}", emails));
                }
            }
            FetchSource::Chat { platform, limit } => {
                let twelve_hours_ago = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i32
                    - 43200;
                let plat = if platform == "all" {
                    None
                } else {
                    Some(platform.as_str())
                };
                let messages = state.ontology_repository.get_recent_messages_filtered(
                    rule.user_id,
                    plat,
                    twelve_hours_ago,
                    *limit,
                );
                if let Ok(msgs) = messages {
                    let formatted: Vec<String> = msgs
                        .iter()
                        .map(|m| format!("[{}] {}: {}", m.platform, m.sender_name, m.content))
                        .collect();
                    prefetched.push_str(&format!(
                        "\n\n--- Recent chat messages ---\n{}",
                        formatted.join("\n")
                    ));
                }
            }
            FetchSource::Weather { location } => {
                let loc = if !location.is_empty() {
                    location.clone()
                } else {
                    state
                        .user_core
                        .get_user_info(rule.user_id)
                        .ok()
                        .and_then(|i| i.location)
                        .unwrap_or_else(|| "unknown".to_string())
                };
                match crate::utils::tool_exec::get_weather(
                    state,
                    &loc,
                    "metric",
                    "current",
                    rule.user_id,
                )
                .await
                {
                    Ok(weather) => {
                        prefetched.push_str(&format!("\n\n--- Current weather ---\n{}", weather));
                    }
                    Err(e) => {
                        warn!("Rule {} weather fetch failed: {}", rule.id, e);
                    }
                }
            }
            FetchSource::Internet { query } => {
                match crate::utils::tool_exec::ask_perplexity(
                    state,
                    query,
                    "Return factual information concisely.",
                )
                .await
                {
                    Ok(result) => {
                        prefetched.push_str(&format!(
                            "\n\n--- Internet search: {} ---\n{}",
                            query, result
                        ));
                    }
                    Err(e) => {
                        warn!("Rule {} internet fetch failed: {}", rule.id, e);
                    }
                }
            }
            FetchSource::Tesla => {
                let result = crate::tool_call_utils::tesla::handle_tesla_command(
                    state,
                    rule.user_id,
                    r#"{"command":"charge_status"}"#,
                    true,
                )
                .await;
                if !result.is_empty() {
                    prefetched.push_str(&format!("\n\n--- Tesla status ---\n{}", result));
                }
            }
            FetchSource::Mcp { server, tool, args } => {
                let tool_name = format!("mcp:{}:{}", server, tool);
                let args_str = if args.is_empty() { "{}" } else { args };
                let result = crate::tool_call_utils::mcp::handle_mcp_tool_call(
                    state,
                    rule.user_id,
                    &tool_name,
                    args_str,
                )
                .await;
                if !result.is_empty() {
                    prefetched
                        .push_str(&format!("\n\n--- MCP {}:{} ---\n{}", server, tool, result));
                }
            }
            FetchSource::Pinned => {
                let pinned = state
                    .ontology_repository
                    .get_pinned_messages(rule.user_id)
                    .unwrap_or_default();
                if !pinned.is_empty() {
                    let formatted: Vec<String> = pinned
                        .iter()
                        .map(|m| {
                            let status_tag = m
                                .status
                                .as_deref()
                                .map(|s| format!(" [status={}]", s))
                                .unwrap_or_default();
                            format!(
                                "[id={}] [{}]{} {}: {}",
                                m.id, m.platform, status_tag, m.sender_name, m.content
                            )
                        })
                        .collect();
                    prefetched.push_str(&format!(
                        "\n\n--- Tracked/pinned items ---\n{}",
                        formatted.join("\n")
                    ));
                }
            }
        }
    }
    prefetched
}

// ---------------------------------------------------------------------------
// LLM condition evaluation
// ---------------------------------------------------------------------------

async fn call_llm_condition(
    state: &Arc<AppState>,
    rule: &OntRule,
    trigger_context: &str,
    prompt: &str,
    prefetched: &str,
    extra_tool_params: Option<&HashMap<String, Box<types::JSONSchemaDefine>>>,
) -> Result<LogicResult, Box<dyn std::error::Error + Send + Sync>> {
    let ctx = ContextBuilder::for_user(state, rule.user_id)
        .with_user_context()
        .build()
        .await?;

    let tz_info = ctx
        .timezone
        .as_ref()
        .map(|t| format!("Current time: {}", t.formatted_now))
        .unwrap_or_default();

    let extra_instructions = if extra_tool_params.is_some() {
        " When should_act=true, also fill in the additional tool parameter fields."
    } else {
        ""
    };
    let system_prompt = format!(
        "You are evaluating whether to notify a user based on a rule they set up.\n\
        Rule: \"{}\"\n\
        Instructions: {}\n\
        {}\n\n\
        Call the `rule_result` tool with your decision. Set should_act=true if the user should be notified, \
        and provide the notification `message` (used as SMS text to the user). Set should_act=false if conditions aren't met.{}\n\
        Keep messages concise (max 480 chars), direct, second person.",
        rule.name, prompt, tz_info, extra_instructions
    );

    let user_msg = format!(
        "Trigger event:\n{}\n{}",
        trigger_context,
        if prefetched.is_empty() {
            String::new()
        } else {
            prefetched.to_string()
        }
    );

    let messages = vec![
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::system,
            content: chat_completion::Content::Text(system_prompt),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::user,
            content: chat_completion::Content::Text(user_msg),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
    ];

    let mut result_properties = HashMap::new();
    result_properties.insert(
        "should_act".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Boolean),
            description: Some("true if the user should be notified".to_string()),
            ..Default::default()
        }),
    );
    result_properties.insert(
        "message".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "Notification message for the user (max 480 chars, second person). This is sent as SMS to the user.".to_string(),
            ),
            ..Default::default()
        }),
    );

    // Merge extra tool params (optional fields the LLM should fill when should_act=true)
    if let Some(extra) = extra_tool_params {
        for (key, schema) in extra {
            result_properties.insert(key.clone(), schema.clone());
        }
    }

    let tool = chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: "rule_result".to_string(),
            description: Some("Return evaluation result".to_string()),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(result_properties),
                required: Some(vec!["should_act".to_string(), "message".to_string()]),
            },
        },
    };

    let request = chat_completion::ChatCompletionRequest::new(ctx.model.clone(), messages)
        .tools(vec![tool])
        .tool_choice(chat_completion::ToolChoiceType::Required)
        .temperature(0.0);

    let result = ctx
        .client
        .chat_completion(request)
        .await
        .map_err(|e| format!("LLM call failed: {}", e))?;

    let choice = result.choices.first().ok_or("No choices in LLM response")?;

    if let Some(ref tool_calls) = choice.message.tool_calls {
        for tc in tool_calls {
            let fn_name = tc.function.name.as_deref().unwrap_or("");
            if fn_name == "rule_result" {
                let args = tc.function.arguments.as_deref().unwrap_or("{}");
                let parsed: LogicResult = serde_json::from_str(args)
                    .map_err(|e| format!("Failed to parse rule_result: {}", e))?;
                return Ok(parsed);
            }
        }
    }

    // Default: don't act
    Ok(LogicResult {
        should_act: false,
        message: None,
        extra: HashMap::new(),
    })
}

// ---------------------------------------------------------------------------
// Flow action execution
// ---------------------------------------------------------------------------

async fn execute_flow_action(
    state: &Arc<AppState>,
    rule: &OntRule,
    trigger_snapshot: Option<&serde_json::Value>,
    action_type: &str,
    config: &serde_json::Value,
    message: &str,
    extras: Option<&HashMap<String, serde_json::Value>>,
) {
    match action_type {
        "notify" => {
            let method = config
                .get("method")
                .and_then(|v| v.as_str())
                .unwrap_or("sms");
            let content_type = format!("rule_{}", method);
            send_notification(state, rule.user_id, message, content_type, None).await;
            info!(
                "Rule {} ({}): sent {} notification",
                rule.id, rule.name, method
            );
        }
        "tool_call" => {
            let tool_name = match config.get("tool").and_then(|v| v.as_str()) {
                Some(t) => t,
                None => {
                    warn!("Rule {} action has no tool specified", rule.id);
                    return;
                }
            };
            if let Some(handler) = state.tool_registry.get(tool_name) {
                let user = match state.user_core.find_by_id(rule.user_id) {
                    Ok(Some(u)) => u,
                    Ok(None) => {
                        error!(
                            "Rule {} references non-existent user {}",
                            rule.id, rule.user_id
                        );
                        return;
                    }
                    Err(e) => {
                        error!("Rule {} user lookup failed: {}", rule.id, e);
                        return;
                    }
                };
                let params = {
                    let mut p = config
                        .get("params")
                        .cloned()
                        .unwrap_or_else(|| serde_json::json!({}));
                    // Merge LLM-generated extra params first (lower priority)
                    if let Some(ex) = extras {
                        if let Some(obj) = p.as_object_mut() {
                            for (k, v) in ex {
                                obj.insert(k.clone(), v.clone());
                            }
                        }
                    }
                    // Auto-injected params override LLM values (higher priority)
                    if tool_name == "respond_to_email" {
                        if let Some(snapshot) = trigger_snapshot {
                            if let Some(room_id) = snapshot.get("room_id").and_then(|v| v.as_str())
                            {
                                if let Some(email_uid) = room_id.strip_prefix("email_") {
                                    p["email_id"] = serde_json::json!(email_uid);
                                }
                            }
                        }
                    }
                    if tool_name == "pin_message" {
                        if let Some(snapshot) = trigger_snapshot {
                            if let Some(mid) = snapshot.get("message_id").and_then(|v| v.as_i64()) {
                                p["message_id"] = serde_json::json!(mid);
                            }
                        }
                    }
                    p.to_string()
                };
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i32;
                let tool_ctx = crate::tools::registry::ToolContext {
                    state,
                    user: &user,
                    user_id: rule.user_id,
                    arguments: &params,
                    image_url: None,
                    tool_call_id: format!("rule_{}", rule.id),
                    user_given_info: "",
                    current_time: now,
                    client: None,
                    model: None,
                    tools: None,
                    completion_messages: None,
                    assistant_content: None,
                    tool_call: None,
                };
                match handler.execute(tool_ctx).await {
                    Ok(_result) => {
                        info!(
                            "Rule {} ({}): tool '{}' executed successfully",
                            rule.id, rule.name, tool_name
                        );
                        // For update_tracked_item, send the LLM's message as SMS notification
                        if tool_name == "update_tracked_item" && !message.is_empty() {
                            send_notification(
                                state,
                                rule.user_id,
                                message,
                                "rule_sms".to_string(),
                                None,
                            )
                            .await;
                        }
                    }
                    Err(e) => {
                        error!(
                            "Rule {} ({}): tool '{}' execution failed: {}",
                            rule.id, rule.name, tool_name, e
                        );
                    }
                }
            } else {
                warn!(
                    "Rule {} ({}): tool '{}' not found in registry",
                    rule.id, rule.name, tool_name
                );
            }
        }
        other => {
            warn!("Unknown action_type '{}' for rule {}", other, rule.id);
        }
    }
}

// ---------------------------------------------------------------------------
// Schedule computation
// ---------------------------------------------------------------------------

/// Compute the next fire timestamp (UTC) for a recurring schedule pattern.
/// Pattern formats: "daily HH:MM", "weekdays HH:MM", "weekly DAY HH:MM", "hourly"
pub fn compute_next_fire_at(pattern: &str, user_tz: &str) -> Option<i32> {
    let tz: chrono_tz::Tz = user_tz.parse().unwrap_or(chrono_tz::UTC);
    let now = chrono::Utc::now().with_timezone(&tz);

    let parts: Vec<&str> = pattern.splitn(2, ' ').collect();
    if parts.is_empty() {
        return None;
    }

    let frequency = parts[0];

    match frequency {
        "hourly" => {
            let next = now + chrono::Duration::hours(1);
            Some(next.with_timezone(&chrono::Utc).timestamp() as i32)
        }
        "daily" => {
            let rest = parts.get(1)?;
            let (hour, minute) = parse_hhmm(rest)?;
            let today = now.date_naive().and_hms_opt(hour, minute, 0)?;
            let next = if today > now.naive_local() {
                today
            } else {
                now.date_naive().succ_opt()?.and_hms_opt(hour, minute, 0)?
            };
            let local = next.and_local_timezone(tz).earliest()?;
            Some(local.with_timezone(&chrono::Utc).timestamp() as i32)
        }
        "weekdays" => {
            let rest = parts.get(1)?;
            let (hour, minute) = parse_hhmm(rest)?;
            let mut candidate = now.date_naive();
            if let Some(today_time) = candidate.and_hms_opt(hour, minute, 0) {
                if today_time > now.naive_local() && candidate.weekday().num_days_from_monday() < 5
                {
                    let local = today_time.and_local_timezone(tz).earliest()?;
                    return Some(local.with_timezone(&chrono::Utc).timestamp() as i32);
                }
            }
            for _ in 0..7 {
                candidate = candidate.succ_opt()?;
                if candidate.weekday().num_days_from_monday() < 5 {
                    let next = candidate.and_hms_opt(hour, minute, 0)?;
                    let local = next.and_local_timezone(tz).earliest()?;
                    return Some(local.with_timezone(&chrono::Utc).timestamp() as i32);
                }
            }
            None
        }
        "weekly" => {
            let rest = parts.get(1)?;
            let sub_parts: Vec<&str> = rest.splitn(2, ' ').collect();
            if sub_parts.len() < 2 {
                return None;
            }
            let day_name = sub_parts[0].to_lowercase();
            let (hour, minute) = parse_hhmm(sub_parts[1])?;

            let target_weekday = match day_name.as_str() {
                "monday" | "mon" => chrono::Weekday::Mon,
                "tuesday" | "tue" => chrono::Weekday::Tue,
                "wednesday" | "wed" => chrono::Weekday::Wed,
                "thursday" | "thu" => chrono::Weekday::Thu,
                "friday" | "fri" => chrono::Weekday::Fri,
                "saturday" | "sat" => chrono::Weekday::Sat,
                "sunday" | "sun" => chrono::Weekday::Sun,
                _ => return None,
            };

            let mut candidate = now.date_naive();
            for _ in 0..8 {
                if candidate.weekday() == target_weekday {
                    let next = candidate.and_hms_opt(hour, minute, 0)?;
                    if next > now.naive_local() {
                        let local = next.and_local_timezone(tz).earliest()?;
                        return Some(local.with_timezone(&chrono::Utc).timestamp() as i32);
                    }
                }
                candidate = candidate.succ_opt()?;
            }
            None
        }
        _ => None,
    }
}

fn parse_hhmm(s: &str) -> Option<(u32, u32)> {
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() >= 2 {
        let hour: u32 = parts[0].parse().ok()?;
        let minute: u32 = parts[1].parse().ok()?;
        Some((hour, minute))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Ontology change signal
// ---------------------------------------------------------------------------

/// Called after every ontology mutation. Finds matching rules and evaluates them.
pub async fn emit_ontology_change(
    state: &Arc<AppState>,
    user_id: i32,
    entity_type: &str,
    entity_id: i32,
    change_type: &str,
    entity_snapshot: serde_json::Value,
) {
    let rules = match state.ontology_repository.get_ontology_change_rules(user_id) {
        Ok(r) => r,
        Err(e) => {
            error!(
                "Failed to load ontology_change rules for user {}: {}",
                user_id, e
            );
            return;
        }
    };

    let trigger_context = format!(
        "{} {} (id={}): {}",
        entity_type,
        change_type,
        entity_id,
        serde_json::to_string(&entity_snapshot).unwrap_or_default()
    );

    for rule in rules {
        if matches_trigger(&rule, entity_type, change_type, &entity_snapshot) {
            let state = Arc::clone(state);
            let rule = rule.clone();
            let ctx = trigger_context.clone();
            let snap = entity_snapshot.clone();
            tokio::spawn(async move {
                evaluate_and_execute(&state, &rule, &ctx, Some(&snap)).await;
            });
        }
    }
}
