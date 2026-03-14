use std::sync::Arc;

use crate::models::ontology_models::PersonWithChannels;
use crate::AppState;

/// Handle a query_* ontology tool call. Returns formatted text for the LLM.
pub async fn handle_query(
    tool_name: &str,
    args: &str,
    state: &Arc<AppState>,
    user_id: i32,
) -> Result<String, String> {
    let entity_type = tool_name
        .strip_prefix("query_")
        .ok_or_else(|| format!("Invalid ontology tool name: {}", tool_name))?;

    let params: serde_json::Value =
        serde_json::from_str(args).map_err(|e| format!("Invalid arguments: {}", e))?;

    match entity_type {
        "person" => query_person(&params, state, user_id),
        "channel" => query_channel(&params, state, user_id),
        "item" => query_item(&params, state, user_id),
        _ => Err(format!("Unknown ontology entity type: {}", entity_type)),
    }
}

fn query_person(
    params: &serde_json::Value,
    state: &Arc<AppState>,
    user_id: i32,
) -> Result<String, String> {
    let name_filter = param_str(params, "name");
    let query_filter = param_str(params, "query");
    let linked = param_str_array(params, "linked_entities");

    if name_filter.is_none() && query_filter.is_none() {
        return Err(
            "Please specify a 'name' or 'query' parameter to search for persons.".to_string(),
        );
    }

    let persons: Vec<PersonWithChannels> = if let Some(name) = &name_filter {
        if name == "all" {
            state
                .ontology_repository
                .get_persons_with_channels(user_id)
                .map_err(|e| format!("Failed to query persons: {}", e))?
        } else {
            match state
                .ontology_repository
                .find_person_by_name(user_id, name)
                .map_err(|e| format!("Failed to query person: {}", e))?
            {
                Some(p) => vec![p],
                None => vec![],
            }
        }
    } else if let Some(q) = &query_filter {
        state
            .ontology_repository
            .search_persons(user_id, q)
            .map_err(|e| format!("Failed to search persons: {}", e))?
    } else {
        vec![]
    };

    // Apply keyword filter if both name and query are specified
    let persons = if name_filter.is_some() && query_filter.is_some() {
        let q = query_filter.as_ref().unwrap().to_lowercase();
        persons
            .into_iter()
            .filter(|p| {
                p.person.name.to_lowercase().contains(&q)
                    || p.display_name().to_lowercase().contains(&q)
            })
            .collect()
    } else {
        persons
    };

    if persons.is_empty() {
        return Ok("No persons found matching your query.".to_string());
    }

    let want_items = linked.contains(&"Item".to_string());
    let want_channels = linked.contains(&"Channel".to_string());

    let mut output = String::new();
    for p in &persons {
        output.push_str(&format!("Person: {}\n", p.display_name()));

        // Always show channels inline for person queries
        if !p.channels.is_empty() || want_channels {
            output.push_str("Channels:\n");
            for ch in &p.channels {
                let handle_str = ch
                    .handle
                    .as_deref()
                    .map(|h| format!(": {}", h))
                    .unwrap_or_default();
                output.push_str(&format!(
                    "  - {}{} (notification: {})\n",
                    ch.platform, handle_str, ch.notification_mode
                ));
            }
        }

        // Linked items (only if explicitly requested)
        if want_items {
            match state
                .ontology_repository
                .get_linked_items_for_person(user_id, p.person.id)
            {
                Ok(linked_items) if !linked_items.is_empty() => {
                    output.push_str("Linked Items:\n");
                    for (link, item) in &linked_items {
                        let due = item
                            .due_at
                            .map(|d| format!(", due: {}", format_timestamp(d)))
                            .unwrap_or_default();
                        output.push_str(&format!(
                            "  - \"{}\" (priority: {}{}) [{}]\n",
                            item.summary, item.priority, due, link.link_type
                        ));
                    }
                }
                Ok(_) => {
                    output.push_str("Linked Items: none\n");
                }
                Err(_) => {}
            }
        }

        output.push('\n');
    }

    Ok(output.trim().to_string())
}

fn query_channel(
    params: &serde_json::Value,
    state: &Arc<AppState>,
    user_id: i32,
) -> Result<String, String> {
    let platform_filter = param_str(params, "platform");
    let person_name_filter = param_str(params, "person_name");
    let notif_filter = param_str(params, "notification_mode");
    let query_filter = param_str(params, "query");
    let linked = param_str_array(params, "linked_entities");

    if platform_filter.is_none()
        && person_name_filter.is_none()
        && notif_filter.is_none()
        && query_filter.is_none()
    {
        return Err(
            "Please specify at least one filter parameter (platform, person_name, notification_mode, or query).".to_string()
        );
    }

    // Load all persons with channels, then filter
    let all_persons = state
        .ontology_repository
        .get_persons_with_channels(user_id)
        .map_err(|e| format!("Failed to query channels: {}", e))?;

    let mut results: Vec<(
        &PersonWithChannels,
        &crate::models::ontology_models::OntChannel,
    )> = Vec::new();

    for p in &all_persons {
        // Filter by person_name
        if let Some(ref pn) = person_name_filter {
            if pn != "all" && p.display_name().to_lowercase() != pn.to_lowercase() {
                continue;
            }
        }

        for ch in &p.channels {
            // Filter by platform
            if let Some(ref plat) = platform_filter {
                if plat != "all" && ch.platform != *plat {
                    continue;
                }
            }

            // Filter by notification_mode
            if let Some(ref nm) = notif_filter {
                if nm != "all" && ch.notification_mode != *nm {
                    continue;
                }
            }

            // Filter by query
            if let Some(ref q) = query_filter {
                let q_lower = q.to_lowercase();
                let matches = ch.platform.to_lowercase().contains(&q_lower)
                    || ch
                        .handle
                        .as_ref()
                        .map(|h| h.to_lowercase().contains(&q_lower))
                        .unwrap_or(false)
                    || p.display_name().to_lowercase().contains(&q_lower);
                if !matches {
                    continue;
                }
            }

            results.push((p, ch));
        }
    }

    if results.is_empty() {
        return Ok("No channels found matching your query.".to_string());
    }

    let want_person = linked.contains(&"Person".to_string());

    let mut output = String::new();
    for (person, ch) in &results {
        let handle_str = ch
            .handle
            .as_deref()
            .map(|h| format!(": {}", h))
            .unwrap_or_default();
        output.push_str(&format!(
            "Channel: {}{} (notification: {}, person: {})\n",
            ch.platform,
            handle_str,
            ch.notification_mode,
            person.display_name()
        ));

        if want_person {
            output.push_str(&format!(
                "  Parent Person: {} (channels: {})\n",
                person.display_name(),
                person.channels.len()
            ));
        }
    }

    Ok(output.trim().to_string())
}

fn query_item(
    params: &serde_json::Value,
    state: &Arc<AppState>,
    user_id: i32,
) -> Result<String, String> {
    let priority_filter = param_str(params, "priority");
    let query_filter = param_str(params, "query");
    let linked = param_str_array(params, "linked_entities");

    if priority_filter.is_none() && query_filter.is_none() {
        return Err(
            "Please specify a 'priority' or 'query' parameter to search for items.".to_string(),
        );
    }

    let all_items = state
        .item_repository
        .get_items(user_id)
        .map_err(|e| format!("Failed to query items: {}", e))?;

    let mut items: Vec<&crate::pg_models::PgItem> = Vec::new();

    for item in &all_items {
        // Filter by priority
        if let Some(ref prio) = priority_filter {
            if prio != "all" {
                if let Ok(p) = prio.parse::<i32>() {
                    if item.priority != p {
                        continue;
                    }
                }
            }
        }

        // Filter by query
        if let Some(ref q) = query_filter {
            if !item.summary.to_lowercase().contains(&q.to_lowercase()) {
                continue;
            }
        }

        items.push(item);
    }

    if items.is_empty() {
        return Ok("No items found matching your query.".to_string());
    }

    let want_person = linked.contains(&"Person".to_string());

    let mut output = String::new();
    for item in &items {
        let due = item
            .due_at
            .map(|d| format!(", due: {}", format_timestamp(d)))
            .unwrap_or_default();
        output.push_str(&format!(
            "Item: \"{}\" (priority: {}{})\n",
            item.summary, item.priority, due
        ));

        if want_person {
            match state
                .ontology_repository
                .get_linked_persons_for_item(user_id, item.id)
            {
                Ok(linked_persons) if !linked_persons.is_empty() => {
                    output.push_str("Linked Persons:\n");
                    for (link, person) in &linked_persons {
                        output.push_str(&format!("  - {} [{}]\n", person.name, link.link_type));
                    }
                }
                Ok(_) => {
                    output.push_str("Linked Persons: none\n");
                }
                Err(_) => {}
            }
        }
    }

    Ok(output.trim().to_string())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn param_str(params: &serde_json::Value, key: &str) -> Option<String> {
    params
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn param_str_array(params: &serde_json::Value, key: &str) -> Vec<String> {
    params
        .get(key)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn format_timestamp(ts: i32) -> String {
    use chrono::{DateTime, Utc};
    DateTime::<Utc>::from_timestamp(ts as i64, 0)
        .map(|dt| dt.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| ts.to_string())
}
