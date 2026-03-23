use std::sync::Arc;

use crate::models::ontology_models::{OntEvent, OntMessage, PersonWithChannels};
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
        "message" => query_message(&params, state, user_id),
        "event" => query_event(&params, state, user_id),
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
            "Please specify a 'name' or 'query' parameter to search for people.".to_string(),
        );
    }

    let persons: Vec<PersonWithChannels> = if let Some(name) = &name_filter {
        if name == "all" {
            state
                .ontology_repository
                .get_persons_with_channels(user_id, 500, 0)
                .map_err(|e| format!("Failed to query people: {}", e))?
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
            .map_err(|e| format!("Failed to search people: {}", e))?
    } else {
        vec![]
    };

    // Apply keyword filter if both name and query are specified
    let persons = if let (Some(_), Some(ref q_ref)) = (&name_filter, &query_filter) {
        let q = q_ref.to_lowercase();
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
        return Ok("No people found matching your query.".to_string());
    }

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
        .get_persons_with_channels(user_id, 500, 0)
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

fn query_message(
    params: &serde_json::Value,
    state: &Arc<AppState>,
    user_id: i32,
) -> Result<String, String> {
    let platform_filter = param_str(params, "platform");
    let sender_filter = param_str(params, "sender_name");
    let query_filter = param_str(params, "query");

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;
    let twelve_hours_ago = now - 12 * 3600;

    let plat = platform_filter.as_deref().filter(|p| *p != "all");
    let messages: Vec<OntMessage> = state
        .ontology_repository
        .get_recent_messages_filtered(user_id, plat, twelve_hours_ago, 20)
        .map_err(|e| format!("Failed to query messages: {}", e))?;

    // Apply sender_name filter
    let messages: Vec<OntMessage> = if let Some(ref sender) = sender_filter {
        if sender != "all" {
            let s_lower = sender.to_lowercase();
            messages
                .into_iter()
                .filter(|m| m.sender_name.to_lowercase().contains(&s_lower))
                .collect()
        } else {
            messages
        }
    } else {
        messages
    };

    // Apply query (free-text keyword) filter
    let messages: Vec<OntMessage> = if let Some(ref q) = query_filter {
        let q_lower = q.to_lowercase();
        messages
            .into_iter()
            .filter(|m| {
                m.sender_name.to_lowercase().contains(&q_lower)
                    || m.content.to_lowercase().contains(&q_lower)
                    || m.platform.to_lowercase().contains(&q_lower)
            })
            .collect()
    } else {
        messages
    };

    let messages: Vec<&OntMessage> = messages.iter().take(20).collect();

    if messages.is_empty() {
        return Ok("No messages found matching your query.".to_string());
    }

    let mut output = format!("Found {} message(s):\n\n", messages.len());

    for (i, m) in messages.iter().enumerate() {
        let content_preview: String = m.content.chars().take(100).collect();
        output.push_str(&format!(
            "{}. [id={}] {} via {} - \"{}\"",
            i + 1,
            m.id,
            m.sender_name,
            m.platform,
            content_preview
        ));

        if i + 1 < messages.len() {
            output.push_str("\n\n");
        }
    }

    Ok(output)
}

fn query_event(
    params: &serde_json::Value,
    state: &Arc<AppState>,
    user_id: i32,
) -> Result<String, String> {
    let status_filter = param_str(params, "status");
    let query_filter = param_str(params, "query");

    let events: Vec<OntEvent> = state
        .ontology_repository
        .get_events(user_id, status_filter.as_deref())
        .map_err(|e| format!("Failed to query events: {}", e))?;

    // Apply free-text filter
    let events: Vec<OntEvent> = if let Some(ref q) = query_filter {
        let q_lower = q.to_lowercase();
        events
            .into_iter()
            .filter(|e| e.description.to_lowercase().contains(&q_lower))
            .collect()
    } else {
        events
    };

    if events.is_empty() {
        return Ok("No tracked obligations found.".to_string());
    }

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i32;

    let mut output = format!("Found {} event(s):\n\n", events.len());

    for (i, e) in events.iter().enumerate() {
        let deadline_str = match e.due_at {
            Some(ts) => {
                let remaining = ts - now;
                let days = remaining / 86400;
                if remaining < 0 {
                    " [OVERDUE]".to_string()
                } else if days == 0 {
                    let hours = (remaining / 3600).max(1);
                    format!(" [due in {} hours]", hours)
                } else {
                    format!(" [due in {} days]", days)
                }
            }
            None => String::new(),
        };

        output.push_str(&format!(
            "{}. [event_id={}] [status={}]{} \"{}\"",
            i + 1,
            e.id,
            e.status,
            deadline_str,
            e.description
        ));

        if i + 1 < events.len() {
            output.push_str("\n\n");
        }
    }

    Ok(output)
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
