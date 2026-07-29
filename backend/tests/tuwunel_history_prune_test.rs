use std::collections::{HashMap, HashSet};

use backend::utils::tuwunel_history_prune::{
    apply_state_diff_chain, is_historical_state_candidate, parse_state_diff,
    parse_state_event_metadata, protected_auth_closure, short_event_id, timeline_index_key,
};

fn compressed(short_state_key: u64, short_event_id: u64) -> [u8; 16] {
    let mut value = [0; 16];
    value[..8].copy_from_slice(&short_state_key.to_be_bytes());
    value[8..].copy_from_slice(&short_event_id.to_be_bytes());
    value
}

#[test]
fn parses_and_applies_tuwunel_state_diff_chain() {
    let first = compressed(11, 101);
    let replaced = compressed(12, 102);
    let replacement = compressed(12, 103);

    let mut root_value = 0_u64.to_be_bytes().to_vec();
    root_value.extend_from_slice(&first);
    root_value.extend_from_slice(&replaced);
    let root = parse_state_diff(&root_value).unwrap();

    let mut child_value = 55_u64.to_be_bytes().to_vec();
    child_value.extend_from_slice(&replacement);
    child_value.extend_from_slice(&0_u64.to_be_bytes());
    child_value.extend_from_slice(&replaced);
    let child = parse_state_diff(&child_value).unwrap();

    let state = apply_state_diff_chain(&[root, child]);
    assert_eq!(
        state.into_iter().collect::<Vec<_>>(),
        vec![first, replacement]
    );
    assert_eq!(short_event_id(&replacement), 103);
}

#[test]
fn malformed_state_diff_is_rejected_before_cleanup() {
    let mut value = 1_u64.to_be_bytes().to_vec();
    value.extend_from_slice(&[7; 9]);
    assert!(parse_state_diff(&value).is_err());
}

#[test]
fn current_state_auth_closure_is_always_protected() {
    let current = parse_state_event_metadata(
        br#"{"event_id":"$current","room_id":"!r:x","state_key":"","origin_server_ts":3000,"auth_events":["$auth"]}"#,
        None,
    )
    .unwrap()
    .unwrap();
    let auth = parse_state_event_metadata(
        br#"{"event_id":"$auth","room_id":"!r:x","state_key":"","origin_server_ts":1000,"auth_events":["$create"]}"#,
        None,
    )
    .unwrap()
    .unwrap();
    let create = parse_state_event_metadata(
        br#"{"event_id":"$create","room_id":"!r:x","state_key":"","origin_server_ts":500,"auth_events":[]}"#,
        None,
    )
    .unwrap()
    .unwrap();
    let historical = parse_state_event_metadata(
        br#"{"event_id":"$old","room_id":"!r:x","state_key":"@old:x","origin_server_ts":500,"auth_events":["$create"]}"#,
        None,
    )
    .unwrap()
    .unwrap();

    let events = [current, auth, create, historical.clone()]
        .into_iter()
        .map(|event| (event.event_id.clone(), event))
        .collect::<HashMap<_, _>>();
    let protected =
        protected_auth_closure(&HashSet::from(["$current".to_owned()]), &events).unwrap();

    assert!(protected.contains("$current"));
    assert!(protected.contains("$auth"));
    assert!(protected.contains("$create"));
    assert!(!protected.contains("$old"));
    assert!(is_historical_state_candidate(
        &historical,
        &protected,
        2_000
    ));
}

#[test]
fn missing_auth_payload_fails_closed() {
    let current = parse_state_event_metadata(
        br#"{"event_id":"$current","room_id":"!r:x","state_key":"","origin_server_ts":3000,"auth_events":["$missing"]}"#,
        None,
    )
    .unwrap()
    .unwrap();
    let events = HashMap::from([(current.event_id.clone(), current)]);
    assert!(protected_auth_closure(&HashSet::from(["$current".to_owned()]), &events).is_err());
}

#[test]
fn timeline_index_key_matches_tuwunel_tuple_serialization() {
    let count = (-7_i64).to_be_bytes();
    let key = timeline_index_key("!room:example.com", 1234, count);
    let expected_bias = (-7_i64).wrapping_sub(i64::MIN) as u64;

    let mut expected = b"!room:example.com\xFF".to_vec();
    expected.extend_from_slice(&1234_u64.to_be_bytes());
    expected.push(0xFF);
    expected.extend_from_slice(&expected_bias.to_be_bytes());
    assert_eq!(key, expected);
}

#[test]
fn non_state_events_are_not_cleanup_candidates() {
    assert!(parse_state_event_metadata(
        br#"{"event_id":"$message","origin_server_ts":1,"auth_events":[]}"#,
        None,
    )
    .unwrap()
    .is_none());
}
