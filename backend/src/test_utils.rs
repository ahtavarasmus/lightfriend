//! Test utilities for SMS e2e tests and Matrix integration tests.
//!
//! Provides mock LLM responses, test state setup, and Matrix test server utilities.

pub mod matrix_test_server;

use crate::UserCoreOps;
use dashmap::DashMap;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use oauth2::{basic::BasicClient, AuthUrl, ClientId, ClientSecret, TokenUrl};
use openai_api_rs::v1::chat_completion;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_sessions::MemoryStore;

/// Embedded migrations for in-memory test database
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("./migrations");

/// Create an in-memory SQLite connection pool with migrations applied
///
/// Uses shared cache mode with a unique database name so all connections
/// from this pool share the same in-memory database, but different tests
/// get isolated databases.
pub fn create_test_pool() -> crate::DbPool {
    use diesel::r2d2::{self, ConnectionManager};
    use diesel::SqliteConnection;
    use std::sync::atomic::{AtomicU64, Ordering};

    // Generate unique database name for this test
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let db_id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let db_url = format!("file:testdb_{}?mode=memory&cache=shared", db_id);

    let manager = ConnectionManager::<SqliteConnection>::new(&db_url);
    let pool = r2d2::Pool::builder()
        .max_size(5) // Allow multiple connections with shared cache
        .connection_customizer(Box::new(crate::SqliteConnectionCustomizer))
        .build(manager)
        .expect("Failed to create test pool");

    // Run migrations
    let mut conn = pool.get().expect("Failed to get connection");
    conn.run_pending_migrations(MIGRATIONS)
        .expect("Failed to run migrations");

    pool
}

/// Create a dummy Google OAuth client for AppState (not used in credit tests)
fn create_dummy_google_oauth_client() -> crate::GoogleOAuthClient {
    BasicClient::new(ClientId::new("test_client_id".to_string()))
        .set_client_secret(ClientSecret::new("test_client_secret".to_string()))
        .set_auth_uri(AuthUrl::new("https://example.com/auth".to_string()).unwrap())
        .set_token_uri(TokenUrl::new("https://example.com/token".to_string()).unwrap())
}

/// Create a dummy Tesla OAuth client for AppState (not used in credit tests)
fn create_dummy_tesla_oauth_client() -> crate::TeslaOAuthClient {
    BasicClient::new(ClientId::new("test_tesla_client_id".to_string()))
        .set_client_secret(ClientSecret::new("test_tesla_secret".to_string()))
        .set_auth_uri(AuthUrl::new("https://example.com/tesla/auth".to_string()).unwrap())
        .set_token_uri(TokenUrl::new("https://example.com/tesla/token".to_string()).unwrap())
}

/// Create a minimal AppState for testing credit deduction
///
/// This creates a real in-memory database with UserCore and UserRepository,
/// but stubs out OAuth clients and other services not needed for credit tests.
pub fn create_test_state() -> Arc<crate::AppState> {
    let pool = create_test_pool();

    let user_core = Arc::new(crate::UserCore::new(pool.clone()));
    let user_repository = Arc::new(crate::UserRepository::new(pool.clone()));
    let item_repository = Arc::new(crate::ItemRepository::new(pool.clone()));
    let totp_repository = Arc::new(crate::repositories::totp_repository::TotpRepository::new(
        pool.clone(),
    ));
    let webauthn_repository =
        Arc::new(crate::repositories::webauthn_repository::WebauthnRepository::new(pool.clone()));
    let admin_alert_repository = Arc::new(
        crate::repositories::admin_alert_repository::AdminAlertRepository::new(pool.clone()),
    );
    let metrics_repository =
        Arc::new(crate::repositories::metrics_repository::MetricsRepository::new(pool.clone()));

    let google_oauth = create_dummy_google_oauth_client();
    let tesla_oauth = create_dummy_tesla_oauth_client();
    let twilio_client = Arc::new(crate::RealTwilioClient::new());
    let twilio_message_service = Arc::new(crate::TwilioMessageService::new(
        twilio_client.clone(),
        pool.clone(),
        user_core.clone(),
        user_repository.clone(),
    ));

    Arc::new(crate::AppState {
        db_pool: pool,
        user_core,
        user_repository,
        item_repository,
        twilio_client,
        twilio_message_service,
        ai_config: crate::AiConfig::default_for_tests(),
        google_calendar_oauth_client: google_oauth.clone(),
        youtube_oauth_client: google_oauth.clone(),
        uber_oauth_client: google_oauth,
        tesla_oauth_client: tesla_oauth,
        session_store: MemoryStore::default(),
        login_limiter: DashMap::new(),
        password_reset_limiter: DashMap::new(),
        password_reset_verify_limiter: DashMap::new(),
        matrix_sync_tasks: Arc::new(Mutex::new(HashMap::new())),
        matrix_clients: Arc::new(Mutex::new(HashMap::new())),
        tesla_monitoring_tasks: Arc::new(DashMap::new()),
        tesla_charging_monitor_tasks: Arc::new(DashMap::new()),
        tesla_waking_vehicles: Arc::new(DashMap::new()),
        password_reset_otps: DashMap::new(),
        phone_verify_limiter: DashMap::new(),
        phone_verify_verify_limiter: DashMap::new(),
        phone_verify_otps: DashMap::new(),
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
        tool_registry: crate::build_tool_registry(),
    })
}

/// Create a test user in the database from TestUserParams
pub fn create_test_user(
    state: &Arc<crate::AppState>,
    params: &TestUserParams,
) -> crate::models::user_models::User {
    use crate::handlers::auth_dtos::NewUser;

    let password_hash =
        bcrypt::hash("test123", bcrypt::DEFAULT_COST).expect("Failed to hash password");

    let new_user = NewUser {
        email: params.email.clone(),
        password_hash,
        phone_number: params.phone_number.clone(),
        time_to_live: 60,
        verified: true,
        credits: params.credits,
        credits_left: params.credits_left,
        charge_when_under: false,
        waiting_checks_count: 0,
        discount: false,
        sub_tier: params.sub_tier.clone(),
    };

    state
        .user_core
        .create_user(new_user)
        .expect("Failed to create test user");
    state
        .user_core
        .find_by_email(&params.email)
        .expect("Failed to find created user")
        .expect("User not found after creation")
}

/// Mock LLM response builder for testing without calling real API.
///
/// Since FinishReason doesn't implement Clone, we store the tool calls
/// and create the full response lazily in to_response().
#[derive(Debug)]
pub struct MockLlmResponse {
    pub content: Option<String>,
    pub tool_calls: Option<Vec<chat_completion::ToolCall>>,
}

impl MockLlmResponse {
    /// Create a response with direct_response tool call (LLM always uses tools via tool_choice: Required)
    pub fn with_direct_response(response: &str) -> Self {
        Self {
            content: None,
            tool_calls: Some(vec![chat_completion::ToolCall {
                id: "call_test_123".to_string(),
                r#type: "function".to_string(),
                function: chat_completion::ToolCallFunction {
                    name: Some("direct_response".to_string()),
                    arguments: Some(
                        serde_json::json!({
                            "response": response
                        })
                        .to_string(),
                    ),
                },
            }]),
        }
    }

    /// Create a response with ask_perplexity tool call
    pub fn with_perplexity_query(query: &str) -> Self {
        Self {
            content: None,
            tool_calls: Some(vec![chat_completion::ToolCall {
                id: "call_test_perplexity".to_string(),
                r#type: "function".to_string(),
                function: chat_completion::ToolCallFunction {
                    name: Some("ask_perplexity".to_string()),
                    arguments: Some(
                        serde_json::json!({
                            "query": query
                        })
                        .to_string(),
                    ),
                },
            }]),
        }
    }

    /// Create a response with get_weather tool call
    pub fn with_weather_query(location: &str, units: &str) -> Self {
        Self {
            content: None,
            tool_calls: Some(vec![chat_completion::ToolCall {
                id: "call_test_weather".to_string(),
                r#type: "function".to_string(),
                function: chat_completion::ToolCallFunction {
                    name: Some("get_weather".to_string()),
                    arguments: Some(
                        serde_json::json!({
                            "location": location,
                            "units": units,
                            "forecast_type": "current"
                        })
                        .to_string(),
                    ),
                },
            }]),
        }
    }

    /// Create a response with empty content (no tool calls, no content)
    pub fn with_empty_response() -> Self {
        Self {
            content: Some("".to_string()),
            tool_calls: None,
        }
    }

    /// Create a response with a very long direct_response
    pub fn with_long_response(length: usize) -> Self {
        let long_text = "a".repeat(length);
        Self::with_direct_response(&long_text)
    }

    /// Create a response with an invalid/malformed tool call
    pub fn with_invalid_tool_call() -> Self {
        Self {
            content: None,
            tool_calls: Some(vec![chat_completion::ToolCall {
                id: "call_test_invalid".to_string(),
                r#type: "function".to_string(),
                function: chat_completion::ToolCallFunction {
                    name: Some("nonexistent_tool".to_string()),
                    arguments: Some("invalid json {{{".to_string()),
                },
            }]),
        }
    }

    /// Create a response with missing function name (LLM malformed)
    pub fn with_missing_function_name() -> Self {
        Self {
            content: None,
            tool_calls: Some(vec![chat_completion::ToolCall {
                id: "call_test_no_name".to_string(),
                r#type: "function".to_string(),
                function: chat_completion::ToolCallFunction {
                    name: None, // Missing function name
                    arguments: Some("{}".to_string()),
                },
            }]),
        }
    }

    /// Create a response with missing arguments (LLM malformed)
    pub fn with_missing_arguments() -> Self {
        Self {
            content: None,
            tool_calls: Some(vec![chat_completion::ToolCall {
                id: "call_test_no_args".to_string(),
                r#type: "function".to_string(),
                function: chat_completion::ToolCallFunction {
                    name: Some("ask_perplexity".to_string()),
                    arguments: None, // Missing arguments
                },
            }]),
        }
    }

    /// Create a response with malformed JSON arguments for a specific tool
    pub fn with_malformed_json_arguments(tool_name: &str) -> Self {
        Self {
            content: None,
            tool_calls: Some(vec![chat_completion::ToolCall {
                id: "call_test_bad_json".to_string(),
                r#type: "function".to_string(),
                function: chat_completion::ToolCallFunction {
                    name: Some(tool_name.to_string()),
                    arguments: Some("{invalid json".to_string()),
                },
            }]),
        }
    }

    /// Convert to full ChatCompletionResponse for use with ProcessSmsOptions::test_with_mock()
    pub fn to_response(&self) -> chat_completion::ChatCompletionResponse {
        use openai_api_rs::v1::common;
        chat_completion::ChatCompletionResponse {
            id: Some("test_response_id".to_string()),
            object: "chat.completion".to_string(),
            created: 0,
            model: "test-model".to_string(),
            choices: vec![chat_completion::ChatCompletionChoice {
                index: 0,
                message: chat_completion::ChatCompletionMessageForResponse {
                    role: chat_completion::MessageRole::assistant,
                    content: self.content.clone(),
                    name: None,
                    tool_calls: self.tool_calls.clone(),
                },
                finish_reason: Some(chat_completion::FinishReason::tool_calls),
                finish_details: None,
            }],
            usage: common::Usage {
                prompt_tokens: 100,
                completion_tokens: 50,
                total_tokens: 150,
            },
            system_fingerprint: None,
            headers: None,
        }
    }
}

/// Phone number prefixes for testing different countries
/// NOTE: These use valid area codes recognized by phonenumber library
/// All countries use unified credit budget (25.0) with callback-based SMS deduction.
pub mod test_phone_numbers {
    pub const US: &str = "+14155551234";
    pub const CANADA: &str = "+16475551234";
    pub const FINLAND: &str = "+358401234567";
    pub const UK: &str = "+447911123456";
    pub const NETHERLANDS: &str = "+31612345678";
    pub const AUSTRALIA: &str = "+61412345678";
    pub const GERMANY: &str = "+4915123456789";
    pub const FRANCE: &str = "+33612345678";
}

/// Test user creation parameters
#[derive(Debug, Clone)]
pub struct TestUserParams {
    pub email: String,
    pub phone_number: String,
    pub credits: f32,
    pub credits_left: f32,
    pub sub_tier: Option<String>,
}

impl TestUserParams {
    /// Create params for a US user
    pub fn us_user(credits_left: f32, credits: f32) -> Self {
        Self {
            email: "test_us@example.com".to_string(),
            phone_number: test_phone_numbers::US.to_string(),
            credits,
            credits_left,
            sub_tier: Some("tier 2".to_string()),
        }
    }

    /// Create params for a Finland user
    pub fn finland_user(credits_left: f32, credits: f32) -> Self {
        Self {
            email: "test_fi@example.com".to_string(),
            phone_number: test_phone_numbers::FINLAND.to_string(),
            credits,
            credits_left,
            sub_tier: Some("tier 2".to_string()),
        }
    }

    /// Create params for a UK user
    pub fn uk_user(credits_left: f32, credits: f32) -> Self {
        Self {
            email: "test_uk@example.com".to_string(),
            phone_number: test_phone_numbers::UK.to_string(),
            credits,
            credits_left,
            sub_tier: Some("tier 2".to_string()),
        }
    }

    /// Create params for a Germany user (notification-only)
    pub fn germany_user(credits_left: f32, credits: f32) -> Self {
        Self {
            email: "test_de@example.com".to_string(),
            phone_number: test_phone_numbers::GERMANY.to_string(),
            credits,
            credits_left,
            sub_tier: Some("tier 2".to_string()),
        }
    }

    /// Create params for a US user with custom sub_tier
    pub fn us_user_with_tier(credits_left: f32, credits: f32, sub_tier: Option<String>) -> Self {
        Self {
            email: "test_us_custom@example.com".to_string(),
            phone_number: test_phone_numbers::US.to_string(),
            credits,
            credits_left,
            sub_tier,
        }
    }

    /// Create params for a Canada user
    pub fn canada_user(credits_left: f32, credits: f32) -> Self {
        Self {
            email: "test_ca@example.com".to_string(),
            phone_number: test_phone_numbers::CANADA.to_string(),
            credits,
            credits_left,
            sub_tier: Some("tier 2".to_string()),
        }
    }

    /// Create params for a Netherlands user
    pub fn netherlands_user(credits_left: f32, credits: f32) -> Self {
        Self {
            email: "test_nl@example.com".to_string(),
            phone_number: test_phone_numbers::NETHERLANDS.to_string(),
            credits,
            credits_left,
            sub_tier: Some("tier 2".to_string()),
        }
    }

    /// Create params for an Australia user
    pub fn australia_user(credits_left: f32, credits: f32) -> Self {
        Self {
            email: "test_au@example.com".to_string(),
            phone_number: test_phone_numbers::AUSTRALIA.to_string(),
            credits,
            credits_left,
            sub_tier: Some("tier 2".to_string()),
        }
    }

    /// Create params for a France user (notification-only)
    pub fn france_user(credits_left: f32, credits: f32) -> Self {
        Self {
            email: "test_fr@example.com".to_string(),
            phone_number: test_phone_numbers::FRANCE.to_string(),
            credits,
            credits_left,
            sub_tier: Some("tier 2".to_string()),
        }
    }
}

/// Deactivate phone service for a user (for testing stolen phone scenario)
pub fn deactivate_phone_service(state: &Arc<crate::AppState>, user_id: i32) {
    state
        .user_core
        .update_phone_service_active(user_id, false)
        .expect("Failed to deactivate phone service");
}

/// Set up test encryption key (call once before BYOT tests)
pub fn setup_test_encryption() {
    use std::sync::Once;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        // 32-byte key base64-encoded for AES-256: "12345678901234567890123456789012"
        std::env::set_var(
            "ENCRYPTION_KEY",
            "MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI=",
        );
    });
}

/// Set BYOT credentials for a user (user pays Twilio directly, skips credit check)
pub fn set_byot_credentials(state: &Arc<crate::AppState>, user_id: i32) {
    setup_test_encryption();
    state
        .user_core
        .update_twilio_credentials(user_id, "AC_test_sid", "test_auth_token")
        .expect("Failed to set BYOT credentials");
    // Also set plan_type to "byot" so is_byot_user() returns true
    set_plan_type(state, user_id, "byot");
}

/// Set the plan_type for a user (used for BYOT testing)
pub fn set_plan_type(state: &Arc<crate::AppState>, user_id: i32, plan_type: &str) {
    use crate::schema::users;
    use diesel::prelude::*;

    let mut conn = state.db_pool.get().expect("Failed to get DB connection");
    diesel::update(users::table.filter(users::id.eq(user_id)))
        .set(users::plan_type.eq(Some(plan_type.to_string())))
        .execute(&mut conn)
        .expect("Failed to set plan_type");
}

/// Set the preferred_number for a user
pub fn set_preferred_number(state: &Arc<crate::AppState>, user_id: i32, number: &str) {
    use crate::schema::users;
    use diesel::prelude::*;

    let mut conn = state.db_pool.get().expect("Failed to get DB connection");
    diesel::update(users::table.filter(users::id.eq(user_id)))
        .set(users::preferred_number.eq(Some(number.to_string())))
        .execute(&mut conn)
        .expect("Failed to set preferred_number");
}

// =============================================================================
// Behavioral Test Assertions
// =============================================================================

/// Assert response is SMS-deliverable (length and non-empty)
pub fn assert_sms_deliverable(body: &str) {
    assert!(
        body.len() <= 480,
        "Response exceeds SMS limit: {} chars",
        body.len()
    );
    assert!(!body.is_empty(), "Response is empty");
}

/// Assert user was charged (credits decreased)
pub fn assert_charged(before_credits: f32, after_credits: f32) {
    assert!(
        after_credits < before_credits,
        "Expected credits to decrease: before={}, after={}",
        before_credits,
        after_credits
    );
}

/// Assert user was NOT charged (credits unchanged)
pub fn assert_not_charged(before_credits: f32, after_credits: f32) {
    assert!(
        (before_credits - after_credits).abs() < 0.001,
        "Expected credits unchanged: before={}, after={}",
        before_credits,
        after_credits
    );
}

/// Assert no user content leaked in response
pub fn assert_no_content_leak(user_input: &str, response_body: &str) {
    // Only check if user input is substantial enough to be a leak
    if user_input.len() > 3 {
        assert!(
            !response_body.contains(user_input),
            "Response leaked user content: found '{}' in response",
            user_input
        );
    }
}

/// Get total credits for a user (credits + credits_left)
pub fn get_total_credits(state: &Arc<crate::AppState>, user_id: i32) -> f32 {
    let user = state
        .user_core
        .find_by_id(user_id)
        .expect("Failed to get user")
        .expect("User not found");
    user.credits + user.credits_left
}

// =============================================================================
// Mock UserCore for Unit Testing
// =============================================================================

pub mod mock_user_core {
    use crate::handlers::filter_handlers::PriorityNotificationInfo;
    use crate::handlers::profile_handlers::CriticalNotificationInfo;
    use crate::models::user_models::{User, UserInfo, UserSettings};
    use crate::repositories::user_core::{DigestSettings, UpdateProfileParams, UserCoreOps};
    use diesel::result::Error as DieselError;
    use std::collections::HashMap;
    use std::error::Error;
    use std::sync::Mutex;

    /// Records all calls made to MockUserCore for assertions
    #[derive(Debug, Clone, Default)]
    pub struct MockCallRecord {
        pub find_by_id_calls: Vec<i32>,
        pub find_by_email_calls: Vec<String>,
        pub find_by_phone_number_calls: Vec<String>,
        pub is_byot_user_calls: Vec<i32>,
        pub get_phone_service_active_calls: Vec<i32>,
        pub get_user_settings_calls: Vec<i32>,
        pub get_user_info_calls: Vec<i32>,
        pub create_user_calls: Vec<String>, // email
        pub update_preferred_number_calls: Vec<(i32, String)>,
    }

    /// Mock implementation of UserCoreOps for testing
    pub struct MockUserCore {
        pub calls: Mutex<MockCallRecord>,

        // Configurable responses
        pub users: Mutex<HashMap<i32, User>>,
        pub users_by_phone: Mutex<HashMap<String, User>>,
        pub users_by_email: Mutex<HashMap<String, User>>,
        pub user_settings: Mutex<HashMap<i32, UserSettings>>,
        pub user_info: Mutex<HashMap<i32, UserInfo>>,
        pub byot_users: Mutex<Vec<i32>>,
        pub phone_service_active: Mutex<HashMap<i32, bool>>,
        pub quiet_mode: Mutex<HashMap<i32, Option<i32>>>,
        pub llm_provider: Mutex<HashMap<i32, String>>,

        // Error injection
        pub find_by_id_error: Mutex<Option<DieselError>>,
        pub find_by_phone_error: Mutex<Option<DieselError>>,
    }

    impl Default for MockUserCore {
        fn default() -> Self {
            Self::new()
        }
    }

    impl MockUserCore {
        pub fn new() -> Self {
            Self {
                calls: Mutex::new(MockCallRecord::default()),
                users: Mutex::new(HashMap::new()),
                users_by_phone: Mutex::new(HashMap::new()),
                users_by_email: Mutex::new(HashMap::new()),
                user_settings: Mutex::new(HashMap::new()),
                user_info: Mutex::new(HashMap::new()),
                byot_users: Mutex::new(Vec::new()),
                phone_service_active: Mutex::new(HashMap::new()),
                quiet_mode: Mutex::new(HashMap::new()),
                llm_provider: Mutex::new(HashMap::new()),
                find_by_id_error: Mutex::new(None),
                find_by_phone_error: Mutex::new(None),
            }
        }

        // Builder methods for test setup
        pub fn with_user(self, user: User) -> Self {
            let id = user.id;
            let phone = user.phone_number.clone();
            let email = user.email.clone();
            self.users.lock().unwrap().insert(id, user.clone());
            self.users_by_phone
                .lock()
                .unwrap()
                .insert(phone, user.clone());
            self.users_by_email.lock().unwrap().insert(email, user);
            self
        }

        pub fn with_byot_user(self, user_id: i32) -> Self {
            self.byot_users.lock().unwrap().push(user_id);
            self
        }

        pub fn with_phone_service_active(self, user_id: i32, active: bool) -> Self {
            self.phone_service_active
                .lock()
                .unwrap()
                .insert(user_id, active);
            self
        }

        pub fn with_user_settings(self, user_id: i32, settings: UserSettings) -> Self {
            self.user_settings.lock().unwrap().insert(user_id, settings);
            self
        }

        pub fn with_user_info(self, user_id: i32, info: UserInfo) -> Self {
            self.user_info.lock().unwrap().insert(user_id, info);
            self
        }

        pub fn get_calls(&self) -> MockCallRecord {
            self.calls.lock().unwrap().clone()
        }

        pub fn clear_calls(&self) {
            *self.calls.lock().unwrap() = MockCallRecord::default();
        }
    }

    impl UserCoreOps for MockUserCore {
        fn find_by_id(&self, user_id: i32) -> Result<Option<User>, DieselError> {
            self.calls.lock().unwrap().find_by_id_calls.push(user_id);
            if let Some(err) = self.find_by_id_error.lock().unwrap().take() {
                return Err(err);
            }
            Ok(self.users.lock().unwrap().get(&user_id).cloned())
        }

        fn find_by_email(&self, email: &str) -> Result<Option<User>, DieselError> {
            self.calls
                .lock()
                .unwrap()
                .find_by_email_calls
                .push(email.to_string());
            Ok(self.users_by_email.lock().unwrap().get(email).cloned())
        }

        fn find_by_phone_number(&self, phone: &str) -> Result<Option<User>, DieselError> {
            self.calls
                .lock()
                .unwrap()
                .find_by_phone_number_calls
                .push(phone.to_string());
            if let Some(err) = self.find_by_phone_error.lock().unwrap().take() {
                return Err(err);
            }
            Ok(self.users_by_phone.lock().unwrap().get(phone).cloned())
        }

        fn find_by_magic_token(&self, _token: &str) -> Result<Option<User>, DieselError> {
            Ok(None)
        }

        fn get_all_users(&self) -> Result<Vec<User>, DieselError> {
            Ok(self.users.lock().unwrap().values().cloned().collect())
        }

        fn get_users_by_tier(&self, tier: &str) -> Result<Vec<User>, DieselError> {
            let users: Vec<User> = self
                .users
                .lock()
                .unwrap()
                .values()
                .filter(|u| u.sub_tier.as_deref() == Some(tier))
                .cloned()
                .collect();
            Ok(users)
        }

        fn create_user(
            &self,
            new_user: crate::handlers::auth_dtos::NewUser,
        ) -> Result<(), DieselError> {
            self.calls
                .lock()
                .unwrap()
                .create_user_calls
                .push(new_user.email.clone());
            Ok(())
        }

        fn delete_user(&self, _user_id: i32) -> Result<(), DieselError> {
            Ok(())
        }

        fn verify_user(&self, _user_id: i32) -> Result<(), DieselError> {
            Ok(())
        }

        fn update_password(&self, _user_id: i32, _password_hash: &str) -> Result<(), DieselError> {
            Ok(())
        }

        fn update_phone_number(&self, _user_id: i32, _phone: &str) -> Result<(), DieselError> {
            Ok(())
        }

        fn update_nickname(&self, _user_id: i32, _nickname: &str) -> Result<(), DieselError> {
            Ok(())
        }

        fn update_preferred_number(&self, user_id: i32, number: &str) -> Result<(), DieselError> {
            self.calls
                .lock()
                .unwrap()
                .update_preferred_number_calls
                .push((user_id, number.to_string()));
            Ok(())
        }

        fn update_discount_tier(
            &self,
            _user_id: i32,
            _tier: Option<&str>,
        ) -> Result<(), DieselError> {
            Ok(())
        }

        fn ensure_user_info_exists(&self, _user_id: i32) -> Result<(), DieselError> {
            Ok(())
        }

        fn get_user_info(&self, user_id: i32) -> Result<UserInfo, DieselError> {
            self.calls.lock().unwrap().get_user_info_calls.push(user_id);
            self.user_info
                .lock()
                .unwrap()
                .get(&user_id)
                .cloned()
                .ok_or(DieselError::NotFound)
        }

        fn update_info(&self, _user_id: i32, _info: &str) -> Result<(), DieselError> {
            Ok(())
        }

        fn update_location(&self, _user_id: i32, _location: &str) -> Result<(), DieselError> {
            Ok(())
        }

        fn update_user_coordinates(
            &self,
            _user_id: i32,
            _lat: f32,
            _lon: f32,
        ) -> Result<(), DieselError> {
            Ok(())
        }

        fn update_nearby_places(&self, _user_id: i32, _places: &str) -> Result<(), DieselError> {
            Ok(())
        }

        fn update_timezone(&self, _user_id: i32, _tz: &str) -> Result<(), DieselError> {
            Ok(())
        }

        fn update_timezone_auto(&self, _user_id: i32, _auto: bool) -> Result<(), DieselError> {
            Ok(())
        }

        fn ensure_user_settings_exist(&self, _user_id: i32) -> Result<(), DieselError> {
            Ok(())
        }

        fn get_user_settings(&self, user_id: i32) -> Result<UserSettings, DieselError> {
            self.calls
                .lock()
                .unwrap()
                .get_user_settings_calls
                .push(user_id);
            self.user_settings
                .lock()
                .unwrap()
                .get(&user_id)
                .cloned()
                .ok_or(DieselError::NotFound)
        }

        fn update_notify(&self, _user_id: i32, _notify: bool) -> Result<(), DieselError> {
            Ok(())
        }

        fn update_agent_language(&self, _user_id: i32, _lang: &str) -> Result<(), DieselError> {
            Ok(())
        }

        fn update_save_context(&self, _user_id: i32, _ctx: i32) -> Result<(), DieselError> {
            Ok(())
        }

        fn update_notification_type(
            &self,
            _user_id: i32,
            _ntype: Option<&str>,
        ) -> Result<(), DieselError> {
            Ok(())
        }

        fn update_llm_provider(&self, user_id: i32, provider: &str) -> Result<(), DieselError> {
            self.llm_provider
                .lock()
                .unwrap()
                .insert(user_id, provider.to_string());
            Ok(())
        }

        fn get_llm_provider(&self, user_id: i32) -> Result<Option<String>, DieselError> {
            Ok(self.llm_provider.lock().unwrap().get(&user_id).cloned())
        }

        fn update_phone_service_active(
            &self,
            _user_id: i32,
            _active: bool,
        ) -> Result<(), DieselError> {
            Ok(())
        }

        fn get_phone_service_active(&self, user_id: i32) -> Result<bool, DieselError> {
            self.calls
                .lock()
                .unwrap()
                .get_phone_service_active_calls
                .push(user_id);
            Ok(*self
                .phone_service_active
                .lock()
                .unwrap()
                .get(&user_id)
                .unwrap_or(&true))
        }

        fn get_default_notification_mode(&self, _user_id: i32) -> Result<String, DieselError> {
            Ok("critical".to_string())
        }

        fn set_default_notification_mode(
            &self,
            _user_id: i32,
            _mode: &str,
        ) -> Result<(), DieselError> {
            Ok(())
        }

        fn get_default_notification_type(&self, _user_id: i32) -> Result<String, DieselError> {
            Ok("sms".to_string())
        }

        fn set_default_notification_type(
            &self,
            _user_id: i32,
            _ntype: &str,
        ) -> Result<(), DieselError> {
            Ok(())
        }

        fn get_default_notify_on_call(&self, _user_id: i32) -> Result<bool, DieselError> {
            Ok(true)
        }

        fn set_default_notify_on_call(
            &self,
            _user_id: i32,
            _notify: bool,
        ) -> Result<(), DieselError> {
            Ok(())
        }

        fn get_phone_contact_notification_mode(
            &self,
            _user_id: i32,
        ) -> Result<String, DieselError> {
            Ok("critical".to_string())
        }

        fn set_phone_contact_notification_mode(
            &self,
            _user_id: i32,
            _mode: &str,
        ) -> Result<(), DieselError> {
            Ok(())
        }

        fn get_phone_contact_notification_type(
            &self,
            _user_id: i32,
        ) -> Result<String, DieselError> {
            Ok("sms".to_string())
        }

        fn set_phone_contact_notification_type(
            &self,
            _user_id: i32,
            _ntype: &str,
        ) -> Result<(), DieselError> {
            Ok(())
        }

        fn get_phone_contact_notify_on_call(&self, _user_id: i32) -> Result<bool, DieselError> {
            Ok(true)
        }

        fn set_phone_contact_notify_on_call(
            &self,
            _user_id: i32,
            _notify: bool,
        ) -> Result<(), DieselError> {
            Ok(())
        }

        fn get_call_notify(&self, _user_id: i32) -> Result<bool, DieselError> {
            Ok(true)
        }

        fn update_call_notify(&self, _user_id: i32, _notify: bool) -> Result<(), DieselError> {
            Ok(())
        }

        fn update_digests(
            &self,
            _user_id: i32,
            _morning: Option<&str>,
            _day: Option<&str>,
            _evening: Option<&str>,
        ) -> Result<(), DieselError> {
            Ok(())
        }

        fn get_digests(&self, _user_id: i32) -> Result<DigestSettings, DieselError> {
            Ok((None, None, None))
        }

        fn get_last_instant_digest_time(&self, _user_id: i32) -> Result<Option<i32>, DieselError> {
            Ok(None)
        }

        fn set_last_instant_digest_time(&self, _user_id: i32, _ts: i32) -> Result<(), DieselError> {
            Ok(())
        }

        fn update_proactive_agent_on(
            &self,
            _user_id: i32,
            _enabled: bool,
        ) -> Result<(), DieselError> {
            Ok(())
        }

        fn get_proactive_agent_on(&self, _user_id: i32) -> Result<bool, DieselError> {
            Ok(true)
        }

        fn update_critical_enabled(
            &self,
            _user_id: i32,
            _enabled: Option<String>,
        ) -> Result<(), DieselError> {
            Ok(())
        }

        fn update_action_on_critical_message(
            &self,
            _user_id: i32,
            _action: Option<String>,
        ) -> Result<(), DieselError> {
            Ok(())
        }

        fn get_critical_notification_info(
            &self,
            _user_id: i32,
        ) -> Result<CriticalNotificationInfo, DieselError> {
            Ok(CriticalNotificationInfo {
                enabled: Some("sms".to_string()),
                average_critical_per_day: 1.0,
                estimated_monthly_price: 5.0,
                call_notify: true,
                action_on_critical_message: None,
            })
        }

        fn get_priority_notification_info(
            &self,
            _user_id: i32,
        ) -> Result<PriorityNotificationInfo, DieselError> {
            Ok(PriorityNotificationInfo {
                average_per_day: 0.5,
                estimated_monthly_price: 2.0,
            })
        }

        fn update_profile(&self, _params: UpdateProfileParams<'_>) -> Result<(), DieselError> {
            Ok(())
        }

        fn get_twilio_credentials(
            &self,
            _user_id: i32,
        ) -> Result<(String, String), Box<dyn Error + Send + Sync>> {
            Ok(("test_sid".to_string(), "test_token".to_string()))
        }

        fn has_twilio_credentials(&self, _user_id: i32) -> bool {
            false
        }

        fn is_byot_user(&self, user_id: i32) -> bool {
            self.calls.lock().unwrap().is_byot_user_calls.push(user_id);
            self.byot_users.lock().unwrap().contains(&user_id)
        }

        fn update_twilio_credentials(
            &self,
            _user_id: i32,
            _sid: &str,
            _token: &str,
        ) -> Result<(), Box<dyn Error + Send + Sync>> {
            Ok(())
        }

        fn clear_twilio_credentials(
            &self,
            _user_id: i32,
        ) -> Result<(), Box<dyn Error + Send + Sync>> {
            Ok(())
        }

        fn get_textbee_credentials(
            &self,
            _user_id: i32,
        ) -> Result<(String, String), Box<dyn Error + Send + Sync>> {
            Err("Not found".into())
        }

        fn update_textbee_credentials(
            &self,
            _user_id: i32,
            _device_id: &str,
            _api_key: &str,
        ) -> Result<(), Box<dyn Error + Send + Sync>> {
            Ok(())
        }

        fn get_openrouter_api_key(
            &self,
            _user_id: i32,
        ) -> Result<String, Box<dyn Error + Send + Sync>> {
            Err("Not found".into())
        }

        fn get_elevenlabs_phone_number_id(
            &self,
            _user_id: i32,
        ) -> Result<Option<String>, DieselError> {
            Ok(None)
        }

        fn set_elevenlabs_phone_number_id(
            &self,
            _user_id: i32,
            _id: &str,
        ) -> Result<(), DieselError> {
            Ok(())
        }

        fn update_subscription_tier(
            &self,
            _user_id: i32,
            _tier: Option<&str>,
        ) -> Result<(), DieselError> {
            Ok(())
        }

        fn update_next_billing_date(&self, _user_id: i32, _ts: i32) -> Result<(), DieselError> {
            Ok(())
        }

        fn get_next_billing_date(&self, _user_id: i32) -> Result<Option<i32>, DieselError> {
            Ok(None)
        }

        fn update_last_credits_notification(
            &self,
            _user_id: i32,
            _ts: i32,
        ) -> Result<(), DieselError> {
            Ok(())
        }

        fn clear_last_credits_notification(&self, _user_id: i32) -> Result<(), DieselError> {
            Ok(())
        }

        fn update_auto_topup(
            &self,
            _user_id: i32,
            _active: bool,
            _amount: Option<f32>,
        ) -> Result<(), DieselError> {
            Ok(())
        }

        fn increment_monthly_message_count(&self, _user_id: i32) -> Result<(), DieselError> {
            Ok(())
        }

        fn reset_monthly_message_count(&self, _user_id: i32) -> Result<(), DieselError> {
            Ok(())
        }

        fn email_exists(&self, email: &str) -> Result<bool, DieselError> {
            Ok(self.users_by_email.lock().unwrap().contains_key(email))
        }

        fn phone_number_exists(&self, phone: &str) -> Result<bool, DieselError> {
            Ok(self.users_by_phone.lock().unwrap().contains_key(phone))
        }

        fn is_admin(&self, _user_id: i32) -> Result<bool, DieselError> {
            Ok(false)
        }

        fn update_sub_country(
            &self,
            _user_id: i32,
            _country: Option<&str>,
        ) -> Result<(), DieselError> {
            Ok(())
        }

        fn set_preferred_number_to_us_default(
            &self,
            _user_id: i32,
        ) -> Result<String, Box<dyn Error + Send + Sync>> {
            Ok("+14155551234".to_string())
        }

        fn set_preferred_number_for_country(
            &self,
            _user_id: i32,
            _country: &str,
        ) -> Result<Option<String>, Box<dyn Error + Send + Sync>> {
            Ok(Some("+14155551234".to_string()))
        }

        fn set_magic_token(&self, _user_id: i32, _token: &str) -> Result<(), DieselError> {
            Ok(())
        }

        fn set_quiet_mode(&self, user_id: i32, until: Option<i32>) -> Result<(), DieselError> {
            self.quiet_mode.lock().unwrap().insert(user_id, until);
            Ok(())
        }

        fn get_quiet_mode(&self, user_id: i32) -> Result<Option<i32>, DieselError> {
            Ok(self
                .quiet_mode
                .lock()
                .unwrap()
                .get(&user_id)
                .copied()
                .flatten())
        }
    }
}

// =============================================================================
// Task Testing Helpers
// =============================================================================

/// Builder for test task parameters
#[derive(Debug, Clone)]
pub struct TestTaskParams {
    pub user_id: i32,
    pub trigger: String,
    pub action: String,
    pub notification_type: Option<String>,
    pub is_permanent: Option<i32>,
    pub recurrence_rule: Option<String>,
    pub recurrence_time: Option<String>,
    pub sources: Option<String>,
    pub condition: Option<String>,
    pub end_time: Option<i32>,
}

impl TestTaskParams {
    /// Create a one-time task with given trigger timestamp
    pub fn once_task(user_id: i32, trigger_ts: i32) -> Self {
        Self {
            user_id,
            trigger: format!("once_{}", trigger_ts),
            action: "test_action".to_string(),
            notification_type: Some("sms".to_string()),
            is_permanent: Some(0),
            recurrence_rule: None,
            recurrence_time: None,
            sources: None,
            condition: None,
            end_time: None,
        }
    }

    /// Create a permanent daily recurring task
    pub fn permanent_daily(user_id: i32, time: &str) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;
        Self {
            user_id,
            trigger: format!("once_{}", now + 86400), // Tomorrow
            action: "test_action".to_string(),
            notification_type: Some("sms".to_string()),
            is_permanent: Some(1),
            recurrence_rule: Some("daily".to_string()),
            recurrence_time: Some(time.to_string()),
            sources: None,
            condition: None,
            end_time: None,
        }
    }

    /// Create a digest task (permanent daily with sources)
    pub fn digest_task(user_id: i32, time: &str) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;
        Self {
            user_id,
            trigger: format!("once_{}", now + 86400),
            action: "generate_digest".to_string(),
            notification_type: Some("sms".to_string()),
            is_permanent: Some(1),
            recurrence_rule: Some("daily".to_string()),
            recurrence_time: Some(time.to_string()),
            sources: Some("email,whatsapp,telegram,signal,calendar".to_string()),
            condition: None,
            end_time: None,
        }
    }

    /// Create a quiet mode task
    pub fn quiet_mode_task(user_id: i32, end_time: Option<i32>) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i32;
        Self {
            user_id,
            trigger: format!("once_{}", now),
            action: r#"{"tool":"quiet_mode"}"#.to_string(),
            notification_type: None,
            is_permanent: None,
            recurrence_rule: None,
            recurrence_time: None,
            sources: None,
            condition: None,
            end_time,
        }
    }

    /// Add sources to the task
    pub fn with_sources(mut self, sources: &str) -> Self {
        self.sources = Some(sources.to_string());
        self
    }

    /// Add condition to the task
    pub fn with_condition(mut self, condition: &str) -> Self {
        self.condition = Some(condition.to_string());
        self
    }

    /// Set the action
    pub fn with_action(mut self, action: &str) -> Self {
        self.action = action.to_string();
        self
    }

    /// Set notification type
    pub fn with_notification_type(mut self, ntype: &str) -> Self {
        self.notification_type = Some(ntype.to_string());
        self
    }
}

/// Create a test task in the database from TestTaskParams
pub fn create_test_task(
    state: &Arc<crate::AppState>,
    params: &TestTaskParams,
) -> crate::models::user_models::Task {
    use crate::models::user_models::NewTask;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let new_task = NewTask {
        user_id: params.user_id,
        trigger: params.trigger.clone(),
        condition: params.condition.clone(),
        action: params.action.clone(),
        notification_type: params.notification_type.clone(),
        status: "active".to_string(),
        created_at: now,
        is_permanent: params.is_permanent,
        recurrence_rule: params.recurrence_rule.clone(),
        recurrence_time: params.recurrence_time.clone(),
        sources: params.sources.clone(),
        end_time: params.end_time,
    };

    state
        .user_repository
        .create_task(&new_task)
        .expect("Failed to create test task");

    // Return the created task (get most recent)
    state
        .user_repository
        .get_user_tasks(params.user_id)
        .expect("Failed to get user tasks")
        .into_iter()
        .next()
        .expect("No tasks found after creation")
}

/// Get all tasks for a user
pub fn get_user_tasks(
    state: &Arc<crate::AppState>,
    user_id: i32,
) -> Vec<crate::models::user_models::Task> {
    state
        .user_repository
        .get_user_tasks(user_id)
        .expect("Failed to get user tasks")
}

// =============================================================================
// Item Testing Helpers
// =============================================================================

/// Builder for test item parameters
#[derive(Debug, Clone)]
pub struct TestItemParams {
    pub user_id: i32,
    pub summary: String,
    pub monitor: bool,
    pub next_check_at: Option<i32>,
    pub priority: i32,
    pub source_id: Option<String>,
}

impl TestItemParams {
    /// Simple reminder item
    pub fn reminder(user_id: i32, summary: &str) -> Self {
        Self {
            user_id,
            summary: summary.to_string(),
            monitor: false,
            next_check_at: None,
            priority: 0,
            source_id: None,
        }
    }

    /// Scheduled reminder (fires at next_check_at)
    pub fn scheduled_reminder(user_id: i32, summary: &str, trigger_at: i32) -> Self {
        Self {
            user_id,
            summary: summary.to_string(),
            monitor: false,
            next_check_at: Some(trigger_at),
            priority: 0,
            source_id: None,
        }
    }

    /// Digest item
    pub fn digest(user_id: i32, summary: &str, trigger_at: i32) -> Self {
        Self {
            user_id,
            summary: summary.to_string(),
            monitor: false,
            next_check_at: Some(trigger_at),
            priority: 0,
            source_id: None,
        }
    }

    /// Monitor item (matches against incoming data)
    pub fn monitor(user_id: i32, summary: &str) -> Self {
        Self {
            user_id,
            summary: summary.to_string(),
            monitor: true,
            next_check_at: None,
            priority: 0,
            source_id: None,
        }
    }

    /// Alert item (system alerts like bridge disconnect)
    pub fn alert(user_id: i32, summary: &str) -> Self {
        Self {
            user_id,
            summary: summary.to_string(),
            monitor: false,
            next_check_at: None,
            priority: 1,
            source_id: None,
        }
    }

    /// Email-sourced item (with source_id for dedup)
    pub fn from_email(user_id: i32, summary: &str, source_id: &str) -> Self {
        Self {
            user_id,
            summary: summary.to_string(),
            monitor: false,
            next_check_at: None,
            priority: 0,
            source_id: Some(source_id.to_string()),
        }
    }

    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_next_check_at(mut self, ts: i32) -> Self {
        self.next_check_at = Some(ts);
        self
    }

    pub fn with_source_id(mut self, source_id: &str) -> Self {
        self.source_id = Some(source_id.to_string());
        self
    }

    pub fn with_monitor(mut self, monitor: bool) -> Self {
        self.monitor = monitor;
        self
    }
}

/// Create a test item in the database from TestItemParams, returns the item
pub fn create_test_item(
    state: &Arc<crate::AppState>,
    params: &TestItemParams,
) -> crate::models::user_models::Item {
    use crate::models::user_models::NewItem;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let new_item = NewItem {
        user_id: params.user_id,
        summary: params.summary.clone(),
        monitor: params.monitor,
        next_check_at: params.next_check_at,
        priority: params.priority,
        source_id: params.source_id.clone(),
        created_at: now,
    };

    let item_id = state
        .item_repository
        .create_item(&new_item)
        .expect("Failed to create test item");

    state
        .item_repository
        .get_item(item_id, params.user_id)
        .expect("Failed to get test item")
        .expect("Item not found after creation")
}

/// Get all items for a user
pub fn get_user_items(
    state: &Arc<crate::AppState>,
    user_id: i32,
) -> Vec<crate::models::user_models::Item> {
    state
        .item_repository
        .get_items(user_id)
        .expect("Failed to get user items")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_direct_response() {
        let mock = MockLlmResponse::with_direct_response("Hello, world!");
        assert!(mock.tool_calls.is_some());

        let tool_calls = mock.tool_calls.as_ref().unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(
            tool_calls[0].function.name,
            Some("direct_response".to_string())
        );
    }

    #[test]
    fn test_mock_to_response() {
        let mock = MockLlmResponse::with_direct_response("Test");
        let response = mock.to_response();

        assert_eq!(response.choices.len(), 1);
        assert!(response.choices[0].message.tool_calls.is_some());
        assert_eq!(
            response.choices[0].finish_reason,
            Some(chat_completion::FinishReason::tool_calls)
        );
    }

    #[test]
    fn test_us_user_params() {
        let params = TestUserParams::us_user(10.0, 5.0);
        assert!(params.phone_number.starts_with("+1"));
        assert_eq!(params.credits_left, 10.0);
        assert_eq!(params.credits, 5.0);
        assert_eq!(params.sub_tier, Some("tier 2".to_string()));
    }

    #[test]
    fn test_finland_user_params() {
        let params = TestUserParams::finland_user(5.0, 2.5);
        assert!(params.phone_number.starts_with("+358"));
    }
}
