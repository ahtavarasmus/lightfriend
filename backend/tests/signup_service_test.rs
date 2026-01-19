//! Unit tests for signup service.
//!
//! Tests the business logic for user registration, including handling
//! new users, existing users, phone number detection, and duplicate handling.

use std::sync::Arc;

use backend::handlers::auth_dtos::NewUser;
use backend::repositories::mock_signup_repository::MockSignupRepository;
use backend::repositories::signup_repository::SignupRepository;
use backend::services::signup_service::{SignupError, SignupResult, SignupService};

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
        // Verify preferred number was set for US user
        assert!(repo.get_preferred_number(user_id).is_some());
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
        // Verify preferred number was set for CA user
        assert!(repo.get_preferred_number(user_id).is_some());
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
        // Verify preferred number was set for FI user
        assert!(repo.get_preferred_number(user_id).is_some());
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
    if let SignupResult::NewUserCreated {
        phone_skipped_duplicate,
        ..
    } = result1
    {
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
            // Preferred number should not be set since phone was skipped
            assert_eq!(repo.get_preferred_number(user_id), None);
        }
        _ => panic!("Expected NewUserCreated for second user"),
    }
}
