use axum::{
    extract::Query,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Default)]
pub struct AttestationQuery {
    pub nonce: Option<String>,
    pub public_key: Option<String>,
    pub user_data: Option<String>,
}

#[derive(Serialize)]
pub struct AttestationMetadataResponse {
    pub attestation_raw_url: String,
    pub attestation_hex_url: String,
    pub build_metadata_url: Option<String>,
    pub commit_sha: Option<String>,
    pub workflow_run_id: Option<String>,
    pub image_ref: Option<String>,
    pub eif_sha256: Option<String>,
    pub pcr0: Option<String>,
    pub pcr1: Option<String>,
    pub pcr2: Option<String>,
    pub kms_contract_address: Option<String>,
}

fn attestation_base_url() -> String {
    std::env::var("SERVER_URL")
        .unwrap_or_else(|_| "http://localhost:3000".to_string())
        .trim_end_matches('/')
        .to_string()
}

fn build_attestation_url(path: &str) -> String {
    format!(
        "{}/{}",
        attestation_base_url(),
        path.trim_start_matches('/')
    )
}

fn build_query(query: &AttestationQuery) -> Vec<(&'static str, &str)> {
    let mut params = Vec::new();
    if let Some(nonce) = query.nonce.as_deref() {
        params.push(("nonce", nonce));
    }
    if let Some(public_key) = query.public_key.as_deref() {
        params.push(("public_key", public_key));
    }
    if let Some(user_data) = query.user_data.as_deref() {
        params.push(("user_data", user_data));
    }
    params
}

async fn fetch_attestation(
    endpoint: &str,
    query: &AttestationQuery,
) -> Result<reqwest::Response, (StatusCode, String)> {
    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:1300{endpoint}");
    client
        .get(url)
        .query(&build_query(query))
        .send()
        .await
        .map_err(|e| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                format!("failed to fetch enclave attestation: {e}"),
            )
        })
}

pub async fn attestation_raw(Query(query): Query<AttestationQuery>) -> Response {
    match fetch_attestation("/attestation/raw", &query).await {
        Ok(resp) => {
            let status =
                StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            match resp.bytes().await {
                Ok(body) => {
                    let mut headers = HeaderMap::new();
                    headers.insert(
                        header::CONTENT_TYPE,
                        HeaderValue::from_static("application/octet-stream"),
                    );
                    (status, headers, body).into_response()
                }
                Err(e) => (
                    StatusCode::BAD_GATEWAY,
                    format!("failed to read raw attestation body: {e}"),
                )
                    .into_response(),
            }
        }
        Err(err) => err.into_response(),
    }
}

pub async fn attestation_hex(Query(query): Query<AttestationQuery>) -> Response {
    match fetch_attestation("/attestation/hex", &query).await {
        Ok(resp) => {
            let status =
                StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            match resp.text().await {
                Ok(body) => {
                    let mut headers = HeaderMap::new();
                    headers.insert(
                        header::CONTENT_TYPE,
                        HeaderValue::from_static("text/plain; charset=utf-8"),
                    );
                    (status, headers, body).into_response()
                }
                Err(e) => (
                    StatusCode::BAD_GATEWAY,
                    format!("failed to read hex attestation body: {e}"),
                )
                    .into_response(),
            }
        }
        Err(err) => err.into_response(),
    }
}

pub async fn attestation_metadata() -> Json<AttestationMetadataResponse> {
    Json(AttestationMetadataResponse {
        attestation_raw_url: build_attestation_url("/.well-known/lightfriend/attestation/raw"),
        attestation_hex_url: build_attestation_url("/.well-known/lightfriend/attestation/hex"),
        build_metadata_url: std::env::var("CURRENT_BUILD_METADATA_URL").ok(),
        commit_sha: std::env::var("CURRENT_COMMIT_SHA").ok(),
        workflow_run_id: std::env::var("CURRENT_WORKFLOW_RUN_ID").ok(),
        image_ref: std::env::var("CURRENT_IMAGE_REF").ok(),
        eif_sha256: std::env::var("CURRENT_EIF_SHA256").ok(),
        pcr0: std::env::var("CURRENT_PCR0").ok(),
        pcr1: std::env::var("CURRENT_PCR1").ok(),
        pcr2: std::env::var("CURRENT_PCR2").ok(),
        kms_contract_address: std::env::var("MARLIN_KMS_CONTRACT_ADDRESS").ok(),
    })
}
