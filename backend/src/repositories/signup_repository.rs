//! Repository trait for signup operations.
//!
//! Defines the minimal interface needed by the signup service,
//! enabling mock implementations for unit testing.

use diesel::result::Error as DieselError;
use thiserror::Error;

use crate::handlers::auth_dtos::NewUser;
use crate::models::user_models::User;

/// Error type for signup repository operations
#[derive(Debug, Error)]
pub enum SignupRepositoryError {
    #[error("Database error: {0}")]
    Database(#[from] DieselError),

    #[error("User not found after creation")]
    UserNotFoundAfterCreation,

    #[error("Configuration error: {0}")]
    Config(String),
}

/// Repository trait for signup-related database operations.
///
/// This trait abstracts the database operations needed for user registration,
/// allowing for mock implementations in tests. It combines operations from
/// both UserCore and UserRepository that are needed during signup.
pub trait SignupRepository: Send + Sync {
    // === Find operations ===

    /// Find user by Stripe customer ID
    fn find_by_stripe_customer_id(
        &self,
        customer_id: &str,
    ) -> Result<Option<User>, SignupRepositoryError>;

    /// Find user by email (case-insensitive)
    fn find_by_email(&self, email: &str) -> Result<Option<User>, SignupRepositoryError>;

    /// Find user by phone number
    fn find_by_phone_number(&self, phone: &str) -> Result<Option<User>, SignupRepositoryError>;

    // === Create/Update operations ===

    /// Create a new user record
    fn create_user(&self, new_user: NewUser) -> Result<(), SignupRepositoryError>;

    /// Link user to Stripe customer ID
    fn set_stripe_customer_id(
        &self,
        user_id: i32,
        customer_id: &str,
    ) -> Result<(), SignupRepositoryError>;

    /// Update user's phone number
    fn update_phone_number(&self, user_id: i32, phone: &str) -> Result<(), SignupRepositoryError>;

    /// Set user's preferred number based on their country
    fn set_preferred_number_for_country(
        &self,
        user_id: i32,
        country: &str,
    ) -> Result<(), SignupRepositoryError>;

    /// Set magic token for password setup link
    fn set_magic_token(&self, user_id: i32, token: &str) -> Result<(), SignupRepositoryError>;

    // === Setup operations ===

    /// Ensure user_settings record exists for user
    fn ensure_user_settings_exist(&self, user_id: i32) -> Result<(), SignupRepositoryError>;

    /// Ensure user_info record exists for user
    fn ensure_user_info_exists(&self, user_id: i32) -> Result<(), SignupRepositoryError>;
}
