use axum::{
    extract::State,
    http::{StatusCode, HeaderMap},
    response::Json,
};
use serde_json::json;
use serde::{Deserialize, Serialize};
use reqwest::header::{HeaderValue, CONTENT_TYPE};
use std::{env, sync::Arc};
use jsonwebtoken::{decode, DecodingKey, Validation, Algorithm};

use crate::{
    AppState,
    handlers::auth_dtos::Claims,
};

const COMPOSIO_API_BASE_URL: &str = "https://backend.composio.dev/api/v1";

#[derive(Debug, Serialize, Deserialize)]
pub struct ExpectedInputField {
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
pub struct Integration {
    enabled: Option<bool>,
    app_id: Option<String>,
    auth_config: Option<serde_json::Value>,
    expected_input_fields: Option<Vec<ExpectedInputField>>,
    logo: Option<String>,
    app_name: Option<String>,
    use_composio_auth: Option<bool>,
    limited_actions: Option<Vec<String>>,
    id: Option<String>,
    auth_scheme: Option<String>,
    name: Option<String>,
    created_at: Option<String>,
    updated_at: Option<String>,
    deleted: Option<bool>,
    default_connector_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConnectionRequest {
    #[serde(rename = "connectionStatus")]
    connection_status: String,
    #[serde(rename = "connectedAccountId")]
    connected_account_id: String,
    #[serde(rename = "redirectUrl")]
    redirect_url: String,
}

#[derive(Debug, Serialize)]
pub struct ConnectionRequestPayload {
    data: serde_json::Value,
    integration_id: String,
}

pub struct ComposioClient {
    client: reqwest::Client,
    api_key: String,
    outlook_integration_id: String,
}

impl ComposioClient {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let api_key = env::var("COMPOSIO_API_KEY")
            .expect("COMPOSIO_API_KEY must be set");
        let outlook_integration_id = env::var("COMPOSIO_OUTLOOK_INTEGRATION_ID")
            .expect("COMPOSIO_OUTLOOK_INTEGRATION_ID must be set");
        
        Ok(Self {
            client: reqwest::Client::new(),
            api_key,
            outlook_integration_id,
        })
    }

    pub fn get_integration_id(&self, integration_name: &str) -> Result<String, Box<dyn std::error::Error>> {
        match integration_name.to_uppercase().as_str() {
            "OUTLOOK" => Ok(self.outlook_integration_id.clone()),
            _ => Err("Unsupported integration".into()),
        }
    }

    fn get_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::HeaderName::from_static("x-api-key"),
            reqwest::header::HeaderValue::from_str(&self.api_key).unwrap()
        );
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            reqwest::header::HeaderValue::from_static("application/json")
        );
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

        // Log the raw response for debugging
        let response_text = response.text().await?;
        println!("Raw integration response: {}", response_text);

        // Parse the response text
        let integration: Integration = serde_json::from_str(&response_text)
            .map_err(|e| format!("Failed to parse integration response: {}. Response body: {}", e, response_text))?;
            
        Ok(integration)
    }

    pub async fn initiate_connection(
        &self,
        integration_id: &str,
        data: serde_json::Value,
    ) -> Result<ConnectionRequest, Box<dyn std::error::Error>> {
        println!("Initiating connection with integration_id: {}", integration_id);
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

        // Log the raw response for debugging
        let response_text = response.text().await?;
        println!("Raw connection response: {}", response_text);

        // Parse the response and extract the redirect URL
        let response_data: serde_json::Value = serde_json::from_str(&response_text)?;
        
        // Try to get the redirect URL from various possible locations
        let redirect_url = response_data.get("redirect_url")
            .or_else(|| response_data.get("redirectUrl"))
            .or_else(|| {
                response_data.get("authConfig")
                    .and_then(|auth_config| auth_config.get("oauth_redirect_uri")
                        .or_else(|| auth_config.get("redirectUrl")))
            })
            .and_then(|url| url.as_str())
            .ok_or("No redirect URL found in response")?;

        Ok(ConnectionRequest {
            connection_status: "PENDING".to_string(),
            connected_account_id: response_data.get("id")
                .and_then(|id| id.as_str())
                .unwrap_or("")
                .to_string(),
            redirect_url: redirect_url.to_string(),
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct IntegrationRequest {
    user_id: i32,
    integration_name: String,
}

#[derive(Debug, Deserialize)]
pub struct ConnectionInitRequest {
    user_id: i32,
    integration_name: String,
    data: Option<serde_json::Value>,
    name: Option<String>,
    description: Option<String>,
}

use axum::extract::Query;
use std::collections::HashMap;
use axum::response::{IntoResponse, Redirect};

// Handler for getting integration details
pub async fn get_integration_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<IntegrationRequest>,
) -> Result<Json<Integration>, (StatusCode, Json<serde_json::Value>)> {
    println!("Received integration request: {:?}", request);
    
    // Extract and validate token
    let auth_header = headers.get("Authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "));

    let token = match auth_header {
        Some(token) => {
            println!("Token successfully extracted");
            token
        },
        None => {
            println!("No authorization token provided");
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "No authorization token provided"}))
            ));
        }
    };

    // Decode JWT token
    let claims = match decode::<Claims>(
        token,
        &DecodingKey::from_secret(std::env::var("JWT_SECRET_KEY")
            .expect("JWT_SECRET_KEY must be set in environment")
            .as_bytes()),
        &Validation::new(Algorithm::HS256)
    ) {
        Ok(token_data) => {
            println!("JWT token successfully decoded");
            token_data.claims
        },
        Err(e) => {
            println!("Failed to decode JWT token: {:?}", e);
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Invalid token"}))
            ));
        }
    };

    // Verify user exists
    if state.user_core.find_by_id(claims.sub).map_err(|e| {
        println!("Database error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("Database error: {}", e)}))
        )
    })?.is_none() {
        println!("User not found for id: {}", claims.sub);
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found"}))
        ));
    }

    println!("User verification successful");

    let composio_client = ComposioClient::new()
        .map_err(|e| {
            println!("Failed to create Composio client: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
        })?;

    let integration_id = composio_client
        .get_integration_id(&request.integration_name)
        .map_err(|e| {
            println!("Failed to get integration ID: {}", e);
            (StatusCode::BAD_REQUEST, Json(json!({"error": e.to_string()})))
        })?;

    println!("Retrieved integration ID: {}", integration_id);

    composio_client
        .get_integration(&integration_id)
        .await
        .map(|integration| {
            tracing::debug!("Successfully retrieved integration details");
            Json(integration)
        })
        .map_err(|e| {
            println!("Failed to get integration details: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
        })
}


// Handler for initiating connection
use tracing::{info, error, warn};

/*
pub async fn initiate_connection_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<ConnectionInitRequest>,
) -> Result<Json<ConnectionRequest>, (StatusCode, Json<serde_json::Value>)> {
    info!(
        user_id = request.user_id,
        integration = %request.integration_name,
        "Initiating calendar connection"
    );
    
    // Extract and validate token
    let auth_header = headers.get("Authorization")
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "));

    let token = match auth_header {
        Some(token) => token,
        None => {
            error!("Authentication failed: No authorization token provided");
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "No authorization token provided"}))
            ));
        }
    };

    // Decode JWT token
    let claims = match decode::<Claims>(
        token,
        &DecodingKey::from_secret(std::env::var("JWT_SECRET_KEY")
            .expect("JWT_SECRET_KEY must be set in environment")
            .as_bytes()),
        &Validation::new(Algorithm::HS256)
    ) {
        Ok(token_data) => token_data.claims,
        Err(e) => {
            error!(error = ?e, "Authentication failed: Invalid JWT token");
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Invalid token"}))
            ));
        }
    };

    // Verify user exists and matches the request
    if claims.sub != request.user_id {
        error!(
            token_user_id = claims.sub,
            request_user_id = request.user_id,
            "Authorization failed: User ID mismatch"
        );
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "You can only create connections for your own account"}))
        ));
    }

    let composio_client = ComposioClient::new()
        .map_err(|e| {
            error!(error = %e, "Failed to initialize Composio client");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
        })?;

    let integration_id = composio_client
        .get_integration_id(&request.integration_name)
        .map_err(|e| {
            error!(
                error = %e,
                integration_name = %request.integration_name,
                "Failed to get integration ID"
            );
            (StatusCode::BAD_REQUEST, Json(json!({"error": e.to_string()})))
        })?;

    info!(
        integration_id = %integration_id,
        integration_name = %request.integration_name,
        "Successfully retrieved integration ID"
    );

    let connection_request = composio_client
        .initiate_connection(&integration_id, request.data.unwrap_or(serde_json::json!({})))
        .await
        .map_err(|e| {
            error!(
                error = %e,
                integration_id = %integration_id,
                "Failed to initiate Composio connection"
            );
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()})))
        })?;

    println!("Successfully initiated connection: {:?}", connection_request);

    // Store the calendar connection in the database using the repository
    use crate::models::user_models::NewComposioConnection;

    // First verify that the user exists
    if state.user_core.find_by_id(request.user_id).map_err(|e| {
        error!(
            error = %e,
            user_id = request.user_id,
            "Failed to verify user existence"
        );
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Database error while verifying user"}))
        )
    })?.is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "User not found"}))
        ));
    }
    

}
*/


// Handler for the OAuth callback
pub async fn oauth_callback_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    // Get the frontend URL from environment variable
    let frontend_url = env::var("FRONTEND_URL").expect("FRONTEND_URL not set");
    
    // Add success/error parameters to the URL based on the params
    let status = if params.contains_key("error") {
        "error"
    } else {
        "success"
    };
    
    let redirect_url = format!("{}/calendar-connected?status={}", frontend_url, status);
    
    Ok(Redirect::to(&redirect_url))
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
