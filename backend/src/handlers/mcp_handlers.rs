//! MCP Server API Handlers
//!
//! Endpoints for managing custom MCP server connections.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::sync::Arc;
use tokio::net::lookup_host;
use tracing::{error, info};
use url::Url;

use crate::handlers::auth_middleware::AuthUser;
use crate::models::mcp_models::{
    CreateMcpServerRequest, McpServerResponse, McpTestConnectionResponse, McpToolInfo,
};
use crate::repositories::mcp_repository::McpRepository;
use crate::services::mcp_client::McpClientService;
use crate::AppState;

fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            ipv4.is_private()
                || ipv4.is_loopback()
                || ipv4.is_link_local()
                || ipv4.is_unspecified()
                || ipv4.octets()[0] == 169 && ipv4.octets()[1] == 254
        }
        IpAddr::V6(ipv6) => {
            ipv6.is_loopback()
                || ipv6.is_unspecified()
                || ipv6.is_unique_local()
                || ipv6.is_unicast_link_local()
        }
    }
}

async fn validate_public_mcp_url(url: &str) -> Result<(), String> {
    let parsed = Url::parse(url).map_err(|_| "Invalid URL".to_string())?;
    let host = parsed
        .host_str()
        .ok_or_else(|| "URL must include a host".to_string())?;
    let port = parsed
        .port_or_known_default()
        .ok_or_else(|| "URL must include a valid port".to_string())?;

    if host.eq_ignore_ascii_case("localhost") {
        return Err("Localhost MCP URLs are not allowed".to_string());
    }

    if let Ok(ip) = host.parse::<IpAddr>() {
        if is_private_ip(ip) {
            return Err("Private or local MCP URLs are not allowed".to_string());
        }
        return Ok(());
    }

    let resolved = lookup_host((host, port))
        .await
        .map_err(|_| "Failed to resolve MCP server host".to_string())?;

    for addr in resolved {
        if is_private_ip(addr.ip()) {
            return Err("MCP URL resolves to a private or local address".to_string());
        }
    }

    Ok(())
}

/// POST /api/mcp/servers - Add a new MCP server
pub async fn create_mcp_server(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(request): Json<CreateMcpServerRequest>,
) -> Result<Json<McpServerResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!(
        "Creating MCP server '{}' for user {}",
        request.name, auth_user.user_id
    );

    // Validate the name
    if request.name.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Server name cannot be empty".to_string(),
            }),
        ));
    }

    // Validate the name doesn't contain special characters that could cause issues
    if !request
        .name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Server name can only contain letters, numbers, hyphens, and underscores"
                    .to_string(),
            }),
        ));
    }

    // Validate URL format
    if !request.url.starts_with("http://") && !request.url.starts_with("https://") {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "URL must start with http:// or https://".to_string(),
            }),
        ));
    }

    validate_public_mcp_url(&request.url)
        .await
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;

    let mcp_repository = McpRepository::new(state.pg_pool.clone());

    // Check if server with this name already exists for user
    if let Ok(Some(_)) = mcp_repository.get_server_by_name(auth_user.user_id, &request.name) {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!("Server '{}' already exists", request.name),
            }),
        ));
    }

    // Create the server
    match mcp_repository.create_server(
        auth_user.user_id,
        &request.name,
        &request.url,
        request.auth_token.as_deref(),
    ) {
        Ok(server) => {
            let response = mcp_repository.to_response(&server).map_err(|e| {
                error!("Failed to create response: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "Failed to create server response".to_string(),
                    }),
                )
            })?;
            Ok(Json(response))
        }
        Err(e) => {
            error!("Failed to create MCP server: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to create server: {}", e),
                }),
            ))
        }
    }
}

/// GET /api/mcp/servers - List all MCP servers for user
pub async fn list_mcp_servers(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<Vec<McpServerResponse>>, (StatusCode, Json<ErrorResponse>)> {
    let mcp_repository = McpRepository::new(state.pg_pool.clone());

    match mcp_repository.get_servers_for_user(auth_user.user_id) {
        Ok(servers) => {
            let responses: Result<Vec<_>, _> = servers
                .iter()
                .map(|s| mcp_repository.to_response(s))
                .collect();

            match responses {
                Ok(resp) => Ok(Json(resp)),
                Err(e) => {
                    error!("Failed to convert servers: {}", e);
                    Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: "Failed to process servers".to_string(),
                        }),
                    ))
                }
            }
        }
        Err(e) => {
            error!("Failed to list MCP servers: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to list servers: {}", e),
                }),
            ))
        }
    }
}

/// GET /api/mcp/servers/:id/tools - List discovered tools from server
pub async fn list_server_tools(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(server_id): Path<i32>,
) -> Result<Json<McpTestConnectionResponse>, (StatusCode, Json<ErrorResponse>)> {
    let mcp_repository = McpRepository::new(state.pg_pool.clone());

    // Get the server
    let server = match mcp_repository.get_server_by_id(server_id, auth_user.user_id) {
        Ok(Some(s)) => s,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Server not found".to_string(),
                }),
            ));
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to get server: {}", e),
                }),
            ));
        }
    };

    // Decrypt credentials
    let url = match mcp_repository.get_decrypted_url(&server) {
        Ok(u) => u,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to decrypt URL: {}", e),
                }),
            ));
        }
    };

    let auth_token = match mcp_repository.get_decrypted_auth_token(&server) {
        Ok(t) => t,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to decrypt auth token: {}", e),
                }),
            ));
        }
    };

    // Connect and list tools
    let mcp_client = McpClientService::new();
    match mcp_client.list_tools(&url, auth_token.as_deref()).await {
        Ok(tools) => {
            let tool_infos: Vec<McpToolInfo> = tools
                .into_iter()
                .map(|t| McpToolInfo {
                    name: t.name,
                    description: t.description,
                    input_schema: Some(t.input_schema),
                })
                .collect();

            Ok(Json(McpTestConnectionResponse {
                success: true,
                tools_count: Some(tool_infos.len()),
                tools: Some(tool_infos),
                error: None,
            }))
        }
        Err(e) => Ok(Json(McpTestConnectionResponse {
            success: false,
            tools_count: None,
            tools: None,
            error: Some(e),
        })),
    }
}

/// POST /api/mcp/servers/:id/test - Test connection to server
pub async fn test_server_connection(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(server_id): Path<i32>,
) -> Result<Json<McpTestConnectionResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Same as list_server_tools but uses test_connection
    let mcp_repository = McpRepository::new(state.pg_pool.clone());

    let server = match mcp_repository.get_server_by_id(server_id, auth_user.user_id) {
        Ok(Some(s)) => s,
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Server not found".to_string(),
                }),
            ));
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to get server: {}", e),
                }),
            ));
        }
    };

    let url = match mcp_repository.get_decrypted_url(&server) {
        Ok(u) => u,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to decrypt URL: {}", e),
                }),
            ));
        }
    };

    let auth_token = match mcp_repository.get_decrypted_auth_token(&server) {
        Ok(t) => t,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to decrypt auth token: {}", e),
                }),
            ));
        }
    };

    let mcp_client = McpClientService::new();
    match mcp_client
        .test_connection(&url, auth_token.as_deref())
        .await
    {
        Ok(tools) => {
            let tool_infos: Vec<McpToolInfo> = tools
                .into_iter()
                .map(|t| McpToolInfo {
                    name: t.name,
                    description: t.description,
                    input_schema: Some(t.input_schema),
                })
                .collect();

            Ok(Json(McpTestConnectionResponse {
                success: true,
                tools_count: Some(tool_infos.len()),
                tools: Some(tool_infos),
                error: None,
            }))
        }
        Err(e) => Ok(Json(McpTestConnectionResponse {
            success: false,
            tools_count: None,
            tools: None,
            error: Some(e),
        })),
    }
}

/// DELETE /api/mcp/servers/:id - Remove server
pub async fn delete_mcp_server(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(server_id): Path<i32>,
) -> Result<Json<SuccessResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!(
        "Deleting MCP server {} for user {}",
        server_id, auth_user.user_id
    );

    let mcp_repository = McpRepository::new(state.pg_pool.clone());

    match mcp_repository.delete_server(server_id, auth_user.user_id) {
        Ok(()) => Ok(Json(SuccessResponse {
            success: true,
            message: "Server deleted".to_string(),
        })),
        Err(e) => {
            error!("Failed to delete MCP server: {}", e);
            Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Failed to delete server: {}", e),
                }),
            ))
        }
    }
}

/// PATCH /api/mcp/servers/:id/toggle - Enable/disable server
pub async fn toggle_mcp_server(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(server_id): Path<i32>,
) -> Result<Json<ToggleResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!(
        "Toggling MCP server {} for user {}",
        server_id, auth_user.user_id
    );

    let mcp_repository = McpRepository::new(state.pg_pool.clone());

    match mcp_repository.toggle_server(server_id, auth_user.user_id) {
        Ok(is_enabled) => Ok(Json(ToggleResponse { is_enabled })),
        Err(e) => {
            error!("Failed to toggle MCP server: {}", e);
            Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Failed to toggle server: {}", e),
                }),
            ))
        }
    }
}

/// POST /api/mcp/test - Test a server URL before adding (no server_id needed)
pub async fn test_url_connection(
    State(_state): State<Arc<AppState>>,
    _auth_user: AuthUser,
    Json(request): Json<TestUrlRequest>,
) -> Result<Json<McpTestConnectionResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Validate URL format
    if !request.url.starts_with("http://") && !request.url.starts_with("https://") {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "URL must start with http:// or https://".to_string(),
            }),
        ));
    }

    validate_public_mcp_url(&request.url)
        .await
        .map_err(|error| (StatusCode::BAD_REQUEST, Json(ErrorResponse { error })))?;

    let mcp_client = McpClientService::new();
    match mcp_client
        .test_connection(&request.url, request.auth_token.as_deref())
        .await
    {
        Ok(tools) => {
            let tool_infos: Vec<McpToolInfo> = tools
                .into_iter()
                .map(|t| McpToolInfo {
                    name: t.name,
                    description: t.description,
                    input_schema: Some(t.input_schema),
                })
                .collect();

            Ok(Json(McpTestConnectionResponse {
                success: true,
                tools_count: Some(tool_infos.len()),
                tools: Some(tool_infos),
                error: None,
            }))
        }
        Err(e) => Ok(Json(McpTestConnectionResponse {
            success: false,
            tools_count: None,
            tools: None,
            error: Some(e),
        })),
    }
}

#[derive(Debug, Deserialize)]
pub struct TestUrlRequest {
    pub url: String,
    pub auth_token: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Serialize)]
pub struct SuccessResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ToggleResponse {
    pub is_enabled: bool,
}
