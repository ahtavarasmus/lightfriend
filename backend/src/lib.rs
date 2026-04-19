// Module declarations - moved from main.rs for library access
pub mod handlers {
    pub mod admin_handlers;
    pub mod admin_stats_handlers;
    pub mod attestation_handlers;
    pub mod auth_dtos;
    pub mod auth_handlers;
    pub mod auth_middleware;
    pub mod billing_handlers;
    pub mod bridge_auth_common;
    pub mod dashboard_handlers;
    pub mod imap_auth;
    pub mod imap_handlers;
    pub mod mcp_handlers;
    pub mod person_handlers;
    pub mod pricing_handlers;
    pub mod profile_handlers;
    pub mod rule_handlers;

    pub mod maintenance_handlers;
    pub mod self_host_handlers;
    pub mod signal_auth;
    pub mod signal_handlers;
    pub mod stats_handlers;
    pub mod stripe_handlers;
    pub mod telegram_auth;
    pub mod telegram_handlers;
    pub mod tesla_auth;
    pub mod totp_handlers;
    pub mod trust_chain_handlers;
    pub mod twilio_handlers;
    pub mod webauthn_handlers;
    pub mod whatsapp_auth;
    pub mod whatsapp_handlers;
    pub mod youtube;
    pub mod youtube_auth;
}
pub mod utils {
    pub mod bridge;
    pub mod bridge_contacts;
    pub mod bridge_responses;
    pub mod country;
    pub mod email;
    pub mod encryption;
    pub mod id_verifier;
    pub mod imap_idle;
    pub mod matrix_auth;
    pub mod notification_utils;
    pub mod plan_features;
    pub mod resend_contacts;
    pub mod tesla_keys;
    pub mod tool_exec;
    pub mod usage;
    pub mod webauthn_config;
}
pub mod proactive {
    pub mod rules;
    pub mod signal_extraction;
    pub mod system_behaviors;
    pub mod utils;
}
pub mod tool_call_utils {
    pub mod bridge;
    pub mod email;
    pub mod internet;
    pub mod mcp;
    pub mod tesla;
    pub mod utils;
    pub mod youtube;
}
pub mod cli;
pub mod api {
    pub mod matrix_client;
    pub mod tesla;
    pub mod tesla_client;
    pub mod tinfoil_client;
    pub mod twilio_availability;
    pub mod twilio_client;
    pub mod twilio_pricing;
    pub mod twilio_sms;
    pub mod twilio_utils;
    pub mod voice_pipeline;
}
pub mod agent_core;
pub mod context;
pub mod tools {
    pub mod email;
    pub mod messaging;
    pub mod ontology;
    pub mod registry;
    pub mod rules;
    pub mod search;
    pub mod tesla;
    pub mod weather;
    pub mod youtube;
}
pub mod models {
    pub mod mcp_models;
    pub mod ontology_models;
    pub mod user_models;
}
pub mod repositories {
    pub mod admin_alert_repository;
    pub mod bandwidth_repository;
    pub mod llm_usage_repository;
    pub mod mcp_repository;
    pub mod metrics_repository;
    pub mod mock_signup_repository;
    pub mod mock_twilio_status_repository;
    pub mod ontology_repository;
    pub mod signup_repository;
    pub mod signup_repository_impl;
    pub mod totp_repository;
    pub mod twilio_status_repository;
    pub mod twilio_status_repository_impl;
    pub mod user_core;
    pub mod user_repository;
    pub mod user_subscriptions;
    pub mod webauthn_repository;
    pub mod whatsapp_bridge_repository;
}
pub mod services {
    pub mod country_service;
    pub mod mcp_client;
    pub mod metrics_service;
    pub mod signup_service;
    pub mod twilio_message_service;
    pub mod twilio_status_service;
}
pub mod pg_models;
pub mod pg_schema;
pub mod jobs {
    pub mod scheduler;
}
pub mod ontology {
    pub mod registry;
}
pub mod ai_config;
pub use ai_config::{AiConfig, AiProvider, ModelPurpose};
pub mod blog {
    pub mod content;
    pub mod handlers;
    pub mod linking;
    pub mod schema;
    pub mod templates;
}

// Test utilities for integration tests
pub mod test_utils;

// Re-export key types for external use
pub use api::matrix_client::{
    IncomingBridgeEvent, IncomingMessageContent, MatrixClientInterface, MatrixClientWrapper,
    MockMatrixCalls, MockMatrixClient, MockRoom, RoomInfo, RoomInterface, RoomMember, RoomWrapper,
};
pub use api::twilio_client::RealTwilioClient;
pub use repositories::admin_alert_repository::AdminAlertRepository;
pub use repositories::bandwidth_repository::BandwidthRepository;
pub use repositories::llm_usage_repository::LlmUsageRepository;
pub use repositories::metrics_repository::MetricsRepository;
pub use repositories::ontology_repository::OntologyRepository;
pub use repositories::totp_repository::TotpRepository;
pub use repositories::user_core::{UserCore, UserCoreOps};
pub use repositories::user_repository::UserRepository;
pub use repositories::webauthn_repository::WebauthnRepository;
pub use repositories::whatsapp_bridge_repository::{WhatsAppBridgeRepository, WhatsAppContact};
pub use services::twilio_message_service::TwilioMessageService;

// Tesla client trait and pure functions
pub use api::tesla_client::{
    format_battery_status, is_actively_charging, is_charging_complete, is_charging_stopped,
    is_climate_ready, should_wake_vehicle, wake_retry_delay, TeslaClientInterface,
};

// AppState and related types - needed by all handler modules
use dashmap::DashMap;
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use governor::{clock::DefaultClock, state::keyed::DefaultKeyedStateStore, RateLimiter};
use oauth2::{basic::BasicClient, EndpointNotSet, EndpointSet};
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};
use tower_sessions::MemoryStore;

pub type PgDbPool = r2d2::Pool<ConnectionManager<PgConnection>>;

pub type GoogleOAuthClient =
    BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;
pub type TeslaOAuthClient =
    BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;

pub struct AppState {
    pub pg_pool: PgDbPool,
    pub user_core: Arc<UserCore>,
    pub user_repository: Arc<UserRepository>,
    pub twilio_client: Arc<RealTwilioClient>,
    pub twilio_message_service: Arc<TwilioMessageService<RealTwilioClient>>,
    pub ai_config: AiConfig,
    pub youtube_oauth_client: GoogleOAuthClient,
    pub tesla_oauth_client: TeslaOAuthClient,
    pub session_store: MemoryStore,
    pub login_limiter:
        DashMap<String, RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>>,
    pub password_reset_limiter:
        DashMap<String, RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>>,
    pub password_reset_verify_limiter:
        DashMap<String, RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>>,
    pub api_rate_limiter:
        DashMap<String, RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>>,
    pub matrix_sync_tasks: Arc<Mutex<HashMap<i32, tokio::task::JoinHandle<()>>>>,
    pub matrix_clients: Arc<Mutex<HashMap<i32, Arc<matrix_sdk::Client>>>>,
    pub tesla_monitoring_tasks: Arc<DashMap<i32, tokio::task::JoinHandle<()>>>,
    pub tesla_charging_monitor_tasks: Arc<DashMap<i32, tokio::task::JoinHandle<()>>>,
    /// Per-IMAP-connection IDLE tasks. Key is `imap_connection.id`
    /// (NOT `user_id`) so users with multiple email accounts each get
    /// their own task.
    pub imap_idle_tasks: Arc<DashMap<i32, tokio::task::JoinHandle<()>>>,
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
    pub admin_alert_repository: Arc<AdminAlertRepository>,
    pub metrics_repository: Arc<MetricsRepository>,
    pub pending_totp_logins: DashMap<String, (i32, i64)>, // (totp_token, (user_id, expiry_timestamp))
    pub pending_password_resets: DashMap<String, (i32, i64)>, // (reset_token, (user_id, expiry_timestamp))
    pub session_to_token: DashMap<String, String>, // stripe_session_id -> magic_token (temporary, for redirect flow)
    pub totp_verify_limiter:
        DashMap<String, RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>>,
    pub webauthn_verify_limiter:
        DashMap<String, RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>>,
    pub llm_usage_repository: Arc<LlmUsageRepository>,
    pub bandwidth_repository: Arc<BandwidthRepository>,
    pub ontology_repository: Arc<OntologyRepository>,
    /// Optional: read-only access to mautrix-whatsapp's PostgreSQL database.
    /// None when WHATSAPP_BRIDGE_DATABASE_URL is unset (e.g. dev environments).
    pub whatsapp_bridge_repository: Option<Arc<WhatsAppBridgeRepository>>,
    pub ontology_registry: ontology::registry::OntologyRegistry,
    pub tool_registry: tools::registry::ToolRegistry,
    pub pending_rule_tests: Arc<DashMap<String, handlers::rule_handlers::PendingRuleTest>>,
    pub maintenance_mode: Arc<AtomicBool>,
    pub blog_store: Arc<blog::content::BlogStore>,
    // (user_id, room_id) -> unix timestamp of last system_important notification
    pub system_notify_cooldowns: DashMap<(i32, String), i32>,
    // user_id -> unix timestamp of last digest delivery
    pub digest_cooldowns: DashMap<i32, i32>,
    // Broadcast channel: signals activity feed subscribers that new data is available for a user_id
    pub activity_feed_tx: tokio::sync::broadcast::Sender<i32>,
}

impl AppState {
    /// Signal that the activity feed has new data for a user.
    /// Subscribers (SSE connections) will notify the frontend to re-fetch.
    pub fn notify_activity_feed(&self, user_id: i32) {
        let _ = self.activity_feed_tx.send(user_id);
    }
}

/// Build the tool registry with all static tool handlers.
pub fn build_tool_registry() -> tools::registry::ToolRegistry {
    use tools::registry::ToolRegistry;

    let mut registry = ToolRegistry::new();

    // Search tools
    registry.register(Arc::new(tools::search::PerplexityHandler));
    registry.register(Arc::new(tools::search::FirecrawlHandler));
    registry.register(Arc::new(tools::search::QrScanHandler));

    // Weather
    registry.register(Arc::new(tools::weather::WeatherHandler));

    // Email tools
    registry.register(Arc::new(tools::email::SendEmailHandler));
    registry.register(Arc::new(tools::email::RespondEmailHandler));

    // Messaging tools
    registry.register(Arc::new(tools::messaging::SendMessageHandler));

    // Rules (Automation -> Logic -> Action)
    registry.register(Arc::new(tools::rules::SetReminderHandler));
    registry.register(Arc::new(tools::rules::CreateEventHandler));
    registry.register(Arc::new(tools::rules::UpdateEventHandler));

    // Tesla tools
    registry.register(Arc::new(tools::tesla::TeslaControlHandler));

    // YouTube
    registry.register(Arc::new(tools::youtube::YouTubeHandler));

    registry
}
