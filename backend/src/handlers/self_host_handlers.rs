use crate::AppState;
use std::sync::Arc;
use crate::handlers::auth_dtos::NewUser;
use crate::handlers::auth_middleware::AuthUser;
use axum::{
    Json,
    extract::State,
    response::Response,
    http::StatusCode,
};

use rand::Rng;
use serde_json::json;
use jsonwebtoken::{encode, Header, EncodingKey};
use chrono::{Duration, Utc};
use std::num::NonZeroU32;
use governor::{Quota, RateLimiter};
use rand::distributions::Alphanumeric;
use std::env;
use serde::{Deserialize, Serialize};
use crate::handlers::auth_handlers::generate_tokens_and_response;

#[derive(Deserialize)]
pub struct SelfHostPingRequest {
    instance_id: String,
}

#[derive(Serialize, Deserialize)]
pub struct PairingVerificationRequest {
    pairing_code: String,
    server_instance_id: String,
}

#[derive(Deserialize, Serialize)]
pub struct PairingVerificationResponse {
    valid: bool,
    number: String,
    message: String,
}

#[derive(Deserialize)]
pub struct SelfHostedSignupRequest {
    pairing_code: String,
    password: Option<String>,
}

#[derive(Deserialize)]
pub struct SelfHostedLoginRequest {
    password: String,
}

#[derive(Serialize)]
pub struct GeneratePairingCodeResponse {
    pairing_code: String,
}

#[derive(Serialize)]
pub struct SelfHostedStatusResponse {
    status: String,
}


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
    match state.user_core.update_preferred_number(&req.twilio_phone) {
        Ok(_) => {
            tracing::debug!("Successfully updated Twilio phone for user: {}", auth_user.user_id);

            if let Ok((account_sid, auth_token, _)) = state.user_core.get_twilio_credentials() {
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
    let user_opt = match state.user_core.get_user() {
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

    match state.user_core.update_twilio_credentials(&req.account_sid, &req.auth_token) {
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

use roxmltree::{Document, Node};
use reqwest;
use tracing;

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
    State(state): State<Arc<AppState>>,
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


use reqwest::Client;
use crate::handlers::auth_dtos::LoginRequest;
// Add this struct for the check-creds response
#[derive(Deserialize, Serialize)]
struct CheckCredsResponse {
    user_id: String,
    phone_number: String,
    preferred_number: String,
    phone_number_country: String,
    messaging_service_sid: Option<String>,
    twilio_account_sid: Option<String>,
    twilio_auth_token: Option<String>,
}

use axum::response::IntoResponse;
pub async fn self_hosted_login(
    State(state): State<Arc<AppState>>,
    Json(login_req): Json<LoginRequest>,
) -> Result<Response, (axum::http::StatusCode, Json<serde_json::Value>)> {
    println!("Self-hosted login attempt for email: {}", login_req.email); // Debug log
    // Define rate limit: 5 attempts per minute
    let quota = Quota::per_minute(NonZeroU32::new(5).unwrap());
    let limiter_key = login_req.email.clone(); // Use email as the key
    // Get or create a keyed rate limiter for this email
    let entry = state.login_limiter
        .entry(limiter_key.clone())
        .or_insert_with(|| RateLimiter::keyed(quota)); // Bind the Entry here
    let limiter = entry.value(); // Now borrow from the bound value
    // Check if rate limit is exceeded
    if limiter.check_key(&limiter_key).is_err() {
        println!("Rate limit exceeded for email: [redacted]");
        return Err((
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error": "Too many login attempts, try again later"})),
        ));
    }
    // Verify credentials against main server
    let client = Client::new();
    let check_resp = client
        .post("https://lightfriend.ai/api/profile/check-creds")
        .json(&login_req)
        .send()
        .await
        .map_err(|_| (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Verification service unavailable"}))
        ))?;
    if !check_resp.status().is_success() {
        println!("Credential check failed for email: [redacted]");
        return Err((
            axum::http::StatusCode::UNAUTHORIZED,
            Json(json!({"error": "Invalid credentials"}))
        ));
    }
    let check_data: CheckCredsResponse = check_resp
        .json()
        .await
        .map_err(|_| (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to process verification response"}))
        ))?;
    let main_phone_number = check_data.phone_number;
    println!("Credentials verified successfully for main user_id: {}", check_data.user_id);
    let user = match state.user_core.get_user() {
        Ok(Some(mut u)) => u,
        Ok(None) => {
            // Create new user
            let new_user = NewUser {
                phone_number: main_phone_number.clone(),
                credits: 0.00,
                credits_left: 0.00,
            };
            state.user_core.create_user(new_user).map_err(|e| {
                println!("User creation failed: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "User creation failed"})),
                )
            })?;
            println!("New self-hosted user created successfully");
            // Get the newly created user
            state.user_core.get_user()
                .map_err(|e| (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": format!("Failed to retrieve user: {}", e)}))
                ))?
                .ok_or_else(|| (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "User not found after creation"}))
                ))?
        }
        Err(e) => {
            println!("Database error while checking for user id 1: {}", e);
            return Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Database error"}))
            ));
        }
    };

    // Set phone country
    if let Err(e) = crate::handlers::profile_handlers::set_user_phone_country(&state, &user.phone_number).await {
        tracing::error!("Failed to set phone country during self-hosted login: {}", e);
        // Proceed anyway
    }
    generate_tokens_and_response(user.id)
}


fn generate_api_key(length: usize) -> String {
    rand::thread_rng()
        .sample_iter(&rand::distributions::Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}
