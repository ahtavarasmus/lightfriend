use crate::models::mcp_models::{McpServer, McpServerResponse, NewMcpServer};
use crate::schema::mcp_servers;
use crate::utils::encryption::{decrypt, encrypt};
use crate::DbPool;
use diesel::prelude::*;
use std::sync::Arc;

pub struct McpRepository {
    pool: DbPool,
}

impl McpRepository {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Create a new MCP server for a user
    pub fn create_server(
        &self,
        user_id: i32,
        name: &str,
        url: &str,
        auth_token: Option<&str>,
    ) -> Result<McpServer, String> {
        let mut conn = self.pool.get().map_err(|e| e.to_string())?;

        // Encrypt the URL and optional auth token
        let url_encrypted = encrypt(url).map_err(|e| format!("Failed to encrypt URL: {}", e))?;
        let auth_token_encrypted = auth_token
            .map(encrypt)
            .transpose()
            .map_err(|e| format!("Failed to encrypt auth token: {}", e))?;

        let now = chrono::Utc::now().timestamp() as i32;

        let new_server = NewMcpServer {
            user_id,
            name: name.to_string(),
            url_encrypted,
            auth_token_encrypted,
            is_enabled: 1,
            created_at: now,
        };

        diesel::insert_into(mcp_servers::table)
            .values(&new_server)
            .execute(&mut conn)
            .map_err(|e| format!("Failed to insert MCP server: {}", e))?;

        // Retrieve the created server
        mcp_servers::table
            .filter(mcp_servers::user_id.eq(user_id))
            .filter(mcp_servers::name.eq(name))
            .first::<McpServer>(&mut conn)
            .map_err(|e| format!("Failed to retrieve created server: {}", e))
    }

    /// Get all MCP servers for a user
    pub fn get_servers_for_user(&self, user_id: i32) -> Result<Vec<McpServer>, String> {
        let mut conn = self.pool.get().map_err(|e| e.to_string())?;

        mcp_servers::table
            .filter(mcp_servers::user_id.eq(user_id))
            .order(mcp_servers::created_at.desc())
            .load::<McpServer>(&mut conn)
            .map_err(|e| format!("Failed to get MCP servers: {}", e))
    }

    /// Get all enabled MCP servers for a user (for AI tool integration)
    pub fn get_enabled_servers_for_user(&self, user_id: i32) -> Result<Vec<McpServer>, String> {
        let mut conn = self.pool.get().map_err(|e| e.to_string())?;

        mcp_servers::table
            .filter(mcp_servers::user_id.eq(user_id))
            .filter(mcp_servers::is_enabled.eq(1))
            .load::<McpServer>(&mut conn)
            .map_err(|e| format!("Failed to get enabled MCP servers: {}", e))
    }

    /// Get a specific MCP server by ID
    pub fn get_server_by_id(
        &self,
        server_id: i32,
        user_id: i32,
    ) -> Result<Option<McpServer>, String> {
        let mut conn = self.pool.get().map_err(|e| e.to_string())?;

        mcp_servers::table
            .filter(mcp_servers::id.eq(server_id))
            .filter(mcp_servers::user_id.eq(user_id))
            .first::<McpServer>(&mut conn)
            .optional()
            .map_err(|e| format!("Failed to get MCP server: {}", e))
    }

    /// Get a specific MCP server by name
    pub fn get_server_by_name(
        &self,
        user_id: i32,
        name: &str,
    ) -> Result<Option<McpServer>, String> {
        let mut conn = self.pool.get().map_err(|e| e.to_string())?;

        mcp_servers::table
            .filter(mcp_servers::user_id.eq(user_id))
            .filter(mcp_servers::name.eq(name))
            .first::<McpServer>(&mut conn)
            .optional()
            .map_err(|e| format!("Failed to get MCP server by name: {}", e))
    }

    /// Toggle server enabled/disabled status
    pub fn toggle_server(&self, server_id: i32, user_id: i32) -> Result<bool, String> {
        let mut conn = self.pool.get().map_err(|e| e.to_string())?;

        // Get current status
        let server = mcp_servers::table
            .filter(mcp_servers::id.eq(server_id))
            .filter(mcp_servers::user_id.eq(user_id))
            .first::<McpServer>(&mut conn)
            .map_err(|e| format!("Server not found: {}", e))?;

        let new_status = if server.is_enabled == 1 { 0 } else { 1 };

        diesel::update(
            mcp_servers::table
                .filter(mcp_servers::id.eq(server_id))
                .filter(mcp_servers::user_id.eq(user_id)),
        )
        .set(mcp_servers::is_enabled.eq(new_status))
        .execute(&mut conn)
        .map_err(|e| format!("Failed to toggle server: {}", e))?;

        Ok(new_status == 1)
    }

    /// Delete an MCP server
    pub fn delete_server(&self, server_id: i32, user_id: i32) -> Result<(), String> {
        let mut conn = self.pool.get().map_err(|e| e.to_string())?;

        let deleted = diesel::delete(
            mcp_servers::table
                .filter(mcp_servers::id.eq(server_id))
                .filter(mcp_servers::user_id.eq(user_id)),
        )
        .execute(&mut conn)
        .map_err(|e| format!("Failed to delete server: {}", e))?;

        if deleted == 0 {
            return Err("Server not found".to_string());
        }

        Ok(())
    }

    /// Convert McpServer to McpServerResponse with decrypted URL
    pub fn to_response(&self, server: &McpServer) -> Result<McpServerResponse, String> {
        let url =
            decrypt(&server.url_encrypted).map_err(|e| format!("Failed to decrypt URL: {}", e))?;

        Ok(McpServerResponse {
            id: server.id.unwrap_or(0),
            name: server.name.clone(),
            url,
            has_auth_token: server.auth_token_encrypted.is_some(),
            is_enabled: server.is_enabled == 1,
            created_at: server.created_at,
        })
    }

    /// Get decrypted URL for a server
    pub fn get_decrypted_url(&self, server: &McpServer) -> Result<String, String> {
        decrypt(&server.url_encrypted).map_err(|e| format!("Failed to decrypt URL: {}", e))
    }

    /// Get decrypted auth token for a server (if present)
    pub fn get_decrypted_auth_token(&self, server: &McpServer) -> Result<Option<String>, String> {
        server
            .auth_token_encrypted
            .as_ref()
            .map(|token| decrypt(token))
            .transpose()
            .map_err(|e| format!("Failed to decrypt auth token: {}", e))
    }
}

/// Arc wrapper for thread-safe sharing
pub type McpRepositoryArc = Arc<McpRepository>;
