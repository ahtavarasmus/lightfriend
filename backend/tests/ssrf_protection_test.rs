use backend::services::mcp_client::McpClientService;
use backend::tool_call_utils::internet::scan_qr_code;
use backend::utils::ssrf::{
    is_disallowed_ip, validate_public_http_target, validate_resolved_addresses,
};
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[test]
fn ssrf_policy_blocks_local_and_special_use_addresses() {
    for address in [
        "0.0.0.0",
        "10.0.0.1",
        "100.64.0.1",
        "127.0.0.1",
        "169.254.169.253",
        "169.254.169.254",
        "172.16.0.1",
        "192.168.0.1",
        "224.0.0.1",
        "255.255.255.255",
        "::",
        "::1",
        "::ffff:127.0.0.1",
        "fd00::1",
        "fe80::1",
        "ff02::1",
    ] {
        let ip: IpAddr = address.parse().unwrap();
        assert!(is_disallowed_ip(ip), "{address} should be blocked");
    }

    for address in ["8.8.8.8", "93.184.216.34", "2606:4700:4700::1111"] {
        let ip: IpAddr = address.parse().unwrap();
        assert!(!is_disallowed_ip(ip), "{address} should be allowed");
    }
}

#[test]
fn ssrf_policy_rejects_non_http_and_private_literal_urls() {
    for url in [
        "file:///etc/passwd",
        "ftp://example.com/file",
        "http://localhost/admin",
        "http://127.0.0.1:3000/admin",
        "http://[::1]/admin",
        "http://169.254.169.253/latest/meta-data/",
        "https://user:password@example.com/",
    ] {
        assert!(
            validate_public_http_target(url).is_err(),
            "{url} should be blocked"
        );
    }

    assert!(validate_public_http_target("https://example.com/image.png").is_ok());
}

#[test]
fn mixed_public_and_private_dns_answers_are_rejected() {
    let mixed = [
        SocketAddr::from(([93, 184, 216, 34], 443)),
        SocketAddr::from(([10, 0, 0, 1], 443)),
    ];
    assert!(validate_resolved_addresses(&mixed).is_err());

    let public = [
        SocketAddr::from(([93, 184, 216, 34], 443)),
        SocketAddr::from(([8, 8, 8, 8], 443)),
    ];
    assert!(validate_resolved_addresses(&public).is_ok());
    assert!(validate_resolved_addresses(&[]).is_err());
}

#[tokio::test]
async fn qr_scanner_rejects_private_targets_before_fetching() {
    let error = scan_qr_code("http://127.0.0.1:3000/private")
        .await
        .expect_err("private QR target should be rejected");

    assert!(error.to_string().contains("Private or local"));
}

#[tokio::test]
async fn mcp_client_rejects_private_targets_before_connecting() {
    let error = McpClientService::new()
        .list_tools("http://169.254.169.253/latest/meta-data/", None)
        .await
        .expect_err("private MCP target should be rejected");

    assert!(error.contains("Private or local"));
}

#[tokio::test]
async fn mcp_client_blocks_private_dns_answers_at_connection_time() {
    let requests = Arc::new(AtomicUsize::new(0));
    let handler_requests = requests.clone();
    let app = axum::Router::new().route(
        "/",
        axum::routing::post(move || {
            let handler_requests = handler_requests.clone();
            async move {
                handler_requests.fetch_add(1, Ordering::SeqCst);
                axum::Json(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 1,
                    "result": {"tools": []}
                }))
            }
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

    let result = McpClientService::new()
        .list_tools(&format!("http://localhost.:{port}/"), None)
        .await;
    server.abort();

    assert!(result.is_err());
    assert_eq!(requests.load(Ordering::SeqCst), 0);
}
