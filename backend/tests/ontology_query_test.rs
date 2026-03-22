//! Integration tests for ontology query handlers.
//!
//! Tests handle_query() directly with JSON args. Verifies formatted output
//! for person and channel queries.

use backend::test_utils::{create_test_state, create_test_user, TestUserParams};
use backend::tools::ontology::handle_query;
use serial_test::serial;

// =============================================================================
// Person queries
// =============================================================================

#[tokio::test]
#[serial]
async fn test_query_person_by_name() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    state
        .ontology_repository
        .upsert_person(user.id, "Alice", "whatsapp", Some("+1234"), None)
        .unwrap();

    let result = handle_query("query_person", r#"{"name": "Alice"}"#, &state, user.id).await;

    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.contains("Person: Alice"));
    assert!(output.contains("whatsapp"));
}

#[tokio::test]
#[serial]
async fn test_query_person_all() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    state
        .ontology_repository
        .create_person(user.id, "Alice")
        .unwrap();
    state
        .ontology_repository
        .create_person(user.id, "Bob")
        .unwrap();

    let result = handle_query("query_person", r#"{"name": "all"}"#, &state, user.id).await;

    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.contains("Alice"));
    assert!(output.contains("Bob"));
}

#[tokio::test]
#[serial]
async fn test_query_person_not_found() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let result = handle_query("query_person", r#"{"name": "Xyzzyfrob"}"#, &state, user.id).await;

    assert!(result.is_ok());
    assert!(result.unwrap().contains("No people found"));
}

#[tokio::test]
#[serial]
async fn test_query_person_no_params_error() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let result = handle_query("query_person", r#"{}"#, &state, user.id).await;

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("specify a 'name' or 'query'"));
}

// =============================================================================
// Channel queries
// =============================================================================

#[tokio::test]
#[serial]
async fn test_query_channel_by_platform() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let person = state
        .ontology_repository
        .create_person(user.id, "Alice")
        .unwrap();
    state
        .ontology_repository
        .add_channel(user.id, person.id, "whatsapp", Some("+1111"), None)
        .unwrap();
    state
        .ontology_repository
        .add_channel(user.id, person.id, "telegram", Some("@alice"), None)
        .unwrap();

    let result = handle_query(
        "query_channel",
        r#"{"platform": "whatsapp"}"#,
        &state,
        user.id,
    )
    .await;

    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.contains("whatsapp"));
    assert!(!output.contains("telegram"));
}

#[tokio::test]
#[serial]
async fn test_query_channel_combined_filters() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let alice = state
        .ontology_repository
        .create_person(user.id, "Alice")
        .unwrap();
    state
        .ontology_repository
        .add_channel(user.id, alice.id, "whatsapp", Some("+1111"), None)
        .unwrap();
    state
        .ontology_repository
        .add_channel(user.id, alice.id, "telegram", Some("@alice"), None)
        .unwrap();

    let bob = state
        .ontology_repository
        .create_person(user.id, "Bob")
        .unwrap();
    state
        .ontology_repository
        .add_channel(user.id, bob.id, "whatsapp", Some("+2222"), None)
        .unwrap();
    state
        .ontology_repository
        .add_channel(user.id, bob.id, "signal", Some("+3333"), None)
        .unwrap();

    // Filter: Alice + whatsapp only
    let result = handle_query(
        "query_channel",
        r#"{"platform": "whatsapp", "person_name": "Alice"}"#,
        &state,
        user.id,
    )
    .await;

    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.contains("whatsapp"));
    assert!(output.contains("Alice"));
    // Should NOT contain Bob's whatsapp or Alice's telegram
    assert!(!output.contains("Bob"));
    assert!(!output.contains("telegram"));
}

#[tokio::test]
#[serial]
async fn test_query_channel_no_params_error() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let result = handle_query("query_channel", r#"{}"#, &state, user.id).await;

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("at least one filter"));
}

// =============================================================================
// =============================================================================
// Error cases
// =============================================================================

#[tokio::test]
#[serial]
async fn test_query_invalid_entity() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let result = handle_query("query_foobar", r#"{}"#, &state, user.id).await;

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Unknown ontology entity type"));
}

#[tokio::test]
#[serial]
async fn test_query_invalid_json() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let result = handle_query("query_person", "{invalid json", &state, user.id).await;

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Invalid arguments"));
}

#[tokio::test]
#[serial]
async fn test_query_invalid_tool_prefix() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));

    let result = handle_query("not_a_query_tool", r#"{}"#, &state, user.id).await;

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Invalid ontology tool name"));
}
