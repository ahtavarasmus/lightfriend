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
// Event queries
// =============================================================================

#[tokio::test]
#[serial]
async fn test_query_event_filters_by_completed_status() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let now = chrono::Utc::now().timestamp() as i32;

    let event = state
        .ontology_repository
        .create_event(&backend::models::ontology_models::NewOntEvent {
            user_id: user.id,
            description: "Invoice follow-up".to_string(),
            remind_at: Some(now - 60),
            due_at: Some(now + 3600),
            status: "completed".to_string(),
            created_at: now,
            updated_at: now,
        })
        .unwrap();

    let result = handle_query(
        "query_event",
        r#"{"status":"completed","query":"invoice"}"#,
        &state,
        user.id,
    )
    .await;

    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.contains(&format!("[event_id={}]", event.id)));
    assert!(output.contains("[status=completed]"));
}

#[tokio::test]
#[serial]
async fn test_query_event_status_all_includes_non_active_events() {
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let now = chrono::Utc::now().timestamp() as i32;

    state
        .ontology_repository
        .create_event(&backend::models::ontology_models::NewOntEvent {
            user_id: user.id,
            description: "Active package".to_string(),
            remind_at: Some(now + 60),
            due_at: Some(now + 3600),
            status: "active".to_string(),
            created_at: now,
            updated_at: now,
        })
        .unwrap();
    state
        .ontology_repository
        .create_event(&backend::models::ontology_models::NewOntEvent {
            user_id: user.id,
            description: "Dismissed package".to_string(),
            remind_at: Some(now - 60),
            due_at: Some(now + 3600),
            status: "dismissed".to_string(),
            created_at: now,
            updated_at: now,
        })
        .unwrap();

    let result = handle_query("query_event", r#"{"status":"all"}"#, &state, user.id).await;

    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.contains("Active package"));
    assert!(output.contains("Dismissed package"));
}

// =============================================================================
// Message queries
// =============================================================================

#[tokio::test]
#[serial]
async fn test_query_message_generic_dedups_per_room_and_excludes_outgoing() {
    // "What's in my messages lately?" — no sender, no query.
    // Expected: one message per distinct (platform, room_id),
    // outgoing ("You") excluded, chatty sender does NOT eat the budget.
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let now = chrono::Utc::now().timestamp() as i32;

    // Chatty friend: 5 messages in room A
    for i in 0..5 {
        state
            .ontology_repository
            .insert_message(&backend::models::ontology_models::NewOntMessage {
                user_id: user.id,
                room_id: "!roomA:hs".to_string(),
                platform: "whatsapp".to_string(),
                sender_name: "Chatty".to_string(),
                sender_key: None,
                content: format!("chatty msg {}", i),
                person_id: None,
                created_at: now - (60 * (10 - i)), // older → newer
                matrix_event_id: None,
            })
            .unwrap();
    }

    // Quiet friend: 1 message in room B
    state
        .ontology_repository
        .insert_message(&backend::models::ontology_models::NewOntMessage {
            user_id: user.id,
            room_id: "!roomB:hs".to_string(),
            platform: "whatsapp".to_string(),
            sender_name: "Quiet".to_string(),
            sender_key: None,
            content: "quiet hello".to_string(),
            person_id: None,
            created_at: now - 30,
            matrix_event_id: None,
        })
        .unwrap();

    // Outgoing message the user sent — must NOT appear in generic digest
    state
        .ontology_repository
        .insert_message(&backend::models::ontology_models::NewOntMessage {
            user_id: user.id,
            room_id: "!roomC:hs".to_string(),
            platform: "whatsapp".to_string(),
            sender_name: "You".to_string(),
            sender_key: None,
            content: "my own outgoing".to_string(),
            person_id: None,
            created_at: now - 10,
            matrix_event_id: None,
        })
        .unwrap();

    let result = handle_query("query_message", r#"{}"#, &state, user.id).await;
    assert!(result.is_ok(), "query failed: {:?}", result);
    let output = result.unwrap();

    // Both conversations visible
    assert!(
        output.contains("Quiet"),
        "quiet sender should survive chatty-bias, got: {}",
        output
    );
    assert!(
        output.contains("Chatty"),
        "chatty sender should still appear, got: {}",
        output
    );

    // Only ONE chatty message (the latest), not all five
    let chatty_count = output.matches("Chatty").count();
    assert_eq!(
        chatty_count, 1,
        "dedup should collapse chatty to 1 row, got {} (output: {})",
        chatty_count, output
    );

    // Outgoing excluded
    assert!(
        !output.contains("my own outgoing"),
        "outgoing should be excluded from generic digest, got: {}",
        output
    );
}

#[tokio::test]
#[serial]
async fn test_query_message_sender_filter_returns_all_matching() {
    // "What did Chatty say" — sender filter set → time-desc mode,
    // no dedup. Should return ALL of chatty's recent messages.
    let state = create_test_state();
    let user = create_test_user(&state, &TestUserParams::us_user(10.0, 5.0));
    let now = chrono::Utc::now().timestamp() as i32;

    for i in 0..3 {
        state
            .ontology_repository
            .insert_message(&backend::models::ontology_models::NewOntMessage {
                user_id: user.id,
                room_id: "!roomA:hs".to_string(),
                platform: "whatsapp".to_string(),
                sender_name: "Chatty".to_string(),
                sender_key: None,
                content: format!("chatty msg {}", i),
                person_id: None,
                created_at: now - (60 * (10 - i)),
                matrix_event_id: None,
            })
            .unwrap();
    }

    let result = handle_query(
        "query_message",
        r#"{"sender_name":"Chatty"}"#,
        &state,
        user.id,
    )
    .await;
    assert!(result.is_ok());
    let output = result.unwrap();

    // All three chatty messages appear
    assert!(output.contains("chatty msg 0"));
    assert!(output.contains("chatty msg 1"));
    assert!(output.contains("chatty msg 2"));
}

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
