use dotenvy::dotenv;
use axum::{
    routing::{get, post, delete},
    Router,
    middleware
};
use tokio::sync::Mutex;
use std::collections::HashMap;
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
    pub mod billing_handlers;
    pub mod admin_handlers;
    pub mod stripe_handlers;
    pub mod oauth_handlers;
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
//use api::vapi_endpoints;
use api::twilio_sms;
use api::elevenlabs;
use api::elevenlabs_webhook;
use api::shazam_call;




type DbPool = r2d2::Pool<ConnectionManager<SqliteConnection>>;

async fn health_check() -> &'static str {
    "OK"
}

pub struct AppState {
    db_pool: DbPool,
    user_repository: Arc<UserRepository>,
    user_conversations: Arc<UserConversations>,
    sessions: shazam_call::CallSessions,
    user_calls: shazam_call::UserCallMap,
}

pub fn validate_env() {
    let _ = std::env::var("JWT_SECRET_KEY")
        .expect("JWT_SECRET_KEY must be set");
    let _ = std::env::var("JWT_REFRESH_KEY")
        .expect("JWT_REFRESH_KEY must be set");
    let _ = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    let _ = std::env::var("PERPLEXITY_API_KEY")
        .expect("PERPLEXITY_API_KEY must be set");
    let _ = std::env::var("ASSISTANT_ID")
        .expect("ASSISTANT_ID must be set");
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
    let _ = std::env::var("ENVIRONMENT") // for dev its 'development' and for prod anything else
        .expect("ENVIRONMENT must be set");
    let _ = std::env::var("FRONTEND_URL") // frontend url
        .expect("FRONTEND_URL must be set");
    let _ = std::env::var("OPENROUTER_API_KEY") 
        .expect("OPENROUTER_API_KEY must be set");
    let _ = std::env::var("STRIPE_CREDITS_PRODUCT_ID")
        .expect("STRIPE_CREDITS_PRODUCT_ID must be set");
    let _ = std::env::var("STRIPE_SECRET_KEY")
        .expect("STRIPE_SECRET_KEY must be set");
    let _ = std::env::var("STRIPE_PUBLISHABLE_KEY")
        .expect("STRIPE_PUBLISHABLE_KEY must be set");
    let _ = std::env::var("STRIPE_WEBHOOK_SECRET")
        .expect("STRIPE_WEBHOOK_SECRET must be set");

    let _ = std::env::var("MESSAGE_COST")
        .expect("MESSAGE_COST must be set");
    let _ = std::env::var("VOICE_SECOND_COST")
        .expect("VOICE_SECOND_COST must be set");
    let _ = std::env::var("CHARGE_BACK_THRESHOLD")
        .expect("CHARGE_BACK_THRESHOLD must be set");
    let _ = std::env::var("TWILIO_ACCOUNT_SID")
        .expect("TWILIO_ACCOUNT_SID must be set");
    let _ = std::env::var("TWILIO_AUTH_TOKEN")
        .expect("TWILIO_AUTH_TOKEN must be set");
    let _ = std::env::var("SHAZAM_PHONE_NUMBER")
        .expect("SHAZAM_PHONE_NUMBER must be set");
    let _ = std::env::var("SHAZAM_API_KEY")
        .expect("SHAZAM_API_KEY must be set");
    let _ = std::env::var("SERVER_URL")
        .expect("SERVER_URL must be set");
    let _ = std::env::var("ENCRYPTION_KEY")
        .expect("ENCRYPTION_KEY must be set");
    let _ = std::env::var("COMPOSIO_API_KEY")
        .expect("COMPOSIO_API_KEY must be set");

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

    let _conn = &mut pool.get().expect("Failed to get DB connection");

    let state = Arc::new(AppState {
        db_pool: pool,
        user_repository: user_repository.clone(),
        user_conversations: user_conversations.clone(),
        sessions: Arc::new(Mutex::new(HashMap::new())),
        user_calls: Arc::new(Mutex::new(HashMap::new())),
    });



    // keep the vapi around if they become cracked at some point
    /*
    let vapi_routes = Router::new()
        .route("/api/vapi/server", post(vapi_endpoints::handle_phone_call_event))
        .route_layer(middleware::from_fn(vapi_endpoints::validate_vapi_secret));
    */
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
    /*

    let oauth_handler_routes= Router::new()
        .route("/auth-params", get(oauth_handlers::fetch_auth_params))
        .route("/initiate-connection", post(oauth_handlers::initiate_connection));
    */


    // Create router with CORS
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
        // Shazam routes
        .route("/api/start-call/{user_id}", post(shazam_call::start_call_for_user))
        .route("/api/twiml", get(shazam_call::twiml_handler).post(shazam_call::twiml_handler))
        .route("/api/stream", get(shazam_call::stream_handler))
        .route("/api/listen/{call_sid}", get(shazam_call::listen_handler))



        /*
        .merge(vapi_routes)
        .merge(oauth_handler_routes)
        */
        .merge(twilio_routes)
        .merge(elevenlabs_routes)
        .merge(elevenlabs_webhook_routes)
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
                .allow_origin(Any) // Be cautious with `Any` in production; restrict to your frontend origin
                .allow_headers([axum::http::header::CONTENT_TYPE, axum::http::header::AUTHORIZATION]) // Explicitly allow Authorization
                .expose_headers([axum::http::header::CONTENT_TYPE])
        )
        .with_state(state.clone());
    // Spawn the scheduler
    let state_for_scheduler = state.clone();
    // Start the scheduler
    tokio::spawn(async move {
        jobs::scheduler::start_scheduler(state_for_scheduler).await;
    });

    // Spawn the Shazam audio processing task
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

