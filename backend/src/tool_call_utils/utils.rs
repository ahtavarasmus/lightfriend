use crate::AppState;
use crate::UserCoreOps;
use chrono::{DateTime, FixedOffset, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;
use openai_api_rs::v1::{api::OpenAIClient, chat_completion};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::sync::Arc;

/// Available runtime tools for scheduled tasks.
/// These are the tools that the AI can use when executing a task.
/// Keep this list updated when adding new tools.
pub struct RuntimeTool {
    pub name: &'static str,
    pub description: &'static str,
    pub params_schema: &'static str,
}

/// Returns the list of available runtime tools for task execution.
/// This is the single source of truth - update this when adding new tools.
pub fn get_available_runtime_tools() -> Vec<RuntimeTool> {
    vec![
        RuntimeTool {
            name: "send_reminder",
            description: "Send a notification/reminder to the user. Use for reminder-only tasks.",
            params_schema: r#"{"message": "The reminder text"}"#,
        },
        RuntimeTool {
            name: "control_tesla",
            description: "Control Tesla vehicle. Commands: climate_on, climate_off, lock, unlock",
            params_schema: r#"{"command": "climate_on|climate_off|lock|unlock|status"}"#,
        },
        RuntimeTool {
            name: "send_chat_message",
            description: "Send message via WhatsApp, Telegram, or Signal",
            params_schema: r#"{"platform": "whatsapp|telegram|signal", "contact": "name", "message": "text"}"#,
        },
        RuntimeTool {
            name: "send_email",
            description: "Send an email",
            params_schema: r#"{"to": "email@example.com", "subject": "Subject", "body": "Email body"}"#,
        },
        RuntimeTool {
            name: "get_weather",
            description: "Get weather information for a location",
            params_schema: r#"{"location": "City name or 'current location'"}"#,
        },
    ]
}

/// Returns the tool names from the runtime tools registry.
pub fn get_runtime_tool_names() -> Vec<String> {
    get_available_runtime_tools()
        .iter()
        .map(|t| t.name.to_string())
        .collect()
}

/// Returns a formatted string listing all available runtime tools for AI prompts.
pub fn get_runtime_tools_prompt() -> String {
    let tools = get_available_runtime_tools();
    let mut result = String::from(
        "AVAILABLE TOOLS (use exact tool name for action_tool, JSON object for action_params):\n",
    );
    for tool in tools {
        result.push_str(&format!(
            "- {} - {} | params: {}\n",
            tool.name, tool.description, tool.params_schema
        ));
    }
    result
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: chat_completion::Content,
    pub tool_calls: Option<Vec<chat_completion::ToolCall>>,
    pub tool_call_id: Option<String>,
}

/// Creates an OpenAI-compatible client for a specific user.
/// Routes to the appropriate provider based on user's llm_provider preference setting.
pub fn create_openai_client_for_user(
    state: &Arc<AppState>,
    user_id: i32,
) -> Result<(OpenAIClient, crate::AiProvider), Box<dyn std::error::Error>> {
    // Use user's LLM provider preference from settings
    let llm_provider_preference = state.user_core.get_llm_provider(user_id).unwrap_or(None);
    let provider = state
        .ai_config
        .provider_for_user_with_preference(llm_provider_preference.as_deref());
    let client = state.ai_config.create_client(provider)?;
    Ok((client, provider))
}

/// Creates an OpenAI-compatible client using OpenRouter (for background tasks without user context)
/// This is used by proactive notifications and other system tasks.
pub fn create_openai_client(
    state: &Arc<AppState>,
) -> Result<OpenAIClient, Box<dyn std::error::Error>> {
    // Use OpenRouter for background tasks
    state
        .ai_config
        .create_client(crate::AiProvider::OpenRouter)
        .map_err(|e| e as Box<dyn std::error::Error>)
}

pub async fn cancel_pending_message(
    state: &Arc<AppState>,
    user_id: i32,
) -> Result<bool, Box<dyn std::error::Error>> {
    let mut senders = state.pending_message_senders.lock().await;
    if let Some(sender) = senders.remove(&user_id) {
        let _ = sender.send(());
        Ok(true) // Cancellation occurred
    } else {
        Ok(false) // No pending message to cancel
    }
}

// Helper function to check if a tool is accessible based on user's status
// Only tier 2 (hosted) subscribers get full access to all tools
pub fn requires_subscription(tool_name: &str, sub_tier: Option<String>) -> bool {
    // Only tier 2 (hosted) subscribers get full access to everything
    if sub_tier == Some("tier 2".to_string()) {
        return false;
    }

    tracing::debug!("Tool {} requires tier 2 subscription", tool_name);
    true
}

/// Parse a datetime string in the user's timezone to UTC.
/// Tries RFC3339 first (backward compat if LLM sends offset), then falls back
/// to naive `YYYY-MM-DDTHH:MM` parsed in the user's timezone.
pub fn parse_user_datetime_to_utc(
    time_str: &str,
    tz: &Tz,
) -> Result<DateTime<Utc>, Box<dyn Error>> {
    // Try parsing as full RFC3339 with timezone offset (e.g. "2026-02-08T07:00:00+02:00" or "...Z")
    if let Ok(dt) = DateTime::<FixedOffset>::parse_from_rfc3339(time_str) {
        return Ok(dt.with_timezone(&Utc));
    }
    // Also try common offset format without seconds (e.g. "2026-02-08T07:00+02:00")
    if time_str.len() > 16 {
        if let Ok(dt) = DateTime::<FixedOffset>::parse_from_rfc3339(&format!(
            "{}:00{}",
            &time_str[..16],
            &time_str[16..]
        )) {
            return Ok(dt.with_timezone(&Utc));
        }
    }

    // Fall back to naive datetime parsing (no timezone offset in string)
    let naive = if time_str.len() == 16 {
        // YYYY-MM-DDTHH:MM format
        NaiveDateTime::parse_from_str(&format!("{}:00", time_str), "%Y-%m-%dT%H:%M:%S")?
    } else {
        NaiveDateTime::parse_from_str(time_str, "%Y-%m-%dT%H:%M:%S")?
    };

    // Interpret naive datetime in user's timezone, then convert to UTC
    let local_dt = tz
        .from_local_datetime(&naive)
        .single()
        .ok_or("Ambiguous or invalid local time")?;

    Ok(local_dt.with_timezone(&Utc))
}
