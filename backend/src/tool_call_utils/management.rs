
pub fn get_create_waiting_check_tool() -> openai_api_rs::v1::chat_completion::Tool {
    use openai_api_rs::v1::{chat_completion, types};
    use std::collections::HashMap;

    let mut waiting_check_properties = HashMap::new();
    waiting_check_properties.insert(
        "content".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::String),
            description: Some("The content to look for in emails".to_string()),
            ..Default::default()
        }),
    );
    waiting_check_properties.insert(
        "due_date".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Number),
            description: Some("Unix timestamp for when this check should be completed by, default to two weeks into the future.".to_string()),
            ..Default::default()
        }),
    );
    waiting_check_properties.insert(
        "remove_when_found".to_string(),
        Box::new(types::JSONSchemaDefine {
            schema_type: Some(types::JSONSchemaType::Boolean),
            description: Some("Whether to remove the check once the content is found, default to true.".to_string()),
            ..Default::default()
        }),
    );
    
    chat_completion::Tool {
        r#type: chat_completion::ToolType::Function,
        function: types::Function {
            name: String::from("create_waiting_check"),
            description: Some(String::from("Creates a waiting check for monitoring emails. Use this when user wants to be notified about specific emails or content in their emails.")),
            parameters: types::FunctionParameters {
                schema_type: types::JSONSchemaType::Object,
                properties: Some(waiting_check_properties),
                required: Some(vec![String::from("content")]),
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
    pub due_date: Option<i64>,
    pub remove_when_found: Option<bool>,
}

pub async fn handle_create_waiting_check(
    state: &Arc<AppState>,
    user_id: i32,
    args: &str,
) -> Result<String, Box<dyn Error>> {
    let args: WaitingCheckArgs = serde_json::from_str(args)?;

    // Calculate default due date (2 weeks from now) if not provided
    let due_date = args.due_date.unwrap_or_else(|| {
        let two_weeks = chrono::Duration::weeks(2);
        (chrono::Utc::now() + two_weeks).timestamp()
    }) as i32;

    // Default remove_when_found to true if not provided
    let remove_when_found = args.remove_when_found.unwrap_or(true);

    let new_check = crate::models::user_models::NewWaitingCheck {
        user_id,
        due_date,
        content: args.content,
        remove_when_found,
        service_type: "imap".to_string(), // Default to email service type
    };

    state.user_repository.create_waiting_check(&new_check)?;

    Ok("I'll keep an eye out for that in your emails and notify you when I find it.".to_string())
}
