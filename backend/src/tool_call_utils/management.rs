
pub fn get_create_waiting_check_tool() -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;

    let mut waiting_check_properties = HashMap::new();
    waiting_check_properties.insert(
        "content".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The content to look for in incoming messages".to_string()),
            ..Default::default()
        }),
    );
    waiting_check_properties.insert(
        "service_type".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("Which service to monitor. Must be either \"messaging\" or \"email\".".to_string()),
            enum_values: Some(vec!["messaging".to_string(), "email".to_string()]),
            ..Default::default()
        }),
    );

    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("create_waiting_check"),
            description: Some(String::from(
                "Creates a waiting check for monitoring email or messages. \
                 Use this when the user wants to be notified in the future about specific content that appears \
                 in their emails or messaging apps such as WhatsApp.",
            )),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(waiting_check_properties),
                required: Some(vec![String::from("content"), String::from("service_type")]),
            },
        },
    }
}


pub fn get_delete_sms_conversation_history_tool() -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;

    let mut placeholder_properties = HashMap::new();
    placeholder_properties.insert(
        "param".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("put nothing here".to_string()),
            ..Default::default()
        }),
    );


    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("delete_sms_conversation_history"),
            description: Some(String::from("Deletes all sms conversation history for a specific user. Use this when user asks to delete their chat history or conversations. It won't delete the history from their phone obviously.")),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(placeholder_properties.clone()),
                required: None,
            },
        },
    }
}

use serde::Deserialize;
use std::sync::Arc;
use crate::AppState;
use std::error::Error;

#[derive(Deserialize)]
pub struct WaitingCheckArgs {
    pub content: String,
    pub service_type: String,
}

pub async fn handle_create_waiting_check(
    state: &Arc<AppState>,
    user_id: i32,
    args: &str,
) -> Result<String, Box<dyn Error>> {
    let args: WaitingCheckArgs = serde_json::from_str(args)?;

    let new_check = crate::models::user_models::NewWaitingCheck {
        user_id,
        content: args.content,
        service_type: args.service_type,
    };

    state.user_repository.create_waiting_check(&new_check).map_err(|e| Box::new(e) as Box<dyn Error>)?;

    Ok("I'll keep an eye out for that and notify you when I find it.".to_string())
}
