use anyhow::{Context, Result};
use axum::http::{HeaderMap, HeaderName, HeaderValue, StatusCode};
use bytes::Bytes;
use tracing::{debug, info};

use crate::db::EnclaveTarget;
use crate::Config;

/// Response from the enclave
pub struct EnclaveResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
}

/// Forward an HTTP request to an enclave
/// On Linux: uses VSOCK for Nitro Enclave communication
/// On other platforms: uses HTTP for development/testing
pub async fn forward_to_enclave(
    config: &Config,
    target: EnclaveTarget,
    method: &str,
    path: &str,
    headers: &HeaderMap,
    body: Bytes,
) -> Result<EnclaveResponse> {
    info!(
        target = ?target,
        method,
        path,
        body_len = body.len(),
        "Forwarding request to enclave"
    );

    #[cfg(target_os = "linux")]
    {
        forward_vsock(config, target, method, path, headers, body).await
    }

    #[cfg(not(target_os = "linux"))]
    {
        forward_http(config, target, method, path, headers, body).await
    }
}

/// Linux implementation using VSOCK for Nitro Enclaves
#[cfg(target_os = "linux")]
async fn forward_vsock(
    config: &Config,
    target: EnclaveTarget,
    method: &str,
    path: &str,
    headers: &HeaderMap,
    body: Bytes,
) -> Result<EnclaveResponse> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio_vsock::VsockStream;

    let (cid, port) = match target {
        EnclaveTarget::Old => (config.old_enclave_cid, config.old_enclave_port),
        EnclaveTarget::New => (config.new_enclave_cid, config.new_enclave_port),
    };

    info!(cid, port, "Connecting to enclave via VSOCK");

    // Build HTTP request
    let mut request = format!("{} {} HTTP/1.1\r\n", method, path);
    request.push_str("Host: enclave\r\n");

    // Forward headers
    for (name, value) in headers.iter() {
        // Skip hop-by-hop headers
        let name_str = name.as_str();
        if name_str == "host" || name_str == "connection" || name_str == "transfer-encoding" {
            continue;
        }
        if let Ok(value_str) = value.to_str() {
            request.push_str(&format!("{}: {}\r\n", name, value_str));
        }
    }

    // Add Content-Length if we have a body
    if !body.is_empty() {
        request.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }

    request.push_str("Connection: close\r\n");
    request.push_str("\r\n");

    let mut request_bytes = request.into_bytes();
    request_bytes.extend_from_slice(&body);

    // Connect to enclave via VSOCK
    let mut stream = VsockStream::connect(cid, port)
        .await
        .context("Failed to connect to enclave via VSOCK")?;

    // Send request
    stream
        .write_all(&request_bytes)
        .await
        .context("Failed to send request to enclave")?;

    // Shutdown write side to signal we're done sending
    stream.shutdown().await.ok();

    // Read response
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .await
        .context("Failed to read response from enclave")?;

    parse_http_response(&response)
}

/// Non-Linux implementation using HTTP for development/testing
#[cfg(not(target_os = "linux"))]
async fn forward_http(
    config: &Config,
    target: EnclaveTarget,
    method: &str,
    path: &str,
    headers: &HeaderMap,
    body: Bytes,
) -> Result<EnclaveResponse> {
    use reqwest::Client;

    // In development mode, use HTTP URLs from env vars
    // OLD_ENCLAVE_URL and NEW_ENCLAVE_URL should be set for testing
    let base_url = match target {
        EnclaveTarget::Old => std::env::var("OLD_ENCLAVE_URL")
            .unwrap_or_else(|_| format!("http://127.0.0.1:{}", config.old_enclave_port)),
        EnclaveTarget::New => std::env::var("NEW_ENCLAVE_URL")
            .unwrap_or_else(|_| format!("http://127.0.0.1:{}", config.new_enclave_port)),
    };

    let url = format!("{}{}", base_url, path);
    info!(url, "Forwarding request via HTTP (development mode)");

    let client = Client::new();

    let mut req = match method {
        "POST" => client.post(&url),
        "GET" => client.get(&url),
        "PUT" => client.put(&url),
        "DELETE" => client.delete(&url),
        _ => client.post(&url),
    };

    // Forward headers
    for (name, value) in headers.iter() {
        let name_str = name.as_str();
        if name_str == "host" || name_str == "connection" || name_str == "transfer-encoding" {
            continue;
        }
        if let Ok(value_str) = value.to_str() {
            req = req.header(name_str, value_str);
        }
    }

    // Send request
    let response = req
        .body(body.to_vec())
        .send()
        .await
        .context("Failed to send HTTP request")?;

    let status = StatusCode::from_u16(response.status().as_u16())
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    let mut resp_headers = HeaderMap::new();
    for (name, value) in response.headers().iter() {
        if let (Ok(name), Ok(value)) = (
            HeaderName::try_from(name.as_str()),
            HeaderValue::try_from(value.as_bytes()),
        ) {
            resp_headers.insert(name, value);
        }
    }

    let body = response.bytes().await.context("Failed to read response body")?;

    debug!(
        status = status.as_u16(),
        body_len = body.len(),
        "Received response from enclave"
    );

    Ok(EnclaveResponse {
        status,
        headers: resp_headers,
        body,
    })
}

/// Parse an HTTP/1.1 response
#[allow(dead_code)]
fn parse_http_response(response: &[u8]) -> Result<EnclaveResponse> {
    let response_str = String::from_utf8_lossy(response);

    // Find header/body separator
    let header_end = response_str
        .find("\r\n\r\n")
        .context("Invalid HTTP response: no header terminator")?;

    let header_section = &response_str[..header_end];
    let body_start = header_end + 4;
    let body = if body_start < response.len() {
        Bytes::copy_from_slice(&response[body_start..])
    } else {
        Bytes::new()
    };

    // Parse status line
    let mut lines = header_section.lines();
    let status_line = lines.next().context("Invalid HTTP response: no status line")?;

    let status = parse_status_line(status_line)?;

    // Parse headers
    let mut headers = HeaderMap::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            let name = name.trim();
            let value = value.trim();
            if let (Ok(name), Ok(value)) = (
                HeaderName::try_from(name),
                HeaderValue::try_from(value),
            ) {
                headers.insert(name, value);
            }
        }
    }

    debug!(
        status = status.as_u16(),
        body_len = body.len(),
        "Received response from enclave"
    );

    Ok(EnclaveResponse {
        status,
        headers,
        body,
    })
}

/// Parse HTTP status line like "HTTP/1.1 200 OK"
fn parse_status_line(line: &str) -> Result<StatusCode> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 2 {
        anyhow::bail!("Invalid status line: {}", line);
    }

    let code: u16 = parts[1]
        .parse()
        .with_context(|| format!("Invalid status code: {}", parts[1]))?;

    StatusCode::from_u16(code).context("Invalid HTTP status code")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_status_line() {
        assert_eq!(parse_status_line("HTTP/1.1 200 OK").unwrap(), StatusCode::OK);
        assert_eq!(
            parse_status_line("HTTP/1.1 404 Not Found").unwrap(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            parse_status_line("HTTP/1.1 500 Internal Server Error").unwrap(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[test]
    fn test_parse_http_response() {
        let response = b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: 5\r\n\r\nhello";
        let parsed = parse_http_response(response).unwrap();

        assert_eq!(parsed.status, StatusCode::OK);
        assert_eq!(parsed.headers.get("content-type").unwrap(), "text/plain");
        assert_eq!(parsed.body.as_ref(), b"hello");
    }

    #[test]
    fn test_parse_http_response_empty_body() {
        let response = b"HTTP/1.1 204 No Content\r\n\r\n";
        let parsed = parse_http_response(response).unwrap();

        assert_eq!(parsed.status, StatusCode::NO_CONTENT);
        assert!(parsed.body.is_empty());
    }
}
