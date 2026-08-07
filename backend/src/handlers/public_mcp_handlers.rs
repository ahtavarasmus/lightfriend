//! Per-user, read-only MCP server for external clients.
//!
//! A dedicated bearer token authenticates one Lightfriend user. The token is
//! hashed at rest and intentionally grants access only to ontology query tools;
//! outbound messaging, vehicle controls, arbitrary remote MCP servers, and
//! billable search tools are not exposed through this credential.

use crate::handlers::auth_middleware::AuthUser;
use crate::ontology::registry::OntologyUserData;
use crate::pg_schema::{mcp_access_tokens, user_secrets};
use crate::{AppState, UserCoreOps};
use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use diesel::prelude::*;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const TOKEN_PREFIX: &str = "lfmcp_";
const TOKEN_BYTES: usize = 32;
const MAX_TOKENS_PER_USER: i64 = 10;
const MAX_LABEL_LEN: usize = 64;
const MAX_TOOL_OUTPUT_CHARS: usize = 32_000;
const TOOL_TIMEOUT: Duration = Duration::from_secs(15);
const CURRENT_PROTOCOL_VERSION: &str = "2026-07-28";
const LEGACY_PROTOCOL_VERSIONS: [&str; 3] = ["2025-11-25", "2025-06-18", "2025-03-26"];
const DEFAULT_LEGACY_PROTOCOL_VERSION: &str = "2025-03-26";
const MCP_PROTOCOL_VERSION_HEADER: &str = "mcp-protocol-version";
const MCP_METHOD_HEADER: &str = "mcp-method";
const MCP_NAME_HEADER: &str = "mcp-name";

#[derive(Debug, Queryable, Selectable)]
#[diesel(table_name = mcp_access_tokens)]
#[allow(dead_code)] // Full-row mapping keeps Diesel inserts/lookups simple; secrets are never serialized.
struct McpAccessToken {
    id: i32,
    user_id: i32,
    token_hash: String,
    token_prefix: String,
    label: String,
    created_at: i32,
    last_used_at: Option<i32>,
    revoked_at: Option<i32>,
}

#[derive(Insertable)]
#[diesel(table_name = mcp_access_tokens)]
struct NewMcpAccessToken {
    user_id: i32,
    token_hash: String,
    token_prefix: String,
    label: String,
    created_at: i32,
}

#[derive(Deserialize)]
pub struct CreateMcpTokenRequest {
    pub label: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct McpTokenSummary {
    pub id: i32,
    pub token_prefix: String,
    pub label: String,
    pub created_at: i32,
    pub last_used_at: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct CreateMcpTokenResponse {
    #[serde(flatten)]
    pub summary: McpTokenSummary,
    /// Returned exactly once. Only its SHA-256 digest is persisted.
    pub token: String,
    pub endpoint: String,
}

#[derive(Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

pub async fn create_token(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(req): Json<CreateMcpTokenRequest>,
) -> Result<Response, (StatusCode, Json<Value>)> {
    require_active_subscription(&state, auth_user.user_id)?;
    let label = sanitize_label(&req.label).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "label must be 1..=64 printable characters"})),
        )
    })?;

    let mut conn = state.pg_pool.get().map_err(pool_error)?;
    // Some accounts have not connected a secret-bearing integration yet and
    // therefore have no `user_secrets` row. Create an empty owner row so the
    // token can still inherit the existing "Delete my data" cascade boundary.
    diesel::insert_into(user_secrets::table)
        .values(user_secrets::user_id.eq(auth_user.user_id))
        .on_conflict(user_secrets::user_id)
        .do_nothing()
        .execute(&mut conn)
        .map_err(db_error)?;
    let active_count = mcp_access_tokens::table
        .filter(mcp_access_tokens::user_id.eq(auth_user.user_id))
        .filter(mcp_access_tokens::revoked_at.is_null())
        .count()
        .get_result::<i64>(&mut conn)
        .map_err(db_error)?;
    if active_count >= MAX_TOKENS_PER_USER {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({"error": "revoke an existing MCP token before creating another"})),
        ));
    }

    let raw_token = generate_token();
    let now = now_unix();
    let row = diesel::insert_into(mcp_access_tokens::table)
        .values(NewMcpAccessToken {
            user_id: auth_user.user_id,
            token_hash: hash_token(&raw_token),
            token_prefix: raw_token.chars().take(14).collect(),
            label,
            created_at: now,
        })
        .get_result::<McpAccessToken>(&mut conn)
        .map_err(db_error)?;

    Ok(no_store(
        Json(CreateMcpTokenResponse {
            summary: token_summary(&row),
            token: raw_token,
            endpoint: "/api/mcp".to_string(),
        })
        .into_response(),
    ))
}

pub async fn list_tokens(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<Json<Vec<McpTokenSummary>>, (StatusCode, Json<Value>)> {
    let mut conn = state.pg_pool.get().map_err(pool_error)?;
    let rows = mcp_access_tokens::table
        .filter(mcp_access_tokens::user_id.eq(auth_user.user_id))
        .filter(mcp_access_tokens::revoked_at.is_null())
        .order(mcp_access_tokens::created_at.desc())
        .select(McpAccessToken::as_select())
        .load::<McpAccessToken>(&mut conn)
        .map_err(db_error)?;
    Ok(Json(rows.iter().map(token_summary).collect()))
}

pub async fn revoke_token(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Path(token_id): Path<i32>,
) -> Result<StatusCode, (StatusCode, Json<Value>)> {
    let mut conn = state.pg_pool.get().map_err(pool_error)?;
    let owned = mcp_access_tokens::table
        .filter(mcp_access_tokens::id.eq(token_id))
        .filter(mcp_access_tokens::user_id.eq(auth_user.user_id))
        .count()
        .get_result::<i64>(&mut conn)
        .map_err(db_error)?;
    if owned == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "token not found"})),
        ));
    }
    diesel::update(
        mcp_access_tokens::table
            .filter(mcp_access_tokens::id.eq(token_id))
            .filter(mcp_access_tokens::user_id.eq(auth_user.user_id))
            .filter(mcp_access_tokens::revoked_at.is_null()),
    )
    .set(mcp_access_tokens::revoked_at.eq(now_unix()))
    .execute(&mut conn)
    .map_err(db_error)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Stateless MCP Streamable HTTP endpoint. Authentication and JSON parsing are
/// performed in this order so unauthenticated callers cannot use malformed
/// request differences to probe the implementation.
pub async fn public_mcp(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !origin_is_allowed(&headers) {
        return http_error(StatusCode::FORBIDDEN, "origin not allowed");
    }

    let Some(raw_token) = extract_bearer(&headers) else {
        return unauthorized();
    };
    if !valid_token_shape(raw_token) {
        return unauthorized();
    }

    let token = match authenticate_token(&state, raw_token) {
        Ok(Some(token)) => token,
        Ok(None) => return unauthorized(),
        Err(error) => {
            tracing::error!(error = %error, "public MCP token lookup failed");
            return http_error(StatusCode::INTERNAL_SERVER_ERROR, "internal error");
        }
    };

    let user = match state.user_core.find_by_id(token.user_id) {
        Ok(Some(user)) if user.sub_tier.as_deref() == Some("tier 2") => user,
        Ok(Some(_)) => {
            return http_error(StatusCode::FORBIDDEN, "active subscription required");
        }
        Ok(None) => return unauthorized(),
        Err(error) => {
            tracing::error!(user_id = token.user_id, error = %error, "public MCP user lookup failed");
            return http_error(StatusCode::INTERNAL_SERVER_ERROR, "internal error");
        }
    };

    let request: JsonRpcRequest = match serde_json::from_slice(&body) {
        Ok(request) => request,
        Err(_) => return rpc_error_response(Value::Null, -32700, "Parse error"),
    };
    if request.jsonrpc != "2.0" || request.method.is_empty() {
        return rpc_error_response(request.id.unwrap_or(Value::Null), -32600, "Invalid Request");
    }

    let protocol_version = match request_protocol_version(&headers) {
        Ok(version) => version,
        Err(response) => return *response,
    };
    if let Err(response) = validate_routing_headers(&headers, &request, protocol_version) {
        return *response;
    }

    touch_token(&state, token.id);
    let id = request.id.clone().unwrap_or(Value::Null);
    let is_notification = request.id.is_none();

    let response = match request.method.as_str() {
        "initialize" if protocol_version != CURRENT_PROTOCOL_VERSION => rpc_result_response(
            id,
            json!({
                "protocolVersion": protocol_version,
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "Lightfriend", "version": env!("CARGO_PKG_VERSION")},
                "instructions": "Read-only access to the authenticated user's Lightfriend messages, people, and tracked events."
            }),
        ),
        "server/discover" if protocol_version == CURRENT_PROTOCOL_VERSION => rpc_result_response(
            id,
            json!({
                "resultType": "complete",
                "supportedVersions": [CURRENT_PROTOCOL_VERSION, "2025-11-25", "2025-06-18", "2025-03-26"],
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "Lightfriend", "version": env!("CARGO_PKG_VERSION")},
                "instructions": "Read-only access to the authenticated user's Lightfriend messages, people, and tracked events.",
                "ttlMs": 0,
                "cacheScope": "private"
            }),
        ),
        "notifications/initialized" | "notifications/cancelled"
            if is_notification && protocol_version != CURRENT_PROTOCOL_VERSION =>
        {
            StatusCode::ACCEPTED.into_response()
        }
        "ping" => rpc_result_response(id, json!({})),
        "tools/list" => {
            let mut result = json!({"tools": public_tool_definitions(&state, user.id)});
            if protocol_version == CURRENT_PROTOCOL_VERSION {
                result["resultType"] = json!("complete");
                result["ttlMs"] = json!(0);
                result["cacheScope"] = json!("private");
            }
            rpc_result_response(id, result)
        }
        "tools/call" => {
            let Some(params) = request.params else {
                return rpc_error_response(id, -32602, "Invalid params");
            };
            match call_public_tool(&state, &user, params).await {
                Ok(mut result) => {
                    if protocol_version == CURRENT_PROTOCOL_VERSION {
                        result["resultType"] = json!("complete");
                    }
                    rpc_result_response(id, result)
                }
                Err(PublicToolError::InvalidParams) => {
                    rpc_error_response(id, -32602, "Invalid params")
                }
                Err(PublicToolError::UnknownTool) => {
                    rpc_error_response(id, -32602, "Unknown or unavailable tool")
                }
            }
        }
        _ if is_notification => StatusCode::ACCEPTED.into_response(),
        _ if protocol_version == CURRENT_PROTOCOL_VERSION => {
            rpc_error_response_with_status(StatusCode::NOT_FOUND, id, -32601, "Method not found")
        }
        _ => rpc_error_response(id, -32601, "Method not found"),
    };
    no_store(response)
}

#[derive(Debug)]
enum PublicToolError {
    InvalidParams,
    UnknownTool,
}

async fn call_public_tool(
    state: &Arc<AppState>,
    user: &crate::models::user_models::User,
    params: Value,
) -> Result<Value, PublicToolError> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .ok_or(PublicToolError::InvalidParams)?;
    let allowed = public_tool_definitions(state, user.id)
        .iter()
        .any(|definition| definition.get("name").and_then(Value::as_str) == Some(name));
    if !allowed {
        return Err(PublicToolError::UnknownTool);
    }
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| json!({}));
    if !arguments.is_object() {
        return Err(PublicToolError::InvalidParams);
    }
    let arguments =
        serde_json::to_string(&arguments).map_err(|_| PublicToolError::InvalidParams)?;

    let outcome = tokio::time::timeout(
        TOOL_TIMEOUT,
        crate::tools::ontology::handle_query(name, &arguments, state, user.id),
    )
    .await;
    let (text, is_error) = match outcome {
        Ok(Ok(text)) => (truncate_chars(text, MAX_TOOL_OUTPUT_CHARS), false),
        Ok(Err(error)) => {
            tracing::warn!(user_id = user.id, tool = name, error = %error, "public MCP tool failed");
            ("Tool execution failed".to_string(), true)
        }
        Err(_) => {
            tracing::warn!(user_id = user.id, tool = name, "public MCP tool timed out");
            ("Tool execution timed out".to_string(), true)
        }
    };
    Ok(json!({
        "content": [{"type": "text", "text": text}],
        "isError": is_error
    }))
}

/// Convert the existing user-aware ontology definitions into MCP tool shape.
pub fn public_tool_definitions(state: &AppState, user_id: i32) -> Vec<Value> {
    let mut dynamic_enums = HashMap::new();
    if let Ok(persons) = state.ontology_repository.get_persons(user_id) {
        dynamic_enums.insert(
            "person_names".to_string(),
            persons.into_iter().map(|person| person.name).collect(),
        );
    }
    if let Ok(accounts) = state.user_repository.get_all_imap_credentials(user_id) {
        dynamic_enums.insert(
            "inbox_selectors".to_string(),
            accounts
                .into_iter()
                .flat_map(|account| account.nickname.into_iter().chain([account.email]))
                .collect(),
        );
    }
    let mut definitions: Vec<Value> = state
        .ontology_registry
        .build_query_tools(&OntologyUserData { dynamic_enums })
        .into_iter()
        .filter_map(|tool| {
            let value = serde_json::to_value(tool).ok()?;
            let function = value.get("function")?;
            let name = function.get("name")?.as_str()?;
            if !matches!(name, "query_person" | "query_message" | "query_event") {
                return None;
            }
            Some(json!({
                "name": name,
                "description": function.get("description").cloned().unwrap_or(Value::Null),
                "inputSchema": function.get("parameters").cloned().unwrap_or_else(|| json!({"type": "object"}))
            }))
        })
        .collect();
    definitions.sort_by(|left, right| {
        left.get("name")
            .and_then(Value::as_str)
            .cmp(&right.get("name").and_then(Value::as_str))
    });
    definitions
}

fn request_protocol_version(headers: &HeaderMap) -> Result<&'static str, Box<Response>> {
    let Some(raw) = headers.get(MCP_PROTOCOL_VERSION_HEADER) else {
        return Ok(DEFAULT_LEGACY_PROTOCOL_VERSION);
    };
    let Ok(version) = raw.to_str() else {
        return Err(Box::new(http_error(
            StatusCode::BAD_REQUEST,
            "unsupported MCP protocol version",
        )));
    };
    if version == CURRENT_PROTOCOL_VERSION {
        return Ok(CURRENT_PROTOCOL_VERSION);
    }
    LEGACY_PROTOCOL_VERSIONS
        .iter()
        .copied()
        .find(|supported| *supported == version)
        .ok_or_else(|| {
            Box::new(http_error(
                StatusCode::BAD_REQUEST,
                "unsupported MCP protocol version",
            ))
        })
}

fn validate_routing_headers(
    headers: &HeaderMap,
    request: &JsonRpcRequest,
    protocol_version: &str,
) -> Result<(), Box<Response>> {
    if protocol_version != CURRENT_PROTOCOL_VERSION {
        return Ok(());
    }
    let method = headers
        .get(MCP_METHOD_HEADER)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            Box::new(http_error(
                StatusCode::BAD_REQUEST,
                "missing MCP routing headers",
            ))
        })?;
    if method != request.method {
        return Err(Box::new(http_error(
            StatusCode::BAD_REQUEST,
            "MCP routing headers do not match request",
        )));
    }
    if request.method == "tools/call" {
        let body_name = request
            .params
            .as_ref()
            .and_then(|params| params.get("name"))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                Box::new(http_error(StatusCode::BAD_REQUEST, "missing MCP tool name"))
            })?;
        let header_name = headers
            .get(MCP_NAME_HEADER)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| {
                Box::new(http_error(
                    StatusCode::BAD_REQUEST,
                    "missing MCP routing headers",
                ))
            })?;
        if header_name != body_name {
            return Err(Box::new(http_error(
                StatusCode::BAD_REQUEST,
                "MCP routing headers do not match request",
            )));
        }
    }
    Ok(())
}

fn require_active_subscription(
    state: &Arc<AppState>,
    user_id: i32,
) -> Result<(), (StatusCode, Json<Value>)> {
    match state.user_core.find_by_id(user_id).map_err(|error| {
        tracing::error!(user_id, error = %error, "MCP token user lookup failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "internal error"})),
        )
    })? {
        Some(user) if user.sub_tier.as_deref() == Some("tier 2") => Ok(()),
        Some(_) => Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "active subscription required"})),
        )),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "user not found"})),
        )),
    }
}

fn authenticate_token(state: &Arc<AppState>, raw: &str) -> anyhow::Result<Option<McpAccessToken>> {
    let mut conn = state.pg_pool.get()?;
    let token = mcp_access_tokens::table
        .filter(mcp_access_tokens::token_hash.eq(hash_token(raw)))
        .filter(mcp_access_tokens::revoked_at.is_null())
        .select(McpAccessToken::as_select())
        .first::<McpAccessToken>(&mut conn)
        .optional()?;
    Ok(token)
}

fn touch_token(state: &Arc<AppState>, token_id: i32) {
    let Ok(mut conn) = state.pg_pool.get() else {
        return;
    };
    if let Err(error) = diesel::update(mcp_access_tokens::table.find(token_id))
        .set(mcp_access_tokens::last_used_at.eq(now_unix()))
        .execute(&mut conn)
    {
        tracing::warn!(token_id, error = %error, "failed to update MCP token last-used time");
    }
}

fn token_summary(row: &McpAccessToken) -> McpTokenSummary {
    McpTokenSummary {
        id: row.id,
        token_prefix: row.token_prefix.clone(),
        label: row.label.clone(),
        created_at: row.created_at,
        last_used_at: row.last_used_at,
    }
}

fn sanitize_label(input: &str) -> Option<String> {
    let cleaned: String = input
        .chars()
        .filter(|character| !character.is_control())
        .collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() || cleaned.chars().count() > MAX_LABEL_LEN {
        None
    } else {
        Some(cleaned.to_string())
    }
}

fn generate_token() -> String {
    let mut bytes = [0u8; TOKEN_BYTES];
    rand::thread_rng().fill_bytes(&mut bytes);
    format!("{}{}", TOKEN_PREFIX, hex::encode(bytes))
}

fn valid_token_shape(raw: &str) -> bool {
    raw.len() == TOKEN_PREFIX.len() + TOKEN_BYTES * 2
        && raw.starts_with(TOKEN_PREFIX)
        && raw[TOKEN_PREFIX.len()..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
}

fn hash_token(raw: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    hex::encode(hasher.finalize())
}

fn extract_bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .or_else(|| {
            headers
                .get(header::AUTHORIZATION)?
                .to_str()
                .ok()?
                .strip_prefix("bearer ")
        })
        .map(str::trim)
        .filter(|token| !token.is_empty())
}

fn origin_is_allowed(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(header::ORIGIN) else {
        return true;
    };
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    let configured = std::env::var("APP_ORIGIN")
        .unwrap_or_else(|_| "https://lightfriend.ai".to_string())
        .trim_end_matches('/')
        .to_string();
    origin == configured || origin == "https://lightfriend.ai"
}

fn truncate_chars(value: String, max: usize) -> String {
    match value.char_indices().nth(max) {
        Some((index, _)) => format!("{}\n[output truncated]", &value[..index]),
        None => value,
    }
}

fn now_unix() -> i32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i32
}

fn rpc_result_response(id: Value, result: Value) -> Response {
    no_store(Json(json!({"jsonrpc": "2.0", "id": id, "result": result})).into_response())
}

fn rpc_error_response(id: Value, code: i32, message: &'static str) -> Response {
    no_store(
        Json(json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {"code": code, "message": message}
        }))
        .into_response(),
    )
}

fn rpc_error_response_with_status(
    status: StatusCode,
    id: Value,
    code: i32,
    message: &'static str,
) -> Response {
    let mut response = rpc_error_response(id, code, message);
    *response.status_mut() = status;
    response
}

fn unauthorized() -> Response {
    http_error(StatusCode::UNAUTHORIZED, "invalid or revoked token")
}

fn http_error(status: StatusCode, message: &'static str) -> Response {
    no_store((status, Json(json!({"error": message}))).into_response())
}

fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn pool_error(error: diesel::r2d2::PoolError) -> (StatusCode, Json<Value>) {
    tracing::error!(error = %error, "MCP token database pool error");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": "internal error"})),
    )
}

fn db_error(error: diesel::result::Error) -> (StatusCode, Json<Value>) {
    tracing::error!(error = %error, "MCP token database error");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": "internal error"})),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_shape_is_exact_and_prefixed() {
        let token = generate_token();
        assert!(valid_token_shape(&token));
        assert!(!valid_token_shape("lfmcp_short"));
        assert!(!valid_token_shape(&token.replace('a', "z")));
    }

    #[test]
    fn label_and_output_sanitizers_are_bounded() {
        assert_eq!(
            sanitize_label("  desktop\nclient  "),
            Some("desktopclient".into())
        );
        assert!(sanitize_label("\n\t").is_none());
        assert!(sanitize_label(&"x".repeat(65)).is_none());
        assert_eq!(truncate_chars("åbc".into(), 2), "åb\n[output truncated]");
    }
}
