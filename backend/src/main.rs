use dotenvy::dotenv;
use axum::{
    routing::{get, post},
    Router,
};
use diesel::prelude::*;
use diesel::r2d2::{self, ConnectionManager};
use tower_http::cors::{CorsLayer, Any};
use tower_http::trace::{TraceLayer, DefaultMakeSpan, DefaultOnResponse};
use tracing::Level;
use std::sync::Arc;

mod handlers {
    pub mod auth_handlers;
    pub mod auth_dtos;
    pub mod profile_handlers;
}
mod models {
    pub mod user_models;
}
mod repositories {
    pub mod user_repository;
}
mod schema;

use repositories::user_repository::UserRepository;

use handlers::auth_handlers::{register, login, get_users};
use handlers::profile_handlers::{get_profile, update_profile};

type DbPool = r2d2::Pool<ConnectionManager<SqliteConnection>>;

pub struct AppState {
    db_pool: DbPool,
    user_repository: Arc<UserRepository>,
}

pub fn validate_env() {
    let _ = std::env::var("JWT_SECRET_KEY")
        .expect("JWT_SECRET_KEY must be set");
    let _ = std::env::var("JWT_REFRESH_KEY")
        .expect("JWT_REFRESH_KEY must be set");
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    validate_env();
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

    // Create router with CORS
    let app = Router::new()
        .route("/api/login", post(login))
        .route("/api/register", post(register))
        .route("/api/admin/users", get(get_users))
        .route("/api/profile/update", post(update_profile))
        .route("/api/profile", get(get_profile))
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
                    axum::http::Method::OPTIONS
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

