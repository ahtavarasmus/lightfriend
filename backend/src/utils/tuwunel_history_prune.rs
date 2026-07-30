use std::collections::{BTreeSet, HashMap, HashSet};

use anyhow::{bail, Context, Result};

pub const COMPRESSED_STATE_EVENT_BYTES: usize = 16;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateDiff {
    pub parent: Option<u64>,
    pub added: Vec<[u8; COMPRESSED_STATE_EVENT_BYTES]>,
    pub removed: Vec<[u8; COMPRESSED_STATE_EVENT_BYTES]>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateEventMetadata {
    pub event_id: String,
    pub room_id: Option<String>,
    pub origin_server_ts: u64,
    pub auth_events: Vec<String>,
}

pub fn parse_state_diff(value: &[u8]) -> Result<StateDiff> {
    if value.len() < 8 {
        bail!("state diff is shorter than its parent hash");
    }

    let parent_raw = u64::from_be_bytes(value[..8].try_into().expect("eight-byte parent hash"));
    let parent = (parent_raw != 0).then_some(parent_raw);
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut offset = 8;
    let mut reading_removed = false;

    while offset < value.len() {
        if !reading_removed && value.get(offset..offset + 8) == Some(&[0; 8]) {
            reading_removed = true;
            offset += 8;
            continue;
        }

        let end = offset + COMPRESSED_STATE_EVENT_BYTES;
        let compressed: [u8; COMPRESSED_STATE_EVENT_BYTES] = value
            .get(offset..end)
            .with_context(|| format!("state diff has a truncated event at byte {offset}"))?
            .try_into()
            .expect("validated compressed state event length");
        if compressed[..8] == [0; 8] {
            bail!("state diff contains an invalid zero short-state-key");
        }
        if reading_removed {
            removed.push(compressed);
        } else {
            added.push(compressed);
        }
        offset = end;
    }

    Ok(StateDiff {
        parent,
        added,
        removed,
    })
}

pub fn apply_state_diff_chain(
    root_to_current: &[StateDiff],
) -> BTreeSet<[u8; COMPRESSED_STATE_EVENT_BYTES]> {
    let mut state = BTreeSet::new();
    for diff in root_to_current {
        state.extend(diff.added.iter().copied());
        for removed in &diff.removed {
            state.remove(removed);
        }
    }
    state
}

pub fn short_event_id(compressed: &[u8; COMPRESSED_STATE_EVENT_BYTES]) -> u64 {
    u64::from_be_bytes(
        compressed[8..]
            .try_into()
            .expect("compressed state event contains an eight-byte event id"),
    )
}

pub fn parse_state_event_metadata(
    value: &[u8],
    fallback_event_id: Option<&str>,
) -> Result<Option<StateEventMetadata>> {
    let pdu: serde_json::Value =
        serde_json::from_slice(value).context("PDU payload is not valid JSON")?;
    let object = pdu
        .as_object()
        .context("PDU payload is not a JSON object")?;

    let Some(state_key) = object.get("state_key") else {
        return Ok(None);
    };
    if !state_key.is_string() {
        bail!("state event has a non-string state_key");
    }

    let event_id = object
        .get("event_id")
        .and_then(serde_json::Value::as_str)
        .or(fallback_event_id)
        .context("state event has no event_id")?;
    let room_id = object
        .get("room_id")
        .map(|value| {
            value
                .as_str()
                .context("state event has a non-string room_id")
                .map(str::to_owned)
        })
        .transpose()?;
    let origin_server_ts = object
        .get("origin_server_ts")
        .and_then(serde_json::Value::as_u64)
        .context("state event has no unsigned origin_server_ts")?;
    let auth_events = object
        .get("auth_events")
        .and_then(serde_json::Value::as_array)
        .context("state event has no auth_events array")?
        .iter()
        .map(|value| {
            value
                .as_str()
                .context("state event auth_events contains a non-string value")
                .map(str::to_owned)
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(Some(StateEventMetadata {
        event_id: event_id.to_owned(),
        room_id,
        origin_server_ts,
        auth_events,
    }))
}

pub fn protected_auth_closure(
    current_state: &HashSet<String>,
    state_events: &HashMap<String, StateEventMetadata>,
) -> Result<HashSet<String>> {
    let (protected, missing) = protected_auth_closure_allowing_missing(current_state, state_events);
    if let Some(event_id) = missing.iter().next() {
        bail!("protected state event payload is missing: {event_id}");
    }

    Ok(protected)
}

pub fn protected_auth_closure_allowing_missing(
    current_state: &HashSet<String>,
    state_events: &HashMap<String, StateEventMetadata>,
) -> (HashSet<String>, HashSet<String>) {
    let mut protected = current_state.clone();
    let mut pending = current_state.iter().cloned().collect::<Vec<_>>();
    let mut missing = HashSet::new();

    while let Some(event_id) = pending.pop() {
        let Some(event) = state_events.get(&event_id) else {
            missing.insert(event_id);
            continue;
        };
        for auth_event_id in &event.auth_events {
            if protected.insert(auth_event_id.clone()) {
                pending.push(auth_event_id.clone());
            }
        }
    }

    (protected, missing)
}

pub fn is_historical_state_candidate(
    event: &StateEventMetadata,
    protected: &HashSet<String>,
    cutoff_millis: u64,
) -> bool {
    event.origin_server_ts < cutoff_millis && !protected.contains(&event.event_id)
}

pub fn pdu_count_bytes(pdu_key: &[u8]) -> Result<[u8; 8]> {
    match pdu_key.len() {
        16 | 24 => Ok(pdu_key[pdu_key.len() - 8..]
            .try_into()
            .expect("validated PDU key count length")),
        length => bail!("unexpected PDU key length {length}"),
    }
}

pub fn timeline_index_key(room_id: &str, origin_server_ts: u64, count: [u8; 8]) -> Vec<u8> {
    let biased_count = i64::from_be_bytes(count).wrapping_sub(i64::MIN) as u64;
    let mut key = Vec::with_capacity(room_id.len() + 18);
    key.extend_from_slice(room_id.as_bytes());
    key.push(0xFF);
    key.extend_from_slice(&origin_server_ts.to_be_bytes());
    key.push(0xFF);
    key.extend_from_slice(&biased_count.to_be_bytes());
    key
}
