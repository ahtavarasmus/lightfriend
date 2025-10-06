use openai_api_rs::v1::{
    api::OpenAIClient,
    chat_completion,
    types,
};
use crate::twilio_sms::TwilioMessageResponse;
use std::env;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use crate::AppState;
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: chat_completion::Content,
    pub tool_calls: Option<Vec<chat_completion::ToolCall>>,
    pub tool_call_id: Option<String>,
}

// Function to create OpenAI client
pub fn create_openai_client(
    state: &Arc<AppState>,
) -> Result<OpenAIClient, Box<dyn std::error::Error>> {

    // TODO get the temp tinfoil api key from lightfriend.ai

    OpenAIClient::builder()
        .with_endpoint("https://openrouter.ai/api/v1")
        .with_api_key("".to_string())
        .build()
        .map_err(|e| e.into())
}


pub async fn cancel_pending_message(
    state: &Arc<AppState>,
    user_id: i32,
) -> Result<bool, Box<dyn std::error::Error>> {
    let mut senders = state.pending_message_senders.lock().await;
    if let Some(sender) = senders.remove(&user_id) {
        let _ = sender.send(());
        Ok(true)  // Cancellation occurred
    } else {
        Ok(false)  // No pending message to cancel
    }
}

// Helper function for boolean deserialization
#[derive(Deserialize)]
#[serde(untagged)]
pub enum BoolValue {
    Bool(bool),
    String(String),
}

impl From<BoolValue> for bool {
    fn from(value: BoolValue) -> Self {
        match value {
            BoolValue::Bool(b) => b,
            BoolValue::String(s) => s.to_lowercase() == "true",
        }
    }
}

pub fn deserialize_bool<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(BoolValue::deserialize(deserializer)?.into())
}

#[derive(Deserialize)]
pub struct EvalResponse {
    #[serde(deserialize_with = "deserialize_bool")]
    pub success: bool,
    pub reason: Option<String>,
}

fn extract_text_from_content(content: &chat_completion::Content) -> String {
    match content {
        chat_completion::Content::Text(text) => text.clone(),
        chat_completion::Content::ImageUrl(urls) => {
            urls.iter()
                .filter_map(|url| url.text.as_ref().map(|t| t.clone()))
                .collect::<Vec<String>>()
                .join(" ")
        },
        _ => "Unsupported content type".to_string(),
    }
}

pub async fn perform_evaluation(
    client: &OpenAIClient,
    messages: &[ChatMessage],
    user_message: &str,
    ai_response: &str,
    fail: bool,
) -> (bool, Option<String>) {
    let eval_messages = vec![
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::system,
            content: chat_completion::Content::Text(
                "You are a conversation evaluator. Assess the latest user's query in the context of the conversation history and the AI's response to it. Use the evaluate_response function to provide feedback.\n\n\
                ### Guidelines:\n\
                - **Success**: True if the AI successfully answered the user's request; false otherwise.".to_string(),
            ),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::user,
            content: chat_completion::Content::Text(format!(
                "Conversation history: {}\nLatest user message: {}\nAI response: {}",
                messages
                    .iter()
                    .filter(|msg| msg.role == "user" || msg.role == "assistant")
                    .map(|msg| {
                        let role = match msg.role.as_str() {
                            "user" => "User",
                            "assistant" => "AI",
                            _ => "Unknown", // Shouldn't happen due to filter
                        };
                        let text = extract_text_from_content(&msg.content);
                        let display_text = if text.chars().count() > 50 {
                            format!("{}...", text.chars().take(50).collect::<String>())
                        } else {
                            text
                        };
                        format!("[{}]: {}", role, display_text)
                    })
                    .collect::<Vec<String>>()
                    .join("\n"),
                user_message,
                ai_response
            )),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
    ];

    let eval_req = chat_completion::ChatCompletionRequest::new(
        "openai/gpt-4o-mini".to_string(),
        eval_messages,
    )
    .tools(create_eval_tools())
    .tool_choice(chat_completion::ToolChoiceType::Required)
    .max_tokens(200);

    match client.chat_completion(eval_req).await {
        Ok(result) => {
            if let Some(tool_calls) = result.choices[0].message.tool_calls.as_ref() {
                if let Some(first_call) = tool_calls.first() {
                    if let Some(args) = &first_call.function.arguments {
                        tracing::debug!("Tool call arguments: {}", args);
                        match serde_json::from_str::<EvalResponse>(args) {
                            Ok(eval) => {
                                tracing::debug!(
                                    "Successfully parsed evaluation response: success={}, reason={:?}",
                                    eval.success,
                                    eval.reason
                                );
                                (eval.success, eval.reason)
                            }
                            Err(e) => {
                                tracing::error!(
                                    "Failed to parse evaluation response: {}, falling back to default",
                                    e
                                );
                                (!fail, Some("Failed to parse evaluation response".to_string()))
                            }
                        }
                    } else {
                        tracing::error!("No arguments found in tool call");
                        (!fail, Some("Missing evaluation arguments".to_string()))
                    }
                } else {
                    tracing::error!("No tool calls found in response");
                    (!fail, Some("No evaluation tool calls found".to_string()))
                }
            } else {
                tracing::error!("No tool calls section in response");
                (!fail, Some("No evaluation tool calls received".to_string()))
            }
        }
        Err(e) => {
            tracing::error!("Failed to get evaluation response: {}", e);
            (!fail, Some("Failed to get evaluation response".to_string()))
        }
    }
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
            description: Some("Brief explanation of why this email was selected as most relevant".to_string()),
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
    client: &OpenAIClient,
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

    let select_tools = vec![
        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: String::from("select_email"),
                description: Some(String::from(
                    "Selects the most relevant email based on the search query"
                )),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(create_email_select_properties()),
                    required: Some(vec![String::from("email_id")]),
                },
            },
        },
    ];

    let select_req = chat_completion::ChatCompletionRequest::new(
        model,
        select_messages,
    )
    .tools(select_tools)
    .tool_choice(chat_completion::ToolChoiceType::Required)
    .max_tokens(200);

    match client.chat_completion(select_req).await {
        Ok(result) => {
            if let Some(tool_calls) = result.choices[0].message.tool_calls.as_ref() {
                if let Some(first_call) = tool_calls.first() {
                    if let Some(args) = &first_call.function.arguments {
                        match serde_json::from_str::<EmailSelectResponse>(args) {
                            Ok(select) => Ok((select.email_id, select.reason)),
                            Err(e) => Err(format!("Failed to parse email selection response: {}", e).into())
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
        Err(e) => Err(format!("Failed to get email selection response: {}", e).into())
    }
}

// Function to create evaluation tool properties
pub fn create_eval_properties() -> HashMap<String, Box<types::JSONSchemaDefine>> {
    let mut eval_properties = HashMap::new();
    eval_properties.insert(
        "success".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Boolean),
            description: Some("Whether the response was successful and provided the information user asked for. Note that the information might not look like success(whatsapp message fetch returns missed call notice), but should still be considered successful.".to_string()),
            ..Default::default()
        }),
    );
    eval_properties.insert(
        "reason".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Reason for failure if success is false, explaining the issue without revealing conversation content".to_string()),
            ..Default::default()
        }),
    );
    eval_properties
}

pub fn create_eval_tools() -> Vec<chat_completion::Tool> {
    vec![
        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: String::from("evaluate_response"),
                description: Some(String::from(
                    "Evaluates the AI response based on success."
                )),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(create_eval_properties()),
                    required: Some(vec![String::from("success")]),
                },
            },
        },
    ]
}
