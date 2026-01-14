//! Mock implementation of SignupRepository for unit testing.
//!
//! Provides an in-memory implementation that can be used in tests
//! without requiring a database connection.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::handlers::auth_dtos::NewUser;
use crate::models::user_models::User;
use crate::repositories::signup_repository::{SignupRepository, SignupRepositoryError};

/// Mock user data stored in memory
#[derive(Clone, Debug)]
struct MockUser {
    id: i32,
    email: String,
    password_hash: String,
    phone_number: String,
    phone_number_country: Option<String>,
    preferred_number: Option<String>,
    stripe_customer_id: Option<String>,
    magic_token: Option<String>,
    settings_exist: bool,
    info_exists: bool,
}

impl MockUser {
    fn to_user(&self) -> User {
        User {
            id: self.id,
            email: self.email.clone(),
            password_hash: self.password_hash.clone(),
            phone_number: self.phone_number.clone(),
            nickname: None,
            time_to_live: Some(0),
            verified: true,
            credits: 0.0,
            preferred_number: self.preferred_number.clone(),
            charge_when_under: false,
            charge_back_to: None,
            stripe_customer_id: self.stripe_customer_id.clone(),
            stripe_payment_method_id: None,
            stripe_checkout_session_id: None,
            matrix_username: None,
            encrypted_matrix_access_token: None,
            sub_tier: None,
            matrix_device_id: None,
            credits_left: 0.0,
            encrypted_matrix_password: None,
            encrypted_matrix_secret_storage_recovery_key: None,
            last_credits_notification: None,
            discount: false,
            discount_tier: None,
            free_reply: false,
            confirm_send_event: None,
            waiting_checks_count: 0,
            next_billing_date_timestamp: None,
            phone_number_country: self.phone_number_country.clone(),
            magic_token: self.magic_token.clone(),
            plan_type: None,
        }
    }
}

/// Mock repository for testing SignupService without a database.
pub struct MockSignupRepository {
    users: Mutex<HashMap<i32, MockUser>>,
    email_to_id: Mutex<HashMap<String, i32>>,
    stripe_to_id: Mutex<HashMap<String, i32>>,
    next_id: Mutex<i32>,
    // Configurable failure modes
    fail_on_create: Mutex<bool>,
    fail_on_find_email: Mutex<bool>,
    fail_on_find_stripe: Mutex<bool>,
}

impl Default for MockSignupRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl MockSignupRepository {
    /// Create a new empty mock repository.
    pub fn new() -> Self {
        Self {
            users: Mutex::new(HashMap::new()),
            email_to_id: Mutex::new(HashMap::new()),
            stripe_to_id: Mutex::new(HashMap::new()),
            next_id: Mutex::new(1),
            fail_on_create: Mutex::new(false),
            fail_on_find_email: Mutex::new(false),
            fail_on_find_stripe: Mutex::new(false),
        }
    }

    /// Configure to fail on next create_user call.
    pub fn set_fail_on_create(&self, fail: bool) {
        *self.fail_on_create.lock().unwrap() = fail;
    }

    /// Configure to fail on find_by_email calls.
    pub fn set_fail_on_find_email(&self, fail: bool) {
        *self.fail_on_find_email.lock().unwrap() = fail;
    }

    /// Configure to fail on find_by_stripe_customer_id calls.
    pub fn set_fail_on_find_stripe(&self, fail: bool) {
        *self.fail_on_find_stripe.lock().unwrap() = fail;
    }

    /// Get count of registered users.
    pub fn user_count(&self) -> usize {
        self.users.lock().unwrap().len()
    }

    /// Check if a user with given email exists.
    pub fn has_user_with_email(&self, email: &str) -> bool {
        self.email_to_id
            .lock()
            .unwrap()
            .contains_key(&email.to_lowercase())
    }

    /// Get the magic token for a user.
    pub fn get_magic_token(&self, user_id: i32) -> Option<String> {
        self.users
            .lock()
            .unwrap()
            .get(&user_id)
            .and_then(|u| u.magic_token.clone())
    }

    /// Check if user settings were initialized.
    pub fn has_user_settings(&self, user_id: i32) -> bool {
        self.users
            .lock()
            .unwrap()
            .get(&user_id)
            .map(|u| u.settings_exist)
            .unwrap_or(false)
    }

    /// Check if user info was initialized.
    pub fn has_user_info(&self, user_id: i32) -> bool {
        self.users
            .lock()
            .unwrap()
            .get(&user_id)
            .map(|u| u.info_exists)
            .unwrap_or(false)
    }

    /// Get user's phone country.
    pub fn get_phone_country(&self, user_id: i32) -> Option<String> {
        self.users
            .lock()
            .unwrap()
            .get(&user_id)
            .and_then(|u| u.phone_number_country.clone())
    }

    /// Get user's preferred number.
    pub fn get_preferred_number(&self, user_id: i32) -> Option<String> {
        self.users
            .lock()
            .unwrap()
            .get(&user_id)
            .and_then(|u| u.preferred_number.clone())
    }

    fn simulate_db_error() -> SignupRepositoryError {
        SignupRepositoryError::Database(diesel::result::Error::DatabaseError(
            diesel::result::DatabaseErrorKind::Unknown,
            Box::new("Simulated failure".to_string()),
        ))
    }
}

impl SignupRepository for MockSignupRepository {
    fn find_by_stripe_customer_id(
        &self,
        customer_id: &str,
    ) -> Result<Option<User>, SignupRepositoryError> {
        if *self.fail_on_find_stripe.lock().unwrap() {
            return Err(Self::simulate_db_error());
        }

        let stripe_to_id = self.stripe_to_id.lock().unwrap();
        let users = self.users.lock().unwrap();

        Ok(stripe_to_id
            .get(customer_id)
            .and_then(|id| users.get(id))
            .map(|u| u.to_user()))
    }

    fn find_by_email(&self, email: &str) -> Result<Option<User>, SignupRepositoryError> {
        if *self.fail_on_find_email.lock().unwrap() {
            return Err(Self::simulate_db_error());
        }

        let email_to_id = self.email_to_id.lock().unwrap();
        let users = self.users.lock().unwrap();

        Ok(email_to_id
            .get(&email.to_lowercase())
            .and_then(|id| users.get(id))
            .map(|u| u.to_user()))
    }

    fn create_user(&self, new_user: NewUser) -> Result<(), SignupRepositoryError> {
        if *self.fail_on_create.lock().unwrap() {
            return Err(Self::simulate_db_error());
        }

        let mut users = self.users.lock().unwrap();
        let mut email_to_id = self.email_to_id.lock().unwrap();
        let mut next_id = self.next_id.lock().unwrap();

        let id = *next_id;
        *next_id += 1;

        let mock_user = MockUser {
            id,
            email: new_user.email.clone(),
            password_hash: new_user.password_hash,
            phone_number: new_user.phone_number,
            phone_number_country: None,
            preferred_number: None,
            stripe_customer_id: None,
            magic_token: None,
            settings_exist: false,
            info_exists: false,
        };

        email_to_id.insert(new_user.email.to_lowercase(), id);
        users.insert(id, mock_user);

        Ok(())
    }

    fn set_stripe_customer_id(
        &self,
        user_id: i32,
        customer_id: &str,
    ) -> Result<(), SignupRepositoryError> {
        let mut users = self.users.lock().unwrap();
        let mut stripe_to_id = self.stripe_to_id.lock().unwrap();

        if let Some(user) = users.get_mut(&user_id) {
            user.stripe_customer_id = Some(customer_id.to_string());
            stripe_to_id.insert(customer_id.to_string(), user_id);
        }

        Ok(())
    }

    fn update_phone_number(&self, user_id: i32, phone: &str) -> Result<(), SignupRepositoryError> {
        let mut users = self.users.lock().unwrap();

        if let Some(user) = users.get_mut(&user_id) {
            user.phone_number = phone.to_string();
        }

        Ok(())
    }

    fn update_phone_number_country(
        &self,
        user_id: i32,
        country: Option<&str>,
    ) -> Result<(), SignupRepositoryError> {
        let mut users = self.users.lock().unwrap();

        if let Some(user) = users.get_mut(&user_id) {
            user.phone_number_country = country.map(String::from);
        }

        Ok(())
    }

    fn set_preferred_number_for_country(
        &self,
        user_id: i32,
        country: &str,
    ) -> Result<(), SignupRepositoryError> {
        let mut users = self.users.lock().unwrap();

        if let Some(user) = users.get_mut(&user_id) {
            // Simulate setting preferred number based on country
            user.preferred_number = match country {
                "US" => Some("+18001234567".to_string()),
                "CA" => Some("+18001234567".to_string()),
                "FI" => Some("+358401234567".to_string()),
                "GB" => Some("+447911123456".to_string()),
                "NL" => Some("+31612345678".to_string()),
                "AU" => Some("+61412345678".to_string()),
                _ => None,
            };
        }

        Ok(())
    }

    fn set_magic_token(&self, user_id: i32, token: &str) -> Result<(), SignupRepositoryError> {
        let mut users = self.users.lock().unwrap();

        if let Some(user) = users.get_mut(&user_id) {
            user.magic_token = Some(token.to_string());
        }

        Ok(())
    }

    fn ensure_user_settings_exist(&self, user_id: i32) -> Result<(), SignupRepositoryError> {
        let mut users = self.users.lock().unwrap();

        if let Some(user) = users.get_mut(&user_id) {
            user.settings_exist = true;
        }

        Ok(())
    }

    fn ensure_user_info_exists(&self, user_id: i32) -> Result<(), SignupRepositoryError> {
        let mut users = self.users.lock().unwrap();

        if let Some(user) = users.get_mut(&user_id) {
            user.info_exists = true;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_find_user() {
        let repo = MockSignupRepository::new();

        let new_user = NewUser {
            email: "test@example.com".to_string(),
            password_hash: "hash".to_string(),
            phone_number: "+14155551234".to_string(),
            time_to_live: 0,
            verified: true,
            credits: 0.0,
            credits_left: 0.0,
            charge_when_under: false,
            waiting_checks_count: 0,
            discount: false,
            sub_tier: None,
        };

        repo.create_user(new_user).unwrap();
        assert_eq!(repo.user_count(), 1);

        let found = repo.find_by_email("test@example.com").unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().email, "test@example.com");
    }

    #[test]
    fn test_case_insensitive_email() {
        let repo = MockSignupRepository::new();

        let new_user = NewUser {
            email: "Test@Example.COM".to_string(),
            password_hash: "hash".to_string(),
            phone_number: "+14155551234".to_string(),
            time_to_live: 0,
            verified: true,
            credits: 0.0,
            credits_left: 0.0,
            charge_when_under: false,
            waiting_checks_count: 0,
            discount: false,
            sub_tier: None,
        };

        repo.create_user(new_user).unwrap();

        // Should find with different case
        let found = repo.find_by_email("test@example.com").unwrap();
        assert!(found.is_some());
    }

    #[test]
    fn test_stripe_customer_link() {
        let repo = MockSignupRepository::new();

        let new_user = NewUser {
            email: "test@example.com".to_string(),
            password_hash: "hash".to_string(),
            phone_number: "+14155551234".to_string(),
            time_to_live: 0,
            verified: true,
            credits: 0.0,
            credits_left: 0.0,
            charge_when_under: false,
            waiting_checks_count: 0,
            discount: false,
            sub_tier: None,
        };

        repo.create_user(new_user).unwrap();

        let user = repo.find_by_email("test@example.com").unwrap().unwrap();
        repo.set_stripe_customer_id(user.id, "cus_123").unwrap();

        let found_by_stripe = repo.find_by_stripe_customer_id("cus_123").unwrap();
        assert!(found_by_stripe.is_some());
        assert_eq!(found_by_stripe.unwrap().id, user.id);
    }

    #[test]
    fn test_failure_modes() {
        let repo = MockSignupRepository::new();

        repo.set_fail_on_create(true);
        let new_user = NewUser {
            email: "test@example.com".to_string(),
            password_hash: "hash".to_string(),
            phone_number: "+14155551234".to_string(),
            time_to_live: 0,
            verified: true,
            credits: 0.0,
            credits_left: 0.0,
            charge_when_under: false,
            waiting_checks_count: 0,
            discount: false,
            sub_tier: None,
        };

        assert!(repo.create_user(new_user).is_err());
    }
}
