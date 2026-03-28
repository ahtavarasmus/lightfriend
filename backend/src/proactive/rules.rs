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
    Events,
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
// Prompt templates: stored as "template:<id>" in flow_config, resolved at eval time
// ---------------------------------------------------------------------------

/// Resolve a prompt string. If it starts with "template:", expand to the
/// current canonical prompt text. Otherwise return as-is (custom prompt).
pub fn resolve_prompt_template(prompt: &str, trigger_type: &str) -> String {
    let id = match prompt.strip_prefix("template:") {
        Some(id) => id,
        None => return prompt.to_string(),
    };
    let is_schedule = trigger_type == "schedule";
    match id {
        "summarize" => if is_schedule {
            "Summarize recent messages and emails into a brief digest. Focus on key points, action items, and anything that needs attention. Also mention any tracked items with approaching deadlines. Format as a numbered list, one item per line."
        } else {
            "Summarize this message along with recent conversation context. Highlight key points and any action needed."
        }.to_string(),
        "filter_important" => if is_schedule {
            "Review recent messages and emails. Only notify if delaying over 2 hours could cause harm, financial loss, or miss a time-sensitive opportunity. Examples: emergencies, someone asking to meet now, immediate decisions needed. Routine updates and vague requests are NOT critical. If nothing critical, respond with just 'skip'."
        } else {
            "Only notify if delaying this message over 2 hours could cause harm, financial loss, or miss a time-sensitive opportunity. Examples: emergencies, someone asking to meet now, immediate decisions needed. Routine updates, casual messages, and vague requests are NOT critical. If not critical, respond with just 'skip'."
        }.to_string(),
        "track_items_update" => "Does this message update an already-tracked obligation with a concrete next step or deadline? Create or update events for specific commitments like paying, booking, confirming, or sending something. Do not use one umbrella event for an entire trip or situation. Routine updates should be tracked silently in the background. If this message changes a tracked obligation's status, due date, or best reminder time, act on it. Otherwise skip.".to_string(),
        "track_items_create" => "Should this message create a new tracked obligation? Only create one for a concrete commitment the user could forget and would benefit from being reminded about at the right time, such as paying, booking, confirming, sending, or following up by a certain date. Do not create umbrella events for whole situations like trip planning when the message is really about a smaller obligation inside it. If nothing specific should be tracked, respond with just 'skip'.".to_string(),
        _ if id.starts_with("check_condition:") => {
            let condition = id.strip_prefix("check_condition:").unwrap_or("");
            if is_schedule {
                format!("Check if the following condition is met based on recent messages: {}. If the condition is not met, respond with just 'skip'.", condition)
            } else {
                format!("Check if this message matches the following condition: {}. If it doesn't match, respond with just 'skip'.", condition)
            }
        }
        _ => prompt.to_string(), // unknown template, use as-is
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
    // Delay before evaluating (seconds). During this window, check if user
    // already saw the message. None or 0 = immediate (no seen-check).
    // Default 300 (5 min) for ontology_change rules.
    pub delay_seconds: Option<i32>,
    // Group chat mode: "all" = all messages, "mention_only" = only @mentions
    pub group_mode: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ActionConfig {
    pub method: Option<String>,
    pub message: Option<String>,
    pub tool: Option<String>,
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LogicResult {
    condition_met: bool,
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

    // Must match change type (default to "created" if not specified)
    let expected_change = config.change.as_deref().unwrap_or("created");
    if !expected_change.eq_ignore_ascii_case(change_type) {
        return false;
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

    // Group chat "mention_only" filter: skip messages that don't contain @mention
    if let Some(ref mode) = config.group_mode {
        if mode == "mention_only" {
            let content = entity_snapshot
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            // Check for @mention patterns (bridged platforms relay mentions as @name)
            if !content.contains('@') {
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
    // Check expiry FIRST (before marking as triggered)
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

    // Mark as triggered (only if not expired)
    let _ = state
        .ontology_repository
        .update_rule_last_triggered(rule.id);

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
            // Safety: group chat messages must never be evaluated with LLM.
            // If the trigger has group_mode set, skip LLM and execute true_branch directly.
            let trigger: TriggerConfig =
                serde_json::from_str(&rule.trigger_config).unwrap_or_default();
            if trigger.group_mode.is_some() {
                info!(
                    "Rule {} ({}): skipping LLM eval for group chat, executing action directly",
                    rule.id, rule.name
                );
                if let Some(branch) = true_branch.as_ref() {
                    return Box::pin(evaluate_flow(
                        state,
                        rule,
                        trigger_context,
                        trigger_snapshot,
                        branch,
                        prev_message,
                        prev_extras,
                    ))
                    .await;
                }
                return Ok(());
            }

            // Resolve template prompts to actual text
            let resolved_prompt = resolve_prompt_template(prompt, &rule.trigger_type);
            // Peek at true_branch to extract tool params for LLM
            let extra_params = extract_tool_params(state, true_branch.as_ref());
            let prefetched = prefetch_sources(state, rule, fetch).await;
            let result = call_llm_condition(
                state,
                rule,
                trigger_context,
                &resolved_prompt,
                &prefetched,
                extra_params.as_ref(),
            )
            .await?;
            let (next, msg, extras) = if result.condition_met {
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
pub(crate) fn extract_tool_params(
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

pub(crate) async fn prefetch_sources(
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
                    rule.user_id,
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
            FetchSource::Events => {
                let events = state
                    .ontology_repository
                    .get_active_events(rule.user_id)
                    .unwrap_or_default();
                if !events.is_empty() {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i32;
                    let formatted: Vec<String> = events
                        .iter()
                        .map(|e| {
                            let deadline_tag = e
                                .due_at
                                .map(|due_at| {
                                    if now > due_at {
                                        " [OVERDUE]".to_string()
                                    } else {
                                        let days_left = (due_at - now) / 86400;
                                        if days_left <= 2 {
                                            format!(" [due in {} days]", days_left)
                                        } else {
                                            String::new()
                                        }
                                    }
                                })
                                .unwrap_or_default();
                            let linked_messages = state
                                .ontology_repository
                                .get_messages_for_event(rule.user_id, e.id)
                                .unwrap_or_default();
                            let oldest_message = linked_messages.first();
                            let newest_message = linked_messages.last();
                            let summarize_message =
                                |label: &str,
                                 message: &crate::models::ontology_models::OntMessage|
                                 -> String {
                                    let preview: String =
                                        message.content.chars().take(140).collect();
                                    format!(
                                        "\n  {}: [{}:{}] {}",
                                        label, message.platform, message.sender_name, preview
                                    )
                                };
                            let origin_context = oldest_message
                                .map(|m| summarize_message("origin", m))
                                .unwrap_or_default();
                            let latest_context = newest_message
                                .filter(|m| Some(m.id) != oldest_message.map(|om| om.id))
                                .map(|m| summarize_message("latest", m))
                                .unwrap_or_default();
                            format!(
                                "[event_id={}] [status={}]{} {}{}{}",
                                e.id,
                                e.status,
                                deadline_tag,
                                e.description,
                                origin_context,
                                latest_context
                            )
                        })
                        .collect();
                    prefetched.push_str(&format!(
                        "\n\n--- Tracked obligations ---\n{}",
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

pub(crate) async fn call_llm_condition(
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
        " When condition_met=true, also fill in the additional tool parameter fields."
    } else {
        ""
    };
    let system_prompt = format!(
        "You are evaluating whether an IF condition matches in a user-defined rule.\n\
        Rule: \"{}\"\n\
        Instructions: {}\n\
        {}\n\n\
        Call the `rule_result` tool with your decision. Set condition_met=true when this IF condition matches and the true branch should run. Set condition_met=false when it does not match and the false branch should run. Fill the `message` field only if the downstream action will actually use it.{}\n\
        Keep messages concise (max 480 chars), direct, second person. NEVER use emojis.",
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
        "condition_met".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Boolean),
            description: Some(
                "true if this IF condition matches and the true branch should run".to_string(),
            ),
            ..Default::default()
        }),
    );
    result_properties.insert(
        "message".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "Optional user-facing text for the downstream action when relevant (max 480 chars, second person). Leave minimal if the action is silent.".to_string(),
            ),
            ..Default::default()
        }),
    );

    // Merge extra tool params (optional fields the LLM should fill when condition_met=true)
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
                required: Some(vec!["condition_met".to_string(), "message".to_string()]),
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

    crate::ai_config::log_llm_usage(
        &state.llm_usage_repository,
        rule.user_id,
        match ctx.provider {
            crate::AiProvider::Tinfoil => "tinfoil",
            crate::AiProvider::OpenRouter => "openrouter",
        },
        &ctx.model,
        "rule_eval",
        &result,
    );

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
        condition_met: false,
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
                let params_value = {
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
                    if tool_name == "create_event" {
                        if let Some(snapshot) = trigger_snapshot {
                            if let Some(mid) = snapshot.get("message_id").and_then(|v| v.as_i64()) {
                                p["message_id"] = serde_json::json!(mid);
                            }
                        }
                    }
                    p
                };
                let params = params_value.to_string();
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
// Message seen-check: did the user already see this message?
// ---------------------------------------------------------------------------

/// Check if the user has already seen a message (via read receipts or email
/// Seen flag). Returns true if the message was seen and should be skipped.
pub(crate) async fn check_message_seen(
    state: &Arc<AppState>,
    user_id: i32,
    platform: &str,
    room_id: &str,
    message_created_at: i32,
) -> bool {
    match platform {
        "email" => {
            // Check IMAP \Seen flag for this email
            let uid = room_id.strip_prefix("email_").unwrap_or(room_id);
            check_email_seen(state, user_id, uid).await
        }
        // Bridge platforms: check Matrix read receipts + user reply history
        _ => check_bridge_seen(state, user_id, room_id, message_created_at).await,
    }
}

/// Check if user read the email in their actual email app via IMAP \Seen flag.
async fn check_email_seen(state: &Arc<AppState>, user_id: i32, email_uid: &str) -> bool {
    use crate::repositories::user_core::UserCoreOps;

    let creds = match state.user_repository.get_imap_credentials(user_id) {
        Ok(Some(c)) => c,
        _ => return false,
    };
    let (_description, password, server, port) = creds;
    let server = match server {
        Some(s) => s,
        None => return false,
    };
    let port = port.unwrap_or(993) as u16;
    let email = match state.user_core.find_by_id(user_id) {
        Ok(Some(u)) => u.email,
        _ => return false,
    };

    // Run in spawn_blocking since imap is synchronous
    let uid = email_uid.to_string();
    tokio::task::spawn_blocking(move || {
        let tls = match native_tls::TlsConnector::new() {
            Ok(t) => t,
            Err(_) => return false,
        };
        let client = match imap::connect((server.as_str(), port), &server, &tls) {
            Ok(c) => c,
            Err(_) => return false,
        };
        let mut session = match client.login(&email, &password) {
            Ok(s) => s,
            Err(_) => return false,
        };
        let _ = session.select("INBOX");

        let is_seen = match session.uid_fetch(&uid, "FLAGS") {
            Ok(messages) => messages
                .iter()
                .any(|msg| msg.flags().iter().any(|flag| flag.to_string() == "\\Seen")),
            Err(_) => false,
        };
        let _ = session.logout();
        is_seen
    })
    .await
    .unwrap_or(false)
}

/// Check if user saw a bridge message via Matrix read receipts.
async fn check_bridge_seen(
    state: &Arc<AppState>,
    user_id: i32,
    room_id: &str,
    message_created_at: i32,
) -> bool {
    let client = match crate::utils::matrix_auth::get_cached_client(user_id, state).await {
        Ok(c) => c,
        Err(_) => return false,
    };
    let matrix_room_id = match matrix_sdk::ruma::OwnedRoomId::try_from(room_id) {
        Ok(id) => id,
        Err(_) => return false,
    };
    let room = match client.get_room(&matrix_room_id) {
        Some(r) => r,
        None => return false,
    };
    match crate::utils::bridge::get_room_seen_timestamp(&room, &client).await {
        Some(seen_ts) => seen_ts >= message_created_at as i64,
        None => false,
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
    // Skip completed messages early (before evaluating any rules)
    if entity_type == "Message" {
        if let Some(status) = entity_snapshot.get("status").and_then(|v| v.as_str()) {
            if status == "completed" {
                return;
            }
        }
    }

    // === System behaviors (automatic, no user rules needed) ===
    let is_group = entity_snapshot
        .get("is_group")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if entity_type == "Message" && !is_group {
        let state_sys = Arc::clone(state);
        let snap_sys = entity_snapshot.clone();
        let platform_sys = entity_snapshot
            .get("platform")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let room_id_sys = entity_snapshot
            .get("room_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let created_at_sys = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i32;

        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(300)).await;

            if check_message_seen(
                &state_sys,
                user_id,
                &platform_sys,
                &room_id_sys,
                created_at_sys,
            )
            .await
            {
                return;
            }

            if let Err(e) = crate::proactive::system_behaviors::run_system_behaviors(
                &state_sys, user_id, &snap_sys,
            )
            .await
            {
                error!("System behaviors failed for user {}: {}", user_id, e);
            }
        });
    }

    // === Custom rules ===
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

    // Extract message metadata for seen-checks (if this is a Message event)
    let msg_platform = entity_snapshot
        .get("platform")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let msg_room_id = entity_snapshot
        .get("room_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let msg_created_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i32;

    for rule in rules {
        if matches_trigger(&rule, entity_type, change_type, &entity_snapshot) {
            let state = Arc::clone(state);
            let rule = rule.clone();
            let ctx = trigger_context.clone();
            let snap = entity_snapshot.clone();
            let platform = msg_platform.clone();
            let room_id = msg_room_id.clone();
            let is_message = entity_type == "Message";

            // Parse delay from trigger config (default 300s for ontology_change)
            let trigger: TriggerConfig =
                serde_json::from_str(&rule.trigger_config).unwrap_or_default();
            let delay = trigger.delay_seconds.unwrap_or(600);

            tokio::spawn(async move {
                if delay > 0 && is_message {
                    // Wait, then check if user already saw the message
                    tokio::time::sleep(tokio::time::Duration::from_secs(delay as u64)).await;

                    if check_message_seen(&state, rule.user_id, &platform, &room_id, msg_created_at)
                        .await
                    {
                        info!(
                            "Rule {} ({}): skipping - user already saw the message",
                            rule.id, rule.name
                        );
                        return;
                    }
                }
                evaluate_and_execute(&state, &rule, &ctx, Some(&snap)).await;
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Rule test: step-by-step evaluation for the test panel
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum RuleTestStep {
    Prefetching {
        sources: Vec<String>,
    },
    EvaluatingLlm {
        prompt_preview: String,
    },
    LlmResult {
        decided: bool,
        message: Option<String>,
    },
    CheckingKeyword {
        keyword: String,
    },
    KeywordResult {
        matched: bool,
    },
    WouldExecute {
        action_type: String,
        description: String,
    },
    NoAction {
        reason: String,
    },
    Error {
        message: String,
    },
    Complete,
}

/// Walk a flow tree for testing: real LLM calls, but actions are described
/// instead of executed. Each step is sent through `tx`.
pub async fn evaluate_flow_test(
    state: &Arc<AppState>,
    rule: &OntRule,
    trigger_context: &str,
    node: &FlowNode,
    tx: &tokio::sync::mpsc::Sender<RuleTestStep>,
) {
    match node {
        FlowNode::LlmCondition {
            prompt,
            fetch,
            true_branch,
            false_branch,
        } => {
            // Prefetch
            if !fetch.is_empty() {
                let source_names: Vec<String> = fetch
                    .iter()
                    .map(|s| match s {
                        FetchSource::Email => "email".into(),
                        FetchSource::Chat { platform, .. } => format!("chat ({})", platform),
                        FetchSource::Weather { .. } => "weather".into(),
                        FetchSource::Internet { query } => format!("internet: {}", query),
                        FetchSource::Tesla => "tesla".into(),
                        FetchSource::Mcp { server, tool, .. } => format!("mcp {}:{}", server, tool),
                        FetchSource::Events => "tracked obligations".into(),
                    })
                    .collect();
                let _ = tx
                    .send(RuleTestStep::Prefetching {
                        sources: source_names,
                    })
                    .await;
            }
            let prefetched = prefetch_sources(state, rule, fetch).await;

            let resolved_prompt = resolve_prompt_template(prompt, &rule.trigger_type);
            let preview = if resolved_prompt.len() > 120 {
                format!("{}...", &resolved_prompt[..120])
            } else {
                resolved_prompt.clone()
            };
            let _ = tx
                .send(RuleTestStep::EvaluatingLlm {
                    prompt_preview: preview,
                })
                .await;

            let extra_params = extract_tool_params(state, true_branch.as_ref());
            match call_llm_condition(
                state,
                rule,
                trigger_context,
                &resolved_prompt,
                &prefetched,
                extra_params.as_ref(),
            )
            .await
            {
                Ok(result) => {
                    let decided = result.condition_met;
                    let msg = result.message.clone();
                    let _ = tx
                        .send(RuleTestStep::LlmResult {
                            decided,
                            message: msg,
                        })
                        .await;

                    let next = if decided { true_branch } else { false_branch };
                    if let Some(branch) = next.as_ref() {
                        Box::pin(evaluate_flow_test(state, rule, trigger_context, branch, tx))
                            .await;
                    } else {
                        let reason = if decided {
                            "Condition was true but no action configured"
                        } else {
                            "Condition was false and no else branch"
                        };
                        let _ = tx
                            .send(RuleTestStep::NoAction {
                                reason: reason.to_string(),
                            })
                            .await;
                    }
                }
                Err(e) => {
                    let _ = tx
                        .send(RuleTestStep::Error {
                            message: format!("LLM error: {}", e),
                        })
                        .await;
                }
            }
        }
        FlowNode::KeywordCondition {
            keyword,
            true_branch,
            false_branch,
        } => {
            let _ = tx
                .send(RuleTestStep::CheckingKeyword {
                    keyword: keyword.clone(),
                })
                .await;
            let matched = !keyword.is_empty()
                && trigger_context
                    .to_lowercase()
                    .contains(&keyword.to_lowercase());
            let _ = tx.send(RuleTestStep::KeywordResult { matched }).await;

            let next = if matched { true_branch } else { false_branch };
            if let Some(branch) = next.as_ref() {
                Box::pin(evaluate_flow_test(state, rule, trigger_context, branch, tx)).await;
            } else {
                let reason = if matched {
                    "Keyword matched but no action configured"
                } else {
                    "Keyword did not match and no else branch"
                };
                let _ = tx
                    .send(RuleTestStep::NoAction {
                        reason: reason.to_string(),
                    })
                    .await;
            }
        }
        FlowNode::Action {
            action_type,
            config,
        } => {
            let description = match action_type.as_str() {
                "notify" => {
                    let method = config
                        .get("method")
                        .and_then(|v| v.as_str())
                        .unwrap_or("sms");
                    format!("send {} notification", method)
                }
                "tool_call" => {
                    let tool = config
                        .get("tool")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    format!("call tool '{}'", tool)
                }
                other => format!("execute '{}' action", other),
            };
            let _ = tx
                .send(RuleTestStep::WouldExecute {
                    action_type: action_type.clone(),
                    description,
                })
                .await;
        }
    }
}
