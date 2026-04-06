//! Shared agent core: system prompt, tool loading, and tool dispatch.
//!
//! Both the SMS pipeline (`twilio_sms.rs`) and voice pipeline (`voice_pipeline.rs`)
//! call into these functions so that prompt, tools, and dispatch logic stay in one place.

use std::sync::Arc;

use openai_api_rs::v1::chat_completion;

use crate::context::AgentContext;
use crate::models::user_models::User;
use crate::AppState;

// ---------------------------------------------------------------------------
// ChannelMode
// ---------------------------------------------------------------------------

/// Which transport channel the agent is serving.
pub enum ChannelMode {
    Sms,
    Voice,
    WebChat,
}

// ---------------------------------------------------------------------------
// build_system_prompt
// ---------------------------------------------------------------------------

/// Build the full system prompt from an already-assembled `AgentContext`.
///
/// The core prompt (date, user info, contacts, tool rules, date/time rules) is
/// shared across all modes. A mode-specific suffix is appended.
pub fn build_system_prompt(ctx: &AgentContext, mode: ChannelMode) -> String {
    let tz = ctx.timezone.as_ref();
    let formatted_time = tz.map(|t| t.formatted_now.as_str()).unwrap_or("unknown");
    let timezone_str = tz.map(|t| t.tz_str.as_str()).unwrap_or("UTC");
    let offset = tz.map(|t| t.offset_string.as_str()).unwrap_or("+00:00");

    let contacts_info = ctx.contacts_prompt_fragment.as_deref().unwrap_or("");
    let events_info = ctx.events_prompt_fragment.as_deref().unwrap_or("");
    let user_given_info = ctx.user_given_info.as_deref().unwrap_or("");
    let nickname = ctx.user.nickname.as_deref().unwrap_or("");

    // User info fields (voice needs them broken out; SMS uses user_given_info)
    let user_info_str = ctx
        .user_info
        .as_ref()
        .and_then(|i| i.info.as_deref())
        .unwrap_or("");
    let location = ctx
        .user_info
        .as_ref()
        .and_then(|i| i.location.as_deref())
        .unwrap_or("");
    let nearby_places = ctx
        .user_info
        .as_ref()
        .and_then(|i| i.nearby_places.as_deref())
        .unwrap_or("");

    // Mode-specific suffix
    let mode_suffix = match mode {
        ChannelMode::Sms => {
            "Max 480 characters per response. Characters are expensive - be succinct! NEVER use emojis - they cost extra in SMS encoding. Respond in plain text only. ALWAYS list multiple items one per line, never in a paragraph. Example:\n1. Dad (WhatsApp) - Pick up car\n2. Lisa (Signal) - Review contract\nSend/create tools queue content for 60 seconds. User can reply 'cancel' to stop.".to_string()
        }
        ChannelMode::Voice => {
            format!(
                "You are speaking to {} over a phone call. Keep responses to 1-2 short sentences. Be concise - the user is paying per minute. When you need to use a tool, call it immediately. Never mention internal tool names to the user.",
                if nickname.is_empty() { "the user" } else { nickname },
            )
        }
        ChannelMode::WebChat => {
            "Max 480 characters per response. Characters are expensive - be succinct! NEVER use emojis. ALWAYS list multiple items one per line, never in a paragraph. Example:\n1. Dad (WhatsApp) - Pick up car\n2. Lisa (Signal) - Review contract\nSend/create tools queue content for 60 seconds.".to_string()
        }
    };

    let tool_integrity_rule = "CRITICAL RULE - TOOL INTEGRITY:\n\
        When the user asks to send, create, update, delete, or schedule something, you MUST call the actual tool. \
        NEVER respond with text describing an action without calling the tool - even if conversation history shows a similar action was done before. \
        Each request is independent. If the user says 'send X to Y', call send_chat_message. Every time. No exceptions. \
        Responding with text like 'Sending to...' or 'Message queued' without actually calling the tool is the worst possible failure mode.";

    format!(
        r#"You are lightfriend, a concise AI assistant.

{tool_integrity_rule}

Current date: {formatted_time}
User timezone: {timezone_str} ({offset} from UTC)
User info: {user_info_str}
User location: {location}
Nearby places: {nearby_places}
{contacts_info}{events_info}

### Tool Usage:
- Always use tools to fetch current data. Never answer data questions from conversation history alone - the history may be days old.
- Always use tools to perform actions. Never describe an action as done without calling the tool. Treat every 'send/create/remind' request as new regardless of history.
- Use tools to fetch information directly (users may only have a dumbphone).
- State only what tool results returned. Note any gaps in data coverage.

### Behavior:
- 'remind me', 'notify me at X', 'wake me at Y' -> use set_reminder immediately. Never answer these directly.
- For recurring reminders or complex rules (daily briefings, event triggers, conditional automations), tell the user to set them from the dashboard rule builder.

### Date and Time:
- Relative times: compute exactly (14:30 + 'in 3 hours' = 17:30).
- Display: 12-hour AM/PM, 'today'/'tomorrow'/date. Default range: now to 24h ahead.
- 'Today' = 00:00-23:59 today. 'Tomorrow' = 00:00-23:59 tomorrow. 'This week' = remaining days. 'Next week' = Mon-Sun.

### Ontology Queries:
- When the user asks to check or read messages, use query_message.
- When they ask about contacts, use query_person.
- When they ask about events or calendar, use query_event.
- Messages with sender "You" are messages sent by this user (you are talking to them right now). When the user asks "what did I send" or "my messages", look for sender_name "You".

Provide all information immediately; only ask follow-ups when confirming send/create actions. Call all needed tools upfront.
Never fabricate information. Use tools to fetch latest information before answering.
User information: {user_given_info}

{mode_suffix}"#,
    )
}

// ---------------------------------------------------------------------------
// build_tools
// ---------------------------------------------------------------------------

/// Load all tool definitions for a user: static registry + ontology + optional MCP.
pub async fn build_tools(
    state: &Arc<AppState>,
    user_id: i32,
    include_mcp: bool,
) -> Vec<chat_completion::Tool> {
    let mut tools = state.tool_registry.definitions_for_user(state, user_id);

    // Ontology query tools (dynamically built with user-specific enum values)
    let ontology_user_data = {
        let mut dynamic_enums = std::collections::HashMap::new();
        if let Ok(persons) = state.ontology_repository.get_persons(user_id) {
            let names: Vec<String> = persons.iter().map(|p| p.name.clone()).collect();
            dynamic_enums.insert("person_names".to_string(), names);
        }
        crate::ontology::registry::OntologyUserData { dynamic_enums }
    };
    tools.extend(
        state
            .ontology_registry
            .build_query_tools(&ontology_user_data),
    );

    // MCP tools (user's custom MCP server tools)
    if include_mcp {
        let mcp_tools = crate::tool_call_utils::mcp::get_mcp_tools_for_user(state, user_id).await;
        tools.extend(mcp_tools);
    }

    tools
}

// ---------------------------------------------------------------------------
// dispatch_tool
// ---------------------------------------------------------------------------

/// The outcome of dispatching a single tool call.
pub enum ToolDispatchResult {
    /// Tool produced an answer string.
    Answer(String),
    /// Tool produced an answer and also created a persistent item (reminder, event).
    AnswerWithTask { answer: String, task_id: i32 },
    /// Tool handled the response itself (outgoing tools like send_email).
    EarlyReturn {
        response: crate::api::twilio_sms::TwilioResponse,
        status: axum::http::StatusCode,
    },
    /// User's subscription does not cover this tool.
    SubscriptionRequired(String),
    /// Tool name not recognized by any handler.
    Unknown(String),
    /// Tool handler returned an error.
    Error(String),
}

/// Extra context needed by some tools (send_email, create_item).
/// These are specific to the SMS/WebChat pipeline where the full OpenAI completion
/// context is available. Voice passes `None`.
pub struct ToolDispatchExtras<'a> {
    pub client: &'a openai_api_rs::v1::api::OpenAIClient,
    pub model: &'a str,
    pub tools: &'a Vec<chat_completion::Tool>,
    pub completion_messages: &'a Vec<chat_completion::ChatCompletionMessage>,
    pub assistant_content: Option<&'a str>,
    pub tool_call: Option<&'a chat_completion::ToolCall>,
    pub image_url: Option<&'a str>,
    /// When true, skip SMS confirmation sends (web dashboard path)
    pub skip_sms: bool,
}

/// Dispatch a single tool call through the correct handler.
///
/// Routing order:
/// 1. Subscription check
/// 2. `query_*` - ontology queries
/// 3. `mcp:*` - MCP tool calls
/// 4. Static tool registry
/// 5. Unknown
#[allow(clippy::too_many_arguments)]
pub async fn dispatch_tool(
    state: &Arc<AppState>,
    user: &User,
    name: &str,
    arguments: &str,
    tool_call_id: &str,
    user_given_info: &str,
    current_time: i32,
    extras: Option<ToolDispatchExtras<'_>>,
) -> ToolDispatchResult {
    // 1. Subscription check
    if crate::tool_call_utils::utils::requires_subscription(name, user.sub_tier.clone()) {
        return ToolDispatchResult::SubscriptionRequired(format!(
            "This feature ({}) requires a subscription. Please visit our website to subscribe.",
            name
        ));
    }

    // 2. Ontology query tools
    if name.starts_with("query_") {
        tracing::debug!("Executing ontology tool call: {}", name);
        return match crate::tools::ontology::handle_query(name, arguments, state, user.id).await {
            Ok(answer) => ToolDispatchResult::Answer(answer),
            Err(e) => ToolDispatchResult::Answer(e),
        };
    }

    // 3. MCP tools
    if crate::tool_call_utils::mcp::is_mcp_tool(name) {
        tracing::debug!("Executing MCP tool call: {}", name);
        let result =
            crate::tool_call_utils::mcp::handle_mcp_tool_call(state, user.id, name, arguments)
                .await;
        return ToolDispatchResult::Answer(result);
    }

    // 4. Static tool registry
    let tool_ctx = crate::tools::registry::ToolContext {
        state,
        user,
        user_id: user.id,
        arguments,
        image_url: extras.as_ref().and_then(|e| e.image_url),
        tool_call_id: tool_call_id.to_string(),
        user_given_info,
        current_time,
        skip_sms: extras.as_ref().map(|e| e.skip_sms).unwrap_or(false),
        client: extras.as_ref().map(|e| e.client),
        model: extras.as_ref().map(|e| e.model),
        tools: extras.as_ref().map(|e| e.tools),
        completion_messages: extras.as_ref().map(|e| e.completion_messages),
        assistant_content: extras.as_ref().and_then(|e| e.assistant_content),
        tool_call: extras.as_ref().and_then(|e| e.tool_call),
    };

    match state.tool_registry.get(name) {
        Some(handler) => match handler.execute(tool_ctx).await {
            Ok(crate::tools::registry::ToolResult::Answer(answer)) => {
                ToolDispatchResult::Answer(answer)
            }
            Ok(crate::tools::registry::ToolResult::AnswerWithTask { answer, task_id }) => {
                ToolDispatchResult::AnswerWithTask { answer, task_id }
            }
            Ok(crate::tools::registry::ToolResult::EarlyReturn { response, status }) => {
                ToolDispatchResult::EarlyReturn { response, status }
            }
            Err(e) => ToolDispatchResult::Error(e),
        },
        None => ToolDispatchResult::Unknown(format!("Unknown tool: {}", name)),
    }
}
