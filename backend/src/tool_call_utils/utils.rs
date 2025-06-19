use openai_api_rs::v1::{
    api::OpenAIClient,
    chat_completion,
    types,
};
use crate::twilio_sms::TwilioMessageResponse;
use std::env;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: chat_completion::Content,
}

// Function to create OpenAI client
pub fn create_openai_client() -> Result<OpenAIClient, Box<dyn std::error::Error>> {
    let api_key = env::var("OPENROUTER_API_KEY")?;
    
    OpenAIClient::builder()
        .with_endpoint("https://openrouter.ai/api/v1")
        .with_api_key(api_key)
        .build()
        .map_err(|e| e.into())
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

// Function to create clarification tool properties
pub fn create_clarify_properties() -> HashMap<String, Box<types::JSONSchemaDefine>> {
    let mut clarify_properties = HashMap::new();
    clarify_properties.insert(
        "is_clarifying".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Boolean),
            description: Some("Whether the AI's response is asking a clarifying question instead of providing an answer".to_string()),
            ..Default::default()
        }),
    );
    clarify_properties.insert(
        "explanation".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Brief explanation of why this is or isn't a clarifying question without revealing conversation content.".to_string()),
            ..Default::default()
        }),
    );
    clarify_properties
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
pub struct ClarifyResponse {
    #[serde(deserialize_with = "deserialize_bool")]
    pub is_clarifying: bool,
    pub explanation: Option<String>,
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

pub async fn perform_clarification_check(
    client: &OpenAIClient,
    messages: &[ChatMessage],
    user_message: &str,
    ai_response: &str,
) -> (bool, Option<String>) {
    let mut clarify_messages = vec![
        chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::system,
            content: chat_completion::Content::Text(
                "You are an evaluator that determines if an AI response is asking for REQUIRED information OR is seeking for CONFIRMATION to complete the UNFINISHED task. Unfinished task is an answer where the AI did not provide ANY useful information yet to the user. Default to FALSE otherwise.\n\n\
                Examples of TRUE clarifying questions:\n\
                - User: 'Send a message to mom' -> AI: 'I see multiple contacts named mom. Which one should I send the message to?'\n\
                - User: 'Check my calendar' -> AI: 'For which date range would you like me to check your calendar?'\n\
                - User: 'What's the weather?' -> AI: 'Which location would you like the weather for?'\n\n\
                - User: 'Can you send message hey hows it going to mom?' -> AI: 'I found the contact \'Mom\' on WhatsApp. Do you want me to send \'hey hows it going\' to this contact?'\n\n\
                Examples that should be FALSE (complete answers with optional follow-ups):\n\
                - User: 'Show contacts named mom' -> AI: 'You have 2 contacts: 1. Mom (mobile) 2. Mom (work).'\n\
                - User: 'Get my recent emails' -> AI: 'Here are your latest emails: [email list]. Would you like to see more?'\n\
                - User: 'Check weather in London' -> AI: 'It's sunny and 20°C in London. Would you like to check another city?'\n\n\
                Key rules:\n\
                2. Follow-up questions after answering the original question are NOT clarifying questions\n\
                3. Only mark TRUE if the AI is asking to confirm the data it got from the user or it CANNOT provide an answer without more information\n\
                4. When in doubt, return FALSE".to_string(),
            ),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        },
    ];

    // Add up to 3 previous message pairs for context (only user and assistant messages)
    let context_messages: Vec<_> = messages
        .iter()
        .filter(|msg| msg.role == "user" || msg.role == "assistant")
        .rev()
        .take(6) // Take 6 messages (3 pairs of user-assistant exchanges)
        .collect();

    if !context_messages.is_empty() {
        clarify_messages.push(chat_completion::ChatCompletionMessage {
            role: chat_completion::MessageRole::user,
            content: chat_completion::Content::Text(
                format!(
                    "Previous conversation:\n{}",
                    context_messages
                        .iter()
                        .rev() // Reverse back to chronological order
                        .map(|msg| {
                            let role = match msg.role.as_str() {
                                "user" => "User",
                                "assistant" => "AI",
                                _ => "Unknown", // Shouldn't happen due to filter
                            };
                            let text = extract_text_from_content(&msg.content);
                            format!("[{}]: {}", role, text)
                        })
                        .collect::<Vec<String>>()
                        .join("\n")
                )
            ),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        });
    }

    // Add the current exchange
    clarify_messages.push(chat_completion::ChatCompletionMessage {
        role: chat_completion::MessageRole::user,
        content: chat_completion::Content::Text(format!(
            "Current exchange:\nUser: {}\nAI: {}",
            user_message,
            ai_response
        )),
        name: None,
        tool_calls: None,
        tool_call_id: None,
    });

    let clarify_req = chat_completion::ChatCompletionRequest::new(
        "openai/gpt-4o-mini".to_string(),
        clarify_messages,
    )
    .tools(create_clarify_tools())
    .tool_choice(chat_completion::ToolChoiceType::Required)
    .max_tokens(100);

    match client.chat_completion(clarify_req).await {
        Ok(result) => {
            if let Some(tool_calls) = result.choices[0].message.tool_calls.as_ref() {
                if let Some(first_call) = tool_calls.first() {
                    if let Some(args) = &first_call.function.arguments {
                        match serde_json::from_str::<ClarifyResponse>(args) {
                            Ok(clarify) => {
                                tracing::debug!(
                                    "Clarification check result: is_clarifying={}, explanation={:?}",
                                    clarify.is_clarifying,
                                    clarify.explanation
                                );
                                (clarify.is_clarifying, clarify.explanation)
                            }
                            Err(e) => {
                                tracing::error!("Failed to parse clarification response: {}", e);
                                (false, Some("Failed to parse clarification check".to_string()))
                            }
                        }
                    } else {
                        tracing::error!("No arguments found in clarification tool call");
                        (false, Some("Missing clarification check arguments".to_string()))
                    }
                } else {
                    tracing::error!("No clarification tool calls found");
                    (false, Some("No clarification check tool calls found".to_string()))
                }
            } else {
                tracing::error!("No tool calls section in clarification response");
                (false, Some("No clarification check tool calls received".to_string()))
            }
        }
        Err(e) => {
            tracing::error!("Failed to get clarification check response: {}", e);
            (false, Some("Failed to get clarification check response".to_string()))
        }
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


// Function to create clarification tools
pub fn create_clarify_tools() -> Vec<chat_completion::Tool> {
    vec![
        chat_completion::Tool {
            r#type: chat_completion::ToolType::Function,
            function: types::Function {
                name: String::from("check_clarification"),
                description: Some(String::from(
                    "Determines if the AI's response is asking a clarifying question instead of providing an answer"
                )),
                parameters: types::FunctionParameters {
                    schema_type: types::JSONSchemaType::Object,
                    properties: Some(create_clarify_properties()),
                    required: Some(vec![String::from("is_clarifying")]),
                },
            },
        },
    ]
}

// Helper function to check if a tool is accessible based on user's status
pub fn requires_subscription(tool_name: &str, sub_tier: Option<String>, has_discount: bool) -> bool {
    // Tier 2 subscribers get access to everything
    if Some("tier 2".to_string()) == sub_tier || has_discount {
        println!("✅ User has tier 2 subscription - granting full access");
        return false;
    } else if Some("tier 1".to_string()) == sub_tier {
        let in_allowed_tools = tool_name.contains("perplexity") ||
            tool_name.contains("weather") ||
            tool_name.contains("assistant");

        if in_allowed_tools {
            return false;
        }
    }
    return true;
}


// Function to create evaluation tools
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
