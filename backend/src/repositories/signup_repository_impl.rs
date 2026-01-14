//! Composite implementation of SignupRepository.
//!
//! Wraps UserCore and UserRepository to provide all signup-related operations
//! through a single trait interface.

use std::sync::Arc;

use crate::handlers::auth_dtos::NewUser;
use crate::models::user_models::User;
use crate::repositories::signup_repository::{SignupRepository, SignupRepositoryError};
use crate::repositories::user_core::UserCore;
use crate::repositories::user_repository::UserRepository;

/// Composite repository that delegates to UserCore and UserRepository.
///
/// This allows the SignupService to work with a single repository interface
/// while the underlying implementation uses the existing repository classes.
pub struct CompositeSignupRepository {
    user_core: Arc<UserCore>,
    user_repository: Arc<UserRepository>,
}

impl CompositeSignupRepository {
    /// Create a new composite repository from existing repositories.
    pub fn new(user_core: Arc<UserCore>, user_repository: Arc<UserRepository>) -> Self {
        Self {
            user_core,
            user_repository,
        }
    }
}

impl SignupRepository for CompositeSignupRepository {
    fn find_by_stripe_customer_id(
        &self,
        customer_id: &str,
    ) -> Result<Option<User>, SignupRepositoryError> {
        self.user_repository
            .find_by_stripe_customer_id(customer_id)
            .map_err(SignupRepositoryError::Database)
    }

    fn find_by_email(&self, email: &str) -> Result<Option<User>, SignupRepositoryError> {
        self.user_core
            .find_by_email(email)
            .map_err(SignupRepositoryError::Database)
    }

    fn find_by_phone_number(&self, phone: &str) -> Result<Option<User>, SignupRepositoryError> {
        self.user_core
            .find_by_phone_number(phone)
            .map_err(SignupRepositoryError::Database)
    }

    fn create_user(&self, new_user: NewUser) -> Result<(), SignupRepositoryError> {
        self.user_core
            .create_user(new_user)
            .map_err(SignupRepositoryError::Database)
    }

    fn set_stripe_customer_id(
        &self,
        user_id: i32,
        customer_id: &str,
    ) -> Result<(), SignupRepositoryError> {
        self.user_repository
            .set_stripe_customer_id(user_id, customer_id)
            .map_err(SignupRepositoryError::Database)
    }

    fn update_phone_number(&self, user_id: i32, phone: &str) -> Result<(), SignupRepositoryError> {
        self.user_core
            .update_phone_number(user_id, phone)
            .map_err(SignupRepositoryError::Database)
    }

    fn update_phone_number_country(
        &self,
        user_id: i32,
        country: Option<&str>,
    ) -> Result<(), SignupRepositoryError> {
        self.user_core
            .update_phone_number_country(user_id, country)
            .map_err(SignupRepositoryError::Database)
    }

    fn set_preferred_number_for_country(
        &self,
        user_id: i32,
        country: &str,
    ) -> Result<(), SignupRepositoryError> {
        self.user_core
            .set_preferred_number_for_country(user_id, country)
            .map_err(|e| SignupRepositoryError::Config(e.to_string()))?;
        Ok(())
    }

    fn set_magic_token(&self, user_id: i32, token: &str) -> Result<(), SignupRepositoryError> {
        self.user_core
            .set_magic_token(user_id, token)
            .map_err(SignupRepositoryError::Database)
    }

    fn ensure_user_settings_exist(&self, user_id: i32) -> Result<(), SignupRepositoryError> {
        self.user_core
            .ensure_user_settings_exist(user_id)
            .map_err(SignupRepositoryError::Database)
    }

    fn ensure_user_info_exists(&self, user_id: i32) -> Result<(), SignupRepositoryError> {
        self.user_core
            .ensure_user_info_exists(user_id)
            .map_err(SignupRepositoryError::Database)
    }
}
