//! Signup service containing business logic for user registration.
//!
//! Separates business logic from webhook handling, enabling unit testing
//! with mock repositories.

use std::sync::Arc;

use rand::Rng;
use thiserror::Error;

use crate::handlers::auth_dtos::NewUser;
use crate::models::user_models::User;
use crate::repositories::signup_repository::{SignupRepository, SignupRepositoryError};
use crate::services::country_service;

/// Errors that can occur during signup
#[derive(Debug, Error)]
pub enum SignupError {
    #[error("Email is required")]
    EmptyEmail,

    #[error("Repository error: {0}")]
    Repository(#[from] SignupRepositoryError),

    #[error("User not found after creation")]
    UserNotFoundAfterCreation,
}

/// Result of processing a new subscription from Stripe webhook.
#[derive(Debug)]
pub enum SignupResult {
    /// An existing user was linked to the Stripe customer.
    ExistingUserLinked {
        user_id: i32,
        /// Whether to send a "subscription activated" email.
        send_welcome_email: bool,
        /// Whether the phone number was updated.
        phone_updated: bool,
    },

    /// A new user was created.
    NewUserCreated {
        user_id: i32,
        /// Magic token for password setup link.
        magic_token: String,
        /// User's email for sending the magic link.
        email: String,
        /// Whether the phone number was skipped because it's already in use by another account.
        /// If true, the user should be prompted to enter a different phone number.
        phone_skipped_duplicate: bool,
    },
}

/// Service for handling new subscription signups.
///
/// Extracts the business logic from the Stripe webhook handler,
/// making it testable with mock repositories.
pub struct SignupService<R: SignupRepository> {
    repository: Arc<R>,
}

impl<R: SignupRepository> SignupService<R> {
    /// Create a new signup service with the given repository.
    pub fn new(repository: Arc<R>) -> Self {
        Self { repository }
    }

    /// Handle a new subscription for a Stripe customer.
    ///
    /// Called when `CustomerSubscriptionCreated` event is received
    /// and no existing user is found by `stripe_customer_id`.
    ///
    /// This handles two cases:
    /// 1. User with matching email exists - link them to Stripe customer
    /// 2. No matching user - create new account with magic token
    ///
    /// # Arguments
    /// * `email` - Customer email from Stripe
    /// * `phone` - Customer phone from Stripe (may be empty)
    /// * `stripe_customer_id` - The Stripe customer ID to link
    ///
    /// # Returns
    /// * `SignupResult::ExistingUserLinked` if user already existed
    /// * `SignupResult::NewUserCreated` if new user was created
    pub fn handle_new_subscription(
        &self,
        email: &str,
        phone: &str,
        stripe_customer_id: &str,
    ) -> Result<SignupResult, SignupError> {
        if email.is_empty() {
            return Err(SignupError::EmptyEmail);
        }

        // Check if user with this email already exists
        match self.repository.find_by_email(email)? {
            Some(existing_user) => {
                self.link_existing_user(&existing_user, phone, stripe_customer_id)
            }
            None => self.create_new_user(email, phone, stripe_customer_id),
        }
    }

    /// Link an existing user to a Stripe customer.
    fn link_existing_user(
        &self,
        user: &User,
        phone: &str,
        stripe_customer_id: &str,
    ) -> Result<SignupResult, SignupError> {
        // Link to Stripe customer
        self.repository
            .set_stripe_customer_id(user.id, stripe_customer_id)?;

        // Update phone if provided and user doesn't have one
        let phone_updated = if !phone.is_empty() && user.phone_number.is_empty() {
            self.repository.update_phone_number(user.id, phone)?;
            self.setup_phone_country(user.id, phone)?;
            true
        } else {
            false
        };

        Ok(SignupResult::ExistingUserLinked {
            user_id: user.id,
            send_welcome_email: true,
            phone_updated,
        })
    }

    /// Create a new user from Stripe customer data.
    fn create_new_user(
        &self,
        email: &str,
        phone: &str,
        stripe_customer_id: &str,
    ) -> Result<SignupResult, SignupError> {
        // Generate magic token for password setup
        let magic_token = self.generate_magic_token();

        // Calculate joined_at timestamp
        let joined_at = chrono::Utc::now().timestamp() as i32;

        // Check if phone number is already in use by another account
        let phone_is_duplicate = if !phone.is_empty() {
            self.repository.find_by_phone_number(phone)?.is_some()
        } else {
            false
        };

        // If phone is duplicate, create user without phone number
        // User will need to set their phone number later
        let phone_to_use = if phone_is_duplicate {
            String::new()
        } else {
            phone.to_string()
        };

        // Create user with placeholder password
        let new_user = NewUser {
            email: email.to_string(),
            password_hash: "NOT_SET".to_string(),
            phone_number: phone_to_use.clone(),
            time_to_live: joined_at,
            verified: true, // No phone verification needed
            credits: 0.0,
            credits_left: 0.0,
            charge_when_under: false,
            waiting_checks_count: 0,
            discount: false,
            sub_tier: None,
        };

        self.repository.create_user(new_user)?;

        // Retrieve created user
        let created_user = self
            .repository
            .find_by_email(email)?
            .ok_or(SignupError::UserNotFoundAfterCreation)?;

        // Link to Stripe customer
        self.repository
            .set_stripe_customer_id(created_user.id, stripe_customer_id)?;

        // Set phone country and preferred number (only if we used the phone)
        if !phone_to_use.is_empty() {
            self.setup_phone_country(created_user.id, &phone_to_use)?;
        }

        // Ensure settings and info records exist
        self.repository.ensure_user_settings_exist(created_user.id)?;
        self.repository.ensure_user_info_exists(created_user.id)?;

        // Set magic token for password setup
        self.repository
            .set_magic_token(created_user.id, &magic_token)?;

        Ok(SignupResult::NewUserCreated {
            user_id: created_user.id,
            magic_token,
            email: email.to_string(),
            phone_skipped_duplicate: phone_is_duplicate,
        })
    }

    /// Generate a 64-character alphanumeric magic token.
    fn generate_magic_token(&self) -> String {
        rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(64)
            .map(char::from)
            .collect()
    }

    /// Detect country from phone number and set preferred number.
    fn setup_phone_country(&self, user_id: i32, phone: &str) -> Result<Option<String>, SignupError> {
        let country = country_service::detect_country(phone);

        // Update phone country in database
        self.repository
            .update_phone_number_country(user_id, country.as_deref())?;

        // Set preferred number based on country
        if let Some(ref c) = country {
            // Ignore errors from set_preferred_number - it's not critical
            let _ = self.repository.set_preferred_number_for_country(user_id, c);
        }

        Ok(country)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repositories::mock_signup_repository::MockSignupRepository;

    fn create_test_service() -> SignupService<MockSignupRepository> {
        let repo = Arc::new(MockSignupRepository::new());
        SignupService::new(repo)
    }

    #[test]
    fn test_new_user_created_with_magic_token() {
        let repo = Arc::new(MockSignupRepository::new());
        let service = SignupService::new(repo.clone());

        let result = service
            .handle_new_subscription("test@example.com", "+14155551234", "cus_123")
            .unwrap();

        match result {
            SignupResult::NewUserCreated {
                user_id,
                magic_token,
                email,
                phone_skipped_duplicate,
            } => {
                assert_eq!(email, "test@example.com");
                assert_eq!(magic_token.len(), 64);
                assert!(!phone_skipped_duplicate);
                assert!(repo.has_user_with_email("test@example.com"));
                assert!(repo.get_magic_token(user_id).is_some());
            }
            _ => panic!("Expected NewUserCreated"),
        }
    }

    #[test]
    fn test_new_user_phone_country_detected() {
        let repo = Arc::new(MockSignupRepository::new());
        let service = SignupService::new(repo.clone());

        let result = service
            .handle_new_subscription("test@example.com", "+14155551234", "cus_123")
            .unwrap();

        if let SignupResult::NewUserCreated { user_id, .. } = result {
            assert_eq!(repo.get_phone_country(user_id), Some("US".to_string()));
        }
    }

    #[test]
    fn test_new_user_settings_initialized() {
        let repo = Arc::new(MockSignupRepository::new());
        let service = SignupService::new(repo.clone());

        let result = service
            .handle_new_subscription("test@example.com", "+14155551234", "cus_123")
            .unwrap();

        if let SignupResult::NewUserCreated { user_id, .. } = result {
            assert!(repo.has_user_settings(user_id));
            assert!(repo.has_user_info(user_id));
        }
    }

    #[test]
    fn test_existing_user_linked_to_stripe() {
        let repo = Arc::new(MockSignupRepository::new());
        let service = SignupService::new(repo.clone());

        // First create a user
        service
            .handle_new_subscription("test@example.com", "+14155551234", "cus_old")
            .unwrap();

        // Now simulate another subscription with same email but different Stripe ID
        // First, we need to find the user to simulate the "existing user" case
        let user = repo.find_by_email("test@example.com").unwrap().unwrap();

        // Create a new service call that will find the existing user
        let result = service
            .handle_new_subscription("test@example.com", "+14165551234", "cus_new")
            .unwrap();

        match result {
            SignupResult::ExistingUserLinked {
                user_id,
                send_welcome_email,
                ..
            } => {
                assert_eq!(user_id, user.id);
                assert!(send_welcome_email);
            }
            _ => panic!("Expected ExistingUserLinked"),
        }
    }

    #[test]
    fn test_existing_user_phone_updated_if_empty() {
        let repo = Arc::new(MockSignupRepository::new());
        let service = SignupService::new(repo.clone());

        // Create user with no phone
        let new_user = NewUser {
            email: "test@example.com".to_string(),
            password_hash: "hash".to_string(),
            phone_number: "".to_string(), // Empty phone
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

        // Now link with a phone number
        let result = service
            .handle_new_subscription("test@example.com", "+14155551234", "cus_123")
            .unwrap();

        match result {
            SignupResult::ExistingUserLinked { phone_updated, .. } => {
                assert!(phone_updated);
            }
            _ => panic!("Expected ExistingUserLinked"),
        }
    }

    #[test]
    fn test_existing_user_phone_not_overwritten() {
        let repo = Arc::new(MockSignupRepository::new());
        let service = SignupService::new(repo.clone());

        // Create user with existing phone
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

        // Try to link with different phone number
        let result = service
            .handle_new_subscription("test@example.com", "+14165559999", "cus_123")
            .unwrap();

        match result {
            SignupResult::ExistingUserLinked { phone_updated, .. } => {
                assert!(!phone_updated);
            }
            _ => panic!("Expected ExistingUserLinked"),
        }
    }

    #[test]
    fn test_empty_email_rejected() {
        let service = create_test_service();

        let result = service.handle_new_subscription("", "+14155551234", "cus_123");

        assert!(matches!(result, Err(SignupError::EmptyEmail)));
    }

    #[test]
    fn test_database_error_propagated() {
        let repo = Arc::new(MockSignupRepository::new());
        repo.set_fail_on_find_email(true);
        let service = SignupService::new(repo);

        let result = service.handle_new_subscription("test@example.com", "+14155551234", "cus_123");

        assert!(matches!(result, Err(SignupError::Repository(_))));
    }

    #[test]
    fn test_canadian_phone_detected() {
        let repo = Arc::new(MockSignupRepository::new());
        let service = SignupService::new(repo.clone());

        let result = service
            .handle_new_subscription("canadian@example.com", "+14165551234", "cus_123")
            .unwrap();

        if let SignupResult::NewUserCreated { user_id, .. } = result {
            assert_eq!(repo.get_phone_country(user_id), Some("CA".to_string()));
        }
    }

    #[test]
    fn test_european_phone_detected() {
        let repo = Arc::new(MockSignupRepository::new());
        let service = SignupService::new(repo.clone());

        let result = service
            .handle_new_subscription("finnish@example.com", "+358401234567", "cus_123")
            .unwrap();

        if let SignupResult::NewUserCreated { user_id, .. } = result {
            assert_eq!(repo.get_phone_country(user_id), Some("FI".to_string()));
        }
    }

    #[test]
    fn test_duplicate_phone_skipped() {
        let repo = Arc::new(MockSignupRepository::new());
        let service = SignupService::new(repo.clone());

        // First user with phone number
        let result1 = service
            .handle_new_subscription("user1@example.com", "+14155551234", "cus_123")
            .unwrap();

        // Verify first user was created with phone
        if let SignupResult::NewUserCreated { phone_skipped_duplicate, .. } = result1 {
            assert!(!phone_skipped_duplicate);
        } else {
            panic!("Expected NewUserCreated for first user");
        }

        // Second user with same phone number but different email
        let result2 = service
            .handle_new_subscription("user2@example.com", "+14155551234", "cus_456")
            .unwrap();

        // Verify second user was created but phone was skipped
        match result2 {
            SignupResult::NewUserCreated {
                phone_skipped_duplicate,
                user_id,
                ..
            } => {
                assert!(phone_skipped_duplicate);
                // Phone country should not be set since phone was skipped
                assert_eq!(repo.get_phone_country(user_id), None);
            }
            _ => panic!("Expected NewUserCreated for second user"),
        }
    }
}
