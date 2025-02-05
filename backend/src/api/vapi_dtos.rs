use serde::{Serialize, Deserialize};
use axum::response::{IntoResponse, Response, Json};

#[derive(Serialize, Debug)]
pub struct ServerResponse {
    pub status: String,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

impl IntoResponse for ServerResponse {
    fn into_response(self) -> Response {
        Json(self).into_response()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MessageResponse {
    pub message: Message,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AssistantOverrides {
    pub first_message: String,
    pub variable_values: VariableValues,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VariableValues {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    #[serde(rename = "type")]
    pub message_type: String,
    pub customer: Option<Customer>,
    pub tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Customer {
    pub number: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub r#type: String,
    pub function: Function,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Function {
    pub name: String,
    pub arguments: serde_json::Value,
}

impl MessageResponse {
    pub fn get_phone_number(&self) -> Option<String> {
        self.message.customer.as_ref().map(|c| c.number.clone())
    }

    pub fn get_request_type(&self) -> String {
        self.message.message_type.clone()
    }
    
    pub fn get_tool_calls(&self) -> Option<Vec<ToolCall>> {
        self.message.tool_calls.clone()
    }
}
