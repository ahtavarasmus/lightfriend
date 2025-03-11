use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use std::env;

const COMPOSIO_API_BASE_URL: &str = "https://backend.composio.dev/api/v1";

#[derive(Debug, Serialize, Deserialize)]
struct ExpectedInputField {
    name: String,
    #[serde(rename = "type")]
    field_type: String,
    description: String,
    display_name: String,
    default: Option<serde_json::Value>,
    required: bool,
    expected_from_customer: bool,
    is_secret: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct Integration {
    enabled: bool,
    app_id: String,
    auth_config: serde_json::Value,
    expected_input_fields: Vec<ExpectedInputField>,
    logo: String,
    app_name: String,
    use_composio_auth: bool,
    limited_actions: Vec<String>,
    id: String,
    auth_scheme: String,
    name: String,
    created_at: String,
    updated_at: String,
    deleted: bool,
    default_connector_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ConnectionRequest {
    connection_status: String,
    connected_account_id: String,
    redirect_url: String,
}

#[derive(Debug, Serialize)]
struct ConnectionRequestPayload {
    data: serde_json::Value,
    integration_id: String,
}

pub struct ComposioClient {
    client: reqwest::Client,
    api_key: String,
}

impl ComposioClient {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let api_key = env::var("COMPOSIO_API_KEY")
            .expect("COMPOSIO_API_KEY must be set");
        
        Ok(Self {
            client: reqwest::Client::new(),
            api_key,
        })
    }

    fn get_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", HeaderValue::from_str(&self.api_key).unwrap());
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers
    }

    pub async fn get_integration(&self, integration_id: &str) -> Result<Integration, Box<dyn std::error::Error>> {
        let url = format!("{}/integrations/{}", COMPOSIO_API_BASE_URL, integration_id);
        
        let response = self.client
            .get(&url)
            .headers(self.get_headers())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(format!("Failed to get integration: {}", response.status()).into());
        }

        let integration = response.json::<Integration>().await?;
        Ok(integration)
    }

    pub async fn initiate_connection(
        &self,
        integration_id: &str,
        data: serde_json::Value,
    ) -> Result<ConnectionRequest, Box<dyn std::error::Error>> {
        let url = format!("{}/connectedAccounts", COMPOSIO_API_BASE_URL);
        
        let payload = ConnectionRequestPayload {
            data,
            integration_id: integration_id.to_string(),
        };

        let response = self.client
            .post(&url)
            .headers(self.get_headers())
            .json(&payload)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(format!("Failed to initiate connection: {}", response.status()).into());
        }

        let connection_request = response.json::<ConnectionRequest>().await?;
        Ok(connection_request)
    }
}

// Handler for getting integration details
pub async fn get_integration_handler(
    State(composio_client): State<ComposioClient>,
    Json(integration_id): Json<String>,
) -> Result<Json<Integration>, (StatusCode, String)> {
    composio_client
        .get_integration(&integration_id)
        .await
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

// Handler for initiating connection
pub async fn initiate_connection_handler(
    State(composio_client): State<ComposioClient>,
    Json(payload): Json<ConnectionRequestPayload>,
) -> Result<Json<ConnectionRequest>, (StatusCode, String)> {
    composio_client
        .initiate_connection(&payload.integration_id, payload.data)
        .await
        .map(Json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}
/*

Use composio as a http client to make request to the connected account service on your behalf, without managing authentication on your side.

curl -X POST https://backend.composio.dev/api/v2/actions/proxy \
     -H "x-api-key: <apiKey>" \
     -H "Content-Type: application/json" \
     -d '{
  "connectedAccountId": "connectedAccountId",
  "endpoint": "endpoint",
  "method": "GET",
  "parameters": [
    {
      "name": "name",
      "in": "query",
      "value": "value"
    }
  ]

}'

Request
This endpoint expects an object.
connectedAccountId string Required

The connected account uuid to use for the action.
endpoint string Required

The endpoint to call for the action. If the given url is relative, it will be resolved relative to the base_url set in the connected account info.
method enum Required
Allowed values: GETPOSTPUTPATCHDELETE

The HTTP method to use for the action.
parameters list of objects Required
body map from strings to any Optional

The body to be sent to the endpoint. This can either be a JSON field or a string.

{
  "data": {
    "key": "value"
  },
  "successful": true,
  "error": "error",
  "logId": "logId",
  "sessionInfo": {
    "key": "value"
  }
}
*/
