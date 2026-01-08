use crate::AppState;
use std::sync::Arc;
use crate::handlers::auth_middleware::AuthUser;
use axum::{
    Json,
    extract::{State, Query},
    http::StatusCode,
};

use serde_json::json;
use std::env;
use serde::{Deserialize, Serialize};


#[derive(Deserialize)]
pub struct UpdateServerIpRequest {
    server_ip: String,
}

#[derive(Deserialize)]
pub struct UpdateTwilioPhoneRequest {
    twilio_phone: String,
}

#[derive(Deserialize)]
pub struct UpdateTwilioCredsRequest {
    account_sid: String,
    auth_token: String,
}

#[derive(Deserialize)]
pub struct UpdateTextBeeCredsRequest {
    textbee_api_key: String,
    textbee_device_id: String,
}

pub async fn update_twilio_phone(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(req): Json<UpdateTwilioPhoneRequest>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    match state.user_core.update_preferred_number(auth_user.user_id, &req.twilio_phone) {
        Ok(_) => {
            tracing::debug!("Successfully updated Twilio phone for user: {}", auth_user.user_id);

            if let Ok((account_sid, auth_token)) = state.user_core.get_twilio_credentials(auth_user.user_id) {
                let phone = req.twilio_phone.clone();
                let user_id = auth_user.user_id;
                let state_clone = state.clone();
                tokio::spawn(async move {
                    if let Err(e) = crate::api::twilio_utils::set_twilio_webhook(&account_sid, &auth_token, &phone, user_id, state_clone).await {
                        tracing::error!("Failed to set Twilio webhook for phone {}: {}", phone, e);
                        // Proceed anyway(probably user hasn't given their twilio credentials yet, we will try again when they do)
                    } else {
                        tracing::debug!("Successfully set Twilio webhook for phone: {}", phone);
                    }
                });
            } else {
                tracing::warn!("Twilio credentials not found for user {}, skipping webhook update", auth_user.user_id);
            }

            Ok(StatusCode::OK)
        },
        Err(e) => {
            tracing::error!("Failed to update Twilio phone: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to update Twilio phone"}))
            ))
        }
    }
}

pub async fn update_twilio_creds(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(req): Json<UpdateTwilioCredsRequest>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    let user_opt = match state.user_core.find_by_id(auth_user.user_id) {
        Ok(opt) => opt,
        Err(e) => {
            tracing::error!("Failed to fetch user: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to fetch user"}))
            ));
        }
    };

    let user = match user_opt {
        Some(u) => u,
        None => {
            tracing::error!("User not found: {}", auth_user.user_id);
            return Err((
                StatusCode::NOT_FOUND,
                Json(json!({"error": "User not found"}))
            ));
        }
    };

    match state.user_core.update_twilio_credentials(auth_user.user_id, &req.account_sid, &req.auth_token) {
        Ok(_) => {
            tracing::debug!("Successfully updated Twilio credentials for user: {}", auth_user.user_id);

            if let Some(phone) = user.preferred_number {
                let account_sid = req.account_sid.clone();
                let auth_token = req.auth_token.clone();
                let phone = phone.clone();
                let user_id = auth_user.user_id;
                let state_clone = state.clone();
                tokio::spawn(async move {
                    if let Err(e) = crate::api::twilio_utils::set_twilio_webhook(&account_sid, &auth_token, &phone, user_id, state_clone).await {
                        tracing::error!("Failed to set Twilio webhook for phone {}: {}", phone, e);
                        // Proceed anyway(probably user hasn't inputted their twilio number yet, we try again when they do)
                    } else {
                        tracing::debug!("Successfully set Twilio webhook for phone: {}", phone);
                    }
                });
            }

            Ok(StatusCode::OK)
        },
        Err(e) => {
            tracing::error!("Failed to update Twilio credentials: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to update Twilio credentials"}))
            ))
        }
    }
}

/// Clear BYOT Twilio credentials (manual removal)
pub async fn clear_twilio_creds(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    match state.user_core.clear_twilio_credentials(auth_user.user_id) {
        Ok(_) => {
            tracing::info!("Successfully cleared BYOT credentials for user: {}", auth_user.user_id);
            Ok(StatusCode::OK)
        },
        Err(e) => {
            tracing::error!("Failed to clear BYOT credentials: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to clear BYOT credentials"}))
            ))
        }
    }
}

pub async fn update_textbee_creds(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(req): Json<UpdateTextBeeCredsRequest>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    match state.user_core.update_textbee_credentials(auth_user.user_id, &req.textbee_device_id, &req.textbee_api_key) {
        Ok(_) => {
            println!("Successfully updated TextBee credentials for user: {}", auth_user.user_id);
            Ok(StatusCode::OK)
        },
        Err(e) => {
            tracing::error!("Failed to update TextBee credentials: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to update TextBee credentials"}))
            ))
        }
    }
}

use roxmltree::Document;
use reqwest;

#[derive(serde::Deserialize)]
pub struct SetupSubdomainRequest {
    pub ip_address: String,
}

#[derive(serde::Serialize)]
pub struct SetupSubdomainResponse {
    pub subdomain: String,
    pub status: String,
}

#[derive(Debug, Clone)]
struct DnsHost {
    name: String,
    record_type: String,
    address: String,
    mx_pref: Option<u32>,
    ttl: u32,
}

pub async fn setup_subdomain(
    State(_state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(req): Json<SetupSubdomainRequest>,
) -> Result<Json<SetupSubdomainResponse>, (StatusCode, Json<serde_json::Value>)> {
    tracing::info!("Entering setup_subdomain for user_id: {}", auth_user.user_id);
    tracing::info!("Requested IP address: {}", req.ip_address);

    let api_user = env::var("NAMECHEAP_API_USER").expect("NAMECHEAP_API_USER must be set");
    let api_key = env::var("NAMECHEAP_API_KEY").expect("NAMECHEAP_API_KEY must be set");
    let client_ip = env::var("NAMECHEAP_CLIENT_IP").expect("NAMECHEAP_CLIENT_IP must be set");
    let is_sandbox = env::var("NAMECHEAP_SANDBOX").unwrap_or("true".to_string()) == "true";

    tracing::info!("Loaded environment variables: api_user={}, is_sandbox={}", api_user, is_sandbox);

    let base_url = if is_sandbox {
        "https://api.sandbox.namecheap.com/xml.response"
    } else {
        "https://api.namecheap.com/xml.response"
    };

    let sld = "lightfriend";
    let tld = "ai";
    let subdomain_name = auth_user.user_id.to_string();
    let subdomain = format!("my.{}.lightfriend.ai", subdomain_name);
    let target_ip = req.ip_address.clone();

    tracing::info!("Constructed subdomain: {}", subdomain);
    tracing::info!("Target IP: {}", target_ip);

    let client = reqwest::Client::new();

    // Helper function to make API request and return XML string if successful
    async fn make_api_request(client: &reqwest::Client, url: &str) -> Result<String, (StatusCode, Json<serde_json::Value>)> {
        tracing::info!("Making API request to URL: {}", url);

        let response = client.get(url).send().await.map_err(|e| {
            tracing::error!("Failed to make Namecheap API request: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to make API request"})))
        })?;

        tracing::info!("API response status: {}", response.status());

        if !response.status().is_success() {
            tracing::error!("Namecheap API request failed with status: {}", response.status());
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "API request failed"}))));
        }

        let text = response.text().await.map_err(|e| {
            tracing::error!("Failed to read API response: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to read API response"})))
        })?;

        tracing::info!("Received API response text (length: {})", text.len());

        let doc = Document::parse(&text).map_err(|e| {
            tracing::error!("Failed to parse XML: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to parse API response"})))
        })?;

        let status = doc.root().descendants().find(|n| n.has_tag_name("ApiResponse")).and_then(|n| n.attribute("Status"));
        if status != Some("OK") {
            let error_msg = doc.root().descendants().find(|n| n.has_tag_name("Error")).map(|n| n.text().unwrap_or("Unknown error")).unwrap_or("Unknown error");
            tracing::error!("API response error: {}", error_msg);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": error_msg}))));
        }

        tracing::info!("API request successful");

        Ok(text)
    }

    // Step 1: Check if using our DNS with getList
    tracing::info!("Step 1: Checking if using our DNS");

    let get_list_url = format!(
        "{}?ApiUser={}&ApiKey={}&UserName={}&Command=namecheap.domains.dns.getList&ClientIp={}&SLD={}&TLD={}",
        base_url, api_user, api_key, api_user, client_ip, sld, tld
    );

    let xml = make_api_request(&client, &get_list_url).await?;
    let doc = Document::parse(&xml).unwrap();

    let is_using_our_dns = doc.descendants()
        .find(|n| n.has_tag_name("DomainDNSGetListResult"))
        .and_then(|n| n.attribute("IsUsingOurDNS"))
        .map(|v| v == "true")
        .unwrap_or(false);

    tracing::info!("Is using our DNS: {}", is_using_our_dns);

    if !is_using_our_dns {
        tracing::info!("Setting to default DNS");

        // Set to default DNS
        let set_default_url = format!(
            "{}?ApiUser={}&ApiKey={}&UserName={}&Command=namecheap.domains.dns.setDefault&ClientIp={}&SLD={}&TLD={}",
            base_url, api_user, api_key, api_user, client_ip, sld, tld
        );

        let xml = make_api_request(&client, &set_default_url).await?;
        let doc = Document::parse(&xml).unwrap();

        let updated = doc.descendants()
            .find(|n| n.has_tag_name("DomainDNSSetDefaultResult"))
            .and_then(|n| n.attribute("Updated"))
            .map(|v| v == "true")
            .unwrap_or(false);

        tracing::info!("Default DNS set updated: {}", updated);

        if !updated {
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to set default DNS"}))));
        }
    }

    // Step 2: Get current hosts
    tracing::info!("Step 2: Getting current hosts");

    let get_hosts_url = format!(
        "{}?ApiUser={}&ApiKey={}&UserName={}&Command=namecheap.domains.dns.getHosts&ClientIp={}&SLD={}&TLD={}",
        base_url, api_user, api_key, api_user, client_ip, sld, tld
    );

    let xml = make_api_request(&client, &get_hosts_url).await?;
    let doc = Document::parse(&xml).unwrap();

    let hosts_result = doc.descendants().find(|n| n.has_tag_name("DomainDNSGetHostsResult")).ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": "Missing DomainDNSGetHostsResult in response"}))
    ))?;

    let mut hosts: Vec<DnsHost> = vec![];

    for host_node in hosts_result.children().filter(|n| n.has_tag_name("host")) {
        let name = host_node.attribute("Name").unwrap_or("").to_string();
        let record_type = host_node.attribute("Type").unwrap_or("").to_string();
        let address = host_node.attribute("Address").unwrap_or("").to_string();
        let mx_pref = host_node.attribute("MXPref").and_then(|s| s.parse::<u32>().ok());
        let ttl = host_node.attribute("TTL").and_then(|s| s.parse::<u32>().ok()).unwrap_or(1800);

        hosts.push(DnsHost {
            name,
            record_type,
            address,
            mx_pref,
            ttl,
        });
    }

    tracing::info!("Retrieved {} hosts", hosts.len());

    // Step 3: Check if subdomain exists and update or add
    tracing::info!("Step 3: Checking for subdomain: {}", subdomain_name);

    let mut found = false;
    for host in hosts.iter_mut() {
        if host.name == subdomain_name && host.record_type == "A" {
            found = true;
            tracing::info!("Subdomain found, current address: {}", host.address);
            if host.address == target_ip {
                // Already set to the same IP
                tracing::info!("Subdomain already set to target IP");
                return Ok(Json(SetupSubdomainResponse {
                    subdomain,
                    status: "success".to_string(),
                }));
            } else {
                // Override with new IP
                tracing::info!("Updating subdomain address to: {}", target_ip);
                host.address = target_ip.clone();
            }
            break; // Assuming only one A record per hostname
        }
    }

    if !found {
        tracing::info!("Subdomain not found, adding new A record with IP: {}", target_ip);
        // Add new A record
        hosts.push(DnsHost {
            name: subdomain_name,
            record_type: "A".to_string(),
            address: target_ip,
            mx_pref: Some(10),
            ttl: 1800,
        });
    }

    // Step 4: Set the updated hosts
    tracing::info!("Step 4: Setting updated hosts, total: {}", hosts.len());

    let mut set_hosts_params = format!(
        "{}?ApiUser={}&ApiKey={}&UserName={}&Command=namecheap.domains.dns.setHosts&ClientIp={}&SLD={}&TLD={}",
        base_url, api_user, api_key, api_user, client_ip, sld, tld
    );

    for (i, host) in hosts.iter().enumerate() {
        let idx = i + 1;
        set_hosts_params.push_str(&format!("&HostName{}={}", idx, host.name));
        set_hosts_params.push_str(&format!("&RecordType{}={}", idx, host.record_type));
        set_hosts_params.push_str(&format!("&Address{}={}", idx, host.address));
        set_hosts_params.push_str(&format!("&TTL{}={}", idx, host.ttl));
        if let Some(mx_pref) = host.mx_pref {
            if host.record_type == "MX" {
                set_hosts_params.push_str(&format!("&MXPref{}={}", idx, mx_pref));
            }
        }
    }

    let xml = make_api_request(&client, &set_hosts_params).await?;
    let doc = Document::parse(&xml).unwrap();

    let is_success = doc.descendants()
        .find(|n| n.has_tag_name("DomainDNSSetHostsResult"))
        .and_then(|n| n.attribute("IsSuccess"))
        .map(|v| v == "true")
        .unwrap_or(false);

    tracing::info!("Set hosts success: {}", is_success);

    if !is_success {
        return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to set hosts"}))));
    }

    tracing::info!("Subdomain setup successful");

    Ok(Json(SetupSubdomainResponse {
        subdomain,
        status: "success".to_string(),
    }))
}


use axum::{response::Json as AxumJson};
use tracing;

#[derive(Deserialize)]
pub struct TokenRequest {
    pub token: String,
}

#[derive(Serialize)]
pub struct VerifyTokenResponse {
    pub user_id: String,
    pub phone_number: String,
    pub preferred_number: String,
    pub phone_number_country: String,
    pub messaging_service_sid: Option<String>, 
    pub twilio_account_sid: Option<String>,
    pub twilio_auth_token: Option<String>,
    pub server_url: Option<String>,
    pub tinfoil_api_key: Option<String>,
}

use crate::handlers::auth_middleware::Tier3SelfHostedUser;

pub async fn verify_token(
    State(state): State<Arc<AppState>>,
    tier3_user: Tier3SelfHostedUser,
    Json(token_req): Json<TokenRequest>,
) -> Result<AxumJson<VerifyTokenResponse>, (StatusCode, AxumJson<serde_json::Value>)> {
    let user = state.user_core.find_by_id(tier3_user.user_id)
        .map_err(|_| (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(serde_json::json!({"error": "Failed to fetch user"}))
        ))?
        .ok_or((
            StatusCode::NOT_FOUND,
            AxumJson(serde_json::json!({"error": "User not found"}))
        ))?;

    // Verify and invalidate token
    let settings = match state.user_core.verify_and_invalidate_magic_login_token(tier3_user.user_id, &token_req.token) {
        Ok(s) => s,
        Err(_) => {
            return Err((
                StatusCode::UNAUTHORIZED,
                AxumJson(serde_json::json!({"error": "Invalid or expired token"}))
            ));
        }
    };

    // TODO: Get decrypted Twilio creds

    let response = VerifyTokenResponse {
        user_id: tier3_user.user_id.to_string(),
        phone_number: user.phone_number,
        preferred_number: user.preferred_number.unwrap_or_default(),
        phone_number_country: user.phone_number_country.unwrap_or_default(),
        messaging_service_sid: None, // Or from settings if added
        twilio_account_sid: None,
        twilio_auth_token: None,
        server_url: settings.server_url,
        tinfoil_api_key: None,
    };

    tracing::info!("Magic token verified and invalidated for user {}", tier3_user.user_id);
    Ok(AxumJson(response))
}


// Query param for regenerate
#[derive(Deserialize)]
pub struct MagicLinkQuery {
    regenerate: Option<bool>,
}

#[derive(Serialize)]
pub struct MagicLinkResponse {
    link: String,
}

pub async fn get_magic_link(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Query(query): Query<MagicLinkQuery>,
) -> Result<AxumJson<MagicLinkResponse>, (StatusCode, AxumJson<serde_json::Value>)> {
    let user = state.user_core
        .find_by_id(auth_user.user_id)
        .map_err(|_| (
            StatusCode::INTERNAL_SERVER_ERROR,
            AxumJson(json!({"error": "Failed to fetch user"}))
        ))?
        .ok_or((
            StatusCode::NOT_FOUND,
            AxumJson(json!({"error": "User not found"}))
        ))?;

    if user.sub_tier.as_deref() != Some("tier 3") {
        return Err((
            StatusCode::FORBIDDEN,
            AxumJson(json!({"error": "Magic link requires Tier 3 subscription"}))
        ));
    }

    // Get or generate token (forces regenerate if query.regenerate == Some(true))
    let force_regenerate = query.regenerate.unwrap_or(false);
    let token = if force_regenerate {
        state.user_core.generate_magic_login_token(auth_user.user_id).map_err(|e| {
            tracing::error!("Failed to generate magic token: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to generate magic link"}))
            )
        })?
    } else {
        state.user_core.get_or_generate_magic_login_token(auth_user.user_id).map_err(|e| {
            tracing::error!("Failed to get/generate magic token: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AxumJson(json!({"error": "Failed to get magic link"}))
            )
        })?
    };

    let mut link = format!("https://{}.lightfriend.ai/login?token={}", user.id, token);

    if std::env::var("ENVIRONMENT").expect("ENVIRONMENT not set") == "development" {
        link = format!("http://localhost:8090/login?token={}",token);
    }

    tracing::info!("Magic link fetched/generated for user {}: {}", auth_user.user_id, link);
    Ok(AxumJson(MagicLinkResponse { link }))
}

pub async fn update_server_ip(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser,
    Json(req): Json<UpdateServerIpRequest>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {

    // Fetch the user to check sub_tier
    let user = match state.user_core.find_by_id(auth_user.user_id) {
        Ok(Some(u)) => u,
        Ok(None) => return Err((StatusCode::NOT_FOUND, Json(json!({"error": "User not found"})))),
        Err(e) => {
            tracing::error!("Failed to fetch user: {}", e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"}))));
        }
    };

    // Check if sub_tier is Some("tier 3")
    if user.sub_tier != Some("tier 3".to_string()) {
        return Err((StatusCode::FORBIDDEN, Json(json!({"error": "Requires self-hosted subscription"}))));
    }

    // Update the server_ip in user_settings
    match state.user_core.update_server_ip(auth_user.user_id, &req.server_ip) {
        Ok(_) => {
            tracing::debug!("Successfully updated server IP for user: {}", auth_user.user_id);

            // Call setup_subdomain to update DNS if necessary
            let setup_req = SetupSubdomainRequest {
                ip_address: req.server_ip.clone(),
            };
            match setup_subdomain(State(state.clone()), auth_user, Json(setup_req)).await {
                Ok(_) => Ok(StatusCode::OK),
                Err(e) => {
                    tracing::error!("Failed to setup subdomain: {:?}", e);
                    Err(e)
                }
            }
        },
        Err(e) => {
            tracing::error!("Failed to update server IP: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to update server IP"}))
            ))
        }
    }
}

use chrono::{DateTime, Utc};

pub async fn create_temp_tinfoil_api_key(
    user_id: String,
    end_timestamp: i64,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let admin_key = env::var("TINFOIL_ADMIN_API_KEY")
        .map_err(|_| "TINFOIL_ADMIN_API_KEY must be set")?;
    let client = reqwest::Client::new();

    let dt = DateTime::<Utc>::from_timestamp(end_timestamp, 0)
        .ok_or("Invalid timestamp")?;
    let expires_at = dt.to_rfc3339();

    let body = json!({
        "name": format!("Temporary API Key for User {}", user_id),
        "expires_at": expires_at,
        "max_tokens": 1000000,
        "metadata": {
            "user_id": user_id
        }
    });

    let response = client
        .post("https://api.tinfoil.sh/api/keys")
        .header("Authorization", format!("Bearer {}", admin_key))
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .json(&body)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("API request failed with status: {}", response.status()).into());
    }

    let json_resp: serde_json::Value = response.json().await?;
    let new_key = json_resp
        .get("key")
        .and_then(|k| k.as_str())
        .ok_or("Invalid response: missing 'key' field")?
        .to_string();

    tracing::info!("Created temporary Tinfoil API key for user {}: {}", user_id, new_key);
    Ok(new_key)
}

// Handler for checking tier 3 country capability
#[derive(Deserialize)]
pub struct CheckTier3AvailabilityQuery {
    pub country: String,
}

pub async fn check_tier3_availability(
    State(state): State<Arc<AppState>>,
    Query(query): Query<CheckTier3AvailabilityQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    use crate::api::twilio_availability;

    match twilio_availability::get_country_capability(&state, &query.country).await {
        Ok(capability) => {
            // All tier 3 plans are $40/month base
            let base_price = 40.0;

            // US/CA get unlimited (no credits needed), others get 0 credits (must buy separately)
            let credits_included = if capability.plan_type == "us_ca" {
                -1.0  // -1 indicates unlimited
            } else {
                0.0  // Must buy credits separately
            };

            Ok(Json(json!({
                "available": capability.available,
                "plan_type": capability.plan_type,
                "can_receive_sms": capability.can_receive_sms,
                "base_price": base_price,
                "credits_included": credits_included,
                "message_cost_eur": capability.outbound_sms_price,
                "outbound_sms_price": capability.outbound_sms_price,
                "inbound_sms_price": capability.inbound_sms_price,
                "outbound_voice_price_per_min": capability.outbound_voice_price_per_min,
                "inbound_voice_price_per_min": capability.inbound_voice_price_per_min,
            })))
        }
        Err(e) => Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": e})),
        )),
    }
}

#[derive(Deserialize)]
pub struct RenewTinfoilKeyRequest {
    pub user_id: i32,
    pub tokens_consumed: i64,
}

/// Endpoint for self-hosted instances to request Tinfoil API key renewal
/// This is called when the self-hosted instance detects it's running low on tokens
///
/// POST /api/self-hosted/renew-tinfoil-key
/// Body: { "user_id": 123, "tokens_consumed": 50000 }
///
/// Returns: { "new_api_key": "tk_...", "expiry": 1234567890 }
pub async fn renew_tinfoil_key(
    State(state): State<Arc<AppState>>,
    Json(request): Json<RenewTinfoilKeyRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let user_id = request.user_id;
    let tokens_consumed = request.tokens_consumed;

    tracing::info!("Tinfoil API key renewal requested for user {} (tokens consumed: {})", user_id, tokens_consumed);

    // Verify user exists and has tier 3 subscription
    let user = state.user_core.find_by_id(user_id)
        .map_err(|e| {
            tracing::error!("Database error finding user {}: {}", user_id, e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Database error"})))
        })?
        .ok_or_else(|| {
            tracing::warn!("User {} not found for Tinfoil renewal", user_id);
            (StatusCode::NOT_FOUND, Json(json!({"error": "User not found"})))
        })?;

    // Check if user has tier 3 subscription
    if user.sub_tier.as_deref() != Some("tier 3") {
        tracing::warn!("User {} attempted Tinfoil renewal but is tier {:?}", user_id, user.sub_tier);
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Tinfoil API key renewal is only available for tier 3 (self-hosted) users"}))
        ));
    }

    // Get next billing date for expiry calculation
    let next_billing = user.next_billing_date_timestamp
        .ok_or_else(|| {
            tracing::error!("User {} has no next_billing_date_timestamp set", user_id);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "No billing date set"})))
        })?;

    // Calculate days until renewal
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;
    let days_until_renewal = ((next_billing - now) / 86400).max(0);

    // Generate new Tinfoil API key using subaccount lifecycle utility
    let new_key = crate::utils::subaccount_lifecycle::regenerate_tinfoil_key(
        &state,
        user_id,
        next_billing as i64,
    ).await.map_err(|e| {
        tracing::error!("Failed to regenerate Tinfoil key for user {}: {}", user_id, e);
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": format!("Failed to generate new key: {}", e)})))
    })?;

    // Send email notification to admin (spawn as background task to not block response)
    let state_clone = state.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::utils::notification_utils::send_tinfoil_renewal_notification(
            &state_clone,
            user_id,
            days_until_renewal,
            tokens_consumed,
        ).await {
            tracing::error!("Failed to send Tinfoil renewal notification email: {}", e);
        }
    });

    tracing::info!("Successfully renewed Tinfoil API key for user {} (expires: {})", user_id, next_billing);

    Ok(Json(json!({
        "new_api_key": new_key,
        "expiry": next_billing,
        "message": "Tinfoil API key renewed successfully"
    })))
}
