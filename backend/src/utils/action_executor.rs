//! Action Executor for Task Runtime
//!
//! This module handles executing task action_specs at runtime using AI + tools.
//! It's called by the scheduler when scheduled tasks are due, or when recurring
//! task conditions are matched.

use std::sync::Arc;
use std::collections::HashMap;
use crate::AppState;
use crate::ModelPurpose;
use openai_api_rs::v1::chat_completion;
use crate::tool_call_utils::utils::create_openai_client_for_user;
use serde::Deserialize;

/// Result of executing a task's action_spec
pub enum ActionResult {
    Success { message: String },
    Failed { error: String },
}

/// Get the tools available for task runtime execution.
/// This is a subset of SMS tools plus send_reminder, excluding recursive tools.
fn get_task_runtime_tools() -> Vec<chat_completion::Tool> {
    vec![
        // send_reminder - the key tool for notifications
        crate::tool_call_utils::management::get_send_reminder_tool(),
        // Tesla control
        crate::tool_call_utils::tesla::get_tesla_control_tool(),
        // Weather
        crate::tool_call_utils::internet::get_weather_tool(),
        // Chat messaging
        crate::tool_call_utils::bridge::get_send_chat_message_tool(),
        // Email
        crate::tool_call_utils::email::get_send_email_tool(),
        // Calendar
        crate::tool_call_utils::calendar::get_fetch_calendar_event_tool(),
        // Web search
        crate::tool_call_utils::internet::get_ask_perplexity_tool(),
    ]
}

/// Execute a single tool call and return the result
async fn execute_tool_call(
    state: &Arc<AppState>,
    user_id: i32,
    tool_name: &str,
    arguments: &str,
    notification_type: &str,
) -> Result<String, String> {
    match tool_name {
        "send_reminder" => {
            crate::tool_call_utils::management::handle_send_reminder(
                state, user_id, arguments, notification_type
            ).await.map_err(|e| e.to_string())
        }
        "control_tesla" => {
            Ok(crate::tool_call_utils::tesla::handle_tesla_command(
                state, user_id, arguments, true // silent mode - don't send extra notification
            ).await)
        }
        "get_weather" => {
            #[derive(Deserialize)]
            struct WeatherArgs {
                location: String,
                units: Option<String>,
                forecast_type: Option<String>,
            }
            let args: WeatherArgs = serde_json::from_str(arguments).map_err(|e| e.to_string())?;
            let units = args.units.unwrap_or_else(|| "metric".to_string());
            let forecast_type = args.forecast_type.unwrap_or_else(|| "current".to_string());
            crate::utils::tool_exec::get_weather(state, &args.location, &units, &forecast_type, user_id)
                .await
                .map_err(|e| e.to_string())
        }
        "send_chat_message" => {
            // Get user info for send_chat_message
            let user = state.user_core.find_by_id(user_id)
                .map_err(|e| format!("Failed to get user: {:?}", e))?
                .ok_or_else(|| "User not found".to_string())?;
            match crate::tool_call_utils::bridge::handle_send_chat_message(
                state, user_id, arguments, &user, None
            ).await {
                Ok((_, _, json_resp)) => Ok(json_resp.message.clone()),
                Err(e) => Err(format!("Failed to send chat message: {}", e))
            }
        }
        "send_email" => {
            let user = state.user_core.find_by_id(user_id)
                .map_err(|e| format!("Failed to get user: {:?}", e))?
                .ok_or_else(|| "User not found".to_string())?;
            match crate::tool_call_utils::email::handle_send_email(
                state, user_id, arguments, &user
            ).await {
                Ok((_, _, json_resp)) => Ok(json_resp.message.clone()),
                Err(e) => Err(format!("Failed to send email: {}", e))
            }
        }
        "fetch_calendar_events" => {
            Ok(crate::tool_call_utils::calendar::handle_fetch_calendar_events(
                state, user_id, arguments
            ).await)
        }
        "ask_perplexity" => {
            #[derive(Deserialize)]
            struct PerplexityArgs {
                query: String,
            }
            let args: PerplexityArgs = serde_json::from_str(arguments).map_err(|e| e.to_string())?;
            let sys_prompt = "You are helping with a scheduled task. Provide concise, actionable information.";
            crate::utils::tool_exec::ask_perplexity(state, &args.query, sys_prompt)
                .await
                .map_err(|e| e.to_string())
        }
        _ => {
            Err(format!("Unknown tool: {}", tool_name))
        }
    }
}

/// Execute a task's action_spec using AI + tools.
///
/// This function:
/// 1. Calls AI with the action_spec and available tools
/// 2. Executes up to 2 tool calls
/// 3. Returns the result
///
/// `trigger_context` - Optional context about what triggered the task (e.g., the incoming message)
pub async fn execute_action_spec(
    state: &Arc<AppState>,
    user_id: i32,
    action_spec: &str,
    notification_type: &str,
    trigger_context: Option<&str>,
) -> ActionResult {
    tracing::debug!("Executing action_spec for user {}: {}", user_id, action_spec);

    // Create AI client for this user
    let (client, provider) = match create_openai_client_for_user(state, user_id) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to create AI client for user {}: {}", user_id, e);
            return ActionResult::Failed {
                error: format!("Failed to create AI client: {}", e)
            };
        }
    };

    let system_prompt = format!(
        "You are a task executor. Execute the following task instructions using the available tools.\n\n\
        IMPORTANT:\n\
        - You MUST use tools to complete the task\n\
        - Maximum 2 tool calls allowed\n\
        - Use send_reminder to notify the user about results or reminders\n\
        - Be concise and action-oriented\n\
        - When sending notifications, ALWAYS include WHY the action was triggered (the trigger context) along with what action was taken. \
        For example: 'Ville messaged about leaving soon, so I turned on your Tesla climate.' NOT just 'Turned on Tesla climate.'\n\n\
        Notification preference: {}",
        notification_type
    );

    // Build the user message with task and optional trigger context
    let user_message = if let Some(context) = trigger_context {
        format!(
            "Execute this task:\n\n{}\n\n---\nTrigger context (the event that triggered this task):\n{}",
            action_spec, context
        )
    } else {
        format!("Execute this task:\n\n{}", action_spec)
    };

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
            content: chat_completion::Content::Text(user_message),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
    ];

    let tools = get_task_runtime_tools();
    let model = state.ai_config.model(provider, ModelPurpose::Default).to_string();

    // First AI call - get tool calls
    let request = chat_completion::ChatCompletionRequest::new(model.clone(), messages.clone())
        .tools(tools)
        .tool_choice(chat_completion::ToolChoiceType::Required)
        .max_tokens(300);

    let result = match client.chat_completion(request).await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("AI call failed for task execution: {}", e);
            return ActionResult::Failed {
                error: format!("AI call failed: {}", e)
            };
        }
    };

    // Extract tool calls
    let tool_calls = match result.choices[0].message.tool_calls.as_ref() {
        Some(calls) => calls,
        None => {
            tracing::warn!("No tool calls returned for action_spec");
            return ActionResult::Failed {
                error: "AI did not return any tool calls".to_string()
            };
        }
    };

    // Execute tool calls (max 2)
    let mut tool_answers: HashMap<String, String> = HashMap::new();
    let mut executed_count = 0;

    for tool_call in tool_calls.iter().take(2) {
        let tool_call_id = tool_call.id.clone();
        let tool_name = match &tool_call.function.name {
            Some(n) => n,
            None => continue,
        };
        let arguments = match &tool_call.function.arguments {
            Some(a) => a,
            None => continue,
        };

        tracing::debug!("Executing tool call: {} with args: {}", tool_name, arguments);

        match execute_tool_call(state, user_id, tool_name, arguments, notification_type).await {
            Ok(answer) => {
                tracing::debug!("Tool {} succeeded: {}", tool_name, answer);
                tool_answers.insert(tool_call_id, answer);
                executed_count += 1;
            }
            Err(e) => {
                tracing::error!("Tool {} failed: {}", tool_name, e);
                tool_answers.insert(tool_call_id, format!("Error: {}", e));
            }
        }
    }

    if executed_count == 0 {
        return ActionResult::Failed {
            error: "No tools executed successfully".to_string()
        };
    }

    // Build follow-up messages with tool results
    let mut follow_up_messages = messages;
    follow_up_messages.push(chat_completion::ChatCompletionMessage {
        role: chat_completion::MessageRole::assistant,
        content: chat_completion::Content::Text(
            result.choices[0].message.content.clone().unwrap_or_default()
        ),
        name: None,
        tool_calls: result.choices[0].message.tool_calls.clone(),
        tool_call_id: None,
    });

    // Add tool responses
    for tool_call in tool_calls.iter().take(2) {
        let tool_answer = tool_answers.get(&tool_call.id).cloned().unwrap_or_default();
        follow_up_messages.push(chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::tool,
            content: chat_completion::Content::Text(tool_answer),
            name: None,
            tool_calls: None,
            tool_call_id: Some(tool_call.id.clone()),
        });
    }

    // Follow-up AI call for summary
    let follow_up_request = chat_completion::ChatCompletionRequest::new(model, follow_up_messages)
        .max_tokens(150);

    let final_response = match client.chat_completion(follow_up_request).await {
        Ok(r) => r.choices[0].message.content.clone().unwrap_or_else(|| "Task completed".to_string()),
        Err(e) => {
            tracing::warn!("Follow-up AI call failed, using default response: {}", e);
            "Task completed".to_string()
        }
    };

    tracing::info!("Task execution completed for user {}: {}", user_id, final_response);
    ActionResult::Success { message: final_response }
}
