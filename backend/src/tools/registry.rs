use std::collections::HashMap;
use std::sync::Arc;

use axum::http::StatusCode;
use openai_api_rs::v1::chat_completion;

use crate::api::twilio_sms::TwilioResponse;
use crate::models::user_models::User;
use crate::AppState;

/// Context passed to every tool handler during execution.
pub struct ToolContext<'a> {
    pub state: &'a Arc<AppState>,
    pub user: &'a User,
    pub user_id: i32,
    pub arguments: &'a str,
    pub image_url: Option<&'a str>,
    pub tool_call_id: String,
    /// User-provided info string (used by perplexity for context)
    pub user_given_info: &'a str,
    /// Current timestamp for history entries
    pub current_time: i32,
    // Fields needed by create_item which requires the full completion context
    pub client: Option<&'a openai_api_rs::v1::api::OpenAIClient>,
    pub model: Option<&'a str>,
    pub tools: Option<&'a Vec<chat_completion::Tool>>,
    pub completion_messages: Option<&'a Vec<chat_completion::ChatCompletionMessage>>,
    pub assistant_content: Option<&'a str>,
    pub tool_call: Option<&'a chat_completion::ToolCall>,
}

/// The result of executing a tool.
pub enum ToolResult {
    /// Tool produced an answer string - continue to follow-up model call
    Answer(String),
    /// Tool produced an answer and also created a task
    AnswerWithTask { answer: String, task_id: i32 },
    /// Tool handled response directly (outgoing tools: send_email, send_chat)
    /// These return early with history already written
    EarlyReturn {
        response: TwilioResponse,
        status: StatusCode,
    },
}

#[async_trait::async_trait]
pub trait ToolHandler: Send + Sync {
    /// The tool name the model uses to call this
    fn name(&self) -> &'static str;

    /// OpenAI-format tool definition
    fn definition(&self) -> chat_completion::Tool;

    /// Whether this tool sends data externally (requires user confirmation)
    fn is_outgoing(&self) -> bool {
        false
    }

    /// Whether this tool performs actions that should only fire from trusted senders.
    /// Restricted tools are blocked when a task is triggered by an untrusted sender
    /// (one with no matching contact profile), preventing prompt injection from
    /// causing unintended external actions like sending emails or unlocking a car.
    fn is_restricted(&self) -> bool {
        false
    }

    /// Execute the tool
    async fn execute(&self, ctx: ToolContext<'_>) -> Result<ToolResult, String>;
}

pub struct ToolRegistry {
    handlers: HashMap<String, Arc<dyn ToolHandler>>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    pub fn register(&mut self, handler: Arc<dyn ToolHandler>) {
        self.handlers.insert(handler.name().to_string(), handler);
    }

    pub fn definitions(&self) -> Vec<chat_completion::Tool> {
        self.handlers.values().map(|h| h.definition()).collect()
    }

    pub fn get(&self, name: &str) -> Option<&Arc<dyn ToolHandler>> {
        self.handlers.get(name)
    }

    /// Check if a tool is restricted (requires trusted sender for task execution).
    /// Returns false for unknown tools so they fall through to the catch-all handler.
    pub fn is_restricted(&self, name: &str) -> bool {
        self.handlers
            .get(name)
            .map(|h| h.is_restricted())
            .unwrap_or(false)
    }
}

/// Write history entries for outgoing tools (send_email, send_chat_message, etc.)
/// that return early with a response.
pub fn write_outgoing_history(
    state: &Arc<AppState>,
    user_id: i32,
    tool_name: &str,
    tool_call_id: &str,
    message: &str,
    current_time: i32,
) {
    // Write assistant history entry
    let history_entry = crate::models::user_models::NewMessageHistory {
        user_id,
        role: "assistant".to_string(),
        encrypted_content: message.to_string(),
        tool_name: Some(tool_name.to_string()),
        tool_call_id: Some(tool_call_id.to_string()),
        tool_calls_json: None,
        created_at: chrono::Utc::now().timestamp() as i32,
        conversation_id: "".to_string(),
    };
    if let Err(e) = state.user_repository.create_message_history(&history_entry) {
        tracing::error!(
            "Failed to store {} tool message in history: {}",
            tool_name,
            e
        );
    }

    // Write tool response history entry
    let tool_message = crate::models::user_models::NewMessageHistory {
        user_id,
        role: "tool".to_string(),
        encrypted_content: message.to_string(),
        tool_name: Some(tool_name.to_string()),
        tool_call_id: Some(tool_call_id.to_string()),
        tool_calls_json: None,
        created_at: current_time,
        conversation_id: "".to_string(),
    };
    if let Err(e) = state.user_repository.create_message_history(&tool_message) {
        tracing::error!("Failed to store tool response for {}: {}", tool_name, e);
    }
}

/// Write error history for outgoing tools that fail.
pub fn write_outgoing_error_history(
    state: &Arc<AppState>,
    user_id: i32,
    tool_name: &str,
    tool_call_id: &str,
    error_msg: &str,
    current_time: i32,
) {
    let tool_message = crate::models::user_models::NewMessageHistory {
        user_id,
        role: "tool".to_string(),
        encrypted_content: error_msg.to_string(),
        tool_name: Some(tool_name.to_string()),
        tool_call_id: Some(tool_call_id.to_string()),
        tool_calls_json: None,
        created_at: current_time,
        conversation_id: "".to_string(),
    };
    if let Err(store_e) = state.user_repository.create_message_history(&tool_message) {
        tracing::error!(
            "Failed to store tool error response for {}: {}",
            tool_name,
            store_e
        );
    }
}
