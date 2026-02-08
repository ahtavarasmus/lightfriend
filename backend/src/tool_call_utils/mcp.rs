//! MCP Tool Integration
//!
//! Provides functions to:
//! 1. Get dynamic MCP tools for a user to include in AI requests
//! 2. Route and handle MCP tool calls from the AI

use crate::repositories::mcp_repository::McpRepository;
use crate::services::mcp_client::McpClientService;
use crate::AppState;
use openai_api_rs::v1::{chat_completion, types};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Get all MCP tools for a user, converted to OpenAI function calling format.
///
/// Tool names are prefixed with "mcp:{server_name}:" to enable routing.
/// For example, a tool "turn_on_light" from server "homeassistant" becomes:
/// "mcp:homeassistant:turn_on_light"
pub async fn get_mcp_tools_for_user(
    state: &Arc<AppState>,
    user_id: i32,
) -> Vec<chat_completion::Tool> {
    let mut tools = Vec::new();

    let mcp_repository = McpRepository::new(state.db_pool.clone());

    // Get user's enabled MCP servers
    let servers = match mcp_repository.get_enabled_servers_for_user(user_id) {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to get MCP servers for user {}: {}", user_id, e);
            return tools;
        }
    };

    if servers.is_empty() {
        return tools;
    }

    debug!(
        "Found {} enabled MCP servers for user {}",
        servers.len(),
        user_id
    );

    let mcp_client = McpClientService::new();
    let server_count = servers.len();

    for server in servers {
        // Decrypt URL and auth token
        let url = match mcp_repository.get_decrypted_url(&server) {
            Ok(u) => u,
            Err(e) => {
                warn!(
                    "Failed to decrypt URL for MCP server '{}': {}",
                    server.name, e
                );
                continue;
            }
        };

        let auth_token = match mcp_repository.get_decrypted_auth_token(&server) {
            Ok(t) => t,
            Err(e) => {
                warn!(
                    "Failed to decrypt auth token for MCP server '{}': {}",
                    server.name, e
                );
                continue;
            }
        };

        // List tools from this server
        match mcp_client.list_tools(&url, auth_token.as_deref()).await {
            Ok(mcp_tools) => {
                debug!(
                    "Got {} tools from MCP server '{}'",
                    mcp_tools.len(),
                    server.name
                );

                for mcp_tool in mcp_tools {
                    // Convert MCP tool schema to OpenAI format
                    let parameters = convert_mcp_schema_to_openai(&mcp_tool.input_schema);

                    // Create prefixed tool name
                    let tool_name = format!("mcp:{}:{}", server.name, mcp_tool.name);

                    let description = mcp_tool
                        .description
                        .unwrap_or_else(|| format!("Tool from {} MCP server", server.name));

                    tools.push(chat_completion::Tool {
                        r#type: chat_completion::ToolType::Function,
                        function: types::Function {
                            name: tool_name,
                            description: Some(description),
                            parameters,
                        },
                    });
                }
            }
            Err(e) => {
                warn!(
                    "Failed to list tools from MCP server '{}': {}",
                    server.name, e
                );
            }
        }
    }

    info!(
        "Loaded {} MCP tools for user {} from {} servers",
        tools.len(),
        user_id,
        server_count
    );

    tools
}

/// Handle an MCP tool call from the AI.
///
/// Tool names are in format "mcp:{server_name}:{tool_name}"
pub async fn handle_mcp_tool_call(
    state: &Arc<AppState>,
    user_id: i32,
    tool_name: &str,
    arguments: &str,
) -> String {
    // Parse the tool name: mcp:servername:toolname
    let parts: Vec<&str> = tool_name.splitn(3, ':').collect();
    if parts.len() != 3 || parts[0] != "mcp" {
        return format!("Invalid MCP tool name format: {}", tool_name);
    }

    let server_name = parts[1];
    let actual_tool_name = parts[2];

    debug!(
        "Handling MCP tool call: server='{}', tool='{}' for user {}",
        server_name, actual_tool_name, user_id
    );

    let mcp_repository = McpRepository::new(state.db_pool.clone());

    // Look up server config
    let server = match mcp_repository.get_server_by_name(user_id, server_name) {
        Ok(Some(s)) => s,
        Ok(None) => {
            return format!(
                "MCP server '{}' not found or not enabled for your account",
                server_name
            );
        }
        Err(e) => {
            error!("Failed to get MCP server '{}': {}", server_name, e);
            return format!("Error accessing MCP server: {}", e);
        }
    };

    // Check if server is enabled
    if server.is_enabled != 1 {
        return format!("MCP server '{}' is currently disabled", server_name);
    }

    // Decrypt credentials
    let url = match mcp_repository.get_decrypted_url(&server) {
        Ok(u) => u,
        Err(e) => {
            error!(
                "Failed to decrypt URL for MCP server '{}': {}",
                server_name, e
            );
            return "Error: Failed to access server credentials".to_string();
        }
    };

    let auth_token = match mcp_repository.get_decrypted_auth_token(&server) {
        Ok(t) => t,
        Err(e) => {
            error!(
                "Failed to decrypt auth token for MCP server '{}': {}",
                server_name, e
            );
            return "Error: Failed to access server credentials".to_string();
        }
    };

    // Parse arguments
    let args: Value = match serde_json::from_str(arguments) {
        Ok(v) => v,
        Err(e) => {
            warn!("Failed to parse MCP tool arguments: {}", e);
            serde_json::json!({})
        }
    };

    // Call the MCP tool
    let mcp_client = McpClientService::new();
    match mcp_client
        .call_tool(&url, auth_token.as_deref(), actual_tool_name, args)
        .await
    {
        Ok(result) => {
            info!(
                "MCP tool '{}' executed successfully for user {}",
                actual_tool_name, user_id
            );
            result
        }
        Err(e) => {
            error!(
                "MCP tool '{}' failed for user {}: {}",
                actual_tool_name, user_id, e
            );
            format!("MCP tool error: {}", e)
        }
    }
}

/// Check if a tool name is an MCP tool
pub fn is_mcp_tool(tool_name: &str) -> bool {
    tool_name.starts_with("mcp:")
}

/// Convert MCP JSON Schema to OpenAI FunctionParameters format
fn convert_mcp_schema_to_openai(schema: &Value) -> types::FunctionParameters {
    // MCP uses standard JSON Schema which is largely compatible with OpenAI's format
    // We need to extract properties and required fields

    let schema_type = types::JSONSchemaType::Object;

    let properties: Option<HashMap<String, Box<types::JSONSchemaDefine>>> = schema
        .get("properties")
        .and_then(|p| p.as_object())
        .map(|props| {
            props
                .iter()
                .map(|(name, value)| {
                    let define = json_value_to_schema_define(value);
                    (name.clone(), Box::new(define))
                })
                .collect()
        });

    let required: Option<Vec<String>> =
        schema
            .get("required")
            .and_then(|r| r.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            });

    types::FunctionParameters {
        schema_type,
        properties,
        required,
    }
}

/// Convert a JSON Schema value to JSONSchemaDefine
fn json_value_to_schema_define(value: &Value) -> types::JSONSchemaDefine {
    let schema_type = value.get("type").and_then(|t| t.as_str()).map(|t| match t {
        "string" => types::JSONSchemaType::String,
        "number" | "integer" => types::JSONSchemaType::Number, // JSON Schema integer maps to Number
        "boolean" => types::JSONSchemaType::Boolean,
        "array" => types::JSONSchemaType::Array,
        "object" => types::JSONSchemaType::Object,
        _ => types::JSONSchemaType::String, // Default to string
    });

    let description = value
        .get("description")
        .and_then(|d| d.as_str())
        .map(String::from);

    let enum_values = value.get("enum").and_then(|e| e.as_array()).map(|arr| {
        arr.iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect()
    });

    // Handle nested properties for object types
    let properties: Option<HashMap<String, Box<types::JSONSchemaDefine>>> = value
        .get("properties")
        .and_then(|p| p.as_object())
        .map(|props| {
            props
                .iter()
                .map(|(name, prop_value)| {
                    let define = json_value_to_schema_define(prop_value);
                    (name.clone(), Box::new(define))
                })
                .collect()
        });

    let required: Option<Vec<String>> =
        value.get("required").and_then(|r| r.as_array()).map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        });

    // Handle array items
    let items = value.get("items").map(|i| {
        let item_define = json_value_to_schema_define(i);
        Box::new(item_define)
    });

    types::JSONSchemaDefine {
        schema_type,
        description,
        enum_values,
        properties,
        required,
        items,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_mcp_tool() {
        assert!(is_mcp_tool("mcp:homeassistant:turn_on_light"));
        assert!(is_mcp_tool("mcp:custom:do_something"));
        assert!(!is_mcp_tool("get_weather"));
        assert!(!is_mcp_tool("control_tesla"));
    }

    #[test]
    fn test_convert_simple_schema() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum results"
                }
            },
            "required": ["query"]
        });

        let params = convert_mcp_schema_to_openai(&schema);
        assert!(params.properties.is_some());
        assert!(params.required.is_some());
        assert_eq!(params.required.as_ref().unwrap().len(), 1);
        assert_eq!(params.required.as_ref().unwrap()[0], "query");
    }

    #[test]
    fn test_parse_mcp_tool_name() {
        let tool_name = "mcp:homeassistant:turn_on_light";
        let parts: Vec<&str> = tool_name.splitn(3, ':').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "mcp");
        assert_eq!(parts[1], "homeassistant");
        assert_eq!(parts[2], "turn_on_light");
    }
}
