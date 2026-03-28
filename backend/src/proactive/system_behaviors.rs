use std::collections::HashMap;
use std::sync::Arc;

use crate::context::ContextBuilder;
use crate::proactive::utils::send_notification;
use crate::repositories::user_core::UserCoreOps;
use crate::AppState;
use openai_api_rs::v1::{chat_completion, types};

pub async fn run_system_behaviors(
    state: &Arc<AppState>,
    user_id: i32,
    entity_snapshot: &serde_json::Value,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let settings = state.user_core.get_user_settings(user_id)?;
    if !settings.system_important_notify {
        return Ok(());
    }

    // Skip group messages - too noisy for system defaults
    if entity_snapshot
        .get("is_group")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        return Ok(());
    }

    let ctx = ContextBuilder::for_user(state, user_id)
        .with_user_context()
        .build()
        .await?;

    let tz_info = ctx
        .timezone
        .as_ref()
        .map(|t| format!("Current time: {}", t.formatted_now))
        .unwrap_or_default();

    // Build contacts context from persons
    let contacts_ctx = ctx
        .persons
        .as_ref()
        .map(|persons| {
            let names: Vec<String> = persons
                .iter()
                .filter_map(|p| {
                    let name = &p.person.name;
                    Some(name.clone())
                })
                .take(20)
                .collect();
            if names.is_empty() {
                String::new()
            } else {
                format!("Known contacts: {}", names.join(", "))
            }
        })
        .unwrap_or_default();

    let system_prompt = format!(
        "You are evaluating whether an incoming message is important enough to notify the user.\n\
        \n\
        {}\n\
        {}\n\
        \n\
        Only set should_notify=true if delaying this message over 2 hours could cause harm, \
        financial loss, or miss a time-sensitive opportunity. Examples: emergencies, someone \
        asking to meet now, immediate decisions needed. Routine updates, casual messages, \
        and vague requests are NOT important.\n\
        \n\
        If should_notify=false, set notification_message to empty string.\n\
        If should_notify=true, write a concise notification (max 480 chars, second person).",
        tz_info, contacts_ctx
    );

    let sender_name = entity_snapshot
        .get("sender_name")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown");
    let platform = entity_snapshot
        .get("platform")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let content = entity_snapshot
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let user_msg = format!("Message from {} on {}:\n{}", sender_name, platform, content);

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

    let mut properties = HashMap::new();
    properties.insert(
        "should_notify".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Boolean),
            description: Some(
                "true if this message is important enough to notify the user immediately"
                    .to_string(),
            ),
            ..Default::default()
        }),
    );
    properties.insert(
        "notification_message".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some(
                "Concise notification text (max 480 chars, second person). Empty string if should_notify=false."
                    .to_string(),
            ),
            ..Default::default()
        }),
    );

    let tool = chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: "system_behavior_result".to_string(),
            description: Some("Return importance evaluation result".to_string()),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(vec![
                    "should_notify".to_string(),
                    "notification_message".to_string(),
                ]),
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
        .map_err(|e| format!("System behavior LLM call failed: {}", e))?;

    crate::ai_config::log_llm_usage(
        &state.llm_usage_repository,
        user_id,
        match ctx.provider {
            crate::AiProvider::Tinfoil => "tinfoil",
            crate::AiProvider::OpenRouter => "openrouter",
        },
        &ctx.model,
        "system_important",
        &result,
    );

    let choice = result.choices.first().ok_or("No choices in LLM response")?;

    if let Some(ref tool_calls) = choice.message.tool_calls {
        for tc in tool_calls {
            let fn_name = tc.function.name.as_deref().unwrap_or("");
            if fn_name == "system_behavior_result" {
                let args = tc.function.arguments.as_deref().unwrap_or("{}");
                let parsed: serde_json::Value = serde_json::from_str(args)
                    .map_err(|e| format!("Failed to parse system_behavior_result: {}", e))?;

                let should_notify = parsed
                    .get("should_notify")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if should_notify {
                    let notification_message = parsed
                        .get("notification_message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    if !notification_message.is_empty() {
                        send_notification(
                            state,
                            user_id,
                            notification_message,
                            "system_important".to_string(),
                            None,
                        )
                        .await;
                    }
                }

                return Ok(());
            }
        }
    }

    Ok(())
}
