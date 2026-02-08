//! MCP Client Service
//!
//! Connects to remote MCP servers over HTTP (Streamable HTTP transport)
//! to list available tools and execute tool calls.
//!
//! MCP uses JSON-RPC 2.0 over HTTP with Server-Sent Events for responses.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;
use tracing::{debug, info, warn};

/// MCP JSON-RPC request
#[derive(Debug, Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    id: i64,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
}

/// MCP JSON-RPC response
#[derive(Debug, Deserialize)]
struct JsonRpcResponse {
    #[allow(dead_code)]
    jsonrpc: String,
    #[allow(dead_code)]
    id: Option<i64>,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
    #[serde(default)]
    #[allow(dead_code)]
    data: Option<Value>,
}

/// MCP Tool definition from server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, rename = "inputSchema")]
    pub input_schema: Value,
}

/// Response from tools/list
#[derive(Debug, Deserialize)]
struct ListToolsResult {
    tools: Vec<McpTool>,
}

/// Response from tools/call
#[derive(Debug, Deserialize)]
struct CallToolResult {
    #[serde(default)]
    content: Vec<ToolContent>,
    #[serde(default, rename = "isError")]
    is_error: bool,
}

#[derive(Debug, Deserialize)]
struct ToolContent {
    #[serde(rename = "type")]
    content_type: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    data: Option<String>,
    #[serde(default, rename = "mimeType")]
    #[allow(dead_code)]
    mime_type: Option<String>,
}

/// MCP Client for connecting to remote MCP servers
pub struct McpClientService {
    client: Client,
}

impl McpClientService {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self { client }
    }

    /// List available tools from an MCP server
    pub async fn list_tools(
        &self,
        url: &str,
        auth_token: Option<&str>,
    ) -> Result<Vec<McpTool>, String> {
        info!("Listing tools from MCP server: {}", url);

        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: 1,
            method: "tools/list".to_string(),
            params: None,
        };

        let response = self.send_request(url, auth_token, &request).await?;

        match response.result {
            Some(result) => {
                let list_result: ListToolsResult = serde_json::from_value(result)
                    .map_err(|e| format!("Failed to parse tools list: {}", e))?;
                debug!("Found {} tools on MCP server", list_result.tools.len());
                Ok(list_result.tools)
            }
            None => {
                if let Some(err) = response.error {
                    Err(format!(
                        "MCP server error: {} (code {})",
                        err.message, err.code
                    ))
                } else {
                    Err("No result from MCP server".to_string())
                }
            }
        }
    }

    /// Call a tool on an MCP server
    pub async fn call_tool(
        &self,
        url: &str,
        auth_token: Option<&str>,
        tool_name: &str,
        arguments: Value,
    ) -> Result<String, String> {
        info!("Calling MCP tool '{}' on server: {}", tool_name, url);

        let params = serde_json::json!({
            "name": tool_name,
            "arguments": arguments
        });

        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: 2,
            method: "tools/call".to_string(),
            params: Some(params),
        };

        let response = self.send_request(url, auth_token, &request).await?;

        match response.result {
            Some(result) => {
                let call_result: CallToolResult = serde_json::from_value(result)
                    .map_err(|e| format!("Failed to parse tool result: {}", e))?;

                if call_result.is_error {
                    // Extract error message from content
                    let error_texts: Vec<String> = call_result
                        .content
                        .iter()
                        .filter_map(|c| c.text.clone())
                        .collect();
                    let error_text = error_texts.join("\n");
                    return Err(format!("Tool error: {}", error_text));
                }

                // Combine text content from response
                let text_parts: Vec<String> = call_result
                    .content
                    .iter()
                    .filter_map(|c| {
                        if c.content_type == "text" {
                            c.text.clone()
                        } else if c.content_type == "resource" || c.content_type == "image" {
                            // For non-text content, return a description
                            Some(format!("[{} content]", c.content_type))
                        } else {
                            c.text.clone()
                        }
                    })
                    .collect();
                let text_result = text_parts.join("\n");

                if text_result.is_empty() {
                    Ok("Tool executed successfully (no output)".to_string())
                } else {
                    Ok(text_result)
                }
            }
            None => {
                if let Some(err) = response.error {
                    Err(format!(
                        "MCP server error: {} (code {})",
                        err.message, err.code
                    ))
                } else {
                    Err("No result from tool call".to_string())
                }
            }
        }
    }

    /// Test connection to an MCP server
    pub async fn test_connection(
        &self,
        url: &str,
        auth_token: Option<&str>,
    ) -> Result<Vec<McpTool>, String> {
        // First, try to initialize the connection
        let init_request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: 0,
            method: "initialize".to_string(),
            params: Some(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "clientInfo": {
                    "name": "Lightfriend",
                    "version": "1.0.0"
                }
            })),
        };

        // Try to initialize - some servers require this, others don't
        match self.send_request(url, auth_token, &init_request).await {
            Ok(response) => {
                debug!("MCP initialize response: {:?}", response.result);
                // Send initialized notification (some servers require this)
                let _ = self
                    .send_notification(url, auth_token, "notifications/initialized")
                    .await;
            }
            Err(e) => {
                // Some servers don't require initialization, try listing tools anyway
                warn!("MCP initialize failed (may be optional): {}", e);
            }
        }

        // Now list tools to verify the connection works
        self.list_tools(url, auth_token).await
    }

    /// Send a JSON-RPC request to the MCP server
    async fn send_request(
        &self,
        url: &str,
        auth_token: Option<&str>,
        request: &JsonRpcRequest,
    ) -> Result<JsonRpcResponse, String> {
        let mut req = self.client.post(url).json(request);

        if let Some(token) = auth_token {
            req = req.bearer_auth(token);
        }

        let response = req
            .header("Accept", "application/json, text/event-stream")
            .send()
            .await
            .map_err(|e| format!("Failed to connect to MCP server: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(format!("MCP server returned {}: {}", status, error_text));
        }

        // Check content type for SSE vs JSON
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        if content_type.contains("text/event-stream") {
            // Handle SSE response
            self.parse_sse_response(response).await
        } else {
            // Handle regular JSON response
            response
                .json::<JsonRpcResponse>()
                .await
                .map_err(|e| format!("Failed to parse MCP response: {}", e))
        }
    }

    /// Parse Server-Sent Events response
    async fn parse_sse_response(
        &self,
        response: reqwest::Response,
    ) -> Result<JsonRpcResponse, String> {
        let text = response
            .text()
            .await
            .map_err(|e| format!("Failed to read SSE response: {}", e))?;

        // Parse SSE format - look for data: lines
        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data: ") {
                if data.trim().is_empty() {
                    continue;
                }
                // Try to parse as JSON-RPC response
                if let Ok(response) = serde_json::from_str::<JsonRpcResponse>(data) {
                    return Ok(response);
                }
            }
        }

        // If no valid JSON-RPC response found, try parsing the whole thing as JSON
        serde_json::from_str::<JsonRpcResponse>(&text)
            .map_err(|e| format!("Failed to parse SSE response: {}", e))
    }

    /// Send a notification (no response expected)
    async fn send_notification(
        &self,
        url: &str,
        auth_token: Option<&str>,
        method: &str,
    ) -> Result<(), String> {
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method
        });

        let mut req = self.client.post(url).json(&notification);

        if let Some(token) = auth_token {
            req = req.bearer_auth(token);
        }

        let _ = req.send().await;
        Ok(())
    }
}

impl Default for McpClientService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_rpc_request_serialization() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: 1,
            method: "tools/list".to_string(),
            params: None,
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"method\":\"tools/list\""));
        // params should be omitted when None
        assert!(!json.contains("params"));
    }

    #[test]
    fn test_json_rpc_request_with_params() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: 2,
            method: "tools/call".to_string(),
            params: Some(serde_json::json!({"name": "test"})),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"params\""));
    }
}
