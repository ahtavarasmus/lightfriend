use dotenvy::dotenv;
use axum::{
    routing::{get, post, delete},
    Router,
    middleware
};
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use tower_http::cors::{CorsLayer, Any};
use tower_http::trace::{TraceLayer, DefaultMakeSpan, DefaultOnResponse};
use tracing::Level;
use std::sync::Arc;
use sentry;

mod handlers {
    pub mod auth_handlers;
    pub mod auth_dtos;
    pub mod profile_handlers;
}
mod api {
    pub mod vapi_endpoints;
    pub mod vapi_dtos;
    pub mod twilio_sms;
    pub mod twilio_utils;
    pub mod elevenlabs;
    pub mod paddle_webhooks;
    pub mod paddle_utils;
}

mod config {
    pub mod phone_numbers;
}

mod models {
    pub mod user_models;
}
mod repositories {
    pub mod user_repository;
    pub mod user_conversations;
    pub mod user_subscriptions;
}
mod schema;
mod jobs {
    pub mod scheduler;
}


use repositories::user_repository::UserRepository;
use repositories::user_conversations::UserConversations;
use repositories::user_subscriptions::UserSubscription;

use handlers::auth_handlers;
use handlers::profile_handlers;
use api::vapi_endpoints;
use api::twilio_sms;
use api::elevenlabs;
use api::paddle_webhooks;




type DbPool = r2d2::Pool<ConnectionManager<SqliteConnection>>;

async fn health_check() -> &'static str {
    "OK"
}

pub struct AppState {
    db_pool: DbPool,
    user_repository: Arc<UserRepository>,
    user_conversations: Arc<UserConversations>,
    user_subscriptions: Arc<UserSubscription>,
}

pub fn validate_env() {
    let _ = std::env::var("JWT_SECRET_KEY")
        .expect("JWT_SECRET_KEY must be set");
    let _ = std::env::var("JWT_REFRESH_KEY")
        .expect("JWT_REFRESH_KEY must be set");
    let _ = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    let _ = std::env::var("VAPI_API_KEY")
        .expect("VAPI_API_KEY must be set");
    let _ = std::env::var("PERPLEXITY_API_KEY")
        .expect("PERPLEXITY_API_KEY must be set");
    let _ = std::env::var("ASSISTANT_ID")
        .expect("ASSISTANT_ID must be set");
    let _ = std::env::var("VAPI_SERVER_URL_SECRET")
        .expect("VAPI_SERVER_URL_SECRET must be set");
    let _ = std::env::var("ELEVENLABS_SERVER_URL_SECRET")
        .expect("ELEVENLABS_SERVER_URL_SECRET must be set");
    let _ = std::env::var("FIN_PHONE")
        .expect("FIN_PHONE must be set");
    let _ = std::env::var("USA_PHONE")
        .expect("USA_PHONE must be set");
    let _ = std::env::var("NLD_PHONE")
        .expect("NLD_PHONE must be set");
    let _ = std::env::var("CHZ_PHONE")
        .expect("CHZ_PHONE must be set");
    let _ = std::env::var("TWILIO_ACCOUNT_SID")
        .expect("TWILIO_ACCOUNT_SID must be set");
    let _ = std::env::var("TWILIO_AUTH_TOKEN")
        .expect("TWILIO_AUTH_TOKEN must be set");
    let _ = std::env::var("PADDLE_WEBHOOK_SECRET")
        .expect("PADDLE_WEBHOOK_SECRET must be set");
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    validate_env();

    let _guard = sentry::init(("https://07fbdaf63c1270c8509844b775045dd3@o4507415184539648.ingest.de.sentry.io/4508802101411920", sentry::ClientOptions {
        release: sentry::release_name!(),
        ..Default::default()
    }));

    // Sentry will capture this
    // panic!("Everything is on fire!");

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();
    // Set up database connection pool
    let manager = ConnectionManager::<SqliteConnection>::new("database.db");
    let pool = r2d2::Pool::builder()
        .build(manager)
        .expect("Failed to create pool");

    let user_repository = Arc::new(UserRepository::new(pool.clone()));
    let user_conversations = Arc::new(UserConversations::new(pool.clone()));
    let user_subscriptions = Arc::new(UserSubscription::new(pool.clone()));

    let _conn = &mut pool.get().expect("Failed to get DB connection");

    let state = Arc::new(AppState {
        db_pool: pool,
        user_repository,
        user_conversations,
        user_subscriptions,
    });

    // Create a router for VAPI routes with secret validation
    let vapi_routes = Router::new()
        .route("/api/vapi/server", post(vapi_endpoints::handle_phone_call_event))
        .route_layer(middleware::from_fn(vapi_endpoints::validate_vapi_secret));
    let twilio_routes = Router::new()
        .route("/api/sms/server", post(twilio_sms::handle_incoming_sms));
    let elevenlabs_routes = Router::new()
        .route("/api/call/perplexity", post(elevenlabs::handle_perplexity_tool_call))
        .route("/api/call/assistant", post(elevenlabs::fetch_assistant))
        .route_layer(middleware::from_fn(elevenlabs::validate_elevenlabs_secret));

    let paddle_routes = Router::new()
        .route("/api/webhooks/paddle", post(paddle_webhooks::handle_subscription_webhook))
        .route_layer(middleware::from_fn(paddle_webhooks::validate_paddle_secret));


    // Create router with CORS
    let app = Router::new()
        .route("/api/health", get(health_check))
        .route("/api/login", post(auth_handlers::login))
        .route("/api/register", post(auth_handlers::register))
        .route("/api/admin/users", get(auth_handlers::get_users))

        .route("/api/admin/verify/{user_id}", post(auth_handlers::verify_user))
        .route("/api/admin/preferred-number/{user_id}", post(auth_handlers::update_preferred_number_admin))
        .route("/api/admin/broadcast", post(auth_handlers::broadcast_message))
        .route("/api/admin/set-preferred-number-default/{user_id}", post(auth_handlers::set_preferred_number_default))
        .route("/api/profile/update", post(profile_handlers::update_profile))
        .route("/api/profile/preferred-number", post(profile_handlers::update_preferred_number))
        .route("/api/profile", get(profile_handlers::get_profile))
        .route("/api/profile/delete/{user_id}", delete(auth_handlers::delete_user))
        .route("/api/profile/increase-iq/{user_id}", post(profile_handlers::increase_iq))
        .route("/api/profile/reset-iq/{user_id}", post(profile_handlers::reset_iq))
        .route("/api/profile/notify-credits/{user_id}", post(profile_handlers::update_notify_credits))
        .route("/api/profile/usage", post(profile_handlers::get_usage_data))
        .route("/api/profile/get-customer-portal-link/{user_id}", get(profile_handlers::get_customer_portal_link))


        .merge(vapi_routes)
        .merge(twilio_routes)
        .merge(elevenlabs_routes)
        .merge(paddle_routes)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO))
        )
        .layer(
            CorsLayer::new()
                .allow_methods([
                    axum::http::Method::GET,
                    axum::http::Method::POST,
                    axum::http::Method::OPTIONS,
                    axum::http::Method::DELETE,
                ])
                .allow_origin(Any)
                .allow_headers(Any)
                .expose_headers([axum::http::header::CONTENT_TYPE])
        )
        .with_state(state.clone());
    // Start the scheduler
    let state_for_scheduler = state;
    tokio::spawn(async move {
        jobs::scheduler::start_scheduler(state_for_scheduler).await;
    });

    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:3000").await.unwrap();
    axum::serve(listener, app.into_make_service()).await.unwrap();
}

