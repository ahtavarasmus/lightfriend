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
use tower_http::cors::{CorsLayer, Any};
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
}

mod utils {
    pub mod encryption;
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
    oauth_client: GoogleOAuthClient,
    session_store: MemoryStore,
}

pub fn validate_env() {
    let required_vars = [
        "JWT_SECRET_KEY", "JWT_REFRESH_KEY", "DATABASE_URL", "PERPLEXITY_API_KEY",
        "ASSISTANT_ID", "ELEVENLABS_SERVER_URL_SECRET", "FIN_PHONE", "USA_PHONE",
        "NLD_PHONE", "CHZ_PHONE", "TWILIO_ACCOUNT_SID", "TWILIO_AUTH_TOKEN",
        "ENVIRONMENT", "FRONTEND_URL", "OPENROUTER_API_KEY", "STRIPE_CREDITS_PRODUCT_ID",
        "STRIPE_SECRET_KEY", "STRIPE_PUBLISHABLE_KEY", "STRIPE_WEBHOOK_SECRET",
        "MESSAGE_COST", "VOICE_SECOND_COST", "CHARGE_BACK_THRESHOLD", "SHAZAM_PHONE_NUMBER",
        "SHAZAM_API_KEY", "SERVER_URL", "ENCRYPTION_KEY", "COMPOSIO_API_KEY",
        "GOOGLE_CALENDAR_CLIENT_ID", "GOOGLE_CALENDAR_CLIENT_SECRET",
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
        .with_max_level(Level::INFO)
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

    let oauth_client = BasicClient::new(ClientId::new(client_id))
        .set_client_secret(ClientSecret::new(client_secret))
        .set_auth_uri(AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string()).expect("Invalid auth URL"))
        .set_token_uri(TokenUrl::new("https://oauth2.googleapis.com/token".to_string()).expect("Invalid token URL"))
        .set_redirect_uri(RedirectUrl::new(format!("{}/api/auth/google/callback", server_url_oauth)).expect("Invalid redirect URL"));

    println!("Redirect URI: {}/api/auth/google/callback", server_url_oauth);

    let session_store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(session_store.clone())
        .with_secure(false) // Set to true in production with HTTPS
        .with_same_site(tower_sessions::cookie::SameSite::Lax);

    let state = Arc::new(AppState {
        db_pool: pool,
        user_repository: user_repository.clone(),
        user_conversations: user_conversations.clone(),
        sessions: Arc::new(Mutex::new(HashMap::new())),
        user_calls: Arc::new(Mutex::new(HashMap::new())),
        oauth_client,
        session_store: session_store.clone(),
    });

    let twilio_routes = Router::new()
        .route("/api/sms/server", post(twilio_sms::handle_incoming_sms));
    let elevenlabs_routes = Router::new()
        .route("/api/call/assistant", post(elevenlabs::fetch_assistant))
        .route("/api/call/perplexity", post(elevenlabs::handle_perplexity_tool_call))
        .route("/api/call/sms", post(elevenlabs::handle_send_sms_tool_call))
        .route("/api/call/weather", post(elevenlabs::handle_weather_tool_call))
        .route("/api/call/shazam", get(elevenlabs::handle_shazam_tool_call))
        .route_layer(middleware::from_fn(elevenlabs::validate_elevenlabs_secret));
    let elevenlabs_webhook_routes = Router::new()
        .route("/api/webhook/elevenlabs", post(elevenlabs_webhook::elevenlabs_webhook))
        .route_layer(middleware::from_fn(elevenlabs_webhook::validate_elevenlabs_hmac));
    let google_calendar_routes = Router::new()
        .route("/api/auth/google/login", get(google_calendar_auth::google_login))
        .route("/api/auth/google/callback", get(google_calendar_auth::google_callback));

    let app = Router::new()
        .route("/api/health", get(health_check))
        .route("/api/login", post(auth_handlers::login))
        .route("/api/register", post(auth_handlers::register))
        .route("/api/admin/users", get(auth_handlers::get_users))
        .route("/api/admin/verify/{user_id}", post(admin_handlers::verify_user))
        .route("/api/admin/preferred-number/{user_id}", post(admin_handlers::update_preferred_number_admin))
        .route("/api/admin/broadcast", post(admin_handlers::broadcast_message))
        .route("/api/admin/set-preferred-number-default/{user_id}", post(admin_handlers::set_preferred_number_default))
        .route("/api/profile/delete/{user_id}", delete(profile_handlers::delete_user))
        .route("/api/profile/update", post(profile_handlers::update_profile))
        .route("/api/profile/preferred-number", post(profile_handlers::update_preferred_number))
        .route("/api/profile", get(profile_handlers::get_profile))
        .route("/api/profile/update-notify/{user_id}", post(profile_handlers::update_notify))
        .route("/api/billing/increase-credits/{user_id}", post(billing_handlers::increase_credits))
        .route("/api/billing/usage", post(billing_handlers::get_usage_data))
        .route("/api/billing/update-auto-topup/{user_id}", post(billing_handlers::update_topup))
        .route("/api/billing/reset-credits/{user_id}", post(billing_handlers::reset_credits))
        .route("/api/stripe/checkout-session/{user_id}", post(stripe_handlers::create_checkout_session))
        .route("/api/stripe/webhook", post(stripe_handlers::stripe_webhook))
        .route("/api/stripe/automatic-charge/{user_id}", post(stripe_handlers::automatic_charge))
        .route("/api/stripe/customer-portal/{user_id}", get(stripe_handlers::create_customer_portal_session))
        .route("/api/start-call/{user_id}", post(shazam_call::start_call_for_user))
        .route("/api/twiml", get(shazam_call::twiml_handler).post(shazam_call::twiml_handler))
        .route("/api/stream", get(shazam_call::stream_handler))
        .route("/api/listen/{call_sid}", get(shazam_call::listen_handler))
        .merge(google_calendar_routes)
        .merge(twilio_routes)
        .merge(elevenlabs_routes)
        .merge(elevenlabs_webhook_routes)
        .layer(session_layer) // Apply the session layer here
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO))
        )
        .layer(
            CorsLayer::new()
                .allow_methods([axum::http::Method::GET, axum::http::Method::POST, axum::http::Method::OPTIONS, axum::http::Method::DELETE])
                .allow_origin(Any) // Restrict in production
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
    let listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();
    axum::serve(listener, app.into_make_service()).await.unwrap();
}
