
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
