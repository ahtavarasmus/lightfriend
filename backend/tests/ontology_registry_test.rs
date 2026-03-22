//! Unit tests for OntologyRegistry.
//!
//! Pure logic tests - no DB needed. Tests build_query_tools() tool generation,
//! static/dynamic enums, and linked_entities parameters.

use backend::ontology::registry::{OntologyRegistry, OntologyUserData};
use std::collections::HashMap;

fn empty_user_data() -> OntologyUserData {
    OntologyUserData {
        dynamic_enums: HashMap::new(),
    }
}

fn user_data_with_names(names: Vec<&str>) -> OntologyUserData {
    let mut dynamic_enums = HashMap::new();
    dynamic_enums.insert(
        "person_names".to_string(),
        names.into_iter().map(|n| n.to_string()).collect(),
    );
    OntologyUserData { dynamic_enums }
}

/// Helper: extract enum values from a tool's property.
fn get_enum_values(
    tools: &[openai_api_rs::v1::chat_completion::Tool],
    tool_name: &str,
    prop_name: &str,
) -> Option<Vec<String>> {
    let tool = tools.iter().find(|t| t.function.name == tool_name)?;
    let props = tool.function.parameters.properties.as_ref()?;
    let prop = props.get(prop_name)?;
    prop.enum_values.clone()
}

/// Helper: check if a tool has a specific property.
fn has_property(
    tools: &[openai_api_rs::v1::chat_completion::Tool],
    tool_name: &str,
    prop_name: &str,
) -> bool {
    tools
        .iter()
        .find(|t| t.function.name == tool_name)
        .and_then(|t| t.function.parameters.properties.as_ref())
        .map(|props| props.contains_key(prop_name))
        .unwrap_or(false)
}

// =============================================================================
// Tool generation
// =============================================================================

#[test]
fn test_registry_generates_three_tools() {
    let registry = OntologyRegistry::build();
    let tools = registry.build_query_tools(&empty_user_data());

    assert_eq!(tools.len(), 3);

    let names: Vec<&str> = tools.iter().map(|t| t.function.name.as_str()).collect();
    assert!(names.contains(&"query_person"));
    assert!(names.contains(&"query_channel"));
    assert!(names.contains(&"query_message"));
}

// =============================================================================
// Static enums
// =============================================================================

#[test]
fn test_registry_static_enums_include_all() {
    let registry = OntologyRegistry::build();
    let tools = registry.build_query_tools(&empty_user_data());

    // Platform enum on channel: "all" + static values
    let platform_vals =
        get_enum_values(&tools, "query_channel", "platform").expect("platform enum should exist");
    assert!(platform_vals.contains(&"all".to_string()));
    assert!(platform_vals.contains(&"whatsapp".to_string()));
    assert!(platform_vals.contains(&"telegram".to_string()));
    assert!(platform_vals.contains(&"signal".to_string()));
    assert!(platform_vals.contains(&"email".to_string()));

    // Platform enum on message
    let msg_platform_vals = get_enum_values(&tools, "query_message", "platform")
        .expect("message platform enum should exist");
    assert!(msg_platform_vals.contains(&"all".to_string()));
    assert!(msg_platform_vals.contains(&"whatsapp".to_string()));

    // Notification mode enum
    let notif_vals = get_enum_values(&tools, "query_channel", "notification_mode")
        .expect("notification_mode enum should exist");
    assert!(notif_vals.contains(&"all".to_string()));
    assert!(notif_vals.contains(&"alert".to_string()));
    assert!(notif_vals.contains(&"silent".to_string()));
    assert!(notif_vals.contains(&"off".to_string()));
}

// =============================================================================
// Dynamic enums
// =============================================================================

#[test]
fn test_registry_dynamic_enums_injected() {
    let registry = OntologyRegistry::build();
    let user_data = user_data_with_names(vec!["Alice", "Bob"]);
    let tools = registry.build_query_tools(&user_data);

    let name_vals =
        get_enum_values(&tools, "query_person", "name").expect("name enum should exist");
    assert!(name_vals.contains(&"all".to_string()));
    assert!(name_vals.contains(&"Alice".to_string()));
    assert!(name_vals.contains(&"Bob".to_string()));

    // Also injected into channel's person_name
    let person_name_vals = get_enum_values(&tools, "query_channel", "person_name")
        .expect("person_name enum should exist");
    assert!(person_name_vals.contains(&"all".to_string()));
    assert!(person_name_vals.contains(&"Alice".to_string()));
    assert!(person_name_vals.contains(&"Bob".to_string()));

    // Also injected into message's sender_name
    let sender_name_vals = get_enum_values(&tools, "query_message", "sender_name")
        .expect("sender_name enum should exist");
    assert!(sender_name_vals.contains(&"all".to_string()));
    assert!(sender_name_vals.contains(&"Alice".to_string()));
    assert!(sender_name_vals.contains(&"Bob".to_string()));
}

#[test]
fn test_registry_dynamic_enums_fallback() {
    let registry = OntologyRegistry::build();
    let tools = registry.build_query_tools(&empty_user_data());

    // With empty dynamic_enums, name should only have "all"
    let name_vals =
        get_enum_values(&tools, "query_person", "name").expect("name enum should exist");
    assert_eq!(name_vals, vec!["all".to_string()]);
}

// =============================================================================
// Linked entities parameter
// =============================================================================

#[test]
fn test_registry_linked_entities_param() {
    let registry = OntologyRegistry::build();
    let tools = registry.build_query_tools(&empty_user_data());

    // Person linkable_to: Channel
    assert!(has_property(&tools, "query_person", "linked_entities"));
    let person_linkable = get_linked_entities_enum(&tools, "query_person");
    assert!(person_linkable.contains(&"Channel".to_string()));

    // Channel linkable_to: Person
    assert!(has_property(&tools, "query_channel", "linked_entities"));
    let channel_linkable = get_linked_entities_enum(&tools, "query_channel");
    assert!(channel_linkable.contains(&"Person".to_string()));

    // Message has no linked_entities (linkable_to is empty)
    assert!(!has_property(&tools, "query_message", "linked_entities"));
}

/// Helper: extract linked_entities array item enum values.
fn get_linked_entities_enum(
    tools: &[openai_api_rs::v1::chat_completion::Tool],
    tool_name: &str,
) -> Vec<String> {
    let tool = tools.iter().find(|t| t.function.name == tool_name).unwrap();
    let props = tool.function.parameters.properties.as_ref().unwrap();
    let le = props.get("linked_entities").unwrap();
    le.items
        .as_ref()
        .unwrap()
        .enum_values
        .clone()
        .unwrap_or_default()
}

// =============================================================================
// Free-text query param always present
// =============================================================================

#[test]
fn test_registry_query_param_on_all_tools() {
    let registry = OntologyRegistry::build();
    let tools = registry.build_query_tools(&empty_user_data());

    for tool in &tools {
        assert!(
            has_property(&tools, &tool.function.name, "query"),
            "Tool {} should have a 'query' parameter",
            tool.function.name
        );
    }
}
