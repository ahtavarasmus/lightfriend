use anyhow::Result;
use axum::{routing::post, Router};
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use parent_proxy::{handlers, AppState, Config, Database};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Load configuration
    dotenvy::dotenv().ok();
    let config = Config::from_env()?;

    info!("Starting parent-proxy on port {}", config.listen_port);
    info!(
        "Old enclave: CID={}, Port={}",
        config.old_enclave_cid, config.old_enclave_port
    );
    info!(
        "New enclave: CID={}, Port={}",
        config.new_enclave_cid, config.new_enclave_port
    );

    // Initialize database connection
    let db = Database::new(&config.database_path)?;

    let listen_port = config.listen_port;
    let state = Arc::new(AppState { db, config });

    // Build router
    let app = Router::new()
        // Twilio SMS webhook
        .route("/api/sms/server", post(handlers::twilio_sms_webhook))
        // ElevenLabs voice webhook
        .route(
            "/api/webhook/elevenlabs",
            post(handlers::elevenlabs_webhook),
        )
        // Health check
        .route("/health", axum::routing::get(handlers::health_check))
        .with_state(state);

    let listener = TcpListener::bind(format!("0.0.0.0:{}", listen_port)).await?;
    info!("Parent proxy listening on port {}", listen_port);

    axum::serve(listener, app).await?;

    Ok(())
}
