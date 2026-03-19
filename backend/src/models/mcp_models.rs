use serde::{Deserialize, Serialize};

/// MCP Tool discovered from a remote server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: serde_json::Value,
}

/// Response for API - decrypted server info (without sensitive tokens)
#[derive(Debug, Clone, Serialize)]
pub struct McpServerResponse {
    pub id: i32,
    pub name: String,
    pub url: String, // Decrypted URL for display
    pub has_auth_token: bool,
    pub is_enabled: bool,
    pub created_at: i32,
}

/// Request to create a new MCP server
#[derive(Debug, Clone, Deserialize)]
pub struct CreateMcpServerRequest {
    pub name: String,
    pub url: String,
    pub auth_token: Option<String>,
}

/// Response for test connection
#[derive(Debug, Clone, Serialize)]
pub struct McpTestConnectionResponse {
    pub success: bool,
    pub tools_count: Option<usize>,
    pub tools: Option<Vec<McpToolInfo>>,
    pub error: Option<String>,
}

/// Tool info for display
#[derive(Debug, Clone, Serialize)]
pub struct McpToolInfo {
    pub name: String,
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<serde_json::Value>,
}
