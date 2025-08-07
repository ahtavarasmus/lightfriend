use dotenvy::dotenv;
use axum::{
    routing::{get, post, delete},
    Router,
    middleware
};
use tokio::sync::Mutex;
use tower_sessions::{MemoryStore, SessionManagerLayer};
use std::collections::HashMap;
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use dashmap::DashMap;
use governor::{RateLimiter, clock::DefaultClock, state::keyed::DefaultKeyedStateStore};
use oauth2::{
    basic::BasicClient,
    AuthUrl,
    ClientId,
    ClientSecret,
    RedirectUrl,
    TokenUrl,
    EndpointSet,
    EndpointNotSet,
};
use tower_http::cors::{CorsLayer, AllowOrigin};
use tower_http::services::ServeDir;
use tower_http::trace::{TraceLayer, DefaultMakeSpan, DefaultOnResponse};
use tracing::Level;
use std::sync::Arc;
use sentry;

mod handlers {
    pub mod auth_handlers;
    pub mod auth_dtos;
    pub mod profile_handlers;
    pub mod billing_handlers;
    pub mod admin_handlers;
    pub mod stripe_handlers;
    pub mod google_calendar_auth;
    pub mod google_calendar;
    pub mod imap_auth;
    pub mod auth_middleware;
    pub mod imap_handlers;
    pub mod whatsapp_auth;
    pub mod whatsapp_handlers;
    pub mod filter_handlers;
    pub mod google_tasks;
    pub mod google_tasks_auth;
    pub mod telegram_auth;
    pub mod telegram_handlers;
    pub mod self_host_handlers;
    pub mod twilio_handlers;
}

mod utils {
    pub mod encryption;
    pub mod tool_exec;
    pub mod usage;
    pub mod matrix_auth;
    pub mod bridge;
    pub mod elevenlabs_prompts;
    pub mod imap_utils;
    pub mod qr_utils;
}

mod proactive {
    pub mod utils;
}

mod tool_call_utils {
    pub mod email;
    pub mod calendar;
    pub mod tasks;
    pub mod utils;
    pub mod internet;
    pub mod management;
    pub mod confirm;
    pub mod bridge;
}

mod api {
    pub mod vapi_endpoints;
    pub mod vapi_dtos;
    pub mod twilio_sms;
    pub mod twilio_utils;
    pub mod elevenlabs;
    pub mod elevenlabs_webhook;
    pub mod shazam_call;
}


mod error;

mod models {
    pub mod user_models;
}
mod repositories {
    pub mod user_core;
    pub mod user_repository;
    pub mod user_subscriptions;
    pub mod connection_auth;
}
mod schema;
mod jobs {
    pub mod scheduler;
}

use repositories::user_core::UserCore;
use repositories::user_repository::UserRepository;

use handlers::{
    auth_handlers, self_host_handlers, profile_handlers, billing_handlers,
    admin_handlers, stripe_handlers, google_calendar_auth, google_calendar,
    google_tasks_auth, google_tasks, imap_auth, imap_handlers,
    whatsapp_auth, whatsapp_handlers, telegram_auth, telegram_handlers,
    filter_handlers, twilio_handlers,
};
use api::{twilio_sms, elevenlabs, elevenlabs_webhook, shazam_call};

type DbPool = r2d2::Pool<ConnectionManager<SqliteConnection>>;

async fn health_check() -> &'static str {
    "OK"
}

type GoogleOAuthClient = BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;

pub struct AppState {
    db_pool: DbPool,
    user_core: Arc<UserCore>,
    user_repository: Arc<UserRepository>,
    sessions: shazam_call::CallSessions,
    user_calls: shazam_call::UserCallMap,
    google_calendar_oauth_client: GoogleOAuthClient,
    google_tasks_oauth_client: GoogleOAuthClient,
    gmail_oauth_client: GoogleOAuthClient,
    session_store: MemoryStore,
    login_limiter: DashMap<String, RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>>,
    password_reset_limiter: DashMap<String, RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>>,
    password_reset_verify_limiter: DashMap<String, RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>>,
    matrix_sync_tasks: Arc<Mutex<HashMap<i32, tokio::task::JoinHandle<()>>>>,
    matrix_invitation_tasks: Arc<Mutex<HashMap<i32, tokio::task::JoinHandle<()>>>>,
    matrix_clients: Arc<Mutex<HashMap<i32, Arc<matrix_sdk::Client>>>>,
    password_reset_otps: DashMap<String, (String, u64)>, // (email, (otp, expiration))
}

pub fn validate_env() {
    let required_vars = [
        "JWT_SECRET_KEY", "JWT_REFRESH_KEY", "DATABASE_URL", "PERPLEXITY_API_KEY",
        "ASSISTANT_ID", "ELEVENLABS_SERVER_URL_SECRET", "FIN_PHONE", "USA_PHONE",
        "AUS_PHONE", "GB_PHONE", "TWILIO_ACCOUNT_SID", "TWILIO_AUTH_TOKEN",
        "ENVIRONMENT", "FRONTEND_URL", "STRIPE_CREDITS_PRODUCT_ID",
        "STRIPE_SUBSCRIPTION_WORLD_PRICE_ID",
        "STRIPE_SECRET_KEY", "STRIPE_PUBLISHABLE_KEY", "STRIPE_WEBHOOK_SECRET",
        "SHAZAM_PHONE_NUMBER", "SHAZAM_API_KEY", "SERVER_URL", 
        "ENCRYPTION_KEY", "COMPOSIO_API_KEY", "GOOGLE_CALENDAR_CLIENT_ID", 
        "GOOGLE_CALENDAR_CLIENT_SECRET", "MATRIX_HOMESERVER", "MATRIX_SHARED_SECRET",
        "WHATSAPP_BRIDGE_BOT", "GOOGLE_CALENDAR_CLIENT_SECRET", "OPENROUTER_API_KEY",
        "MATRIX_HOMESERVER_PERSISTENT_STORE_PATH",
    ];
    for var in required_vars.iter() {
        std::env::var(var).expect(&format!("{} must be set", var));
    }
}


#[tokio::main]
async fn main() {
    dotenv().ok();

    let _guard = sentry::init(("https://07fbdaf63c1270c8509844b775045dd3@o4507415184539648.ingest.de.sentry.io/4508802101411920", sentry::ClientOptions {
        release: sentry::release_name!(),
        ..Default::default()
    }));

    use tracing_subscriber::{fmt, EnvFilter};
    
    // Create filter that sets Matrix SDK logs to WARN and keeps our app at DEBUG
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            EnvFilter::new("info,lightfriend=debug")
                .add_directive("matrix_sdk=error".parse().unwrap())  // Changed from warn to error
                .add_directive("tokio-runtime-worker=off".parse().unwrap())
                .add_directive("ruma=warn".parse().unwrap())
                .add_directive("eyeball=warn".parse().unwrap())
                .add_directive("matrix_sdk::encryption=error".parse().unwrap())  // Added specific filter for encryption module
        });

    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    let manager = ConnectionManager::<SqliteConnection>::new("database.db");
    let pool = r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create pool");

    let user_core= Arc::new(UserCore::new(pool.clone()));
    let user_repository = Arc::new(UserRepository::new(pool.clone()));

    let client_id = std::env::var("GOOGLE_CALENDAR_CLIENT_ID").expect("GOOGLE_CALENDAR_CLIENT_ID must be set");
    let client_secret = std::env::var("GOOGLE_CALENDAR_CLIENT_SECRET").expect("GOOGLE_CALENDAR_CLIENT_SECRET must be set");
    let client_id_clone = client_id.clone();
    let client_secret_clone = client_secret.clone();
    let server_url_oauth = std::env::var("SERVER_URL_OAUTH").expect("SERVER_URL_OAUTH must be set");

    let google_calendar_oauth_client = BasicClient::new(ClientId::new(client_id.clone()))
        .set_client_secret(ClientSecret::new(client_secret.clone()))
        .set_auth_uri(AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string()).expect("Invalid auth URL"))
        .set_token_uri(TokenUrl::new("https://oauth2.googleapis.com/token".to_string()).expect("Invalid token URL"))
        .set_redirect_uri(RedirectUrl::new(format!("{}/api/auth/google/calendar/callback", server_url_oauth)).expect("Invalid redirect URL"));
    let gmail_oauth_client = BasicClient::new(ClientId::new(client_id_clone))
        .set_client_secret(ClientSecret::new(client_secret_clone))
        .set_auth_uri(AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string()).expect("Invalid auth URL"))
        .set_token_uri(TokenUrl::new("https://oauth2.googleapis.com/token".to_string()).expect("Invalid token URL"))
        .set_redirect_uri(RedirectUrl::new(format!("{}/api/auth/google/gmail/callback", server_url_oauth)).expect("Invalid redirect URL"));

    let session_store = MemoryStore::default();
    let is_prod = std::env::var("ENVIRONMENT") != Ok("development".to_string());
    let session_layer = SessionManagerLayer::new(session_store.clone())
        .with_secure(is_prod)
        .with_same_site(tower_sessions::cookie::SameSite::Lax);

    let google_tasks_oauth_client = BasicClient::new(ClientId::new(client_id))
        .set_client_secret(ClientSecret::new(client_secret))
        .set_auth_uri(AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string()).expect("Invalid auth URL"))
        .set_token_uri(TokenUrl::new("https://oauth2.googleapis.com/token".to_string()).expect("Invalid token URL"))
        .set_redirect_uri(RedirectUrl::new(format!("{}/api/auth/google/tasks/callback", server_url_oauth)).expect("Invalid redirect URL"));

    let matrix_sync_tasks = Arc::new(Mutex::new(HashMap::new()));
    let matrix_invitation_tasks = Arc::new(Mutex::new(HashMap::new()));
    let matrix_clients = Arc::new(Mutex::new(HashMap::new()));

    let state = Arc::new(AppState {
        db_pool: pool,
        password_reset_otps: DashMap::new(),
        user_core: user_core.clone(),
        user_repository: user_repository.clone(),
        sessions: Arc::new(Mutex::new(HashMap::new())),
        user_calls: Arc::new(Mutex::new(HashMap::new())),
        google_calendar_oauth_client,
        google_tasks_oauth_client,
        gmail_oauth_client,
        session_store: session_store.clone(),
        login_limiter: DashMap::new(),
        password_reset_limiter: DashMap::new(),
        password_reset_verify_limiter: DashMap::new(),
        matrix_sync_tasks,
        matrix_invitation_tasks,
        matrix_clients,
    });

    let twilio_routes = Router::new()
        .route("/api/sms/server", post(twilio_sms::handle_regular_sms))
        .layer(middleware::from_fn_with_state(state.clone(), api::twilio_utils::validate_twilio_signature));

    let user_twilio_routes = Router::new()
        .route("/api/sms/server/{user_id}", post(twilio_sms::handle_incoming_sms))
        .route_layer(middleware::from_fn(api::twilio_utils::validate_user_twilio_signature));

    let textbee_routes = Router::new()
        .route("/api/sms/textbee-server", post(twilio_sms::handle_textbee_sms));
        // textbee requests are validated using device_id and phone number combo

    let elevenlabs_free_routes = Router::new()
        .route("/api/call/assistant", post(elevenlabs::fetch_assistant))
        .route("/api/call/weather", post(elevenlabs::handle_weather_tool_call))
        .route("/api/call/perplexity", post(elevenlabs::handle_perplexity_tool_call))
        .route_layer(middleware::from_fn(elevenlabs::validate_elevenlabs_secret));

    let elevenlabs_routes = Router::new()
        .route("/api/call/sms", post(elevenlabs::handle_send_sms_tool_call))
        .route("/api/call/shazam", get(elevenlabs::handle_shazam_tool_call))
        .route("/api/call/calendar", get(elevenlabs::handle_calendar_tool_call))
        .route("/api/call/calendar/confirm", get(elevenlabs::handle_calendar_event_confirm))
        .route("/api/call/email", get(elevenlabs::handle_email_fetch_tool_call))
        .route("/api/call/email/specific", post(elevenlabs::handle_email_search_tool_call))
        .route("/api/call/waiting_check", post(elevenlabs::handle_create_waiting_check_tool_call))
        .route("/api/call/monitoring-status", post(elevenlabs::handle_update_monitoring_status_tool_call))
        .route("/api/call/email/respond-confirm", post(elevenlabs::handle_email_response_tool_call))
        .route("/api/call/tasks", get(elevenlabs::handle_tasks_fetching_tool_call))
        .route("/api/call/tasks/create", post(elevenlabs::handle_tasks_creation_tool_call))
        .route("/api/call/fetch-recent-messages", get(elevenlabs::handle_fetch_recent_messages_tool_call))
        .route("/api/call/fetch-chat-messages", get(elevenlabs::handle_fetch_specific_chat_messages_tool_call))
        .route("/api/call/search-chat-contacts", post(elevenlabs::handle_search_chat_contacts_tool_call))
        .route("/api/call/send-chat-message", post(elevenlabs::handle_confirm_send_chat_message))
        .layer(middleware::from_fn_with_state(state.clone(), handlers::auth_middleware::check_subscription_access))
        .route_layer(middleware::from_fn(elevenlabs::validate_elevenlabs_secret));

    let elevenlabs_webhook_routes = Router::new()
        .route("/api/webhook/elevenlabs", post(elevenlabs_webhook::elevenlabs_webhook))
        .route_layer(middleware::from_fn(elevenlabs_webhook::validate_elevenlabs_hmac));

    let auth_built_in_webhook_routes = Router::new()
        .route("/api/stripe/webhook", post(stripe_handlers::stripe_webhook))
        .route("/api/auth/google/calendar/callback", get(google_calendar_auth::google_callback))
        .route("/api/auth/google/tasks/callback", get(google_tasks_auth::google_tasks_callback));


    // Public routes that don't need authentication
    let public_routes = Router::new()
        .route("/api/health", get(health_check))
        .route("/api/login", post(auth_handlers::login))
        .route("/api/register", post(auth_handlers::register))
        .route("/api/password-reset/request", post(auth_handlers::request_password_reset))
        .route("/api/password-reset/verify", post(auth_handlers::verify_password_reset))
        .route("/api/self-hosted/signup", post(self_host_handlers::self_hosted_signup))
        .route("/api/self-hosted/login", post(self_host_handlers::self_hosted_login))
        .route("/api/self-hosting-status", get(self_host_handlers::self_hosted_status))
        .route("/api/check-pairing", post(self_host_handlers::check_pairing))
        .route("/api/self-host-ping", post(self_host_handlers::self_host_ping))
        .route("/api/country-info", post(twilio_handlers::get_country_info));

    // Admin routes that need admin authentication
    let admin_routes = Router::new()
        .route("/testing", post(auth_handlers::testing_handler))
        .route("/api/admin/users", get(auth_handlers::get_users))
        .route("/api/admin/verify/{user_id}", post(admin_handlers::verify_user))
        .route("/api/admin/preferred-number/{user_id}", post(admin_handlers::update_preferred_number_admin))
        .route("/api/admin/broadcast", post(admin_handlers::broadcast_message))
        .route("/api/admin/broadcast-email", post(admin_handlers::broadcast_email))
        .route("/api/admin/usage-logs", get(admin_handlers::get_usage_logs))
        .route("/api/admin/subscription/{user_id}/{tier}", post(admin_handlers::update_subscription_tier))
        .route("/api/billing/reset-credits/{user_id}", post(billing_handlers::reset_credits))
        .route("/api/admin/test-sms", post(admin_handlers::test_sms))
        .route("/api/admin/test-sms-with-image", post(admin_handlers::test_sms_with_image))
        .route("/api/admin/monthly-credits/{user_id}/{amount}", post(admin_handlers::update_monthly_credits))
        .route("/api/admin/discount-tier/{user_id}/{tier}", post(admin_handlers::update_discount_tier))
        .route_layer(middleware::from_fn_with_state(state.clone(), handlers::auth_middleware::require_admin));

    // Protected routes that need user authentication
    let protected_routes = Router::new()
        .route("/api/profile/delete/{user_id}", delete(profile_handlers::delete_user))
        .route("/api/profile/update", post(profile_handlers::update_profile))
        .route("/api/profile/generate-pairing-code", post(self_host_handlers::generate_pairing_code))
        .route("/api/profile/server-ip", post(self_host_handlers::update_server_ip))
        .route("/api/profile/twilio-phone", post(self_host_handlers::update_twilio_phone))
        .route("/api/profile/twilio-creds", post(self_host_handlers::update_twilio_creds))
        .route("/api/profile/textbee-creds", post(self_host_handlers::update_textbee_creds))
        .route("/api/profile/timezone", post(profile_handlers::update_timezone))
        .route("/api/profile/preferred-number", post(profile_handlers::update_preferred_number))
        .route("/api/profile", get(profile_handlers::get_profile))
        .route("/api/profile/update-notify/{user_id}", post(profile_handlers::update_notify))
        .route("/api/profile/digests", post(profile_handlers::update_digests))
        .route("/api/profile/digests", get(profile_handlers::get_digests))
        .route("/api/profile/critical", post(profile_handlers::update_critical_enabled))
        .route("/api/profile/critical", get(profile_handlers::get_critical_enabled))
        .route("/api/profile/proactive-agent", post(profile_handlers::update_proactive_agent_on))
        .route("/api/profile/proactive-agent", get(profile_handlers::get_proactive_agent_on))

        .route("/api/billing/increase-credits/{user_id}", post(billing_handlers::increase_credits))
        .route("/api/billing/usage", post(billing_handlers::get_usage_data))
        .route("/api/billing/update-auto-topup/{user_id}", post(billing_handlers::update_topup))

        .route("/api/stripe/checkout-session/{user_id}", post(stripe_handlers::create_checkout_session))
        .route("/api/stripe/unified-subscription-checkout/{user_id}", post(stripe_handlers::create_unified_subscription_checkout))
        .route("/api/stripe/customer-portal/{user_id}", get(stripe_handlers::create_customer_portal_session))

        .route("/api/auth/google/calendar/login", get(google_calendar_auth::google_login))
        .route("/api/auth/google/calendar/connection", delete(google_calendar_auth::delete_google_calendar_connection))
        .route("/api/auth/google/calendar/status", get(google_calendar::google_calendar_status))
        .route("/api/auth/google/calendar/email", get(google_calendar::get_calendar_email))
        .route("/api/calendar/events", get(google_calendar::handle_calendar_fetching_route))
        .route("/api/calendar/create", post(google_calendar::create_calendar_event))

        .route("/api/auth/google/tasks/login", get(google_tasks_auth::google_tasks_login))
        .route("/api/auth/google/tasks/connection", delete(google_tasks_auth::delete_google_tasks_connection))
        .route("/api/auth/google/tasks/refresh", post(google_tasks_auth::refresh_google_tasks_token))
        .route("/api/auth/google/tasks/status", get(google_tasks::google_tasks_status))
        .route("/api/tasks", get(google_tasks::handle_tasks_fetching_route))
        .route("/api/tasks/create", post(google_tasks::handle_tasks_creation_route))

        .route("/api/auth/imap/login", post(imap_auth::imap_login))
        .route("/api/auth/imap/status", get(imap_auth::imap_status))
        .route("/api/auth/imap/disconnect", delete(imap_auth::delete_imap_connection))
        .route("/api/imap/previews", get(imap_handlers::fetch_imap_previews))
        .route("/api/imap/message/{email_id}", get(imap_handlers::fetch_single_imap_email))
        .route("/api/imap/full_emails", get(imap_handlers::fetch_full_imap_emails))
        .route("/api/imap/reply", post(imap_handlers::respond_to_email))

        .route("/api/auth/telegram/status", get(telegram_auth::get_telegram_status))
        .route("/api/auth/telegram/connect", get(telegram_auth::start_telegram_connection))
        .route("/api/auth/telegram/disconnect", delete(telegram_auth::disconnect_telegram))
        .route("/api/auth/telegram/resync", post(telegram_auth::resync_telegram))
        .route("/api/telegram/test-messages", get(telegram_handlers::test_fetch_messages))
        .route("/api/telegram/send", post(telegram_handlers::send_message))
        .route("/api/telegram/search-rooms", post(telegram_handlers::search_telegram_rooms_handler))
        .route("/api/telegram/search-rooms", get(telegram_handlers::search_rooms_handler))

        .route("/api/auth/whatsapp/status", get(whatsapp_auth::get_whatsapp_status))
        .route("/api/auth/whatsapp/connect", get(whatsapp_auth::start_whatsapp_connection))
        .route("/api/auth/whatsapp/disconnect", delete(whatsapp_auth::disconnect_whatsapp))
        .route("/api/auth/whatsapp/resync", post(whatsapp_auth::resync_whatsapp))
        .route("/api/whatsapp/test-messages", get(whatsapp_handlers::test_fetch_messages))
        .route("/api/whatsapp/send", post(whatsapp_handlers::send_message))
        .route("/api/whatsapp/search-rooms", post(whatsapp_handlers::search_whatsapp_rooms_handler))
        .route("/api/whatsapp/search-rooms", get(whatsapp_handlers::search_rooms_handler))

        // Filter routes
        .route("/api/filters/waiting-checks", get(filter_handlers::get_waiting_checks))
        .route("/api/filters/waiting-check/{service_type}", post(filter_handlers::create_waiting_check))
        .route("/api/filters/waiting-check/{service_type}/{content}", delete(filter_handlers::delete_waiting_check))

        .route("/api/filters/monitored-contacts", get(filter_handlers::get_priority_senders))
        .route("/api/filters/monitored-contact/{service_type}", post(filter_handlers::create_priority_sender))
        .route("/api/filters/monitored-contact/{service_type}/{content}", delete(filter_handlers::delete_priority_sender))

        .route("/api/filters/priority-sender/{service_type}", post(filter_handlers::create_priority_sender))
        .route("/api/filters/priority-sender/{service_type}/{sender}", delete(filter_handlers::delete_priority_sender))
        .route("/api/filters/priority-senders/{service_type}", get(filter_handlers::get_priority_senders))

        .route("/api/filters/keyword/{service_type}", post(filter_handlers::create_keyword))
        .route("/api/filters/keyword/{service_type}/{keyword}", delete(filter_handlers::delete_keyword))

        // WhatsApp filter toggle routes
        // Generic filter toggle routes
        .route("/api/profile/email-judgments", get(profile_handlers::get_email_judgments))
        .route_layer(middleware::from_fn(handlers::auth_middleware::require_auth));


    let app = Router::new()
        .merge(public_routes)
        .merge(admin_routes)
        .merge(protected_routes)
        .merge(auth_built_in_webhook_routes)
        .route("/api/twiml", get(shazam_call::twiml_handler).post(shazam_call::twiml_handler))
        .route("/api/stream", get(shazam_call::stream_handler))
        .route("/api/listen/{call_sid}", get(shazam_call::listen_handler))
        .merge(user_twilio_routes)  // More specific routes first
        .merge(textbee_routes)
        .merge(twilio_routes)       // More general routes last
        .merge(elevenlabs_routes)
        .merge(elevenlabs_free_routes)
        .merge(elevenlabs_webhook_routes)
        .nest_service("/uploads", ServeDir::new("uploads"))
        // Serve static files (robots.txt, sitemap.xml) at the root
        .layer(session_layer)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO))
        )
        .layer(
            CorsLayer::new()
                .allow_methods([axum::http::Method::GET, axum::http::Method::POST, axum::http::Method::OPTIONS, axum::http::Method::DELETE])
                .allow_origin(AllowOrigin::exact(std::env::var("FRONTEND_URL").expect("FRONTEND_URL must be set").parse().expect("Invalid FRONTEND_URL"))) // Restrict in production
                .allow_headers([
                    axum::http::header::CONTENT_TYPE,
                    axum::http::header::AUTHORIZATION,
                    axum::http::header::ACCEPT,
                    axum::http::header::ORIGIN,
                ])
                .expose_headers([
                    axum::http::header::CONTENT_TYPE,
                    axum::http::header::CONTENT_LENGTH,
                ])
                .allow_credentials(true)
        )
        .with_state(state.clone());

    let state_for_scheduler = state.clone();
    tokio::spawn(async move {
        jobs::scheduler::start_scheduler(state_for_scheduler).await;
    });

    let shazam_state = crate::api::shazam_call::ShazamState {
        sessions: state.sessions.clone(),
        user_calls: state.user_calls.clone(),
        user_core: state.user_core.clone(),
        user_repository: state.user_repository.clone(),
    };
    tokio::spawn(async move {
        crate::api::shazam_call::process_audio_with_shazam(Arc::new(shazam_state)).await;
    });

    use tokio::net::TcpListener;
    let port = match std::env::var("ENVIRONMENT").as_deref() {
        Ok("staging") => 3100, // actually prod, but just saying staging
        _ => 3000,
    };

    validate_env();
    tracing::info!("Starting server on port {}", port);
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await.unwrap();
    axum::serve(listener, app.into_make_service()).await.unwrap();
}
