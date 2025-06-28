use crate::models::user_models::WaitingCheck;
use openai_api_rs::v1::{
    chat_completion,
    types,
};
use crate::tool_call_utils::utils::create_openai_client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct MatchResponse {
    waiting_check_id: Option<i32>,
    is_critical: bool,
    what_to_inform: String,
}

pub async fn check_message_importance(
    message: &str,
    waiting_checks: Vec<WaitingCheck>,
) -> Result<(Option<i32>, bool, String), Box<dyn std::error::Error>> {
    let client = create_openai_client()?;

    let waiting_checks_str = waiting_checks
        .iter()
        .map(|check| {
            format!(
                "ID: {}, Content to watch for: {}",
                check.id.unwrap_or(-1),
                check.content,
            )
        })
        .collect::<Vec<String>>()
        .join("\n");

    let messages = vec![
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::system,
            content: chat_completion::Content::Text(
                "You are an AI that analyzes messages to determine if they match any waiting checks or if they are otherwise critical and require immediate attention. A message is considered critical ONLY if it absolutely cannot wait to be mentioned in the next scheduled notification summary - it must be something that requires truly immediate attention like emergencies, extremely time-sensitive matters, or critical updates that would be problematic if delayed until the next summary. Most normal updates, even if important, should wait for the scheduled summary unless they are genuinely urgent and time-critical. When reporting critical messages, provide an extremely concise SMS-friendly message (ideally under 160 characters) that clearly states what requires immediate attention.".to_string(),
            ),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::user,
            content: chat_completion::Content::Text(format!(
                "Analyze this message:\n\n{}\n\nAgainst these waiting checks:\n\n{}\n\nDetermine:\n1. If it matches any waiting check (return the ID if it does)\n2. If the message is otherwise critical and requires immediate attention",
                message, waiting_checks_str
            )),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
    ];

    let mut properties = std::collections::HashMap::new();
    properties.insert(
        "waiting_check_id".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Number),
            description: Some("The ID of the matched waiting check, if any. None if no match found.".to_string()),
            ..Default::default()
        }),
    );
    properties.insert(
        "is_critical".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Boolean),
            description: Some("Whether the message is critical and requires immediate attention".to_string()),
            ..Default::default()
        }),
    );
    properties.insert(
        "what_to_inform".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Concise SMS-friendly message (under 160 chars) about what requires immediate attention".to_string()),
            ..Default::default()
        }),
    );

    let tools = vec![chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("analyze_message"),
            description: Some(String::from(
                "Analyzes if a message matches any waiting checks or is critical"
            )),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(properties),
                required: Some(vec![
                    String::from("is_critical"),
                    String::from("what_to_inform"),
                ]),
            },
        },
    }];

    let request = chat_completion::ChatCompletionRequest::new(
        "openai/gpt-4o-mini".to_string(),
        messages,
    )
    .tools(tools)
    .tool_choice(chat_completion::ToolChoiceType::Required)
    .max_tokens(200);

    match client.chat_completion(request).await {
        Ok(result) => {
            if let Some(tool_calls) = result.choices[0].message.tool_calls.as_ref() {
                if let Some(first_call) = tool_calls.first() {
                    if let Some(args) = &first_call.function.arguments {
                        match serde_json::from_str::<MatchResponse>(args) {
                            Ok(response) => {
                                tracing::debug!(
                                    "Message analysis result: check_id={:?}, critical={}, message={}",
                                    response.waiting_check_id,
                                    response.is_critical,
                                    response.what_to_inform
                                );
                                Ok((response.waiting_check_id, response.is_critical, response.what_to_inform))
                            }
                            Err(e) => {
                                tracing::error!("Failed to parse message analysis response: {}", e);
                                Ok((None, false, "".to_string()))
                            }
                        }
                    } else {
                        tracing::error!("No arguments found in tool call");
                        Ok((None, false, "".to_string()))
                    }
                } else {
                    tracing::error!("No tool calls found");
                    Ok((None, false, "".to_string()))
                }
            } else {
                tracing::error!("No tool calls section in response");
                Ok((None, false, "".to_string()))
            }
        }
        Err(e) => {
            tracing::error!("Failed to get message analysis: {}", e);
            Err(e.into())
        }
    }
}

