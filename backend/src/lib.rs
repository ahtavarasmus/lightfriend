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
    pub mod dashboard_handlers;
    pub mod filter_handlers;
    pub mod google_calendar;
    pub mod google_calendar_auth;
    pub mod google_maps;
    pub mod imap_auth;
    pub mod imap_handlers;
    pub mod instagram_auth;
    pub mod instagram_handlers;
    pub mod instagram_reels;
    pub mod mcp_handlers;
    pub mod messenger_auth;
    pub mod messenger_handlers;
    pub mod pricing_handlers;
    pub mod profile_handlers;
    pub mod reddit;
    pub mod refund_handlers;
    pub mod rumble;
    pub mod self_host_handlers;
    pub mod seo_pages;
    pub mod signal_auth;
    pub mod signal_handlers;
    pub mod spotify;
    pub mod stats_handlers;
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
    pub mod wellbeing_handlers;
    pub mod whatsapp_auth;
    pub mod whatsapp_handlers;
    pub mod ws_handler;
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
    pub mod migration_proxy;
    pub mod notification_utils;
    pub mod plan_features;
    pub mod tesla_keys;
    pub mod tool_exec;
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
    pub mod mcp;
    pub mod tesla;
    pub mod utils;
    pub mod youtube;
}
pub mod cli;
pub mod api {
    pub mod elevenlabs;
    pub mod elevenlabs_webhook;
    pub mod internal_routing;
    pub mod matrix_client;
    pub mod tesla;
    pub mod tesla_client;
    pub mod twilio_availability;
    pub mod twilio_client;
    pub mod twilio_pricing;
    pub mod twilio_sms;
    pub mod twilio_utils;
}
pub mod context;
pub mod error;
pub mod tools {
    pub mod calendar;
    pub mod email;
    pub mod items;
    pub mod messaging;
    pub mod quiet_mode;
    pub mod registry;
    pub mod respond;
    pub mod schedule;
    pub mod search;
    pub mod tesla;
    pub mod weather;
    pub mod youtube;
}
pub mod models {
    pub mod mcp_models;
    pub mod user_models;
}
pub mod repositories {
    pub mod admin_alert_repository;
    pub mod item_repository;
    pub mod mcp_repository;
    pub mod metrics_repository;
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
    pub mod wellbeing_repository;
}
pub mod services {
    pub mod country_service;
    pub mod mcp_client;
    pub mod metrics_service;
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
pub use api::matrix_client::{
    IncomingBridgeEvent, IncomingMessageContent, MatrixClientInterface, MatrixClientWrapper,
    MockMatrixCalls, MockMatrixClient, MockRoom, RoomInfo, RoomInterface, RoomMember, RoomWrapper,
};
pub use api::twilio_client::RealTwilioClient;
pub use repositories::admin_alert_repository::AdminAlertRepository;
pub use repositories::item_repository::ItemRepository;
pub use repositories::metrics_repository::MetricsRepository;
pub use repositories::totp_repository::TotpRepository;
pub use repositories::user_core::{UserCore, UserCoreOps};
pub use repositories::user_repository::UserRepository;
pub use repositories::webauthn_repository::WebauthnRepository;
pub use repositories::wellbeing_repository::WellbeingRepository;
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
    pub item_repository: Arc<ItemRepository>,
    pub twilio_client: Arc<RealTwilioClient>,
    pub twilio_message_service: Arc<TwilioMessageService<RealTwilioClient>>,
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
    pub ws_notification_senders: Arc<DashMap<i32, tokio::sync::broadcast::Sender<String>>>,
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
    pub wellbeing_repository: Arc<WellbeingRepository>,
    pub tool_registry: tools::registry::ToolRegistry,
}

/// Build the tool registry with all static tool handlers.
pub fn build_tool_registry() -> tools::registry::ToolRegistry {
    use tools::registry::ToolRegistry;

    let mut registry = ToolRegistry::new();

    // Search tools
    registry.register(Arc::new(tools::search::PerplexityHandler));
    registry.register(Arc::new(tools::search::FirecrawlHandler));
    registry.register(Arc::new(tools::search::DirectionsHandler));
    registry.register(Arc::new(tools::search::QrScanHandler));

    // Weather
    registry.register(Arc::new(tools::weather::WeatherHandler));

    // Email tools
    registry.register(Arc::new(tools::email::FetchEmailsHandler));
    registry.register(Arc::new(tools::email::FetchSpecificEmailHandler));
    registry.register(Arc::new(tools::email::SendEmailHandler));
    registry.register(Arc::new(tools::email::RespondEmailHandler));

    // Messaging tools
    registry.register(Arc::new(tools::messaging::SearchContactsHandler));
    registry.register(Arc::new(tools::messaging::FetchRecentHandler));
    registry.register(Arc::new(tools::messaging::FetchMessagesHandler));
    registry.register(Arc::new(tools::messaging::SendMessageHandler));

    // Calendar tools
    registry.register(Arc::new(tools::calendar::FetchEventsHandler));
    registry.register(Arc::new(tools::calendar::CreateEventHandler));

    // Schedule/management tools
    registry.register(Arc::new(tools::schedule::CreateItemHandler));

    // Tesla tools
    registry.register(Arc::new(tools::tesla::TeslaControlHandler));
    registry.register(Arc::new(tools::tesla::TeslaSwitchHandler));

    // YouTube
    registry.register(Arc::new(tools::youtube::YouTubeHandler));

    // Item tracking tools
    registry.register(Arc::new(tools::items::ListTrackedItemsHandler));
    registry.register(Arc::new(tools::items::UpdateTrackedItemHandler));

    // Direct response
    registry.register(Arc::new(tools::respond::DirectResponseHandler));

    // Quiet mode
    registry.register(Arc::new(tools::quiet_mode::QuietModeHandler));

    registry
}
