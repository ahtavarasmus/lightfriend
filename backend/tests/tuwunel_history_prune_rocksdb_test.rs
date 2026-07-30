#![cfg(feature = "tuwunel-restore")]

use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use backend::utils::tuwunel_history_prune::timeline_index_key;
use rocksdb::{ColumnFamilyDescriptor, DBWithThreadMode, MultiThreaded, Options};

type RocksDb = DBWithThreadMode<MultiThreaded>;

const COLUMNS: &[&str] = &[
    "default",
    "roomid_shortstatehash",
    "shortstatehash_statediff",
    "shorteventid_eventid",
    "pduid_pdu",
    "eventid_pduid",
    "eventid_outlierpdu",
    "eventid_originalpdu",
    "roomid_tscount_pducount",
    "userdeviceid_token",
];

fn pdu_key(count: i64) -> Vec<u8> {
    let mut key = 1_u64.to_be_bytes().to_vec();
    key.extend_from_slice(&count.to_be_bytes());
    key
}

fn compressed(short_state_key: u64, short_event_id: u64) -> [u8; 16] {
    let mut value = [0; 16];
    value[..8].copy_from_slice(&short_state_key.to_be_bytes());
    value[8..].copy_from_slice(&short_event_id.to_be_bytes());
    value
}

fn open_database(path: &std::path::Path, create: bool) -> RocksDb {
    let mut options = Options::default();
    options.create_if_missing(create);
    options.create_missing_column_families(create);
    let descriptors = COLUMNS
        .iter()
        .map(|name| ColumnFamilyDescriptor::new(*name, Options::default()));
    RocksDb::open_cf_descriptors(&options, path, descriptors).unwrap()
}

fn put_timeline_pdu(db: &RocksDb, event_id: &str, count: i64, value: &[u8]) {
    let pduid_pdu = db.cf_handle("pduid_pdu").unwrap();
    let eventid_pduid = db.cf_handle("eventid_pduid").unwrap();
    let timeline_index = db.cf_handle("roomid_tscount_pducount").unwrap();
    let key = pdu_key(count);
    let pdu: serde_json::Value = serde_json::from_slice(value).unwrap();
    let timestamp = pdu["origin_server_ts"].as_u64().unwrap();

    db.put_cf(&pduid_pdu, &key, value).unwrap();
    db.put_cf(&eventid_pduid, event_id.as_bytes(), &key)
        .unwrap();
    db.put_cf(
        &timeline_index,
        timeline_index_key("!room:example.com", timestamp, count.to_be_bytes()),
        count.to_be_bytes(),
    )
    .unwrap();
}

#[test]
fn offline_prune_removes_only_unprotected_historical_state_payloads() {
    let test_id = uuid::Uuid::new_v4();
    let database_path = std::env::temp_dir().join(format!("tuwunel-prune-{test_id}"));
    let status_path = std::env::temp_dir().join(format!("tuwunel-prune-{test_id}.json"));
    let db = open_database(&database_path, true);

    let room_state = db.cf_handle("roomid_shortstatehash").unwrap();
    let state_diffs = db.cf_handle("shortstatehash_statediff").unwrap();
    let short_events = db.cf_handle("shorteventid_eventid").unwrap();
    let outliers = db.cf_handle("eventid_outlierpdu").unwrap();
    let originals = db.cf_handle("eventid_originalpdu").unwrap();
    let credentials = db.cf_handle("userdeviceid_token").unwrap();

    db.put_cf(&room_state, b"!room:example.com", 2_u64.to_be_bytes())
        .unwrap();
    let mut retained_state_diff = 0_u64.to_be_bytes().to_vec();
    retained_state_diff.extend_from_slice(&compressed(1, 103));
    retained_state_diff.extend_from_slice(&compressed(2, 102));
    db.put_cf(&state_diffs, 1_u64.to_be_bytes(), retained_state_diff)
        .unwrap();
    let mut current_state_diff = 1_u64.to_be_bytes().to_vec();
    current_state_diff.extend_from_slice(&compressed(1, 101));
    current_state_diff.extend_from_slice(&[0; 8]);
    current_state_diff.extend_from_slice(&compressed(1, 103));
    db.put_cf(&state_diffs, 2_u64.to_be_bytes(), current_state_diff)
        .unwrap();
    db.put_cf(&short_events, 101_u64.to_be_bytes(), b"$current")
        .unwrap();
    db.put_cf(&short_events, 102_u64.to_be_bytes(), b"$create")
        .unwrap();
    db.put_cf(&short_events, 103_u64.to_be_bytes(), b"$old-state")
        .unwrap();
    db.put_cf(
        &credentials,
        b"@user:example.com\xFFDEVICE",
        b"secret-token",
    )
    .unwrap();

    put_timeline_pdu(
        &db,
        "$create",
        1,
        br#"{"event_id":"$create","room_id":"!room:example.com","state_key":"","origin_server_ts":1,"auth_events":[]}"#,
    );
    put_timeline_pdu(
        &db,
        "$current",
        2,
        br#"{"event_id":"$current","room_id":"!room:example.com","state_key":"","origin_server_ts":2,"auth_events":["$create"]}"#,
    );
    put_timeline_pdu(
        &db,
        "$old-state",
        3,
        br#"{"event_id":"$old-state","room_id":"!room:example.com","state_key":"@old:example.com","origin_server_ts":3,"auth_events":["$create"]}"#,
    );
    put_timeline_pdu(
        &db,
        "$orphan-state",
        6,
        br#"{"event_id":"$orphan-state","room_id":"!room:example.com","state_key":"@orphan:example.com","origin_server_ts":3,"auth_events":["$create"]}"#,
    );
    put_timeline_pdu(
        &db,
        "$message",
        4,
        br#"{"event_id":"$message","room_id":"!room:example.com","origin_server_ts":4,"auth_events":["$create"]}"#,
    );
    let fresh_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    put_timeline_pdu(
        &db,
        "$fresh-state",
        5,
        format!(
            r#"{{"event_id":"$fresh-state","room_id":"!room:example.com","state_key":"@fresh:example.com","origin_server_ts":{fresh_timestamp},"auth_events":["$create"]}}"#
        )
        .as_bytes(),
    );
    db.put_cf(
        &outliers,
        b"$old-outlier",
        br#"{"event_id":"$old-outlier","room_id":"!room:example.com","state_key":"@outlier:example.com","origin_server_ts":3,"auth_events":["$create"]}"#,
    )
    .unwrap();
    db.put_cf(
        &originals,
        b"$orphan-state",
        br#"{"event_id":"$orphan-state","state_key":"@orphan:example.com"}"#,
    )
    .unwrap();
    db.put_cf(
        &originals,
        b"$old-state",
        br#"{"event_id":"$old-state","state_key":"@old:example.com"}"#,
    )
    .unwrap();
    drop(room_state);
    drop(state_diffs);
    drop(short_events);
    drop(outliers);
    drop(originals);
    drop(credentials);
    drop(db);

    let output = Command::new(env!("CARGO_BIN_EXE_tuwunel_prune_history"))
        .arg(&database_path)
        .arg(&status_path)
        .env("TUWUNEL_HISTORICAL_STATE_PRUNE_RETENTION_SECS", "60")
        .env("TUWUNEL_HISTORICAL_STATE_COMPACTION_ENABLED", "false")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let db = open_database(&database_path, false);
    let pduid_pdu = db.cf_handle("pduid_pdu").unwrap();
    let eventid_pduid = db.cf_handle("eventid_pduid").unwrap();
    let timeline_index = db.cf_handle("roomid_tscount_pducount").unwrap();
    let outliers = db.cf_handle("eventid_outlierpdu").unwrap();
    let originals = db.cf_handle("eventid_originalpdu").unwrap();
    let credentials = db.cf_handle("userdeviceid_token").unwrap();

    assert!(db.get_cf(&pduid_pdu, pdu_key(6)).unwrap().is_none());
    assert!(db
        .get_cf(&eventid_pduid, b"$orphan-state")
        .unwrap()
        .is_none());
    assert!(db
        .get_cf(
            &timeline_index,
            timeline_index_key("!room:example.com", 3, 6_i64.to_be_bytes())
        )
        .unwrap()
        .is_none());
    assert!(db.get_cf(&outliers, b"$old-outlier").unwrap().is_none());
    assert!(db.get_cf(&originals, b"$orphan-state").unwrap().is_none());

    assert!(db.get_cf(&pduid_pdu, pdu_key(1)).unwrap().is_some());
    assert!(db.get_cf(&pduid_pdu, pdu_key(2)).unwrap().is_some());
    assert!(db.get_cf(&pduid_pdu, pdu_key(3)).unwrap().is_some());
    assert!(db.get_cf(&eventid_pduid, b"$old-state").unwrap().is_some());
    assert!(db.get_cf(&originals, b"$old-state").unwrap().is_some());
    assert!(db.get_cf(&pduid_pdu, pdu_key(4)).unwrap().is_some());
    assert!(db.get_cf(&pduid_pdu, pdu_key(5)).unwrap().is_some());
    assert_eq!(
        db.get_cf(&credentials, b"@user:example.com\xFFDEVICE")
            .unwrap()
            .unwrap(),
        b"secret-token"
    );

    let status: serde_json::Value =
        serde_json::from_slice(&fs::read(&status_path).unwrap()).unwrap();
    assert_eq!(status["status"], "success");
    assert_eq!(status["historical_state_events_deleted"], 2);
    assert_eq!(status["timeline_payloads_deleted"], 1);
    assert_eq!(status["outlier_payloads_deleted"], 1);
    assert_eq!(status["current_state_events_preserved"], 2);
    assert_eq!(status["retained_state_events_preserved"], 3);
    assert_eq!(status["state_hashes_loaded"], 2);
    assert_eq!(status["compaction_status"], "disabled");

    drop(pduid_pdu);
    drop(eventid_pduid);
    drop(timeline_index);
    drop(outliers);
    drop(originals);
    drop(credentials);
    drop(db);
    fs::remove_dir_all(database_path).unwrap();
    fs::remove_file(status_path).unwrap();
}
