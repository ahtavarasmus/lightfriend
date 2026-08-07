use axum::body::{to_bytes, Bytes};
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::Json;
use backend::handlers::auth_middleware::AuthUser;
use backend::handlers::public_mcp_handlers::{
    create_token, list_tokens, public_mcp, revoke_token, CreateMcpTokenRequest,
    CreateMcpTokenResponse,
};
use backend::test_utils::{create_test_state, create_test_user, TestUserParams};
use serde_json::{json, Value};
use serial_test::serial;
use std::collections::HashSet;

fn owner_auth(user_id: i32) -> AuthUser {
    AuthUser {
        user_id,
        is_admin: false,
    }
}

async fn response_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), 128 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn mint_mcp_token(
    state: std::sync::Arc<backend::AppState>,
    user_id: i32,
    label: &str,
) -> CreateMcpTokenResponse {
    let response = create_token(
        State(state),
        owner_auth(user_id),
        Json(CreateMcpTokenRequest {
            label: label.into(),
        }),
    )
    .await
    .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store"
    );
    serde_json::from_value(response_json(response).await).unwrap()
}

async fn rpc(
    state: std::sync::Arc<backend::AppState>,
    token: &str,
    payload: Value,
) -> axum::response::Response {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
    );
    public_mcp(
        State(state),
        headers,
        Bytes::from(serde_json::to_vec(&payload).unwrap()),
    )
    .await
}

async fn current_rpc(
    state: std::sync::Arc<backend::AppState>,
    token: &str,
    payload: Value,
) -> axum::response::Response {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
    );
    headers.insert(
        "mcp-protocol-version",
        HeaderValue::from_static("2026-07-28"),
    );
    let method = payload["method"].as_str().unwrap();
    headers.insert("mcp-method", HeaderValue::from_str(method).unwrap());
    if method == "tools/call" {
        headers.insert(
            "mcp-name",
            HeaderValue::from_str(payload["params"]["name"].as_str().unwrap()).unwrap(),
        );
    }
    public_mcp(
        State(state),
        headers,
        Bytes::from(serde_json::to_vec(&payload).unwrap()),
    )
    .await
}

#[tokio::test]
#[serial]
async fn external_client_discovers_only_read_only_user_tools() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let created = mint_mcp_token(state.clone(), user.id, "desktop client").await;

    let discovered = current_rpc(
        state.clone(),
        &created.token,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "server/discover",
            "params": {"_meta": {"io.modelcontextprotocol/clientInfo": {"name": "test", "version": "1"}}}
        }),
    )
    .await;
    assert_eq!(discovered.status(), StatusCode::OK);
    let discovered = response_json(discovered).await;
    assert_eq!(discovered["result"]["resultType"], "complete");
    assert!(discovered["result"]["supportedVersions"]
        .as_array()
        .unwrap()
        .contains(&json!("2026-07-28")));
    assert!(discovered["result"]["capabilities"]["tools"].is_object());

    let listed = current_rpc(
        state,
        &created.token,
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
    )
    .await;
    assert_eq!(listed.status(), StatusCode::OK);
    assert_eq!(
        listed.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store"
    );
    let listed = response_json(listed).await;
    assert_eq!(listed["result"]["resultType"], "complete");
    assert_eq!(listed["result"]["cacheScope"], "private");
    let names: HashSet<&str> = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    assert_eq!(
        names,
        HashSet::from(["query_person", "query_message", "query_event"])
    );
    assert!(!names.contains("send_chat_message"));
    assert!(!names.contains("tesla_control"));
}

#[tokio::test]
#[serial]
async fn tool_calls_are_tenant_scoped_and_outgoing_tools_are_rejected() {
    let state = create_test_state();
    let owner = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let other = create_test_user(
        &state,
        &TestUserParams {
            email: "other-mcp@example.com".into(),
            phone_number: "+14155550991".into(),
            credits: 10.0,
            credits_left: 5.0,
            sub_tier: Some("tier 2".into()),
        },
    );
    state
        .ontology_repository
        .create_person(other.id, "Other Tenant Secret Name")
        .unwrap();
    let created = mint_mcp_token(state.clone(), owner.id, "scope test").await;

    let query = current_rpc(
        state.clone(),
        &created.token,
        json!({
            "jsonrpc": "2.0",
            "id": "query",
            "method": "tools/call",
            "params": {"name": "query_person", "arguments": {"name": "all"}}
        }),
    )
    .await;
    let query = response_json(query).await;
    let text = query["result"]["content"][0]["text"].as_str().unwrap();
    assert!(!text.contains("Other Tenant Secret Name"));

    let outgoing = current_rpc(
        state,
        &created.token,
        json!({
            "jsonrpc": "2.0",
            "id": "send",
            "method": "tools/call",
            "params": {"name": "send_chat_message", "arguments": {"message": "no"}}
        }),
    )
    .await;
    let outgoing = response_json(outgoing).await;
    assert_eq!(outgoing["error"]["code"], -32602);
    assert_eq!(outgoing["error"]["message"], "Unknown or unavailable tool");
}

#[tokio::test]
#[serial]
async fn legacy_initialize_works_and_current_routing_headers_must_match_body() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let created = mint_mcp_token(state.clone(), user.id, "compatibility").await;

    let initialized = rpc(
        state.clone(),
        &created.token,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "legacy", "version": "1"}}
        }),
    )
    .await;
    assert_eq!(initialized.status(), StatusCode::OK);
    assert_eq!(
        response_json(initialized).await["result"]["protocolVersion"],
        "2025-03-26"
    );

    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", created.token)).unwrap(),
    );
    headers.insert(
        "mcp-protocol-version",
        HeaderValue::from_static("2026-07-28"),
    );
    headers.insert("mcp-method", HeaderValue::from_static("tools/list"));
    let mismatched = public_mcp(
        State(state),
        headers,
        Bytes::from_static(br#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"query_person","arguments":{"name":"all"}}}"#),
    )
    .await;
    assert_eq!(mismatched.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[serial]
async fn revoked_token_and_cross_origin_request_fail_closed() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let created = mint_mcp_token(state.clone(), user.id, "revocation test").await;

    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {}", created.token)).unwrap(),
    );
    headers.insert(
        header::ORIGIN,
        HeaderValue::from_static("https://evil.example"),
    );
    let blocked = public_mcp(
        State(state.clone()),
        headers,
        Bytes::from_static(br#"{"jsonrpc":"2.0","id":1,"method":"tools/list"}"#),
    )
    .await;
    assert_eq!(blocked.status(), StatusCode::FORBIDDEN);

    assert_eq!(
        revoke_token(
            State(state.clone()),
            owner_auth(user.id),
            Path(created.summary.id)
        )
        .await
        .unwrap(),
        StatusCode::NO_CONTENT
    );
    let revoked = rpc(
        state,
        &created.token,
        json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}),
    )
    .await;
    assert_eq!(revoked.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[serial]
async fn token_management_is_scoped_to_its_owner() {
    let state = create_test_state();
    let owner = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let other = create_test_user(
        &state,
        &TestUserParams {
            email: "other-mcp-owner@example.com".into(),
            phone_number: "+14155550993".into(),
            credits: 10.0,
            credits_left: 5.0,
            sub_tier: Some("tier 2".into()),
        },
    );
    let created = mint_mcp_token(state.clone(), owner.id, "owner only").await;

    let cross_user_revoke = revoke_token(
        State(state.clone()),
        owner_auth(other.id),
        Path(created.summary.id),
    )
    .await
    .expect_err("another user must not be able to revoke the token");
    assert_eq!(cross_user_revoke.0, StatusCode::NOT_FOUND);

    let Json(other_tokens) = list_tokens(State(state.clone()), owner_auth(other.id))
        .await
        .unwrap();
    assert!(other_tokens.is_empty());
    let Json(owner_tokens) = list_tokens(State(state), owner_auth(owner.id))
        .await
        .unwrap();
    assert_eq!(owner_tokens.len(), 1);
    assert_eq!(owner_tokens[0].id, created.summary.id);
}

#[tokio::test]
#[serial]
async fn non_subscriber_cannot_create_or_use_mcp_token() {
    let state = create_test_state();
    let user = create_test_user(
        &state,
        &TestUserParams {
            email: "free-mcp@example.com".into(),
            phone_number: "+14155550992".into(),
            credits: 0.0,
            credits_left: 0.0,
            sub_tier: None,
        },
    );
    let error = create_token(
        State(state),
        owner_auth(user.id),
        Json(CreateMcpTokenRequest {
            label: "not allowed".into(),
        }),
    )
    .await
    .expect_err("non-subscriber token creation must fail");
    assert_eq!(error.0, StatusCode::FORBIDDEN);
}
