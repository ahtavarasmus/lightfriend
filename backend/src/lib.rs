// Module declarations - moved from main.rs for library access
pub mod handlers {
    pub mod admin_handlers;
    pub mod admin_stats_handlers;
    pub mod auth_dtos;
    pub mod auth_handlers;
    pub mod auth_middleware;
    pub mod billing_handlers;
    pub mod bluesky;
    pub mod bridge_auth_common;
    pub mod contact_profile_handlers;
    pub mod filter_handlers;
    pub mod google_calendar;
    pub mod google_calendar_auth;
    pub mod google_maps;
    pub mod imap_auth;
    pub mod imap_handlers;
    pub mod instagram_auth;
    pub mod instagram_handlers;
    pub mod instagram_reels;
    pub mod messenger_auth;
    pub mod messenger_handlers;
    pub mod pricing_handlers;
    pub mod profile_handlers;
    pub mod reddit;
    pub mod refund_handlers;
    pub mod rumble;
    pub mod self_host_handlers;
    pub mod signal_auth;
    pub mod signal_handlers;
    pub mod spotify;
    pub mod streamable;
    pub mod stripe_handlers;
    pub mod telegram_auth;
    pub mod telegram_handlers;
    pub mod tesla_auth;
    pub mod tiktok;
    pub mod totp_handlers;
    pub mod twilio_handlers;
    pub mod twitter;
    pub mod uber_auth;
    pub mod webauthn_handlers;
    pub mod whatsapp_auth;
    pub mod whatsapp_handlers;
    pub mod youtube;
    pub mod youtube_auth;
}
pub mod utils {
    pub mod action_executor;
    pub mod bridge;
    pub mod country;
    pub mod elevenlabs_prompts;
    pub mod email;
    pub mod encryption;
    pub mod matrix_auth;
    pub mod notification_utils;
    pub mod tesla_keys;
    pub mod tool_exec;
    pub mod us_number_pool;
    pub mod usage;
    pub mod webauthn_config;
}
pub mod proactive {
    pub mod utils;
}
pub mod tool_call_utils {
    pub mod bridge;
    pub mod calendar;
    pub mod email;
    pub mod internet;
    pub mod management;
    pub mod tesla;
    pub mod utils;
}
pub mod api {
    pub mod elevenlabs;
    pub mod elevenlabs_webhook;
    pub mod tesla;
    pub mod twilio_availability;
    pub mod twilio_client;
    pub mod twilio_pricing;
    pub mod twilio_sms;
    pub mod twilio_utils;
}
pub mod error;
pub mod models {
    pub mod user_models;
}
pub mod repositories {
    pub mod connection_auth;
    #[cfg(test)]
    pub mod mock_signup_repository;
    pub mod mock_twilio_status_repository;
    pub mod signup_repository;
    pub mod signup_repository_impl;
    pub mod totp_repository;
    pub mod twilio_status_repository;
    pub mod twilio_status_repository_impl;
    pub mod user_core;
    pub mod user_repository;
    pub mod user_subscriptions;
    pub mod webauthn_repository;
}
pub mod services {
    pub mod country_service;
    pub mod signup_service;
    pub mod twilio_message_service;
    pub mod twilio_status_service;
}
pub mod schema;
pub mod jobs {
    pub mod scheduler;
}
pub mod ai_config;
pub use ai_config::{AiConfig, AiProvider, ModelPurpose};

// Test utilities for integration tests
pub mod test_utils;

// Re-export key types for external use
pub use repositories::totp_repository::TotpRepository;
pub use repositories::user_core::UserCore;
pub use repositories::user_repository::UserRepository;
pub use repositories::webauthn_repository::WebauthnRepository;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rand::Rng;
use reqwest::Client;
use serde_json::json;

// AppState and related types - needed by all handler modules
use dashmap::DashMap;
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use governor::{clock::DefaultClock, state::keyed::DefaultKeyedStateStore, RateLimiter};
use oauth2::{basic::BasicClient, EndpointNotSet, EndpointSet};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};
use tower_sessions::MemoryStore;

pub type DbPool = r2d2::Pool<ConnectionManager<SqliteConnection>>;

/// SQLite connection customizer that sets busy_timeout on each connection.
/// This makes SQLite wait up to 5 seconds for locks instead of failing immediately.
#[derive(Debug)]
pub struct SqliteConnectionCustomizer;

impl diesel::r2d2::CustomizeConnection<SqliteConnection, diesel::r2d2::Error>
    for SqliteConnectionCustomizer
{
    fn on_acquire(&self, conn: &mut SqliteConnection) -> Result<(), diesel::r2d2::Error> {
        diesel::sql_query("PRAGMA busy_timeout = 5000;")
            .execute(conn)
            .map_err(diesel::r2d2::Error::QueryError)?;
        Ok(())
    }
}

pub type GoogleOAuthClient =
    BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;
pub type TeslaOAuthClient =
    BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;

pub struct AppState {
    pub db_pool: DbPool,
    pub user_core: Arc<UserCore>,
    pub user_repository: Arc<UserRepository>,
    pub ai_config: AiConfig,
    pub google_calendar_oauth_client: GoogleOAuthClient,
    pub youtube_oauth_client: GoogleOAuthClient,
    pub uber_oauth_client: GoogleOAuthClient,
    pub tesla_oauth_client: TeslaOAuthClient,
    pub session_store: MemoryStore,
    pub login_limiter:
        DashMap<String, RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>>,
    pub password_reset_limiter:
        DashMap<String, RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>>,
    pub password_reset_verify_limiter:
        DashMap<String, RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>>,
    pub matrix_sync_tasks: Arc<Mutex<HashMap<i32, tokio::task::JoinHandle<()>>>>,
    pub matrix_clients: Arc<Mutex<HashMap<i32, Arc<matrix_sdk::Client>>>>,
    pub tesla_monitoring_tasks: Arc<DashMap<i32, tokio::task::JoinHandle<()>>>,
    pub tesla_charging_monitor_tasks: Arc<DashMap<i32, tokio::task::JoinHandle<()>>>,
    // Track vehicles currently being woken to prevent parallel wake attempts
    // Key: VIN, Value: broadcast sender that notifies waiters when wake completes
    pub tesla_waking_vehicles: Arc<DashMap<String, tokio::sync::broadcast::Sender<bool>>>,
    pub password_reset_otps: DashMap<String, (String, u64)>, // (email, (otp, expiration))
    pub phone_verify_limiter:
        DashMap<String, RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>>,
    pub phone_verify_verify_limiter:
        DashMap<String, RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>>,
    pub phone_verify_otps: DashMap<String, (String, u64)>,
    pub pending_message_senders: Arc<Mutex<HashMap<i32, oneshot::Sender<()>>>>,
    pub totp_repository: Arc<TotpRepository>,
    pub webauthn_repository: Arc<WebauthnRepository>,
    pub pending_totp_logins: DashMap<String, (i32, i64)>, // (totp_token, (user_id, expiry_timestamp))
    pub pending_password_resets: DashMap<String, (i32, i64)>, // (reset_token, (user_id, expiry_timestamp))
    pub session_to_token: DashMap<String, String>, // stripe_session_id -> magic_token (temporary, for redirect flow)
    pub totp_verify_limiter:
        DashMap<String, RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>>,
    pub webauthn_verify_limiter:
        DashMap<String, RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>>,
}

pub struct TwilioConfig {
    pub account_sid: String,
    pub auth_token: String,
    pub from_number: String,
}

impl Default for TwilioConfig {
    fn default() -> Self {
        Self {
            account_sid: std::env::var("TWILIO_ACCOUNT_SID")
                .expect("TWILIO_ACCOUNT_SID must be set"),
            auth_token: std::env::var("TWILIO_AUTH_TOKEN").expect("TWILIO_AUTH_TOKEN must be set"),
            from_number: std::env::var("TWILIO_FROM_NUMBER")
                .expect("TWILIO_FROM_NUMBER must be set"),
        }
    }
}

impl TwilioConfig {
    pub fn new() -> Self {
        Self::default()
    }
}

pub fn generate_otp() -> String {
    let mut rng = rand::thread_rng();
    format!("{:06}", rng.gen_range(0..999999))
}

pub async fn send_otp(config: &TwilioConfig, to_number: &str, otp: &str) -> Result<(), String> {
    let client = Client::new();
    let message = format!("Your verification code is: {}. Valid for 10 minutes.", otp);

    // Create basic auth header
    let auth = format!("{}:{}", config.account_sid, config.auth_token);
    let encoded_auth = BASE64.encode(auth.as_bytes());

    // Prepare the request URL
    let url = format!(
        "https://api.twilio.com/2010-04-01/Accounts/{}/Messages.json",
        config.account_sid
    );

    // Create form data
    let form = json!({
        "From": config.from_number,
        "To": to_number,
        "Body": message,
    });

    // Send the request
    let response = client
        .post(&url)
        .header("Authorization", format!("Basic {}", encoded_auth))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&form)
        .send()
        .await
        .map_err(|e| format!("Failed to send request: {}", e))?;

    // Check if the request was successful
    if !response.status().is_success() {
        let error_text = response
            .text()
            .await
            .unwrap_or_else(|_| "Unknown error".to_string());
        return Err(format!("Twilio API error: {}", error_text));
    }

    Ok(())
}
