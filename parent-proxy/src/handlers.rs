use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use serde::Deserialize;
use std::sync::Arc;
use tracing::{error, info, warn};

use crate::db::EnclaveTarget;
use crate::proxy::forward_to_enclave;
use crate::AppState;

/// Health check endpoint
pub async fn health_check() -> &'static str {
    "OK"
}

/// Twilio SMS webhook handler
/// Extracts phone number from form body and routes to appropriate enclave
pub async fn twilio_sms_webhook(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    // Parse the URL-encoded form body to extract the From phone number
    let phone_number = match parse_twilio_from_number(&body) {
        Ok(number) => number,
        Err(e) => {
            error!(error = %e, "Failed to parse Twilio webhook body");
            return (StatusCode::BAD_REQUEST, "Invalid request body").into_response();
        }
    };

    info!(phone_number = %phone_number, "Routing Twilio SMS webhook");

    // Look up which enclave to route to
    let target = match state.db.get_enclave_by_phone(&phone_number) {
        Ok(target) => target,
        Err(e) => {
            error!(error = %e, "Database error looking up user");
            // Default to new enclave on DB error
            EnclaveTarget::New
        }
    };

    info!(phone_number = %phone_number, target = ?target, "Forwarding Twilio SMS webhook");

    // Forward the request to the enclave
    match forward_to_enclave(
        &state.config,
        target,
        "POST",
        "/api/sms/server",
        &headers,
        body,
    )
    .await
    {
        Ok(response) => {
            let mut resp = Response::builder()
                .status(response.status);

            // Copy response headers
            for (name, value) in response.headers.iter() {
                resp = resp.header(name, value);
            }

            resp.body(Body::from(response.body))
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
        }
        Err(e) => {
            error!(error = %e, "Failed to forward request to enclave");
            (StatusCode::BAD_GATEWAY, "Failed to reach backend").into_response()
        }
    }
}

/// ElevenLabs voice webhook handler
/// Extracts user_id from JSON body and routes to appropriate enclave
pub async fn elevenlabs_webhook(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    // Parse the JSON body to extract the user_id
    let user_id = match parse_elevenlabs_user_id(&body) {
        Ok(id) => id,
        Err(e) => {
            warn!(error = %e, "Failed to parse ElevenLabs webhook body, routing to new enclave");
            // If we can't parse the user_id, route to new enclave
            return forward_elevenlabs_to_enclave(&state, EnclaveTarget::New, &headers, body).await;
        }
    };

    info!(user_id, "Routing ElevenLabs webhook");

    // Look up which enclave to route to
    let target = match state.db.get_enclave_by_user_id(user_id) {
        Ok(target) => target,
        Err(e) => {
            error!(error = %e, "Database error looking up user");
            // Default to new enclave on DB error
            EnclaveTarget::New
        }
    };

    info!(user_id, target = ?target, "Forwarding ElevenLabs webhook");

    forward_elevenlabs_to_enclave(&state, target, &headers, body).await
}

/// Forward ElevenLabs webhook to enclave
async fn forward_elevenlabs_to_enclave(
    state: &AppState,
    target: EnclaveTarget,
    headers: &HeaderMap,
    body: Bytes,
) -> Response {
    match forward_to_enclave(
        &state.config,
        target,
        "POST",
        "/api/webhook/elevenlabs",
        headers,
        body,
    )
    .await
    {
        Ok(response) => {
            let mut resp = Response::builder()
                .status(response.status);

            // Copy response headers
            for (name, value) in response.headers.iter() {
                resp = resp.header(name, value);
            }

            resp.body(Body::from(response.body))
                .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
        }
        Err(e) => {
            error!(error = %e, "Failed to forward request to enclave");
            (StatusCode::BAD_GATEWAY, "Failed to reach backend").into_response()
        }
    }
}

/// Parse the "From" phone number from Twilio's URL-encoded form body
fn parse_twilio_from_number(body: &[u8]) -> anyhow::Result<String> {
    #[derive(Deserialize)]
    struct TwilioParams {
        #[serde(rename = "From")]
        from: String,
    }

    let params: TwilioParams = serde_urlencoded::from_bytes(body)
        .map_err(|e| anyhow::anyhow!("Failed to parse form body: {}", e))?;

    Ok(params.from)
}

/// Parse the user_id from ElevenLabs JSON body
/// Path: data.conversation_initiation_client_data.dynamic_variables.user_id
fn parse_elevenlabs_user_id(body: &[u8]) -> anyhow::Result<i32> {
    #[derive(Deserialize)]
    struct ElevenLabsPayload {
        data: Option<ElevenLabsData>,
    }

    #[derive(Deserialize)]
    struct ElevenLabsData {
        conversation_initiation_client_data: Option<ConversationInitData>,
    }

    #[derive(Deserialize)]
    struct ConversationInitData {
        dynamic_variables: Option<DynamicVariables>,
    }

    #[derive(Deserialize)]
    struct DynamicVariables {
        user_id: Option<serde_json::Value>,
    }

    let payload: ElevenLabsPayload = serde_json::from_slice(body)
        .map_err(|e| anyhow::anyhow!("Failed to parse JSON body: {}", e))?;

    let user_id = payload
        .data
        .and_then(|d| d.conversation_initiation_client_data)
        .and_then(|c| c.dynamic_variables)
        .and_then(|v| v.user_id)
        .ok_or_else(|| anyhow::anyhow!("user_id not found in payload"))?;

    // user_id can be a string or number
    match user_id {
        serde_json::Value::Number(n) => n
            .as_i64()
            .map(|i| i as i32)
            .ok_or_else(|| anyhow::anyhow!("user_id is not a valid integer")),
        serde_json::Value::String(s) => s
            .parse::<i32>()
            .map_err(|e| anyhow::anyhow!("Failed to parse user_id as integer: {}", e)),
        _ => Err(anyhow::anyhow!("user_id is neither a string nor number")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_twilio_from_number() {
        let body = b"ToCountry=US&ToState=&SmsMessageSid=SM123&NumMedia=0&ToCity=&FromZip=&SmsSid=SM123&FromState=&SmsStatus=received&FromCity=&Body=Hello&FromCountry=US&To=%2B12025551234&ToZip=&NumSegments=1&From=%2B14155551234&AccountSid=AC123&ApiVersion=2010-04-01";
        let number = parse_twilio_from_number(body).unwrap();
        assert_eq!(number, "+14155551234");
    }

    #[test]
    fn test_parse_twilio_from_number_simple() {
        let body = b"From=%2B14155551234&To=%2B12025551234&Body=Test";
        let number = parse_twilio_from_number(body).unwrap();
        assert_eq!(number, "+14155551234");
    }

    #[test]
    fn test_parse_elevenlabs_user_id_number() {
        let body = r#"{
            "data": {
                "conversation_initiation_client_data": {
                    "dynamic_variables": {
                        "user_id": 12345
                    }
                }
            }
        }"#;
        let user_id = parse_elevenlabs_user_id(body.as_bytes()).unwrap();
        assert_eq!(user_id, 12345);
    }

    #[test]
    fn test_parse_elevenlabs_user_id_string() {
        let body = r#"{
            "data": {
                "conversation_initiation_client_data": {
                    "dynamic_variables": {
                        "user_id": "67890"
                    }
                }
            }
        }"#;
        let user_id = parse_elevenlabs_user_id(body.as_bytes()).unwrap();
        assert_eq!(user_id, 67890);
    }

    #[test]
    fn test_parse_elevenlabs_user_id_missing() {
        let body = r#"{"data": {}}"#;
        assert!(parse_elevenlabs_user_id(body.as_bytes()).is_err());
    }

    #[test]
    fn test_parse_elevenlabs_user_id_empty_body() {
        let body = r#"{}"#;
        assert!(parse_elevenlabs_user_id(body.as_bytes()).is_err());
    }
}
