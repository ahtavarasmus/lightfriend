use axum::{
    body::Body,
    http::{header::USER_AGENT, HeaderMap, Method, Request},
    middleware::Next,
    response::Response,
};
use serde::Serialize;
use std::{net::IpAddr, sync::OnceLock, time::Duration};

const DATAFAST_ENDPOINT: &str = "https://datafa.st/api/ai-crawls";
const DATAFAST_WEBSITE_ID: &str = "dfid_ICHRky5CwoxQQSthciEQz";
const DATAFAST_DOMAIN: &str = "lightfriend.ai";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DataFastBotEvent {
    website_id: &'static str,
    domain: &'static str,
    href: String,
    ai: DataFastBotDetails,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DataFastBotDetails {
    user_agent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    ip: Option<String>,
    status_code: u16,
    source: &'static str,
}

/// Reports AI crawler traffic after the response has completed. Reporting is
/// best-effort and happens in a detached task, so DataFast can never delay or
/// fail a Lightfriend request.
pub async fn track_bot_traffic(request: Request<Body>, next: Next) -> Response {
    let event = candidate_event(&request);
    let response = next.run(request).await;

    if let Some((user_agent, ip, href)) = event {
        let status_code = response.status().as_u16();
        tokio::spawn(async move {
            report_event(DataFastBotEvent {
                website_id: DATAFAST_WEBSITE_ID,
                domain: DATAFAST_DOMAIN,
                href,
                ai: DataFastBotDetails {
                    user_agent,
                    ip,
                    status_code,
                    source: "server_middleware",
                },
            })
            .await;
        });
    }

    response
}

fn candidate_event(request: &Request<Body>) -> Option<(String, Option<String>, String)> {
    if !tracking_enabled()
        || !matches!(request.method(), &Method::GET | &Method::HEAD)
        || !is_trackable_public_path(request.uri().path())
    {
        return None;
    }

    let user_agent = request
        .headers()
        .get(USER_AGENT)
        .and_then(|value| value.to_str().ok())?;
    if !is_likely_bot(user_agent) {
        return None;
    }

    Some((
        user_agent.to_owned(),
        cloudflare_client_ip(request.headers()),
        public_href(request.uri().path()),
    ))
}

fn tracking_enabled() -> bool {
    match std::env::var("DATAFAST_BOT_TRACKING_ENABLED") {
        Ok(value) => matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes"
        ),
        Err(_) => matches!(
            std::env::var("ENVIRONMENT").as_deref(),
            Ok("production" | "staging")
        ),
    }
}

/// Performs only a cheap prefilter. DataFast makes the authoritative crawler
/// classification server-side and discards false positives.
pub fn is_likely_bot(user_agent: &str) -> bool {
    let user_agent = user_agent.to_ascii_lowercase();
    [
        "bot",
        "crawler",
        "spider",
        "chatgpt",
        "gptbot",
        "claude",
        "perplexity",
        "bing",
        "google",
        "applebot",
        "bytespider",
        "ccbot",
        "anthropic",
        "cohere",
        "youbot",
    ]
    .iter()
    .any(|marker| user_agent.contains(marker))
}

/// Excludes private application routes and files that generate noisy asset
/// fetches. Text formats intended for crawlers remain trackable.
pub fn is_trackable_public_path(path: &str) -> bool {
    if matches!(
        path,
        "/robots.txt" | "/llms.txt" | "/llms-full.txt" | "/assets/llm.txt"
    ) {
        return true;
    }

    const PRIVATE_PREFIXES: &[&str] = &[
        "/api",
        "/uploads",
        "/assets",
        "/admin",
        "/billing",
        "/login",
        "/password-reset",
        "/set-password",
        "/subscription-success",
        "/.well-known",
    ];

    if PRIVATE_PREFIXES
        .iter()
        .any(|prefix| path == *prefix || path.starts_with(&format!("{prefix}/")))
    {
        return false;
    }

    let filename = path.rsplit('/').next().unwrap_or_default();
    match filename.rsplit_once('.') {
        Some((_, extension)) => matches!(extension, "html" | "md" | "txt" | "xml"),
        None => true,
    }
}

fn cloudflare_client_ip(headers: &HeaderMap) -> Option<String> {
    headers
        .get("CF-Connecting-IP")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<IpAddr>().ok())
        .map(|ip| ip.to_string())
}

fn public_href(path: &str) -> String {
    format!("https://{DATAFAST_DOMAIN}{path}")
}

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(Duration::from_millis(500))
            .timeout(Duration::from_secs(2))
            .build()
            .expect("static DataFast HTTP client configuration must be valid")
    })
}

async fn report_event(event: DataFastBotEvent) {
    let mut request = client().post(DATAFAST_ENDPOINT).json(&event);
    if let Ok(token) = std::env::var("DATAFAST_BOT_TOKEN") {
        if !token.trim().is_empty() {
            request = request.bearer_auth(token.trim());
        }
    }

    match request.send().await {
        Ok(response) if response.status().is_success() => {}
        Ok(response) => tracing::debug!(
            status = %response.status(),
            "DataFast rejected a bot-traffic event"
        ),
        Err(error) => tracing::debug!(error = %error, "Could not report DataFast bot traffic"),
    }
}
