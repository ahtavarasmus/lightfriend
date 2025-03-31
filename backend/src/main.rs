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
use urlencoding;
use tower_http::cors::{CorsLayer, Any, AllowOrigin};
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
    pub mod oauth_handlers;
    pub mod composio_auth;
    pub mod unipile_auth;
    pub mod google_calendar_auth;
    pub mod google_calendar;
    pub mod gmail_auth;
    pub mod gmail;
    pub mod imap_auth;
    pub mod auth_middleware;
    pub mod imap_handlers;
}

mod utils {
    pub mod encryption;
    pub mod tool_exec;
    pub mod usage;
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

mod models {
    pub mod user_models;
}
mod repositories {
    pub mod user_repository;
    pub mod user_conversations;
}
mod schema;
mod jobs {
    pub mod scheduler;
}

use repositories::user_repository::UserRepository;
use repositories::user_conversations::UserConversations;

use handlers::auth_handlers;
use handlers::profile_handlers;
use handlers::billing_handlers;
use handlers::admin_handlers;
use handlers::stripe_handlers;
use handlers::oauth_handlers;
use handlers::composio_auth;
use handlers::unipile_auth;
use handlers::google_calendar_auth;
use handlers::google_calendar;
use handlers::gmail_auth;
use handlers::gmail;
use handlers::imap_auth;
use handlers::imap_handlers;
use api::twilio_sms;
use api::elevenlabs;
use api::elevenlabs_webhook;
use api::shazam_call;

type DbPool = r2d2::Pool<ConnectionManager<SqliteConnection>>;

async fn health_check() -> &'static str {
    "OK"
}

type GoogleOAuthClient = BasicClient<EndpointSet, EndpointNotSet, EndpointNotSet, EndpointNotSet, EndpointSet>;

pub struct AppState {
    db_pool: DbPool,
    user_repository: Arc<UserRepository>,
    user_conversations: Arc<UserConversations>,
    sessions: shazam_call::CallSessions,
    user_calls: shazam_call::UserCallMap,
    google_calendar_oauth_client: GoogleOAuthClient,
    gmail_oauth_client: GoogleOAuthClient,
    session_store: MemoryStore,
}

pub fn validate_env() {
    let required_vars = [
        "JWT_SECRET_KEY", "JWT_REFRESH_KEY", "DATABASE_URL", "PERPLEXITY_API_KEY",
        "ASSISTANT_ID", "ELEVENLABS_SERVER_URL_SECRET", "FIN_PHONE", "USA_PHONE",
        "NLD_PHONE", "CHZ_PHONE", "AUS_PHONE", "GB_PHONE","TWILIO_ACCOUNT_SID", "TWILIO_AUTH_TOKEN",
        "ENVIRONMENT", "FRONTEND_URL", "OPENROUTER_API_KEY", "STRIPE_CREDITS_PRODUCT_ID",
        "STRIPE_SECRET_KEY", "STRIPE_PUBLISHABLE_KEY", "STRIPE_WEBHOOK_SECRET",
        "MESSAGE_COST", "MESSAGE_COST_US", "VOICE_SECOND_COST", "CHARGE_BACK_THRESHOLD", 
        "SHAZAM_PHONE_NUMBER", "SHAZAM_EUROPE_PHONE_NUMBER","SHAZAM_API_KEY", "SERVER_URL", 
        "ENCRYPTION_KEY", "COMPOSIO_API_KEY", "GOOGLE_CALENDAR_CLIENT_ID", 
        "GOOGLE_CALENDAR_CLIENT_SECRET",
    ];
    for var in required_vars.iter() {
        std::env::var(var).expect(&format!("{} must be set", var));
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    validate_env();

    let _guard = sentry::init(("https://07fbdaf63c1270c8509844b775045dd3@o4507415184539648.ingest.de.sentry.io/4508802101411920", sentry::ClientOptions {
        release: sentry::release_name!(),
        ..Default::default()
    }));

    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .init();

    let manager = ConnectionManager::<SqliteConnection>::new("database.db");
    let pool = r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create pool");

    let user_repository = Arc::new(UserRepository::new(pool.clone()));
    let user_conversations = Arc::new(UserConversations::new(pool.clone()));

    let client_id = std::env::var("GOOGLE_CALENDAR_CLIENT_ID").expect("GOOGLE_CALENDAR_CLIENT_ID must be set");
    let client_secret = std::env::var("GOOGLE_CALENDAR_CLIENT_SECRET").expect("GOOGLE_CALENDAR_CLIENT_SECRET must be set");
    let server_url_oauth = std::env::var("SERVER_URL_OAUTH").expect("SERVER_URL_OAUTH must be set");

    let google_calendar_oauth_client = BasicClient::new(ClientId::new(client_id.clone()))
        .set_client_secret(ClientSecret::new(client_secret.clone()))
        .set_auth_uri(AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string()).expect("Invalid auth URL"))
        .set_token_uri(TokenUrl::new("https://oauth2.googleapis.com/token".to_string()).expect("Invalid token URL"))
        .set_redirect_uri(RedirectUrl::new(format!("{}/api/auth/google/calendar/callback", server_url_oauth)).expect("Invalid redirect URL"));
    let gmail_oauth_client = BasicClient::new(ClientId::new(client_id))
        .set_client_secret(ClientSecret::new(client_secret))
        .set_auth_uri(AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string()).expect("Invalid auth URL"))
        .set_token_uri(TokenUrl::new("https://oauth2.googleapis.com/token".to_string()).expect("Invalid token URL"))
        .set_redirect_uri(RedirectUrl::new(format!("{}/api/auth/google/gmail/callback", server_url_oauth)).expect("Invalid redirect URL"));

    let session_store = MemoryStore::default();
    let is_prod = std::env::var("ENVIRONMENT") != Ok("development".to_string());
    let session_layer = SessionManagerLayer::new(session_store.clone())
        .with_secure(is_prod)
        .with_same_site(tower_sessions::cookie::SameSite::Lax);

    let state = Arc::new(AppState {
        db_pool: pool,
        user_repository: user_repository.clone(),
        user_conversations: user_conversations.clone(),
        sessions: Arc::new(Mutex::new(HashMap::new())),
        user_calls: Arc::new(Mutex::new(HashMap::new())),
        google_calendar_oauth_client,
        gmail_oauth_client,
        session_store: session_store.clone(),
    });

    let twilio_routes = Router::new()
        .route("/api/sms/server", post(twilio_sms::handle_incoming_sms))
        .route_layer(middleware::from_fn(api::twilio_utils::validate_twilio_signature));

    let elevenlabs_routes = Router::new()
        .route("/api/call/assistant", post(elevenlabs::fetch_assistant))
        .route("/api/call/perplexity", post(elevenlabs::handle_perplexity_tool_call))
        .route("/api/call/sms", post(elevenlabs::handle_send_sms_tool_call))
        .route("/api/call/weather", post(elevenlabs::handle_weather_tool_call))
        .route("/api/call/shazam", get(elevenlabs::handle_shazam_tool_call))
        .route("/api/call/calendar", get(elevenlabs::handle_calendar_tool_call))
        .route("/api/call/email", get(elevenlabs::handle_email_fetch_tool_call))
        .route_layer(middleware::from_fn(elevenlabs::validate_elevenlabs_secret));

    let elevenlabs_webhook_routes = Router::new()
        .route("/api/webhook/elevenlabs", post(elevenlabs_webhook::elevenlabs_webhook))
        .route_layer(middleware::from_fn(elevenlabs_webhook::validate_elevenlabs_hmac));

    let auth_built_in_webhook_routes = Router::new()
        .route("/api/stripe/webhook", post(stripe_handlers::stripe_webhook))
        .route("/api/auth/google/calendar/callback", get(google_calendar_auth::google_callback))
        .route("/api/auth/google/gmail/callback", get(gmail_auth::gmail_callback));


    // Public routes that don't need authentication
    let public_routes = Router::new()
        .route("/api/health", get(health_check))
        .route("/api/login", post(auth_handlers::login))
        .route("/api/register", post(auth_handlers::register));

    // Admin routes that need admin authentication
    let admin_routes = Router::new()
        .route("/api/admin/users", get(auth_handlers::get_users))
        .route("/api/admin/verify/{user_id}", post(admin_handlers::verify_user))
        .route("/api/admin/preferred-number/{user_id}", post(admin_handlers::update_preferred_number_admin))
        .route("/api/admin/broadcast", post(admin_handlers::broadcast_message))
        .route("/api/admin/set-preferred-number-default/{user_id}", post(admin_handlers::set_preferred_number_default))
        .route("/api/billing/reset-credits/{user_id}", post(billing_handlers::reset_credits))
        .route_layer(middleware::from_fn_with_state(state.clone(), handlers::auth_middleware::require_admin));

    // Protected routes that need user authentication
    let protected_routes = Router::new()
        .route("/api/profile/delete/{user_id}", delete(profile_handlers::delete_user))
        .route("/api/profile/update", post(profile_handlers::update_profile))
        .route("/api/profile/timezone", post(profile_handlers::update_timezone))
        .route("/api/profile/preferred-number", post(profile_handlers::update_preferred_number))
        .route("/api/profile", get(profile_handlers::get_profile))
        .route("/api/profile/update-notify/{user_id}", post(profile_handlers::update_notify))

        .route("/api/billing/increase-credits/{user_id}", post(billing_handlers::increase_credits))
        .route("/api/billing/usage", post(billing_handlers::get_usage_data))
        .route("/api/billing/update-auto-topup/{user_id}", post(billing_handlers::update_topup))

        .route("/api/stripe/checkout-session/{user_id}", post(stripe_handlers::create_checkout_session))
        // TODO can use this on the topping up credits if user already has bought some before
        // .route("/api/stripe/automatic-charge/{user_id}", post(stripe_handlers::automatic_charge))
        .route("/api/stripe/customer-portal/{user_id}", get(stripe_handlers::create_customer_portal_session))

        .route("/api/auth/google/calendar/login", get(google_calendar_auth::google_login))
        .route("/api/auth/google/calendar/connection", delete(google_calendar_auth::delete_google_calendar_connection))
        .route("/api/auth/google/calendar/status", get(google_calendar::google_calendar_status))
        .route("/api/auth/google/calendar/email", get(google_calendar::get_calendar_email))
        .route("/api/calendar/events", get(google_calendar::handle_calendar_fetching_route))

        .route("/api/auth/google/gmail/login", get(gmail_auth::gmail_login))
        .route("/api/auth/google/gmail/delete_connection", delete(gmail_auth::delete_gmail_connection))
        .route("/api/auth/google/gmail/refresh", post(gmail_auth::refresh_gmail_token))
        .route("/api/auth/google/gmail/test_fetch", get(gmail::test_gmail_fetch))
        .route("/api/auth/google/gmail/status", get(gmail::gmail_status))
        .route("/api/gmail/previews", get(gmail::fetch_email_previews))
        .route("/api/gmail/message/{id}", get(gmail::fetch_single_email))

        .route("/api/auth/imap/login", post(imap_auth::imap_login))
        .route("/api/auth/imap/status", get(imap_auth::imap_status))
        .route("/api/auth/imap/disconnect", delete(imap_auth::delete_imap_connection))
        .route("/api/imap/previews", get(imap_handlers::fetch_imap_previews))
        .route("/api/imap/message/{id}", get(imap_handlers::fetch_single_imap_email))
        .route("/api/imap/full_emails", get(imap_handlers::fetch_full_imap_emails))

        .route_layer(middleware::from_fn(handlers::auth_middleware::require_auth));


    let app = Router::new()
        .merge(public_routes)
        .merge(admin_routes)
        .merge(protected_routes)
        .merge(auth_built_in_webhook_routes)
        .route("/api/twiml", get(shazam_call::twiml_handler).post(shazam_call::twiml_handler))
        .route("/api/stream", get(shazam_call::stream_handler))
        .route("/api/listen/{call_sid}", get(shazam_call::listen_handler))
        .merge(twilio_routes)
        .merge(elevenlabs_routes)
        .merge(elevenlabs_webhook_routes)
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
                .allow_headers([axum::http::header::CONTENT_TYPE, axum::http::header::AUTHORIZATION])
                .expose_headers([axum::http::header::CONTENT_TYPE])
        )
        .with_state(state.clone());

    let state_for_scheduler = state.clone();
    tokio::spawn(async move {
        jobs::scheduler::start_scheduler(state_for_scheduler).await;
    });

    let shazam_state = crate::api::shazam_call::ShazamState {
        sessions: state.sessions.clone(),
        user_calls: state.user_calls.clone(),
        user_repository: state.user_repository.clone(),
        user_conversations: state.user_conversations.clone(),
    };
    tokio::spawn(async move {
        crate::api::shazam_call::process_audio_with_shazam(Arc::new(shazam_state)).await;
    });

    use tokio::net::TcpListener;
    let port = match std::env::var("ENVIRONMENT").as_deref() {
        Ok("staging") => 3100, // actually prod, but just saying staging
        _ => 3000,
    };
    tracing::info!("Starting server on port {}", port);
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).await.unwrap();
    axum::serve(listener, app.into_make_service()).await.unwrap();
}
