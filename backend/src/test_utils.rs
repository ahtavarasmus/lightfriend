//! Test utilities for SMS e2e tests
//!
//! Provides mock LLM responses and test state setup for integration tests.

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
    let totp_repository = Arc::new(crate::repositories::totp_repository::TotpRepository::new(
        pool.clone(),
    ));
    let webauthn_repository =
        Arc::new(crate::repositories::webauthn_repository::WebauthnRepository::new(pool.clone()));

    let google_oauth = create_dummy_google_oauth_client();
    let tesla_oauth = create_dummy_tesla_oauth_client();

    Arc::new(crate::AppState {
        db_pool: pool,
        user_core,
        user_repository,
        twilio_client: Arc::new(crate::RealTwilioClient::new()),
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
        pending_totp_logins: DashMap::new(),
        pending_password_resets: DashMap::new(),
        session_to_token: DashMap::new(),
        totp_verify_limiter: DashMap::new(),
        webauthn_verify_limiter: DashMap::new(),
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

/// Phone number prefixes for testing different country pricing
pub mod test_phone_numbers {
    /// US phone number (+1 prefix) - uses credits_left count-based pricing
    pub const US: &str = "+15551234567";
    /// Finland phone number (+358 prefix) - uses euro segment pricing
    pub const FINLAND: &str = "+358401234567";
    /// UK phone number (+44 prefix) - uses euro segment pricing
    pub const UK: &str = "+447911123456";
    /// Germany phone number (+49 prefix) - notification-only pricing
    pub const GERMANY: &str = "+4915112345678";
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
