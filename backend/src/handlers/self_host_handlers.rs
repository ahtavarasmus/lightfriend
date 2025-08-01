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
use std::time::{SystemTime, UNIX_EPOCH};
use crate::api::twilio_sms;
use serde_json::json;
use jsonwebtoken::{encode, Header, EncodingKey};
use chrono::{Duration, Utc};
use std::num::NonZeroU32;
use governor::{Quota, RateLimiter};
use rand::distributions::Alphanumeric;
use uuid::Uuid;
use std::env;
use serde::{Deserialize, Serialize};

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


pub async fn self_hosted_status(
    State(state): State<Arc<AppState>>,
) -> Result<Json<SelfHostedStatusResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Check if environment is self-hosted
    let env_type = std::env::var("ENVIRONMENT").unwrap_or_else(|_| "normal".to_string());
    
    if env_type == "self_hosted" {
        // Check if there are any users in the database
        match state.user_core.get_all_users() {
            Ok(users) => {
                let status = if users.is_empty() {
                    "self-hosted-signup"
                } else {
                    "self-hosted-login"
                };
                println!("status: {}", status);
                Ok(Json(SelfHostedStatusResponse { status: status.to_string() }))
            }
            Err(e) => {
                println!("Database error while checking users: {}", e);
                Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Database error"}))
                ))
            }
        }
    } else {
        println!("status: normal");
        Ok(Json(SelfHostedStatusResponse {
            status: "normal".to_string()
        }))
    }
}

pub async fn self_hosted_signup(
    State(state): State<Arc<AppState>>,
    Json(signup_req): Json<SelfHostedSignupRequest>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    // Verify we're in self-hosted mode
    let env_type = std::env::var("ENVIRONMENT").unwrap_or_else(|_| "normal".to_string());
    if env_type != "self_hosted" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Self-hosted signup is only available in self-hosted mode"}))
        ));
    }

    // Check if any users exist
    let users = state.user_core.get_all_users().map_err(|e| {
        println!("Database error while checking users: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Database error"}))
        )
    })?;

    if !users.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Self-hosted instance is already set up"}))
        ));
    }

    // If password is provided, this is the second step (password creation)
    if let Some(password) = signup_req.password {

        // Hash the password
        let password_hash = bcrypt::hash(&password, bcrypt::DEFAULT_COST).map_err(|e| {
            println!("Password hashing failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Password hashing failed"}))
            )
        })?;

        let user = state.user_core.find_by_email("admin@local")
            .map_err(|e| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to retrieve user"}))
            ))?
            .ok_or_else(|| (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "User not found after creation"}))
            ))?;

        if user.password_hash == "".to_string() {
            // Update the password since it's empty
            state.user_core.update_password(user.email.as_str(), password_hash.as_str()).map_err(|e| {
                println!("Failed to set password for the self hosted: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Failed to set password for the self hosted instance"}))
                )
            })?;
        } else {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Password can only be set once"}))
            ));
        }

        // Generate tokens
        let access_token = encode(
            &Header::default(),
            &json!({
                "sub": user.id,
                "exp": (Utc::now() + Duration::minutes(15)).timestamp(),
                "type": "access"
            }),
            &EncodingKey::from_secret(std::env::var("JWT_SECRET_KEY")
                .expect("JWT_SECRET_KEY must be set in environment")
                .as_bytes()),
        ).map_err(|_| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Token generation failed"}))
        ))?;

        let refresh_token = encode(
            &Header::default(),
            &json!({
                "sub": user.id,
                "exp": (Utc::now() + Duration::days(7)).timestamp(),
                "type": "refresh"
            }),
            &EncodingKey::from_secret(std::env::var("JWT_REFRESH_KEY")
                .expect("JWT_REFRESH_KEY must be set in environment")
                .as_bytes()),
        ).map_err(|_| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Token generation failed"}))
        ))?;

        // Create response with tokens
        let mut response = Response::new(
            axum::body::Body::from(
                Json(json!({
                    "message": "Self-hosted setup complete",
                    "token": access_token
                })).to_string()
            )
        );

        let cookie_options = "; HttpOnly; Secure; SameSite=Strict; Path=/";
        response.headers_mut().insert(
            "Set-Cookie",
            format!("access_token={}{}; Max-Age=900", access_token, cookie_options)
                .parse()
                .unwrap(),
        );

        response.headers_mut().insert(
            "Set-Cookie",
            format!("refresh_token={}{}; Max-Age=604800", refresh_token, cookie_options)
                .parse()
                .unwrap(),
        );

        response.headers_mut().insert(
            "Content-Type",
            "application/json".parse().unwrap()
        );

        Ok(response)
    } else {
        // This is the first step (pairing code verification)
        // Generate a server instance ID
        let server_instance_id = Uuid::new_v4().to_string();

        // Send verification request to main server
        let server_url = env::var("SERVER_URL").expect("SERVER_URL must be set");
        let client = reqwest::Client::new();
        let verification_response = client
            .post(&format!("{}/api/check-pairing", server_url))
            .json(&PairingVerificationRequest {
                pairing_code: signup_req.pairing_code.clone(),
                server_instance_id: server_instance_id.clone(),
            })
            .send()
            .await
            .map_err(|e| {
                println!("Failed to send verification request: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Failed to verify pairing code"}))
                )
            })?;

        if !verification_response.status().is_success() {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": "Invalid pairing code"}))
            ));
        }

        let verification_result = verification_response.json::<PairingVerificationResponse>().await
            .map_err(|e| {
                println!("Failed to parse verification response: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({"error": "Failed to verify pairing code"}))
                )
            })?;

        if !verification_result.valid {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": verification_result.message}))
            ));
        }

        // Create the admin user
        let new_user = NewUser {
            email: "admin@local".to_string(),
            password_hash: "".to_string(),
            phone_number: verification_result.number,
            time_to_live: 0, 
            verified: true,
            credits: 0.00,
            credits_left: 0.00,
            charge_when_under: false,
            waiting_checks_count: 0,
            discount: false,
            sub_tier: Some("self-hosted".to_string()),
        };

        state.user_core.create_user(new_user).map_err(|e| {
            println!("User creation failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to create admin user"}))
            )
        })?;

        state.user_core.update_instance_id_to_self_hosted(server_instance_id.as_str()).map_err(|e| {
            println!("Failed to set profile for the self hosted: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to set server instance ID"}))
            )
        })?;

        Ok(Response::new(
            axum::body::Body::from(
                Json(json!({
                    "message": "Pairing code verified. Please create a password."
                })).to_string()
            )
        ))
    }
}

pub async fn check_pairing(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PairingVerificationRequest>,
) -> Result<Json<PairingVerificationResponse>, (StatusCode, Json<serde_json::Value>)> {
    println!("Attempting to verify pairing code for server instance: {}", req.server_instance_id);

    // Verify the pairing code and update server instance ID
    match state.user_core.verify_pairing_code(&req.pairing_code, &req.server_instance_id) {
        Ok((true, number)) => {
            println!("Pairing code verified successfully for server instance: {}", req.server_instance_id);
            Ok(Json(PairingVerificationResponse {
                valid: true,
                number: number.unwrap_or("".to_string()),
                message: "Pairing code verified successfully".to_string(),
            }))
        },
        Ok((false, number)) => {
            println!("Invalid pairing code provided for server instance: {}", req.server_instance_id);
            Ok(Json(PairingVerificationResponse {
                valid: false,
                number: number.unwrap_or("".to_string()),
                message: "Invalid pairing code".to_string(),
            }))
        },
        Err(e) => {
            println!("Database error while verifying pairing code: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Failed to verify pairing code due to database error"}))
            ))
        }
    }
}

// generates a new pairing code for the user. normally this is done on stripe subscription webhook.
pub async fn generate_pairing_code(
    State(state): State<Arc<AppState>>,
    auth_user: AuthUser, 
) -> Result<Json<GeneratePairingCodeResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Generate a random 6-character pairing code
    let pairing_code: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(6)
        .map(char::from)
        .collect::<String>()
        .to_uppercase();

    // Save the new pairing code 
    if let Err(e) = state.user_core.set_server_instance_id(auth_user.user_id, &pairing_code.as_str()) {
        println!("Failed to save pairing code: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Failed to generate pairing code"}))
        ));
    }

    Ok(Json(GeneratePairingCodeResponse {
        pairing_code
    }))
}

pub async fn self_host_ping(
    State(state): State<Arc<AppState>>,
    Json(ping_req): Json<SelfHostPingRequest>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    tracing::debug!("Received self-host ping from instance: {}", ping_req.instance_id);

    // Find user with this instance ID
    match state.user_core.find_user_by_pairing_code(&ping_req.instance_id) {
        Ok(Some(user_id)) => {
            tracing::debug!("Self-host ping successful for instance: {}", ping_req.instance_id);
            Ok(StatusCode::OK)
        },
        Ok(None) => {
            tracing::error!("Invalid instance ID received in ping: {}", ping_req.instance_id);
            Err((
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Invalid instance ID"}))
            ))
        },
        Err(e) => {
            tracing::error!("Database error while processing ping: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Database error"}))
            ))
        }
    }
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
            match setup_subdomain(State(state.clone()), auth_user.clone(), Json(setup_req)).await {
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

pub async fn self_hosted_login(
    State(state): State<Arc<AppState>>,
    Json(login_req): Json<SelfHostedLoginRequest>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    // Verify we're in self-hosted mode
    let env_type = std::env::var("ENVIRONMENT").unwrap_or_else(|_| "normal".to_string());
    if env_type != "self_hosted" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Self-hosted login is only available in self-hosted mode"}))
        ));
    }

    // Find the admin user
    let user = match state.user_core.find_by_email("admin@local") {
        Ok(Some(user)) => user,
        Ok(None) => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "Self-hosted instance not set up"}))
            ));
        }
        Err(_) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Database error"}))
            ));
        }
    };

    // Verify password
    match bcrypt::verify(&login_req.password, &user.password_hash) {
        Ok(valid) => {
            if !valid {
                return Err((
                    StatusCode::UNAUTHORIZED,
                    Json(json!({"error": "Invalid password"}))
                ));
            }
        }
        Err(_) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Password verification failed"}))
            ));
        }
    }

    // Generate tokens
    let access_token = encode(
        &Header::default(),
        &json!({
            "sub": user.id,
            "exp": (Utc::now() + Duration::minutes(15)).timestamp(),
            "type": "access"
        }),
        &EncodingKey::from_secret(std::env::var("JWT_SECRET_KEY")
            .expect("JWT_SECRET_KEY must be set in environment")
            .as_bytes()),
    ).map_err(|_| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": "Token generation failed"}))
    ))?;

    let refresh_token = encode(
        &Header::default(),
        &json!({
            "sub": user.id,
            "exp": (Utc::now() + Duration::days(7)).timestamp(),
            "type": "refresh"
        }),
        &EncodingKey::from_secret(std::env::var("JWT_REFRESH_KEY")
            .expect("JWT_REFRESH_KEY must be set in environment")
            .as_bytes()),
    ).map_err(|_| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": "Token generation failed"}))
    ))?;

    // Create response with tokens
    let mut response = Response::new(
        axum::body::Body::from(
            Json(json!({
                "message": "Login successful",
                "token": access_token
            })).to_string()
        )
    );

    let cookie_options = "; HttpOnly; Secure; SameSite=Strict; Path=/";
    response.headers_mut().insert(
        "Set-Cookie",
        format!("access_token={}{}; Max-Age=900", access_token, cookie_options)
            .parse()
            .unwrap(),
    );

    response.headers_mut().insert(
        "Set-Cookie",
        format!("refresh_token={}{}; Max-Age=604800", refresh_token, cookie_options)
            .parse()
            .unwrap(),
    );

    response.headers_mut().insert(
        "Content-Type",
        "application/json".parse().unwrap()
    );

    Ok(response)
}
