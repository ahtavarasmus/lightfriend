use crate::AppState;
use crate::UserCoreOps;
use openai_api_rs::v1::{api::OpenAIClient, chat_completion, types};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
            name: "fetch_calendar_events",
            description: "Check/fetch calendar events",
            params_schema: r#"{}"#,
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
pub fn requires_subscription(
    tool_name: &str,
    sub_tier: Option<String>,
    has_discount: bool,
) -> bool {
    // Only tier 2 (hosted) subscribers and users with discount get full access to everything
    if sub_tier == Some("tier 2".to_string()) || has_discount {
        println!("✅ User has tier 2 subscription or discount - granting full access");
        return false;
    }

    println!("❌ Tool {} requires tier 2 subscription", tool_name);
    true
}

// Function to create evaluation tools
// Function to create email selection tool properties
pub fn create_email_select_properties() -> HashMap<String, Box<types::JSONSchemaDefine>> {
    let mut select_properties = HashMap::new();
    select_properties.insert(
        "email_id".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The ID of the most relevant email".to_string()),
            ..Default::default()
        }),
    );
    select_properties.insert(
        "reason".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "Brief explanation of why this email was selected as most relevant".to_string(),
            ),
            ..Default::default()
        }),
    );
    select_properties
}

#[derive(Deserialize)]
pub struct EmailSelectResponse {
    pub email_id: String,
    pub reason: Option<String>,
}

pub async fn select_most_relevant_email(
    state: &Arc<AppState>,
    provider: crate::AiProvider,
    model: String,
    query: &str,
    emails: &str,
) -> Result<(String, Option<String>), Box<dyn std::error::Error>> {
    let select_messages = vec![
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::system,
            content: chat_completion::Content::Text(
                "You are an email search assistant. Your task is to analyze a list of emails and select the one that best matches the user's search query. Consider subject, sender, content, and date in your analysis.".to_string(),
            ),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::user,
            content: chat_completion::Content::Text(format!(
                "Search query: '{}'\n\nAvailable emails:\n{}",
                query, emails
            )),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
    ];

    let select_tools = vec![chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("select_email"),
            description: Some(String::from(
                "Selects the most relevant email based on the search query",
            )),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(create_email_select_properties()),
                required: Some(vec![String::from("email_id")]),
            },
        },
    }];

    let select_req = chat_completion::ChatCompletionRequest::new(model, select_messages)
        .tools(select_tools)
        .tool_choice(chat_completion::ToolChoiceType::Required)
        .max_tokens(200);

    match state.ai_config.chat_completion(provider, &select_req).await {
        Ok(result) => {
            if let Some(tool_calls) = result.choices[0].message.tool_calls.as_ref() {
                if let Some(first_call) = tool_calls.first() {
                    if let Some(args) = &first_call.function.arguments {
                        match serde_json::from_str::<EmailSelectResponse>(args) {
                            Ok(select) => Ok((select.email_id, select.reason)),
                            Err(e) => {
                                Err(format!("Failed to parse email selection response: {}", e)
                                    .into())
                            }
                        }
                    } else {
                        Err("No arguments found in email selection tool call".into())
                    }
                } else {
                    Err("No email selection tool calls found".into())
                }
            } else {
                Err("No tool calls section in email selection response".into())
            }
        }
        Err(e) => Err(format!("Failed to get email selection response: {}", e).into()),
    }
}
