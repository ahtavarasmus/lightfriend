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
    pub mod twilio;
}

mod config {
    pub mod phone_numbers;
}

mod models {
    pub mod user_models;
}
mod repositories {
    pub mod user_repository;
}
mod schema;


use repositories::user_repository::UserRepository;

use handlers::auth_handlers::{register, login, get_users, delete_user, verify_user, broadcast_message};
use handlers::profile_handlers::{get_profile, update_profile, increase_iq, reset_iq, update_notify_credits};
use api::vapi_endpoints::{vapi_server, handle_phone_call_event, handle_phone_call_event_print};

type DbPool = r2d2::Pool<ConnectionManager<SqliteConnection>>;

async fn health_check() -> &'static str {
    "OK"
}

pub struct AppState {
    db_pool: DbPool,
    user_repository: Arc<UserRepository>,
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

    let _conn = &mut pool.get().expect("Failed to get DB connection");

    let state = Arc::new(AppState {
        db_pool: pool,
        user_repository,
    });

    // Create a router for VAPI routes with secret validation
    let vapi_routes = Router::new()
        .route("/api/server", post(handle_phone_call_event))
        .route_layer(middleware::from_fn(api::vapi_endpoints::validate_vapi_secret));


    // Create router with CORS
    let app = Router::new()
        .route("/api/health", get(health_check))
        .route("/api/login", post(login))
        .route("/api/register", post(register))
        .route("/api/admin/users", get(get_users))
        .route("/api/admin/verify/:user_id", post(verify_user))
        .route("/api/admin/broadcast", post(broadcast_message))
        .route("/api/profile/update", post(update_profile))
        .route("/api/profile", get(get_profile))
        .route("/api/profile/delete/:user_id", delete(delete_user))
        .route("/api/profile/increase-iq/:user_id", post(increase_iq))
        .route("/api/profile/reset-iq/:user_id", post(reset_iq))
        .route("/api/profile/notify-credits/:user_id", post(update_notify_credits))
        .merge(vapi_routes)
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
        .with_state(state);
    // Start server
    axum::Server::bind(&"127.0.0.1:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

