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

/// Parse `phone` and return canonical E.164 (e.g. "+14155551234") if it's a
/// real phone number per libphonenumber. Returns None for invalid input.
///
/// Forgiving on input formatting (libphonenumber strips spaces, dashes,
/// parens, etc. during parse) so e.g. "+1 415-555-1234" round-trips to
/// "+14155551234". A leading `+` is still required since we don't know
/// the country to default to when Stripe sends us national format like
/// "06 86322159" — those get rejected.
fn validate_and_normalize_e164(phone: &str) -> Option<String> {
    if !phone.starts_with('+') {
        return None;
    }
    let parsed = phonenumber::parse(None, phone).ok()?;
    if !phonenumber::is_valid(&parsed) {
        return None;
    }
    Some(parsed.format().mode(phonenumber::Mode::E164).to_string())
}

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
        /// Whether the phone number from Stripe was skipped (already taken, or
        /// not a valid E.164 phone). When true, the user is prompted via the
        /// magic-link email to set a phone in their profile.
        phone_skipped: bool,
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

        // Update phone if provided, parseable as a real phone number, and
        // the user doesn't already have one. Store the canonical E.164 form
        // so downstream lookups (find_by_phone_number) match consistently
        // regardless of how the customer formatted it in Stripe Checkout.
        let phone_updated = if !phone.is_empty() && user.phone_number.is_empty() {
            if let Some(normalized) = validate_and_normalize_e164(phone) {
                self.repository.update_phone_number(user.id, &normalized)?;
                self.setup_phone_country(user.id, &normalized)?;
                true
            } else {
                false
            }
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

        // Skip the Stripe-supplied phone if it's empty, can't be parsed as a
        // real phone number (Stripe Checkout doesn't enforce E.164, so we
        // sometimes get strings like "06 86322159"), or already taken by
        // another account. In all those cases the user lands with an empty
        // phone and the magic-link email tells them to set it via their
        // profile. Otherwise store the canonical E.164 form for consistent
        // downstream lookups.
        let normalized = if phone.is_empty() {
            None
        } else {
            validate_and_normalize_e164(phone)
        };
        let phone_skipped = if phone.is_empty() {
            false
        } else if let Some(ref n) = normalized {
            self.repository.find_by_phone_number(n)?.is_some()
        } else {
            true
        };

        let phone_to_use = if phone_skipped {
            String::new()
        } else {
            normalized.unwrap_or_default()
        };

        // Create user with placeholder password
        let new_user = NewUser {
            email: email.to_string(),
            password_hash: "NOT_SET".to_string(),
            phone_number: phone_to_use.clone(),
            time_to_live: joined_at,
            credits: 0.0,
            credits_left: 0.0,
            charge_when_under: false,
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
        self.repository
            .ensure_user_settings_exist(created_user.id)?;
        self.repository.ensure_user_info_exists(created_user.id)?;

        // Set magic token for password setup
        self.repository
            .set_magic_token(created_user.id, &magic_token)?;

        Ok(SignupResult::NewUserCreated {
            user_id: created_user.id,
            magic_token,
            email: email.to_string(),
            phone_skipped,
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
    fn setup_phone_country(
        &self,
        user_id: i32,
        phone: &str,
    ) -> Result<Option<String>, SignupError> {
        // Detect country from phone number using libphonenumber
        let country = crate::utils::country::get_country_code_from_phone(phone);

        // Set preferred number based on country
        if let Some(ref c) = country {
            // Ignore errors from set_preferred_number - it's not critical
            let _ = self.repository.set_preferred_number_for_country(user_id, c);
        }

        Ok(country)
    }
}
