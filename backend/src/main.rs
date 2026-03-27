use axum::body::{to_bytes, Body};
use axum::extract::OriginalUri;
use axum::http::{HeaderValue, Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{
    extract::DefaultBodyLimit,
    middleware,
    routing::{any, delete, get, patch, post, put},
    Router,
};
use dashmap::DashMap;
use diesel::r2d2::{self, ConnectionManager};
use oauth2::{basic::BasicClient, AuthUrl, ClientId, ClientSecret, RedirectUrl, TokenUrl};
use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tower_sessions::{MemoryStore, SessionManagerLayer};
use tracing::Level;

// Import modules and types from library crate
use api::{elevenlabs, elevenlabs_webhook, twilio_sms};
use backend::{
    api, handlers, jobs, utils, AdminAlertRepository, AiConfig, AppState, LlmUsageRepository,
    TotpRepository, UserCore, UserCoreOps, UserRepository, WebauthnRepository,
};
use handlers::{
    admin_handlers, attestation_handlers, auth_handlers, billing_handlers, bridge_auth_common,
    dashboard_handlers, imap_auth, imap_handlers, person_handlers, profile_handlers, rule_handlers,
    self_host_handlers, signal_auth, signal_handlers, stripe_handlers, telegram_auth,
    telegram_handlers, tesla_auth, twilio_handlers, whatsapp_auth, whatsapp_handlers, youtube,
    youtube_auth,
};

async fn health_check() -> &'static str {
    "OK"
}

const TELEGRAM_PUBLIC_BASE_URL: &str = "http://127.0.0.1:29317";
const TELEGRAM_PUBLIC_PROXY_BODY_LIMIT: usize = 10 * 1024 * 1024;

async fn proxy_telegram_public(original_uri: OriginalUri, request: Request<Body>) -> Response {
    let path_and_query = match original_uri.0.path_and_query() {
        Some(value) => value.as_str(),
        None => return StatusCode::BAD_REQUEST.into_response(),
    };
    let target_url = format!("{TELEGRAM_PUBLIC_BASE_URL}{path_and_query}");
    let method = request.method().clone();
    let request_headers = request.headers().clone();
    let body = match to_bytes(request.into_body(), TELEGRAM_PUBLIC_PROXY_BODY_LIMIT).await {
        Ok(body) => body,
        Err(error) => {
            tracing::warn!("Failed to read proxied Telegram public request body: {error}");
            return (
                StatusCode::BAD_REQUEST,
                "Failed to read Telegram public request body",
            )
                .into_response();
        }
    };

    let client = reqwest::Client::new();
    let mut upstream_request = client.request(method, &target_url);
    for (name, value) in request_headers.iter() {
        let key = name.as_str();
        if matches!(
            key,
            "host" | "connection" | "content-length" | "transfer-encoding" | "keep-alive"
        ) {
            continue;
        }
        upstream_request = upstream_request.header(name, value);
    }
    let upstream_response = match upstream_request.body(body).send().await {
        Ok(response) => response,
        Err(error) => {
            tracing::error!("Failed to proxy Telegram public request to {target_url}: {error}");
            return (
                StatusCode::BAD_GATEWAY,
                "Telegram login page is currently unavailable",
            )
                .into_response();
        }
    };

    let status = upstream_response.status();
    let upstream_headers = upstream_response.headers().clone();
    let response_body = match upstream_response.bytes().await {
        Ok(body) => body,
        Err(error) => {
            tracing::error!("Failed to read proxied Telegram public response body: {error}");
            return (
                StatusCode::BAD_GATEWAY,
                "Telegram login page returned an invalid response",
            )
                .into_response();
        }
    };

    let mut response = Response::builder().status(status);
    for (name, value) in upstream_headers.iter() {
        let key = name.as_str();
        if matches!(
            key,
            "connection" | "content-length" | "transfer-encoding" | "keep-alive"
        ) {
            continue;
        }
        response = response.header(name, value);
    }

    match response.body(Body::from(response_body)) {
        Ok(response) => response,
        Err(error) => {
            tracing::error!("Failed to build proxied Telegram public response: {error}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to build Telegram login response",
            )
                .into_response()
        }
    }
}

pub fn validate_env() {
    // Core variables (always required regardless of environment)
    let core_vars = [
        "JWT_SECRET_KEY",
        "JWT_REFRESH_KEY",
        "PG_DATABASE_URL",
        "ENCRYPTION_KEY",
        "MATRIX_SHARED_SECRET",
    ];

    for var in core_vars.iter() {
        std::env::var(var).unwrap_or_else(|_| panic!("{} must be set", var));
    }

    // Production-only validation for live application features
    let environment = std::env::var("ENVIRONMENT").unwrap_or_else(|_| "development".to_string());

    if environment == "production" {
        let production_vars = [
            // Billing (Stripe)
            "STRIPE_SECRET_KEY",
            "STRIPE_PUBLISHABLE_KEY",
            "STRIPE_WEBHOOK_SECRET",
            "STRIPE_CREDITS_PRODUCT_ID",
            // SMS/Voice (Twilio)
            "TWILIO_ACCOUNT_SID",
            "TWILIO_AUTH_TOKEN",
            // Voice AI (ElevenLabs)
            "ELEVENLABS_SERVER_URL_SECRET",
            // Regional phone numbers
            "FIN_PHONE",
            "USA_PHONE",
            "AUS_PHONE",
            "GB_PHONE",
            "NL_PHONE",
            "CAN_PHONE",
            // Production server config
            "SERVER_URL",
            "ASSISTANT_ID",
        ];

        for var in production_vars.iter() {
            std::env::var(var)
                .unwrap_or_else(|_| panic!("{} must be set in production environment", var));
        }
    }

    // Note: The following are truly optional even in production:
    // - PERPLEXITY_API_KEY, OPENROUTER_API_KEY (AI features gracefully degrade)
    // - Bridge bot IDs (may have defaults)
}

/// Bootstrap an admin user on first startup if the database is empty.
/// This is safe to call on every startup - it only creates a user if:
/// 1. No users exist in the database
/// 2. ADMIN_EMAILS env var is set (uses first email from the list)
///
/// CRITICAL: This never overrides existing users.
async fn bootstrap_admin_if_needed(
    user_core: &Arc<UserCore>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use backend::handlers::auth_dtos::NewUser;

    // Check if any users exist - if so, skip bootstrap entirely
    let users = user_core.get_all_users()?;
    if !users.is_empty() {
        tracing::info!(
            "Database has {} users, skipping admin bootstrap",
            users.len()
        );
        return Ok(());
    }

    // Get first email from ADMIN_EMAILS list for bootstrap
    let admin_emails =
        std::env::var("ADMIN_EMAILS").expect("ADMIN_EMAILS environment variable is required");

    let email = admin_emails
        .split(',')
        .next()
        .map(|e| e.trim().to_string())
        .filter(|e| !e.is_empty())
        .expect("ADMIN_EMAILS must contain at least one email");

    let password = std::env::var("BOOTSTRAP_ADMIN_PASSWORD")
        .expect("BOOTSTRAP_ADMIN_PASSWORD environment variable is required for admin bootstrap");

    let phone = std::env::var("BOOTSTRAP_ADMIN_PHONE").unwrap_or_else(|_| "+12345678".to_string());

    // Hash the password
    let password_hash = bcrypt::hash(&password, bcrypt::DEFAULT_COST)?;

    // Create the admin user
    let new_user = NewUser {
        email: email.clone(),
        password_hash,
        phone_number: phone.clone(),
        time_to_live: 60,
        credits: 1000.0,
        credits_left: 1000.0,
        charge_when_under: false,
        sub_tier: Some("2".to_string()), // tier 2 = sentinel (full access)
    };

    match user_core.create_user(new_user) {
        Ok(()) => {
            tracing::info!("✓ Bootstrap admin created: {} (phone={})", email, phone);
            Ok(())
        }
        Err(e) => {
            tracing::error!("Failed to create bootstrap admin: {}", e);
            Err(Box::new(std::io::Error::other(e.to_string())))
        }
    }
}

#[tokio::main]
async fn main() {
    // Check for CLI commands first
    match backend::cli::run_cli().await {
        Ok(true) => return,
        Ok(false) => {}
        Err(e) => {
            eprintln!("CLI error: {}", e);
            std::process::exit(1);
        }
    }

    let _guard = std::env::var("SENTRY_DSN").ok().map(|dsn| {
        sentry::init((
            dsn,
            sentry::ClientOptions {
                release: sentry::release_name!(),
                ..Default::default()
            },
        ))
    });
    use tracing_subscriber::{fmt, EnvFilter};

    // Create filter that sets Matrix SDK logs to WARN and keeps our app at DEBUG
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("info,lightfriend=debug")
            .add_directive("matrix_sdk=error".parse().unwrap()) // Changed from warn to error
            .add_directive("tokio-runtime-worker=off".parse().unwrap())
            .add_directive("ruma=warn".parse().unwrap())
            .add_directive("eyeball=warn".parse().unwrap())
            .add_directive("matrix_sdk::encryption=error".parse().unwrap()) // Added specific filter for encryption module
    });
    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    // Validate required ADMIN_EMAILS env var early
    let admin_emails =
        std::env::var("ADMIN_EMAILS").expect("ADMIN_EMAILS environment variable is required");
    let admin_list: Vec<&str> = admin_emails
        .split(',')
        .map(|e| e.trim())
        .filter(|e| !e.is_empty())
        .collect();
    if admin_list.is_empty() {
        panic!("ADMIN_EMAILS must contain at least one email address");
    }
    tracing::info!("Admin emails configured: {:?}", admin_list);

    let pg_database_url =
        std::env::var("PG_DATABASE_URL").expect("PG_DATABASE_URL must be set in environment");
    let pg_manager = ConnectionManager::<diesel::PgConnection>::new(pg_database_url);
    let pg_pool = r2d2::Pool::builder()
        .build(pg_manager)
        .expect("Failed to create PG pool");

    // Run PG migrations
    {
        use diesel_migrations::{EmbeddedMigrations, MigrationHarness};
        const PG_MIGRATIONS: EmbeddedMigrations =
            diesel_migrations::embed_migrations!("pg_migrations");
        let mut pg_conn = pg_pool.get().expect("Failed to get PG connection");
        pg_conn
            .run_pending_migrations(PG_MIGRATIONS)
            .expect("Failed to run PG migrations");
        tracing::info!("PG migrations applied successfully");
    }

    let user_core = Arc::new(UserCore::new(pg_pool.clone()));

    // Bootstrap admin user on first startup (only if database is empty)
    if let Err(e) = bootstrap_admin_if_needed(&user_core).await {
        tracing::warn!("Admin bootstrap failed (app will continue): {}", e);
    }

    let user_repository = Arc::new(UserRepository::new(pg_pool.clone()));
    let totp_repository = Arc::new(TotpRepository::new(pg_pool.clone()));
    let webauthn_repository = Arc::new(WebauthnRepository::new(pg_pool.clone()));
    let admin_alert_repository = Arc::new(AdminAlertRepository::new(pg_pool.clone()));
    let metrics_repository = Arc::new(backend::MetricsRepository::new(pg_pool.clone()));
    let llm_usage_repository = Arc::new(LlmUsageRepository::new(pg_pool.clone()));
    let ontology_repository = Arc::new(backend::OntologyRepository::new(pg_pool.clone()));
    let server_url_oauth =
        std::env::var("SERVER_URL_OAUTH").unwrap_or_else(|_| "http://localhost:3000".to_string());
    let server_url =
        std::env::var("SERVER_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());
    let session_store = MemoryStore::default();
    let is_prod = std::env::var("ENVIRONMENT") != Ok("development".to_string());
    let session_layer = SessionManagerLayer::new(session_store.clone())
        .with_secure(is_prod)
        .with_same_site(tower_sessions::cookie::SameSite::Lax);

    // Tesla OAuth client
    let tesla_client_id = std::env::var("TESLA_CLIENT_ID")
        .unwrap_or_else(|_| "default-tesla-client-id-for-testing".to_string());
    let tesla_client_secret = std::env::var("TESLA_CLIENT_SECRET")
        .unwrap_or_else(|_| "default-tesla-secret-for-testing".to_string());
    let tesla_redirect_url =
        std::env::var("TESLA_REDIRECT_URL").unwrap_or_else(|_| server_url.clone());
    let tesla_oauth_client = BasicClient::new(ClientId::new(tesla_client_id))
        .set_client_secret(ClientSecret::new(tesla_client_secret))
        .set_auth_uri(
            AuthUrl::new("https://auth.tesla.com/oauth2/v3/authorize".to_string())
                .expect("Invalid auth URL"),
        )
        .set_token_uri(
            TokenUrl::new("https://fleet-auth.prd.vn.cloud.tesla.com/oauth2/v3/token".to_string())
                .expect("Invalid token URL"),
        )
        .set_redirect_uri(
            RedirectUrl::new(format!("{}/api/auth/tesla/callback", tesla_redirect_url))
                .expect("Invalid redirect URL"),
        );

    // YouTube OAuth client
    let youtube_client_id = std::env::var("YOUTUBE_CLIENT_ID")
        .unwrap_or_else(|_| "default-youtube-client-id-for-testing".to_string());
    let youtube_client_secret = std::env::var("YOUTUBE_CLIENT_SECRET")
        .unwrap_or_else(|_| "default-youtube-secret-for-testing".to_string());
    let youtube_oauth_client = BasicClient::new(ClientId::new(youtube_client_id))
        .set_client_secret(ClientSecret::new(youtube_client_secret))
        .set_auth_uri(
            AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())
                .expect("Invalid auth URL"),
        )
        .set_token_uri(
            TokenUrl::new("https://oauth2.googleapis.com/token".to_string())
                .expect("Invalid token URL"),
        )
        .set_redirect_uri(
            RedirectUrl::new(format!("{}/api/auth/youtube/callback", server_url_oauth))
                .expect("Invalid redirect URL"),
        );

    let matrix_sync_tasks = Arc::new(Mutex::new(HashMap::new()));
    let matrix_clients = Arc::new(Mutex::new(HashMap::new()));
    let twilio_client = Arc::new(backend::RealTwilioClient::new());
    let twilio_message_service = Arc::new(backend::TwilioMessageService::new(
        twilio_client.clone(),
        pg_pool.clone(),
        user_core.clone(),
        user_repository.clone(),
    ));
    let state = Arc::new(AppState {
        pg_pool,
        user_core: user_core.clone(),
        user_repository: user_repository.clone(),
        twilio_client,
        twilio_message_service,
        ai_config: AiConfig::from_env(),
        tesla_oauth_client,
        youtube_oauth_client,
        session_store: session_store.clone(),
        login_limiter: DashMap::new(),
        password_reset_limiter: DashMap::new(),
        password_reset_verify_limiter: DashMap::new(),
        api_rate_limiter: DashMap::new(),
        password_reset_otps: DashMap::new(),
        phone_verify_otps: DashMap::new(),
        matrix_sync_tasks,
        matrix_clients,
        tesla_monitoring_tasks: Arc::new(DashMap::new()),
        tesla_charging_monitor_tasks: Arc::new(DashMap::new()),
        tesla_waking_vehicles: Arc::new(DashMap::new()),
        phone_verify_limiter: DashMap::new(),
        phone_verify_verify_limiter: DashMap::new(),
        pending_message_senders: Arc::new(Mutex::new(HashMap::new())),
        totp_repository,
        webauthn_repository,
        admin_alert_repository,
        metrics_repository,
        pending_totp_logins: DashMap::new(),
        pending_password_resets: DashMap::new(),
        session_to_token: DashMap::new(),
        totp_verify_limiter: DashMap::new(),
        webauthn_verify_limiter: DashMap::new(),
        llm_usage_repository,
        ontology_repository,
        ontology_registry: backend::ontology::registry::OntologyRegistry::build(),
        tool_registry: backend::build_tool_registry(),
        pending_rule_tests: Arc::new(DashMap::new()),
        maintenance_mode: Arc::new(AtomicBool::new(false)),
    });
    // SMS server route - validates signature using user lookup
    let twilio_sms_routes = Router::new()
        .route("/api/sms/server", post(twilio_sms::handle_regular_sms))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            api::twilio_utils::validate_twilio_signature,
        ));
    // Status callback route - validates signature using main Twilio account
    let twilio_status_routes = Router::new()
        .route(
            "/api/twilio/status-callback",
            post(twilio_handlers::twilio_status_callback),
        )
        .layer(middleware::from_fn(
            api::twilio_utils::validate_twilio_status_callback_signature,
        ));
    let twilio_routes = twilio_sms_routes.merge(twilio_status_routes);
    let textbee_routes = Router::new().route(
        "/api/sms/textbee-server",
        post(twilio_sms::handle_textbee_sms),
    );
    // textbee requests are validated using device_id and phone number combo
    let elevenlabs_free_routes = Router::new()
        .route("/api/call/assistant", post(elevenlabs::fetch_assistant))
        .route(
            "/api/call/weather",
            post(elevenlabs::handle_weather_tool_call),
        )
        .route(
            "/api/call/perplexity",
            post(elevenlabs::handle_perplexity_tool_call),
        )
        .route_layer(middleware::from_fn(elevenlabs::validate_elevenlabs_secret));
    let elevenlabs_routes = Router::new()
        .route("/api/call/sms", post(elevenlabs::handle_send_sms_tool_call))
        .route(
            "/api/call/email",
            get(elevenlabs::handle_email_fetch_tool_call),
        )
        .route(
            "/api/call/email/specific",
            post(elevenlabs::handle_email_search_tool_call),
        )
        .route(
            "/api/call/email/respond",
            post(elevenlabs::handle_respond_to_email),
        )
        .route("/api/call/email/send", post(elevenlabs::handle_email_send))
        .route(
            "/api/call/cancel-message",
            get(elevenlabs::handle_cancel_pending_message_tool_call),
        )
        .route(
            "/api/call/fetch-recent-messages",
            get(elevenlabs::handle_fetch_recent_messages_tool_call),
        )
        .route(
            "/api/call/fetch-chat-messages",
            get(elevenlabs::handle_fetch_specific_chat_messages_tool_call),
        )
        .route(
            "/api/call/search-chat-contacts",
            post(elevenlabs::handle_search_chat_contacts_tool_call),
        )
        .route(
            "/api/call/send-chat-message",
            post(elevenlabs::handle_send_chat_message),
        )
        .route(
            "/api/call/firecrawl",
            post(elevenlabs::handle_firecrawl_tool_call),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            handlers::auth_middleware::check_subscription_access,
        ))
        .route_layer(middleware::from_fn(elevenlabs::validate_elevenlabs_secret));
    let elevenlabs_webhook_routes = Router::new()
        .route(
            "/api/webhook/elevenlabs",
            post(elevenlabs_webhook::elevenlabs_webhook),
        )
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            elevenlabs_webhook::validate_elevenlabs_hmac,
        ));
    let auth_built_in_webhook_routes = Router::new()
        .route("/api/stripe/webhook", post(stripe_handlers::stripe_webhook))
        .route("/api/auth/tesla/callback", get(tesla_auth::tesla_callback))
        .route(
            "/api/auth/youtube/callback",
            get(youtube_auth::youtube_callback),
        );
    // Public routes that don't need authentication. there's ratelimiting though
    let public_routes = Router::new()
        .route("/api/health", get(health_check))
        .route(
            "/.well-known/lightfriend/attestation",
            get(attestation_handlers::attestation_metadata),
        )
        .route(
            "/.well-known/lightfriend/attestation/raw",
            get(attestation_handlers::attestation_raw),
        )
        .route(
            "/.well-known/lightfriend/attestation/hex",
            get(attestation_handlers::attestation_hex),
        )
        .route("/api/unsubscribe", get(admin_handlers::unsubscribe))
        .route("/api/login", post(auth_handlers::login))
        .route("/api/register", post(auth_handlers::register))
        .route("/api/logout", post(auth_handlers::logout))
        .route("/api/auth/refresh", post(auth_handlers::refresh_token))
        .route(
            "/api/password-reset/validate/{token}",
            get(auth_handlers::validate_reset_token),
        )
        .route(
            "/api/password-reset/complete",
            post(auth_handlers::complete_password_reset),
        )
        .route(
            "/api/phone-verify/request",
            post(auth_handlers::request_phone_verify),
        )
        .route(
            "/api/phone-verify/verify",
            post(auth_handlers::verify_phone_verify),
        )
        .route(
            "/api/pricing/byot/{country_code}",
            get(handlers::pricing_handlers::get_byot_country_pricing),
        )
        .route(
            "/api/pricing/all-countries",
            get(handlers::pricing_handlers::get_all_countries),
        )
        .route(
            "/api/pricing/country/{country_code}",
            get(handlers::pricing_handlers::get_single_country_pricing),
        )
        .route(
            "/api/totp/verify",
            post(handlers::totp_handlers::verify_login),
        )
        // WebAuthn public routes (for login flow)
        .route(
            "/api/webauthn/login/start",
            post(handlers::webauthn_handlers::login_start),
        )
        .route(
            "/api/webauthn/verify-login",
            post(handlers::webauthn_handlers::verify_login),
        )
        // Magic link and guest checkout routes (subscribe-first flow)
        .route(
            "/api/stripe/guest-checkout",
            post(stripe_handlers::create_guest_checkout),
        )
        .route(
            "/api/auth/magic/{token}",
            get(auth_handlers::validate_magic_link),
        )
        .route(
            "/api/auth/session-token/{session_id}",
            get(auth_handlers::get_token_from_session),
        )
        .route(
            "/api/auth/set-password",
            post(auth_handlers::set_password_from_magic_link),
        )
        .route("/api/waitlist", post(auth_handlers::add_to_waitlist))
        // Public stats endpoint
        .route(
            "/api/stats/smartphone-free-days",
            get(handlers::stats_handlers::get_smartphone_free_days),
        );
    // Admin routes that need admin authentication
    let admin_routes = Router::new()
        .route("/api/admin/users", get(auth_handlers::get_users))
        .route(
            "/api/admin/preferred-number/{user_id}",
            post(admin_handlers::update_preferred_number_admin),
        )
        .route(
            "/api/admin/broadcast-email",
            post(admin_handlers::broadcast_email),
        )
        .route("/api/admin/usage-logs", get(admin_handlers::get_usage_logs))
        .route(
            "/api/admin/subscription/{user_id}/{tier}",
            post(admin_handlers::update_subscription_tier),
        )
        .route(
            "/api/admin/plan-type/{user_id}/{plan_type}",
            post(admin_handlers::update_plan_type),
        )
        .route(
            "/api/billing/reset-credits/{user_id}",
            post(billing_handlers::reset_credits),
        )
        .route(
            "/api/admin/monthly-credits/{user_id}/{amount}",
            post(admin_handlers::update_monthly_credits),
        )
        .route(
            "/api/admin/send-password-reset/{user_id}",
            post(admin_handlers::send_password_reset_link),
        )
        .route(
            "/api/admin/change-password",
            post(admin_handlers::change_admin_password),
        )
        .route(
            "/api/admin/set-twilio-creds",
            post(admin_handlers::set_user_twilio_credentials),
        )
        .route(
            "/api/admin/users/{user_id}/message-stats",
            get(admin_handlers::get_user_message_stats),
        )
        .route(
            "/api/admin/global-message-stats",
            get(admin_handlers::get_global_message_stats),
        )
        .route(
            "/api/admin/stats/costs",
            get(handlers::admin_stats_handlers::get_cost_stats),
        )
        .route(
            "/api/admin/stats/usage",
            get(handlers::admin_stats_handlers::get_usage_stats),
        )
        .route(
            "/api/admin/stats/llm",
            get(handlers::admin_stats_handlers::get_llm_stats),
        )
        // Alert management routes
        .route("/api/admin/alerts", get(admin_handlers::get_alerts))
        .route(
            "/api/admin/alerts/count",
            get(admin_handlers::get_alert_count),
        )
        .route(
            "/api/admin/alerts/{id}/acknowledge",
            post(admin_handlers::acknowledge_alert),
        )
        .route(
            "/api/admin/alerts/acknowledge-all",
            post(admin_handlers::acknowledge_all_alerts),
        )
        .route(
            "/api/admin/alerts/disabled-types",
            get(admin_handlers::get_disabled_alert_types),
        )
        .route(
            "/api/admin/alerts/disable/{alert_type}",
            post(admin_handlers::disable_alert_type),
        )
        .route(
            "/api/admin/alerts/enable/{alert_type}",
            post(admin_handlers::enable_alert_type),
        )
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            handlers::auth_middleware::require_admin,
        ));
    // Protected routes that need user authentication
    let protected_routes = Router::new()
        .route("/api/auth/status", get(auth_handlers::auth_status))
        // TOTP 2FA routes
        .route(
            "/api/totp/setup/start",
            post(handlers::totp_handlers::setup_start),
        )
        .route(
            "/api/totp/setup/verify",
            post(handlers::totp_handlers::setup_verify),
        )
        .route("/api/totp/disable", post(handlers::totp_handlers::disable))
        .route("/api/totp/status", get(handlers::totp_handlers::get_status))
        .route(
            "/api/totp/backup-codes/regenerate",
            post(handlers::totp_handlers::regenerate_backup_codes),
        )
        // WebAuthn routes (passkeys)
        .route(
            "/api/webauthn/status",
            get(handlers::webauthn_handlers::get_status),
        )
        .route(
            "/api/webauthn/passkeys",
            get(handlers::webauthn_handlers::list_passkeys),
        )
        .route(
            "/api/webauthn/register/start",
            post(handlers::webauthn_handlers::register_start),
        )
        .route(
            "/api/webauthn/register/finish",
            post(handlers::webauthn_handlers::register_finish),
        )
        .route(
            "/api/webauthn/authenticate/start",
            post(handlers::webauthn_handlers::authenticate_start),
        )
        .route(
            "/api/webauthn/authenticate/finish",
            post(handlers::webauthn_handlers::authenticate_finish),
        )
        .route(
            "/api/webauthn/passkey",
            delete(handlers::webauthn_handlers::delete_passkey),
        )
        .route(
            "/api/webauthn/passkey/rename",
            patch(handlers::webauthn_handlers::rename_passkey),
        )
        .route(
            "/api/profile/delete/{user_id}",
            delete(profile_handlers::delete_user),
        )
        .route(
            "/api/profile/update",
            post(profile_handlers::update_profile),
        )
        .route(
            "/api/profile/sensitive-change-requirements",
            get(profile_handlers::check_sensitive_change_requirements),
        )
        .route(
            "/api/profile/field",
            patch(profile_handlers::patch_profile_field),
        )
        .route(
            "/api/profile/twilio-phone",
            post(self_host_handlers::update_twilio_phone),
        )
        .route(
            "/api/profile/twilio-creds",
            post(self_host_handlers::update_twilio_creds),
        )
        .route(
            "/api/profile/twilio-creds",
            delete(self_host_handlers::clear_twilio_creds),
        )
        .route(
            "/api/profile/timezone",
            post(profile_handlers::update_timezone),
        )
        .route("/api/profile", get(profile_handlers::get_profile))
        .route(
            "/api/profile/available-sending-numbers",
            get(profile_handlers::get_available_sending_numbers),
        )
        .route(
            "/api/pricing/dashboard-credits",
            get(handlers::pricing_handlers::get_dashboard_credits),
        )
        .route(
            "/api/pricing/usage-projection",
            get(handlers::pricing_handlers::get_usage_projection),
        )
        .route(
            "/api/pricing/byot-usage",
            get(handlers::pricing_handlers::get_byot_usage),
        )
        .route(
            "/api/profile/update-notify/{user_id}",
            post(profile_handlers::update_notify),
        )
        .route(
            "/api/profile/critical",
            post(profile_handlers::update_critical_settings),
        )
        .route(
            "/api/profile/critical",
            get(profile_handlers::get_critical_settings),
        )
        .route(
            "/api/profile/quiet-mode",
            get(profile_handlers::get_quiet_mode),
        )
        .route(
            "/api/profile/quiet-mode",
            post(profile_handlers::set_quiet_mode),
        )
        .route(
            "/api/profile/quiet-rules",
            post(profile_handlers::add_quiet_rule),
        )
        .route(
            "/api/profile/quiet-rules",
            delete(profile_handlers::delete_quiet_rules),
        )
        .route(
            "/api/profile/get_nearby_places",
            get(profile_handlers::get_nearby_places),
        )
        .route("/api/chat/web", post(profile_handlers::web_chat))
        .route(
            "/api/chat/web-stream",
            get(profile_handlers::web_chat_stream),
        )
        .route(
            "/api/chat/web-with-image",
            post(profile_handlers::web_chat_with_image),
        )
        .route(
            "/api/billing/increase-credits/{user_id}",
            post(billing_handlers::increase_credits),
        )
        .route("/api/billing/usage", post(billing_handlers::get_usage_data))
        .route(
            "/api/billing/update-auto-topup/{user_id}",
            post(billing_handlers::update_topup),
        )
        .route(
            "/api/stripe/checkout-session/{user_id}",
            post(stripe_handlers::create_checkout_session),
        )
        .route(
            "/api/stripe/unified-subscription-checkout/{user_id}",
            post(stripe_handlers::create_unified_subscription_checkout),
        )
        .route(
            "/api/stripe/customer-portal/{user_id}",
            get(stripe_handlers::create_customer_portal_session),
        )
        .route("/api/auth/tesla/login", get(tesla_auth::tesla_login))
        .route(
            "/api/auth/tesla/connection",
            delete(tesla_auth::tesla_disconnect),
        )
        .route("/api/auth/tesla/status", get(tesla_auth::tesla_status))
        .route("/api/auth/tesla/scopes", get(tesla_auth::tesla_scopes))
        .route(
            "/api/auth/tesla/scopes/refresh",
            post(tesla_auth::tesla_refresh_scopes),
        )
        .route(
            "/api/auth/tesla/virtual-key",
            get(tesla_auth::get_virtual_key_link),
        )
        .route("/api/tesla/command", post(tesla_auth::tesla_command))
        .route(
            "/api/tesla/command-stream",
            get(tesla_auth::tesla_command_stream),
        )
        .route(
            "/api/tesla/battery-status",
            get(tesla_auth::tesla_battery_status),
        )
        .route(
            "/api/tesla/charge-limit",
            post(tesla_auth::set_charge_limit),
        )
        .route("/api/tesla/vehicles", get(tesla_auth::tesla_list_vehicles))
        .route(
            "/api/tesla/select-vehicle",
            post(tesla_auth::tesla_select_vehicle),
        )
        .route(
            "/api/tesla/mark-paired",
            post(tesla_auth::tesla_mark_paired),
        )
        .route(
            "/api/tesla/climate-notify/status",
            get(tesla_auth::get_climate_notify_status),
        )
        .route(
            "/api/tesla/climate-notify/start",
            post(tesla_auth::start_climate_notify),
        )
        .route(
            "/api/tesla/climate-notify/cancel",
            post(tesla_auth::cancel_climate_notify),
        )
        .route(
            "/api/tesla/charging-notify/status",
            get(tesla_auth::get_charging_notify_status),
        )
        .route(
            "/api/tesla/charging-notify/start",
            post(tesla_auth::start_charging_notify),
        )
        .route(
            "/api/tesla/charging-notify/cancel",
            post(tesla_auth::cancel_charging_notify),
        )
        .route("/api/auth/youtube/login", get(youtube_auth::youtube_login))
        .route(
            "/api/auth/youtube/status",
            get(youtube_auth::youtube_status),
        )
        .route(
            "/api/auth/youtube/upgrade",
            get(youtube_auth::youtube_upgrade_scope),
        )
        .route(
            "/api/auth/youtube/downgrade",
            get(youtube_auth::youtube_downgrade_scope),
        )
        .route(
            "/api/auth/youtube/connection",
            delete(youtube_auth::delete_youtube_connection),
        )
        .route(
            "/api/youtube/subscriptions",
            get(youtube::get_subscription_feed),
        )
        .route("/api/youtube/search", get(youtube::search_youtube))
        .route(
            "/api/youtube/channel/{channel_id}/videos",
            get(youtube::get_channel_videos),
        )
        .route("/api/youtube/video", get(youtube::get_video_details))
        .route(
            "/api/youtube/subscribe",
            post(youtube::subscribe_to_channel),
        )
        .route(
            "/api/youtube/unsubscribe/{channel_id}",
            delete(youtube::unsubscribe_from_channel),
        )
        .route(
            "/api/youtube/video/{video_id}/comments",
            get(youtube::get_video_comments),
        )
        .route(
            "/api/youtube/video/{video_id}/comments",
            post(youtube::post_video_comment),
        )
        .route(
            "/api/youtube/video/{video_id}/rate",
            post(youtube::rate_video),
        )
        .route("/api/auth/imap/login", post(imap_auth::imap_login))
        .route("/api/auth/imap/status", get(imap_auth::imap_status))
        .route(
            "/api/auth/imap/disconnect",
            delete(imap_auth::delete_imap_connection),
        )
        .route(
            "/api/imap/previews",
            get(imap_handlers::fetch_imap_previews),
        )
        .route(
            "/api/imap/message/{email_id}",
            get(imap_handlers::fetch_single_imap_email),
        )
        .route(
            "/api/imap/full_emails",
            get(imap_handlers::fetch_full_imap_emails),
        )
        .route("/api/imap/reply", post(imap_handlers::respond_to_email))
        .route("/api/imap/send", post(imap_handlers::send_email))
        .route(
            "/api/auth/telegram/status",
            get(telegram_auth::get_telegram_status),
        )
        .route(
            "/api/auth/telegram/connect",
            get(telegram_auth::start_telegram_connection),
        )
        .route(
            "/api/auth/telegram/disconnect",
            delete(telegram_auth::disconnect_telegram),
        )
        .route(
            "/api/auth/telegram/resync",
            post(telegram_auth::resync_telegram),
        )
        .route(
            "/api/auth/telegram/health",
            get(telegram_auth::check_telegram_health),
        )
        .route(
            "/api/telegram/test-messages",
            get(telegram_handlers::test_fetch_messages),
        )
        .route("/api/telegram/send", post(telegram_handlers::send_message))
        .route(
            "/api/telegram/search-rooms",
            post(telegram_handlers::search_telegram_rooms_handler),
        )
        .route(
            "/api/telegram/search-rooms",
            get(telegram_handlers::search_rooms_handler),
        )
        .route(
            "/api/auth/signal/status",
            get(signal_auth::get_signal_status),
        )
        .route(
            "/api/auth/signal/connect",
            get(signal_auth::start_signal_connection),
        )
        .route(
            "/api/auth/signal/disconnect",
            delete(signal_auth::disconnect_signal),
        )
        .route("/api/auth/signal/resync", post(signal_auth::resync_signal))
        .route(
            "/api/auth/signal/health",
            get(signal_auth::check_signal_health),
        )
        .route(
            "/api/signal/test-messages",
            get(signal_handlers::test_fetch_messages),
        )
        .route("/api/signal/send", post(signal_handlers::send_message))
        .route(
            "/api/signal/search-rooms",
            post(signal_handlers::search_signal_rooms_handler),
        )
        .route(
            "/api/signal/search-rooms",
            get(signal_handlers::search_rooms_handler),
        )
        .route(
            "/api/auth/whatsapp/status",
            get(whatsapp_auth::get_whatsapp_status),
        )
        .route(
            "/api/auth/whatsapp/connect",
            get(whatsapp_auth::start_whatsapp_connection),
        )
        .route(
            "/api/auth/whatsapp/connect-phone",
            post(whatsapp_auth::start_whatsapp_phone_connection),
        )
        .route(
            "/api/auth/whatsapp/disconnect",
            delete(whatsapp_auth::disconnect_whatsapp),
        )
        .route(
            "/api/auth/whatsapp/resync",
            post(whatsapp_auth::resync_whatsapp),
        )
        .route(
            "/api/auth/whatsapp/health",
            get(whatsapp_auth::check_whatsapp_health),
        )
        .route(
            "/api/whatsapp/test-messages",
            get(whatsapp_handlers::test_fetch_messages),
        )
        .route("/api/whatsapp/send", post(whatsapp_handlers::send_message))
        .route(
            "/api/whatsapp/search-rooms",
            post(whatsapp_handlers::search_whatsapp_rooms_handler),
        )
        .route(
            "/api/whatsapp/search-rooms",
            get(whatsapp_handlers::search_rooms_handler),
        )
        // Matrix connection reset route (clears all Matrix credentials when auth fails)
        .route(
            "/api/auth/matrix/reset",
            delete(bridge_auth_common::reset_matrix_connection),
        )
        // Item edit with AI
        // Dashboard routes
        .route(
            "/api/dashboard/summary",
            get(dashboard_handlers::get_dashboard_summary),
        )
        .route(
            "/api/dashboard/activity-feed",
            get(dashboard_handlers::get_activity_feed),
        )
        .route(
            "/api/dashboard/senders",
            get(dashboard_handlers::get_senders),
        )
        .route(
            "/api/dashboard/rule-sources",
            get(dashboard_handlers::get_rule_sources),
        )
        .route(
            "/api/events/{id}/dismiss",
            post(dashboard_handlers::dismiss_event),
        )
        .route(
            "/api/events/{id}",
            get(dashboard_handlers::get_event_detail),
        )
        // Person + Channel (ontology) routes
        .route(
            "/api/persons",
            get(person_handlers::get_persons).post(person_handlers::create_person),
        )
        .route(
            "/api/persons/{id}",
            put(person_handlers::update_person).delete(person_handlers::delete_person),
        )
        .route(
            "/api/persons/{id}/channels",
            post(person_handlers::add_person_channel),
        )
        .route(
            "/api/persons/{person_id}/channels/{channel_id}",
            put(person_handlers::update_person_channel)
                .delete(person_handlers::delete_person_channel),
        )
        .route("/api/persons/merge", post(person_handlers::merge_persons))
        .route(
            "/api/persons/search/{service}",
            get(person_handlers::search_chats),
        )
        // Rule (automation) routes
        .route(
            "/api/rules",
            get(rule_handlers::list_rules).post(rule_handlers::create_rule),
        )
        .route("/api/rules/test", post(rule_handlers::start_rule_test))
        .route(
            "/api/rules/test-stream",
            get(rule_handlers::test_rule_stream),
        )
        .route(
            "/api/rules/{id}",
            get(rule_handlers::get_rule)
                .put(rule_handlers::update_rule)
                .delete(rule_handlers::delete_rule),
        )
        .route(
            "/api/rules/{id}/status",
            patch(rule_handlers::update_rule_status),
        )
        // Web-based voice call routes (browser to ElevenLabs)
        .route(
            "/api/call/web-signed-url",
            get(elevenlabs::get_web_signed_url),
        )
        .route("/api/call/web-end", post(elevenlabs::end_web_call))
        .route(
            "/api/call/web-check-credits",
            get(elevenlabs::check_web_call_credits),
        )
        // MCP Server routes (custom tool integrations)
        .route(
            "/api/mcp/servers",
            get(handlers::mcp_handlers::list_mcp_servers)
                .post(handlers::mcp_handlers::create_mcp_server),
        )
        .route(
            "/api/mcp/servers/{id}",
            delete(handlers::mcp_handlers::delete_mcp_server),
        )
        .route(
            "/api/mcp/servers/{id}/tools",
            get(handlers::mcp_handlers::list_server_tools),
        )
        .route(
            "/api/mcp/servers/{id}/test",
            post(handlers::mcp_handlers::test_server_connection),
        )
        .route(
            "/api/mcp/servers/{id}/toggle",
            patch(handlers::mcp_handlers::toggle_mcp_server),
        )
        .route(
            "/api/mcp/test",
            post(handlers::mcp_handlers::test_url_connection),
        )
        .route_layer(middleware::from_fn(handlers::auth_middleware::require_auth));
    // Internal maintenance endpoints (localhost-only, no auth)
    let maintenance_routes = Router::new()
        .route(
            "/api/internal/maintenance/enable",
            post(handlers::maintenance_handlers::enable_maintenance),
        )
        .route(
            "/api/internal/maintenance/disable",
            post(handlers::maintenance_handlers::disable_maintenance),
        )
        .route(
            "/api/internal/maintenance/status",
            get(handlers::maintenance_handlers::maintenance_status),
        );

    let app = Router::new()
        .merge(maintenance_routes)
        .merge(public_routes)
        .merge(admin_routes)
        .merge(protected_routes)
        .merge(auth_built_in_webhook_routes)
        .route(
            "/.well-known/appspecific/com.tesla.3p.public-key.pem",
            get(tesla_auth::serve_tesla_public_key),
        )
        .merge(textbee_routes)
        .merge(twilio_routes)
        .merge(elevenlabs_routes)
        .merge(elevenlabs_free_routes)
        .merge(elevenlabs_webhook_routes)
        .route("/public/{*path}", any(proxy_telegram_public))
        .route("/public", any(proxy_telegram_public))
        .nest_service("/uploads", ServeDir::new("uploads"))
        .fallback_service(
            ServeDir::new("public").not_found_service(ServeFile::new("public/index.html")),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            handlers::auth_middleware::apply_api_rate_limit,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            handlers::maintenance_handlers::maintenance_guard,
        ))
        .layer(session_layer)
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
        .layer({
            let frontend_url = std::env::var("FRONTEND_URL").unwrap_or_default();
            let cors = CorsLayer::new()
                .allow_methods([
                    axum::http::Method::GET,
                    axum::http::Method::POST,
                    axum::http::Method::OPTIONS,
                    axum::http::Method::DELETE,
                    axum::http::Method::PATCH,
                    axum::http::Method::PUT,
                ])
                .allow_headers([
                    axum::http::header::CONTENT_TYPE,
                    axum::http::header::AUTHORIZATION,
                    axum::http::header::ACCEPT,
                    axum::http::header::ORIGIN,
                ])
                .expose_headers([
                    axum::http::header::CONTENT_TYPE,
                    axum::http::header::CONTENT_LENGTH,
                ]);
            if frontend_url.is_empty() {
                let server_origin = std::env::var("SERVER_URL")
                    .expect("SERVER_URL must be set when FRONTEND_URL is not configured");
                cors.allow_origin(AllowOrigin::exact(
                    server_origin.parse().expect("Invalid SERVER_URL"),
                ))
            } else {
                // Cross-origin mode (local dev): exact origin with credentials
                cors.allow_origin(AllowOrigin::exact(
                    frontend_url.parse().expect("Invalid FRONTEND_URL"),
                ))
                .allow_credentials(true)
            }
        })
        // Security headers to prevent clickjacking, XSS, and other attacks
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::X_FRAME_OPTIONS,
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::REFERRER_POLICY,
            HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            axum::http::header::X_XSS_PROTECTION,
            HeaderValue::from_static("1; mode=block"),
        ))
        .with_state(state.clone());
    let state_for_scheduler = state.clone();
    tokio::spawn(async move {
        jobs::scheduler::start_scheduler(state_for_scheduler).await;
    });
    use tokio::net::TcpListener;
    let port = match std::env::var("ENVIRONMENT").as_deref() {
        Ok("staging") => 3100, // actually prod, but just saying staging
        _ => 3000,
    };
    validate_env();

    // Initialize Tesla keys and register in all regions
    tracing::info!("Initializing Tesla integration...");
    match utils::tesla_keys::generate_or_load_keys() {
        Ok(_) => {
            tracing::info!("Tesla EC key pair ready");
            tracing::info!(
                "Public key will be served at /.well-known/appspecific/com.tesla.3p.public-key.pem"
            );

            // Register app in all Tesla regions (EU, NA, AP) for proxy to work globally
            tracing::info!("Registering app in all Tesla Fleet API regions...");
            let regions = vec![
                ("EU", "https://fleet-api.prd.eu.vn.cloud.tesla.com"),
                ("NA", "https://fleet-api.prd.na.vn.cloud.tesla.com"),
                ("AP", "https://fleet-api.prd.ap.vn.cloud.tesla.com"),
            ];

            for (name, url) in regions {
                let client = api::tesla::TeslaClient::new_with_region(url);
                match client.register_in_region().await {
                    Ok(_) => tracing::info!("✓ Registered in {} region", name),
                    Err(e) => tracing::warn!("Failed to register in {} region: {} (this may be ok if already registered)", name, e),
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to initialize Tesla keys: {}", e);
            tracing::warn!("Tesla integration will not be available");
        }
    }

    tracing::info!("Starting server on port {}", port);
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .unwrap();
    axum::serve(listener, app.into_make_service())
        .await
        .unwrap();
}
