use axum::http::HeaderValue;
use axum::{
    middleware,
    routing::{delete, get, patch, post, put},
    Router,
};
use dashmap::DashMap;
use diesel::r2d2::{self, ConnectionManager};
use diesel::SqliteConnection;
use dotenvy::dotenv;
use oauth2::{basic::BasicClient, AuthUrl, ClientId, ClientSecret, RedirectUrl, TokenUrl};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use tower_sessions::{MemoryStore, SessionManagerLayer};
use tracing::Level;

// Import modules and types from library crate
use api::{elevenlabs, elevenlabs_webhook, twilio_sms};
use backend::{
    api, handlers, jobs, utils, AdminAlertRepository, AiConfig, AppState,
    SqliteConnectionCustomizer, TotpRepository, UserCore, UserCoreOps, UserRepository,
    WebauthnRepository,
};
use handlers::{
    admin_handlers, auth_handlers, billing_handlers, bridge_auth_common, contact_profile_handlers,
    dashboard_handlers, filter_handlers, google_calendar, google_calendar_auth, imap_auth,
    imap_handlers, instagram_auth, instagram_handlers, messenger_auth, messenger_handlers,
    profile_handlers, refund_handlers, self_host_handlers, signal_auth, signal_handlers,
    stripe_handlers, telegram_auth, telegram_handlers, tesla_auth, twilio_handlers, uber_auth,
    whatsapp_auth, whatsapp_handlers, youtube, youtube_auth,
};

async fn health_check() -> &'static str {
    "OK"
}

pub fn validate_env() {
    // Core variables (always required regardless of environment)
    let core_vars = [
        "JWT_SECRET_KEY",
        "JWT_REFRESH_KEY",
        "DATABASE_URL",
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
            "STRIPE_SUBSCRIPTION_WORLD_PRICE_ID",
            // SMS/Voice (Twilio)
            "TWILIO_ACCOUNT_SID",
            "TWILIO_AUTH_TOKEN",
            // Voice AI (ElevenLabs)
            "ELEVENLABS_SERVER_URL_SECRET",
            // Regional phone numbers
            "FIN_PHONE",
            "USA_PHONE",
            "AUS_PHONE",
            // Music recognition (Shazam)
            "SHAZAM_PHONE_NUMBER",
            "SHAZAM_API_KEY",
            // Production server config
            "SERVER_URL",
            "ASSISTANT_ID",
            // External integrations
            "COMPOSIO_API_KEY",
        ];

        for var in production_vars.iter() {
            std::env::var(var)
                .unwrap_or_else(|_| panic!("{} must be set in production environment", var));
        }
    }

    // Note: The following are truly optional even in production:
    // - PERPLEXITY_API_KEY, OPENROUTER_API_KEY (AI features gracefully degrade)
    // - GOOGLE_CALENDAR_CLIENT_ID/SECRET (OAuth, user-specific)
    // - SENTRY_DSN (error tracking, optional)
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

    // Password defaults to 12345678 if not set
    let password =
        std::env::var("BOOTSTRAP_ADMIN_PASSWORD").unwrap_or_else(|_| "12345678".to_string());

    let phone = std::env::var("BOOTSTRAP_ADMIN_PHONE").unwrap_or_else(|_| "+12345678".to_string());

    // Hash the password
    let password_hash = bcrypt::hash(&password, bcrypt::DEFAULT_COST)?;

    // Create the admin user
    let new_user = NewUser {
        email: email.clone(),
        password_hash,
        phone_number: phone.clone(),
        time_to_live: 60,
        verified: true,
        credits: 1000.0,
        credits_left: 1000.0,
        charge_when_under: false,
        waiting_checks_count: 0,
        discount: false,
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
    dotenv().ok();
    let _guard = sentry::init(("https://07fbdaf63c1270c8509844b775045dd3@o4507415184539648.ingest.de.sentry.io/4508802101411920", sentry::ClientOptions {
        release: sentry::release_name!(),
        ..Default::default()
    }));
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

    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set in environment");
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    let pool = r2d2::Pool::builder()
        .connection_customizer(Box::new(SqliteConnectionCustomizer))
        .build(manager)
        .expect("Failed to create pool");
    let user_core = Arc::new(UserCore::new(pool.clone()));

    // Bootstrap admin user on first startup (only if database is empty)
    if let Err(e) = bootstrap_admin_if_needed(&user_core).await {
        tracing::warn!("Admin bootstrap failed (app will continue): {}", e);
    }

    let user_repository = Arc::new(UserRepository::new(pool.clone()));
    let totp_repository = Arc::new(TotpRepository::new(pool.clone()));
    let webauthn_repository = Arc::new(WebauthnRepository::new(pool.clone()));
    let admin_alert_repository = Arc::new(AdminAlertRepository::new(pool.clone()));
    let metrics_repository = Arc::new(backend::MetricsRepository::new(pool.clone()));
    let server_url_oauth =
        std::env::var("SERVER_URL_OAUTH").unwrap_or_else(|_| "http://localhost:3000".to_string());
    let server_url =
        std::env::var("SERVER_URL").unwrap_or_else(|_| "http://localhost:3000".to_string());
    let client_id = std::env::var("GOOGLE_CALENDAR_CLIENT_ID")
        .unwrap_or_else(|_| "default-client-id-for-testing".to_string());
    let client_secret = std::env::var("GOOGLE_CALENDAR_CLIENT_SECRET")
        .unwrap_or_else(|_| "default-secret-for-testing".to_string());
    let google_calendar_oauth_client = BasicClient::new(ClientId::new(client_id.clone()))
        .set_client_secret(ClientSecret::new(client_secret.clone()))
        .set_auth_uri(
            AuthUrl::new("https://accounts.google.com/o/oauth2/v2/auth".to_string())
                .expect("Invalid auth URL"),
        )
        .set_token_uri(
            TokenUrl::new("https://oauth2.googleapis.com/token".to_string())
                .expect("Invalid token URL"),
        )
        .set_redirect_uri(
            RedirectUrl::new(format!(
                "{}/api/auth/google/calendar/callback",
                server_url_oauth
            ))
            .expect("Invalid redirect URL"),
        );
    let uber_url_oauth =
        std::env::var("UBER_API_URL").unwrap_or_else(|_| "https://login.uber.com".to_string());
    let uber_client_id = std::env::var("UBER_CLIENT_ID")
        .unwrap_or_else(|_| "default-uber-client-id-for-testing".to_string());
    let uber_client_secret = std::env::var("UBER_CLIENT_SECRET")
        .unwrap_or_else(|_| "default-uber-secret-for-testing".to_string());
    let uber_oauth_client = BasicClient::new(ClientId::new(uber_client_id))
        .set_client_secret(ClientSecret::new(uber_client_secret))
        .set_auth_uri(
            AuthUrl::new(format!("{}/oauth/v2/authorize", uber_url_oauth))
                .expect("Invalid auth URL"),
        )
        .set_token_uri(
            TokenUrl::new(format!("{}/oauth/v2/token", uber_url_oauth)).expect("Invalid token URL"),
        )
        .set_redirect_uri(
            RedirectUrl::new(format!("{}/api/auth/uber/callback", server_url_oauth))
                .expect("Invalid redirect URL"),
        );
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
        pool.clone(),
        user_core.clone(),
        user_repository.clone(),
    ));
    let state = Arc::new(AppState {
        db_pool: pool,
        user_core: user_core.clone(),
        user_repository: user_repository.clone(),
        twilio_client,
        twilio_message_service,
        ai_config: AiConfig::from_env(),
        google_calendar_oauth_client,
        uber_oauth_client,
        tesla_oauth_client,
        youtube_oauth_client,
        session_store: session_store.clone(),
        login_limiter: DashMap::new(),
        password_reset_limiter: DashMap::new(),
        password_reset_verify_limiter: DashMap::new(),
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
            "/api/call/calendar",
            get(elevenlabs::handle_calendar_tool_call),
        )
        .route(
            "/api/call/calendar/create",
            get(elevenlabs::handle_calendar_event_creation),
        )
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
            "/api/call/task",
            post(elevenlabs::handle_create_task_tool_call),
        )
        .route(
            "/api/call/monitoring-status",
            post(elevenlabs::handle_update_monitoring_status_tool_call),
        )
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
            "/api/call/directions",
            post(elevenlabs::handle_directions_tool_call),
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
        .route(
            "/api/auth/google/calendar/callback",
            get(google_calendar_auth::google_callback),
        )
        .route("/api/auth/uber/callback", get(uber_auth::uber_callback))
        .route("/api/auth/tesla/callback", get(tesla_auth::tesla_callback))
        .route(
            "/api/auth/youtube/callback",
            get(youtube_auth::youtube_callback),
        );
    // Public routes that don't need authentication. there's ratelimiting though
    let public_routes = Router::new()
        .route("/api/health", get(health_check))
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
        .route("/testing", post(auth_handlers::testing_handler))
        .route("/api/admin/users", get(auth_handlers::get_users))
        .route(
            "/api/admin/verify/{user_id}",
            post(admin_handlers::verify_user),
        )
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
            "/api/admin/discount-tier/{user_id}/{tier}",
            post(admin_handlers::update_discount_tier),
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
        .route(
            "/api/admin/dashboard/triage/{item_type}/{id}",
            delete(handlers::dashboard_handlers::dismiss_triage_item),
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
            "/api/profile/textbee-creds",
            post(self_host_handlers::update_textbee_creds),
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
            "/api/profile/digests",
            post(profile_handlers::update_digests),
        )
        .route("/api/profile/digests", get(profile_handlers::get_digests))
        .route(
            "/api/profile/critical",
            post(profile_handlers::update_critical_settings),
        )
        .route(
            "/api/profile/critical",
            get(profile_handlers::get_critical_settings),
        )
        .route(
            "/api/profile/proactive-agent",
            post(profile_handlers::update_proactive_agent_on),
        )
        .route(
            "/api/profile/proactive-agent",
            get(profile_handlers::get_proactive_agent_on),
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
            "/api/profile/get_nearby_places",
            get(profile_handlers::get_nearby_places),
        )
        .route("/api/chat/web", post(profile_handlers::web_chat))
        .route(
            "/api/chat/web-with-image",
            post(profile_handlers::web_chat_with_image),
        )
        .route(
            "/api/chat/digest",
            get(profile_handlers::get_instant_digest),
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
            "/api/refund/eligibility",
            get(refund_handlers::get_refund_eligibility),
        )
        .route("/api/refund/request", post(refund_handlers::request_refund))
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
        .route(
            "/api/auth/google/calendar/login",
            get(google_calendar_auth::google_login),
        )
        .route(
            "/api/auth/google/calendar/connection",
            delete(google_calendar_auth::delete_google_calendar_connection),
        )
        .route(
            "/api/auth/google/calendar/status",
            get(google_calendar::google_calendar_status),
        )
        .route(
            "/api/auth/google/calendar/email",
            get(google_calendar::get_calendar_email),
        )
        .route(
            "/api/calendar/events",
            get(google_calendar::handle_calendar_fetching_route),
        )
        .route(
            "/api/calendar/create",
            post(google_calendar::create_calendar_event),
        )
        .route("/api/auth/uber/login", get(uber_auth::uber_login))
        .route(
            "/api/auth/uber/connection",
            delete(uber_auth::uber_disconnect),
        )
        .route("/api/auth/uber/status", get(uber_auth::uber_status))
        //.route("api/uber", get(uber::test_status_change))
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
        // External media platforms (no platform auth required, just user auth)
        .route(
            "/api/tiktok/resolve",
            post(handlers::tiktok::resolve_tiktok_url),
        )
        .route(
            "/api/instagram/resolve",
            post(handlers::instagram_reels::resolve_instagram_reel),
        )
        .route(
            "/api/twitter/resolve",
            post(handlers::twitter::resolve_twitter_url),
        )
        .route(
            "/api/reddit/resolve",
            post(handlers::reddit::resolve_reddit_url),
        )
        .route(
            "/api/spotify/resolve",
            post(handlers::spotify::resolve_spotify_url),
        )
        .route(
            "/api/rumble/resolve",
            post(handlers::rumble::resolve_rumble_url),
        )
        .route(
            "/api/streamable/resolve",
            post(handlers::streamable::resolve_streamable_url),
        )
        .route(
            "/api/bluesky/resolve",
            post(handlers::bluesky::resolve_bluesky_url),
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
            "/api/auth/messenger/status",
            get(messenger_auth::get_messenger_status),
        )
        .route(
            "/api/auth/messenger/connect",
            get(messenger_auth::start_messenger_connection),
        )
        .route(
            "/api/auth/messenger/disconnect",
            delete(messenger_auth::disconnect_messenger),
        )
        .route(
            "/api/auth/messenger/resync",
            post(messenger_auth::resync_messenger),
        )
        .route(
            "/api/messenger/test-messages",
            get(messenger_handlers::test_fetch_messenger_messages),
        )
        .route(
            "/api/messenger/send",
            post(messenger_handlers::send_messenger_message),
        )
        .route(
            "/api/messenger/search-rooms",
            post(messenger_handlers::search_messenger_rooms_handler),
        )
        .route(
            "/api/messenger/rooms",
            get(messenger_handlers::search_messenger_rooms_handler),
        )
        .route(
            "/api/auth/instagram/status",
            get(instagram_auth::get_instagram_status),
        )
        .route(
            "/api/auth/instagram/connect",
            get(instagram_auth::start_instagram_connection),
        )
        .route(
            "/api/auth/instagram/disconnect",
            delete(instagram_auth::disconnect_instagram),
        )
        .route(
            "/api/auth/instagram/resync",
            post(instagram_auth::resync_instagram),
        )
        .route(
            "/api/instagram/test-messages",
            get(instagram_handlers::test_fetch_instagram_messages),
        )
        .route(
            "/api/instagram/send",
            post(instagram_handlers::send_instagram_message),
        )
        .route(
            "/api/instagram/search-rooms",
            post(instagram_handlers::search_instagram_rooms_handler),
        )
        .route(
            "/api/instagram/rooms",
            get(instagram_handlers::search_instagram_rooms_handler),
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
        // Task routes (reminders and message monitoring)
        .route(
            "/api/filters/tasks",
            get(filter_handlers::get_tasks).post(filter_handlers::create_task),
        )
        .route(
            "/api/filters/task/{task_id}",
            delete(filter_handlers::cancel_task),
        )
        .route(
            "/api/filters/task/{task_id}/permanence",
            patch(filter_handlers::set_task_permanence),
        )
        .route(
            "/api/tasks/{task_id}/edit-ai",
            post(filter_handlers::edit_task_with_ai),
        )
        .route(
            "/api/tasks/{task_id}",
            get(filter_handlers::get_task).delete(filter_handlers::cancel_task),
        )
        .route(
            "/api/filters/monitored-contacts",
            get(filter_handlers::get_priority_senders),
        )
        .route(
            "/api/filters/monitored-contact/{service_type}",
            post(filter_handlers::create_priority_sender),
        )
        .route(
            "/api/filters/monitored-contact/{service_type}/{content}",
            delete(filter_handlers::delete_priority_sender),
        )
        .route(
            "/api/filters/priority-sender/{service_type}",
            post(filter_handlers::create_priority_sender),
        )
        .route(
            "/api/filters/priority-sender/{service_type}/{sender}",
            delete(filter_handlers::delete_priority_sender),
        )
        .route(
            "/api/filters/priority-senders/{service_type}",
            get(filter_handlers::get_priority_senders),
        )
        .route(
            "/api/filters/keyword/{service_type}",
            post(filter_handlers::create_keyword),
        )
        .route(
            "/api/filters/keyword/{service_type}/{keyword}",
            delete(filter_handlers::delete_keyword),
        )
        // Dashboard routes
        .route(
            "/api/dashboard/summary",
            get(dashboard_handlers::get_dashboard_summary),
        )
        // Triage routes
        .route("/api/triage", get(dashboard_handlers::get_triage_items))
        .route(
            "/api/triage/{id}/execute",
            post(dashboard_handlers::execute_triage_item),
        )
        .route(
            "/api/triage/{id}/snooze",
            post(dashboard_handlers::snooze_triage_item),
        )
        .route(
            "/api/triage/{id}",
            delete(dashboard_handlers::dismiss_triage_item_by_id),
        )
        // Contact Profiles routes
        .route(
            "/api/contact-profiles",
            get(contact_profile_handlers::get_contact_profiles),
        )
        .route(
            "/api/contact-profiles",
            post(contact_profile_handlers::create_contact_profile),
        )
        .route(
            "/api/contact-profiles/default-mode",
            put(contact_profile_handlers::update_default_mode),
        )
        .route(
            "/api/contact-profiles/phone-contact-mode",
            put(contact_profile_handlers::update_phone_contact_mode),
        )
        .route(
            "/api/contact-profiles/search/{service}",
            get(contact_profile_handlers::search_chats),
        )
        .route(
            "/api/contact-profiles/{id}",
            put(contact_profile_handlers::update_contact_profile),
        )
        .route(
            "/api/contact-profiles/{id}",
            delete(contact_profile_handlers::delete_contact_profile),
        )
        // WhatsApp filter toggle routes
        // Generic filter toggle routes
        .route(
            "/api/profile/email-judgments",
            get(profile_handlers::get_email_judgments),
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
    let app = Router::new()
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
        .nest_service("/uploads", ServeDir::new("uploads"))
        // Serve static SEO files at root
        .nest_service(
            "/robots.txt",
            tower_http::services::ServeFile::new("static/robots.txt"),
        )
        .nest_service(
            "/sitemap.xml",
            tower_http::services::ServeFile::new("static/sitemap.xml"),
        )
        .nest_service(
            "/llms.txt",
            tower_http::services::ServeFile::new("static/llms.txt"),
        )
        .nest_service(
            "/llms-full.txt",
            tower_http::services::ServeFile::new("static/llms-full.txt"),
        )
        .layer(session_layer)
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
        .layer(
            CorsLayer::new()
                .allow_methods([
                    axum::http::Method::GET,
                    axum::http::Method::POST,
                    axum::http::Method::OPTIONS,
                    axum::http::Method::DELETE,
                    axum::http::Method::PATCH,
                    axum::http::Method::PUT,
                ])
                .allow_origin(AllowOrigin::exact(
                    std::env::var("FRONTEND_URL")
                        .unwrap_or_else(|_| "http://localhost:8080".to_string())
                        .parse()
                        .expect("Invalid FRONTEND_URL"),
                )) // Restrict in production
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
                .allow_credentials(true),
        )
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
